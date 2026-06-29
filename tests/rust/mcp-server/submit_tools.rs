use std::{
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard},
};

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
        .get("brainstormLens.roadmap.phases")
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
        "brainstormLens.roadmapPhaseIndex",
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
    let repository_context_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repository context requestRef");
    let generation_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_generation_rules".to_string(),
    })
    .expect("read repository context generation rules");
    for field in [
        "enumRefs.repositoryShape",
        "enumRefs.capabilityStatus",
        "enumRefs.surfaceRelevance",
        "enumRefs.suggestedUse",
        "enumRefs.contextCoverage",
        "enumRefs.confidence",
    ] {
        assert!(
            generation_rules.fields.get(field).is_some(),
            "missing RepositoryContext enum field {field}"
        );
    }
    assert_eq!(
        generation_rules.fields["enumRefs.contextCoverage"].value,
        json!(["focused", "partial", "broad", "insufficient"])
    );
    assert_eq!(
        generation_rules.fields["enumRefs.surfaceRelevance"].value,
        json!([
            "implemented_capability",
            "architecture_boundary",
            "extension_point",
            "validation_surface",
            "delivery_context",
            "unrelated"
        ])
    );
    let write_contract = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_write_contract".to_string(),
    })
    .expect("read repository context write contract");
    assert_eq!(
        write_contract.fields["outputContract.resultTemplate"].value["contextQuality"]["coverage"],
        "focused"
    );
    assert_eq!(
        write_contract.fields["outputContract.resultTemplate"].value["contextQuality"]["warnings"]
            [0]["code"],
        "LOW_CONFIDENCE_REPOSITORY_SCAN"
    );
    assert!(
        write_contract.fields["outputContract.resultTemplate"].value["contextQuality"]["warnings"]
            [0]["message"]
            .as_str()
            .expect("contextQuality warning message")
            .contains("Use [] only when there are no warnings")
    );
    assert_eq!(
        write_contract.fields["outputContract.resultTemplate"].value["warnings"][0]["code"],
        "LOW_CONFIDENCE_REPOSITORY_SCAN"
    );
    assert!(
        write_contract.fields["outputContract.bindingRules"].value[0]
            .as_str()
            .expect("binding rule")
            .contains("source.requestRef")
    );
    assert!(
        write_contract.fields["outputContract.resultTemplate"].value["source"]
            .get("requestRef")
            .is_some()
    );
    assert!(
        write_contract.fields["outputContract.resultTemplate"].value["source"]
            .get("requestId")
            .is_none()
    );
    assert_eq!(
        write_contract.fields["outputContract.schemaProjection"].value["enumFields"]
            ["contextQuality.coverage"],
        "enumRefs.contextCoverage"
    );
    assert!(
        write_contract.fields["outputContract.schemaProjection"].value["objectShapeRules"]
            ["technologySignals"]
            .as_str()
            .expect("technologySignals shape rule")
            .contains("object")
    );
    assert!(
        write_contract.fields["outputContract.schemaProjection"].value["objectShapeRules"]
            ["contextQuality.warnings[]"]
            .as_str()
            .expect("contextQuality warnings shape rule")
            .contains("code and message")
    );
    assert!(
        write_contract.fields["outputContract.schemaProjection"].value["objectShapeRules"]
            ["warnings[]"]
            .as_str()
            .expect("warnings shape rule")
            .contains("code and message")
    );

    let mut repository_candidate = write_contract.fields["outputContract.resultTemplate"]
        .value
        .clone();
    repository_candidate["source"]["requestRef"] = json!(repository_context_request_ref);
    write_candidate_target(
        &fixture,
        repository_context_request_ref,
        &repository_candidate,
    );
    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        repository_context_request_ref,
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
fn greenfield_technical_baseline_needing_confirmation_uses_user_gate() {
    let fixture = Fixture::new("technical-baseline-greenfield-user-gate");
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
    let mut candidate = greenfield_technical_baseline_candidate_json();
    candidate["status"] = json!("needs_user_confirmation");
    candidate["approval"] = json!({
        "type": "none",
        "reason": "The recommended stack still needs user confirmation."
    });
    candidate["requiresUserConfirmation"] = json!(true);
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "user_gate", "{result:#}");
    assert_eq!(result["gate"]["gateId"], "greenfield_baseline_confirmation");
}

#[test]
fn greenfield_technical_baseline_requires_complete_track_model() {
    let fixture = Fixture::new("technical-baseline-greenfield-tracks");
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
    let mut candidate = greenfield_technical_baseline_candidate_json();
    candidate["stack"] = json!({
        "frontend": "vite-react",
        "backend": "spring-boot",
        "database": "sqlite"
    });
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(
            |issue| issue["code"] == "GREENFIELD_BASELINE_TRACKS_INCOMPLETE"
                && issue["fieldPath"] == "stack.tracks"
        ));
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
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_domain_model_context",
        ],
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

    assert!(request_root
        .pointer("/sourceRefs/previousRuntimeDeliveryRef")
        .is_none());
    assert!(
        request_root.get("sectionOutputs").is_none(),
        "architecture request root must not expose all section contracts"
    );
    let runtime_contract =
        architecture_section_contract(&fixture, &architecture_request_ref, "runtime_delivery");
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
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_domain_model_context",
        ],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let core_group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture core group");
    assert!(!core_group
        .fields
        .contains_key("sourceRefs.previousRuntimeDeliveryRef"));
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref,
    })
    .expect("inspect architecture request");
    let inspect_core_fields = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "architecture_core_context")
        .expect("architecture core read group")
        .fields
        .clone();
    assert!(!inspect_core_fields.contains(&"sourceRefs.previousRuntimeDeliveryRef".to_string()));
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
    assert!(
        request_root.get("sectionOutputs").is_none(),
        "architecture request root must not expose all section contracts"
    );
    let runtime_contract =
        architecture_section_contract(&fixture, &refreshed_ref, "runtime_delivery");
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
        request_ref: refreshed_ref.clone(),
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture core group");
    assert_eq!(
        core_group.fields["sourceRefs.previousRuntimeDeliveryRef"].value,
        json!(previous_runtime_ref)
    );
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_ref,
    })
    .expect("inspect architecture request");
    let inspect_core_fields = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "architecture_core_context")
        .expect("architecture core read group")
        .fields
        .clone();
    assert!(inspect_core_fields.contains(&"sourceRefs.previousRuntimeDeliveryRef".to_string()));
}

#[test]
fn architecture_section_submit_advances_same_request_to_next_section() {
    let fixture = Fixture::new("architecture-next-section");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let architecture_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);
    assert!(
        architecture_root.get("sectionOutputs").is_none(),
        "architecture request root must not expose all section contracts"
    );
    assert!(architecture_root["currentSectionContract"]["resultTemplate"]["content"].is_object());
    let architecture_rules =
        architecture_root["currentSectionContract"]["generationRules"].to_string();
    assert!(architecture_rules.contains("existing project and technical baseline shape"));
    assert!(architecture_rules.contains("avoid pass-through wrappers"));
    let coverage_template =
        architecture_section_contract(&fixture, &architecture_request_ref, "coverage")
            ["resultTemplate"]["content"]
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
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["coverage"][0]["type"],
        json!("modules")
    );
    assert!(coverage_template["acceptanceMatrix"][0]["coverage"][0]["refs"].is_array());
    assert!(coverage_template["acceptanceMatrix"][0]["coverage"][0]["description"].is_string());
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
    let frontend_template =
        architecture_section_contract(&fixture, &architecture_request_ref, "frontend_experience");
    let frontend_template_refs = frontend_template["resultTemplate"]["content"]
        ["frontendExperience"]["sourceRefs"]
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
fn planning_contract_preserves_brainstorm_requirement_detail_index() {
    let fixture = Fixture::new("pgc-detail-index");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        candidate_with_planning_details_json(),
    );
    let architecture_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);
    let source_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        fields: vec!["sourceRefs.planningContractRef".to_string()],
    })
    .expect("read planning contract ref")
    .fields;
    let planning_contract_ref = source_fields["sourceRefs.planningContractRef"]
        .value
        .as_str()
        .expect("planning contract ref");
    let planning_contract: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(planning_contract_ref))
            .expect("read planning contract"),
    )
    .expect("parse planning contract");

    assert_eq!(
        planning_contract["planningInputs"]["actors"][0]["id"],
        json!("actor_staff")
    );
    assert_eq!(
        planning_contract["planningInputs"]["capabilityGroups"][0]["id"],
        json!("cap_account_opening")
    );

    let details = planning_contract["requirementDetails"]["items"]
        .as_array()
        .expect("requirement details");
    for expected_detail_id in [
        "detail.scope.scope_1.1",
        "detail.scope.scope_1.2",
        "detail.deferred.deferred_1.1",
        "detail.excluded.excluded_1.1",
        "detail.acceptance.acc_1",
        "detail.businessFlow.flow_account_opening",
        "detail.concept.concept_security_account",
        "detail.frontend.view.view_account_list",
        "detail.frontend.action.action_open_account",
        "detail.frontend.path_account_lifecycle",
        "detail.assumption.assumption_1",
    ] {
        assert!(
            details
                .iter()
                .any(|detail| detail["detailId"] == json!(expected_detail_id)),
            "missing PGC requirement detail {expected_detail_id}"
        );
    }
    let allowed_impact_tags = [
        "scope",
        "data_model",
        "business_flow",
        "frontend",
        "interface",
        "acceptance",
        "runtime",
    ];
    let allowed_lifecycle_stages = [
        "create",
        "query_select",
        "view",
        "update",
        "approve_or_process",
        "state_change",
        "terminate_or_cancel",
        "blocking_or_exception",
        "not_applicable",
    ];
    let allowed_qualities = ["thin", "usable", "rich"];
    for detail in details {
        assert!(
            allowed_qualities.contains(&detail["quality"].as_str().expect("quality")),
            "invalid PGC detail quality: {detail:#}"
        );
        assert!(
            allowed_lifecycle_stages
                .contains(&detail["lifecycleStage"].as_str().expect("lifecycleStage")),
            "invalid PGC detail lifecycleStage: {detail:#}"
        );
        for tag in detail["impactTags"].as_array().expect("impactTags") {
            assert!(
                allowed_impact_tags.contains(&tag.as_str().expect("impact tag")),
                "invalid PGC detail impactTag: {detail:#}"
            );
        }
    }
    assert!(details.iter().any(|detail| {
        detail["detailId"] == json!("detail.concept.concept_security_account")
            && detail["conceptRefs"] == json!(["concept_security_account"])
    }));
    let first_scope_detail = details
        .iter()
        .find(|detail| detail["detailId"] == json!("detail.scope.scope_1.1"))
        .expect("first scope detail");
    assert_eq!(
        first_scope_detail["sourceFieldRefs"],
        json!(["brainstorm.scope.included[0].items[0]"])
    );

    let core_group = architecture_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("read groups")
        .iter()
        .find(|group| group["groupId"] == json!("architecture_core_context"))
        .expect("architecture core group");
    let core_fields = core_group["fields"].as_array().expect("core fields");
    assert!(core_fields.contains(&json!(
        "contextProjection.requirementDetailTransfer.requirementDetails"
    )));
    assert!(core_fields.contains(&json!(
        "contextProjection.requirementDetailTransfer.businessFlows"
    )));
    assert!(!core_fields.contains(&json!("contextProjection")));
    assert!(!core_fields.contains(&json!("contextProjection.requirementDetailTransfer")));
    assert!(!core_fields.contains(&json!(
        "contextProjection.requirementDetailTransfer.frontendExperienceDetails"
    )));
    let domain_group = architecture_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("read groups")
        .iter()
        .find(|group| group["groupId"] == json!("architecture_domain_model_context"))
        .expect("domain model group");
    assert_eq!(
        domain_group["fields"],
        json!([
            "contextProjection.requirementDetailTransfer.actors",
            "contextProjection.requirementDetailTransfer.capabilityGroups"
        ])
    );
    let projected_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        fields: vec!["contextProjection.requirementDetailTransfer.requirementDetails".to_string()],
    })
    .expect("read projected requirement details")
    .fields;
    let projected_details =
        &projected_fields["contextProjection.requirementDetailTransfer.requirementDetails"].value;
    assert!(projected_details["items"][0]
        .get("sourceFieldRefs")
        .is_none());
    assert!(projected_details.get("extractionWarnings").is_none());
    assert!(
        projected_details["extractionWarningCount"].is_number(),
        "projected details did not expose extractionWarningCount: {projected_details:#}"
    );
    assert_eq!(
        projected_details["fullDetailSource"],
        "sourceRefs.planningContractRef#/requirementDetails"
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
    candidate["content"]["acceptanceMatrix"][0]["coverage"] = json!(["module.account-service"]);
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
        issue["code"] == "ACCEPTANCE_MATRIX_INVALID"
            && issue["fieldPath"] == "content.acceptanceMatrix[0].coverage[0].type"
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
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptResponsibilities"][0]
            .is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptVerificationIntents"][0]
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
fn taskplan_request_keeps_deferred_scope_out_of_current_scope_refs() {
    let fixture = Fixture::new("taskplan-scope-boundary");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        candidate_with_planning_details_json(),
    );
    let result = complete_architecture_sections(&fixture, &architecture_request_ref);
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let taskplan_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "allowedRefs.scopeRefs".to_string(),
            "allowedRefs.deferredScopeRefs".to_string(),
            "allowedRefs.excludedScopeRefs".to_string(),
            "contextProjection.requirementDetailTransfer.requirementDetailAssignment".to_string(),
        ],
    })
    .expect("read taskplan scope fields")
    .fields;

    assert_eq!(fields["allowedRefs.scopeRefs"].value, json!(["scope_1"]));
    assert_eq!(
        fields["allowedRefs.deferredScopeRefs"].value,
        json!(["deferred_1"])
    );
    assert_eq!(
        fields["allowedRefs.excludedScopeRefs"].value,
        json!(["excluded_1"])
    );
    let assignment =
        &fields["contextProjection.requirementDetailTransfer.requirementDetailAssignment"].value;
    let item = &assignment["items"][0];
    assert!(item.get("coverage").is_none());
    assert!(item["quality"].is_string());
    assert!(item["coverageStatus"].is_string());
    assert!(item["artifactRefs"].is_object() || item["artifactRefs"].is_null());
    assert!(item["coverageReason"].is_null() || item["coverageReason"].is_string());
}

#[test]
fn taskplan_request_derives_frontend_workflow_closure_requirements() {
    let fixture = Fixture::new("taskplan-workflow-closure");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_json,
    );
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let taskplan_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    let closure_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "contextProjection.requirementDetailTransfer.workflowClosureRequirements".to_string(),
            "generationRules.workflowClosureRules".to_string(),
        ],
    })
    .expect("read workflow closure fields")
    .fields;
    let requirements = closure_fields
        ["contextProjection.requirementDetailTransfer.workflowClosureRequirements"]
        .value
        .as_array()
        .expect("workflow closure requirements");
    assert_eq!(requirements.len(), 1, "{requirements:#?}");
    assert_eq!(
        requirements[0]["closureId"],
        json!("closure:flow.account-lifecycle:step.submit-open-account")
    );
    assert_eq!(
        requirements[0]["interfaceRefs"],
        json!(["api.account.open"])
    );
    assert_eq!(
        requirements[0]["operationPathRefs"],
        json!(["path_account_lifecycle"])
    );
    assert_eq!(
        requirements[0]["requiredEvidence"],
        json!([
            "user_action",
            "declared_interface_invocation",
            "state_or_persistence_change",
            "success_or_blocking_feedback"
        ])
    );
    assert_eq!(
        closure_fields["generationRules.workflowClosureRules"].value["requirementSource"],
        json!("contextProjection.requirementDetailTransfer.workflowClosureRequirements")
    );
}

#[test]
fn task_execution_request_carries_task_scoped_frontend_closure_guidance() {
    let fixture = Fixture::new("task-exec-frontend-guidance");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, taskplan_request_ref);
    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_eq!(accepted["next"]["kind"], "execute_task");
    assert_eq!(accepted["next"]["submitTool"], "loom.recordTaskResultFile");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");

    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
    })
    .expect("inspect execution request");
    let core_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_core")
        .expect("core group");
    assert!(core_group.fields.contains(
        &"task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs".to_string()
    ));
    assert!(core_group
        .fields
        .contains(&"executionRules.frontendImplementationOrganizationRules".to_string()));
    assert!(core_group
        .fields
        .contains(&"executionRules.interactiveVerificationProbePolicy".to_string()));
    assert!(core_group
        .fields
        .contains(&"executionRules.controlledRuntimeProbeRules".to_string()));
    assert!(!core_group
        .fields
        .contains(&"sourceContext.architectureArtifactProjection.frontendExperience".to_string()));

    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        fields: vec![
            "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"
                .to_string(),
            "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"
                .to_string(),
            "sourceContext.architectureArtifactProjection.interfaces".to_string(),
            "executionRules.frontendImplementationOrganizationRules".to_string(),
            "executionRules.interactiveVerificationProbePolicy".to_string(),
            "executionRules.controlledRuntimeProbeRules".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.requiredTopLevelFields".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution fields")
    .fields;
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"].value
            [0]["closureId"],
        json!("closure:flow.account-lifecycle:step.submit-open-account")
    );
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"]
            .value[0]["interfaces"][0]["interfaceId"],
        json!("api.account.open")
    );
    let interfaces = &fields["sourceContext.architectureArtifactProjection.interfaces"].value;
    assert_eq!(interfaces.as_array().expect("interfaces").len(), 1);
    assert_eq!(interfaces[0]["interfaceId"], json!("api.account.open"));
    assert!(serde_json::to_string(
        &fields["executionRules.frontendImplementationOrganizationRules"].value
    )
    .unwrap()
    .contains("reachable entry"));
    assert!(serde_json::to_string(
        &fields["executionRules.interactiveVerificationProbePolicy"].value
    )
    .unwrap()
    .contains("smallest applicable probe plan"));
    assert!(
        serde_json::to_string(&fields["executionRules.controlledRuntimeProbeRules"].value)
            .unwrap()
            .contains("Never run long-lived runtime")
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["frontendExperienceSelfCheck"]
            ["closureRequirementIds"],
        json!(["closure:flow.account-lifecycle:step.submit-open-account"])
    );
    assert!(fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendExperienceSelfCheck")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("runtimeDeliveryEvidence")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("conceptEvidence")));
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/App.tsx"]);
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write task result");
    let record_result = call_submit(
        "loom.recordTaskResultFile",
        execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(record_result["state"], "auto_runnable", "{record_result:#}");
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
    assert_eq!(
        execution_result["stopAllowed"],
        json!(false),
        "{execution_result:#}"
    );
    assert!(execution_result.get("continuationPolicy").is_none());
    assert!(execution_result["agentInstruction"]
        .as_str()
        .expect("agent instruction")
        .contains("Do not stop at a progress recap"));
    assert_eq!(execution_result["next"]["kind"], "execute_task");
    assert_eq!(
        execution_result["next"]["submitTool"],
        "loom.recordTaskResultFile"
    );
    let execution_request_ref = execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let resumed_execution = continue_delivery(fixture.root_str());
    assert_eq!(resumed_execution["state"], "auto_runnable");
    assert_eq!(resumed_execution["next"]["kind"], "execute_task");
    assert_eq!(
        resumed_execution["next"]["requestRef"],
        json!(execution_request_ref)
    );
    assert_eq!(
        resumed_execution["stopAllowed"],
        json!(false),
        "{resumed_execution:#}"
    );
    assert!(resumed_execution.get("continuationPolicy").is_none());
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
            "outputContract.requiredTopLevelFields".to_string(),
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
    assert!(!execution_fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendExperienceSelfCheck")));
    assert!(!execution_fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("runtimeDeliveryEvidence")));
    assert!(!execution_fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("conceptEvidence")));
    assert!(execution_fields["outputContract.resultTemplate"]
        .value
        .get("frontendExperienceSelfCheck")
        .is_none());
    assert!(execution_fields["outputContract.resultTemplate"]
        .value
        .get("runtimeDeliveryEvidence")
        .is_none());
    assert!(execution_fields["outputContract.resultTemplate"]
        .value
        .get("conceptEvidence")
        .is_none());
    let execution_rules_text =
        serde_json::to_string(&execution_fields).expect("serialize execution rules");
    assert!(execution_rules_text.contains("write-producing verification commands"));
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
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
        .all(
            |field| field != "executionRules.frontendImplementationOrganizationRules"
                && field != "executionRules.interactiveVerificationProbePolicy"
                && field != "executionRules.controlledRuntimeProbeRules"
                && field != "executionRules.runtimeDeliveryExecutionRules"
                && field != "task.runtimeDeliveryRequirement"
                && field != "outputContract.schemaShape.properties.frontendExperienceSelfCheck"
                && field != "outputContract.schemaShape.properties.runtimeDeliveryEvidence"
                && field != "outputContract.schemaShape.properties.conceptEvidence"
        ));

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
    let resumed_task_result_repair = continue_delivery(fixture.root_str());
    assert_eq!(
        resumed_task_result_repair["state"], "auto_runnable",
        "{resumed_task_result_repair:#}"
    );
    assert_eq!(
        resumed_task_result_repair["next"]["requestRef"],
        json!(task_result_repair_action_ref),
        "{resumed_task_result_repair:#}"
    );
    assert_eq!(
        resumed_task_result_repair["next"]["artifactKind"],
        "task_result_repair"
    );
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
    assert_eq!(
        task_result_repair_fields["outputContract.resultTemplate"].value["changedFiles"],
        json!(["src/main.tsx"])
    );
    assert!(
        task_result_repair_fields["outputContract.resultTemplate"].value
            ["requirementDetailEvidence"][0]
            .is_object()
    );
    let task_result_repair_root =
        read_request_root_value(fixture.root_str(), &task_result_repair_action_ref);
    assert!(task_result_repair_root["source"].get("issues").is_none());
    let task_result_repair_read_plan_fields = task_result_repair_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("repair read groups")
        .iter()
        .flat_map(|group| group["fields"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(task_result_repair_read_plan_fields.contains(&"outputContract.resultTemplate"));
    assert!(task_result_repair_read_plan_fields.contains(&"repairContract.issueConflicts"));
    assert!(task_result_repair_read_plan_fields.contains(&"repairContract.minimalRepairRules"));
    assert!(!task_result_repair_read_plan_fields.contains(&"source.issues"));
    let repair_contract = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: task_result_repair_action_ref.clone(),
        group_id: "task_result_repair_context".to_string(),
    })
    .expect("read task result repair context");
    assert_eq!(
        repair_contract.fields["repairContract.profile"].value,
        json!("minimal_task_result_repair")
    );
    assert!(repair_contract.fields["repairContract.issueConflicts"]
        .value
        .is_array());
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
    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    let latest_refs = index["phases"][0]["latestRefs"]
        .as_object()
        .expect("latest refs");
    assert!(
        !latest_refs.contains_key("activeTaskResultRepairActionRef"),
        "accepted repair must clear stale activeTaskResultRepairActionRef: {latest_refs:#?}"
    );
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
    assert!(
        review_root.get("changeSet").is_none(),
        "review request root must not expose full changeSet: {review_root:#}"
    );
    assert!(
        review_root.get("reviewSignals").is_none(),
        "review request root must not duplicate outputContract.reviewSignals: {review_root:#}"
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
    let review_matrices_group = review_inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "review_matrices")
        .expect("review_matrices group");
    assert!(review_matrices_group
        .fields
        .iter()
        .any(|field| field == "outputContract.reviewSignals.items"));
    assert!(!review_matrices_group
        .fields
        .iter()
        .any(|field| field.starts_with("reviewSignals.")));
    assert!(!review_matrices_group.fields.iter().any(|field| {
        field == "outputContract.reviewSignals.requirementDetailEvidence"
            || field == "outputContract.reviewSignals.frontendWorkflowClosure"
    }));
    let review_matrices = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_matrices".to_string(),
    })
    .expect("read review matrices");
    let review_signals = review_matrices.fields["outputContract.reviewSignals.items"]
        .value
        .as_array()
        .expect("review signals");
    assert!(review_signals
        .iter()
        .any(|signal| signal["kind"] == json!("task_run_summary")));
    assert!(review_signals
        .iter()
        .any(|signal| signal["kind"] == json!("requirement_detail_evidence")));
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
fn runtime_task_execution_request_uses_field_level_runtime_rules() {
    let fixture = Fixture::new("task-exec-runtime-rules");
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
        "reason": "This task changes runtime delivery wiring.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": [{
            "checkId": "check-runtime-wiring",
            "contractField": "runtimeSurfaces",
            "objective": "Verify runtime surface wiring still works.",
            "acceptableEvidence": ["runtime_api_check", "static_check"]
        }],
        "evidenceExpectedInTaskResult": ["runtimeDeliveryEvidence"],
        "forbiddenActions": []
    });
    write_json_atomic(&group_path, &group_value).expect("write runtime group file");

    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
    })
    .expect("inspect runtime execution request");
    let read_fields = inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
        .cloned()
        .collect::<Vec<_>>();
    assert!(read_fields.contains(&"executionRules.controlledRuntimeProbeRules".to_string()));
    assert!(read_fields.contains(&"executionRules.runtimeDeliveryExecutionRules".to_string()));
    assert!(read_fields
        .contains(&"task.runtimeDeliveryRequirement.requiredCodeLevelChecks".to_string()));
    assert!(!read_fields.contains(&"task.runtimeDeliveryRequirement".to_string()));
    assert!(!read_fields
        .contains(&"executionRules.frontendImplementationOrganizationRules".to_string()));

    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        fields: vec![
            "task.runtimeDeliveryRequirement.requiredCodeLevelChecks".to_string(),
            "executionRules.controlledRuntimeProbeRules".to_string(),
            "executionRules.runtimeDeliveryExecutionRules".to_string(),
            "outputContract.requiredTopLevelFields".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read runtime execution fields")
    .fields;
    assert_eq!(
        fields["task.runtimeDeliveryRequirement.requiredCodeLevelChecks"].value[0]["checkId"],
        json!("check-runtime-wiring")
    );
    assert!(
        serde_json::to_string(&fields["executionRules.controlledRuntimeProbeRules"].value)
            .unwrap()
            .contains("foreground blocking verification commands")
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["runtimeDeliveryEvidence"]["codeLevelChecks"]
            [0]["checkId"],
        json!("check-runtime-wiring")
    );
    assert!(fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("runtimeDeliveryEvidence")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendExperienceSelfCheck")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("conceptEvidence")));
}

#[test]
fn task_result_repair_template_resets_conflicting_runtime_evidence() {
    let fixture = Fixture::new("task-result-repair-runtime-template");
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
        "reason": "This task changes runtime delivery wiring.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": [{
            "checkId": "check-runtime-wiring",
            "contractField": "runtimeSurfaces",
            "objective": "Verify runtime surface wiring still works.",
            "acceptableEvidence": ["runtime_api_check", "static_check"]
        }],
        "evidenceExpectedInTaskResult": ["runtimeDeliveryEvidence"],
        "forbiddenActions": []
    });
    write_json_atomic(&group_path, &group_value).expect("write runtime group file");

    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read runtime result template")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/runtime.ts"]);
    result["runtimeDeliveryEvidence"]["codeLevelChecks"][0]["checkId"] =
        json!("wrong-runtime-check");
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write bad task result");

    let invalid = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(invalid["state"], "auto_runnable", "{invalid:#}");
    assert_eq!(invalid["next"]["artifactKind"], "task_result_repair");
    let repair_request_ref = invalid["next"]["requestRef"]
        .as_str()
        .expect("repair request ref")
        .to_string();
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref,
        fields: vec![
            "outputContract.resultTemplate".to_string(),
            "repairContract.issueConflicts".to_string(),
        ],
    })
    .expect("read task result repair template")
    .fields;

    assert_eq!(
        repair_fields["outputContract.resultTemplate"].value["runtimeDeliveryEvidence"]
            ["codeLevelChecks"][0]["checkId"],
        json!("check-runtime-wiring")
    );
    assert!(
        serde_json::to_string(&repair_fields["repairContract.issueConflicts"].value)
            .expect("serialize issue conflicts")
            .contains("wrong-runtime-check")
    );
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
    let resumed = continue_delivery(fixture.root_str());
    assert_eq!(resumed["state"], "auto_runnable", "{resumed:#}");
    assert_eq!(
        resumed["next"]["executionKind"],
        "delivery_execution_repair"
    );
    assert_eq!(resumed["next"]["requestRef"], json!(repair_request_ref));
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec![
            "outputContract.resultTemplate".to_string(),
            "outputContract.blockedReasonOptions".to_string(),
        ],
    })
    .expect("read execution repair result template")
    .fields;
    assert!(
        repair_fields["outputContract.resultTemplate"].value["verificationResults"][0].is_object()
    );
    assert!(repair_fields["outputContract.blockedReasonOptions"]
        .value
        .is_array());
    let repair_root = read_request_root_value(fixture.root_str(), repair_request_ref);
    assert!(repair_root.get("taskConceptGrounding").is_none());
    assert!(repair_root.get("blockedOutput").is_none());
    remove_task_result_optional_validation_fields(&fixture, repair_request_ref);
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
fn failed_task_result_routes_to_review_after_four_failed_attempts() {
    let fixture = Fixture::new("failed-task-result-retry-budget");
    let mut request_ref = start_planned_task_execution(&fixture);

    for attempt in 1..=3 {
        write_failed_task_result_candidate_with_id(
            &fixture,
            &request_ref,
            &format!("result-failed-task-account-{attempt:03}"),
        );
        let result = call_submit(
            "loom.recordTaskResultFile",
            &request_ref,
            fixture.root_str(),
        );
        assert_eq!(
            result["state"], "auto_runnable",
            "attempt {attempt}: {result:#}"
        );
        assert_eq!(
            result["next"]["executionKind"], "delivery_execution_repair",
            "attempt {attempt}: {result:#}"
        );
        assert_eq!(
            result["next"]["repairContext"]["attemptCount"], attempt,
            "attempt {attempt}: {result:#}"
        );
        request_ref = result["next"]["requestRef"]
            .as_str()
            .expect("next repair requestRef")
            .to_string();
    }

    write_failed_task_result_candidate_with_id(
        &fixture,
        &request_ref,
        "result-failed-task-account-004",
    );
    let result = call_submit(
        "loom.recordTaskResultFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(
        result["next"]["artifactKind"], "review_result",
        "{result:#}"
    );
    assert_eq!(result["next"]["submitTool"], "loom.reviewAcceptFile");
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
    let resumed = continue_delivery(fixture.root_str());
    assert_eq!(resumed["state"], "auto_runnable", "{resumed:#}");
    assert_eq!(resumed["next"]["artifactKind"], "taskplan_repair");
    assert_eq!(resumed["next"]["requestRef"], json!(repair_request_ref));
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
    assert!(
        repair_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptResponsibilities"][0]
            .is_object()
    );
    assert!(
        repair_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptVerificationIntents"][0]
            .is_object()
    );
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
    let resumed = continue_delivery(fixture.root_str());
    assert_eq!(resumed["state"], "auto_runnable", "{resumed:#}");
    assert_eq!(
        resumed["next"]["artifactKind"],
        "architecture_artifact_repair"
    );
    assert_eq!(resumed["next"]["requestRef"], json!(repair_request_ref));
    let repair_root = read_request_root_value(fixture.root_str(), repair_request_ref);
    assert!(
        repair_root.get("sectionOutputs").is_none(),
        "architecture repair request root must not expose all section contracts"
    );
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
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read delivery index"))
            .expect("parse delivery index");
    assert_eq!(index["status"], "completed");
    let status: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/status.json")).expect("read status"),
    )
    .expect("parse status");
    assert_eq!(status["activeDeliveryId"], Value::Null);
    assert_eq!(status["lastCompletedDeliveryId"], delivery_id);
    assert_eq!(status["deliveries"][0]["status"], "completed");
    let continued = continue_delivery(fixture.root_str());
    assert_eq!(continued["state"], "done", "{continued:#}");
}

#[test]
fn review_accept_rejects_continue_without_next_phase_preview() {
    let fixture = Fixture::new("review-continue-without-preview");
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

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "nextAction.type"
    }));
    assert_eq!(
        active_phase_id(fixture.root_str(), &delivery_id),
        "phase-1".to_string()
    );
}

#[test]
fn review_accept_approved_materializes_next_phase_from_preview() {
    let fixture = Fixture::new("review-auto-next-phase");
    let review_request_ref = complete_task_execution_to_review_with_candidate(
        &fixture,
        candidate_with_next_phase_preview(),
    );
    let template_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read review template")
    .fields;
    assert_eq!(
        template_fields["outputContract.resultTemplate"].value["nextAction"]["type"],
        "continue_to_next_phase"
    );
    assert_eq!(
        template_fields["outputContract.resultTemplate"].value["nextAction"]["targetPhaseId"],
        "phase-2"
    );

    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(
        result["next"]["artifactKind"], "repository_context_candidate",
        "{result:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    assert_eq!(
        active_phase_id(fixture.root_str(), &delivery_id),
        "phase-2".to_string()
    );
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    assert_eq!(index["status"], "planning");
    let phase_2 = index["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .find(|phase| phase["phaseId"] == "phase-2")
        .expect("phase-2");
    assert_eq!(phase_2["nextAction"]["kind"], "repository_context_request");
    assert!(phase_2["latestRefs"]["brainstormRequestRef"].is_null());
    assert!(phase_2["latestRefs"]["technicalBaseline"].is_string());

    let repository_context_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repository context request ref");
    let brainstorm_contract_ref = phase_2["latestRefs"]["brainstormContract"]
        .as_str()
        .expect("brainstorm contract ref");
    let technical_baseline_ref =
        format!(".loom/deliveries/{delivery_id}/contracts/technical-baseline.json");
    write_candidate_target(
        &fixture,
        repository_context_request_ref,
        &repository_context_candidate_json(
            repository_context_request_ref,
            brainstorm_contract_ref,
            &technical_baseline_ref,
        ),
    );

    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        repository_context_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repository_result["state"], "user_gate",
        "{repository_result:#}"
    );
    assert_eq!(
        repository_result["gate"]["gateId"],
        "phase_brainstorm_required"
    );
    let phase_2_request_ref = repository_result["requestRef"]
        .as_str()
        .expect("phase-2 brainstorm request ref");
    let refreshed_index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let refreshed_index: Value = serde_json::from_str(
        &std::fs::read_to_string(refreshed_index_path).expect("read refreshed index"),
    )
    .expect("parse refreshed index");
    let refreshed_phase_2 = refreshed_index["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .find(|phase| phase["phaseId"] == "phase-2")
        .expect("refreshed phase-2");
    assert_eq!(
        refreshed_phase_2["latestRefs"]["brainstormRequestRef"],
        phase_2_request_ref
    );
    let phase_2_request = read_request_root_value(fixture.root_str(), phase_2_request_ref);
    assert_eq!(phase_2_request["nextPhaseSeed"]["phaseId"], "phase-2");
    assert_eq!(
        phase_2_request["nextPhaseSeed"]["title"],
        "资金账户基础能力"
    );
    assert_eq!(
        phase_2_request["nextPhaseSeed"]["scopePreview"],
        json!(["资金账户开户", "密码管理", "存款与取款", "账户关联"])
    );
    let phase_2_read_groups = phase_2_request["requestReadPlan"]["groups"]
        .as_array()
        .expect("read groups");
    assert!(
        phase_2_read_groups
            .iter()
            .any(|group| group["groupId"] == "knowledge_context_plan"),
        "phase continuation Brainstorm request must keep request-scoped knowledge reads"
    );
    let next_phase_seed_group = phase_2_read_groups
        .iter()
        .find(|group| group["groupId"] == "next_phase_seed")
        .expect("next phase seed group");
    assert_eq!(
        next_phase_seed_group["fields"],
        json!([
            "nextPhaseSeed.fromPhaseId",
            "nextPhaseSeed.phaseId",
            "nextPhaseSeed.title",
            "nextPhaseSeed.goal",
            "nextPhaseSeed.scopePreview",
            "nextPhaseSeed.reason",
            "nextPhaseSeed.usageRule"
        ])
    );
    let phase_continuation_group = phase_2_read_groups
        .iter()
        .find(|group| group["groupId"] == "phase_continuation_context")
        .expect("phase continuation context group");
    let phase_continuation_fields = phase_continuation_group["fields"]
        .as_array()
        .expect("phase continuation fields");
    for broad_field in [
        "phaseContinuationContext.activePhase",
        "deliveryContext.scope.deferred",
        "latestRepositoryContext.existingCapabilities",
        "latestRepositoryContext.relevantSurfaces",
        "confirmedRequirementDecisionsIndex.decisions",
    ] {
        assert!(
            !phase_continuation_fields
                .iter()
                .any(|field| field.as_str() == Some(broad_field)),
            "broad read field leaked into phase continuation group: {broad_field}"
        );
    }
    for stale_or_duplicate_prefix in [
        "deliveryContext.phasePlan.current.",
        "deliveryContext.phasePlan.nextPhasePreview.",
        "latestConfirmedRequirementDecision.phasePlan.current.",
        "latestConfirmedRequirementDecision.phasePlan.nextPhasePreview.",
    ] {
        assert!(
            !phase_continuation_fields.iter().any(|field| field
                .as_str()
                .is_some_and(|field| field.starts_with(stale_or_duplicate_prefix))),
            "stale or duplicate phase read leaked into phase continuation group: {stale_or_duplicate_prefix}"
        );
    }
    for required_field in [
        "phaseContinuationContext.activePhase.phaseId",
        "phaseContinuationContext.activePhase.title",
        "phaseContinuationContext.activePhase.goal",
        "phaseContinuationContext.repository.repoSummary",
        "phaseContinuationContext.repository.capabilitySummaries",
        "phaseContinuationContext.repository.surfaceSummaries",
    ] {
        assert!(
            phase_continuation_fields
                .iter()
                .any(|field| field.as_str() == Some(required_field)),
            "missing field-level phase continuation read: {required_field}"
        );
    }

    let phase_2_candidate_request_ref =
        confirm_phase2_brainstorm_to_candidate_write(&fixture, phase_2_request_ref);
    let phase_2_candidate_request =
        read_request_root_value(fixture.root_str(), &phase_2_candidate_request_ref);
    assert_eq!(
        phase_2_candidate_request["postSubmit"]["nextAction"]["kind"],
        "planning_contract_create"
    );
    write_candidate_target(
        &fixture,
        &phase_2_candidate_request_ref,
        &phase2_candidate_json(),
    );
    let phase_2_brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &phase_2_candidate_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        phase_2_brainstorm_result["state"], "auto_runnable",
        "{phase_2_brainstorm_result:#}"
    );
    assert_eq!(
        phase_2_brainstorm_result["next"]["artifactKind"],
        "architecture_section_candidate"
    );
    let phase_2_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "brainstormContract");
    let phase_2_contract: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(&phase_2_contract_ref))
            .expect("read phase-2 brainstorm contract"),
    )
    .expect("parse phase-2 brainstorm contract");
    assert_eq!(
        phase_2_contract["handoff"]["nextNode"],
        "planning_generation_contract"
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
fn review_accept_allows_normalized_changed_file_refs() {
    let fixture = Fixture::new("review-changed-file-ref-normalization");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-file-ref",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "code",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "The changed file evidence supports this finding.",
            "evidence": "The changed file was read through a normalized changed_file ref.",
            "readRefs": [{"type": "changed_file", "ref": "./src/main.tsx", "reason": "Read the changed file."}],
            "evidenceRefs": [{"type": "changed_file", "ref": "file:src/main.tsx", "reason": "Changed file evidence."}],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
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
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
}

#[test]
fn review_accept_rejects_pending_action_refs_with_wrong_route() {
    let fixture = Fixture::new("review-pending-action-route-mismatch");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "blocked",
        "architecture_artifact_repair",
        vec![json!({
            "findingId": "finding-arch",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "contract",
            "failureClass": "contract_gap",
            "category": "architecture_design_gap",
            "summary": "Architecture contract is missing required detail.",
            "evidence": "The review packet shows a contract gap.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "architecture_artifact_repair"
        })],
    );
    mutate_review_result_candidate(&fixture, &review_request_ref, |candidate| {
        candidate["pendingActions"] = json!([{
            "type": "taskplan_repair",
            "findingRefs": ["finding-arch"],
            "reason": "Wrong route for this finding."
        }]);
    });

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "pendingActions[].findingRefs"
    }));
}

#[test]
fn review_accept_rejects_warning_only_repair_route() {
    let fixture = Fixture::new("review-warning-only-repair");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "approved_with_notes",
        "execution_repair",
        vec![json!({
            "findingId": "finding-warning",
            "severity": "minor",
            "severityClass": "warning",
            "evidenceKind": "code",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "Non-blocking warning.",
            "evidence": "This should not route repair.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
            "introducedByCurrentTask": "yes",
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
            && issue["fieldPath"] == "nextAction.type"
    }));
}

#[test]
fn review_request_uses_git_diff_refs_without_inlining_diffs() {
    let fixture = Fixture::new("review-git-diff-refs");
    let execution_request_ref = start_planned_task_execution(&fixture);
    run_git(&fixture, &["init"]);
    run_git(&fixture, &["config", "user.email", "loom@example.test"]);
    run_git(&fixture, &["config", "user.name", "Loom Test"]);
    run_git(&fixture, &["add", "."]);
    run_git(&fixture, &["commit", "-m", "test: baseline"]);
    std::fs::write(
        fixture.root.join("src/main.tsx"),
        "export const app = 'changed';\n",
    )
    .expect("modify tracked source");
    write_task_result_candidate(&fixture, &execution_request_ref);
    let task_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    let review_request_ref = task_result["next"]["requestRef"]
        .as_str()
        .expect("review requestRef");

    let change_context = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "change_context".to_string(),
    })
    .expect("read change context");
    assert_eq!(
        change_context.fields["changeContext.mode"].value,
        json!("git_diff_ref")
    );
    let changed_files = change_context.fields["changeContext.changedFiles"]
        .value
        .as_array()
        .expect("changed files");
    let diff_ref = changed_files[0]["diffRef"].as_str().expect("diffRef");
    assert!(fixture.root.join(diff_ref).exists());
    let request_root = read_request_root_value(fixture.root_str(), review_request_ref);
    assert!(
        request_root.get("changeContext").is_none(),
        "diff refs must stay in private request storage"
    );
    assert!(
        request_root.get("changeSet").is_none(),
        "changeSet must stay in private request storage"
    );
    assert!(!serde_json::to_string(&request_root)
        .expect("serialize request root")
        .contains("export const app = 'changed'"));
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
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_domain_model_context",
        ],
    );
    let domain_model_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        group_id: "architecture_domain_model_context".to_string(),
    })
    .expect("read architecture repair domain model group");
    assert!(domain_model_group
        .fields
        .get("contextProjection.requirementDetailTransfer.actors")
        .is_some());
    assert!(domain_model_group
        .fields
        .get("contextProjection.requirementDetailTransfer.capabilityGroups")
        .is_some());
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
    assert!(
        repair_root.get("sectionOutputs").is_none(),
        "architecture repair request root must not expose all section contracts"
    );
    let repair_coverage_template =
        architecture_section_contract(&fixture, &repair_action_ref, "coverage")["resultTemplate"]
            ["content"]
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
    assert_eq!(
        repair_coverage_template["acceptanceMatrix"][0]["coverage"][0]["type"],
        json!("modules")
    );
    assert!(repair_coverage_template["acceptanceMatrix"][0]["coverage"][0]["refs"].is_array());
    assert!(
        repair_coverage_template["acceptanceMatrix"][0]["coverage"][0]["description"].is_string()
    );
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

fn confirm_phase2_brainstorm_to_candidate_write(
    fixture: &Fixture,
    phase_2_request_ref: &str,
) -> String {
    let server = LoomMcpServer::default();
    let mut request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        phase_2_request_ref,
        "phase_scope",
        "确认第二阶段为资金账户基础能力。",
        json!({
            "scope": {
                "included": ["资金账户开户", "密码管理", "存款与取款", "证券账户与资金账户关联"],
                "deferred": ["交易客户端", "中央撮合", "行情发布"],
                "excluded": []
            },
            "recommendation": {
                "label": "资金账户基础能力",
                "reason": "资金账户承接已完成的证券账户闭环，是交易客户端和撮合前的依赖。"
            }
        }),
    )["requestRef"]
        .as_str()
        .expect("phase2 concept requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "concept_grounding",
        "确认资金账户业务规则、状态和边界。",
        json!({
            "objects": ["资金账户", "证券账户关联关系"],
            "operations": ["开户", "密码管理", "存款", "取款", "账户关联"],
            "rules": ["资金账户负责现金余额和资金冻结", "证券账户负责交易身份和持仓", "资金账户销户前需要清空余额和解除关联"],
            "boundaries": ["交易撮合递延", "行情发布递延"]
        }),
    )["requestRef"]
        .as_str()
        .expect("phase2 frontend requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "frontend_experience",
        "确认工作人员后台资金账户管理页面路径。",
        json!({
            "required": true,
            "surfaces": ["资金账户管理页面"],
            "targetDiscovery": ["分页查询列表", "按投资者、资金账户号、关联证券账户查询"],
            "operationPaths": ["开户从新建入口进入", "存取款和关联操作先查询并选择目标账户"],
            "mustNot": ["不能把资金账户能力混入证券账户模块"]
        }),
    )["requestRef"]
        .as_str()
        .expect("phase2 final summary requestRef")
        .to_string();
    let write_action = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "final_summary",
        "用户已确认第二阶段范围、业务理解、页面办理路径和提交前核对。",
        json!({
            "coverageChecklist": ["资金账户基础能力", "开户/密码/存取款/账户关联规则", "工作人员后台办理路径"],
            "readyToWriteCandidate": true
        }),
    );
    assert_eq!(write_action["state"], "auto_runnable", "{write_action:#}");
    write_action["next"]["requestRef"]
        .as_str()
        .expect("phase2 candidate write requestRef")
        .to_string()
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
        if step["repeatMode"].as_str() == Some("per_candidate_phase_cut") {
            for query_id in ["capability_closure_A", "capability_closure_B"] {
                server
                    .invoke_tool(
                        "loom.knowledgeBrainstormContext",
                        Some(
                            json!({
                                "projectRoot": fixture.root_str(),
                                "requestRef": request_ref,
                                "block": block,
                                "stepId": step_id,
                                "queryId": query_id,
                                "querySubject": format!("{block} {step_id} {query_id}"),
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
            continue;
        }
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
    complete_architecture_sections_with(
        fixture,
        architecture_request_ref,
        architecture_section_candidate_json,
    )
}

fn complete_architecture_sections_with(
    fixture: &Fixture,
    architecture_request_ref: &str,
    candidate_fn: fn(&Fixture, &str) -> Value,
) -> Value {
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
            &candidate_fn(fixture, &current_request_ref),
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

fn write_taskplan_grouped_candidates_for_workflow_closure(fixture: &Fixture, request_ref: &str) {
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
            "allowedRefs.entityRefs".to_string(),
            "allowedRefs.interfaceRefs".to_string(),
            "allowedRefs.userFlowRefs".to_string(),
            "allowedRefs.stateMachineRefs".to_string(),
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })
    .expect("read taskplan fields")
    .fields;
    let scope_id = fields["allowedRefs.scopeRefs"].value[0]
        .as_str()
        .expect("scope ref");
    let acceptance_id = fields["allowedRefs.acceptanceRefs"].value[0]
        .as_str()
        .expect("acceptance ref");
    let detail_id = fields["allowedRefs.requirementDetailIds"].value[0]
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
    let group_id = "group-account-ui";
    let task_id = "task-account-ui-001";
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
                "title": "Account UI workflow",
                "objective": "Wire the account UI workflow to the declared backend interface.",
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
                "title": "Account UI workflow",
                "objective": "Wire the account UI workflow to the declared backend interface.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            },
            "tasks": [{
                "taskId": task_id,
                "groupId": group_id,
                "title": "Wire account opening UI",
                "taskKind": "ui_flow_increment",
                "implementationActions": ["wire_reference_in_api_or_ui", "add_or_update_tests"],
                "objective": "Wire the account opening UI action to the declared open-account API and verify success feedback.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "requirementDetailRefs": [detail_id],
                "writeBoundary": {
                    "forbiddenPaths": [".loom"],
                    "artifactRefs": {
                        "modules": ["module.account-service"],
                        "entities": ["entity.account"],
                        "interfaces": ["api.account.open"],
                        "userFlows": ["flow.account-lifecycle"],
                        "stateMachines": ["machine.account-status"],
                        "decisions": [],
                        "risks": []
                    }
                },
                "verificationIntents": [{
                    "verificationId": "verify-account-ui-001",
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "behavior": "Verify the UI action invokes the declared API and shows success feedback.",
                    "preferredEvidence": ["runtime_api_check"],
                    "acceptableEvidence": ["automated_test", "runtime_api_check", "manual_command_output"]
                }],
                "frontendExperienceRequirement": {
                    "frontendExperienceRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience",
                    "experienceLevel": "usable_internal_product",
                    "mustSatisfy": true
                },
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

fn write_task_result_candidate_with_empty_changed_files(fixture: &Fixture, request_ref: &str) {
    write_task_result_candidate(fixture, request_ref);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read result file")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let result_path = fixture.root.join(result_file);
    let mut result: Value =
        serde_json::from_str(&std::fs::read_to_string(&result_path).expect("read task result"))
            .expect("parse task result");
    result["changedFiles"] = json!([]);
    write_json_atomic(&result_path, &result).expect("write empty changedFiles task result");
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

#[test]
fn task_result_repair_template_preserves_previous_changed_files_for_replacement() {
    let fixture = Fixture::new("task-result-repair-preserves-changed-files");
    let execution_request_ref = start_planned_task_execution(&fixture);

    write_task_result_candidate(&fixture, &execution_request_ref);
    let accepted = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");

    write_task_result_candidate_with_empty_changed_files(&fixture, &execution_request_ref);
    let invalid_replacement = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        invalid_replacement["state"], "auto_runnable",
        "{invalid_replacement:#}"
    );
    assert_eq!(
        invalid_replacement["next"]["artifactKind"],
        "task_result_repair"
    );
    let repair_request_ref = invalid_replacement["next"]["requestRef"]
        .as_str()
        .expect("repair request ref");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read repair fields")
    .fields;

    assert_eq!(
        repair_fields["outputContract.resultTemplate"].value["changedFiles"],
        json!(["src/main.tsx"])
    );
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("frontendExperienceSelfCheck")
        .is_none());
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("runtimeDeliveryEvidence")
        .is_none());
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("conceptEvidence")
        .is_none());
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
    write_failed_task_result_candidate_with_id(
        fixture,
        request_ref,
        "result-failed-task-account-001",
    );
}

fn write_failed_task_result_candidate_with_id(
    fixture: &Fixture,
    request_ref: &str,
    task_result_id: &str,
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
            "taskResultId": task_result_id,
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
            "outputContract.allowedRefs.taskIds".to_string(),
            "outputContract.allowedRefs.acceptanceRefs".to_string(),
            "outputContract.allowedRefs.taskResultIds".to_string(),
        ],
    })
    .expect("read review fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("review result file");
    let first_task_id = fields["outputContract.allowedRefs.taskIds"]
        .value
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(str::to_string);
    let findings = findings
        .into_iter()
        .map(|mut finding| {
            if matches!(
                finding.get("severity").and_then(Value::as_str),
                Some("critical" | "major")
            ) {
                finding["taskRelevance"] = json!("direct");
                finding["scopeRelation"] = json!("within_task_changed_files");
                let route = finding
                    .get("recommendedNextAction")
                    .and_then(Value::as_str)
                    .unwrap_or("done");
                let has_refs = finding
                    .get("taskRefs")
                    .and_then(Value::as_array)
                    .map(|items| !items.is_empty())
                    .unwrap_or(false);
                if !has_refs && !matches!(route, "manual_review" | "needs_user_decision") {
                    if let Some(task_id) = &first_task_id {
                        finding["taskRefs"] = json!([task_id]);
                    }
                }
            }
            finding
        })
        .collect::<Vec<_>>();
    let acceptance_refs = fields["outputContract.allowedRefs.acceptanceRefs"]
        .value
        .as_array()
        .cloned()
        .unwrap_or_default();
    let task_result_ids = fields["outputContract.allowedRefs.taskResultIds"]
        .value
        .as_array()
        .cloned()
        .unwrap_or_default();
    let supporting_task_results = task_result_ids
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let must_acceptance = acceptance_refs
        .iter()
        .filter_map(Value::as_str)
        .map(|acceptance_ref| {
            json!({
                "acceptanceRef": acceptance_ref,
                "status": "satisfied",
                "supportingTaskResults": supporting_task_results,
                "evidenceStatus": "sufficient",
                "notes": []
            })
        })
        .collect::<Vec<_>>();
    let total_must = must_acceptance.len();
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
                "mustAcceptance": must_acceptance,
                "summary": {
                    "totalMust": total_must,
                    "satisfied": total_must,
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

fn review_result_candidate_path(fixture: &Fixture, request_ref: &str) -> PathBuf {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read review result file")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("review result file");
    fixture.root.join(result_file)
}

fn mutate_review_result_candidate<F>(fixture: &Fixture, request_ref: &str, mutate: F)
where
    F: FnOnce(&mut Value),
{
    let path = review_result_candidate_path(fixture, request_ref);
    let mut candidate: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read review result"))
            .expect("parse review result");
    mutate(&mut candidate);
    write_json_atomic(&path, &candidate).expect("write mutated review result");
}

fn run_git(fixture: &Fixture, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(&fixture.root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect architecture request");
    let readable_fields = inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter().cloned())
        .collect::<Vec<_>>();
    let requested_fields = [
        "allowedRefs.scopeRefs",
        "allowedRefs.acceptanceRefs",
        "allowedRefs.deferredScopeRefs",
        "allowedRefs.excludedScopeRefs",
        "allowedRefs.requirementDetailIds",
        "contextProjection.planningContractId",
        "contextProjection.technicalBaseline.technicalBaselineId",
        "contextProjection.requirementDetailTransfer.acceptanceDetails",
        "contextProjection.requirementDetailTransfer.requirementDetails",
        "frontendExperienceSource.confirmedFrontendExperienceRef",
        "frontendExperienceSource.currentFrontendExperienceRef",
    ]
    .iter()
    .filter(|field| readable_fields.contains(&field.to_string()))
    .map(|field| field.to_string())
    .collect::<Vec<_>>();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: requested_fields,
    })
    .expect("read architecture request fields")
    .fields;
    let source_refs = request_root
        .get("sourceRefs")
        .cloned()
        .unwrap_or_else(|| json!({}));
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
    let frontend_authority_ref = request_root
        .pointer("/frontendExperienceSource/confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            request_root
                .pointer("/frontendExperienceSource/currentFrontendExperienceRef")
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

fn architecture_section_candidate_with_workflow_closure_json(
    fixture: &Fixture,
    request_ref: &str,
) -> Value {
    let mut candidate = architecture_section_candidate_json(fixture, request_ref);
    match candidate["section"].as_str().unwrap_or_default() {
        "domain_contract" => {
            candidate["content"]["dataModel"]["entities"] = json!([{
                "entityId": "entity.account",
                "name": "Account",
                "type": "aggregate",
                "moduleRefs": ["module.account-service"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "fields": [{
                    "fieldId": "field.account.id",
                    "name": "accountId",
                    "type": "string",
                    "required": true
                }],
                "constraints": []
            }]);
            candidate["content"]["interfaces"] = json!([{
                "interfaceId": "api.account.open",
                "name": "Open account API",
                "type": "http_api",
                "role": "command",
                "method": "POST",
                "path": "/api/accounts",
                "moduleRefs": ["module.account-service"],
                "entityRefs": ["entity.account"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "requestSchema": [{
                    "fieldId": "field.request.investorName",
                    "name": "investorName",
                    "type": "string",
                    "required": true
                }],
                "responseSchema": [{
                    "fieldId": "field.response.accountId",
                    "name": "accountId",
                    "type": "string",
                    "required": true
                }],
                "errorSchema": [{
                    "fieldId": "field.error.message",
                    "name": "message",
                    "type": "string",
                    "required": true
                }]
            }]);
        }
        "behavior" => {
            candidate["content"]["userFlows"] = json!([{
                "flowId": "flow.account-lifecycle",
                "name": "Account lifecycle",
                "kind": "user_interaction",
                "moduleRefs": ["module.account-service"],
                "interfaceRefs": ["api.account.open"],
                "entityRefs": ["entity.account"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "entry": {
                    "type": "frontend_action",
                    "ref": "action_open_account"
                },
                "steps": [{
                    "stepId": "step.submit-open-account",
                    "actor": "工作人员",
                    "action": "提交开户表单",
                    "interfaceRefs": ["api.account.open"],
                    "stateMachineRefs": ["machine.account-status"]
                }],
                "outcomes": [{
                    "type": "success",
                    "description": "返回开户成功反馈并刷新账户列表。"
                }]
            }]);
            candidate["content"]["stateMachines"] = json!([{
                "machineId": "machine.account-status",
                "name": "Account status",
                "entityRef": "entity.account",
                "entityRefs": ["entity.account"],
                "moduleRefs": ["module.account-service"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "states": [{"stateId": "active", "name": "正常", "terminal": false}],
                "transitions": [],
                "rules": []
            }]);
        }
        "frontend_experience" => {
            let refs = candidate["content"]["frontendExperience"]["sourceRefs"].clone();
            candidate["content"]["frontendExperience"] = json!({
                "required": true,
                "kind": "staff_console",
                "experienceLevel": "usable_internal_product",
                "surfaces": [{
                    "surfaceId": "surface_account_admin",
                    "name": "证券账户管理",
                    "workflowRefs": ["flow.account-lifecycle"],
                    "audienceRefs": ["audience_staff"]
                }],
                "dataViews": [{
                    "viewId": "view_account_list",
                    "name": "证券账户列表"
                }],
                "actions": [{
                    "actionId": "action_open_account",
                    "label": "新建证券账户"
                }],
                "operationPaths": [{
                    "pathId": "path_account_lifecycle",
                    "name": "证券账户生命周期办理",
                    "surfaceRef": "surface_account_admin",
                    "workflowRef": "flow.account-lifecycle",
                    "dataViewRefs": ["view_account_list"],
                    "actionRefs": ["action_open_account"]
                }],
                "sourceRefs": refs
            });
        }
        _ => {}
    }
    candidate
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

fn remove_task_result_optional_validation_fields(fixture: &Fixture, request_ref: &str) {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let index = state::request_index::get_request_index_entry(fixture.root_str(), request_id)
        .expect("request index entry");
    let request_path = fixture.root.join(index.request_file);
    let mut root: Value =
        serde_json::from_str(&std::fs::read_to_string(&request_path).expect("read request file"))
            .expect("parse request file");
    let groups = root["requestReadPlan"]["groups"]
        .as_array_mut()
        .expect("read plan groups");
    for group in groups {
        let Some(fields) = group["fields"].as_array_mut() else {
            continue;
        };
        fields.retain(|field| {
            !matches!(
                field.as_str(),
                Some("task.conceptRefs" | "outputContract.blockedReasonOptions")
            )
        });
    }
    write_json_atomic(&request_path, &root).expect("write legacy-shaped request");
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

fn architecture_section_contract(fixture: &Fixture, request_ref: &str, section: &str) -> Value {
    private_architecture_section_outputs(fixture, request_ref)
        .into_iter()
        .find(|output| output["section"] == json!(section))
        .unwrap_or_else(|| panic!("{section} section contract"))
}

fn private_architecture_section_outputs(fixture: &Fixture, request_ref: &str) -> Vec<Value> {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let relative =
        state::request_manifest::request_storage_ref(&fixture.root, request_id, "sectionOutputs")
            .expect("read private sectionOutputs ref")
            .expect("private sectionOutputs ref");
    let path = fixture.root.join(relative);
    serde_json::from_str(&std::fs::read_to_string(path).expect("read private sectionOutputs"))
        .expect("parse private sectionOutputs")
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

fn greenfield_technical_baseline_candidate_json() -> Value {
    json!({
        "status": "confirmed",
        "source": "agent_recommended_for_greenfield",
        "projectKind": "greenfield",
        "scope": "project",
        "stack": {
            "tracks": {
                "web": {
                    "status": "selected",
                    "selection": "React + Vite + TypeScript",
                    "source": "user_confirmed",
                    "rationale": "Matches the confirmed staff-facing web workflow."
                },
                "app": {
                    "status": "not_applicable",
                    "selection": "No native/mobile app in this phase",
                    "source": "requirement_scope",
                    "rationale": "The confirmed phase only needs a staff web surface."
                },
                "backend": {
                    "status": "selected",
                    "selection": "Java + Spring Boot",
                    "source": "user_confirmed",
                    "rationale": "Supports the required account workflow APIs."
                },
                "persistence": {
                    "status": "selected",
                    "selection": "SQLite",
                    "source": "user_confirmed",
                    "rationale": "Enough for local phase verification."
                },
                "dataAccess": {
                    "status": "selected",
                    "selection": "Spring Data JPA",
                    "source": "user_confirmed",
                    "rationale": "Aligns with Spring Boot persistence."
                },
                "externalServices": {
                    "status": "not_needed",
                    "selection": "No external services in this phase",
                    "source": "requirement_scope",
                    "rationale": "The phase can run locally without third-party integration."
                }
            }
        },
        "constraints": [],
        "evidence": [{
            "path": Value::Null,
            "reason": "Greenfield stack was selected from the confirmed phase scope."
        }],
        "approval": {
            "type": "user_confirmed",
            "confirmedAt": "2026-06-24T10:30:00+08:00",
            "reason": "User confirmed the final technology baseline."
        },
        "confidence": "high",
        "requiresUserConfirmation": false,
        "reasoningSummary": [
            "The selected stack supports the confirmed greenfield phase without adding extra surfaces."
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

fn candidate_with_next_phase_preview() -> Value {
    let mut candidate = valid_candidate_json();
    candidate["scope"]["deferred"] = json!([{
        "id": "deferred_1",
        "label": "资金账户基础能力",
        "items": ["资金账户开户", "密码管理", "存款与取款", "证券账户与资金账户关联"],
        "reason": "资金账户依赖已完成的证券账户主数据，适合作为下一阶段。",
        "source": "user_confirmed"
    }]);
    candidate["roadmap"]["required"] = json!(true);
    candidate["phasePlan"]["nextPhasePreview"] = json!({
        "kind": "candidate",
        "suggestedPhaseId": "phase-next",
        "title": "资金账户基础能力",
        "goal": "在证券账户基础上实现资金账户开户、密码、存取款与账户关联。",
        "scopePreview": ["资金账户开户", "密码管理", "存款与取款", "账户关联"],
        "reason": "资金账户是交易客户端和撮合前的下一层依赖。"
    });
    candidate
}

fn phase2_candidate_json() -> Value {
    let mut candidate = valid_candidate_json();
    candidate["requestSummary"]["title"] = json!("资金账户基础能力");
    candidate["requestSummary"]["oneLine"] = json!("实现资金账户开户、密码、存取款与账户关联");
    candidate["requestSummary"]["businessGoal"] = json!("承接证券账户闭环，完成资金账户基础能力");
    candidate["scope"]["included"][0]["label"] = json!("资金账户基础能力");
    candidate["scope"]["included"][0]["items"] = json!([
        "资金账户开户",
        "密码管理",
        "存款与取款",
        "证券账户与资金账户关联"
    ]);
    candidate["scope"]["included"][0]["reason"] =
        json!("phase2 承接已完成的证券账户底座，补齐交易前置的资金账户能力。");
    candidate["roadmap"]["currentPhaseId"] = json!("phase-2");
    candidate["roadmap"]["phases"][0]["phaseId"] = json!("phase-2");
    candidate["roadmap"]["phases"][0]["title"] = json!("资金账户基础能力");
    candidate["roadmap"]["phases"][0]["name"] = json!("资金账户基础能力");
    candidate["roadmap"]["phases"][0]["goal"] = json!("实现资金账户开户、密码、存取款与账户关联。");
    candidate["phasePlan"]["current"]["phaseId"] = json!("phase-2");
    candidate["phasePlan"]["current"]["title"] = json!("资金账户基础能力");
    candidate["phasePlan"]["current"]["goal"] = json!("实现资金账户开户、密码、存取款与账户关联。");
    candidate["phasePlan"]["nextPhasePreview"] = json!({
        "kind": "none",
        "reason": "phase2 之后的范围由后续阶段确认。"
    });
    candidate["acceptance"][0]["statement"] =
        json!("工作人员可以完成资金账户开户、密码管理、存取款和证券账户关联。");
    candidate
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

fn candidate_with_planning_details_json() -> Value {
    let mut candidate = valid_candidate_with_frontend_json();
    candidate["scope"]["deferred"] = json!([{
        "id": "deferred_1",
        "label": "资金账户",
        "items": ["资金账户开户留到后续阶段"],
        "reason": "当前阶段只做证券账户闭环",
        "source": "user_confirmed"
    }]);
    candidate["scope"]["excluded"] = json!([{
        "id": "excluded_1",
        "label": "中央撮合",
        "items": ["撮合成交不在当前阶段实现"],
        "reason": "避免把交易核心提前塞入账户阶段",
        "source": "user_confirmed"
    }]);
    candidate["phasePlan"]["nextPhasePreview"] = json!({
        "kind": "candidate",
        "suggestedPhaseId": "phase-next",
        "title": "资金账户基础能力",
        "goal": "在证券账户基础上实现资金账户开户、密码、存取款与账户关联。",
        "scopePreview": ["资金账户开户", "密码管理", "存款与取款", "账户关联"],
        "reason": "资金账户是交易客户端和撮合前的下一层依赖。"
    });
    candidate["acceptance"][0]["capabilityRefs"] = json!(["cap_account_opening"]);
    candidate["scope"]["assumptions"] = json!([{
        "id": "assumption_1",
        "text": "开户前工作人员已完成必要身份材料核验。",
        "requiresConfirmation": false
    }]);
    candidate["domainModel"] = json!({
        "actors": [{
            "id": "actor_staff",
            "name": "工作人员",
            "description": "办理证券账户开户、挂失补办和销户。"
        }],
        "capabilityGroups": [{
            "id": "cap_account_opening",
            "name": "证券账户开户",
            "description": "个人与法人证券账户开户能力。"
        }],
        "businessFlows": [{
            "id": "flow_account_opening",
            "name": "证券账户开户流程",
            "actors": ["actor_staff"],
            "capabilityRefs": ["cap_account_opening"],
            "summary": "工作人员录入开户材料，系统校验资格并创建证券账户后给出中文成功反馈。"
        }]
    });
    candidate["conceptGrounding"]["phaseConceptGrounding"] = json!({
        "mode": "concepts_present",
        "reason": "证券账户与资金账户容易混淆，需要在规划合同中保留语义边界。",
        "concepts": [{
            "conceptId": "concept_security_account",
            "term": "证券账户",
            "normalizedName": "证券账户",
            "explanation": "证券账户用于登记证券持有与交易资格，不等同于资金账户。",
            "mustNotMisinterpretAs": ["资金账户"],
            "phaseRelevance": "current",
            "priority": "must_understand",
            "attentionRank": 1,
            "riskFactors": ["scope_confusion_risk"],
            "scopeRefs": ["scope_1"],
            "acceptanceRefs": ["acc_1"],
            "humanReadableReason": "当前阶段只实现证券账户闭环。"
        }]
    });
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
