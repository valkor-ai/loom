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
                "requestText": "实现股票交易系统，请按模块依赖关系整理阶段优先级，每个阶段不要太大，优先按单模块能力闭环划分。"
            }))),
        )
        .expect("plan call");
    let value = structured(result);

    assert_eq!(value["state"], "user_gate");
    assert!(!value["prompt"]
        .as_str()
        .expect("prompt")
        .contains("phase_scope"));
    let prompt = value["prompt"].as_str().expect("prompt");
    assert!(prompt.contains("active phase boundary options"));
    assert!(!prompt.contains("phase-1 boundary"));
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
            "current_block_rules",
            "knowledge_context_plan",
            "block_confirmation_contract",
        ]
    );
    assert!(inspected.submit_tool.is_none());
    assert!(inspected.write_targets.is_empty());
    let knowledge_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "knowledge_context_plan")
        .expect("knowledge_context_plan group");
    assert!(knowledge_group.required);
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
    let current_block_rules = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        group_id: "current_block_rules".to_string(),
    })
    .expect("read current block rules");
    let rules_text =
        serde_json::to_string(&current_block_rules.fields).expect("serialize current block rules");
    assert!(rules_text.contains("active phase"));
    assert!(!rules_text.contains("active phase-1"));
    assert!(rules_text.contains("call loom.knowledgeBrainstormContext"));
    assert!(rules_text.contains("full-project roadmap"));
    assert!(rules_text.contains("full multi-stage roadmap"));
    assert!(rules_text.contains("Do not output numbered full-project phases"));
    assert!(!current_block_rules
        .fields
        .keys()
        .any(|field| field.contains("candidateWrite")));
    let knowledge_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "clarificationConversationProtocol.userVisibleBlockTitle".to_string(),
            "clarificationConversationProtocol.blockRule".to_string(),
            "blockConfirmationContract".to_string(),
            "knowledgeQueryPlan.toolContract".to_string(),
            "knowledgeQueryPlan.sharedRules".to_string(),
            "knowledgeQueryPlan.blocks.phase_scope.executionOrder".to_string(),
        ],
    })
    .expect("knowledge query plan fields");
    assert_eq!(
        knowledge_fields.fields["clarificationConversationProtocol.userVisibleBlockTitle"].value,
        "阶段范围确认"
    );
    assert_eq!(
        knowledge_fields.fields["blockConfirmationContract"].value["tool"],
        "loom.brainstormConfirmBlock"
    );
    assert!(
        knowledge_fields.fields["clarificationConversationProtocol.blockRule"]
            .value
            .as_str()
            .unwrap_or_default()
            .contains("not a full multi-stage project roadmap")
    );
    assert_eq!(
        knowledge_fields.fields["knowledgeQueryPlan.toolContract"].value["contextTool"],
        "loom.knowledgeBrainstormContext"
    );
    assert!(knowledge_fields.fields["knowledgeQueryPlan.sharedRules"]
        .value
        .to_string()
        .contains("do not silently fall back"));
    assert!(
        knowledge_fields.fields["knowledgeQueryPlan.blocks.phase_scope.executionOrder"]
            .value
            .to_string()
            .contains("Do not output or confirm the overall dependency sequence")
    );
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
    let mut request_ref = planned["requestRef"]
        .as_str()
        .expect("requestRef")
        .to_string();

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
    assert!(
        server
            .invoke_tool(
                "loom.readFieldGroup",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "groupId": "candidate_write_contract"
                }))),
            )
            .is_err(),
        "clarification request must not expose candidate_write_contract"
    );

    request_ref = confirm_block(
        &server,
        &fixture,
        &request_ref,
        "phase_scope",
        "确认第一阶段为证券账户模块闭环。",
        json!({
            "scope": {
                "included": ["证券账户开户", "证券账户挂失补办", "证券账户销户", "账户状态管理"],
                "deferred": ["资金账户", "交易客户端", "中央撮合"],
                "excluded": []
            },
            "recommendation": {
                "label": "证券账户模块闭环",
                "reason": "证券账户是交易身份和持仓归属的上游基础对象。"
            }
        }),
    )["requestRef"]
        .as_str()
        .expect("concept request ref")
        .to_string();
    assert_eq!(
        state::inspect_request(InspectRequestInput {
            project_root: fixture.root_str().to_string(),
            request_ref: request_ref.clone(),
        })
        .expect("inspect concept request")
        .request_kind,
        "brainstorm_clarification_block"
    );
    request_ref = confirm_block(
        &server,
        &fixture,
        &request_ref,
        "concept_grounding",
        "确认证券账户业务规则、状态和边界。",
        json!({
            "objects": ["证券账户"],
            "operations": ["开户", "挂失补办", "销户"],
            "rules": ["开户需要资格校验", "挂失后冻结证券", "销户前必须清空持仓"],
            "boundaries": ["资金账户递延", "交易客户端递延"]
        }),
    )["requestRef"]
        .as_str()
        .expect("frontend request ref")
        .to_string();
    request_ref = confirm_block(
        &server,
        &fixture,
        &request_ref,
        "frontend_experience",
        "确认工作人员后台证券账户管理页面路径。",
        json!({
            "required": true,
            "surfaces": ["证券账户管理页面"],
            "targetDiscovery": ["分页查询列表", "按账户号、姓名、证件号查询"],
            "operationPaths": ["开户从新建入口进入", "挂失补办和销户先查询并选择目标账户"],
            "mustNot": ["不能只靠内部主键触发办理动作"]
        }),
    )["requestRef"]
        .as_str()
        .expect("final summary request ref")
        .to_string();
    let write_action = confirm_block(
        &server,
        &fixture,
        &request_ref,
        "final_summary",
        "用户已确认阶段范围、业务理解、页面办理路径和提交前核对。",
        json!({
            "coverageChecklist": ["证券账户模块闭环", "开户/挂失补办/销户规则", "工作人员后台办理路径"],
            "readyToWriteCandidate": true
        }),
    );
    assert_eq!(write_action["state"], "auto_runnable", "{write_action:#}");
    let request_ref = write_action["next"]["requestRef"]
        .as_str()
        .expect("candidate write requestRef");

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
    let template = &write_contract["fields"]["outputContract.resultTemplate"];
    assert!(template["scope"]["deferred"][0].is_object());
    assert!(template["scope"]["assumptions"][0].is_object());
    assert_eq!(
        template["phasePlan"]["nextPhasePreview"]["kind"],
        "candidate"
    );
    assert!(template["frontendExperience"]["audiences"][0]["audienceId"].is_string());
    assert!(template["frontendExperience"]["surfaces"][0]["surfaceId"].is_string());
    assert!(template["frontendExperience"]["dataViews"][0]["viewId"].is_string());
    assert!(template["frontendExperience"]["actions"][0]["actionId"].is_string());
    assert!(template["frontendExperience"]["operationPaths"][0]["pathId"].is_string());
    assert!(write_contract["fields"]["rules.candidateWrite"]
        .to_string()
        .contains("never replace typed object arrays with string arrays"));
    assert!(write_contract["fields"]["rules.candidateWrite"]
        .to_string()
        .contains("scope.deferred is non-empty"));
    let mut candidate = write_contract["fields"]["outputContract.resultTemplate"].clone();
    populate_confirmed_brainstorm_candidate(&mut candidate);

    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request");
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.brainstormAcceptFile")
    );
    assert_eq!(inspected.write_targets.len(), 1);
    let compact_request = read_compact_request_root(&fixture, &inspected.request_id);
    for key in [
        "artifactKind",
        "submitTool",
        "writeTargets",
        "writeMode",
        "outputContract",
    ] {
        assert!(
            compact_request.get(key).is_none(),
            "candidate write compact root must not duplicate {key}: {compact_request:#}"
        );
    }
    let manifest = read_request_storage_manifest(&fixture, &inspected.request_id);
    assert!(manifest["refs"]["outputContract"].is_object());
    assert!(manifest["refs"]["rules"].is_object());
    assert!(manifest["refs"]["enumRefs"].is_object());
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

fn confirm_block(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    block: &str,
    summary: &str,
    confirmed_data: Value,
) -> Value {
    structured(
        server
            .invoke_tool(
                "loom.brainstormConfirmBlock",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "block": block,
                    "summary": summary,
                    "confirmedData": confirmed_data
                }))),
            )
            .expect("confirm brainstorm block"),
    )
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

fn read_compact_request_root(fixture: &Fixture, request_id: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            fixture
                .root
                .join(".loom/requests")
                .join(format!("{request_id}.json")),
        )
        .expect("read compact request root"),
    )
    .expect("parse compact request root")
}

fn read_request_storage_manifest(fixture: &Fixture, request_id: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            fixture
                .root
                .join(".loom/requests")
                .join(format!("{request_id}.manifest.json")),
        )
        .expect("read request storage manifest"),
    )
    .expect("parse request storage manifest")
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
