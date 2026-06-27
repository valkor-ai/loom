use std::path::Path;

use contracts::{
    ArchitectureSectionCandidateAgentWritable, ArchitectureSectionGroup,
    PlanningGenerationContract, TechnicalBaselineContract,
};
use delivery_core::{
    ArtifactKind, LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult, RouteAction,
    RouteActionKind, TransitionStore,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{to_project_relative, DeliveryPhaseLocator},
};

use crate::{
    paths::{architecture_candidate_file, architecture_request_file, section_name},
    write_artifact_result, SectionOutput,
};

const SECTION_ORDER: [ArchitectureSectionGroup; 6] = [
    ArchitectureSectionGroup::Foundation,
    ArchitectureSectionGroup::DomainContract,
    ArchitectureSectionGroup::Behavior,
    ArchitectureSectionGroup::FrontendExperience,
    ArchitectureSectionGroup::RuntimeDelivery,
    ArchitectureSectionGroup::Coverage,
];

pub fn materialize_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match materialize_request_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: project_root.to_string(),
            error: LoomMcpFailure {
                code: "ARCHITECTURE_REQUEST_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("architecture".to_string()),
                route_action: Some("architecture_artifact_contract".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn materialize_request_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let store = FileTransitionStore;
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let phase = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(format!(
                "phase {} does not exist in delivery {}",
                phase_id, delivery_id
            ))
        })?;
    if let Some(existing_request_ref) = phase.latest_refs.get("architectureRequestRef").cloned() {
        let inspected = state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        });
        if inspected
            .as_ref()
            .map(|request| request.request_kind == "architecture_sections_generation")
            .unwrap_or(false)
        {
            return write_artifact_result(
                project_root,
                &existing_request_ref,
                ArtifactKind::ArchitectureSectionCandidate,
            );
        }
    }

    let planning_contract_ref = phase
        .latest_refs
        .get("planningContract")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest planningContract ref is missing".to_string(),
            )
        })?;
    let planning_contract = read_planning_contract(root, &planning_contract_ref)?;
    let technical_baseline_ref = phase
        .latest_refs
        .get("technicalBaseline")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest technicalBaseline ref is missing".to_string(),
            )
        })?;
    let technical_baseline = read_technical_baseline(root, &technical_baseline_ref)?;
    let brainstorm_contract_ref = phase
        .latest_refs
        .get("brainstormContract")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest brainstormContract ref is missing".to_string(),
            )
        })?;

    let request_id = format!("arch_{}", state::store::now_millis());
    let has_previous_runtime_delivery = phase.latest_refs.contains_key("runtimeDelivery");
    let frontend_experience_source = build_frontend_experience_source(phase);
    let section_outputs = build_section_outputs(
        root,
        &request_id,
        delivery_id,
        phase_id,
        has_previous_runtime_delivery,
        &frontend_experience_source,
        &planning_contract,
    )?;
    let current_output = section_outputs.first().cloned().ok_or_else(|| {
        state::store::StateError::StateCorrupted(
            "architecture section outputs are empty".to_string(),
        )
    })?;
    let request_root = build_request_root(
        &request_id,
        delivery_id,
        phase_id,
        &planning_contract_ref,
        &planning_contract,
        &technical_baseline_ref,
        &technical_baseline,
        &brainstorm_contract_ref,
        phase,
        has_previous_runtime_delivery,
        &frontend_experience_source,
        &current_output,
        &section_outputs,
    )?;
    let request_file = to_project_relative(
        root,
        &architecture_request_file(root, &locator, &request_id),
    )?;
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "architecture_sections_generation".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    if let Some(active_phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        active_phase
            .latest_refs
            .insert("architectureRequestId".to_string(), request_id);
        active_phase.latest_refs.insert(
            "architectureRequestRef".to_string(),
            stored.request_ref.clone(),
        );
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    write_artifact_result(
        project_root,
        &stored.request_ref,
        ArtifactKind::ArchitectureSectionCandidate,
    )
}

fn build_request_root(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    planning_contract_ref: &str,
    planning_contract: &PlanningGenerationContract,
    technical_baseline_ref: &str,
    _technical_baseline: &TechnicalBaselineContract,
    brainstorm_contract_ref: &str,
    phase: &delivery_core::DeliveryPhaseState,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    current_output: &SectionOutput,
    section_outputs: &[SectionOutput],
) -> Result<Value, state::store::StateError> {
    let candidate_schema =
        serde_json::to_value(schema_for!(ArchitectureSectionCandidateAgentWritable))
            .unwrap_or_else(|_| json!({ "type": "object" }));
    let source_refs = build_source_refs(
        planning_contract_ref,
        technical_baseline_ref,
        brainstorm_contract_ref,
        phase,
    );
    let allowed_refs = build_allowed_refs(planning_contract);
    let context_projection = build_context_projection(planning_contract);
    Ok(json!({
        "schemaVersion": "1.0",
        "requestType": "architecture_sections_generation",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::ArchitectureSectionCandidate,
        "sourceRefs": source_refs,
        "contextProjection": context_projection,
        "frontendExperienceSource": frontend_experience_source,
        "allowedRefs": allowed_refs,
        "sectionState": {
            "order": SECTION_ORDER,
            "currentSection": current_output.section,
            "completedSections": [],
        },
        "sectionOutputs": section_outputs.iter().map(section_output_to_value).collect::<Vec<_>>(),
        "currentSectionContract": section_output_to_value(current_output),
        "enumRefs": {
            "section": SECTION_ORDER,
            "status": ["ready", "blocked"],
            "coverageStatus": ["covered", "partial", "not_applicable", "deferred", "uncovered"],
            "acceptancePriority": ["must", "should", "could"]
        },
        "rules": {
            "onlyCurrentPhase": true,
            "followTechnicalBaseline": true,
            "doNotImplementDeferredScope": true,
            "doNotWriteFinalAacJson": true,
            "requirementDetailTransfer": "Use contextProjection.requirementDetailTransfer as the current phase detail authority.",
            "frontendExperienceAuthority": "When confirmed/current frontend refs exist, frontend_experience must consume them and must not downgrade the confirmed target.",
            "runtimeDeliveryAuthority": runtime_delivery_authority(has_previous_runtime_delivery)
        },
        "outputContract": {
            "artifactKind": ArtifactKind::ArchitectureSectionCandidate,
            "writeMode": delivery_core::WriteMode::ArchitectureSection,
            "submitTool": "loom.architectureSectionSubmitFile",
            "writeTargets": [{
                "targetId": section_name(current_output.section),
                "path": current_output.candidate_file,
                "required": true,
                "description": format!("Write the {} Architecture section candidate JSON.", section_name(current_output.section))
            }],
            "schemaShape": candidate_schema,
            "schemaProjection": {
                "requiredTopLevelFields": [
                    "schemaVersion",
                    "requestId",
                    "deliveryId",
                    "phaseId",
                    "section",
                    "status",
                    "content",
                    "createdAt"
                ],
                "requiredContentKeys": required_content_keys(current_output.section)
            }
        },
        "postSubmit": {
            "nextAction": RouteAction {
                kind: RouteActionKind::ArchitectureArtifactContract,
                source: "architecture_section_submit".to_string(),
                reason: "architecture_section_ready".to_string(),
                prompt: None,
                accepted_responses: vec![],
                request_ref: None,
                details: None,
                target_phase_id: None
            }
        },
        "requestReadPlan": {
            "groups": architecture_read_groups(current_output.section, false)
        }
    }))
}

pub(crate) fn architecture_read_groups(
    section: ArchitectureSectionGroup,
    include_repair_context: bool,
) -> Value {
    let mut core_fields = vec![
        "sourceRefs.planningContractRef",
        "sourceRefs.technicalBaselineRef",
        "sourceRefs.brainstormContractRef",
        "sourceRefs.repositoryContextRef",
        "sourceRefs.deliveryConceptGlossaryRef",
        "sourceRefs.phaseConceptGroundingRef",
        "sourceRefs.confirmedFrontendExperienceRef",
        "sourceRefs.currentFrontendExperienceRef",
        "sourceRefs.previousRuntimeDeliveryRef",
        "contextProjection.phaseScope.phaseName",
        "contextProjection.phaseScope.phaseGoal",
        "contextProjection.phaseScope.included",
        "contextProjection.phaseScope.deferred",
        "contextProjection.phaseScope.excluded",
        "contextProjection.phaseScope.acceptanceCandidates",
        "contextProjection.phaseId",
        "contextProjection.planningContractId",
        "contextProjection.technicalBaseline.technicalBaselineId",
        "contextProjection.technicalBaseline.status",
        "contextProjection.technicalBaseline.scope",
        "contextProjection.technicalBaseline.summary",
        "contextProjection.technicalBaseline.mustFollow",
        "contextProjection.requirementDetailTransfer.requirementDetails",
        "contextProjection.requirementDetailTransfer.acceptanceDetails",
        "contextProjection.requirementDetailTransfer.businessFlows",
        "allowedRefs.scopeRefs",
        "allowedRefs.acceptanceRefs",
        "allowedRefs.deferredScopeRefs",
        "allowedRefs.excludedScopeRefs",
        "allowedRefs.requirementDetailIds",
    ];
    if include_repair_context {
        core_fields.insert(9, "repairContext.sourceArchitectureRequestRef");
        core_fields.insert(10, "repairContext.sourceRef");
    }
    let mut groups = vec![
        json!({
            "groupId": "architecture_core_context",
            "required": true,
            "purpose": if include_repair_context {
                "Read the current-phase planning authority, repair context, and allowed refs before generating the replacement Architecture section."
            } else {
                "Read the current-phase planning authority and allowed refs before generating the current Architecture section."
            },
            "whenToRead": if include_repair_context {
                "Read before drafting any replacement Architecture section candidate."
            } else {
                "Read before drafting any Architecture section candidate."
            },
            "fields": core_fields
        }),
        json!({
            "groupId": "architecture_section_contract",
            "required": true,
            "purpose": if include_repair_context {
                "Read the current section contract, schema projection, and write target before writing the replacement section candidate."
            } else {
                "Read the current section contract, schema projection, and write target before writing the section candidate."
            },
            "whenToRead": if include_repair_context {
                "Read immediately before writing the current replacement Architecture section candidate."
            } else {
                "Read immediately before writing the current Architecture section candidate."
            },
            "fields": [
                "sectionState.currentSection",
                "currentSectionContract.section",
                "currentSectionContract.candidateFile",
                "currentSectionContract.schemaRef",
                "currentSectionContract.resultTemplate",
                "currentSectionContract.enumRefs",
                "currentSectionContract.generationRules",
                "outputContract.writeTargets",
                "outputContract.submitTool",
                "outputContract.schemaProjection",
                "enumRefs.section",
                "enumRefs.status",
                "enumRefs.coverageStatus",
                "enumRefs.acceptancePriority"
            ]
        }),
    ];
    if matches!(section, ArchitectureSectionGroup::FrontendExperience) {
        groups.push(json!({
            "groupId": "architecture_frontend_context",
            "required": true,
            "purpose": "Read the frontend authority refs for frontend_experience.",
            "whenToRead": "Read when sectionState.currentSection is frontend_experience.",
            "fields": [
                "frontendExperienceSource.confirmedFrontendExperienceRef",
                "frontendExperienceSource.currentFrontendExperienceRef",
                "frontendExperienceSource.repositoryContextRef",
                "frontendExperienceSource.authorityRule",
                "contextProjection.requirementDetailTransfer.frontendExperienceDetails",
                "contextProjection.requirementDetailTransfer.userFacingLanguage"
            ]
        }));
    }
    Value::Array(groups)
}

fn build_source_refs(
    planning_contract_ref: &str,
    technical_baseline_ref: &str,
    brainstorm_contract_ref: &str,
    phase: &delivery_core::DeliveryPhaseState,
) -> Value {
    let mut value = json!({
        "planningContractRef": planning_contract_ref,
        "technicalBaselineRef": technical_baseline_ref,
        "brainstormContractRef": brainstorm_contract_ref,
    });
    if let Some(repository_context_ref) = phase.latest_refs.get("latestRepositoryContext") {
        value["repositoryContextRef"] = json!(repository_context_ref);
    }
    if let Some(delivery_glossary_ref) = phase.latest_refs.get("deliveryConceptGlossary") {
        value["deliveryConceptGlossaryRef"] = json!(delivery_glossary_ref);
    }
    if let Some(phase_grounding_ref) = phase.latest_refs.get("phaseConceptGrounding") {
        value["phaseConceptGroundingRef"] = json!(phase_grounding_ref);
    }
    if let Some(confirmed_frontend_ref) = phase.latest_refs.get("confirmedFrontendExperience") {
        value["confirmedFrontendExperienceRef"] = json!(confirmed_frontend_ref);
    }
    if let Some(current_frontend_ref) = phase.latest_refs.get("currentFrontendExperience") {
        value["currentFrontendExperienceRef"] = json!(current_frontend_ref);
    }
    if let Some(previous_runtime_ref) = phase.latest_refs.get("runtimeDelivery") {
        value["previousRuntimeDeliveryRef"] = json!(previous_runtime_ref);
    }
    value
}

fn build_frontend_experience_source(phase: &delivery_core::DeliveryPhaseState) -> Value {
    json!({
        "confirmedFrontendExperienceRef": phase.latest_refs.get("confirmedFrontendExperience"),
        "currentFrontendExperienceRef": phase.latest_refs.get("currentFrontendExperience"),
        "repositoryContextRef": phase.latest_refs.get("latestRepositoryContext"),
        "authorityRule": "Use confirmed/current frontend refs as the frontend_experience authority. RepositoryContext and TechnicalBaseline are implementation facts only."
    })
}

fn frontend_source_refs_template(frontend_experience_source: &Value) -> Value {
    let authority_ref = frontend_experience_source
        .get("confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            frontend_experience_source
                .get("currentFrontendExperienceRef")
                .and_then(Value::as_str)
        })
        .unwrap_or_default();
    json!({
        "brainstormFrontendExperienceRef": authority_ref
    })
}

fn build_allowed_refs(planning_contract: &PlanningGenerationContract) -> Value {
    json!({
        "scopeRefs": planning_contract
            .phase_scope
            .included
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        "acceptanceRefs": planning_contract
            .phase_scope
            .acceptance_candidates
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        "deferredScopeRefs": planning_contract
            .phase_scope
            .deferred
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        "excludedScopeRefs": planning_contract
            .phase_scope
            .excluded
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        "requirementDetailIds": planning_contract
            .requirement_details
            .items
            .iter()
            .map(|item| item.detail_id.clone())
            .collect::<Vec<_>>()
    })
}

fn build_context_projection(planning_contract: &PlanningGenerationContract) -> Value {
    json!({
        "phaseId": planning_contract.source.phase_id,
        "planningContractId": planning_contract.planning_contract_id,
        "phaseScope": planning_contract.phase_scope,
        "technicalBaseline": planning_contract.technical_baseline,
        "requirementDetailTransfer": {
            "requirementDetails": planning_contract.requirement_details,
            "acceptanceDetails": planning_contract.phase_scope.acceptance_candidates,
            "frontendExperienceDetails": planning_contract.planning_inputs.frontend_experience,
            "userFacingLanguage": planning_contract.planning_inputs.user_facing_language,
            "businessFlows": planning_contract.planning_inputs.business_flows
        }
    })
}

fn build_section_outputs(
    project_root: &Path,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    planning_contract: &PlanningGenerationContract,
) -> Result<Vec<SectionOutput>, state::store::StateError> {
    SECTION_ORDER
        .iter()
        .copied()
        .map(|section| {
            Ok(SectionOutput {
                section,
                candidate_file: to_project_relative(
                    project_root,
                    &architecture_candidate_file(project_root, request_id, section),
                )?,
                schema_ref: format!("architecture-section-{}-v1", section_name(section)),
                schema_shape: section_schema_shape(section, has_previous_runtime_delivery),
                result_template: section_result_template(
                    request_id,
                    delivery_id,
                    phase_id,
                    section,
                    has_previous_runtime_delivery,
                    frontend_experience_source,
                    planning_contract,
                ),
                enum_refs: section_enum_refs(section, has_previous_runtime_delivery),
                generation_rules: section_generation_rules(section, has_previous_runtime_delivery),
            })
        })
        .collect()
}

fn section_output_to_value(output: &SectionOutput) -> Value {
    json!({
        "section": output.section,
        "candidateFile": output.candidate_file,
        "schemaRef": output.schema_ref,
        "schemaShape": output.schema_shape,
        "resultTemplate": output.result_template,
        "enumRefs": output.enum_refs,
        "generationRules": output.generation_rules
    })
}

pub fn section_order() -> &'static [ArchitectureSectionGroup] {
    &SECTION_ORDER
}

pub fn required_content_keys(section: ArchitectureSectionGroup) -> Vec<&'static str> {
    match section {
        ArchitectureSectionGroup::Foundation => vec!["source", "engineeringBoundary", "modules"],
        ArchitectureSectionGroup::DomainContract => vec!["dataModel", "interfaces"],
        ArchitectureSectionGroup::Behavior => vec!["userFlows", "stateMachines"],
        ArchitectureSectionGroup::FrontendExperience => vec!["frontendExperience"],
        ArchitectureSectionGroup::RuntimeDelivery => vec!["runtimeDelivery"],
        ArchitectureSectionGroup::Coverage => {
            vec![
                "acceptanceMatrix",
                "detailCoverage",
                "risksAndDecisions",
                "handoff",
            ]
        }
    }
}

fn section_schema_shape(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
) -> Value {
    json!({
        "schemaVersion": "1.0",
        "requestId": "string",
        "deliveryId": "string",
        "phaseId": "string",
        "section": section,
        "status": "ready | blocked",
        "content": section_content_shape(section, has_previous_runtime_delivery),
        "blockedReasons": [{
            "code": "string",
            "message": "string",
            "nextNode": "string"
        }],
        "createdAt": "string"
    })
}

fn section_content_shape(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
) -> Value {
    match section {
        ArchitectureSectionGroup::Foundation => json!({
            "source": {
                "planningGenerationContractId": "string",
                "technicalBaselineId": "string"
            },
            "engineeringBoundary": {
                "summary": "string",
                "applications": ["object"],
                "modules": ["object"]
            },
            "modules": ["object"]
        }),
        ArchitectureSectionGroup::DomainContract => json!({
            "dataModel": {
                "entities": ["object"],
                "relationships": ["object"],
                "constraints": ["object"]
            },
            "interfaces": ["object"]
        }),
        ArchitectureSectionGroup::Behavior => json!({
            "userFlows": ["object"],
            "stateMachines": ["object"]
        }),
        ArchitectureSectionGroup::FrontendExperience => json!({
            "frontendExperience": {
                "required": "boolean",
                "experienceLevel": "string",
                "surfaces": ["object"],
                "dataViews": ["object"],
                "actions": ["object"],
                "operationPaths": ["object"],
                "sourceRefs": {
                    "brainstormFrontendExperienceRef": "string"
                }
            }
        }),
        ArchitectureSectionGroup::RuntimeDelivery => {
            runtime_delivery_content_shape(has_previous_runtime_delivery)
        }
        ArchitectureSectionGroup::Coverage => json!({
            "acceptanceMatrix": [{
                "acceptanceId": "string",
                "priority": "must | should | could",
                "statement": "string",
                "coverageStatus": "covered | partial | not_applicable | deferred | uncovered",
                "reason": "string",
                "coverage": [{
                    "type": "string",
                    "refs": ["string"],
                    "description": "string"
                }],
                "verificationHints": [{
                    "kind": "string",
                    "description": "string"
                }]
            }],
            "detailCoverage": [{
                "detailId": "string",
                "coverageStatus": "covered | partial | not_applicable | deferred | uncovered",
                "artifactRefs": {
                    "modules": ["string"],
                    "entities": ["string"],
                    "fields": ["string"],
                    "constraints": ["string"],
                    "interfaces": ["string"],
                    "userFlows": ["string"],
                    "stateMachines": ["string"],
                    "frontendDataViews": ["string"],
                    "frontendActions": ["string"],
                    "frontendOperationPaths": ["string"],
                    "acceptanceMatrix": ["string"]
                },
                "reason": "string"
            }],
            "risksAndDecisions": "object",
            "handoff": {
                "readyForTaskPlan": "boolean",
                "blockingReasons": ["string"],
                "nextNode": "task_plan | architecture_artifact_repair | needs_user_decision | blocked"
            }
        }),
    }
}

fn runtime_delivery_content_shape(has_previous_runtime_delivery: bool) -> Value {
    let mut basis = serde_json::Map::new();
    basis.insert(
        "technicalBaselineRef".to_string(),
        Value::String("string".to_string()),
    );
    if has_previous_runtime_delivery {
        basis.insert(
            "previousRuntimeDeliveryRef".to_string(),
            Value::String("string".to_string()),
        );
    }
    json!({
        "runtimeDelivery": {
            "status": runtime_delivery_status_values(has_previous_runtime_delivery).join(" | "),
            "basis": Value::Object(basis),
            "build": "object",
            "start": "object",
            "runtimeSurfaces": ["object"],
            "taskPlanningGuidance": "object"
        }
    })
}

fn section_result_template(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    planning_contract: &PlanningGenerationContract,
) -> Value {
    json!({
        "schemaVersion": "1.0",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "section": section,
        "status": "ready",
        "content": section_content_template(
            section,
            has_previous_runtime_delivery,
            frontend_experience_source,
            planning_contract
        ),
        "blockedReasons": [],
        "createdAt": "ISO-8601 datetime"
    })
}

fn section_content_template(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    planning_contract: &PlanningGenerationContract,
) -> Value {
    match section {
        ArchitectureSectionGroup::Foundation => json!({
            "source": {
                "planningGenerationContractId": "",
                "technicalBaselineId": ""
            },
            "engineeringBoundary": {
                "summary": "",
                "applications": [{
                    "applicationId": "app_1",
                    "name": "",
                    "kind": "",
                    "rootPath": "."
                }],
                "modules": [{
                    "moduleId": "module_1",
                    "name": "",
                    "scopeRefs": [],
                    "acceptanceRefs": [],
                    "summary": ""
                }]
            },
            "modules": [{
                "moduleId": "module_1",
                "name": "",
                "responsibility": "",
                "scopeRefs": [],
                "acceptanceRefs": []
            }]
        }),
        ArchitectureSectionGroup::DomainContract => json!({
            "dataModel": {
                "entities": [{
                    "entityId": "entity_1",
                    "name": "",
                    "fields": [],
                    "constraints": [],
                    "scopeRefs": [],
                    "acceptanceRefs": []
                }],
                "relationships": [],
                "constraints": []
            },
            "interfaces": [{
                "interfaceId": "interface_1",
                "name": "",
                "kind": "",
                "operations": [],
                "scopeRefs": [],
                "acceptanceRefs": []
            }]
        }),
        ArchitectureSectionGroup::Behavior => json!({
            "userFlows": [{
                "flowId": "flow_1",
                "name": "",
                "steps": [],
                "scopeRefs": [],
                "acceptanceRefs": []
            }],
            "stateMachines": [{
                "machineId": "state_machine_1",
                "name": "",
                "states": [],
                "transitions": [],
                "scopeRefs": [],
                "acceptanceRefs": []
            }]
        }),
        ArchitectureSectionGroup::FrontendExperience => json!({
            "frontendExperience": {
                "required": true,
                "experienceLevel": "usable_internal_product",
                "surfaces": [{
                    "surfaceId": "surface_1",
                    "name": "",
                    "purpose": "",
                    "audienceRefs": []
                }],
                "dataViews": [{
                    "viewId": "view_1",
                    "name": "",
                    "fields": [],
                    "sourceRefs": []
                }],
                "actions": [{
                    "actionId": "action_1",
                    "label": "",
                    "entryPoint": "",
                    "sourceRefs": []
                }],
                "operationPaths": [{
                    "pathId": "path_1",
                    "name": "",
                    "surfaceRef": "surface_1",
                    "dataViewRefs": ["view_1"],
                    "actionRefs": ["action_1"],
                    "sourceRefs": []
                }],
                "sourceRefs": frontend_source_refs_template(frontend_experience_source)
            }
        }),
        ArchitectureSectionGroup::RuntimeDelivery => {
            runtime_delivery_content_template(has_previous_runtime_delivery)
        }
        ArchitectureSectionGroup::Coverage => coverage_content_template(planning_contract),
    }
}

fn coverage_content_template(planning_contract: &PlanningGenerationContract) -> Value {
    let acceptance_matrix = planning_contract
        .phase_scope
        .acceptance_candidates
        .iter()
        .map(|acceptance| {
            json!({
                "acceptanceId": acceptance.id,
                "priority": acceptance.priority,
                "statement": acceptance.statement,
                "coverageStatus": "covered",
                "reason": "",
                "coverage": [],
                "verificationHints": []
            })
        })
        .collect::<Vec<_>>();
    let detail_coverage = planning_contract
        .requirement_details
        .items
        .iter()
        .map(|detail| {
            json!({
                "detailId": detail.detail_id,
                "coverageStatus": "covered",
                "artifactRefs": detail_coverage_artifact_refs_template(),
                "reason": ""
            })
        })
        .collect::<Vec<_>>();
    json!({
        "acceptanceMatrix": acceptance_matrix,
        "detailCoverage": detail_coverage,
        "risksAndDecisions": {
            "decisions": [],
            "risks": []
        },
        "handoff": {
            "readyForTaskPlan": true,
            "blockingReasons": [],
            "nextNode": "task_plan"
        }
    })
}

fn runtime_delivery_content_template(has_previous_runtime_delivery: bool) -> Value {
    let mut basis = serde_json::Map::new();
    basis.insert(
        "technicalBaselineRef".to_string(),
        Value::String("".to_string()),
    );
    if has_previous_runtime_delivery {
        basis.insert(
            "previousRuntimeDeliveryRef".to_string(),
            Value::String("".to_string()),
        );
    }
    json!({
        "runtimeDelivery": {
            "status": "modified",
            "basis": Value::Object(basis),
            "build": {
                "command": "",
                "output": ""
            },
            "start": {
                "command": "",
                "port": null
            },
            "runtimeSurfaces": [{
                "surfaceId": "runtime_surface_1",
                "kind": "",
                "urlPath": "",
                "purpose": ""
            }],
            "taskPlanningGuidance": {
                "runtimeAffectingTasks": [],
                "closureRequired": true
            }
        }
    })
}

fn detail_coverage_artifact_refs_template() -> Value {
    json!({
        "modules": [],
        "entities": [],
        "fields": [],
        "constraints": [],
        "interfaces": [],
        "userFlows": [],
        "stateMachines": [],
        "frontendDataViews": [],
        "frontendActions": [],
        "frontendOperationPaths": [],
        "acceptanceMatrix": []
    })
}

fn runtime_delivery_status_values(has_previous_runtime_delivery: bool) -> Vec<&'static str> {
    if has_previous_runtime_delivery {
        vec!["modified", "unchanged", "not_applicable"]
    } else {
        vec!["modified", "not_applicable"]
    }
}

fn runtime_delivery_authority(has_previous_runtime_delivery: bool) -> &'static str {
    if has_previous_runtime_delivery {
        "A previous runtime delivery exists in sourceRefs.previousRuntimeDeliveryRef. Use runtimeDelivery.status=unchanged only when copying that ref exactly; otherwise use modified or not_applicable."
    } else {
        "No previous runtime delivery exists for this phase. runtimeDelivery.status must be modified or not_applicable; do not use unchanged and do not write basis.previousRuntimeDeliveryRef."
    }
}

fn section_enum_refs(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
) -> Value {
    match section {
        ArchitectureSectionGroup::Coverage => json!({
            "coverageStatus": ["covered", "partial", "not_applicable", "deferred", "uncovered"],
            "acceptancePriority": ["must", "should", "could"]
        }),
        ArchitectureSectionGroup::RuntimeDelivery => json!({
            "runtimeDeliveryStatus": runtime_delivery_status_values(has_previous_runtime_delivery)
        }),
        _ => json!({}),
    }
}

fn section_generation_rules(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
) -> Vec<String> {
    match section {
        ArchitectureSectionGroup::Foundation => vec![
            "Carry the planning and technical baseline identity into content.source.".to_string(),
            "Define the engineering boundary and current-phase modules only.".to_string(),
            "Follow the existing project and technical baseline shape before introducing a new module, adapter, or abstraction.".to_string(),
            "Add a seam only when it supports current-phase behavior, verification, or meaningful isolation; avoid pass-through wrappers.".to_string(),
            "Use allowedRefs.scopeRefs and allowedRefs.acceptanceRefs exactly; do not invent ids."
                .to_string(),
        ],
        ArchitectureSectionGroup::DomainContract => vec![
            "Represent current-phase business objects, key fields, relationships, constraints, and interfaces."
                .to_string(),
            "Use contextProjection.requirementDetailTransfer as the current phase detail authority."
                .to_string(),
            "Preserve confirmed business terminology; record conflicts instead of casually renaming domain concepts."
                .to_string(),
        ],
        ArchitectureSectionGroup::Behavior => vec![
            "Represent current-phase user flows, state machines, blockers, and success outcomes."
                .to_string(),
            "Do not reference future or deferred scope as if it were current-phase behavior."
                .to_string(),
        ],
        ArchitectureSectionGroup::FrontendExperience => vec![
            "Read frontendExperienceSource before writing this section.".to_string(),
            "Preserve the confirmed/current frontend target instead of rediscovering it.".to_string(),
            "Use RepositoryContext and TechnicalBaseline only as implementation facts.".to_string(),
        ],
        ArchitectureSectionGroup::RuntimeDelivery => vec![
            "Represent current-phase runtime delivery readiness, not a generic deployment wishlist."
                .to_string(),
            runtime_delivery_authority(has_previous_runtime_delivery).to_string(),
        ],
        ArchitectureSectionGroup::Coverage => vec![
            "Map every current-phase acceptance candidate to AAC artifacts without inventing acceptance ids."
                .to_string(),
            "Use requirementDetailTransfer.requirementDetails.items as the canonical detail index."
                .to_string(),
            "detailCoverage must store detailId plus artifact refs; do not copy full detail summaries."
                .to_string(),
            "Record only architecture trade-offs that affect later implementation, verification, or repair routing."
                .to_string(),
        ],
    }
}

fn read_planning_contract(
    project_root: &Path,
    relative_ref: &str,
) -> Result<PlanningGenerationContract, state::store::StateError> {
    let absolute = state::paths::from_project_relative(project_root, relative_ref)?;
    state::store::read_json(&absolute)
}

fn read_technical_baseline(
    project_root: &Path,
    relative_ref: &str,
) -> Result<TechnicalBaselineContract, state::store::StateError> {
    let absolute = state::paths::from_project_relative(project_root, relative_ref)?;
    state::store::read_json(&absolute)
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}
