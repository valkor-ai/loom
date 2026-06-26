use delivery_core::{InspectRequestInput, ReadRequestFieldsInput};
use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use state::{paths::from_project_relative, store::write_json_atomic};
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
    assert!(!value["prompt"]
        .as_str()
        .expect("prompt")
        .contains("phase_scope"));
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
fn brainstorm_full_confirmation_flow_accepts_and_advances_to_technical_baseline() {
    let fixture = Fixture::new("brainstorm-full-flow");
    let server = LoomMcpServer::default();

    let planned = structured(
        server
            .invoke_tool(
                "loom.plan",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestText": "基于股票交易系统实验大纲，第一阶段先确认证券账户模块闭环，并需要工作人员后台页面办理开户、挂失补办、销户。"
                }))),
            )
            .expect("plan call"),
    );
    assert_eq!(planned["state"], "user_gate");
    assert!(!planned["prompt"]
        .as_str()
        .expect("planned prompt")
        .contains("phase_scope"));
    let request_ref = planned["requestRef"].as_str().expect("requestRef");

    structured(
        server
            .invoke_tool(
                "loom.readFieldGroup",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "groupId": "conversation_protocol"
                }))),
            )
            .expect("read conversation protocol"),
    );
    let write_contract = structured(
        server
            .invoke_tool(
                "loom.readFieldGroup",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "groupId": "candidate_write_contract"
                }))),
            )
            .expect("read candidate write contract"),
    );
    assert!(
        write_contract["fields"]["outputContract.resultTemplate"]["clarificationProgress"]
            ["confirmedBlocks"]
            .is_array()
    );
    assert!(
        write_contract["fields"]["outputContract.resultTemplate"]["clarificationProgress"]
            .get("completedBlocks")
            .is_none()
    );
    let mut candidate = write_contract["fields"]["outputContract.resultTemplate"].clone();
    populate_confirmed_brainstorm_candidate(&mut candidate);

    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request");
    let target_path = inspected.write_targets[0]["path"]
        .as_str()
        .expect("candidate target path");
    let target_file =
        from_project_relative(&fixture.root, target_path).expect("candidate target absolute path");
    write_json_atomic(&target_file, &candidate).expect("write candidate");

    let accepted = structured(
        server
            .invoke_tool(
                "loom.brainstormAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "writtenTargetIds": ["candidate"]
                }))),
            )
            .expect("brainstorm accept"),
    );

    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_eq!(
        accepted["next"]["artifactKind"],
        "technical_baseline_candidate"
    );
    assert_ne!(
        accepted["gate"]["currentBlock"], "phase_scope",
        "accepted full confirmation must not reopen the phase scope gate"
    );
    let baseline_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("technical baseline request ref");
    let baseline = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: baseline_ref.to_string(),
    })
    .expect("inspect technical baseline request");
    assert_eq!(baseline.request_kind, "technical_baseline_request");
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
    assert!(!continued["prompt"]
        .as_str()
        .expect("continued prompt")
        .contains("phase_scope"));
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

fn populate_confirmed_brainstorm_candidate(candidate: &mut Value) {
    candidate["requestSummary"]["title"] = json!("证券账户模块闭环");
    candidate["requestSummary"]["oneLine"] = json!("第一阶段完成证券账户生命周期办理路径。");
    candidate["requestSummary"]["businessGoal"] = json!("先完成交易身份和持仓归属账户的闭环。");
    candidate["scope"]["included"][0]["label"] = json!("证券账户模块闭环");
    candidate["scope"]["included"][0]["items"] = json!(["开户", "挂失补办", "销户", "状态管理"]);
    candidate["scope"]["included"][0]["reason"] =
        json!("证券账户是资金账户和交易链路的上游基础对象。");
    candidate["roadmap"]["phases"][0]["title"] = json!("证券账户模块闭环");
    candidate["roadmap"]["phases"][0]["name"] = json!("证券账户模块闭环");
    candidate["roadmap"]["phases"][0]["goal"] = json!("完成证券账户生命周期办理能力。");
    candidate["phasePlan"]["current"]["title"] = json!("证券账户模块闭环");
    candidate["phasePlan"]["current"]["goal"] =
        json!("工作人员可以办理开户、挂失补办、销户并看到状态回读。");
    candidate["acceptance"][0]["statement"] =
        json!("工作人员可以完成证券账户开户、挂失补办、销户，并看到中文反馈。");
    candidate["domainModel"]["businessFlows"] = json!([
        {
            "id": "flow_open",
            "name": "证券账户开户",
            "actors": ["工作人员"],
            "capabilityRefs": ["scope_1"],
            "summary": "录入个人或法人资料，校验开户资格，生成证券账户号。"
        },
        {
            "id": "flow_close",
            "name": "证券账户销户",
            "actors": ["工作人员"],
            "capabilityRefs": ["scope_1"],
            "summary": "销户前校验持仓清空，满足后关闭账户。"
        }
    ]);
    candidate["conceptGrounding"]["phaseConceptGrounding"]["reason"] =
        json!("用户确认证券账户与资金账户边界、挂失冻结、销户清仓规则。");
    candidate["conceptConfirmation"]["confirmationSummary"] =
        json!("用户已确认证券账户业务边界和关键阻断规则。");
    candidate["frontendExperience"]["kind"] = json!("staff_admin_workspace");
    candidate["frontendExperience"]["audiences"] = json!([{ "audienceId": "aud_staff", "name": "工作人员", "primaryJobs": ["办理证券账户业务"] }]);
    candidate["frontendExperience"]["surfaces"] = json!([{ "surfaceId": "surface_account_management", "name": "证券账户管理页面", "audienceRefs": ["aud_staff"], "primaryJobs": ["查询", "开户", "挂失补办", "销户"] }]);
    candidate["frontendExperience"]["operationPaths"] = json!([{
        "pathId": "path_manage_account",
        "name": "证券账户管理办理路径",
        "userGoal": "工作人员通过列表查询目标账户，并办理挂失补办或销户；开户从新建入口进入。",
        "surfaceRef": "surface_account_management",
        "targetObject": "证券账户",
        "selectionMode": "query_and_select",
        "selectionSummary": "开户不依赖查询；挂失补办和销户先查询并选择目标账户。",
        "dataViewRefs": [],
        "actionRefs": [],
        "requiredStates": ["success", "business_blocking", "error"],
        "sourceRefs": []
    }]);
    candidate["frontendExperience"]["mustNot"] = json!(["不能只靠内部主键触发办理动作"]);
    candidate["frontendExperience"]["confirmationSummary"] =
        json!("用户已确认工作人员后台证券账户管理页面路径。");
    candidate["userConfirmation"]["confirmedAt"] = json!("2026-06-24T10:00:00+08:00");
    candidate["userConfirmation"]["confirmationSummary"] =
        json!("用户已确认阶段范围、业务理解、页面办理路径和提交前核对。");
    candidate["clarificationProgress"]["confirmedBlocks"][0]["summary"] =
        json!("确认第一阶段为证券账户模块闭环。");
    candidate["clarificationProgress"]["confirmedBlocks"][1]["summary"] =
        json!("确认证券账户业务规则、状态和边界。");
    candidate["clarificationProgress"]["confirmedBlocks"][2]["summary"] =
        json!("确认工作人员后台证券账户管理页面路径。");
}

struct Fixture {
    root: std::path::PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
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
