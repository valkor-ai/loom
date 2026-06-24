use std::sync::{Mutex, MutexGuard};

use delivery_core::{InspectRequestInput, ReadRequestFieldsInput};
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
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);

    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
    })
    .expect("inspect architecture request");

    assert_eq!(inspected.request_kind, "architecture_sections_generation");
    assert_eq!(
        inspected.write_targets[0]["targetId"],
        json!("foundation"),
        "{inspected:#?}"
    );
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.architectureSectionSubmitFile")
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let planning_contract = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/planning/phase-1/pgc.json");
    assert!(planning_contract.exists());
}

#[test]
fn architecture_section_submit_advances_same_request_to_next_section() {
    let fixture = Fixture::new("architecture-next-section");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);

    write_candidate_target(
        &fixture,
        &architecture_request_ref,
        &architecture_section_candidate_json(&fixture, &architecture_request_ref),
    );

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["requestRef"],
        json!(architecture_request_ref),
        "{result:#}"
    );
    assert_eq!(
        result["next"]["writeTargets"][0]["targetId"],
        "domain_contract"
    );

    let continued = continue_delivery(fixture.root_str());
    assert_eq!(continued["state"], "auto_runnable", "{continued:#}");
    assert_eq!(
        continued["next"]["requestRef"], result["next"]["requestRef"],
        "{continued:#}"
    );
    assert_eq!(
        continued["next"]["writeTargets"][0]["targetId"],
        "domain_contract"
    );
}

#[test]
fn architecture_coverage_submit_persists_aac_and_routes_to_taskplan_generation() {
    let fixture = Fixture::new("architecture-full-chain");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let current_request_ref = architecture_request_ref.clone();

    for expected_section in [
        "foundation",
        "domain_contract",
        "behavior",
        "frontend_experience",
        "runtime_delivery",
        "coverage",
    ] {
        let inspected = state::inspect_request(InspectRequestInput {
            project_root: fixture.root_str().to_string(),
            request_ref: current_request_ref.clone(),
        })
        .expect("inspect current architecture request");
        assert_eq!(
            inspected.write_targets[0]["targetId"],
            json!(expected_section)
        );

        write_candidate_target(
            &fixture,
            &current_request_ref,
            &architecture_section_candidate_json(&fixture, &current_request_ref),
        );

        let result = call_submit(
            "loom.architectureSectionSubmitFile",
            &current_request_ref,
            fixture.root_str(),
        );

        if expected_section != "coverage" {
            assert_eq!(result["state"], "auto_runnable", "{result:#}");
            assert_eq!(
                result["next"]["requestRef"],
                json!(current_request_ref),
                "{result:#}"
            );
        } else {
            assert_eq!(result["state"], "failed", "{result:#}");
            assert_eq!(result["error"]["code"], "not_implemented_for_batch");
            assert_eq!(result["error"]["routeAction"], "taskplan_generation");
        }
    }

    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    assert_eq!(
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact"),
        format!(".loom/deliveries/{delivery_id}/contracts/architecture/phase-1/aac.json")
    );
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/architecture/phase-1/aac.json")
        .exists());
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/architecture/phase-1/latest.json")
        .exists());
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
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request for submit");
    let written_target_ids = inspected
        .write_targets
        .iter()
        .filter_map(|target| target.get("targetId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": project_root,
        "requestRef": request_ref,
        "writtenTargetIds": written_target_ids
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

fn start_existing_project_architecture_flow(fixture: &Fixture) -> String {
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    std::fs::create_dir_all(fixture.root.join("src")).expect("create src");
    std::fs::write(
        fixture.root.join("src/main.tsx"),
        "export const app = true;\n",
    )
    .expect("write entrypoint");

    let request_ref = start_brainstorm_request(fixture);
    write_candidate_target(fixture, &request_ref, &valid_candidate_json());

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
        fixture,
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
        .expect("repository context requestRef")
        .to_string();

    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let brainstorm_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "brainstormContract");
    let technical_baseline_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "technicalBaseline");
    write_candidate_target(
        fixture,
        &repository_context_request_ref,
        &repository_context_candidate_json(
            &repository_context_request_ref,
            &brainstorm_contract_ref,
            &technical_baseline_ref,
        ),
    );

    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        &repository_context_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repository_result["state"], "auto_runnable",
        "{repository_result:#}"
    );
    assert_eq!(
        repository_result["next"]["artifactKind"],
        "architecture_section_candidate"
    );
    repository_result["next"]["requestRef"]
        .as_str()
        .expect("architecture requestRef")
        .to_string()
}

fn architecture_section_candidate_json(fixture: &Fixture, request_ref: &str) -> Value {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let section = request_root["sectionState"]["currentSection"]
        .as_str()
        .expect("currentSection");
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "sourceRefs".to_string(),
            "allowedRefs".to_string(),
            "contextProjection.planningContractId".to_string(),
            "contextProjection.technicalBaseline".to_string(),
            "contextProjection.requirementDetailTransfer.acceptanceDetails".to_string(),
            "contextProjection.requirementDetailTransfer.requirementDetails".to_string(),
            "frontendExperienceSource".to_string(),
        ],
    })
    .expect("read architecture request fields")
    .fields;
    let source_refs = &fields["sourceRefs"].value;
    let allowed_refs = &fields["allowedRefs"].value;
    let planning_contract_id = fields["contextProjection.planningContractId"]
        .value
        .as_str()
        .expect("planningContractId");
    let technical_baseline_id = fields["contextProjection.technicalBaseline"]
        .value
        .get("technicalBaselineId")
        .and_then(Value::as_str)
        .expect("technicalBaselineId");
    let acceptance_details = fields
        ["contextProjection.requirementDetailTransfer.acceptanceDetails"]
        .value
        .as_array()
        .cloned()
        .unwrap_or_default();
    let acceptance_id = allowed_refs["acceptanceRefs"][0]
        .as_str()
        .expect("acceptanceRef");
    let detail_id = allowed_refs["requirementDetailIds"][0]
        .as_str()
        .expect("detailId");
    let acceptance_priority = acceptance_details
        .first()
        .and_then(|item| item.get("priority"))
        .and_then(Value::as_str)
        .unwrap_or("must");
    let acceptance_statement = acceptance_details
        .first()
        .and_then(|item| item.get("statement"))
        .and_then(Value::as_str)
        .unwrap_or("Current phase acceptance is covered by the architecture.");
    let frontend_source = &fields["frontendExperienceSource"].value;
    let frontend_authority_ref = frontend_source
        .get("confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            frontend_source
                .get("currentFrontendExperienceRef")
                .and_then(Value::as_str)
        });
    let technical_baseline_ref = source_refs
        .get("technicalBaselineRef")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let content = match section {
        "foundation" => json!({
            "source": {
                "planningGenerationContractId": planning_contract_id,
                "technicalBaselineId": technical_baseline_id
            },
            "engineeringBoundary": {
                "summary": "Current phase stays inside the confirmed account-delivery boundary.",
                "applications": [],
                "modules": []
            },
            "modules": [{
                "moduleId": "module.account-service",
                "name": "account-service",
                "summary": "Handles the current phase account workflow."
            }]
        }),
        "domain_contract" => json!({
            "dataModel": {
                "entities": [{
                    "entityId": "entity.account",
                    "name": "Account"
                }],
                "relationships": [],
                "constraints": []
            },
            "interfaces": [{
                "interfaceId": "api.account",
                "name": "Account API"
            }]
        }),
        "behavior" => json!({
            "userFlows": [{
                "flowId": "flow.account-lifecycle",
                "name": "Account lifecycle"
            }],
            "stateMachines": [{
                "machineId": "machine.account-status",
                "name": "Account status"
            }]
        }),
        "frontend_experience" => json!({
            "frontendExperience": {
                "required": true,
                "surfaces": [],
                "dataViews": [],
                "actions": [],
                "operationPaths": [],
                "sourceRefs": {
                    "brainstormFrontendExperienceRef": frontend_authority_ref
                }
            }
        }),
        "runtime_delivery" => json!({
            "runtimeDelivery": {
                "status": "modified",
                "basis": {
                    "technicalBaselineRef": technical_baseline_ref
                },
                "applications": [],
                "requiredChecks": []
            }
        }),
        "coverage" => json!({
            "acceptanceMatrix": [{
                "acceptanceId": acceptance_id,
                "priority": acceptance_priority,
                "statement": acceptance_statement,
                "coverageStatus": "covered",
                "coverage": [{
                    "type": "module",
                    "refs": ["module.account-service"],
                    "description": "Covered by the account-service module."
                }],
                "verificationHints": []
            }],
            "detailCoverage": [{
                "detailId": detail_id,
                "coverageStatus": "covered",
                "artifactRefs": {
                    "modules": ["module.account-service"],
                    "entities": [],
                    "fields": [],
                    "constraints": [],
                    "interfaces": [],
                    "userFlows": [],
                    "stateMachines": [],
                    "frontendDataViews": [],
                    "frontendActions": [],
                    "frontendOperationPaths": [],
                    "acceptanceMatrix": [acceptance_id]
                }
            }],
            "risksAndDecisions": {
                "decisions": []
            },
            "handoff": {
                "readyForTaskPlan": true,
                "blockingReasons": [],
                "nextNode": "task_plan"
            }
        }),
        other => panic!("unexpected architecture section: {other}"),
    };

    json!({
        "schemaVersion": "1.0",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "section": section,
        "status": "ready",
        "content": content,
        "createdAt": "2026-06-24T10:00:00+08:00"
    })
}

fn read_request_root_value(project_root: &str, request_ref: &str) -> Value {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let index = state::request_index::get_request_index_entry(project_root, request_id)
        .expect("request index entry");
    let request_path = std::path::Path::new(project_root).join(index.request_file);
    serde_json::from_str(&std::fs::read_to_string(request_path).expect("read request file"))
        .expect("parse request file")
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
