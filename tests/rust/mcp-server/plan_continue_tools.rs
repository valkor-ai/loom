use delivery_core::{InspectRequestInput, ReadRequestFieldsInput};
use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};

#[test]
fn plan_returns_user_gate_and_creates_brainstorm_delivery() {
    let fixture = Fixture::new("plan-tool");
    let server = LoomMcpServer::default();

    let result = server
        .invoke_tool(
            "loom.plan",
            Some(args(json!({
                "projectRoot": fixture.root_str(),
                "requestText": "实现证券账户开户流程"
            }))),
        )
        .expect("plan call");
    let value = structured(result);

    assert_eq!(value["state"], "user_gate");
    assert_eq!(value["gate"]["currentBlock"], "phase_scope");
    let request_ref = value["requestRef"].as_str().expect("requestRef");
    assert_eq!(count_entries(&fixture.root.join(".loom/deliveries")), 1);
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request");
    let group_ids = inspected
        .read_groups
        .iter()
        .map(|group| group.group_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        group_ids,
        vec![
            "conversation_protocol",
            "requirement_context",
            "requirement_full_text",
            "phase_scope_rules",
            "knowledge_context_plan",
            "concept_grounding_rules",
            "frontend_experience_rules",
            "final_summary_rules",
            "candidate_write_contract",
        ]
    );
    let requirement_context = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "requirement_context")
        .expect("requirement_context group");
    assert!(
        !requirement_context
            .fields
            .contains(&"requirementContext.normalizedText".to_string()),
        "default requirement_context group must stay compact"
    );
    let full_text = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "requirement_full_text")
        .expect("requirement_full_text group");
    assert!(!full_text.required);
    assert_eq!(
        full_text.fields,
        vec!["requirementContext.normalizedText".to_string()]
    );
    let knowledge_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "clarificationConversationProtocol.internalTermRule".to_string(),
            "clarificationConversationProtocol.userVisibleBlockNames".to_string(),
            "outputContract.resultTemplate".to_string(),
            "knowledgeQueryPlan.toolContract".to_string(),
            "knowledgeQueryPlan.sharedRules".to_string(),
        ],
    })
    .expect("knowledge query plan fields");
    assert!(
        knowledge_fields.fields["clarificationConversationProtocol.internalTermRule"]
            .value
            .as_str()
            .expect("internal term rule")
            .contains("never ask the user to confirm internal block ids")
    );
    assert_eq!(
        knowledge_fields.fields["clarificationConversationProtocol.userVisibleBlockNames"].value
            ["concept_grounding"],
        "业务理解与规则确认"
    );
    assert!(
        knowledge_fields.fields["outputContract.resultTemplate"].value["clarificationProgress"]
            ["confirmedBlocks"]
            .is_array()
    );
    assert!(
        knowledge_fields.fields["outputContract.resultTemplate"].value["clarificationProgress"]
            .get("completedBlocks")
            .is_none()
    );
    assert_eq!(
        knowledge_fields.fields["knowledgeQueryPlan.toolContract"].value["contextTool"],
        "loom.knowledgeBrainstormContext"
    );
    assert!(knowledge_fields.fields["knowledgeQueryPlan.sharedRules"]
        .value
        .to_string()
        .contains("do not silently fall back"));
}

#[test]
fn continue_replays_current_brainstorm_gate_after_plan() {
    let fixture = Fixture::new("continue-after-plan");
    let server = LoomMcpServer::default();

    let planned = structured(
        server
            .invoke_tool(
                "loom.plan",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestText": "实现证券账户开户流程"
                }))),
            )
            .expect("plan call"),
    );
    let continued = structured(
        server
            .invoke_tool(
                "loom.continue",
                Some(args(json!({ "projectRoot": fixture.root_str() }))),
            )
            .expect("continue call"),
    );

    assert_eq!(continued["state"], "user_gate");
    assert_eq!(continued["requestRef"], planned["requestRef"]);
    assert_eq!(continued["gate"]["currentBlock"], "phase_scope");
}

#[test]
fn continue_blocks_after_init_when_no_active_delivery_exists() {
    let fixture = Fixture::new("continue-tool");
    let server = LoomMcpServer::default();

    server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("initProject");
    let result = server
        .invoke_tool(
            "loom.continue",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("continue");
    let value = structured(result);

    assert_eq!(value["state"], "blocked");
    assert_eq!(value["recommendedTool"], "loom.plan");
}

fn count_entries(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .expect("read dir")
        .filter_map(Result::ok)
        .count()
}

fn args(value: Value) -> rmcp::model::JsonObject {
    value.as_object().cloned().expect("json object args")
}

fn structured(result: rmcp::model::CallToolResult) -> Value {
    serde_json::to_value(result).expect("call result to value")["structuredContent"].clone()
}

struct Fixture {
    root: std::path::PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK.lock().expect("env lock");
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-plan-{name}-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        Self {
            root,
            _guard: guard,
        }
    }

    fn root_str(&self) -> &str {
        self.root.to_str().expect("fixture path utf8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
