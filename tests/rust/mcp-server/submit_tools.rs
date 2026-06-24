use std::sync::{Mutex, MutexGuard};

use delivery_core::InspectRequestInput;
use mcp_server::server::LoomMcpServer;
use serde_json::{json, Value};
use state::{store::write_json_atomic, write_native_request, NativeRequestInput};

#[test]
fn submit_tool_returns_repairable_error_for_missing_target_file() {
    let fixture = Fixture::new("submit-missing-target");
    let stored = write_brainstorm_request(&fixture, "req_submit_missing", false);
    let result = call_submit(
        "loom.brainstormAcceptFile",
        &stored.request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error");
    assert_eq!(result["targetFile"], ".loom/agent-writable/candidate.json");
    assert_eq!(result["targetIds"], json!(["candidate"]));
    assert_eq!(result["issues"][0]["code"], "TARGET_MISSING");
    assert_eq!(result["resubmitTool"], "loom.brainstormAcceptFile");
    assert!(result.get("submitCommand").is_none());
}

#[test]
fn brainstorm_submit_returns_user_gate_when_clarification_is_incomplete() {
    let fixture = Fixture::new("submit-user-gate");
    let request_ref = start_brainstorm_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &json!({}));

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "user_gate");
    assert_eq!(result["gate"]["currentBlock"], "phase_scope");
    assert_eq!(result["requestRef"], request_ref);
}

#[test]
fn brainstorm_submit_returns_repairable_error_for_schema_invalid_candidate() {
    let fixture = Fixture::new("submit-schema-invalid");
    let request_ref = start_brainstorm_request(&fixture);
    write_candidate_target(
        &fixture,
        &request_ref,
        &json!({
            "requestSummary": {
                "title": "坏结构",
                "oneLine": "坏结构",
                "complexity": "small"
            },
            "clarificationProgress": "broken"
        }),
    );

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error");
    assert_eq!(
        result["issues"][0]["code"],
        "CLARIFICATION_PROGRESS_INVALID"
    );
    assert_eq!(result["resubmitTool"], "loom.brainstormAcceptFile");
}

#[test]
fn brainstorm_submit_accepts_valid_candidate_and_hands_off_to_batch_eight() {
    let fixture = Fixture::new("submit-success");
    let request_ref = start_brainstorm_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["artifactKind"],
        "technical_baseline_candidate"
    );
    assert_eq!(
        result["next"]["submitTool"],
        "loom.technicalBaselineAcceptFile"
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("brainstorm/contract.json")
        .exists());
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("brainstorm/phases/phase-1/decision-snapshot.json")
        .exists());
    let technical_baseline_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("technical baseline requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: technical_baseline_request_ref.to_string(),
    })
    .expect("inspect technical baseline request");
    assert_eq!(inspected.request_kind, "technical_baseline_request");
}

#[test]
fn continue_reuses_same_technical_baseline_request_after_brainstorm_accept() {
    let fixture = Fixture::new("continue-technical-baseline");
    let request_ref = start_brainstorm_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let submitted = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let continued = continue_delivery(fixture.root_str());

    assert_eq!(submitted["state"], "auto_runnable");
    assert_eq!(continued["state"], "auto_runnable");
    assert_eq!(
        continued["next"]["requestRef"],
        submitted["next"]["requestRef"]
    );
    assert_eq!(
        continued["next"]["artifactKind"],
        "technical_baseline_candidate"
    );
}

#[test]
fn technical_baseline_accept_routes_existing_project_to_repository_context() {
    let fixture = Fixture::new("technical-baseline-existing-project");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    let request_ref = start_brainstorm_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["artifactKind"],
        "repository_context_candidate"
    );
    assert_eq!(
        result["next"]["submitTool"],
        "loom.repositoryContextAcceptFile"
    );
}

#[test]
fn repository_context_accept_persists_pgc_and_hands_off_to_architecture() {
    let fixture = Fixture::new("repository-context-pgc");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    write_json_atomic(
        &fixture.root.join("src/main.tsx"),
        &json!("export const app = true;"),
    )
    .expect("write entrypoint");
    let request_ref = start_brainstorm_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );
    let baseline_result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    let repository_context_request_ref = baseline_result["next"]["requestRef"]
        .as_str()
        .expect(&format!(
            "repository context requestRef: {baseline_result:#}"
        ))
        .to_string();

    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let brainstorm_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "brainstormContract");
    let technical_baseline_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "technicalBaseline");
    write_candidate_target(
        &fixture,
        &repository_context_request_ref,
        &repository_context_candidate_json(
            &repository_context_request_ref,
            &brainstorm_contract_ref,
            &technical_baseline_ref,
        ),
    );

    let result = call_submit(
        "loom.repositoryContextAcceptFile",
        &repository_context_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "failed");
    assert_eq!(result["error"]["code"], "not_implemented_for_batch");
    assert_eq!(
        result["error"]["routeAction"],
        "architecture_artifact_contract"
    );
    let planning_contract = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/planning/phase-1/pgc.json");
    assert!(planning_contract.exists());
}

#[test]
fn brainstorm_submit_rejects_stale_request_binding() {
    let fixture = Fixture::new("submit-stale-request");
    let request_ref = start_brainstorm_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let delivery_index = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&delivery_index).expect("read index"))
            .expect("parse index");
    index["phases"][0]["latestRefs"]["brainstormRequestRef"] =
        Value::String("loom://projects/stale/requests/other".to_string());
    write_json_atomic(&delivery_index, &index).expect("write stale index");
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "failed");
    assert_eq!(result["error"]["code"], "STALE_BRAINSTORM_REQUEST");
    assert_eq!(result["error"]["recoveryTool"], "loom.continue");
}

fn write_brainstorm_request(
    fixture: &Fixture,
    request_id: &str,
    write_target: bool,
) -> state::StoredRequest {
    if write_target {
        write_json_atomic(
            &fixture.root.join(".loom/agent-writable/candidate.json"),
            &json!({ "summary": "ok" }),
        )
        .expect("write target");
    }
    write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: request_id.to_string(),
            request_kind: "brainstorm_candidate".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "artifactKind": "brainstorm_candidate",
                "submitTool": "loom.brainstormAcceptFile",
                "outputContract": {
                    "writeMode": "single_json",
                    "writeTargets": [{
                        "targetId": "candidate",
                        "path": ".loom/agent-writable/candidate.json",
                        "required": true,
                        "description": "Brainstorm candidate JSON."
                    }]
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "required": true,
                        "purpose": "Read core fields.",
                        "whenToRead": "Before writing.",
                        "fields": ["writeTargets"]
                    }]
                }
            }),
        },
    )
    .expect("write request")
}

fn call_submit(tool_name: &str, request_ref: &str, project_root: &str) -> serde_json::Value {
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": project_root,
        "requestRef": request_ref,
        "writtenTargetIds": ["candidate"]
    })
    .as_object()
    .expect("arguments object")
    .clone();
    server
        .invoke_tool(tool_name, Some(arguments))
        .expect("call tool")
        .structured_content
        .expect("structured content")
}

fn continue_delivery(project_root: &str) -> serde_json::Value {
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": project_root
    })
    .as_object()
    .expect("arguments object")
    .clone();
    server
        .invoke_tool("loom.continue", Some(arguments))
        .expect("continue")
        .structured_content
        .expect("structured content")
}

fn start_brainstorm_request(fixture: &Fixture) -> String {
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": fixture.root_str(),
        "requestText": "实现证券账户开户流程"
    })
    .as_object()
    .expect("arguments object")
    .clone();
    let result = server
        .invoke_tool("loom.plan", Some(arguments))
        .expect("plan call")
        .structured_content
        .expect("structured content");
    result["requestRef"]
        .as_str()
        .expect("requestRef")
        .to_string()
}

fn write_candidate_target(fixture: &Fixture, request_ref: &str, value: &Value) {
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request");
    let target = inspected.write_targets.first().expect("write target");
    let path = target["path"].as_str().expect("target path");
    write_json_atomic(&fixture.root.join(path), value).expect("write candidate");
}

fn latest_ref_for_phase(project_root: &str, delivery_id: &str, key: &str) -> String {
    let index_path = std::path::Path::new(project_root)
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    index["phases"][0]["latestRefs"][key]
        .as_str()
        .expect("latest ref")
        .to_string()
}

fn request_delivery_id(project_root: &str, request_ref: &str) -> String {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    state::request_index::get_request_index_entry(project_root, request_id)
        .expect("request index entry")
        .delivery_id
        .expect("delivery id")
}

fn technical_baseline_candidate_json(project_kind: &str, approval_type: &str) -> Value {
    json!({
        "status": "confirmed",
        "source": if project_kind == "existing_project" {
            "detected_from_repo"
        } else {
            "agent_recommended_for_greenfield"
        },
        "projectKind": project_kind,
        "scope": "project",
        "stack": {
            "frontend": "vite-react",
            "backend": "spring-boot",
            "database": "postgres"
        },
        "constraints": [],
        "evidence": [{
            "path": if project_kind == "existing_project" { json!("package.json") } else { Value::Null },
            "reason": "Repository signals support the selected stack."
        }],
        "approval": {
            "type": approval_type,
            "reason": "Current repository stack is stable."
        },
        "confidence": "high",
        "requiresUserConfirmation": false,
        "reasoningSummary": [
            "The current phase should preserve a stable application stack."
        ],
        "alternatives": []
    })
}

fn repository_context_candidate_json(
    request_ref: &str,
    brainstorm_contract_ref: &str,
    technical_baseline_ref: &str,
) -> Value {
    json!({
        "status": "ready",
        "source": {
            "requestRef": request_ref,
            "brainstormContractRef": brainstorm_contract_ref,
            "technicalBaselineRef": technical_baseline_ref
        },
        "requestLens": {
            "projectKind": "existing_project",
            "baselineProjectKind": "existing_project",
            "repositoryMode": "existing_project",
            "phaseDevelopmentMode": "initial_delivery",
            "scanPurpose": "phase_start_repository_snapshot",
            "primaryConsumer": "phase_brainstorm",
            "laterConsumers": ["PGC", "AAC", "TaskPlan"]
        },
        "repoOverview": {
            "summary": "Repository contains an existing frontend application entrypoint.",
            "repositoryShape": "single_package",
            "primaryApplications": [{
                "applicationId": "app_web",
                "name": "web",
                "kind": "frontend",
                "rootPath": "src"
            }]
        },
        "technologySignals": {
            "primaryLanguages": ["typescript"],
            "frameworks": ["react"],
            "packageManagers": ["npm"],
            "buildCommands": ["npm run build"],
            "testCommands": ["npm test"],
            "notes": []
        },
        "structureSignals": {
            "rootPaths": [{
                "path": "src",
                "role": "application_root"
            }],
            "entryPoints": [{
                "path": "src/main.tsx",
                "kind": "entrypoint",
                "description": "frontend entrypoint"
            }],
            "configurationFiles": ["package.json"]
        },
        "existingCapabilities": [{
            "capabilityId": "capability_ui_root",
            "name": "frontend entrypoint",
            "status": "partial",
            "summary": "The repository already has a frontend entrypoint.",
            "surfaceRefs": ["surface_ui_root"],
            "confidence": "high",
            "deliveryRelevance": "Can be extended for the current phase."
        }],
        "relevantSurfaces": [{
            "surfaceId": "surface_ui_root",
            "kind": "ui",
            "path": "src/main.tsx",
            "summary": "Existing frontend entrypoint.",
            "relevance": "extension_point",
            "suggestedUse": "inspect_or_extend"
        }],
        "recommendedReadRefs": [{
            "path": "src/main.tsx",
            "reason": "implemented_capability",
            "priority": "high",
            "summary": "Inspect the existing entrypoint before modifying UI flow.",
            "surfaceRefs": ["surface_ui_root"]
        }],
        "contextQuality": {
            "coverage": "focused",
            "confidence": "high",
            "warnings": []
        },
        "warnings": []
    })
}

fn valid_candidate_json() -> Value {
    json!({
        "requestSummary": {
            "title": "证券账户开户流程",
            "oneLine": "实现证券账户开户闭环",
            "businessGoal": "完成第一阶段证券账户业务闭环",
            "complexity": "medium"
        },
        "scope": {
            "included": [{
                "id": "scope_1",
                "label": "证券账户开户",
                "items": ["个人开户", "法人开户"],
                "reason": "当前阶段先完成证券账户能力闭环",
                "source": "user_confirmed"
            }],
            "excluded": [],
            "deferred": [],
            "assumptions": []
        },
        "roadmap": {
            "required": false,
            "currentPhaseId": "phase-1",
            "phases": [{
                "phaseId": "phase-1",
                "title": "证券账户开户流程",
                "name": "证券账户开户流程",
                "status": "scope_confirmed",
                "goal": "实现证券账户开户闭环",
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "dependsOn": []
            }]
        },
        "phasePlan": {
            "current": {
                "phaseId": "phase-1",
                "title": "证券账户开户流程",
                "goal": "实现证券账户开户闭环",
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "status": "scope_confirmed"
            },
            "nextPhasePreview": {
                "kind": "none",
                "reason": "下一阶段在当前提交之外处理。"
            }
        },
        "acceptance": [{
            "id": "acc_1",
            "statement": "工作人员可以完成证券账户开户并得到成功反馈。",
            "capabilityRefs": [],
            "sourceRefs": [],
            "priority": "must"
        }],
        "userConfirmation": {
            "confirmed": true,
            "confirmedAt": "2026-06-24T10:00:00+08:00",
            "confirmationSummary": "用户已确认阶段范围、业务规则和提交前核对。",
            "confirmationBasis": {
                "initialRequestOnly": false,
                "summaryPresentedToUser": true,
                "confirmedAfterSummary": true,
                "presentedItems": [
                    "phase_scope",
                    "concept_grounding",
                    "final_summary"
                ]
            }
        },
        "conceptGrounding": {
            "phaseConceptGrounding": {
                "mode": "none_required",
                "reason": "当前范围不需要额外术语表才能继续。",
                "concepts": []
            },
            "glossaryUpdates": []
        },
        "conceptConfirmation": {
            "shownToUser": true,
            "confirmedConceptRefs": [],
            "confirmationSummary": "用户已确认当前阶段的业务理解。"
        },
        "clarificationProgress": {
            "mode": "progressive_blocks",
            "confirmedBlocks": [
                {
                    "block": "phase_scope",
                    "summary": "已确认当前阶段范围。",
                    "confirmedByUser": true
                },
                {
                    "block": "concept_grounding",
                    "summary": "已确认业务规则和边界。",
                    "confirmedByUser": true
                }
            ],
            "skippedBlocks": [
                {
                    "block": "frontend_experience",
                    "reason": "本用例只验证提交链路，不要求页面路径细节。"
                }
            ],
            "finalSummaryConfirmed": true
        }
    })
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
            "loom-mcp-submit-{name}-{}-{}",
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
