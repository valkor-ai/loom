use std::path::Path;

use contracts::{
    api_quality_enum_refs, api_quality_seed_read_fields, build_ui_quality_seed,
    ui_quality_enum_refs, ui_surface_decision_candidate_shape,
    ui_surface_decision_candidate_template, ui_surface_decision_enum_refs,
    ArchitectureSectionGroup, PlanningGenerationContract, TechnicalBaselineContract,
    COVERAGE_ARTIFACT_TYPES,
};
use delivery_core::{
    read_selectors_value_from_paths, ArtifactKind, LoomMcpActionResult, LoomMcpFailure,
    LoomMcpFailureResult, RouteAction, RouteActionKind, TransitionStore,
};
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
    // API applicability is produced after Foundation declares structured application interactions.
    let api_quality_seed = Value::Null;
    let section_outputs = build_section_outputs(
        root,
        &request_id,
        has_previous_runtime_delivery,
        &frontend_experience_source,
        &planning_contract,
        &api_quality_seed,
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
        &api_quality_seed,
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
    technical_baseline: &TechnicalBaselineContract,
    brainstorm_contract_ref: &str,
    phase: &delivery_core::DeliveryPhaseState,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    current_output: &SectionOutput,
    section_outputs: &[SectionOutput],
    api_quality_seed: &Value,
) -> Result<Value, state::store::StateError> {
    let source_refs = build_source_refs(
        planning_contract_ref,
        technical_baseline_ref,
        brainstorm_contract_ref,
        phase,
    );
    let allowed_refs = build_allowed_refs(planning_contract);
    let context_projection = build_context_projection(planning_contract);
    let ui_quality_seed = build_ui_quality_seed(
        planning_contract
            .planning_inputs
            .frontend_experience
            .as_ref(),
        Some(technical_baseline),
    );
    let architecture_quality_seed = build_architecture_quality_seed();
    let mut root = json!({
        "schemaVersion": "1.0",
        "requestType": "architecture_sections_generation",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::ArchitectureSectionCandidate,
        "sourceRefs": source_refs,
        "contextProjection": context_projection,
        "frontendExperienceSource": frontend_experience_source,
        "uiQualitySeed": ui_quality_seed,
        "architectureQualitySeed": architecture_quality_seed,
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
            "acceptancePriority": ["must", "should", "could"],
            "architectureQuality": architecture_quality_enum_refs(),
            "uiQuality": ui_quality_enum_refs(),
            "uiSurfaceDecision": ui_surface_decision_enum_refs()
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
            "schemaShape": current_output.schema_shape.clone(),
            "schemaProjection": {
                "requiredTopLevelFields": [
                    "status",
                    "content"
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
            "groups": architecture_read_groups(
                current_output.section,
                false,
                false,
                &source_refs,
                frontend_experience_source,
                api_quality_seed
            )
        }
    });
    if !api_quality_seed.is_null() {
        root["apiQualitySeed"] = api_quality_seed.clone();
        root["enumRefs"]["apiQuality"] = api_quality_enum_refs();
    }
    Ok(root)
}

pub(crate) fn architecture_read_groups(
    section: ArchitectureSectionGroup,
    include_repair_context: bool,
    include_repair_source_ref: bool,
    source_refs: &Value,
    frontend_experience_source: &Value,
    api_quality_seed: &Value,
) -> Value {
    let mut core_fields = vec![
        "sourceRefs.planningContractRef",
        "sourceRefs.technicalBaselineRef",
        "sourceRefs.brainstormContractRef",
    ];
    for ref_key in [
        "repositoryContextRef",
        "deliveryConceptGlossaryRef",
        "phaseConceptGroundingRef",
        "confirmedFrontendExperienceRef",
        "currentFrontendExperienceRef",
        "previousRuntimeDeliveryRef",
    ] {
        if has_non_null_key(source_refs, ref_key) {
            core_fields.push(match ref_key {
                "repositoryContextRef" => "sourceRefs.repositoryContextRef",
                "deliveryConceptGlossaryRef" => "sourceRefs.deliveryConceptGlossaryRef",
                "phaseConceptGroundingRef" => "sourceRefs.phaseConceptGroundingRef",
                "confirmedFrontendExperienceRef" => "sourceRefs.confirmedFrontendExperienceRef",
                "currentFrontendExperienceRef" => "sourceRefs.currentFrontendExperienceRef",
                "previousRuntimeDeliveryRef" => "sourceRefs.previousRuntimeDeliveryRef",
                _ => unreachable!(),
            });
        }
    }
    if include_repair_context {
        core_fields.push("repairContext.sourceArchitectureRequestRef");
        if include_repair_source_ref {
            core_fields.push("repairContext.sourceRef");
        }
    }
    core_fields.extend([
        "contextProjection.phaseScope.phaseName",
        "contextProjection.phaseScope.phaseGoal",
        "contextProjection.phaseScopeSummary.includedIds",
        "contextProjection.phaseScopeSummary.includedLabels",
        "contextProjection.phaseScopeSummary.includedItems",
        "contextProjection.phaseScopeSummary.deferredIds",
        "contextProjection.phaseScopeSummary.deferredLabels",
        "contextProjection.phaseScopeSummary.deferredItems",
        "contextProjection.phaseScopeSummary.excludedIds",
        "contextProjection.phaseScopeSummary.excludedLabels",
        "contextProjection.phaseScopeSummary.excludedItems",
        "contextProjection.phaseId",
        "contextProjection.planningContractId",
        "contextProjection.technicalBaseline.technicalBaselineId",
        "contextProjection.technicalBaseline.status",
        "contextProjection.technicalBaseline.scope",
        "contextProjection.technicalBaseline.summary",
        "contextProjection.technicalBaseline.mustFollow",
        "architectureQualitySeed.required",
        "architectureQualitySeed.qualityLevel",
        "architectureQualitySeed.techReferenceProfile.loadMode",
        "architectureQualitySeed.techReferenceProfile.groups.arch",
        "architectureQualitySeed.techReferenceProfile.referenceLoadPlan",
    ]);
    if matches!(
        section,
        ArchitectureSectionGroup::Foundation
            | ArchitectureSectionGroup::DomainContract
            | ArchitectureSectionGroup::Behavior
            | ArchitectureSectionGroup::Coverage
    ) {
        core_fields.extend([
            "contextProjection.phaseScope.acceptanceCandidates",
            "contextProjection.requirementDetailTransfer.requirementDetails",
            "contextProjection.requirementDetailTransfer.acceptanceDetails",
            "contextProjection.requirementDetailTransfer.businessFlows",
            "allowedRefs.scopeRefs",
            "allowedRefs.acceptanceRefs",
            "allowedRefs.deferredScopeRefs",
            "allowedRefs.excludedScopeRefs",
            "allowedRefs.requirementDetailIds",
        ]);
    }
    let mut contract_fields = vec![
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
        "enumRefs.acceptancePriority",
        "enumRefs.architectureQuality",
    ];
    if !api_quality_seed.is_null() && matches!(section, ArchitectureSectionGroup::DomainContract) {
        contract_fields.push("enumRefs.apiQuality");
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
            "selectors": read_selectors_value_from_paths(core_fields)
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
            "selectors": read_selectors_value_from_paths(contract_fields)
        }),
    ];
    if !api_quality_seed.is_null() {
        groups.push(json!({
            "groupId": "architecture_api_quality_context",
            "required": matches!(section, ArchitectureSectionGroup::DomainContract),
            "purpose": "Read the MCP-derived API quality seed only when generating or repairing the current HTTP interface section.",
            "whenToRead": "Read when sectionState.currentSection is domain_contract, or when Loom is rebuilding a repair request that must preserve API applicability.",
            "selectors": read_selectors_value_from_paths(api_quality_seed_read_fields())
        }));
    }
    if matches!(section, ArchitectureSectionGroup::FrontendExperience) {
        let mut frontend_fields = vec!["frontendExperienceSource.authorityRule"];
        for ref_key in [
            "confirmedFrontendExperienceRef",
            "currentFrontendExperienceRef",
            "repositoryContextRef",
        ] {
            if has_non_null_key(frontend_experience_source, ref_key) {
                frontend_fields.push(match ref_key {
                    "confirmedFrontendExperienceRef" => {
                        "frontendExperienceSource.confirmedFrontendExperienceRef"
                    }
                    "currentFrontendExperienceRef" => {
                        "frontendExperienceSource.currentFrontendExperienceRef"
                    }
                    "repositoryContextRef" => "frontendExperienceSource.repositoryContextRef",
                    _ => unreachable!(),
                });
            }
        }
        frontend_fields.extend([
            "contextProjection.requirementDetailTransfer.frontendExperienceDetails",
            "contextProjection.requirementDetailTransfer.userFacingLanguage",
            "uiQualitySeed.required",
            "uiQualitySeed.scenarioCandidates",
            "uiQualitySeed.qualityLevel",
            "uiQualitySeed.surfacePolicyCandidates",
            "uiQualitySeed.layoutBaselineCandidates",
            "uiQualitySeed.densityCandidates",
            "uiQualitySeed.semanticTokenPolicy",
            "uiQualitySeed.requiredReferenceGroups",
            "uiQualitySeed.referenceLoadPlan",
            "uiQualitySeed.qualityRulePreview",
            "uiQualitySeed.stackReferenceCandidates",
            "uiQualitySeed.designTokenAssetPlan",
            "uiQualitySeed.forbiddenUserVisibleContent",
            "uiQualitySeed.requiredUiStates",
            "uiQualitySeed.selectionRule",
        ]);
        groups.push(json!({
            "groupId": "architecture_frontend_context",
            "required": true,
            "purpose": "Read the frontend authority refs for frontend_experience.",
            "whenToRead": "Read when sectionState.currentSection is frontend_experience.",
            "selectors": read_selectors_value_from_paths(frontend_fields)
        }));
    }
    if matches!(
        section,
        ArchitectureSectionGroup::Foundation
            | ArchitectureSectionGroup::DomainContract
            | ArchitectureSectionGroup::Behavior
    ) {
        groups.push(json!({
            "groupId": "architecture_domain_model_context",
            "required": true,
            "purpose": "Read compact actors and capability groups for structural, domain, and behavior architecture sections.",
            "whenToRead": "Read when sectionState.currentSection is foundation, domain_contract, or behavior.",
            "selectors": read_selectors_value_from_paths([
                "contextProjection.requirementDetailTransfer.actors",
                "contextProjection.requirementDetailTransfer.capabilityGroups"
            ])
        }));
    }
    Value::Array(groups)
}

fn has_non_null_key(value: &Value, key: &str) -> bool {
    value.get(key).is_some_and(|item| !item.is_null())
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
    let mut value = json!({
        "authorityRule": "Use confirmed/current frontend refs as the frontend_experience authority. RepositoryContext and TechnicalBaseline are implementation facts only."
    });
    if let Some(confirmed_frontend_ref) = phase.latest_refs.get("confirmedFrontendExperience") {
        value["confirmedFrontendExperienceRef"] = json!(confirmed_frontend_ref);
    }
    if let Some(current_frontend_ref) = phase.latest_refs.get("currentFrontendExperience") {
        value["currentFrontendExperienceRef"] = json!(current_frontend_ref);
    }
    if let Some(repository_context_ref) = phase.latest_refs.get("latestRepositoryContext") {
        value["repositoryContextRef"] = json!(repository_context_ref);
    }
    value
}

fn build_architecture_quality_seed() -> Value {
    let arch_groups = vec![
        "core", "patterns", "system", "data", "nfr", "adr", "failure",
    ];
    json!({
        "required": true,
        "qualityLevel": "production_delivery_architecture",
        "techReferenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": {
                "arch": arch_groups.clone()
            },
            "referenceLoadPlan": arch_groups.iter().map(|item| {
                json!({
                    "refId": format!("tech.arch.{item}"),
                    "path": format!("tech/arch/{item}.md"),
                    "reason": format!("Selected architecture {item} quality reference for this architecture section.")
                })
            }).collect::<Vec<_>>()
        }
    })
}

fn architecture_quality_enum_refs() -> Value {
    json!({
        "decisionStatus": ["accepted", "needs_user_decision"],
        "decisionCategory": [
            "architecture_style",
            "module_boundary",
            "data_boundary",
            "integration_boundary",
            "runtime_boundary",
            "security_boundary",
            "operability"
        ],
        "nfrCategory": [
            "performance",
            "scalability",
            "availability",
            "reliability",
            "security",
            "maintainability",
            "observability",
            "cost"
        ],
        "riskCategory": [
            "data_integrity",
            "integration",
            "runtime",
            "security",
            "operability",
            "maintainability"
        ],
        "riskSeverity": ["low", "medium", "high", "critical"],
        "riskLikelihood": ["low", "medium", "high"]
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
        "phaseScopeSummary": phase_scope_summary(planning_contract),
        "technicalBaseline": planning_contract.technical_baseline,
        "requirementDetailTransfer": {
            "requirementDetails": compact_requirement_details_index(planning_contract),
            "acceptanceDetails": planning_contract.phase_scope.acceptance_candidates,
            "actors": planning_contract.planning_inputs.actors,
            "capabilityGroups": planning_contract.planning_inputs.capability_groups,
            "frontendExperienceDetails": planning_contract.planning_inputs.frontend_experience,
            "userFacingLanguage": planning_contract.planning_inputs.user_facing_language,
            "businessFlows": planning_contract.planning_inputs.business_flows
        }
    })
}

fn phase_scope_summary(planning_contract: &PlanningGenerationContract) -> Value {
    json!({
        "includedIds": scope_ids(&planning_contract.phase_scope.included),
        "includedLabels": scope_labels(&planning_contract.phase_scope.included),
        "includedItems": scope_items(&planning_contract.phase_scope.included),
        "deferredIds": scope_ids(&planning_contract.phase_scope.deferred),
        "deferredLabels": scope_labels(&planning_contract.phase_scope.deferred),
        "deferredItems": scope_items(&planning_contract.phase_scope.deferred),
        "excludedIds": scope_ids(&planning_contract.phase_scope.excluded),
        "excludedLabels": scope_labels(&planning_contract.phase_scope.excluded),
        "excludedItems": scope_items(&planning_contract.phase_scope.excluded)
    })
}

fn scope_ids(items: &[contracts::ScopeItem]) -> Vec<String> {
    items.iter().map(|item| item.id.clone()).collect()
}

fn scope_labels(items: &[contracts::ScopeItem]) -> Vec<String> {
    items.iter().map(|item| item.label.clone()).collect()
}

fn scope_items(items: &[contracts::ScopeItem]) -> Vec<Vec<String>> {
    items.iter().map(|item| item.items.clone()).collect()
}

fn compact_requirement_details_index(planning_contract: &PlanningGenerationContract) -> Value {
    json!({
        "schemaVersion": planning_contract.requirement_details.schema_version,
        "authority": planning_contract.requirement_details.authority,
        "sourceBrainstormContractRef": planning_contract.requirement_details.source_brainstorm_contract_ref,
        "items": planning_contract
            .requirement_details
            .items
            .iter()
            .map(|item| {
                json!({
                    "detailId": item.detail_id,
                    "kind": item.kind,
                    "title": item.title,
                    "summary": item.summary,
                    "requiredForCurrentPhase": item.required_for_current_phase,
                    "priority": item.priority,
                    "sourceRefs": item.source_refs,
                    "scopeRefs": item.scope_refs,
                    "acceptanceRefs": item.acceptance_refs,
                    "conceptRefs": item.concept_refs,
                    "frontendRefs": item.frontend_refs,
                    "impactTags": item.impact_tags,
                    "lifecycleStage": item.lifecycle_stage,
                    "quality": item.quality,
                    "unresolvedNote": item.unresolved_note,
                })
            })
            .collect::<Vec<_>>(),
        "extractionWarningCount": planning_contract.requirement_details.extraction_warnings.len(),
        "fullDetailSource": "sourceRefs.planningContractRef#/requirementDetails"
    })
}

fn build_section_outputs(
    project_root: &Path,
    request_id: &str,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    planning_contract: &PlanningGenerationContract,
    api_quality_seed: &Value,
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
                schema_shape: section_schema_shape(
                    section,
                    has_previous_runtime_delivery,
                    api_quality_seed,
                ),
                result_template: section_result_template(
                    section,
                    has_previous_runtime_delivery,
                    frontend_experience_source,
                    planning_contract,
                    api_quality_seed,
                ),
                enum_refs: section_enum_refs(
                    section,
                    has_previous_runtime_delivery,
                    api_quality_seed,
                ),
                generation_rules: section_generation_rules(
                    section,
                    has_previous_runtime_delivery,
                    api_quality_seed,
                ),
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
        ArchitectureSectionGroup::Foundation => vec!["engineeringBoundary", "modules"],
        ArchitectureSectionGroup::DomainContract => vec!["dataModel", "interfaces"],
        ArchitectureSectionGroup::Behavior => vec!["userFlows", "stateMachines"],
        ArchitectureSectionGroup::FrontendExperience => vec!["frontendExperience"],
        ArchitectureSectionGroup::RuntimeDelivery => vec!["runtimeDelivery"],
        ArchitectureSectionGroup::Coverage => {
            vec![
                "acceptanceMatrix",
                "detailCoverage",
                "architectureQuality",
                "handoff",
            ]
        }
    }
}

pub(crate) fn section_schema_shape(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    api_quality_seed: &Value,
) -> Value {
    json!({
        "status": "ready | blocked",
        "content": section_content_shape(section, has_previous_runtime_delivery, api_quality_seed),
        "blockedReasons": [{
            "code": "string",
            "message": "string",
            "nextNode": "string"
        }]
    })
}

fn section_content_shape(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    api_quality_seed: &Value,
) -> Value {
    match section {
        ArchitectureSectionGroup::Foundation => json!({
            "engineeringBoundary": {
                "summary": "string",
                "applications": ["object"],
                "modules": ["object"],
                "applicationInteractions": [{
                    "interactionId": "string",
                    "providerApplicationRef": "string",
                    "consumerApplicationRefs": ["string"],
                    "providerModuleRef": "string",
                    "interactionType": "http_api | service_method | external_adapter | event | job | cli_command",
                    "protocol": "string",
                    "interfaceRefs": ["string for an existing project API contract interface"],
                    "qualityTraits": {
                        "authRequirement": "not_applicable | required | optional | deferred_with_risk",
                        "paginationRequired": "boolean",
                        "contractArtifactRequired": "boolean",
                        "compatibilityRequired": "boolean",
                        "operationalPolicies": ["idempotency | cache | retry | rate_limit | request_id"]
                    },
                    "scopeRefs": ["string"],
                    "acceptanceRefs": ["string"]
                }]
            },
            "modules": ["object"]
        }),
        ArchitectureSectionGroup::DomainContract => json!({
            "dataModel": {
                "entities": ["object"],
                "relationships": ["object"],
                "constraints": ["object"]
            },
            "interfaces": domain_contract_interfaces_shape(api_quality_seed),
            "apiContract": api_contract_shape(api_quality_seed)
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
                "uiSurfaceRegistry": {
                    "registryId": "string",
                    "selectionRule": "string",
                    "surfaces": [{
                        "surfaceId": "string",
                        "surfaceRole": "app_shell | page | panel | drawer | modal | table | form | detail | widget | navigation | feedback_area",
                        "businessPurpose": "string",
                        "productIntent": {
                            "userRole": "string",
                            "businessObject": "string",
                            "primaryJob": "string",
                            "successOutcome": "string"
                        },
                        "compositionModel": {
                            "requiredRegions": ["string"],
                            "forbiddenRegions": ["string"],
                            "primaryRegion": "string",
                            "supportingRegions": ["string"]
                        },
                        "informationModel": {
                            "mustShow": ["string"],
                            "scanPriority": ["string"],
                            "identityFields": ["string"],
                            "statusFields": ["string"],
                            "longContentPolicy": "string"
                        },
                        "actionModel": {
                            "primaryActions": ["string"],
                            "contextualActions": ["string"],
                            "dangerousActions": ["string"],
                            "placementRule": "string",
                            "postSuccessUpdate": "string"
                        },
                        "statePlacementModel": {
                            "loading": "string",
                            "empty": "string",
                            "error": "string",
                            "success": "string",
                            "business_blocking": "string",
                            "validation": "string",
                            "disabled": "string"
                        },
                        "visualModel": {
                            "layoutBaseline": "string",
                            "density": "string",
                            "tokenPolicy": "string",
                            "componentPolicy": "string",
                            "antiDemoRules": ["string"]
                        },
                        "responsiveModel": {
                            "desktop": "string",
                            "tablet": "string",
                            "mobile": "string"
                        },
                        "requiredComposition": ["string"],
                        "forbiddenComposition": ["string"],
                        "stateRefs": ["loading | success | error | empty | business_blocking"],
                        "dataViewRefs": ["string"],
                        "actionRefs": ["string"],
                        "operationPathRefs": ["string"],
                        "workflowRefs": ["string"],
                        "interfaceRefs": ["string"]
                    }]
                },
                "surfaceDecisionCandidate": ui_surface_decision_candidate_shape(),
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
            "architectureQuality": {
                "decisions": [{
                    "decisionId": "string",
                    "category": "architecture_style | module_boundary | data_boundary | integration_boundary | runtime_boundary | security_boundary | operability",
                    "title": "string",
                    "status": "accepted | needs_user_decision",
                    "context": "string",
                    "decision": "string",
                    "alternativesConsidered": [{
                        "name": "string",
                        "tradeoff": "string",
                        "rejectedBecause": "string"
                    }],
                    "consequences": {
                        "positive": ["string"],
                        "negative": ["string"],
                        "neutral": ["string"]
                    },
                    "sourceRefs": {
                        "scopeRefs": ["string"],
                        "acceptanceRefs": ["string"],
                        "requirementDetailRefs": ["string"]
                    },
                    "verificationHints": ["string"]
                }],
                "nfrs": [{
                    "nfrId": "string",
                    "category": "performance | reliability | security | maintainability | observability | cost",
                    "target": "string",
                    "rationale": "string",
                    "architectureRefs": {
                        "decisions": ["string"],
                        "risks": ["string"]
                    },
                    "verificationStrategy": "string"
                }],
                "risks": [{
                    "riskId": "string",
                    "category": "data_integrity | integration | runtime | security | operability | maintainability",
                    "severity": "low | medium | high | critical",
                    "likelihood": "low | medium | high",
                    "impact": "string",
                    "mitigation": "string",
                    "ownerArtifactRefs": {
                        "modules": ["string"],
                        "interfaces": ["string"],
                        "decisions": ["string"],
                        "nfrs": ["string"]
                    },
                    "verificationHints": ["string"]
                }]
            },
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
            "runtimeKind": "string",
            "basis": Value::Object(basis),
            "build": {
                "command": "string",
                "workingDirectory": "string",
                "outputs": ["string"],
                "codeLevelExpectations": ["string"]
            },
            "start": {
                "command": "string",
                "workingDirectory": "string",
                "port": "number",
                "codeLevelExpectations": ["string"]
            },
            "runtimeSurfaces": ["object"],
            "httpProbes": {
                "previewPath": "string",
                "expectedStatus": "2xx_or_3xx"
            },
            "frontend": "optional object when a separate frontend surface exists",
            "api": "optional object when a separate API/backend surface exists",
            "environment": {
                "required": ["string"],
                "optional": ["string"]
            },
            "taskPlanningGuidance": {
                "requireRuntimeDeliveryRequirementWhenTaskTouches": ["string"],
                "doNotRequireForTaskKinds": ["string"],
                "verificationBoundary": "code_level_only",
                "doNotRequireCleanInstallOrContainerBuild": "boolean"
            }
        }
    })
}

fn domain_contract_interfaces_shape(api_quality_seed: &Value) -> Value {
    if api_quality_seed.is_null() {
        return json!(["object"]);
    }
    json!([{
        "interfaceId": "string",
        "name": "string",
        "type": "http_api | service_method | external_adapter | event | job | cli_command",
        "resource": "string when type=http_api",
        "operationKind": "create | read_list | read_detail | replace | update | delete | state_transition | domain_action | search | export",
        "method": "GET | POST | PUT | PATCH | DELETE | HEAD | OPTIONS when type=http_api",
        "path": "string when type=http_api",
        "requestSchema": ["object"],
        "responseSchema": ["object"],
        "statusCodes": {
            "success": ["number"],
            "validation": ["number"],
            "businessConflict": ["number"],
            "notFound": ["number"],
            "auth": ["number"],
            "rateLimit": ["number"],
            "serviceUnavailable": ["number"],
            "serverError": ["number"]
        },
        "errorSchema": ["object"],
        "paginationPolicy": "optional object for unbounded collection endpoints",
        "authPolicy": "optional object for protected operations",
        "contractFileRefs": ["string"],
        "idempotencyPolicy": "optional object for retry-sensitive or duplicate-sensitive operations",
        "cachePolicy": "optional object for cacheable reads",
        "conditionalRequestPolicy": "optional object for optimistic concurrency or cache validators",
        "rateLimitPolicy": "optional object for abuse-sensitive endpoints",
        "retryPolicy": "optional object for retryable dependency/runtime failures",
        "requestIdPolicy": "optional object for traceable critical APIs",
        "scopeRefs": ["string"],
        "acceptanceRefs": ["string"],
        "requirementDetailRefs": ["string"]
    }])
}

pub(crate) fn section_result_template(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    planning_contract: &PlanningGenerationContract,
    api_quality_seed: &Value,
) -> Value {
    json!({
        "status": "ready",
        "content": section_content_template(
            section,
            has_previous_runtime_delivery,
            frontend_experience_source,
            planning_contract,
            api_quality_seed
        ),
        "blockedReasons": []
    })
}

fn section_content_template(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    frontend_experience_source: &Value,
    planning_contract: &PlanningGenerationContract,
    api_quality_seed: &Value,
) -> Value {
    match section {
        ArchitectureSectionGroup::Foundation => json!({
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
                }],
                "applicationInteractions": []
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
            "interfaces": domain_contract_interfaces_template(api_quality_seed),
            "apiContract": api_contract_template(api_quality_seed)
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
                "uiSurfaceRegistry": {
                    "registryId": "ui-registry-1",
                    "selectionRule": "Use this registry as the source for TaskPlan frontendExperienceRequirement execution guidance. Each frontend task should receive only the surfaces, data views, actions, operation paths, states, and bindings it owns.",
                    "surfaces": [{
                        "surfaceId": "surface_1",
                        "surfaceRole": "page",
                        "businessPurpose": "",
                        "productIntent": {
                            "userRole": "",
                            "businessObject": "",
                            "primaryJob": "",
                            "successOutcome": ""
                        },
                        "compositionModel": {
                            "requiredRegions": [
                                "business navigation or local context",
                                "task-relevant data region",
                                "task-relevant action region",
                                "scoped feedback region"
                            ],
                            "forbiddenRegions": [
                                "decorative or explanatory region that displaces the task workflow"
                            ],
                            "primaryRegion": "task-relevant data or form region",
                            "supportingRegions": [
                                "navigation/context",
                                "detail/summary",
                                "feedback"
                            ]
                        },
                        "informationModel": {
                            "mustShow": [
                                "business object identity",
                                "business object status",
                                "fields required to complete the task"
                            ],
                            "scanPriority": [
                                "identity",
                                "status",
                                "decision fields",
                                "available actions"
                            ],
                            "identityFields": [],
                            "statusFields": [],
                            "longContentPolicy": "Preserve scanability with truncation, wrapping, drill-down, or responsive reflow based on the selected scenario."
                        },
                        "actionModel": {
                            "primaryActions": ["task-owned primary action"],
                            "contextualActions": [],
                            "dangerousActions": [],
                            "placementRule": "Place actions where the user makes the decision, keeping affected object identity visible.",
                            "postSuccessUpdate": "Update the affected row, detail, count, state, or route; do not rely only on a toast."
                        },
                        "statePlacementModel": {
                            "loading": "Near the region or control waiting for data or mutation.",
                            "empty": "In the data/form region with business next action when applicable.",
                            "error": "Near the affected region with recovery path.",
                            "success": "Inline object update plus short confirmation when useful.",
                            "business_blocking": "Near the blocked field, row, detail, or action.",
                            "validation": "Near the field and summary for longer forms.",
                            "disabled": "On or near disabled controls with unlock reason when actionable."
                        },
                        "visualModel": {
                            "layoutBaseline": "custom_product_layout",
                            "density": "balanced",
                            "tokenPolicy": "Use existing or planned semantic tokens before page-local styling.",
                            "componentPolicy": "Use task-fit components instead of decorative cards or explainer sections.",
                            "antiDemoRules": [
                                "no runtime commands or delivery notes in product UI",
                                "no marketing hero for operational surfaces",
                                "no decorative filler before required workflow content"
                            ]
                        },
                        "responsiveModel": {
                            "desktop": "Keep primary task surface and action path visible without layout shift.",
                            "tablet": "Preserve task order while reducing secondary regions.",
                            "mobile": "Use drill-down, cards, or stacked regions when dense comparison is not required."
                        },
                        "requiredComposition": [
                            "business navigation or context",
                            "task-relevant data view",
                            "task-relevant action area",
                            "local loading, empty, error, success, and business-blocking feedback"
                        ],
                        "forbiddenComposition": [
                            "surface composition unrelated to the task-owned business workflow",
                            "decorative or explanatory sections that displace required data, actions, states, or feedback"
                        ],
                        "stateRefs": ["loading", "success", "error", "empty", "business_blocking"],
                        "dataViewRefs": ["view_1"],
                        "actionRefs": ["action_1"],
                        "operationPathRefs": ["path_1"],
                        "workflowRefs": [],
                        "interfaceRefs": []
                    }]
                },
                "surfaceDecisionCandidate": ui_surface_decision_candidate_template(),
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
                "coverage": [acceptance_coverage_artifact_template()],
                "verificationHints": [{
                    "kind": "manual",
                    "description": ""
                }]
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
                "artifactRefs": detail_coverage_artifact_refs_template()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "acceptanceMatrix": acceptance_matrix,
        "detailCoverage": detail_coverage,
        "architectureQuality": architecture_quality_template(),
        "handoff": {
            "readyForTaskPlan": true,
            "blockingReasons": [],
            "nextNode": "task_plan"
        }
    })
}

fn architecture_quality_template() -> Value {
    json!({
        "decisions": [{
            "decisionId": "adr-current-001",
            "category": "architecture_style",
            "title": "Current phase architecture decision",
            "status": "accepted",
            "context": "State the current-phase forces from requirementDetailTransfer and the confirmed technical baseline.",
            "decision": "State the selected architecture approach for this phase.",
            "alternativesConsidered": [{
                "name": "alternative architecture approach",
                "tradeoff": "Concrete trade-off compared with the selected approach.",
                "rejectedBecause": "Why this alternative is not the best fit for the current phase."
            }],
            "consequences": {
                "positive": ["Implementation or verification benefit."],
                "negative": ["Implementation or operation cost to watch."],
                "neutral": ["Known side effect that does not block delivery."]
            },
            "sourceRefs": {
                "scopeRefs": ["allowedRefs.scopeRefs item"],
                "acceptanceRefs": ["allowedRefs.acceptanceRefs item"],
                "requirementDetailRefs": ["allowedRefs.requirementDetailIds item"]
            },
            "verificationHints": ["How later tasks or review can prove this decision was respected."]
        }],
        "nfrs": [{
            "nfrId": "nfr-current-001",
            "category": "maintainability",
            "target": "Concrete quality target for this phase.",
            "rationale": "Why this target matters for the current phase.",
            "architectureRefs": {
                "decisions": ["adr-current-001"],
                "risks": ["risk-current-001"]
            },
            "verificationStrategy": "How TaskPlan, tests, static checks, or review can verify this quality target."
        }],
        "risks": [{
            "riskId": "risk-current-001",
            "category": "data_integrity",
            "severity": "medium",
            "likelihood": "medium",
            "impact": "Concrete implementation or operation impact if this risk occurs.",
            "mitigation": "Concrete design or task-plan mitigation.",
            "ownerArtifactRefs": {
                "modules": ["module_1"],
                "interfaces": ["interface_1"],
                "decisions": ["adr-current-001"],
                "nfrs": ["nfr-current-001"]
            },
            "verificationHints": ["How later tasks or review can prove mitigation was implemented."]
        }]
    })
}

fn acceptance_coverage_artifact_template() -> Value {
    json!({
        "type": "module",
        "refs": [],
        "description": ""
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
            "runtimeKind": "",
            "basis": Value::Object(basis),
            "build": {
                "command": "",
                "workingDirectory": ".",
                "outputs": [],
                "codeLevelExpectations": [""]
            },
            "start": {
                "command": "",
                "workingDirectory": ".",
                "codeLevelExpectations": [""]
            },
            "runtimeSurfaces": [{
                "surfaceId": "runtime_surface_1",
                "kind": "",
                "urlPath": "",
                "purpose": ""
            }],
            "httpProbes": {
                "previewPath": "/",
                "expectedStatus": "2xx_or_3xx"
            },
            "environment": {
                "required": [],
                "optional": []
            },
            "taskPlanningGuidance": {
                "requireRuntimeDeliveryRequirementWhenTaskTouches": [
                    "build_or_packaging",
                    "runtime_entry",
                    "serving_or_routing",
                    "configuration_or_environment",
                    "generated_artifacts",
                    "runtime_surface"
                ],
                "doNotRequireForTaskKinds": [
                    "domain_only_validation",
                    "pure_unit_test_additions"
                ],
                "verificationBoundary": "code_level_only",
                "doNotRequireCleanInstallOrContainerBuild": true
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

fn domain_contract_interfaces_template(api_quality_seed: &Value) -> Value {
    if api_quality_seed.is_null() {
        return json!([]);
    }
    json!([{
        "interfaceId": "api_current_001",
        "name": "Current phase API or service interface",
        "type": "http_api",
        "resource": "",
        "operationKind": "create",
        "method": "POST",
        "path": "/api/current-resources",
        "requestSchema": [{
            "field": "replace_with_request_field",
            "required": true,
            "kind": "string",
            "validation": "business validation rule"
        }],
        "responseSchema": [{
            "field": "id",
            "required": true,
            "kind": "identifier",
            "meaning": "Created or affected resource id"
        }],
        "statusCodes": {
            "success": [201],
            "validation": [400, 422],
            "businessConflict": [409],
            "notFound": [404],
            "auth": [],
            "rateLimit": [],
            "serviceUnavailable": [],
            "serverError": [500]
        },
        "errorSchema": [{
            "field": "message",
            "required": true,
            "kind": "user_actionable_message"
        }],
        "paginationPolicy": {
            "strategy": "not_applicable",
            "requestFields": [],
            "responseFields": []
        },
        "authPolicy": {
            "required": "not_applicable",
            "actorRefs": [],
            "permissionRefs": []
        },
        "contractFileRefs": [],
        "idempotencyPolicy": {
            "required": false,
            "keyHeader": "",
            "duplicateBehavior": ""
        },
        "cachePolicy": {
            "strategy": "not_applicable",
            "validators": []
        },
        "conditionalRequestPolicy": {
            "required": false,
            "staleUpdateStatus": null
        },
        "rateLimitPolicy": {
            "applies": false,
            "status": null,
            "headers": []
        },
        "retryPolicy": {
            "retryableStatuses": [],
            "retryAfterHeader": false
        },
        "requestIdPolicy": {
            "header": "",
            "includedInErrorBody": false
        },
        "scopeRefs": [],
        "acceptanceRefs": [],
        "requirementDetailRefs": []
    }])
}

fn api_contract_shape(api_quality_seed: &Value) -> Value {
    if api_quality_seed.is_null() {
        return Value::Null;
    }
    json!({
        "publicExposure": {
            "basePath": "string",
            "preservePath": "boolean"
        },
        "browserBinding": {
            "mode": "same_origin | external_origin",
            "baseUrl": "string",
            "pathOwnership": "interface_path"
        }
    })
}

fn api_contract_template(api_quality_seed: &Value) -> Value {
    if api_quality_seed.is_null() {
        return Value::Null;
    }
    json!({
        "publicExposure": {
            "basePath": "/api",
            "preservePath": true
        },
        "browserBinding": {
            "mode": "same_origin",
            "baseUrl": "",
            "pathOwnership": "interface_path"
        }
    })
}

pub(crate) fn section_enum_refs(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    api_quality_seed: &Value,
) -> Value {
    match section {
        ArchitectureSectionGroup::Coverage => json!({
            "coverageStatus": ["covered", "partial", "not_applicable", "deferred", "uncovered"],
            "acceptancePriority": ["must", "should", "could"],
            "coverageArtifactType": COVERAGE_ARTIFACT_TYPES,
            "architectureQuality": architecture_quality_enum_refs()
        }),
        ArchitectureSectionGroup::RuntimeDelivery => json!({
            "runtimeDeliveryStatus": runtime_delivery_status_values(has_previous_runtime_delivery)
        }),
        ArchitectureSectionGroup::DomainContract if !api_quality_seed.is_null() => {
            json!({ "apiQuality": api_quality_enum_refs() })
        }
        ArchitectureSectionGroup::FrontendExperience => json!({
            "uiQuality": ui_quality_enum_refs(),
            "uiSurfaceDecision": ui_surface_decision_enum_refs()
        }),
        _ => json!({}),
    }
}

pub(crate) fn section_generation_rules(
    section: ArchitectureSectionGroup,
    has_previous_runtime_delivery: bool,
    api_quality_seed: &Value,
) -> Vec<String> {
    match section {
        ArchitectureSectionGroup::Foundation => vec![
            "Carry the planning and technical baseline identity into content.source.".to_string(),
            "Define the engineering boundary and current-phase modules only.".to_string(),
            "Declare every current-phase cross-application or cross-module communication boundary in engineeringBoundary.applicationInteractions. Choose interactionType from the structured protocol kinds; do not rely on API, backend, or framework words in business prose to activate later interface design.".to_string(),
            "For an existing accepted interface, use interfaceRefs. For a new boundary, leave interfaceRefs empty and provide provider/consumer ownership plus qualityTraits so MCP can generate the precise DomainContract reference plan.".to_string(),
            "For each http_api interaction, write the complete qualityTraits object. Set authRequirement from current actor/permission requirements or an existing interface policy; use not_applicable only when the current interface is intentionally unauthenticated, and use deferred_with_risk when protection is required but deferred.".to_string(),
            "Set paginationRequired only for a collection that can grow beyond a bounded current-phase list or whose confirmed data view requires paging. Set contractArtifactRequired only for an existing contract file, an explicit OpenAPI/documentation request, code generation, or a separately consumed/public contract. Set compatibilityRequired only when changing an interface with an existing or separately deployed consumer.".to_string(),
            "List operationalPolicies only for current idempotency, cache, retry, rate-limit, or request-id behavior supported by requirements, accepted architecture, or an existing API convention; use an empty array otherwise.".to_string(),
            "Read only files listed in architectureQualitySeed.techReferenceProfile.referenceLoadPlan; selected architecture groups are evidence labels only and do not copy reference prose into the candidate.".to_string(),
            "Describe why the chosen module and application boundary is sufficient for this phase and where later phases may extend without implementing deferred scope.".to_string(),
            "Follow the existing project and technical baseline shape before introducing a new module, adapter, or abstraction.".to_string(),
            "Add an abstraction only when it supports current-phase behavior, verification, or meaningful isolation; avoid pass-through wrappers.".to_string(),
            "Use allowedRefs.scopeRefs and allowedRefs.acceptanceRefs exactly; do not invent ids."
                .to_string(),
        ],
        ArchitectureSectionGroup::DomainContract => {
            let mut rules = vec![
                "Represent current-phase business objects, key fields, relationships, constraints, and interfaces."
                    .to_string(),
                "Use contextProjection.requirementDetailTransfer as the current phase detail authority."
                    .to_string(),
                "Consume the confirmed technical baseline stack as input; do not redo database or framework selection in architecture.".to_string(),
                "Describe data ownership, transaction boundaries, invariant enforcement, migration impact, and read/write consistency for the selected current-phase storage stack.".to_string(),
                "Preserve confirmed business terminology; record conflicts instead of casually renaming domain concepts."
                    .to_string(),
            ];
            if !api_quality_seed.is_null() {
                rules.extend([
                    "Model current-phase HTTP/API contracts in content.interfaces using apiQualitySeed.interfaceContract and files listed in apiQualitySeed.techReferenceProfile.referenceLoadPlan.".to_string(),
                    "For HTTP APIs, include resource, operationKind, method, path, requestSchema, responseSchema, statusCodes, errorSchema, and current-phase refs; include pagination/auth/contract/evolution/operations fields only when selected or applicable.".to_string(),
                    "Write content.apiContract.publicExposure and content.apiContract.browserBinding once for the current API surface. publicExposure.basePath is the externally served prefix, preservePath must describe proxy behavior, and browserBinding.pathOwnership must remain interface_path. Do not repeat these fields inside every interface.".to_string(),
                    "Do not introduce versioned API paths or OpenAPI files unless apiQualitySeed selects evolution or contract references or existing repository context requires them.".to_string(),
                ]);
            }
            rules
        }
        ArchitectureSectionGroup::Behavior => vec![
            "Represent current-phase user flows, state machines, blockers, and success outcomes."
                .to_string(),
            "Include failure paths, recovery behavior, consistency expectations, and business-blocking outcomes for stateful flows.".to_string(),
            "Do not reference future or deferred scope as if it were current-phase behavior."
                .to_string(),
        ],
        ArchitectureSectionGroup::FrontendExperience => vec![
            "Read frontendExperienceSource before writing this section.".to_string(),
            "Read uiQualitySeed before writing the UI surface decision candidate.".to_string(),
            "Preserve the confirmed/current frontend target instead of rediscovering it.".to_string(),
            "Use RepositoryContext and TechnicalBaseline only as implementation facts.".to_string(),
            "Write surfaceDecisionCandidate as the semantic UI decision input: ranked known patterns, selected known/hybrid/custom mode, semantic facts, layout anatomy, regions, information, actions, states, composition constraints, and content boundary.".to_string(),
            "For custom mode, fill nearestKnownPatterns plus complete semanticFacts, layoutModel, regionModel, actionModel, stateModel, compositionConstraints, and contentBoundary. Custom is stricter than known/hybrid, not a relaxed fallback.".to_string(),
            "Do not write referenceProfile, referenceLoadPlan, or derived rule lists inside surfaceDecisionCandidate. MCP owns reference planning and uiSurfaceDecisionContract.qualityRules derivation during submit.".to_string(),
            "Write uiSurfaceRegistry for every business UI surface that the current phase can task: app shells, pages, panels, drawers, modals, tables, forms, detail views, widgets, navigation, and feedback areas.".to_string(),
            "For each uiSurfaceRegistry surface, state the business purpose plus productIntent, compositionModel, informationModel, actionModel, statePlacementModel, visualModel, and responsiveModel only when those details are not already better represented by surfaceDecisionCandidate model objects.".to_string(),
            "Use requiredComposition/forbiddenComposition/stateRefs/dataViewRefs/actionRefs/operationPathRefs/workflowRefs/interfaceRefs as compact linking fields. Put product quality meaning in the model objects, not in repeated prose.".to_string(),
            "Business UI surfaces must directly serve the selected scenario and task workflow; honor uiQualitySeed.forbiddenUserVisibleContent without repeating reference prose.".to_string(),
        ],
        ArchitectureSectionGroup::RuntimeDelivery => vec![
            "Represent current-phase runtime delivery readiness, not a generic deployment wishlist."
                .to_string(),
            runtime_delivery_authority(has_previous_runtime_delivery).to_string(),
            "For status=modified, fill build.command, runtimeSurfaces, httpProbes.previewPath, httpProbes.expectedStatus, and taskPlanningGuidance so TaskPlan and Deploy do not guess runtime facts."
                .to_string(),
            "Do not choose deploymentShape manually. MCP derives it during submit from frontend/api endpoint objects, runtimeSurfaces, servedBy, and role-labeled commands.".to_string(),
            "Do not author httpProbes.apiPaths or api.probePaths. Loom derives accepted API probe paths from DomainContract HTTP interfaces and carries them into runtime, integration, browser, and deploy projections.".to_string(),
            "Include frontend or api only when the current phase has a separate frontend or backend/API surface; omit unused optional endpoint objects."
                .to_string(),
            "Omit unknown optional runtime fields instead of writing null; include start.port only when a fixed port is known."
                .to_string(),
            "Runtime delivery is a code-level contract. Do not require Docker, clean install, registry access, or deploy success here."
                .to_string(),
            "Represent observability and runtime failure implications only when they affect current-phase build, start, probe, environment, or runtime surfaces."
                .to_string(),
        ],
        ArchitectureSectionGroup::Coverage => vec![
            "Map every current-phase acceptance candidate to AAC artifacts without inventing acceptance ids."
                .to_string(),
            "acceptanceMatrix.coverage entries must be objects with type, refs, and description."
                .to_string(),
            "Use requirementDetailTransfer.requirementDetails.items as the canonical detail index."
                .to_string(),
            "detailCoverage must store detailId plus artifact refs; do not copy full detail summaries."
                .to_string(),
            "Omit reason when coverageStatus=covered; write a non-empty reason when coverageStatus is partial, not_applicable, deferred, or uncovered."
                .to_string(),
            "Write content.architectureQuality with non-empty decisions, nfrs, and risks arrays using currentSectionContract.resultTemplate shape."
                .to_string(),
            "Each architectureQuality decision must include alternativesConsidered, consequences, sourceRefs, and verificationHints."
                .to_string(),
            "Each architectureQuality nfr must be concrete enough for TaskPlan or Review to verify; do not write vague quality words without a verificationStrategy."
                .to_string(),
            "Each architectureQuality risk must include severity, likelihood, impact, mitigation, ownerArtifactRefs, and verificationHints."
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_request_schema_exposes_surface_decision_candidate() {
        let content_shape = section_content_shape(
            ArchitectureSectionGroup::FrontendExperience,
            false,
            &Value::Null,
        );

        assert!(
            content_shape
                .pointer("/frontendExperience/surfaceDecisionCandidate/selectedPattern/mode")
                .is_some(),
            "frontend schema must ask the agent for a surface decision candidate"
        );
        assert!(
            content_shape
                .pointer("/frontendExperience/surfaceDecisionCandidate/semanticFacts")
                .is_some(),
            "frontend schema must collect semantic facts for MCP normalization"
        );
        assert!(
            content_shape
                .pointer("/frontendExperience/surfaceDecisionCandidate/contentBoundary")
                .is_some(),
            "frontend schema must collect content-boundary facts"
        );
    }

    #[test]
    fn frontend_request_exposes_surface_decision_enums_and_rules() {
        let enum_refs = section_enum_refs(
            ArchitectureSectionGroup::FrontendExperience,
            false,
            &Value::Null,
        );
        assert!(
            enum_refs
                .pointer("/uiSurfaceDecision/patternMode")
                .is_some(),
            "frontend enum refs must include surface decision enums"
        );

        let rules = section_generation_rules(
            ArchitectureSectionGroup::FrontendExperience,
            false,
            &Value::Null,
        );
        assert!(
            rules
                .iter()
                .any(|rule| rule.contains("surfaceDecisionCandidate")),
            "frontend generation rules must name surfaceDecisionCandidate"
        );
        assert!(
            rules.iter().any(|rule| rule.contains("Custom is stricter")),
            "custom mode must be described as stricter, not relaxed"
        );
        assert!(
            rules.iter().any(|rule| rule.contains("MCP owns reference")),
            "reference and rule derivation must stay MCP-owned"
        );
    }

    #[test]
    fn api_quality_seed_has_a_stable_read_group_after_domain_contract() {
        let seed = json!({
            "required": true,
            "techReferenceProfile": {"groups": {"api": ["core"]}}
        });
        let groups = architecture_read_groups(
            ArchitectureSectionGroup::Coverage,
            true,
            false,
            &json!({}),
            &json!({}),
            &seed,
        );
        let group = groups
            .as_array()
            .and_then(|items| {
                items.iter().find(|item| {
                    item.get("groupId").and_then(Value::as_str)
                        == Some("architecture_api_quality_context")
                })
            })
            .expect("API quality read group");
        assert!(
            group["selectors"].to_string().contains("apiQualitySeed"),
            "the dedicated group must expose the seed fields to MCP repair logic"
        );
    }

    #[test]
    fn foundation_request_requires_complete_structured_api_quality_traits() {
        let content_shape =
            section_content_shape(ArchitectureSectionGroup::Foundation, false, &Value::Null);
        let traits = content_shape
            .pointer("/engineeringBoundary/applicationInteractions/0/qualityTraits")
            .expect("foundation qualityTraits shape");
        assert_eq!(
            traits["authRequirement"],
            json!("not_applicable | required | optional | deferred_with_risk")
        );
        for field in [
            "paginationRequired",
            "contractArtifactRequired",
            "compatibilityRequired",
        ] {
            assert_eq!(traits[field], json!("boolean"), "missing {field}");
        }
        assert!(traits["operationalPolicies"].is_array());
    }

    #[test]
    fn foundation_generation_rules_explain_each_api_reference_signal() {
        let rules =
            section_generation_rules(ArchitectureSectionGroup::Foundation, false, &Value::Null)
                .join("\n");

        for signal in [
            "authRequirement",
            "paginationRequired",
            "contractArtifactRequired",
            "compatibilityRequired",
            "operationalPolicies",
        ] {
            assert!(
                rules.contains(signal),
                "foundation generation rules must explain {signal}"
            );
        }
        assert!(rules.contains("use an empty array otherwise"));
        assert!(rules.contains("existing or separately deployed consumer"));
    }
}
