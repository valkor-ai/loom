use std::sync::{Mutex, MutexGuard};

use delivery_core::{
    DomainDispatcher, InspectRequestInput, ReadFieldGroupInput, ReadRequestFieldsInput,
    RouteAction, RouteActionKind,
};
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
    assert_eq!(result["resubmitTool"], "brainstormAcceptFile");
    assert!(result.get("submitCommand").is_none());
}

#[test]
fn brainstorm_clarification_request_has_no_candidate_write_contract() {
    let fixture = Fixture::new("submit-user-gate");
    let request_ref = start_brainstorm_request(&fixture);
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.clone(),
    })
    .expect("inspect clarification request");

    assert_eq!(inspected.request_kind, "brainstorm_clarification_block");
    assert!(inspected.submit_tool.is_none());
    assert!(inspected.write_targets.is_empty());
    assert!(!inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "candidate_write_contract"));
}

#[test]
fn brainstorm_submit_returns_repairable_error_for_schema_invalid_candidate() {
    let fixture = Fixture::new("submit-schema-invalid");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
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
fn brainstorm_submit_repairs_legacy_progress_shape_instead_of_reopening_phase_gate() {
    let fixture = Fixture::new("submit-legacy-progress-shape");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let mut candidate = valid_candidate_json();
    candidate["clarificationProgress"] = json!({
        "mode": "progressive_blocks",
        "completedBlocks": [
            "phase_scope",
            "concept_grounding",
            "frontend_experience",
            "final_summary"
        ],
        "currentBlock": "final_summary",
        "finalSummaryConfirmed": true
    });
    write_candidate_target(&fixture, &request_ref, &candidate);

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(
        result["issues"][0]["code"],
        "CLARIFICATION_PROGRESS_LEGACY_FIELDS"
    );
    assert_eq!(result["resubmitTool"], "loom.brainstormAcceptFile");
}

#[test]
fn brainstorm_submit_accepts_valid_candidate_and_hands_off_to_batch_eight() {
    let fixture = Fixture::new("submit-success");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
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
    let baseline_context = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: technical_baseline_request_ref.to_string(),
        group_id: "technical_baseline_context".to_string(),
    })
    .expect("read technical baseline context");
    assert!(baseline_context
        .fields
        .get("brainstormLens.acceptance")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.domainModel")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.roadmap")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.phasePlan")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.scope")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.frontendExperience")
        .is_none());
    for field in [
        "brainstormLens.summary.title",
        "brainstormLens.summary.oneLine",
        "brainstormLens.summary.businessGoal",
        "brainstormLens.scope.included",
        "brainstormLens.scope.deferred",
        "brainstormLens.roadmap.phases",
        "brainstormLens.phasePlan.nextPhasePreview",
        "currentPhaseLens.phaseId",
        "currentPhaseLens.goal",
    ] {
        assert!(
            baseline_context.fields.get(field).is_some(),
            "missing field-level baseline context {field}"
        );
    }
    for field in [
        "brainstormLens.domainModel.capabilityGroups",
        "brainstormLens.domainModel.businessFlows",
        "brainstormLens.frontendExperience.required",
        "brainstormLens.frontendExperience.surfaces",
    ] {
        assert!(
            baseline_context.fields.get(field).is_none(),
            "optional absent baseline context field should not be exposed: {field}"
        );
    }
    assert!(baseline_context.fields["brainstormLens.acceptanceIndex"].value[0]["id"].is_string());
    assert!(
        baseline_context.fields["brainstormLens.acceptanceIndex"].value[0]
            .get("statement")
            .is_none()
    );
    let selection_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "technical_baseline_selection_guidance")
        .expect("technical baseline selection guidance group");
    assert!(selection_group.required);
    assert_eq!(
        selection_group.fields,
        vec!["selectionGuidance".to_string()]
    );
    let selection = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: technical_baseline_request_ref.to_string(),
        group_id: "technical_baseline_selection_guidance".to_string(),
    })
    .expect("read technical baseline selection guidance");
    let guidance = &selection.fields["selectionGuidance"].value;
    assert!(guidance["trackModel"]["coreTracks"]
        .as_array()
        .expect("core tracks")
        .contains(&json!("backend")));
    assert!(guidance["commonOptions"]["backend"]["examples"]
        .as_array()
        .expect("backend examples")
        .contains(&json!("Java + Spring Boot")));
    assert!(guidance["commonOptions"]["backend"]["examples"]
        .as_array()
        .expect("backend examples")
        .contains(&json!("Python + FastAPI")));
    assert!(guidance["commonOptions"]["dataAccess"]["examples"]
        .as_array()
        .expect("data access examples")
        .contains(&json!("MyBatis Plus")));
    assert!(guidance["shorthandNormalization"]["backend"]
        .as_array()
        .expect("backend shorthand rules")
        .iter()
        .any(
            |rule| rule.as_str().unwrap_or_default().contains("backend=Java")
                && rule
                    .as_str()
                    .unwrap_or_default()
                    .contains("Java + Spring Boot")
        ));
    assert!(
        guidance["userFacingConfirmationProtocol"]["mandatorySections"]
            .as_array()
            .expect("mandatory sections")
            .iter()
            .any(|section| section
                .as_str()
                .unwrap_or_default()
                .contains("Adjustable technology range"))
    );
    assert!(guidance["replyProtocolForUser"]["partialAdjustmentExample"]
        .as_str()
        .expect("partial adjustment example")
        .contains("persistence=PostgreSQL"));
    assert!(guidance["userFacingConfirmationProtocol"]["wordingRules"]
        .as_array()
        .expect("wording rules")
        .iter()
        .any(|rule| rule
            .as_str()
            .unwrap_or_default()
            .contains("Do not mention Loom internals")));
}

#[test]
fn continue_reuses_same_technical_baseline_request_after_brainstorm_accept() {
    let fixture = Fixture::new("continue-technical-baseline");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
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
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
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
fn technical_baseline_conflict_with_previous_baseline_requires_user_gate() {
    let fixture = Fixture::new("technical-baseline-previous-conflict");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    write_previous_technical_baseline(&fixture, &delivery_id);
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
    let previous = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: baseline_request_ref.clone(),
        fields: vec!["previousBaselineContext.previousBaselineRef".to_string()],
    })
    .expect("read previous baseline context");
    assert_eq!(
        previous.fields["previousBaselineContext.previousBaselineRef"].value,
        json!(format!(
            ".loom/deliveries/{delivery_id}/contracts/technical-baseline.json"
        ))
    );

    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );
    let conflict = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    assert_eq!(conflict["state"], "user_gate", "{conflict:#}");
    assert_eq!(
        conflict["gate"]["gateId"],
        "previous_baseline_change_confirmation"
    );

    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "user_confirmed"),
    );
    let confirmed = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    assert_eq!(confirmed["state"], "auto_runnable", "{confirmed:#}");
    assert_eq!(
        confirmed["next"]["artifactKind"],
        "repository_context_candidate"
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
fn architecture_read_groups_follow_current_section() {
    let fixture = Fixture::new("architecture-read-groups-by-section");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);

    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "frontend_experience");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_frontend_context",
        ],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
}

#[test]
fn architecture_request_omits_previous_runtime_fields_without_previous_runtime() {
    let fixture = Fixture::new("architecture-runtime-no-previous");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let request_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);
    let source_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        fields: vec!["sourceRefs.previousRuntimeDeliveryRef".to_string()],
    })
    .expect("read source refs")
    .fields;

    assert!(!source_fields.contains_key("sourceRefs.previousRuntimeDeliveryRef"));
    let runtime_contract = runtime_section_contract(&request_root);
    assert_eq!(
        runtime_contract
            .pointer("/enumRefs/runtimeDeliveryStatus")
            .cloned()
            .unwrap(),
        json!(["modified", "not_applicable"])
    );
    assert!(runtime_contract
        .pointer("/schemaShape/content/runtimeDelivery/basis/previousRuntimeDeliveryRef")
        .is_none());
    assert!(runtime_contract
        .pointer("/generationRules")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .any(|rule| rule
            .as_str()
            .unwrap_or_default()
            .contains("do not use unchanged")));
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let core_group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref,
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture core group");
    assert!(!core_group
        .fields
        .contains_key("sourceRefs.previousRuntimeDeliveryRef"));
}

#[test]
fn architecture_request_exposes_previous_runtime_only_when_available() {
    let fixture = Fixture::new("architecture-runtime-with-previous");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let previous_runtime_ref = format!(
        ".loom/deliveries/{delivery_id}/contracts/architecture/phase-0/aac.json#/runtimeDelivery"
    );
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    let active_phase = index["phases"]
        .as_array_mut()
        .expect("phases")
        .iter_mut()
        .find(|phase| phase["phaseId"].as_str() == Some("phase-1"))
        .expect("phase-1");
    let latest_refs = active_phase["latestRefs"]
        .as_object_mut()
        .expect("latestRefs object");
    latest_refs.insert(
        "runtimeDelivery".to_string(),
        Value::String(previous_runtime_ref.clone()),
    );
    latest_refs.remove("architectureRequestRef");
    write_json_atomic(&index_path, &index).expect("write index with previous runtime");

    let result = architecture::ArchitectureDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &RouteAction {
            kind: RouteActionKind::ArchitectureArtifactContract,
            source: "test".to_string(),
            reason: "refresh_architecture_request".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let result = serde_json::to_value(result).expect("serialize architecture result");
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let refreshed_ref = result["next"]["requestRef"]
        .as_str()
        .expect("architecture requestRef")
        .to_string();
    assert_ne!(refreshed_ref, architecture_request_ref);
    let request_root = read_request_root_value(fixture.root_str(), &refreshed_ref);
    let source_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_ref.clone(),
        fields: vec!["sourceRefs.previousRuntimeDeliveryRef".to_string()],
    })
    .expect("read previous runtime ref")
    .fields;

    assert_eq!(
        source_fields["sourceRefs.previousRuntimeDeliveryRef"].value,
        json!(previous_runtime_ref)
    );
    let runtime_contract = runtime_section_contract(&request_root);
    assert_eq!(
        runtime_contract
            .pointer("/enumRefs/runtimeDeliveryStatus")
            .cloned()
            .unwrap(),
        json!(["modified", "unchanged", "not_applicable"])
    );
    assert_eq!(
        runtime_contract
            .pointer("/schemaShape/content/runtimeDelivery/basis/previousRuntimeDeliveryRef")
            .cloned()
            .unwrap(),
        json!("string")
    );

    advance_architecture_to_section(&fixture, &refreshed_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &refreshed_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let core_group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_ref,
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture core group");
    assert_eq!(
        core_group.fields["sourceRefs.previousRuntimeDeliveryRef"].value,
        json!(previous_runtime_ref)
    );
}

#[test]
fn architecture_section_submit_advances_same_request_to_next_section() {
    let fixture = Fixture::new("architecture-next-section");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let architecture_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);
    assert!(architecture_root["currentSectionContract"]["resultTemplate"]["content"].is_object());
    let architecture_rules =
        architecture_root["currentSectionContract"]["generationRules"].to_string();
    assert!(architecture_rules.contains("existing project and technical baseline shape"));
    assert!(architecture_rules.contains("avoid pass-through wrappers"));
    let coverage_template = architecture_root["sectionOutputs"]
        .as_array()
        .expect("section outputs")
        .iter()
        .find(|section| section["section"].as_str() == Some("coverage"))
        .expect("coverage section")["resultTemplate"]["content"]
        .clone();
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["acceptanceId"],
        json!("acc_1")
    );
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["statement"],
        json!("工作人员可以完成证券账户开户并得到成功反馈。")
    );
    assert!(
        coverage_template["acceptanceMatrix"][0]
            .get("artifactRefs")
            .is_none(),
        "acceptanceMatrix must not template artifactRefs"
    );
    assert!(coverage_template["acceptanceMatrix"][0]["coverage"].is_array());
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["kind"],
        json!("manual")
    );
    assert!(
        coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["description"].is_string()
    );
    assert!(
        coverage_template["detailCoverage"][0]["detailId"]
            .as_str()
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        "detailCoverage rows must be pre-keyed by detailId"
    );
    let artifact_refs = &coverage_template["detailCoverage"][0]["artifactRefs"];
    for key in [
        "modules",
        "entities",
        "fields",
        "constraints",
        "interfaces",
        "userFlows",
        "stateMachines",
        "frontendDataViews",
        "frontendActions",
        "frontendOperationPaths",
        "acceptanceMatrix",
    ] {
        assert!(artifact_refs[key].is_array(), "missing artifactRefs.{key}");
    }
    let frontend_authority_ref = architecture_root
        .pointer("/frontendExperienceSource/confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            architecture_root
                .pointer("/frontendExperienceSource/currentFrontendExperienceRef")
                .and_then(Value::as_str)
        })
        .expect("frontend authority ref");
    let frontend_template_refs = architecture_root["sectionOutputs"]
        .as_array()
        .expect("section outputs")
        .iter()
        .find(|section| section["section"].as_str() == Some("frontend_experience"))
        .expect("frontend section")["resultTemplate"]["content"]["frontendExperience"]
        ["sourceRefs"]
        .as_object()
        .expect("frontend sourceRefs");
    assert_eq!(frontend_template_refs.len(), 1);
    assert_eq!(
        frontend_template_refs["brainstormFrontendExperienceRef"],
        json!(frontend_authority_ref)
    );

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
fn architecture_coverage_submit_repairs_missing_acceptance_statement() {
    let fixture = Fixture::new("architecture-coverage-missing-statement");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["acceptanceMatrix"][0]
        .as_object_mut()
        .expect("acceptance row")
        .remove("statement");
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(result["issues"][0]["code"], "ACCEPTANCE_MATRIX_INVALID");
    assert_eq!(
        result["issues"][0]["fieldPath"],
        "content.acceptanceMatrix[0].statement"
    );
}

#[test]
fn architecture_coverage_submit_repairs_string_verification_hints() {
    let fixture = Fixture::new("architecture-coverage-string-hints");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["acceptanceMatrix"][0]["verificationHints"] =
        json!(["Verify the flow manually."]);
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "ACCEPTANCE_MATRIX_INVALID"
            && issue["fieldPath"] == "content.acceptanceMatrix[0].verificationHints[0]"
    }));
}

#[test]
fn architecture_coverage_submit_repairs_schema_shape_before_assembly() {
    let fixture = Fixture::new("architecture-coverage-contract-shape");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["detailCoverage"][0]["artifactRefs"]["modules"] =
        json!(["module.account-service", 42]);
    candidate["content"]["handoff"]["readyForTaskPlan"] = json!("yes");
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    let issues = result["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| {
        issue["code"] == "DETAIL_COVERAGE_INVALID"
            && issue["fieldPath"] == "content.detailCoverage[0].artifactRefs.modules[1]"
    }));
    assert!(issues.iter().any(|issue| {
        issue["code"] == "COVERAGE_HANDOFF_INVALID"
            && issue["fieldPath"] == "content.handoff.readyForTaskPlan"
    }));
}

#[test]
fn architecture_coverage_submit_persists_aac_and_routes_to_taskplan_generation() {
    let fixture = Fixture::new("architecture-full-chain");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let result = complete_architecture_sections(&fixture, &architecture_request_ref);

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(result["next"]["artifactKind"], "task_plan_candidate");
    assert_eq!(result["next"]["submitTool"], "loom.taskPlanAcceptFile");

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
    let taskplan_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
    })
    .expect("inspect taskplan request");
    assert_eq!(inspected.request_kind, "taskplan_generation_request");
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.taskPlanAcceptFile")
    );
    assert_eq!(inspected.write_targets.len(), 2);
    let taskplan_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        group_id: "taskplan_generation_rules".to_string(),
    })
    .expect("read taskplan rules");
    let taskplan_rules_text =
        serde_json::to_string(&taskplan_rules.fields).expect("serialize taskplan generation rules");
    assert!(taskplan_rules_text.contains("Keep next-phase seeds"));
    assert!(taskplan_rules_text.contains("smallest stable verification signal"));
    let compact_taskplan_root = read_request_root_value(fixture.root_str(), taskplan_request_ref);
    assert_no_root_submit_metadata(&compact_taskplan_root);
    let taskplan_contract_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "outputContract.outlineResultTemplate".to_string(),
            "outputContract.groupResultTemplate".to_string(),
            "outputContract.runtimeDeliveryRequirementTemplate".to_string(),
        ],
    })
    .expect("read taskplan contract fields")
    .fields;
    assert!(
        taskplan_contract_fields["outputContract.outlineResultTemplate"].value["groups"][0]
            .is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["group"].is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            .is_object()
    );
    let runtime_requirement_template =
        &taskplan_contract_fields["outputContract.runtimeDeliveryRequirementTemplate"].value;
    assert!(runtime_requirement_template.is_object());
    assert!(
        runtime_requirement_template["requiredCodeLevelChecks"][0]["checkId"].is_string(),
        "{runtime_requirement_template:#}"
    );
    assert!(
        runtime_requirement_template["requiredCodeLevelChecks"][0]["acceptableEvidence"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "{runtime_requirement_template:#}"
    );
    assert!(inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "taskplan_core_context"));
    assert!(inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
        .all(|field| field != "outputContract.outlineSchemaShape"
            && field != "outputContract.groupSchemaShape"));
}

#[test]
fn taskplan_accept_materializes_task_execution_and_task_result_routes_review() {
    let fixture = Fixture::new("taskplan-execution-chain");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    assert_eq!(execution_result["next"]["kind"], "execute_task");
    assert_eq!(
        execution_result["next"]["submitTool"],
        "loom.recordTaskResultFile"
    );
    let execution_request_ref = execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let execution_inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
    })
    .expect("inspect execution request");
    assert_eq!(execution_inspected.request_kind, "task_execution_request");
    let execution_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "source.taskId".to_string(),
            "executionRules.verificationCommandSchedulingRules".to_string(),
            "executionRules.boundaryRules".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution result template")
    .fields;
    assert_eq!(
        execution_fields["outputContract.resultTemplate"].value["taskId"],
        execution_fields["source.taskId"].value
    );
    assert!(
        execution_fields["outputContract.resultTemplate"].value["verificationResults"][0]
            .is_object()
    );
    assert!(
        execution_fields["outputContract.resultTemplate"].value["executionContinuity"].is_object()
    );
    let execution_rules_text =
        serde_json::to_string(&execution_fields).expect("serialize execution rules");
    assert!(execution_rules_text.contains("smallest meaningful verification signal"));
    assert!(execution_rules_text.contains("confirmed business language"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "task_execution_result_contract"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
        .all(|field| field != "outputContract.schemaShape"));

    write_task_result_candidate_without_requirement_detail_evidence(
        &fixture,
        &execution_request_ref,
    );
    let invalid_task_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        invalid_task_result["state"], "auto_runnable",
        "{invalid_task_result:#}"
    );
    assert_eq!(invalid_task_result["next"]["kind"], "write_artifact");
    assert_eq!(
        invalid_task_result["next"]["artifactKind"],
        "task_result_repair"
    );
    assert_eq!(
        invalid_task_result["next"]["submitTool"],
        "loom.repairSubmitFile"
    );

    let task_result_repair_action_ref = invalid_task_result["next"]["requestRef"]
        .as_str()
        .expect("task result repair action requestRef")
        .to_string();
    let task_result_repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: task_result_repair_action_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read task result repair template")
    .fields;
    assert!(
        task_result_repair_fields["outputContract.resultTemplate"].value["verificationResults"][0]
            .is_object()
    );
    let task_result_repair_root =
        read_request_root_value(fixture.root_str(), &task_result_repair_action_ref);
    assert!(task_result_repair_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("repair read groups")
        .iter()
        .flat_map(|group| group["fields"].as_array().into_iter().flatten())
        .any(|field| field.as_str() == Some("outputContract.resultTemplate")));
    write_large_task_result_candidate(&fixture, &task_result_repair_action_ref);
    let task_result = call_submit(
        "loom.repairSubmitFile",
        &task_result_repair_action_ref,
        fixture.root_str(),
    );

    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    assert_eq!(task_result["next"]["kind"], "write_artifact");
    assert_eq!(task_result["next"]["artifactKind"], "review_result");
    assert_eq!(task_result["next"]["submitTool"], "loom.reviewAcceptFile");
    let review_request_ref = task_result["next"]["requestRef"]
        .as_str()
        .expect("review requestRef");
    let review_inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
    })
    .expect("inspect review request");
    assert_eq!(review_inspected.request_kind, "review_request");
    let review_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read review result template")
    .fields;
    let review_root = read_request_root_value(fixture.root_str(), review_request_ref);
    assert!(
        review_root.get("reviewPacket").is_none(),
        "review request root must not expose full reviewPacket: {review_root:#}"
    );
    assert!(
        review_root.get("changeContext").is_none(),
        "review request root must not expose full changeContext: {review_root:#}"
    );
    let review_request_id = request_id_from_ref(review_request_ref);
    let review_storage_manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(
            fixture
                .root
                .join(format!(".loom/requests/{review_request_id}.manifest.json")),
        )
        .expect("read review private storage manifest"),
    )
    .expect("parse review private storage manifest");
    let review_manifest_refs = review_storage_manifest["refs"]
        .as_object()
        .expect("review manifest refs");
    assert!(review_manifest_refs.contains_key("reviewPacket"));
    assert!(review_manifest_refs.contains_key("changeContext"));
    assert!(review_manifest_refs.contains_key("outputContract"));
    assert_eq!(
        review_fields["outputContract.resultTemplate"].value["source"]["requestId"],
        review_root["requestId"]
    );
    let review_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_rules".to_string(),
    })
    .expect("read review rules");
    let review_rules_text =
        serde_json::to_string(&review_rules.fields).expect("serialize review rules");
    assert!(review_rules_text.contains("spec fidelity and project standards"));
    assert!(review_rules_text.contains("smallest repair"));
    assert!(review_fields["outputContract.resultTemplate"].value["coverageAssessment"].is_object());
    let template_evidence_ref =
        &review_fields["outputContract.resultTemplate"].value["findings"][0]["evidenceRefs"][0];
    assert_eq!(template_evidence_ref["type"], "task_result");
    assert!(template_evidence_ref["ref"].is_string());
    assert!(template_evidence_ref["reason"].is_string());
    let review_group_ids = review_inspected
        .read_groups
        .iter()
        .map(|group| group.group_id.as_str())
        .collect::<Vec<_>>();
    let review_packet_group = review_inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "review_packets")
        .expect("review_packets group");
    assert!(review_packet_group
        .fields
        .iter()
        .any(|field| field == "reviewPacket.groupSummaries"));
    assert!(review_packet_group
        .fields
        .iter()
        .any(|field| field == "reviewPacket.taskSummaries"));
    assert!(review_packet_group
        .fields
        .iter()
        .any(|field| field == "reviewPacket.taskResultSummaries"));
    assert!(!review_packet_group
        .fields
        .iter()
        .any(|field| field == "reviewPacket.groups"));
    assert!(!review_packet_group
        .fields
        .iter()
        .any(|field| field == "reviewPacket.tasks"));
    assert!(!review_packet_group
        .fields
        .iter()
        .any(|field| field == "reviewPacket.taskResults"));
    let review_packets = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_packets".to_string(),
    })
    .expect("read review packets");
    assert!(review_packets.fields["reviewPacket.groupSummaries"]
        .value
        .is_array());
    assert!(review_packets.fields["reviewPacket.taskSummaries"]
        .value
        .is_array());
    assert!(review_packets.fields["reviewPacket.taskResultSummaries"]
        .value
        .is_array());
    let review_packets_text =
        serde_json::to_string(&review_packets.fields).expect("serialize review packets");
    assert!(!review_packets_text.contains("very large execution note"));
    assert!(!review_packets_text.contains("very large verification summary"));
    assert!(!review_packets_text.contains("very large detail evidence summary"));
    assert!(
        serde_json::to_vec_pretty(&review_packets.fields["reviewPacket.taskResultSummaries"].value)
            .expect("serialize task result summaries")
            .len()
            < 32 * 1024
    );
    assert!(!review_packets.fields.contains_key("reviewPacket.groups"));
    assert!(!review_packets.fields.contains_key("reviewPacket.tasks"));
    assert!(!review_packets
        .fields
        .contains_key("reviewPacket.taskResults"));
    assert_eq!(
        review_group_ids,
        vec![
            "review_scope",
            "review_packets",
            "change_context",
            "review_matrices",
            "review_rules",
            "review_write_contract"
        ]
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    assert_eq!(index["phases"][0]["nextAction"]["kind"], "review");
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("tasks/phase-1/taskplans/latest.json")
        .exists());
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("tasks/phase-1/runs/latest.json")
        .exists());
    assert_no_read_plan_size_warnings(&fixture);
}

#[test]
fn taskplan_request_omits_null_optional_projection_reads() {
    let fixture = Fixture::new("taskplan-no-optional-projections");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let aac_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact");
    let aac_path = fixture.root.join(&aac_ref);
    let mut aac: Value =
        serde_json::from_str(&std::fs::read_to_string(&aac_path).expect("read AAC"))
            .expect("parse AAC");
    aac["frontendExperience"] = Value::Null;
    aac["runtimeDelivery"] = Value::Null;
    write_json_atomic(&aac_path, &aac).expect("write AAC without optional projections");

    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    let latest_refs = index["phases"]
        .as_array_mut()
        .expect("phases")
        .iter_mut()
        .find(|phase| phase["phaseId"].as_str() == Some("phase-1"))
        .expect("phase-1")["latestRefs"]
        .as_object_mut()
        .expect("latestRefs object");
    latest_refs.remove("taskPlanRequestId");
    latest_refs.remove("taskPlanRequestRef");
    write_json_atomic(&index_path, &index).expect("write index without taskplan request");

    let result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &RouteAction {
            kind: RouteActionKind::TaskplanGeneration,
            source: "test".to_string(),
            reason: "regenerate_taskplan_request".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let result = serde_json::to_value(result).expect("serialize taskplan result");
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect taskplan request");
    assert!(!inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "taskplan_optional_projection"));
    assert!(!inspected.read_groups.iter().any(|group| group
        .fields
        .iter()
        .any(|field| field == "outputContract.runtimeDeliveryClosureTaskTemplate")));
}

#[test]
fn failed_task_result_routes_to_delivery_execution_repair_before_review() {
    let fixture = Fixture::new("failed-task-result-repair");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_failed_task_result_candidate(&fixture, &execution_request_ref);

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["repairOrigin"], "task_failure");
    assert_eq!(
        result["next"]["repairContext"]["repairOrigin"],
        "task_failure"
    );
    assert_eq!(
        result["next"]["repairContext"]["sourceTaskId"],
        result["next"]["taskId"]
    );
    assert!(
        result["next"]["repairContext"]
            .get("repairRequestRef")
            .is_none(),
        "{result:#}"
    );
    assert!(result["next"]["repairContext"]["failedTaskResultRef"]
        .as_str()
        .unwrap()
        .contains("/tasks/phase-1/results/"));
    assert_eq!(result["next"]["repairContext"]["attemptCount"], 1);
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec![
            "outputContract.resultTemplate".to_string(),
            "taskConceptGrounding.conceptRefs".to_string(),
            "blockedOutput.blockedReasons".to_string(),
        ],
    })
    .expect("read execution repair result template")
    .fields;
    assert!(
        repair_fields["outputContract.resultTemplate"].value["verificationResults"][0].is_object()
    );
    assert!(repair_fields["taskConceptGrounding.conceptRefs"]
        .value
        .is_array());
    assert!(repair_fields["blockedOutput.blockedReasons"]
        .value
        .is_array());
    write_task_result_candidate(&fixture, repair_request_ref);
    let repaired_result = call_submit(
        "loom.recordTaskResultFile",
        repair_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repaired_result["state"], "auto_runnable",
        "{repaired_result:#}"
    );
    assert_ne!(
        repaired_result["error"]["code"], "FIELD_NOT_ALLOWED",
        "{repaired_result:#}"
    );
}

#[test]
fn blocked_task_result_routes_to_taskplan_repair() {
    let fixture = Fixture::new("blocked-task-result-taskplan");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_blocked_task_result_candidate(
        &fixture,
        &execution_request_ref,
        "TASKPLAN_INVALID",
        "taskplan_repair",
    );

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(result["next"]["artifactKind"], "taskplan_repair");
    assert_eq!(result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec![
            "outputContract.outlineResultTemplate".to_string(),
            "outputContract.groupResultTemplate".to_string(),
        ],
    })
    .expect("read taskplan repair templates")
    .fields;
    assert!(repair_fields["outputContract.outlineResultTemplate"].value["groups"][0].is_object());
    assert!(repair_fields["outputContract.groupResultTemplate"].value["group"].is_object());
    assert!(repair_fields["outputContract.groupResultTemplate"].value["tasks"][0].is_object());
}

#[test]
fn blocked_task_result_routes_to_architecture_repair() {
    let fixture = Fixture::new("blocked-task-result-architecture");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_blocked_task_result_candidate(
        &fixture,
        &execution_request_ref,
        "DESIGN_INSUFFICIENT",
        "architecture_artifact_repair",
    );

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["artifactKind"],
        "architecture_artifact_repair"
    );
    assert_eq!(result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let repair_root = read_request_root_value(fixture.root_str(), repair_request_ref);
    assert!(repair_root["currentSectionContract"]["resultTemplate"]["content"].is_object());
}

#[test]
fn review_accept_approved_marks_delivery_done() {
    let fixture = Fixture::new("review-approved");
    let review_request_ref = complete_task_execution_to_review(&fixture);

    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "done", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    assert_eq!(
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "reviewResult"),
        format!(".loom/deliveries/{delivery_id}/reviews/phase-1/results/review-phase-1.json")
    );
    let continued = continue_delivery(fixture.root_str());
    assert_eq!(continued["state"], "done", "{continued:#}");
}

#[test]
fn review_accept_continue_to_next_phase_records_phase_transition() {
    let fixture = Fixture::new("review-continue-phase");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    append_done_phase(&fixture, &delivery_id, "phase-2");

    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "approved",
        "continue_to_next_phase",
        vec![],
    );
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "done", "{result:#}");
    let continued = continue_delivery(fixture.root_str());
    assert_eq!(continued["state"], "done", "{continued:#}");
    assert_eq!(
        active_phase_id(fixture.root_str(), &delivery_id),
        "phase-2".to_string()
    );
}

#[test]
fn review_environment_blocker_cannot_route_execution_repair() {
    let fixture = Fixture::new("review-env-blocker");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-env",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "review_limitation",
            "failureClass": "environment_blocker",
            "category": "environment_or_dependency",
            "summary": "Local browser dependency is unavailable.",
            "evidence": "The verification environment is unavailable.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "findings[].failureClass"
    }));
}

#[test]
fn repeated_invalid_review_result_stays_repairable() {
    let fixture = Fixture::new("review-invalid-repairable");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_candidate_target(
        &fixture,
        &review_request_ref,
        &json!({ "schemaVersion": "1.0" }),
    );

    let first = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    let second = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    let third = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(first["state"], "repairable_error", "{first:#}");
    assert_eq!(second["state"], "repairable_error", "{second:#}");
    assert_eq!(third["state"], "repairable_error", "{third:#}");
    assert_eq!(third["resubmitTool"], "loom.reviewAcceptFile");
    assert_eq!(third["fixScope"], "review_result_candidate_only");
}

#[test]
fn review_execution_repair_materializes_repair_task() {
    let fixture = Fixture::new("review-execution-repair");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-product",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "code",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "The implemented flow misses a required behavior.",
            "evidence": "Task result evidence does not cover the required behavior.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "yes",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["repairOrigin"], "review_result");
    assert_eq!(
        result["next"]["repairContext"]["reviewResultRef"],
        latest_ref_for_phase(
            fixture.root_str(),
            &request_delivery_id(fixture.root_str(), &review_request_ref),
            "reviewResult"
        )
    );
    assert_eq!(
        result["next"]["repairContext"]["findingRefs"],
        json!(["finding-product"])
    );
}

#[test]
fn manual_review_resolution_routes_to_execution_repair() {
    let fixture = Fixture::new("manual-review-resolution");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "blocked",
        "manual_review",
        vec![json!({
            "findingId": "finding-manual",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "manual",
            "failureClass": "contract_gap",
            "category": "review_limitation",
            "summary": "Manual decision is required.",
            "evidence": "The review requires user judgment.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "manual_review"
        })],
    );
    let gate = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    assert_eq!(gate["state"], "user_gate", "{gate:#}");
    assert!(gate["gate"].get("readGroups").is_none());
    assert!(gate["gate"].get("writeTargets").is_none());
    assert!(gate["gate"].get("submitTool").is_none());
    let manual_request_ref = gate["requestRef"]
        .as_str()
        .expect("manual review requestRef")
        .to_string();
    let manual_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: manual_request_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read manual review result template")
    .fields;
    assert!(manual_fields["outputContract.resultTemplate"].value["userAnswer"].is_object());
    assert!(manual_fields["outputContract.resultTemplate"].value["nextAction"].is_object());
    write_manual_review_resolution_candidate(&fixture, &manual_request_ref);

    let result = call_submit(
        "loom.reviewResolveFile",
        &manual_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["repairOrigin"], "manual_review_resolution");
    assert!(result["next"]["repairContext"]["manualReviewResolutionRef"]
        .as_str()
        .unwrap()
        .contains("/manual-resolutions/"));
    assert_eq!(
        result["next"]["repairContext"]["userChangeSummary"],
        "修复当前实现问题。"
    );
    let execution_repair_ref = result["next"]["requestRef"]
        .as_str()
        .expect("execution repair requestRef");
    let repair_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_repair_ref.to_string(),
        group_id: "repair_execution_core".to_string(),
    })
    .expect("read execution repair rules");
    let repair_rules_text =
        serde_json::to_string(&repair_rules.fields).expect("serialize repair rules");
    assert!(repair_rules_text.contains("Use repairContext as the failure boundary"));
    assert!(repair_rules_text.contains("rerun that signal"));
}

#[test]
fn taskplan_submit_repairs_runtime_requirement_shape_before_parse() {
    let fixture = Fixture::new("taskplan-runtime-requirement-shape");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["runtimeDeliveryRequirement"] = json!({
        "appliesToThisTask": true,
        "reason": "This task touches runtime delivery.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": ["manual_command_output"],
        "evidenceExpectedInTaskResult": ["runtimeDeliveryEvidence"],
        "forbiddenActions": []
    });
    write_json_atomic(&group_path, &group_value).expect("write invalid group file");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(result["targetFile"], json!(group_file));
    assert!(
        result["issues"].as_array().unwrap().iter().any(|issue| {
            issue["fieldPath"] == "tasks[0].runtimeDeliveryRequirement.requiredCodeLevelChecks[0]"
                && issue["code"] == "TASKPLAN_GROUP_SCHEMA_INVALID"
        }),
        "{result:#}"
    );
}

#[test]
fn taskplan_submit_repairs_task_enum_shape_before_parse() {
    let fixture = Fixture::new("taskplan-enum-shape");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["verificationIntents"][0]["acceptableEvidence"] =
        json!(["static_check", "shell_output"]);
    write_json_atomic(&group_path, &group_value).expect("write invalid group file");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(result["targetFile"], json!(group_file));
    assert!(
        result["issues"].as_array().unwrap().iter().any(|issue| {
            issue["fieldPath"] == "tasks[0].verificationIntents[0].acceptableEvidence[1]"
                && issue["code"] == "TASKPLAN_GROUP_SCHEMA_INVALID"
        }),
        "{result:#}"
    );
}

#[test]
fn taskplan_repair_submit_replaces_taskplan_and_starts_new_run() {
    let fixture = Fixture::new("repair-submit-taskplan");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    let old_run_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRun");
    let repair_result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &delivery_core::RouteAction {
            kind: delivery_core::RouteActionKind::TaskplanRepair,
            source: "test".to_string(),
            reason: "taskplan repair".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let repair_result = serde_json::to_value(repair_result).expect("repair result value");
    assert_eq!(repair_result["state"], "auto_runnable", "{repair_result:#}");
    assert_eq!(repair_result["next"]["artifactKind"], "taskplan_repair");
    assert_eq!(repair_result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_action_ref = repair_result["next"]["requestRef"]
        .as_str()
        .expect("repair action requestRef")
        .to_string();
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        fields: vec![
            "outputContract.runtimeDeliveryRequirementTemplate".to_string(),
            "outputContract.runtimeDeliveryClosureTaskTemplate".to_string(),
        ],
    })
    .expect("read taskplan repair runtime templates")
    .fields;
    assert!(
        repair_fields["outputContract.runtimeDeliveryRequirementTemplate"]
            .value
            .is_object()
    );
    assert!(
        repair_fields["outputContract.runtimeDeliveryClosureTaskTemplate"]
            .value
            .is_object()
    );
    let repair_inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
    })
    .expect("inspect taskplan repair request");
    assert!(repair_inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
        .any(|field| field == "outputContract.runtimeDeliveryRequirementTemplate"));
    write_taskplan_grouped_candidates(&fixture, &repair_action_ref);

    let result = call_submit(
        "loom.repairSubmitFile",
        &repair_action_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "planned_task");
    let new_run_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRun");
    assert_ne!(new_run_ref, old_run_ref);
}

#[test]
fn architecture_repair_submit_rebuilds_aac_and_recreates_taskplan_request() {
    let fixture = Fixture::new("repair-submit-architecture");
    let review_request_ref = complete_task_execution_to_review_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    let old_taskplan_request_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRequestRef");
    let repair_result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &delivery_core::RouteAction {
            kind: delivery_core::RouteActionKind::ArchitectureArtifactRepair,
            source: "test".to_string(),
            reason: "architecture repair".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let repair_result = serde_json::to_value(repair_result).expect("repair result value");
    assert_eq!(repair_result["state"], "auto_runnable", "{repair_result:#}");
    assert_eq!(
        repair_result["next"]["artifactKind"],
        "architecture_artifact_repair"
    );
    assert_eq!(repair_result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_action_ref = repair_result["next"]["requestRef"]
        .as_str()
        .expect("repair action requestRef")
        .to_string();
    assert_architecture_group_ids(
        &fixture,
        &repair_action_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    advance_architecture_to_section(&fixture, &repair_action_ref, "frontend_experience");
    assert_architecture_group_ids(
        &fixture,
        &repair_action_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_frontend_context",
        ],
    );
    let frontend_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        group_id: "architecture_frontend_context".to_string(),
    })
    .expect("read architecture repair frontend group");
    assert!(frontend_group
        .fields
        .get("frontendExperienceSource.confirmedFrontendExperienceRef")
        .or_else(|| frontend_group
            .fields
            .get("frontendExperienceSource.currentFrontendExperienceRef"))
        .is_some());
    let repair_root = read_request_root_value(fixture.root_str(), &repair_action_ref);
    let repair_coverage_template = repair_root["sectionOutputs"]
        .as_array()
        .expect("repair section outputs")
        .iter()
        .find(|section| section["section"].as_str() == Some("coverage"))
        .expect("repair coverage section")["resultTemplate"]["content"]
        .clone();
    assert_eq!(
        repair_coverage_template["acceptanceMatrix"][0]["acceptanceId"],
        json!("acc_1")
    );
    assert!(
        repair_coverage_template["acceptanceMatrix"][0]["statement"]
            .as_str()
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        "repair coverage template must preserve acceptance statements"
    );
    assert!(repair_coverage_template["acceptanceMatrix"][0]["coverage"].is_array());
    assert_eq!(
        repair_coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["kind"],
        json!("manual")
    );
    assert!(
        repair_coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["description"]
            .is_string()
    );
    let frontend_authority_ref = repair_root
        .pointer("/frontendExperienceSource/confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            repair_root
                .pointer("/frontendExperienceSource/currentFrontendExperienceRef")
                .and_then(Value::as_str)
        })
        .expect("repair frontend authority ref");
    let frontend_template_refs = repair_root["currentSectionContract"]["resultTemplate"]["content"]
        ["frontendExperience"]["sourceRefs"]
        .as_object()
        .expect("repair frontend sourceRefs");
    assert_eq!(frontend_template_refs.len(), 1);
    assert_eq!(
        frontend_template_refs["brainstormFrontendExperienceRef"],
        json!(frontend_authority_ref)
    );
    let result = complete_architecture_sections(&fixture, &repair_action_ref);

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(result["next"]["artifactKind"], "task_plan_candidate");
    assert_eq!(result["next"]["submitTool"], "loom.taskPlanAcceptFile");
    let new_taskplan_request_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRequestRef");
    assert_ne!(new_taskplan_request_ref, old_taskplan_request_ref);
}

#[test]
fn brainstorm_submit_rejects_stale_request_binding() {
    let fixture = Fixture::new("submit-stale-request");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
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
                "outputContract": {
                    "artifactKind": "brainstorm_candidate",
                    "submitTool": "loom.brainstormAcceptFile",
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
                        "fields": ["outputContract.writeTargets"]
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

fn start_brainstorm_candidate_write_request(fixture: &Fixture) -> String {
    let server = LoomMcpServer::default();
    let mut request_ref = start_brainstorm_request(fixture);

    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
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
                "reason": "证券账户是资金账户和交易链路的上游基础对象。"
            }
        }),
    )["requestRef"]
        .as_str()
        .expect("concept requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
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
        .expect("frontend requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
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
        .expect("final summary requestRef")
        .to_string();
    let write_action = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "final_summary",
        "用户已确认阶段范围、业务理解、页面办理路径和提交前核对。",
        json!({
            "coverageChecklist": ["证券账户模块闭环", "开户/挂失补办/销户规则", "工作人员后台办理路径"],
            "readyToWriteCandidate": true
        }),
    );
    assert_eq!(write_action["state"], "auto_runnable", "{write_action:#}");
    let candidate_request_ref = write_action["next"]["requestRef"]
        .as_str()
        .expect("candidate write requestRef")
        .to_string();
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: candidate_request_ref.clone(),
    })
    .expect("inspect candidate write request");
    assert_eq!(inspected.request_kind, "brainstorm_candidate_write");
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.brainstormAcceptFile")
    );
    assert_eq!(inspected.write_targets.len(), 1);
    assert!(inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "candidate_write_contract"));
    candidate_request_ref
}

fn confirm_brainstorm_block(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    block: &str,
    summary: &str,
    confirmed_data: Value,
) -> Value {
    if block != "final_summary" {
        run_knowledge_context(server, fixture, request_ref, block);
    }
    let arguments = json!({
        "projectRoot": fixture.root_str(),
        "requestRef": request_ref,
        "block": block,
        "summary": summary,
        "confirmedData": confirmed_data
    })
    .as_object()
    .expect("arguments object")
    .clone();
    let result = server
        .invoke_tool("loom.brainstormConfirmBlock", Some(arguments))
        .expect("confirm brainstorm block")
        .structured_content
        .expect("structured content");
    assert!(
        result["state"] == "user_gate" || result["state"] == "auto_runnable",
        "{result:#}"
    );
    result
}

fn run_knowledge_context(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    block: &str,
) {
    let knowledge_plan = server
        .invoke_tool(
            "loom.readFieldGroup",
            Some(
                json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "groupId": "knowledge_context_plan"
                })
                .as_object()
                .expect("arguments object")
                .clone(),
            ),
        )
        .expect("read knowledge context plan")
        .structured_content
        .expect("structured content");
    let field_name = format!("knowledgeQueryPlan.blocks.{block}.executionOrder");
    let steps = knowledge_plan["fields"][field_name]
        .as_array()
        .expect("knowledge executionOrder");
    for step in steps {
        let step_id = step["stepId"].as_str().expect("stepId");
        server
            .invoke_tool(
                "loom.knowledgeBrainstormContext",
                Some(
                    json!({
                        "projectRoot": fixture.root_str(),
                        "requestRef": request_ref,
                        "block": block,
                        "stepId": step_id,
                        "querySubject": format!("{block} {step_id}"),
                        "naturalLanguageQuery": "证券账户 开户 挂失 补办 销户 资金账户 交易 依赖 闭环",
                        "semanticFocus": ["证券账户", "开户", "挂失", "补办", "销户"]
                    })
                    .as_object()
                    .expect("arguments object")
                    .clone(),
                ),
            )
            .expect("knowledge brainstorm context");
    }
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
    start_existing_project_architecture_flow_with_candidate(fixture, valid_candidate_json())
}

fn start_existing_project_architecture_flow_with_candidate(
    fixture: &Fixture,
    candidate: Value,
) -> String {
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

    let request_ref = start_brainstorm_candidate_write_request(fixture);
    write_candidate_target(fixture, &request_ref, &candidate);

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

fn complete_architecture_sections(fixture: &Fixture, architecture_request_ref: &str) -> Value {
    let current_request_ref = architecture_request_ref.to_string();
    let mut last = json!(null);
    let section_order = [
        "foundation",
        "domain_contract",
        "behavior",
        "frontend_experience",
        "runtime_delivery",
        "coverage",
    ];
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: current_request_ref.clone(),
    })
    .expect("inspect current architecture request");
    let current_section = inspected.write_targets[0]["targetId"]
        .as_str()
        .expect("current architecture target");
    let start_index = section_order
        .iter()
        .position(|section| *section == current_section)
        .expect("known architecture section");
    for expected_section in section_order.iter().skip(start_index) {
        let inspected = state::inspect_request(InspectRequestInput {
            project_root: fixture.root_str().to_string(),
            request_ref: current_request_ref.clone(),
        })
        .expect("inspect current architecture request");
        assert_eq!(
            inspected.write_targets[0]["targetId"],
            json!(*expected_section)
        );

        write_candidate_target(
            fixture,
            &current_request_ref,
            &architecture_section_candidate_json(fixture, &current_request_ref),
        );

        let submit_tool = inspected.submit_tool.as_deref().expect("submit tool");
        last = call_submit(submit_tool, &current_request_ref, fixture.root_str());

        if *expected_section != "coverage" {
            assert_eq!(last["state"], "auto_runnable", "{last:#}");
            assert_eq!(
                last["next"]["requestRef"],
                json!(current_request_ref),
                "{last:#}"
            );
        }
    }
    last
}

fn advance_architecture_to_section(
    fixture: &Fixture,
    architecture_request_ref: &str,
    target: &str,
) {
    for _ in 0..6 {
        let inspected = state::inspect_request(InspectRequestInput {
            project_root: fixture.root_str().to_string(),
            request_ref: architecture_request_ref.to_string(),
        })
        .expect("inspect current architecture request");
        let current = inspected.write_targets[0]["targetId"]
            .as_str()
            .expect("current architecture target");
        if current == target {
            return;
        }
        assert_ne!(
            current, "coverage",
            "target section {target} was not reached"
        );
        write_candidate_target(
            fixture,
            architecture_request_ref,
            &architecture_section_candidate_json(fixture, architecture_request_ref),
        );
        let submit_tool = inspected.submit_tool.as_deref().expect("submit tool");
        let result = call_submit(submit_tool, architecture_request_ref, fixture.root_str());
        assert_eq!(result["state"], "auto_runnable", "{result:#}");
        assert_eq!(
            result["next"]["requestRef"],
            json!(architecture_request_ref)
        );
    }
    panic!("target section {target} was not reached");
}

fn assert_architecture_group_ids(fixture: &Fixture, request_ref: &str, expected: &[&str]) {
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect architecture request");
    let actual = inspected
        .read_groups
        .iter()
        .map(|group| group.group_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{actual:#?}");
}

fn write_taskplan_grouped_candidates(fixture: &Fixture, request_ref: &str) {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "allowedRefs.scopeRefs".to_string(),
            "allowedRefs.acceptanceRefs".to_string(),
            "allowedRefs.requirementDetailIds".to_string(),
            "allowedRefs.moduleRefs".to_string(),
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })
    .expect("read taskplan fields")
    .fields;
    let allowed_refs = json!({
        "scopeRefs": field_value(&fields, "allowedRefs.scopeRefs"),
        "acceptanceRefs": field_value(&fields, "allowedRefs.acceptanceRefs"),
        "requirementDetailIds": field_value(&fields, "allowedRefs.requirementDetailIds"),
        "moduleRefs": field_value(&fields, "allowedRefs.moduleRefs")
    });
    let scope_id = allowed_refs["scopeRefs"][0].as_str().expect("scope ref");
    let acceptance_id = allowed_refs["acceptanceRefs"][0]
        .as_str()
        .expect("acceptance ref");
    let detail_id = allowed_refs["requirementDetailIds"][0]
        .as_str()
        .expect("detail id");
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group file pattern");
    let group_id = "group-account";
    let task_id = "task-account-001";
    write_json_atomic(
        &fixture.root.join(outline_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "taskPlanId": "taskplan-phase-1",
            "groups": [{
                "groupId": group_id,
                "title": "Account capability",
                "objective": "Implement the account capability slice.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            }],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan outline");
    let group_file = group_pattern.replace("{groupId}", group_id);
    write_json_atomic(
        &fixture.root.join(group_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "group": {
                "groupId": group_id,
                "title": "Account capability",
                "objective": "Implement the account capability slice.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            },
            "tasks": [{
                "taskId": task_id,
                "groupId": group_id,
                "title": "Implement account flow",
                "taskKind": "feature_increment",
                "implementationActions": ["create_or_update_interface", "add_or_update_tests"],
                "objective": "Implement account lifecycle behavior with success feedback.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "requirementDetailRefs": [detail_id],
                "writeBoundary": {
                    "forbiddenPaths": [".loom"],
                    "artifactRefs": {
                        "modules": ["module.account-service"],
                        "entities": [],
                        "interfaces": [],
                        "userFlows": [],
                        "stateMachines": [],
                        "decisions": [],
                        "risks": []
                    }
                },
                "verificationIntents": [{
                    "verificationId": "verify-account-001",
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "behavior": "Verify account lifecycle behavior and visible success feedback.",
                    "preferredEvidence": ["static_check"],
                    "acceptableEvidence": ["static_check", "manual_command_output"]
                }],
                "conceptRefs": [],
                "conceptResponsibilities": [],
                "conceptVerificationIntents": []
            }],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan group");
}

fn first_taskplan_group_file(fixture: &Fixture, request_ref: &str) -> String {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })
    .expect("read taskplan file fields")
    .fields;
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group file pattern");
    let outline_path = fixture.root.join(outline_file);
    let outline_value: Value =
        serde_json::from_str(&std::fs::read_to_string(outline_path).expect("read outline file"))
            .expect("parse outline file");
    let group_id = outline_value["groups"][0]["groupId"]
        .as_str()
        .expect("first group id");
    group_pattern.replace("{groupId}", group_id)
}

fn write_task_result_candidate(fixture: &Fixture, request_ref: &str) {
    write_task_result_candidate_with_detail_evidence(fixture, request_ref, true, false);
}

fn write_large_task_result_candidate(fixture: &Fixture, request_ref: &str) {
    write_task_result_candidate_with_detail_evidence(fixture, request_ref, true, true);
}

fn write_task_result_candidate_without_requirement_detail_evidence(
    fixture: &Fixture,
    request_ref: &str,
) {
    write_task_result_candidate_with_detail_evidence(fixture, request_ref, false, false);
}

fn write_task_result_candidate_with_detail_evidence(
    fixture: &Fixture,
    request_ref: &str,
    include_detail_evidence: bool,
    include_large_text: bool,
) {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "task.requirementDetailRefs".to_string(),
            "task.verificationIntents".to_string(),
            "outputContract.resultFile".to_string(),
        ],
    })
    .expect("read execution request fields")
    .fields;
    let task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId");
    let task_id = fields["source.taskId"].value.as_str().expect("taskId");
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let detail_id = fields["task.requirementDetailRefs"].value[0]
        .as_str()
        .expect("requirement detail id");
    let verification_id = fields["task.verificationIntents"].value[0]["verificationId"]
        .as_str()
        .expect("verification id");
    let verification_summary = if include_large_text {
        format!("very large verification summary {}", "x".repeat(20_000))
    } else {
        "Static verification passed for the account flow.".to_string()
    };
    let detail_summary = if include_large_text {
        format!("very large detail evidence summary {}", "x".repeat(20_000))
    } else {
        "The account lifecycle detail is covered by the implemented flow and static verification."
            .to_string()
    };
    let result_notes = if include_large_text {
        json!([format!("very large execution note {}", "x".repeat(20_000))])
    } else {
        json!([])
    };
    let requirement_detail_evidence = if include_detail_evidence {
        json!([{
            "detailId": detail_id,
            "status": "satisfied",
            "verificationIds": [verification_id],
            "evidenceRefs": ["src/main.tsx"],
            "summary": detail_summary
        }])
    } else {
        json!([])
    };
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "taskResultId": "result-task-account-001",
            "taskId": task_id,
            "taskPlanId": task_plan_id,
            "status": "completed",
            "changedFiles": ["src/main.tsx"],
            "noChangeReason": null,
            "verificationResults": [{
                "verificationId": "verify-account-001",
                "status": "passed",
                "evidenceType": "static_check",
                "summary": verification_summary
            }],
            "selfRepairSummary": {
                "attempted": false,
                "attemptCount": 0,
                "stopReason": "not_attempted",
                "progressObserved": false
            },
            "failure": null,
            "executionContinuity": {
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none",
                "notes": []
            },
            "notes": result_notes,
            "frontendExperienceSelfCheck": null,
            "runtimeDeliveryEvidence": null,
            "requirementDetailEvidence": requirement_detail_evidence,
            "conceptEvidence": [],
            "blockedReasons": [],
            "createdAt": "2026-06-24T10:05:00+08:00",
            "updatedAt": "2026-06-24T10:05:00+08:00"
        }),
    )
    .expect("write task result");
}

fn complete_task_execution_to_review(fixture: &Fixture) -> String {
    complete_task_execution_to_review_with_candidate(fixture, valid_candidate_json())
}

fn complete_task_execution_to_review_with_candidate(fixture: &Fixture, candidate: Value) -> String {
    let execution_request_ref = start_planned_task_execution_with_candidate(fixture, candidate);
    write_task_result_candidate(fixture, &execution_request_ref);
    let task_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    assert_eq!(task_result["next"]["artifactKind"], "review_result");
    task_result["next"]["requestRef"]
        .as_str()
        .expect("review requestRef")
        .to_string()
}

fn start_planned_task_execution(fixture: &Fixture) -> String {
    start_planned_task_execution_with_candidate(fixture, valid_candidate_json())
}

fn start_planned_task_execution_with_candidate(fixture: &Fixture, candidate: Value) -> String {
    let architecture_request_ref =
        start_existing_project_architecture_flow_with_candidate(fixture, candidate);
    let taskplan_result = complete_architecture_sections(fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();
    write_taskplan_grouped_candidates(fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string()
}

fn write_failed_task_result_candidate(fixture: &Fixture, request_ref: &str) {
    let fields = execution_result_fields(fixture, request_ref);
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId");
    let task_id = fields["source.taskId"].value.as_str().expect("taskId");
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "taskResultId": "result-failed-task-account-001",
            "taskId": task_id,
            "taskPlanId": task_plan_id,
            "status": "failed",
            "changedFiles": [],
            "noChangeReason": null,
            "verificationResults": [{
                "verificationId": "verify-account-001",
                "status": "failed",
                "evidenceType": "static_check",
                "summary": "Static verification failed."
            }],
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_failed",
                "progressObserved": true
            },
            "failure": {
                "code": "VERIFICATION_FAILED",
                "summary": "Verification failed for the current task."
            },
            "executionContinuity": {
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none",
                "notes": []
            },
            "notes": [],
            "frontendExperienceSelfCheck": null,
            "runtimeDeliveryEvidence": null,
            "requirementDetailEvidence": [],
            "conceptEvidence": [],
            "blockedReasons": [],
            "createdAt": "2026-06-24T10:06:00+08:00",
            "updatedAt": "2026-06-24T10:06:00+08:00"
        }),
    )
    .expect("write failed task result");
}

fn write_blocked_task_result_candidate(
    fixture: &Fixture,
    request_ref: &str,
    code: &str,
    next_node: &str,
) {
    let fields = execution_result_fields(fixture, request_ref);
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId");
    let task_id = fields["source.taskId"].value.as_str().expect("taskId");
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "taskResultId": format!("result-blocked-{code}"),
            "taskId": task_id,
            "taskPlanId": task_plan_id,
            "status": "blocked",
            "changedFiles": [],
            "noChangeReason": {
                "code": "BLOCKED",
                "summary": "The task is blocked by an upstream contract issue."
            },
            "verificationResults": [{
                "verificationId": "verify-account-001",
                "status": "not_run",
                "evidenceType": "static_check",
                "summary": "Verification was not run because the task is blocked."
            }],
            "selfRepairSummary": {
                "attempted": false,
                "attemptCount": 0,
                "stopReason": "not_attempted",
                "progressObserved": false
            },
            "failure": null,
            "executionContinuity": {
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none",
                "notes": []
            },
            "notes": [],
            "frontendExperienceSelfCheck": null,
            "runtimeDeliveryEvidence": null,
            "requirementDetailEvidence": [],
            "conceptEvidence": [],
            "blockedReasons": [{
                "code": code,
                "nextNode": next_node,
                "message": "The task is blocked by an upstream contract issue.",
                "details": {}
            }],
            "createdAt": "2026-06-24T10:06:00+08:00",
            "updatedAt": "2026-06-24T10:06:00+08:00"
        }),
    )
    .expect("write blocked task result");
}

fn execution_result_fields(
    fixture: &Fixture,
    request_ref: &str,
) -> std::collections::BTreeMap<String, delivery_core::FieldReadResult> {
    state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "outputContract.resultFile".to_string(),
        ],
    })
    .expect("read execution request fields")
    .fields
}

fn write_review_result_candidate(
    fixture: &Fixture,
    request_ref: &str,
    decision: &str,
    next_action: &str,
    findings: Vec<Value>,
) {
    let finding_ids = findings
        .iter()
        .filter_map(|finding| finding.get("findingId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "source.phaseId".to_string(),
            "source.taskPlanId".to_string(),
            "source.taskPlanRunId".to_string(),
            "outputContract.resultFile".to_string(),
        ],
    })
    .expect("read review fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("review result file");
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "reviewId": "review-phase-1",
            "source": {
                "requestId": request_id_from_ref(request_ref),
                "phaseId": field_value(&fields, "source.phaseId"),
                "taskPlanId": field_value(&fields, "source.taskPlanId"),
                "taskPlanRunId": field_value(&fields, "source.taskPlanRunId")
            },
            "decision": decision,
            "findings": findings,
            "coverageAssessment": {
                "mustAcceptance": [],
                "summary": {
                    "totalMust": 0,
                    "satisfied": 0,
                    "insufficientEvidence": 0,
                    "notSatisfied": 0,
                    "notReviewed": 0
                }
            },
            "limitations": [],
            "pendingActions": [],
            "nextAction": {
                "type": next_action,
                "reason": "Review selected the next route.",
                "targetPhaseId": if next_action == "continue_to_next_phase" { json!("phase-2") } else { Value::Null },
                "findingRefs": if next_action == "done" || next_action == "continue_to_next_phase" { json!([]) } else { json!(finding_ids.clone()) }
            },
            "createdAt": "2026-06-24T10:10:00+08:00",
            "updatedAt": "2026-06-24T10:10:00+08:00"
        }),
    )
    .expect("write review result");
}

fn write_manual_review_resolution_candidate(fixture: &Fixture, request_ref: &str) {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read manual review fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("manual review resolution file");
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "manualReviewResolutionId": "manual-review-resolution-001",
            "manualReviewRequestId": request_id_from_ref(request_ref),
            "deliveryId": request_delivery_id(fixture.root_str(), request_ref),
            "phaseId": "phase-1",
            "userAnswer": {
                "text": "需要修改，请修复当前实现问题。",
                "selectedShortReply": "request_changes"
            },
            "decision": "request_changes",
            "changeRequest": {
                "summary": "修复当前实现问题。",
                "route": "execution_repair",
                "reason": "需要修改代码或验证证据。",
                "details": {}
            },
            "nextAction": {
                "type": "execution_repair",
                "targetNode": "execution",
                "reason": "User requested execution repair."
            },
            "createdAt": "2026-06-24T10:12:00+08:00"
        }),
    )
    .expect("write manual review resolution");
}

fn request_id_from_ref(request_ref: &str) -> String {
    request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref")
        .to_string()
}

fn field_value(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    field: &str,
) -> Value {
    fields
        .get(field)
        .map(|result| result.value.clone())
        .unwrap_or(Value::Null)
}

fn architecture_section_candidate_json(fixture: &Fixture, request_ref: &str) -> Value {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let section = request_root["sectionState"]["currentSection"]
        .as_str()
        .expect("currentSection");
    let mut requested_fields = vec![
        "sourceRefs.planningContractRef".to_string(),
        "sourceRefs.technicalBaselineRef".to_string(),
        "sourceRefs.brainstormContractRef".to_string(),
        "sourceRefs.repositoryContextRef".to_string(),
        "sourceRefs.deliveryConceptGlossaryRef".to_string(),
        "sourceRefs.phaseConceptGroundingRef".to_string(),
        "sourceRefs.confirmedFrontendExperienceRef".to_string(),
        "sourceRefs.currentFrontendExperienceRef".to_string(),
        "sourceRefs.previousRuntimeDeliveryRef".to_string(),
        "allowedRefs.scopeRefs".to_string(),
        "allowedRefs.acceptanceRefs".to_string(),
        "allowedRefs.deferredScopeRefs".to_string(),
        "allowedRefs.excludedScopeRefs".to_string(),
        "allowedRefs.requirementDetailIds".to_string(),
        "contextProjection.planningContractId".to_string(),
        "contextProjection.technicalBaseline.technicalBaselineId".to_string(),
        "contextProjection.requirementDetailTransfer.acceptanceDetails".to_string(),
        "contextProjection.requirementDetailTransfer.requirementDetails".to_string(),
    ];
    if section == "frontend_experience" {
        requested_fields
            .push("frontendExperienceSource.confirmedFrontendExperienceRef".to_string());
        requested_fields.push("frontendExperienceSource.currentFrontendExperienceRef".to_string());
    }
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: requested_fields,
    })
    .expect("read architecture request fields")
    .fields;
    let source_refs = json!({
        "planningContractRef": field_value(&fields, "sourceRefs.planningContractRef"),
        "technicalBaselineRef": field_value(&fields, "sourceRefs.technicalBaselineRef"),
        "brainstormContractRef": field_value(&fields, "sourceRefs.brainstormContractRef"),
        "repositoryContextRef": field_value(&fields, "sourceRefs.repositoryContextRef"),
        "deliveryConceptGlossaryRef": field_value(&fields, "sourceRefs.deliveryConceptGlossaryRef"),
        "phaseConceptGroundingRef": field_value(&fields, "sourceRefs.phaseConceptGroundingRef"),
        "confirmedFrontendExperienceRef": field_value(&fields, "sourceRefs.confirmedFrontendExperienceRef"),
        "currentFrontendExperienceRef": field_value(&fields, "sourceRefs.currentFrontendExperienceRef"),
        "previousRuntimeDeliveryRef": field_value(&fields, "sourceRefs.previousRuntimeDeliveryRef")
    });
    let allowed_refs = json!({
        "scopeRefs": field_value(&fields, "allowedRefs.scopeRefs"),
        "acceptanceRefs": field_value(&fields, "allowedRefs.acceptanceRefs"),
        "deferredScopeRefs": field_value(&fields, "allowedRefs.deferredScopeRefs"),
        "excludedScopeRefs": field_value(&fields, "allowedRefs.excludedScopeRefs"),
        "requirementDetailIds": field_value(&fields, "allowedRefs.requirementDetailIds")
    });
    let planning_contract_id = fields["contextProjection.planningContractId"]
        .value
        .as_str()
        .expect("planningContractId");
    let technical_baseline_id = fields["contextProjection.technicalBaseline.technicalBaselineId"]
        .value
        .as_str()
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
    let frontend_authority_ref = fields
        .get("frontendExperienceSource.confirmedFrontendExperienceRef")
        .and_then(|field| field.value.as_str())
        .or_else(|| {
            fields
                .get("frontendExperienceSource.currentFrontendExperienceRef")
                .and_then(|field| field.value.as_str())
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

fn assert_no_root_submit_metadata(root: &Value) {
    for key in [
        "artifactKind",
        "submitTool",
        "writeTargets",
        "writeMode",
        "outputContract",
    ] {
        assert!(
            root.get(key).is_none(),
            "compact request root must not duplicate {key}: {root:#}"
        );
    }
}

fn assert_no_read_plan_size_warnings(fixture: &Fixture) {
    let audit_path = fixture.root.join(".loom/metrics/request-size-audit.jsonl");
    let audit = std::fs::read_to_string(&audit_path).expect("read request size audit");
    for line in audit.lines() {
        let entry: Value = serde_json::from_str(line).expect("parse request size audit entry");
        let warnings = entry["readPlanWarnings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            warnings.is_empty(),
            "generated request should not produce read-plan size warnings: {entry:#}"
        );
    }
}

fn runtime_section_contract(request_root: &Value) -> &Value {
    request_root["sectionOutputs"]
        .as_array()
        .expect("sectionOutputs")
        .iter()
        .find(|section| section["section"] == json!("runtime_delivery"))
        .expect("runtime_delivery section contract")
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

fn active_phase_id(project_root: &str, delivery_id: &str) -> String {
    let index_path = std::path::Path::new(project_root)
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    index["activePhaseId"]
        .as_str()
        .expect("active phase id")
        .to_string()
}

fn append_done_phase(fixture: &Fixture, delivery_id: &str, phase_id: &str) {
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    index["phases"].as_array_mut().expect("phases").push(json!({
        "phaseId": phase_id,
        "latestRefs": {},
        "nextAction": {
            "kind": "done",
            "source": "test",
            "reason": "phase_done"
        }
    }));
    write_json_atomic(&index_path, &index).expect("write delivery index");
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

fn write_previous_technical_baseline(fixture: &Fixture, delivery_id: &str) {
    let baseline_path = fixture
        .root
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("contracts/technical-baseline.json");
    std::fs::create_dir_all(baseline_path.parent().expect("baseline parent"))
        .expect("create baseline parent");
    write_json_atomic(
        &baseline_path,
        &json!({
            "schemaVersion": "1.0",
            "technicalBaselineId": "tb_previous",
            "deliveryId": delivery_id,
            "phaseId": "phase-1",
            "status": "confirmed",
            "source": "user_confirmed",
            "projectKind": "existing_project",
            "scope": "project",
            "stack": {
                "frontend": "plain-html",
                "backend": "none"
            },
            "constraints": [],
            "evidence": [{
                "path": "README.md",
                "reason": "Previous baseline was confirmed earlier."
            }],
            "approval": {
                "type": "user_confirmed",
                "reason": "Existing baseline fixture."
            },
            "confidence": "high",
            "requiresUserConfirmation": false,
            "reasoningSummary": ["Previous baseline fixture."],
            "alternatives": [],
            "createdAt": "2026-06-24T09:00:00+08:00",
            "updatedAt": "2026-06-24T09:00:00+08:00"
        }),
    )
    .expect("write previous technical baseline");
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

fn valid_candidate_with_frontend_json() -> Value {
    let mut candidate = valid_candidate_json();
    candidate["frontendExperience"] = json!({
        "required": true,
        "kind": "staff_console",
        "experienceLevel": "usable_internal_product",
        "audiences": [{
            "audienceId": "audience_staff",
            "name": "工作人员",
            "primaryJobs": ["办理证券账户业务"]
        }],
        "surfaces": [{
            "surfaceId": "surface_account_admin",
            "name": "证券账户管理",
            "audienceRefs": ["audience_staff"],
            "primaryJobs": ["开户", "挂失补办", "销户"]
        }],
        "dataViews": [{
            "viewId": "view_account_list",
            "name": "证券账户列表",
            "purpose": "查询并选择证券账户办理后续动作",
            "targetObject": "证券账户",
            "selectionMode": "query_and_select",
            "paginationRequired": true,
            "defaultLoadsFirstPage": true,
            "searchCriteria": [{
                "criterionId": "criterion_account_no",
                "label": "证券账户号",
                "fieldRef": "securityAccountNo",
                "reason": "工作人员按账户号定位办理对象",
                "sourceRefs": []
            }],
            "sourceRefs": []
        }],
        "actions": [{
            "actionId": "action_open_account",
            "label": "新建证券账户",
            "targetObject": "证券账户",
            "entryPoint": "navigation_entry",
            "inputFields": ["个人开户信息", "法人开户信息"],
            "resultObservation": ["response_message", "list_refresh"],
            "refreshPolicy": "办理完成后返回列表并刷新状态",
            "successFeedback": ["开户成功并显示证券账户号"],
            "blockingOrErrorFeedback": ["开户资格不满足时显示中文阻断原因"],
            "sourceRefs": []
        }],
        "operationPaths": [{
            "pathId": "path_account_lifecycle",
            "name": "证券账户生命周期办理",
            "userGoal": "工作人员完成证券账户开户、挂失补办和销户办理",
            "surfaceRef": "surface_account_admin",
            "targetObject": "证券账户",
            "selectionMode": "query_and_select",
            "selectionSummary": "开户从新建入口进入；挂失补办和销户先查询列表并选择目标账户",
            "dataViewRefs": ["view_account_list"],
            "actionRefs": ["action_open_account"],
            "requiredStates": ["success", "business_blocking", "error"],
            "sourceRefs": []
        }],
        "mustNot": ["不能只靠内部主键触发挂失补办或销户"],
        "confirmationSummary": "已确认工作人员后台证券账户管理页面路径。"
    });
    candidate["clarificationProgress"]["confirmedBlocks"] = json!([
        {
            "block": "phase_scope",
            "summary": "已确认当前阶段范围。",
            "confirmedByUser": true
        },
        {
            "block": "concept_grounding",
            "summary": "已确认业务规则和边界。",
            "confirmedByUser": true
        },
        {
            "block": "frontend_experience",
            "summary": "已确认页面办理路径。",
            "confirmedByUser": true
        }
    ]);
    candidate["clarificationProgress"]["skippedBlocks"] = json!([]);
    candidate["userConfirmation"]["confirmationBasis"]["presentedItems"] = json!([
        "phase_scope",
        "concept_grounding",
        "frontend_experience",
        "final_summary"
    ]);
    candidate
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
