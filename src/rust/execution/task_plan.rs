use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    architecture::ArchitectureDetailCoverageEntry, build_code_quality_seed, code_quality_enum_refs,
    code_reference_load_plan, code_reference_selection_for_task_with_context,
    execution::TaskArtifactRefs, package_naming_policy_for_reference_groups,
    planning::RequirementDetailItem, ui_surface_decision_enum_refs, AcceptancePriority,
    ApiContractRequirement, ArchitectureArtifactContract, ArchitectureQualityRequirement,
    BrowserEvidenceEnforcement, BrowserRunnerSource, BrowserVerificationMode,
    BrowserVerificationProfile, CodeQualityRequirement, CodeReferenceTaskContext, CoverageStatus,
    EngineeringQualityRequirement, ImplementationAction, ReferenceLoadPlanItem, TaskDefinition,
    TaskGroupRunState, TaskImplementationObligation, TaskKind, TaskPlan, TaskPlanGroup,
    TaskPlanGroupCandidateAgentWritable, TaskPlanHandoff, TaskPlanOutlineCandidateAgentWritable,
    TaskPlanPolicy, TaskPlanRun, TaskPlanRunNextAction, TaskPlanRunScheduler, TaskPlanRunStatus,
    TaskPlanRunSummary, TaskPlanScopeSnapshot, TaskPlanSource, TaskPlanStatus, TaskRunState,
    TaskRunStatus, TaskWriteBoundary, VerificationEvidence, VerificationIntent,
};
use delivery_core::{
    apply_delivery_index, read_selectors_value_from_paths, ArtifactKind, DeliveryLifecycleStatus,
    DomainDispatcher, ExecuteEditBoundary, ExecuteVerificationPolicy, FileSubmitInput,
    LoomMcpActionResult, LoomMcpAutoRunnableResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpNextAction, LoomMcpRepairableErrorResult, OperationContext, PostSubmitAction,
    RouteAction, RouteActionKind, SubmitAcceptedEvent, TransitionEngine, TransitionStore,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
};

use crate::api_contract::{exposure_projection, interfaces_for_refs, load_project_api_contract};
use crate::browser::{
    derive_browser_verification_profiles, scan_browser_automation_facts,
    task_requires_browser_verification,
};
use crate::paths::{
    task_plan_file, task_plan_group_pattern, task_plan_latest_file,
    task_plan_outline_candidate_file, task_plan_request_file, task_plan_run_file,
    task_plan_run_latest_file,
};
use crate::templates::{
    runtime_delivery_requirement_template, taskplan_group_result_template,
    taskplan_outline_result_template,
};

const TASK_KIND_VALUES: &[&str] = &[
    "feature_increment",
    "data_model_increment",
    "interface_increment",
    "ui_flow_increment",
    "frontend_experience",
    "runtime_delivery",
    "runtime_delivery_closure",
    "integration_increment",
    "verification_increment",
    "refactor_support",
    "configuration_support",
];

const IMPLEMENTATION_ACTION_VALUES: &[&str] = &[
    "create_or_update_entity",
    "create_or_update_persistence",
    "create_or_update_interface",
    "create_or_update_ui_flow",
    "create_or_update_state_machine",
    "create_or_update_business_rule",
    "add_reference_field",
    "validate_reference_format",
    "use_fixture_or_mock_data",
    "wire_reference_in_api_or_ui",
    "create_entity_crud",
    "create_entity_repository",
    "create_entity_admin_page",
    "create_entity_migration",
    "create_or_update_persistence_query",
    "implement_persistence_transaction",
    "optimize_persistence_query",
    "implement_analytical_query",
    "implement_entity_lifecycle",
    "add_or_update_tests",
    "add_or_update_persistence_tests",
    "add_or_update_config",
    "implement_authentication_or_authorization",
    "implement_async_processing",
    "implement_cache_policy",
    "implement_external_service_integration",
    "implement_resilience_policy",
    "configure_service_routing_or_discovery",
    "implement_observability",
    "migrate_framework_implementation",
    "implement_frontend_experience_contract",
    "implement_runtime_delivery_contract",
    "refactor_supporting_code",
];

const VERIFICATION_EVIDENCE_VALUES: &[&str] = &[
    "automated_test",
    "browser_automation",
    "manual_command_output",
    "runtime_api_check",
    "static_check",
    "agent_review_explanation",
];

pub(crate) const UI_OWNERSHIP_DIMENSION_VALUES: &[&str] = &[
    "surface",
    "data_view",
    "action",
    "state",
    "layout",
    "visual_system",
    "content_boundary",
    "integration_feedback",
];

pub fn materialize_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match materialize_request_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => failed(
            project_root,
            "TASKPLAN_REQUEST_FAILED",
            error.to_string(),
            "taskplan_generation",
        ),
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
                "phase {phase_id} does not exist in delivery {delivery_id}"
            ))
        })?;

    if let Some(existing_request_ref) = phase.latest_refs.get("taskPlanRequestRef").cloned() {
        if state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        })
        .map(|request| request.request_kind == "taskplan_generation_request")
        .unwrap_or(false)
        {
            return write_taskplan_result(project_root, &existing_request_ref);
        }
    }

    let baseline_ref = phase
        .latest_refs
        .get("technicalBaseline")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest technicalBaseline ref is missing".to_string(),
            )
        })?;
    let planning_ref = phase
        .latest_refs
        .get("planningContract")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest planningContract ref is missing".to_string(),
            )
        })?;
    let architecture_ref = phase
        .latest_refs
        .get("architectureArtifact")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest architectureArtifact ref is missing".to_string(),
            )
        })?;
    let baseline: contracts::TechnicalBaselineContract = read_project_json(root, &baseline_ref)?;
    let pgc: contracts::PlanningGenerationContract = read_project_json(root, &planning_ref)?;
    let aac: ArchitectureArtifactContract = read_project_json(root, &architecture_ref)?;
    let project_api_contract = load_project_api_contract(root, &aac)?;

    let request_id = format!("taskplan_{}", state::store::now_millis());
    let outline_file =
        to_project_relative(root, &task_plan_outline_candidate_file(root, &request_id))?;
    let group_file_pattern =
        to_project_relative(root, &task_plan_group_pattern(root, &request_id))?;
    let request_file =
        to_project_relative(root, &task_plan_request_file(root, &locator, &request_id))?;

    let request_root = build_request_root(
        &request_id,
        delivery_id,
        phase_id,
        &baseline_ref,
        &planning_ref,
        &architecture_ref,
        &baseline,
        &pgc,
        &aac,
        project_api_contract.as_ref(),
        &outline_file,
        &group_file_pattern,
    );
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "taskplan_generation_request".to_string(),
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
            .insert("taskPlanRequestId".to_string(), request_id);
        active_phase
            .latest_refs
            .insert("taskPlanRequestRef".to_string(), stored.request_ref.clone());
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    write_taskplan_result(project_root, &stored.request_ref)
}

fn build_request_root(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    baseline_ref: &str,
    planning_ref: &str,
    architecture_ref: &str,
    baseline: &contracts::TechnicalBaselineContract,
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
    project_api_contract: Option<&Value>,
    outline_file: &str,
    group_file_pattern: &str,
) -> Value {
    let outline_schema = serde_json::to_value(schema_for!(TaskPlanOutlineCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let group_schema = serde_json::to_value(schema_for!(TaskPlanGroupCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let requirement_transfer = requirement_detail_transfer(pgc, aac);
    let runtime_requirement_template =
        runtime_delivery_requirement_template(aac.runtime_delivery.as_ref());
    let runtime_closure_template = runtime_delivery_closure_task_template(aac);
    let frontend_requirement_template = frontend_experience_requirement_template(aac);
    let engineering_quality_template = engineering_quality_requirement_template(baseline);
    let phase_api_interfaces =
        interfaces_for_refs(project_api_contract, &aac.current_phase_interface_refs);
    let api_contract_template = api_contract_requirement_template(&phase_api_interfaces);
    let code_quality_seed = build_code_quality_seed(baseline);
    let code_quality_template = code_quality_requirement_template(&code_quality_seed);
    let mut source_refs = taskplan_source_refs(baseline_ref, planning_ref, architecture_ref, pgc);
    if let Some(api_contract_ref) = &aac.api_contract_ref {
        source_refs["apiContractRef"] = json!(api_contract_ref);
    }
    let outline_result_template = taskplan_outline_result_template();
    let group_result_template = taskplan_group_result_template();
    let mut output_contract = json!({
        "artifactKind": ArtifactKind::TaskPlanCandidate,
        "writeMode": "taskplan_grouped",
        "submitTool": "loom.taskPlanAcceptFile",
        "outlineFile": outline_file,
        "groupFilePattern": group_file_pattern,
        "writeTargets": [
            {
                "targetId": "outline",
                "path": outline_file,
                "required": true,
                "description": "Write the TaskPlan outline JSON."
            },
            {
                "targetId": "groups",
                "path": group_file_pattern,
                "required": false,
                "description": "Write one TaskPlan group JSON for each outline.groups[].groupId."
            }
        ],
        "pathAuthority": {
            "currentRequestOnly": true,
            "currentRequestId": request_id,
            "rule": "Only outputContract.outlineFile and outputContract.groupFilePattern belong to this TaskPlan generation."
        },
        "outlineSchemaShape": outline_schema,
        "groupSchemaShape": group_schema,
        "outlineResultTemplate": outline_result_template,
        "groupResultTemplate": group_result_template
    });
    if !runtime_requirement_template.is_null() {
        output_contract["runtimeDeliveryRequirementTemplate"] =
            runtime_requirement_template.clone();
    }
    if !runtime_closure_template.is_null() {
        output_contract["runtimeDeliveryClosureTaskTemplate"] = runtime_closure_template.clone();
    }
    if !frontend_requirement_template.is_null() {
        output_contract["frontendExperienceRequirementTemplate"] =
            frontend_requirement_template.clone();
    }
    if !engineering_quality_template.is_null() {
        output_contract["engineeringQualityRequirementTemplate"] =
            engineering_quality_template.clone();
    }
    if !api_contract_template.is_null() {
        output_contract["apiContractRequirementTemplate"] = api_contract_template.clone();
    }
    if !code_quality_template.is_null() {
        output_contract["codeQualityRequirementTemplate"] = code_quality_template.clone();
    }
    let mut context_projection = json!({
        "phaseId": phase_id,
        "planningContractId": pgc.planning_contract_id,
        "architectureArtifactContractId": aac.architecture_artifact_contract_id,
        "technicalBaseline": {
            "technicalBaselineId": baseline.technical_baseline_id,
            "projectKind": baseline.project_kind,
            "stack": baseline.stack
        },
        "requirementDetailTransfer": requirement_transfer
    });
    if !api_contract_template.is_null() {
        if let Some(object) = context_projection.as_object_mut() {
            object.insert(
                "apiContract".to_string(),
                exposure_projection(aac.api_contract_ref.as_deref(), project_api_contract),
            );
            object.insert(
                "apiInterfaces".to_string(),
                json!(phase_api_interfaces
                    .iter()
                    .map(compact_api_interface_for_task_plan)
                    .collect::<Vec<_>>()),
            );
        }
    }
    json!({
        "schemaVersion": "1.0",
        "requestType": "taskplan_grouped_generation",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::TaskPlanCandidate,
        "sourceRefs": source_refs,
        "contextProjection": context_projection,
        "allowedRefs": allowed_refs(pgc, aac),
        "generationRules": generation_rules(aac, &code_quality_seed),
        "enumRefs": enum_refs(),
        "codeQualitySeed": code_quality_seed,
        "outputContract": output_contract,
        "requestReadPlan": {
            "groups": taskplan_read_groups(
                &source_refs,
                &frontend_requirement_template,
                &runtime_requirement_template,
                &runtime_closure_template,
                &engineering_quality_template,
                &api_contract_template,
                &code_quality_template,
                &code_quality_seed
            )
        }
    })
}

fn taskplan_source_refs(
    baseline_ref: &str,
    planning_ref: &str,
    architecture_ref: &str,
    pgc: &contracts::PlanningGenerationContract,
) -> Value {
    let mut value = json!({
        "technicalBaselineRef": baseline_ref,
        "planningGenerationContractRef": planning_ref,
        "architectureArtifactContractRef": architecture_ref,
    });
    if let Some(phase_concept_grounding_ref) = &pgc.context_refs.phase_concept_grounding_ref {
        value["phaseConceptGroundingRef"] = json!(phase_concept_grounding_ref);
    }
    if let Some(delivery_concept_glossary_ref) = &pgc.context_refs.delivery_concept_glossary_ref {
        value["deliveryConceptGlossaryRef"] = json!(delivery_concept_glossary_ref);
    }
    if let Some(repository_context_ref) = &pgc.context_refs.repository_context_ref {
        value["repositoryContextRef"] = json!(repository_context_ref);
    }
    value
}

fn has_non_null_key(value: &Value, key: &str) -> bool {
    value.get(key).is_some_and(|item| !item.is_null())
}

fn taskplan_read_groups(
    source_refs: &Value,
    frontend_requirement_template: &Value,
    runtime_requirement_template: &Value,
    runtime_closure_template: &Value,
    engineering_quality_template: &Value,
    api_contract_template: &Value,
    code_quality_template: &Value,
    code_quality_seed: &Value,
) -> Value {
    let mut core_fields = vec![
        "sourceRefs.technicalBaselineRef",
        "sourceRefs.planningGenerationContractRef",
        "sourceRefs.architectureArtifactContractRef",
    ];
    if has_non_null_key(source_refs, "phaseConceptGroundingRef") {
        core_fields.push("sourceRefs.phaseConceptGroundingRef");
    }
    if has_non_null_key(source_refs, "deliveryConceptGlossaryRef") {
        core_fields.push("sourceRefs.deliveryConceptGlossaryRef");
    }
    if has_non_null_key(source_refs, "repositoryContextRef") {
        core_fields.push("sourceRefs.repositoryContextRef");
    }
    if has_non_null_key(source_refs, "apiContractRef") {
        core_fields.push("sourceRefs.apiContractRef");
    }
    core_fields.extend([
        "contextProjection.phaseId",
        "contextProjection.planningContractId",
        "contextProjection.architectureArtifactContractId",
        "contextProjection.technicalBaseline.stack",
        "allowedRefs.scopeRefs",
        "allowedRefs.acceptanceRefs",
        "allowedRefs.deferredScopeRefs",
        "allowedRefs.excludedScopeRefs",
        "allowedRefs.requirementDetailIds",
        "allowedRefs.moduleRefs",
        "allowedRefs.entityRefs",
        "allowedRefs.interfaceRefs",
        "allowedRefs.userFlowRefs",
        "allowedRefs.stateMachineRefs",
        "allowedRefs.decisionRefs",
        "allowedRefs.nfrRefs",
        "allowedRefs.riskRefs",
    ]);
    if !code_quality_seed.is_null() {
        core_fields.extend([
            "codeQualitySeed.required",
            "codeQualitySeed.qualityLevel",
            "codeQualitySeed.codeStackSignals",
            "codeQualitySeed.unmappedSignals",
        ]);
    }
    if !api_contract_template.is_null() {
        core_fields.extend([
            "contextProjection.apiContract",
            "contextProjection.apiInterfaces",
        ]);
    }
    let groups = vec![
        json!({
            "groupId": "taskplan_core_context",
            "required": true,
            "purpose": "Read the compact phase identity, technical baseline signal, and allowed ownership indexes before writing the TaskPlan outline.",
            "whenToRead": "Read first.",
            "selectors": read_selectors_value_from_paths(core_fields)
        }),
        json!({
            "groupId": "taskplan_requirement_context",
            "required": true,
            "purpose": "Read the current-phase requirement detail index, acceptance and business-flow summaries, workflow closure requirements, and task field mapping.",
            "whenToRead": "Read after taskplan_core_context and before assigning task ownership.",
            "projectionMode": "semantic_index",
            "selectors": read_selectors_value_from_paths([
                "contextProjection.requirementDetailTransfer.requirementDetailAssignment",
                "contextProjection.requirementDetailTransfer.currentPhaseScope",
                "contextProjection.requirementDetailTransfer.acceptanceDetails",
                "contextProjection.requirementDetailTransfer.businessFlowDetails",
                "contextProjection.requirementDetailTransfer.objectOperationDetailRules",
                "contextProjection.requirementDetailTransfer.workflowClosureRequirements",
                "contextProjection.requirementDetailTransfer.conceptRefs",
                "contextProjection.requirementDetailTransfer.taskPlanningFieldMapping"
            ])
        }),
        json!({
            "groupId": "taskplan_artifact_context",
            "required": true,
            "purpose": "Read compact artifact ownership, interface, runtime, UI operation-path, architecture-quality, and verification projections.",
            "whenToRead": "Read after taskplan_requirement_context and before writing group files.",
            "projectionMode": "artifact_ownership_projection",
            "selectors": read_selectors_value_from_paths([
                "contextProjection.requirementDetailTransfer.architectureDetails.modules",
                "contextProjection.requirementDetailTransfer.architectureDetails.applicationInteractions",
                "contextProjection.requirementDetailTransfer.architectureDetails.entities",
                "contextProjection.requirementDetailTransfer.architectureDetails.interfaces",
                "contextProjection.requirementDetailTransfer.architectureDetails.userFlows",
                "contextProjection.requirementDetailTransfer.architectureDetails.stateMachines",
                "contextProjection.requirementDetailTransfer.architectureDetails.frontendOperationPathDetails",
                "contextProjection.requirementDetailTransfer.architectureDetails.architectureQuality"
            ])
        }),
        json!({
            "groupId": "taskplan_generation_rules",
            "required": true,
            "purpose": "Read grouping, reference, verification, frontend, workflow, and runtime rules.",
            "whenToRead": "Read after core context and before writing group files.",
            "selectors": read_selectors_value_from_paths([
                "generationRules.groupedOutputRules",
                "generationRules.scopeAndReferenceRules",
                "generationRules.writeBoundaryRules",
                "generationRules.verificationEvidenceRules",
                "generationRules.detailOwnershipRules",
                "generationRules.conceptGroundingRules",
                "generationRules.frontendExperienceRules",
                "generationRules.workflowClosureRules",
                "generationRules.runtimeDeliveryRules",
                "generationRules.engineeringQualityRules",
                "generationRules.architectureQualityRules",
                "generationRules.apiContractRules",
                "generationRules.codeQualityRules"
            ])
        }),
        json!({
            "groupId": "taskplan_candidate_contract",
            "required": true,
            "purpose": "Read output paths, schema shapes, and enum refs before writing candidates.",
            "whenToRead": "Read before writing output files.",
            "selectors": read_selectors_value_from_paths(taskplan_candidate_contract_fields(
                frontend_requirement_template,
                runtime_requirement_template,
                runtime_closure_template,
                engineering_quality_template,
                api_contract_template,
                code_quality_template
            ))
        }),
    ];
    Value::Array(groups)
}

fn taskplan_candidate_contract_fields(
    frontend_requirement_template: &Value,
    runtime_requirement_template: &Value,
    runtime_closure_template: &Value,
    engineering_quality_template: &Value,
    api_contract_template: &Value,
    code_quality_template: &Value,
) -> Vec<&'static str> {
    let mut fields = vec![
        "enumRefs.taskKind",
        "enumRefs.implementationAction",
        "enumRefs.verificationEvidence",
        "enumRefs.uiQuality",
        "outputContract.outlineFile",
        "outputContract.groupFilePattern",
        "outputContract.pathAuthority",
        "outputContract.outlineResultTemplate",
        "outputContract.groupResultTemplate",
    ];
    if !frontend_requirement_template.is_null() {
        fields.push("outputContract.frontendExperienceRequirementTemplate");
    }
    if !runtime_requirement_template.is_null() {
        fields.push("outputContract.runtimeDeliveryRequirementTemplate");
    }
    if !runtime_closure_template.is_null() {
        fields.push("outputContract.runtimeDeliveryClosureTaskTemplate");
    }
    if !engineering_quality_template.is_null() {
        fields.push("outputContract.engineeringQualityRequirementTemplate");
    }
    if !api_contract_template.is_null() {
        fields.push("outputContract.apiContractRequirementTemplate");
    }
    if !code_quality_template.is_null() {
        fields.push("outputContract.codeQualityRequirementTemplate");
    }
    fields
}

pub fn accept_task_plan_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    match accept_task_plan_file_inner(
        input,
        authorized,
        TaskPlanSubmitMode::Generation,
        dispatcher,
    ) {
        Ok(result) => result,
        Err(error) => failed(
            &input.project_root,
            "TASKPLAN_ACCEPT_FAILED",
            error.to_string(),
            "taskplan_accept",
        ),
    }
}

pub fn accept_task_plan_repair_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    match accept_task_plan_file_inner(input, authorized, TaskPlanSubmitMode::Repair, dispatcher) {
        Ok(result) => result,
        Err(error) => failed(
            &input.project_root,
            "TASKPLAN_REPAIR_ACCEPT_FAILED",
            error.to_string(),
            "taskplan_repair_accept",
        ),
    }
}

#[derive(Debug, Clone, Copy)]
enum TaskPlanSubmitMode {
    Generation,
    Repair,
}

impl TaskPlanSubmitMode {
    fn latest_ref_key(self) -> &'static str {
        match self {
            Self::Generation => "taskPlanRequestRef",
            Self::Repair => "activeRepairActionRef",
        }
    }

    fn resubmit_tool(self) -> &'static str {
        match self {
            Self::Generation => "loom.taskPlanAcceptFile",
            Self::Repair => "loom.repairSubmitFile",
        }
    }

    fn fix_scope(self) -> &'static str {
        match self {
            Self::Generation => "taskplan_grouped_candidates_only",
            Self::Repair => "taskplan_repair_grouped_candidates_only",
        }
    }

    fn stale_code(self) -> &'static str {
        match self {
            Self::Generation => "STALE_TASKPLAN_REQUEST",
            Self::Repair => "STALE_TASKPLAN_REPAIR_REQUEST",
        }
    }

    fn route_action(self) -> &'static str {
        match self {
            Self::Generation => "taskplan_accept",
            Self::Repair => "taskplan_repair_accept",
        }
    }

    fn next_source(self) -> &'static str {
        match self {
            Self::Generation => "task_plan_accept",
            Self::Repair => "taskplan_repair_accept",
        }
    }

    fn next_reason(self) -> &'static str {
        match self {
            Self::Generation => "taskplan_ready",
            Self::Repair => "taskplan_repair_ready",
        }
    }
}

fn accept_task_plan_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    mode: TaskPlanSubmitMode,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskPlan request is missing deliveryId".to_string(),
        )
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("TaskPlan request is missing phaseId".to_string())
    })?;
    if let Some(stale) = ensure_latest_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
        mode,
    )? {
        return Ok(stale);
    }

    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec![
            "sourceRefs.technicalBaselineRef".to_string(),
            "sourceRefs.planningGenerationContractRef".to_string(),
            "sourceRefs.architectureArtifactContractRef".to_string(),
            "allowedRefs.scopeRefs".to_string(),
            "allowedRefs.acceptanceRefs".to_string(),
            "allowedRefs.deferredScopeRefs".to_string(),
            "allowedRefs.excludedScopeRefs".to_string(),
            "allowedRefs.requirementDetailIds".to_string(),
            "allowedRefs.moduleRefs".to_string(),
            "allowedRefs.entityRefs".to_string(),
            "allowedRefs.interfaceRefs".to_string(),
            "allowedRefs.userFlowRefs".to_string(),
            "allowedRefs.stateMachineRefs".to_string(),
            "allowedRefs.decisionRefs".to_string(),
            "allowedRefs.nfrRefs".to_string(),
            "allowedRefs.riskRefs".to_string(),
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })?
    .fields;
    let root = Path::new(&input.project_root);
    let outline_ref = string_field(&fields, "outputContract.outlineFile")?;
    let group_pattern = string_field(&fields, "outputContract.groupFilePattern")?;
    let allowed_refs = json!({
        "scopeRefs": value_field(&fields, "allowedRefs.scopeRefs"),
        "acceptanceRefs": value_field(&fields, "allowedRefs.acceptanceRefs"),
        "deferredScopeRefs": value_field(&fields, "allowedRefs.deferredScopeRefs"),
        "excludedScopeRefs": value_field(&fields, "allowedRefs.excludedScopeRefs"),
        "requirementDetailIds": value_field(&fields, "allowedRefs.requirementDetailIds"),
        "moduleRefs": value_field(&fields, "allowedRefs.moduleRefs"),
        "entityRefs": value_field(&fields, "allowedRefs.entityRefs"),
        "interfaceRefs": value_field(&fields, "allowedRefs.interfaceRefs"),
        "userFlowRefs": value_field(&fields, "allowedRefs.userFlowRefs"),
        "stateMachineRefs": value_field(&fields, "allowedRefs.stateMachineRefs"),
        "decisionRefs": value_field(&fields, "allowedRefs.decisionRefs"),
        "nfrRefs": value_field(&fields, "allowedRefs.nfrRefs"),
        "riskRefs": value_field(&fields, "allowedRefs.riskRefs")
    });
    let mut outline_value = read_project_json_value(root, &outline_ref)?;
    normalize_taskplan_outline_envelope(
        &mut outline_value,
        &authorized.request_id,
        &delivery_id,
        &phase_id,
    );
    let outline: TaskPlanOutlineCandidateAgentWritable = match deserialize_candidate(
        outline_value,
        "outline",
        "TASKPLAN_OUTLINE_SCHEMA_INVALID",
        Some("outline"),
    ) {
        Ok(outline) => outline,
        Err(issue) => {
            return Ok(repairable(
                input,
                authorized,
                outline_ref,
                vec![issue],
                mode,
            ));
        }
    };
    let mut issues = validate_outline(&outline);
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, outline_ref, issues, mode));
    }
    if outline.status != "ready" {
        issues.push(issue(
            "TASKPLAN_OUTLINE_BLOCKED",
            "outline.status",
            "TaskPlan outline must be ready before it can be accepted.",
            Some("outline"),
        ));
        return Ok(repairable(input, authorized, outline_ref, issues, mode));
    }

    let mut groups = Vec::new();
    let mut tasks = Vec::new();
    for group in &outline.groups {
        let group_file = group_pattern.replace("{groupId}", &group.group_id);
        let mut group_value = read_project_json_value(root, &group_file)?;
        normalize_taskplan_group_envelope(
            &mut group_value,
            &authorized.request_id,
            &delivery_id,
            &phase_id,
        );
        let mut candidate: TaskPlanGroupCandidateAgentWritable = match deserialize_candidate(
            group_value,
            "group",
            "TASKPLAN_GROUP_SCHEMA_INVALID",
            Some(&group.group_id),
        ) {
            Ok(candidate) => candidate,
            Err(issue) => {
                return Ok(repairable(input, authorized, group_file, vec![issue], mode));
            }
        };
        normalize_taskplan_write_boundaries(&mut candidate.tasks, &allowed_refs);
        issues.extend(validate_group_candidate(&candidate, group));
        groups.push(candidate.group.clone());
        tasks.extend(candidate.tasks);
    }
    let planning_ref = string_field(&fields, "sourceRefs.planningGenerationContractRef")?;
    let architecture_ref = string_field(&fields, "sourceRefs.architectureArtifactContractRef")?;
    let baseline_ref = string_field(&fields, "sourceRefs.technicalBaselineRef")?;
    let baseline: contracts::TechnicalBaselineContract = read_project_json(root, &baseline_ref)?;
    let pgc: contracts::PlanningGenerationContract = read_project_json(root, &planning_ref)?;
    let aac: ArchitectureArtifactContract = read_project_json(root, &architecture_ref)?;
    normalize_taskplan_candidate_relationships(&mut groups, &mut tasks, &pgc, &aac);
    normalize_runtime_delivery_requirements(&mut tasks, &aac);
    normalize_architecture_quality_artifact_refs(&aac, &mut tasks);
    let engineering_quality_requirements =
        normalize_engineering_quality_requirements(&baseline, &mut tasks);
    let architecture_quality_requirements =
        normalize_architecture_quality_requirements(&aac, &mut tasks);
    let api_contract_requirements =
        normalize_api_contract_requirements(&aac, &mut tasks, &allowed_refs, &baseline);
    normalize_structured_verification_intents(&aac, &mut tasks);
    normalize_task_verification_detail_refs(&mut tasks, &pgc, &aac);
    normalize_implementation_obligations(&baseline, &aac, &mut tasks);
    let code_quality_requirements =
        normalize_code_quality_requirements(&baseline, &aac, &mut tasks);
    issues.extend(validate_quality_requirement_ownership(
        &tasks,
        &engineering_quality_requirements,
        &architecture_quality_requirements,
        &api_contract_requirements,
        &code_quality_requirements,
    ));
    normalize_browser_verification_assignments(&mut tasks);
    issues.extend(validate_browser_verification_assignments(&tasks));
    let browser_automation_facts = scan_browser_automation_facts(root, &baseline);
    let source_browser_profiles =
        derive_browser_verification_profiles(&browser_automation_facts, &tasks);
    let browser_verification_profiles =
        materialize_browser_quality_closure(&mut groups, &mut tasks, source_browser_profiles);
    issues.extend(validate_taskplan_graph(&groups, &tasks));
    issues.extend(validate_taskplan_refs(&groups, &tasks, &allowed_refs));
    issues.extend(validate_runtime_delivery_requirements(&tasks));
    if !issues.is_empty() {
        // Preserve the actionable ownership diagnosis even when an earlier
        // structural check already requires repair. Otherwise a candidate
        // with no eligible owner is reported only as a generic preflight
        // failure and the agent cannot repair the missing assignment.
        issues.extend(validate_requirement_detail_assignments(&tasks, &pgc, &aac));
        return Ok(repairable(input, authorized, outline_ref, issues, mode));
    }

    issues.extend(validate_must_acceptance_task_coverage(&tasks, &pgc));
    issues.extend(validate_frontend_task_presence(&tasks, &aac));
    issues.extend(validate_frontend_quality_requirements(&tasks, &aac));
    issues.extend(validate_requirement_detail_assignments(&tasks, &pgc, &aac));
    issues.extend(validate_workflow_closure_task_assignments(&tasks, &aac));
    issues.extend(validate_runtime_delivery_closure_task(
        &groups,
        &tasks,
        aac.runtime_delivery.as_ref(),
    ));
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, outline_ref, issues, mode));
    }
    normalize_taskplan_write_boundaries(&mut tasks, &allowed_refs);
    let now = state::store::now_string();
    let task_plan = TaskPlan {
        schema_version: "1.0".to_string(),
        task_plan_id: outline.task_plan_id.clone(),
        version: 1,
        status: TaskPlanStatus::Ready,
        source: TaskPlanSource {
            roadmap_id: None,
            phase_id: phase_id.clone(),
            planning_generation_contract_id: pgc.planning_contract_id.clone(),
            architecture_artifact_contract_id: aac.architecture_artifact_contract_id.clone(),
            technical_baseline_id: baseline.technical_baseline_id.clone(),
            api_contract_ref: aac.api_contract_ref.clone(),
        },
        scope_snapshot: TaskPlanScopeSnapshot {
            included_scope_refs: pgc
                .phase_scope
                .included
                .iter()
                .map(|item| item.id.clone())
                .collect(),
            excluded_scope_refs: pgc
                .phase_scope
                .excluded
                .iter()
                .map(|item| item.id.clone())
                .collect(),
            deferred_scope_refs: pgc
                .phase_scope
                .deferred
                .iter()
                .map(|item| item.id.clone())
                .collect(),
            acceptance_refs: pgc
                .phase_scope
                .acceptance_candidates
                .iter()
                .map(|item| item.id.clone())
                .collect(),
        },
        planning_policy: TaskPlanPolicy {
            task_granularity: "engineering_increment".to_string(),
            group_granularity: "engineering_capability".to_string(),
            allow_task_split_during_repair: true,
            allow_task_merge_during_repair: true,
        },
        groups,
        tasks,
        engineering_quality_requirements,
        architecture_quality_requirements,
        api_contract_requirements,
        code_quality_requirements,
        browser_automation_facts,
        browser_verification_profiles,
        handoff: TaskPlanHandoff {
            ready_for_execution: true,
            next_node: "task_execution".to_string(),
            blocked_reasons: vec![],
        },
        created_at: outline.created_at.clone(),
        updated_at: now.clone(),
    };
    let mut task_plan = task_plan;
    for task in &mut task_plan.tasks {
        *task = crate::task_execution::task_with_execution_guidance(
            task.clone(),
            &aac,
            &pgc.planning_inputs.user_facing_language,
        );
    }
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let task_plan_path = task_plan_file(root, &locator, &task_plan.task_plan_id);
    state::store::write_json_atomic(&task_plan_path, &task_plan)?;
    let task_plan_ref = to_project_relative(root, &task_plan_path)?;
    state::store::write_json_atomic(
        &task_plan_latest_file(root, &locator),
        &json!({
            "schemaVersion": "1.0",
            "taskPlanId": task_plan.task_plan_id,
            "taskPlanRef": task_plan_ref,
            "updatedAt": now
        }),
    )?;

    let run = create_task_plan_run(&task_plan);
    let run_path = task_plan_run_file(root, &locator, &run.run_id);
    state::store::write_json_atomic(&run_path, &run)?;
    let run_ref = to_project_relative(root, &run_path)?;
    state::store::write_json_atomic(
        &task_plan_run_latest_file(root, &locator),
        &json!({
            "schemaVersion": "1.0",
            "taskPlanRunId": run.run_id,
            "runRef": run_ref,
            "taskPlanId": task_plan.task_plan_id,
            "updatedAt": run.updated_at
        }),
    )?;
    state::store::remove_file_if_exists(&from_project_relative(root, &outline_ref)?)?;
    for group in &outline.groups {
        let group_file = group_pattern.replace("{groupId}", &group.group_id);
        state::store::remove_file_if_exists(&from_project_relative(root, &group_file)?)?;
    }

    let store = FileTransitionStore;
    let mut status = store
        .load_status(&input.project_root)
        .map_err(to_state_error)?;
    let mut delivery = store
        .load_delivery_index(&input.project_root, &delivery_id)
        .map_err(to_state_error)?;
    let next_action = RouteAction {
        kind: RouteActionKind::ContinueExecution,
        source: mode.next_source().to_string(),
        reason: mode.next_reason().to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(input.request_ref.clone()),
        details: None,
        target_phase_id: None,
    };
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        phase
            .latest_refs
            .insert("taskPlan".to_string(), task_plan_ref.clone());
        phase.latest_refs.insert("taskPlanRun".to_string(), run_ref);
        phase.next_action = Some(next_action.clone());
    }
    delivery.status = DeliveryLifecycleStatus::Executing;
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(&input.project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(&input.project_root, &status)
        .map_err(to_state_error)?;

    let engine = TransitionEngine {
        store: FileTransitionStore,
        dispatcher,
    };
    engine
        .advance_after_submit(
            OperationContext {
                project_root: input.project_root.clone(),
            },
            SubmitAcceptedEvent {
                delivery_id,
                phase_id,
                source_tool: mode.resubmit_tool().to_string(),
                accepted_artifact_ref: task_plan_ref,
                next_action: Some(next_action),
            },
        )
        .map_err(to_state_error)
}

fn write_taskplan_result(
    project_root: &str,
    request_ref: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })?;
    let submit_tool = inspected.submit_tool.ok_or_else(|| {
        state::store::StateError::InvalidArgument("TaskPlan request missing submitTool".to_string())
    })?;
    let write_targets = inspected
        .write_targets
        .iter()
        .map(value_to_write_target)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::WriteArtifact(delivery_core::WriteArtifactNext {
                artifact_kind: ArtifactKind::TaskPlanCandidate,
                request_ref: request_ref.to_string(),
                write_mode: delivery_core::WriteMode::TaskplanGrouped,
                write_targets,
                read_groups: inspected.read_groups,
                submit_tool,
            }),
        ),
    ))
}

fn create_task_plan_run(task_plan: &TaskPlan) -> TaskPlanRun {
    let now = state::store::now_string();
    let mut run = TaskPlanRun {
        schema_version: "1.0".to_string(),
        run_id: format!(
            "run_{}_{}",
            task_plan.source.phase_id,
            state::store::now_millis()
        ),
        task_plan_id: task_plan.task_plan_id.clone(),
        status: TaskPlanRunStatus::NotStarted,
        scheduler: TaskPlanRunScheduler {
            mode: "group_dag".to_string(),
            started_at: None,
            finished_at: None,
        },
        group_states: task_plan
            .groups
            .iter()
            .map(|group| TaskGroupRunState {
                group_id: group.group_id.clone(),
                status: TaskRunStatus::Pending,
                started_at: None,
                finished_at: None,
                depends_on: group.depends_on.clone(),
                task_ids: group.task_ids.clone(),
            })
            .collect(),
        task_states: task_plan
            .tasks
            .iter()
            .map(|task| TaskRunState {
                task_id: task.task_id.clone(),
                group_id: Some(task.group_id.clone()),
                status: TaskRunStatus::Pending,
                result_id: None,
                started_at: None,
                finished_at: None,
                depends_on: task.depends_on.clone(),
                attempts: vec![],
            })
            .collect(),
        summary: TaskPlanRunSummary::default(),
        next_action: Some(TaskPlanRunNextAction {
            r#type: "continue_execution".to_string(),
            reason: "TASKPLAN_READY".to_string(),
            source_task_id: None,
            target_node: "task_execution".to_string(),
        }),
        created_at: now.clone(),
        updated_at: now,
    };
    update_run_summary(&mut run);
    run
}

pub(crate) fn update_run_summary(run: &mut TaskPlanRun) {
    let mut summary = TaskPlanRunSummary {
        total: run.task_states.len() as u32,
        ..TaskPlanRunSummary::default()
    };
    for task in &run.task_states {
        match task.status {
            TaskRunStatus::Pending => summary.pending += 1,
            TaskRunStatus::Running => summary.running += 1,
            TaskRunStatus::Completed => summary.completed += 1,
            TaskRunStatus::CompletedWithNotes => summary.completed_with_notes += 1,
            TaskRunStatus::Blocked => summary.blocked += 1,
            TaskRunStatus::Failed => summary.failed += 1,
        }
    }
    run.summary = summary;
}

fn normalize_taskplan_outline_envelope(
    raw: &mut Value,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) {
    let Some(object) = raw.as_object_mut() else {
        return;
    };
    object.insert("schemaVersion".to_string(), json!("1.0"));
    object.insert("requestId".to_string(), json!(request_id));
    object.insert("deliveryId".to_string(), json!(delivery_id));
    object.insert("phaseId".to_string(), json!(phase_id));
    object.insert(
        "taskPlanId".to_string(),
        json!(format!("taskplan-{phase_id}")),
    );
    object.insert("createdAt".to_string(), json!(state::store::now_string()));
}

fn normalize_taskplan_group_envelope(
    raw: &mut Value,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) {
    let Some(object) = raw.as_object_mut() else {
        return;
    };
    object.insert("schemaVersion".to_string(), json!("1.0"));
    object.insert("requestId".to_string(), json!(request_id));
    object.insert("deliveryId".to_string(), json!(delivery_id));
    object.insert("phaseId".to_string(), json!(phase_id));
    object.insert("createdAt".to_string(), json!(state::store::now_string()));
}

fn validate_outline(
    outline: &TaskPlanOutlineCandidateAgentWritable,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if outline.groups.is_empty() {
        issues.push(issue(
            "GROUPS_REQUIRED",
            "outline.groups",
            "TaskPlan outline must include at least one group.",
            Some("outline"),
        ));
    }
    issues
}

fn validate_group_candidate(
    candidate: &TaskPlanGroupCandidateAgentWritable,
    expected: &TaskPlanGroup,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let target = Some(candidate.group.group_id.as_str());
    if candidate.group.group_id != expected.group_id {
        issues.push(issue(
            "GROUP_ID_MISMATCH",
            "group.groupId",
            "TaskPlan group candidate must match outline groupId.",
            target,
        ));
    }
    if candidate.status != "ready" {
        issues.push(issue(
            "GROUP_CANDIDATE_NOT_READY",
            "group.status",
            "TaskPlan group candidate status must be ready.",
            target,
        ));
    }
    let task_ids = candidate
        .tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    if task_ids != expected.task_ids {
        issues.push(issue(
            "TASK_IDS_MISMATCH",
            "group.taskIds",
            "TaskPlan group taskIds must equal the task ids in its group file.",
            target,
        ));
    }
    for task in &candidate.tasks {
        if task.group_id != candidate.group.group_id {
            issues.push(issue(
                "TASK_GROUP_ID_MISMATCH",
                "tasks[].groupId",
                "Each task.groupId must equal its group.groupId.",
                target,
            ));
        }
        if !task
            .write_boundary
            .forbidden_paths
            .iter()
            .any(|path| path == ".loom")
        {
            issues.push(issue(
                "WRITE_BOUNDARY_MISSING_LOOM",
                "tasks[].writeBoundary.forbiddenPaths",
                "Each task must protect .loom from source edits.",
                target,
            ));
        }
        if task.verification_intents.is_empty() {
            issues.push(issue(
                "VERIFICATION_INTENTS_REQUIRED",
                "tasks[].verificationIntents",
                "Each task must include verification intents.",
                target,
            ));
        }
        if !task.implementation_obligations.is_empty() {
            issues.push(issue(
                "IMPLEMENTATION_OBLIGATIONS_MCP_OWNED",
                "tasks[].implementationObligations",
                "TaskPlan candidates must not author implementationObligations; Loom derives them from accepted contracts and task ownership during acceptance.",
                target,
            ));
        }
    }
    issues
}

fn validate_taskplan_graph(
    groups: &[TaskPlanGroup],
    tasks: &[TaskDefinition],
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let group_ids = groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect::<BTreeSet<_>>();
    for group in groups {
        for dep in &group.depends_on {
            if !group_ids.contains(dep) {
                issues.push(issue(
                    "UNKNOWN_GROUP_DEPENDENCY",
                    "groups[].dependsOn",
                    "Group dependency must reference an existing group.",
                    Some(&group.group_id),
                ));
            }
        }
    }
    if has_group_cycle(groups) {
        issues.push(issue(
            "GROUP_DEPENDENCY_CYCLE",
            "groups[].dependsOn",
            "Group dependencies must not contain a cycle.",
            None,
        ));
    }
    let mut seen_tasks = BTreeSet::new();
    for task in tasks {
        if !seen_tasks.insert(task.task_id.clone()) {
            issues.push(issue(
                "DUPLICATE_TASK_ID",
                "tasks[].taskId",
                "Task ids must be unique.",
                Some(&task.task_id),
            ));
        }
        if !group_ids.contains(&task.group_id) {
            issues.push(issue(
                "UNKNOWN_TASK_GROUP",
                "tasks[].groupId",
                "Task groupId must exist in outline groups.",
                Some(&task.task_id),
            ));
        }
    }
    let task_ids = tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<BTreeSet<_>>();
    for task in tasks {
        for dep in &task.depends_on {
            if !task_ids.contains(dep) {
                issues.push(issue(
                    "UNKNOWN_TASK_DEPENDENCY",
                    "tasks[].dependsOn",
                    "Task dependency must reference an existing task.",
                    Some(&task.task_id),
                ));
            }
        }
    }
    if has_cycle(tasks) {
        issues.push(issue(
            "TASK_DEPENDENCY_CYCLE",
            "tasks[].dependsOn",
            "Task dependencies must not contain a cycle.",
            None,
        ));
    }
    issues
}

fn validate_taskplan_refs(
    groups: &[TaskPlanGroup],
    tasks: &[TaskDefinition],
    allowed_refs: &Value,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let scope_refs = allowed_set(allowed_refs, "scopeRefs");
    let acceptance_refs = allowed_set(allowed_refs, "acceptanceRefs");
    let detail_refs = allowed_set(allowed_refs, "requirementDetailIds");
    for group in groups {
        validate_ref_list(
            &group.scope_refs,
            &scope_refs,
            "UNKNOWN_SCOPE_REF",
            "groups[].scopeRefs",
            &group.group_id,
            &mut issues,
        );
        validate_ref_list(
            &group.acceptance_refs,
            &acceptance_refs,
            "UNKNOWN_ACCEPTANCE_REF",
            "groups[].acceptanceRefs",
            &group.group_id,
            &mut issues,
        );
    }
    for task in tasks {
        validate_ref_list(
            &task.scope_refs,
            &scope_refs,
            "UNKNOWN_SCOPE_REF",
            "tasks[].scopeRefs",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.acceptance_refs,
            &acceptance_refs,
            "UNKNOWN_ACCEPTANCE_REF",
            "tasks[].acceptanceRefs",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.requirement_detail_refs,
            &detail_refs,
            "UNKNOWN_REQUIREMENT_DETAIL_REF",
            "tasks[].requirementDetailRefs",
            &task.task_id,
            &mut issues,
        );
        for intent in &task.verification_intents {
            validate_ref_list(
                &intent.acceptance_refs,
                &acceptance_refs,
                "UNKNOWN_ACCEPTANCE_REF",
                "tasks[].verificationIntents[].acceptanceRefs",
                &task.task_id,
                &mut issues,
            );
            validate_ref_list(
                &intent.requirement_detail_refs,
                &detail_refs,
                "UNKNOWN_REQUIREMENT_DETAIL_REF",
                "tasks[].verificationIntents[].requirementDetailRefs",
                &task.task_id,
                &mut issues,
            );
            for detail in &intent.requirement_detail_refs {
                if !task.requirement_detail_refs.contains(detail) {
                    issues.push(issue(
                        "VERIFICATION_DETAIL_NOT_ON_TASK",
                        "tasks[].verificationIntents[].requirementDetailRefs",
                        "verificationIntents[].requirementDetailRefs must be a subset of the parent task.requirementDetailRefs.",
                        Some(&task.task_id),
                    ));
                }
            }
        }
        validate_ref_list(
            &task.write_boundary.artifact_refs.modules,
            &allowed_set(allowed_refs, "moduleRefs"),
            "UNKNOWN_MODULE_REF",
            "tasks[].writeBoundary.artifactRefs.modules",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.entities,
            &allowed_set(allowed_refs, "entityRefs"),
            "UNKNOWN_ENTITY_REF",
            "tasks[].writeBoundary.artifactRefs.entities",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.interfaces,
            &allowed_set(allowed_refs, "interfaceRefs"),
            "UNKNOWN_INTERFACE_REF",
            "tasks[].writeBoundary.artifactRefs.interfaces",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.consumed_interfaces,
            &allowed_set(allowed_refs, "interfaceRefs"),
            "UNKNOWN_INTERFACE_REF",
            "tasks[].writeBoundary.artifactRefs.consumedInterfaces",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.user_flows,
            &allowed_set(allowed_refs, "userFlowRefs"),
            "UNKNOWN_USER_FLOW_REF",
            "tasks[].writeBoundary.artifactRefs.userFlows",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.state_machines,
            &allowed_set(allowed_refs, "stateMachineRefs"),
            "UNKNOWN_STATE_MACHINE_REF",
            "tasks[].writeBoundary.artifactRefs.stateMachines",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.decisions,
            &allowed_set(allowed_refs, "decisionRefs"),
            "UNKNOWN_DECISION_REF",
            "tasks[].writeBoundary.artifactRefs.decisions",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.nfrs,
            &allowed_set(allowed_refs, "nfrRefs"),
            "UNKNOWN_NFR_REF",
            "tasks[].writeBoundary.artifactRefs.nfrs",
            &task.task_id,
            &mut issues,
        );
        validate_ref_list(
            &task.write_boundary.artifact_refs.risks,
            &allowed_set(allowed_refs, "riskRefs"),
            "UNKNOWN_RISK_REF",
            "tasks[].writeBoundary.artifactRefs.risks",
            &task.task_id,
            &mut issues,
        );
    }
    issues
}

fn normalize_taskplan_write_boundaries(tasks: &mut [TaskDefinition], allowed_refs: &Value) {
    let module_refs = allowed_set(allowed_refs, "moduleRefs");
    let entity_refs = allowed_set(allowed_refs, "entityRefs");
    let interface_refs = allowed_set(allowed_refs, "interfaceRefs");
    let user_flow_refs = allowed_set(allowed_refs, "userFlowRefs");
    let state_machine_refs = allowed_set(allowed_refs, "stateMachineRefs");
    let decision_refs = allowed_set(allowed_refs, "decisionRefs");
    let nfr_refs = allowed_set(allowed_refs, "nfrRefs");
    let risk_refs = allowed_set(allowed_refs, "riskRefs");
    for task in tasks {
        task.write_boundary.forbidden_paths = vec![".loom".to_string()];
        clear_refs_when_unavailable(&mut task.write_boundary.artifact_refs.modules, &module_refs);
        clear_refs_when_unavailable(
            &mut task.write_boundary.artifact_refs.entities,
            &entity_refs,
        );
        clear_refs_when_unavailable(
            &mut task.write_boundary.artifact_refs.interfaces,
            &interface_refs,
        );
        clear_refs_when_unavailable(
            &mut task.write_boundary.artifact_refs.consumed_interfaces,
            &interface_refs,
        );
        clear_refs_when_unavailable(
            &mut task.write_boundary.artifact_refs.user_flows,
            &user_flow_refs,
        );
        clear_refs_when_unavailable(
            &mut task.write_boundary.artifact_refs.state_machines,
            &state_machine_refs,
        );
        clear_refs_when_unavailable(
            &mut task.write_boundary.artifact_refs.decisions,
            &decision_refs,
        );
        clear_refs_when_unavailable(&mut task.write_boundary.artifact_refs.nfrs, &nfr_refs);
        clear_refs_when_unavailable(&mut task.write_boundary.artifact_refs.risks, &risk_refs);
    }
}

fn clear_refs_when_unavailable(refs: &mut Vec<String>, allowed: &BTreeSet<String>) {
    if allowed.is_empty() {
        refs.clear();
    }
}

fn normalize_taskplan_candidate_relationships(
    groups: &mut Vec<TaskPlanGroup>,
    tasks: &mut Vec<TaskDefinition>,
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) {
    normalize_runtime_delivery_closure_group(groups, tasks, aac.runtime_delivery.as_ref());
    canonicalize_task_ownership(groups, tasks, pgc, aac);
    normalize_frontend_experience_requirements(tasks, aac);
}

fn canonicalize_task_ownership(
    groups: &mut [TaskPlanGroup],
    tasks: &mut [TaskDefinition],
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) {
    let included_scope_refs = pgc
        .phase_scope
        .included
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let deferred_or_excluded = pgc
        .phase_scope
        .deferred
        .iter()
        .chain(pgc.phase_scope.excluded.iter())
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let covered = aac
        .detail_coverage
        .iter()
        .filter(|entry| matches!(entry.coverage_status, CoverageStatus::Covered))
        .map(|entry| (entry.detail_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();

    for task in tasks.iter_mut() {
        task.requirement_detail_refs.clear();
        for intent in &mut task.verification_intents {
            intent.requirement_detail_refs.clear();
        }
    }
    for task in tasks.iter_mut().filter(|task| is_business_owner_task(task)) {
        task.scope_refs.clear();
        task.acceptance_refs.clear();
    }
    for task in tasks
        .iter_mut()
        .filter(|task| !is_business_owner_task(task))
    {
        let consumed_interfaces = if task_can_consume_api_contract(task) {
            task.write_boundary.artifact_refs.all_interfaces()
        } else {
            Vec::new()
        };
        task.scope_refs.clear();
        task.acceptance_refs.clear();
        task.requirement_detail_refs.clear();
        task.write_boundary.artifact_refs = TaskArtifactRefs::default();
        task.write_boundary.artifact_refs.consumed_interfaces = consumed_interfaces;
        for intent in &mut task.verification_intents {
            intent.requirement_detail_refs.clear();
        }
    }

    for detail in pgc.requirement_details.items.iter().filter(|detail| {
        detail.required_for_current_phase
            && detail.scope_refs.iter().all(|scope| {
                included_scope_refs.contains(scope.as_str())
                    && !deferred_or_excluded.contains(scope.as_str())
            })
            && covered.contains_key(detail.detail_id.as_str())
    }) {
        let Some(coverage) = covered.get(detail.detail_id.as_str()) else {
            continue;
        };
        let Some(owner_index) = choose_unique_owner(tasks, detail, coverage) else {
            continue;
        };
        let owner = &mut tasks[owner_index];
        push_unique(&mut owner.requirement_detail_refs, detail.detail_id.clone());
        merge_detail_artifact_refs(
            &mut owner.write_boundary.artifact_refs,
            &coverage.artifact_refs,
        );
        for scope_ref in &detail.scope_refs {
            push_unique(&mut owner.scope_refs, scope_ref.clone());
        }
        for acceptance_ref in &detail.acceptance_refs {
            push_unique(&mut owner.acceptance_refs, acceptance_ref.clone());
        }
        if let Some(intent) = owner.verification_intents.first_mut() {
            push_unique(
                &mut intent.requirement_detail_refs,
                detail.detail_id.clone(),
            );
            for acceptance_ref in &detail.acceptance_refs {
                push_unique(&mut intent.acceptance_refs, acceptance_ref.clone());
            }
        }
    }

    canonicalize_acceptance_owners(tasks, pgc);
    normalize_artifact_refs(tasks, aac);
    canonicalize_workflow_closure_owners(tasks, aac);
    canonicalize_group_ownership(groups, tasks);
}

fn canonicalize_group_ownership(groups: &mut [TaskPlanGroup], tasks: &[TaskDefinition]) {
    for group in groups {
        let group_tasks = tasks
            .iter()
            .filter(|task| task.group_id == group.group_id)
            .collect::<Vec<_>>();
        group.scope_refs = group_tasks
            .iter()
            .filter(|task| is_business_owner_task(task))
            .flat_map(|task| task.scope_refs.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        group.acceptance_refs = group_tasks
            .iter()
            .filter(|task| is_business_owner_task(task))
            .flat_map(|task| task.acceptance_refs.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
}

fn canonicalize_acceptance_owners(
    tasks: &mut [TaskDefinition],
    pgc: &contracts::PlanningGenerationContract,
) {
    let candidates = &pgc.phase_scope.acceptance_candidates;
    if candidates.is_empty() {
        return;
    }

    let detail_acceptance_owners = tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| is_business_owner_task(task))
        .flat_map(|(index, task)| {
            task.requirement_detail_refs
                .iter()
                .flat_map(move |detail_id| {
                    pgc.requirement_details
                        .items
                        .iter()
                        .find(|detail| detail.detail_id == *detail_id)
                        .into_iter()
                        .flat_map(|detail| detail.acceptance_refs.iter())
                        .map(move |acceptance_id| (acceptance_id.clone(), index))
                })
        })
        .collect::<BTreeMap<_, _>>();

    for acceptance in candidates {
        let owner_index = detail_acceptance_owners
            .get(&acceptance.id)
            .copied()
            .or_else(|| choose_acceptance_owner(tasks, acceptance));
        let Some(owner_index) = owner_index else {
            continue;
        };
        for task in tasks.iter_mut() {
            task.acceptance_refs
                .retain(|reference| reference != &acceptance.id);
            for intent in &mut task.verification_intents {
                intent
                    .acceptance_refs
                    .retain(|reference| reference != &acceptance.id);
            }
        }
        let owner = &mut tasks[owner_index];
        push_unique(&mut owner.acceptance_refs, acceptance.id.clone());
        if let Some(intent) = owner.verification_intents.first_mut() {
            push_unique(&mut intent.acceptance_refs, acceptance.id.clone());
        }
    }
}

fn choose_acceptance_owner(
    tasks: &[TaskDefinition],
    acceptance: &contracts::AcceptanceCandidate,
) -> Option<usize> {
    let source_refs = acceptance
        .source_refs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let capability_refs = acceptance
        .capability_refs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut candidates = tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| is_business_owner_task(task))
        .map(|(index, task)| {
            let mut score = task_owner_rank(task);
            score += task
                .scope_refs
                .iter()
                .filter(|reference| source_refs.contains(reference.as_str()))
                .count() as u32
                * 20;
            score += task
                .write_boundary
                .artifact_refs
                .modules
                .iter()
                .filter(|reference| capability_refs.contains(reference.as_str()))
                .count() as u32
                * 20;
            score += task
                .write_boundary
                .artifact_refs
                .user_flows
                .iter()
                .filter(|reference| source_refs.contains(reference.as_str()))
                .count() as u32
                * 15;
            (index, score, task.task_id.clone())
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.2.cmp(&right.2)));
    candidates.first().map(|candidate| candidate.0)
}

fn is_business_owner_task(task: &TaskDefinition) -> bool {
    !matches!(
        task.task_kind,
        TaskKind::VerificationIncrement
            | TaskKind::RuntimeDeliveryClosure
            | TaskKind::BrowserQualityClosure
    )
}

fn merge_detail_artifact_refs(
    task_refs: &mut TaskArtifactRefs,
    detail_refs: &contracts::architecture::DetailCoverageArtifactRefs,
) {
    for reference in &detail_refs.modules {
        push_unique(&mut task_refs.modules, reference.clone());
    }
    for reference in &detail_refs.entities {
        push_unique(&mut task_refs.entities, reference.clone());
    }
    for reference in &detail_refs.interfaces {
        push_unique(&mut task_refs.interfaces, reference.clone());
    }
    for reference in &detail_refs.user_flows {
        push_unique(&mut task_refs.user_flows, reference.clone());
    }
    for reference in &detail_refs.state_machines {
        push_unique(&mut task_refs.state_machines, reference.clone());
    }
}

fn choose_unique_owner(
    tasks: &[TaskDefinition],
    detail: &RequirementDetailItem,
    coverage: &ArchitectureDetailCoverageEntry,
) -> Option<usize> {
    let mut candidates = tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| is_business_owner_task(task))
        .map(|(index, task)| {
            (
                index,
                requirement_detail_owner_score(task, detail, coverage),
                task_owner_rank(task),
                task.task_id.clone(),
            )
        })
        .filter(|(_, score, _, _)| *score > 0)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| task_can_own_requirement_detail(task))
            .max_by(|left, right| {
                task_owner_rank(left.1)
                    .cmp(&task_owner_rank(right.1))
                    .then_with(|| right.1.task_id.cmp(&left.1.task_id))
            })
            .map(|(index, _)| index);
    }
    candidates.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.3.cmp(&right.3))
    });
    candidates.first().map(|candidate| candidate.0)
}

fn task_owner_rank(task: &TaskDefinition) -> u32 {
    let kind_rank = match task.task_kind {
        TaskKind::DataModelIncrement => 100,
        TaskKind::InterfaceIncrement => 95,
        TaskKind::FeatureIncrement => 90,
        TaskKind::UiFlowIncrement | TaskKind::FrontendExperience => 85,
        TaskKind::IntegrationIncrement => 75,
        TaskKind::RefactorSupport => 40,
        TaskKind::ConfigurationSupport => 30,
        _ => 10,
    };
    let action_rank = task
        .implementation_actions
        .iter()
        .map(|action| match action {
            ImplementationAction::CreateOrUpdateEntity
            | ImplementationAction::CreateOrUpdatePersistence
            | ImplementationAction::CreateOrUpdatePersistenceQuery
            | ImplementationAction::CreateEntityRepository => 20,
            ImplementationAction::CreateOrUpdateInterface
            | ImplementationAction::CreateEntityCrud => 18,
            ImplementationAction::CreateOrUpdateUiFlow
            | ImplementationAction::ImplementFrontendExperienceContract => 16,
            _ => 0,
        })
        .max()
        .unwrap_or(0);
    kind_rank + action_rank
}

fn normalize_artifact_refs(tasks: &mut [TaskDefinition], aac: &ArchitectureArtifactContract) {
    let allowed = accepted_artifact_ref_sets(aac);
    for field in [
        "modules",
        "entities",
        "interfaces",
        "user_flows",
        "state_machines",
        "decisions",
        "nfrs",
        "risks",
    ] {
        let occurrences = tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| is_business_owner_task(task))
            .flat_map(|(index, task)| {
                artifact_refs_for_field(&task.write_boundary.artifact_refs, field)
                    .iter()
                    .cloned()
                    .map(move |artifact_ref| (artifact_ref, index))
            })
            .fold(
                BTreeMap::<String, Vec<usize>>::new(),
                |mut map, (artifact_ref, index)| {
                    map.entry(artifact_ref).or_default().push(index);
                    map
                },
            );
        let duplicate_owners = occurrences
            .iter()
            .filter(|(_, indices)| indices.len() > 1)
            .map(|(artifact_ref, indices)| {
                (
                    artifact_ref.clone(),
                    choose_artifact_owner(artifact_ref, field, indices, tasks, aac),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for task in tasks.iter_mut() {
            if !is_business_owner_task(task) {
                continue;
            }
            let consumed = {
                let refs =
                    artifact_refs_for_field_mut(&mut task.write_boundary.artifact_refs, field);
                let original = refs.clone();
                refs.retain(|artifact_ref| {
                    allowed
                        .get(field)
                        .is_some_and(|accepted| accepted.contains(artifact_ref))
                        && duplicate_owners
                            .get(artifact_ref)
                            .is_none_or(|owner| owner == &task.task_id)
                });
                if field == "interfaces" {
                    original
                        .into_iter()
                        .filter(|interface_ref| {
                            !refs.contains(interface_ref)
                                && allowed
                                    .get("interfaces")
                                    .is_some_and(|accepted| accepted.contains(interface_ref))
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            };
            for interface_ref in consumed {
                push_unique(
                    &mut task.write_boundary.artifact_refs.consumed_interfaces,
                    interface_ref,
                );
            }
        }
        if field == "interfaces" {
            for task in tasks.iter_mut().filter(|task| is_business_owner_task(task)) {
                if !task_owns_api_contract(task) {
                    let non_owner_refs =
                        std::mem::take(&mut task.write_boundary.artifact_refs.interfaces);
                    for interface_ref in non_owner_refs {
                        push_unique(
                            &mut task.write_boundary.artifact_refs.consumed_interfaces,
                            interface_ref,
                        );
                    }
                }
                task.write_boundary
                    .artifact_refs
                    .consumed_interfaces
                    .retain(|reference| {
                        !task
                            .write_boundary
                            .artifact_refs
                            .interfaces
                            .contains(reference)
                    });
            }
        }
    }
}

fn choose_artifact_owner(
    artifact_ref: &str,
    field: &str,
    candidate_indices: &[usize],
    tasks: &[TaskDefinition],
    aac: &ArchitectureArtifactContract,
) -> String {
    let eligible_indices = if field == "interfaces" {
        let api_owners = candidate_indices
            .iter()
            .copied()
            .filter(|index| task_owns_api_contract(&tasks[*index]))
            .collect::<Vec<_>>();
        if api_owners.is_empty() {
            candidate_indices.to_vec()
        } else {
            api_owners
        }
    } else {
        candidate_indices.to_vec()
    };
    let workflow_owners = eligible_indices
        .iter()
        .copied()
        .filter(|index| {
            task_is_workflow_closure_artifact_owner(&tasks[*index], field, artifact_ref, aac)
        })
        .collect::<Vec<_>>();
    let detail_owners = eligible_indices
        .iter()
        .copied()
        .filter(|index| {
            tasks[*index]
                .requirement_detail_refs
                .iter()
                .any(|detail_id| {
                    aac.detail_coverage
                        .iter()
                        .find(|coverage| coverage.detail_id == *detail_id)
                        .is_some_and(|coverage| {
                            coverage_status_is_covered(coverage)
                                && coverage_artifact_refs_contain(
                                    &coverage.artifact_refs,
                                    field,
                                    artifact_ref,
                                )
                        })
                })
        })
        .collect::<Vec<_>>();
    let candidates = if !workflow_owners.is_empty() {
        workflow_owners
    } else if detail_owners.len() == 1 {
        detail_owners
    } else {
        eligible_indices
    };
    candidates
        .into_iter()
        .max_by(|left, right| {
            task_owner_rank(&tasks[*left])
                .cmp(&task_owner_rank(&tasks[*right]))
                .then_with(|| tasks[*right].task_id.cmp(&tasks[*left].task_id))
        })
        .map(|index| tasks[index].task_id.clone())
        .unwrap_or_default()
}

fn task_is_workflow_closure_artifact_owner(
    task: &TaskDefinition,
    field: &str,
    artifact_ref: &str,
    aac: &ArchitectureArtifactContract,
) -> bool {
    if !task.frontend_experience_requirement.is_some()
        || !task
            .implementation_actions
            .iter()
            .any(|action| matches!(action, ImplementationAction::WireReferenceInApiOrUi))
    {
        return false;
    }
    workflow_closure_requirements(aac)
        .iter()
        .any(|requirement| {
            let workflow_matches = field == "user_flows"
                && requirement.get("workflowRef").and_then(Value::as_str) == Some(artifact_ref);
            let interface_matches = field == "interfaces"
                && string_array_at(requirement, "interfaceRefs")
                    .iter()
                    .any(|reference| reference == artifact_ref);
            (workflow_matches || interface_matches)
                && requirement
                    .get("workflowRef")
                    .and_then(Value::as_str)
                    .is_some_and(|workflow_ref| {
                        task.write_boundary
                            .artifact_refs
                            .user_flows
                            .iter()
                            .any(|reference| reference == workflow_ref)
                    })
        })
}

fn coverage_status_is_covered(coverage: &ArchitectureDetailCoverageEntry) -> bool {
    matches!(coverage.coverage_status, CoverageStatus::Covered)
}

fn coverage_artifact_refs_contain(
    refs: &contracts::architecture::DetailCoverageArtifactRefs,
    field: &str,
    artifact_ref: &str,
) -> bool {
    match field {
        "modules" => refs.modules.as_slice(),
        "entities" => refs.entities.as_slice(),
        "interfaces" => refs.interfaces.as_slice(),
        "user_flows" => refs.user_flows.as_slice(),
        "state_machines" => refs.state_machines.as_slice(),
        _ => &[],
    }
    .iter()
    .any(|reference| reference == artifact_ref)
}

fn artifact_refs_for_field<'a>(refs: &'a TaskArtifactRefs, field: &str) -> &'a [String] {
    match field {
        "modules" => refs.modules.as_slice(),
        "entities" => refs.entities.as_slice(),
        "interfaces" => refs.interfaces.as_slice(),
        "user_flows" => refs.user_flows.as_slice(),
        "state_machines" => refs.state_machines.as_slice(),
        "decisions" => refs.decisions.as_slice(),
        "nfrs" => refs.nfrs.as_slice(),
        "risks" => refs.risks.as_slice(),
        _ => &[],
    }
}

fn canonicalize_workflow_closure_owners(
    tasks: &mut [TaskDefinition],
    aac: &ArchitectureArtifactContract,
) {
    for requirement in workflow_closure_requirements(aac) {
        let Some(workflow_ref) = requirement.get("workflowRef").and_then(Value::as_str) else {
            continue;
        };
        let required_interfaces = string_array_at(&requirement, "interfaceRefs");
        let required_acceptance = string_array_at(&requirement, "acceptanceRefs");
        let mut candidates = tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                is_business_owner_task(task)
                    && task.frontend_experience_requirement.is_some()
                    && task
                        .write_boundary
                        .artifact_refs
                        .user_flows
                        .iter()
                        .any(|reference| reference == workflow_ref)
                    && task.implementation_actions.iter().any(|action| {
                        matches!(action, ImplementationAction::WireReferenceInApiOrUi)
                    })
            })
            .map(|(index, task)| {
                let acceptance_match =
                    intersection_count(&task.acceptance_refs, &required_acceptance) as u32;
                let intent_match = task.verification_intents.iter().any(|intent| {
                    required_acceptance.iter().all(|acceptance_ref| {
                        intent
                            .acceptance_refs
                            .iter()
                            .any(|reference| reference == acceptance_ref)
                    })
                }) as u32;
                (
                    index,
                    task_owner_rank(task) + acceptance_match * 10 + intent_match * 20,
                    task.task_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.2.cmp(&right.2)));
        let Some((owner_index, _, _)) = candidates.first() else {
            continue;
        };
        let owner = &mut tasks[*owner_index];
        push_unique(
            &mut owner.write_boundary.artifact_refs.user_flows,
            workflow_ref.to_string(),
        );
        for interface_ref in required_interfaces {
            push_unique(
                &mut owner.write_boundary.artifact_refs.consumed_interfaces,
                interface_ref,
            );
        }
        for acceptance_ref in required_acceptance {
            push_unique(&mut owner.acceptance_refs, acceptance_ref.clone());
            if let Some(intent) = owner.verification_intents.first_mut() {
                push_unique(&mut intent.acceptance_refs, acceptance_ref);
            }
        }
    }
}

fn accepted_artifact_ref_sets(
    aac: &ArchitectureArtifactContract,
) -> BTreeMap<&'static str, BTreeSet<String>> {
    let mut sets = BTreeMap::new();
    sets.insert(
        "modules",
        aac.modules
            .iter()
            .filter_map(|value| value_id(value, "moduleId"))
            .collect(),
    );
    sets.insert(
        "entities",
        aac.data_model
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value_id(value, "entityId"))
            .collect(),
    );
    sets.insert(
        "interfaces",
        aac.interfaces
            .iter()
            .filter_map(|value| value_id(value, "interfaceId"))
            .collect(),
    );
    sets.insert(
        "user_flows",
        aac.user_flows
            .iter()
            .filter_map(|value| value_id(value, "flowId"))
            .collect(),
    );
    sets.insert(
        "state_machines",
        aac.state_machines
            .iter()
            .filter_map(|value| value_id(value, "stateMachineId"))
            .collect(),
    );
    sets.insert(
        "decisions",
        aac.architecture_quality
            .decisions
            .iter()
            .map(|value| value.decision_id.clone())
            .collect(),
    );
    sets.insert(
        "nfrs",
        aac.architecture_quality
            .nfrs
            .iter()
            .map(|value| value.nfr_id.clone())
            .collect(),
    );
    sets.insert(
        "risks",
        aac.architecture_quality
            .risks
            .iter()
            .map(|value| value.risk_id.clone())
            .collect(),
    );
    sets
}

fn value_id(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn artifact_refs_for_field_mut<'a>(
    refs: &'a mut TaskArtifactRefs,
    field: &str,
) -> &'a mut Vec<String> {
    match field {
        "modules" => &mut refs.modules,
        "entities" => &mut refs.entities,
        "interfaces" => &mut refs.interfaces,
        "user_flows" => &mut refs.user_flows,
        "state_machines" => &mut refs.state_machines,
        "decisions" => &mut refs.decisions,
        "nfrs" => &mut refs.nfrs,
        "risks" => &mut refs.risks,
        _ => &mut refs.modules,
    }
}

fn normalize_runtime_delivery_requirements(
    tasks: &mut [TaskDefinition],
    aac: &ArchitectureArtifactContract,
) {
    let Some(runtime_delivery) = aac.runtime_delivery.as_ref() else {
        for task in tasks {
            task.runtime_delivery_requirement = None;
        }
        return;
    };
    let runtime_ref = "sourceRefs.architectureArtifactContractRef#/runtimeDelivery";
    let contract_fields = runtime_delivery_closure_fields(runtime_delivery);
    let contract_field_set = contract_fields.iter().cloned().collect::<BTreeSet<_>>();

    for task in tasks.iter_mut() {
        let is_closure = matches!(task.task_kind, TaskKind::RuntimeDeliveryClosure);
        // The closure contract is derived from accepted RuntimeDelivery facts. It
        // must not depend on the agent copying a second machine-owned object into
        // the TaskPlan candidate; a missing closure field is therefore materialized
        // before validation instead of becoming a repair loop.
        if is_closure && task.runtime_delivery_requirement.is_none() {
            task.runtime_delivery_requirement = Some(contracts::TaskRuntimeDeliveryRequirement {
                applies_to_this_task: true,
                reason: "Final code-level closure for the RuntimeDeliveryContract.".to_string(),
                runtime_delivery_ref: Some(runtime_ref.to_string()),
                affected_contract_fields: contract_fields.clone(),
                required_code_level_checks: contract_fields
                    .iter()
                    .map(|field| runtime_delivery_check_for_field(field))
                    .collect(),
                evidence_expected_in_task_result: Vec::new(),
                forbidden_actions: Vec::new(),
                source: Some("mcp_derived_from_runtime_delivery".to_string()),
                deployment_failure_ref: None,
            });
        }
        let Some(requirement) = task.runtime_delivery_requirement.as_mut() else {
            continue;
        };
        if is_closure {
            requirement.applies_to_this_task = true;
            requirement.reason =
                "Final code-level closure for the accepted RuntimeDeliveryContract.".to_string();
            requirement.runtime_delivery_ref = Some(runtime_ref.to_string());
            requirement.affected_contract_fields = contract_fields.clone();
            requirement.required_code_level_checks = contract_fields
                .iter()
                .map(|field| runtime_delivery_check_for_field(field))
                .collect();
            requirement.evidence_expected_in_task_result.clear();
            requirement.forbidden_actions.clear();
            requirement.source = Some("mcp_derived_from_runtime_delivery".to_string());
            requirement.deployment_failure_ref = None;
            continue;
        }
        if !is_closure && !requirement.applies_to_this_task {
            requirement.runtime_delivery_ref = None;
            requirement.affected_contract_fields.clear();
            requirement.required_code_level_checks.clear();
            continue;
        }
        requirement.runtime_delivery_ref = Some(runtime_ref.to_string());
        let requested_fields = if is_closure {
            contract_fields.clone()
        } else {
            requirement
                .affected_contract_fields
                .iter()
                .filter(|field| contract_field_set.contains(*field))
                .cloned()
                .collect::<Vec<_>>()
        };
        requirement.affected_contract_fields = unique_strings(requested_fields);

        let raw_checks = std::mem::take(&mut requirement.required_code_level_checks);
        let mut checks_by_field = raw_checks
            .into_iter()
            .filter_map(|check| {
                check
                    .contract_field
                    .clone()
                    .filter(|field| requirement.affected_contract_fields.contains(field))
                    .map(|field| (field, check))
            })
            .collect::<BTreeMap<_, _>>();
        requirement.required_code_level_checks = requirement
            .affected_contract_fields
            .iter()
            .map(|field| {
                let mut check = checks_by_field
                    .remove(field)
                    .unwrap_or_else(|| runtime_delivery_check_for_field(field));
                check.check_id = runtime_delivery_check_id(field);
                check.contract_field = Some(field.clone());
                check
            })
            .collect();
    }

    let closure_index = tasks
        .iter()
        .position(|task| matches!(task.task_kind, TaskKind::RuntimeDeliveryClosure));
    let mut field_owners = BTreeMap::<String, usize>::new();
    for (index, task) in tasks.iter().enumerate() {
        let Some(requirement) = task.runtime_delivery_requirement.as_ref() else {
            continue;
        };
        if !requirement.applies_to_this_task {
            continue;
        }
        for field in &requirement.affected_contract_fields {
            let replace = field_owners.get(field).is_none_or(|current| {
                closure_index == Some(index)
                    || (closure_index != Some(*current)
                        && task_owner_rank(task) > task_owner_rank(&tasks[*current]))
            });
            if replace {
                field_owners.insert(field.clone(), index);
            }
        }
    }
    for (index, task) in tasks.iter_mut().enumerate() {
        let Some(requirement) = task.runtime_delivery_requirement.as_mut() else {
            continue;
        };
        if !requirement.applies_to_this_task {
            continue;
        }
        requirement
            .affected_contract_fields
            .retain(|field| field_owners.get(field) == Some(&index));
        requirement.required_code_level_checks.retain(|check| {
            check
                .contract_field
                .as_ref()
                .is_some_and(|field| requirement.affected_contract_fields.contains(field))
        });
        if requirement.affected_contract_fields.is_empty() {
            requirement.applies_to_this_task = false;
            requirement.runtime_delivery_ref = None;
        }
    }
}

fn normalize_frontend_experience_requirements(
    tasks: &mut [TaskDefinition],
    aac: &ArchitectureArtifactContract,
) {
    if !frontend_experience_required(aac) {
        return;
    }
    let template = frontend_experience_requirement_template(aac);
    if template.is_null() {
        return;
    }
    let expected_surface_contract = frontend_ui_surface_decision_contract(aac).cloned();
    for task in tasks {
        if !task_is_frontend_task(task) {
            continue;
        }
        let task_kind = task.task_kind.clone();
        let implementation_actions = task.implementation_actions.clone();
        let task_scope_projection = task_owned_ui_scope(task, aac);
        let Some(requirement) = task.frontend_experience_requirement.as_mut() else {
            let mut normalized = template.clone();
            normalize_ui_task_scope(
                &mut normalized,
                &template,
                &task_kind,
                &implementation_actions,
                &task_scope_projection,
            );
            task.frontend_experience_requirement = Some(normalized);
            continue;
        };
        if !requirement.is_object() {
            *requirement = template.clone();
        }
        requirement["frontendExperienceRef"] =
            json!("sourceRefs.architectureArtifactContractRef#/frontendExperience");
        requirement["mustSatisfy"] = json!(true);
        if requirement.get("uiTaskScope").is_none() {
            requirement["uiTaskScope"] = template
                .get("uiTaskScope")
                .cloned()
                .unwrap_or_else(|| json!({}));
        }
        normalize_ui_task_scope(
            requirement,
            &template,
            &task_kind,
            &implementation_actions,
            &task_scope_projection,
        );
        if let Some(surface_contract) = expected_surface_contract.as_ref() {
            requirement["uiSurfaceDecisionContractRef"] = json!(
                "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract"
            );
            normalize_ui_task_scope_contract(requirement, surface_contract);
        }
    }
}

fn normalize_ui_task_scope(
    requirement: &mut Value,
    template: &Value,
    task_kind: &TaskKind,
    implementation_actions: &[ImplementationAction],
    task_scope_projection: &Value,
) {
    if !requirement
        .get("uiTaskScope")
        .is_some_and(|scope| scope.is_object())
    {
        requirement["uiTaskScope"] = template
            .get("uiTaskScope")
            .cloned()
            .unwrap_or_else(|| json!({}));
    }
    if let Some(scope) = task_scope_projection.as_object() {
        requirement["uiTaskScope"] = Value::Object(scope.clone());
    }
    // ownershipDimensions is a projection of the canonical task scope. The
    // candidate's value is never an input because it can describe a broader
    // surface than the task actually owns.
    let mut dimensions =
        derived_ui_ownership_dimensions(requirement, task_kind, implementation_actions);
    if dimensions.is_empty() {
        dimensions = vec![
            "surface".to_string(),
            "state".to_string(),
            "visual_system".to_string(),
            "content_boundary".to_string(),
        ];
    }
    requirement["uiTaskScope"]["ownershipDimensions"] = json!(dimensions);
}

fn task_owned_ui_scope(task: &TaskDefinition, aac: &ArchitectureArtifactContract) -> Value {
    let mut holder = json!({"uiTaskScope": {}});
    apply_task_owned_ui_scope(&mut holder, task, aac);
    holder
        .get("uiTaskScope")
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn apply_task_owned_ui_scope(
    requirement: &mut Value,
    task: &TaskDefinition,
    aac: &ArchitectureArtifactContract,
) {
    let Some(frontend) = aac.frontend_experience.as_ref() else {
        return;
    };
    let (surface_ids, view_ids, action_ids, operation_path_ids, workflow_ids, interface_ids) =
        task_owned_frontend_ids(task, aac, frontend);
    let Some(scope) = requirement
        .get_mut("uiTaskScope")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    scope.insert(
        "surfacesInScope".to_string(),
        Value::Array(selected_frontend_scope_values(
            frontend,
            "surfaces",
            "surfaceId",
            &surface_ids,
            true,
        )),
    );
    scope.insert(
        "dataViewsInScope".to_string(),
        Value::Array(selected_frontend_scope_values(
            frontend,
            "dataViews",
            "viewId",
            &view_ids,
            false,
        )),
    );
    scope.insert(
        "actionsInScope".to_string(),
        Value::Array(selected_frontend_scope_values(
            frontend,
            "actions",
            "actionId",
            &action_ids,
            false,
        )),
    );
    scope.insert(
        "operationPathsInScope".to_string(),
        Value::Array(selected_frontend_scope_values(
            frontend,
            "operationPaths",
            "pathId",
            &operation_path_ids,
            false,
        )),
    );
    scope.insert(
        "frontendBackendBindings".to_string(),
        Value::Array(
            aac.interfaces
                .iter()
                .filter(|interface| {
                    string_at(interface, "interfaceId")
                        .is_some_and(|id| interface_ids.contains(&id))
                })
                .map(|interface| {
                    json!({
                        "bindingId": format!("ui-binding:{}", string_at(interface, "interfaceId").unwrap_or_default()),
                        "workflowRefs": workflow_ids.clone(),
                        "operationPathRefs": operation_path_ids.clone(),
                        "interfaces": [interface],
                        "completionRule": "Wire the task-owned UI action or surface to this accepted interface when the task owns the interaction."
                    })
                })
                .collect(),
        ),
    );
    if let Some(surface_contract) = frontend.get("uiSurfaceDecisionContract") {
        let region_values = selected_contract_scope_values(
            surface_contract,
            "regionModel",
            "regionId",
            &surface_ids,
            &view_ids,
            &action_ids,
            &operation_path_ids,
        );
        let region_ids = region_values
            .iter()
            .filter_map(|region| string_at(region, "regionId"))
            .collect::<BTreeSet<_>>();
        scope.insert("regionsInScope".to_string(), Value::Array(region_values));
        scope.insert(
            "actionsInContract".to_string(),
            Value::Array(selected_contract_values_by_ids(
                surface_contract,
                "actionModel",
                "actionId",
                &action_ids,
            )),
        );
        let state_ids = selected_state_ids(
            frontend,
            surface_contract,
            &surface_ids,
            &operation_path_ids,
            &region_ids,
        );
        scope.insert(
            "statesInContract".to_string(),
            Value::Array(selected_contract_values_by_ids(
                surface_contract,
                "stateModel",
                "state",
                &state_ids,
            )),
        );
        scope.insert(
            "qualityRulesInScope".to_string(),
            Value::Array(selected_quality_rule_values(
                surface_contract,
                &surface_ids,
                &view_ids,
                &action_ids,
                &operation_path_ids,
            )),
        );
    }
}

fn selected_contract_values_by_ids(
    contract: &Value,
    array_key: &str,
    id_key: &str,
    ids: &BTreeSet<String>,
) -> Vec<Value> {
    contract
        .get(array_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|value| string_at(value, id_key).is_some_and(|id| ids.contains(&id)))
        .cloned()
        .collect()
}

fn selected_contract_scope_values(
    contract: &Value,
    array_key: &str,
    id_key: &str,
    surface_ids: &BTreeSet<String>,
    view_ids: &BTreeSet<String>,
    action_ids: &BTreeSet<String>,
    operation_path_ids: &BTreeSet<String>,
) -> Vec<Value> {
    contract
        .get(array_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|value| {
            string_at(value, id_key).is_some_and(|id| {
                let refs = value
                    .get("surfaceRefs")
                    .or_else(|| value.get("surfaceIds"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>();
                refs.is_empty() && array_key == "regionModel"
                    || refs
                        .iter()
                        .any(|reference| surface_ids.contains(*reference))
                    || surface_ids.contains(&id)
                    || view_ids.contains(&id)
                    || action_ids.contains(&id)
                    || operation_path_ids.contains(&id)
            })
        })
        .cloned()
        .collect()
}

fn selected_state_ids(
    frontend: &Value,
    contract: &Value,
    surface_ids: &BTreeSet<String>,
    operation_path_ids: &BTreeSet<String>,
    region_ids: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for surface in frontend_surface_values(frontend) {
        if string_at(surface, "surfaceId").is_some_and(|id| surface_ids.contains(&id)) {
            ids.extend(string_array_at(surface, "stateRefs"));
            if let Some(model) = surface
                .get("statePlacementModel")
                .and_then(Value::as_object)
            {
                ids.extend(model.keys().cloned());
            }
        }
    }
    for path in array_at(frontend, "operationPaths") {
        if string_at(path, "pathId").is_some_and(|id| operation_path_ids.contains(&id)) {
            ids.extend(string_array_at(path, "stateRefs"));
        }
    }
    let declared = contract
        .get("stateModel")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| string_at(item, "state"))
        .collect::<BTreeSet<_>>();
    // A task that owns a rendered region owns the complete state contract for
    // that region. Execution reconstructs the same rule when it builds the
    // task-scoped frontend contract, so the TaskPlan projection must expose
    // every declared state instead of making valid execution evidence look
    // invented during Review.
    if !region_ids.is_empty() {
        return declared;
    }
    ids.intersection(&declared).cloned().collect()
}

fn selected_quality_rule_values(
    contract: &Value,
    surface_ids: &BTreeSet<String>,
    view_ids: &BTreeSet<String>,
    action_ids: &BTreeSet<String>,
    operation_path_ids: &BTreeSet<String>,
) -> Vec<Value> {
    contract
        .get("qualityRules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|rule| {
            let refs = rule
                .get("scopeRefs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>();
            refs.is_empty()
                || refs.iter().any(|reference| {
                    surface_ids.contains(*reference)
                        || view_ids.contains(*reference)
                        || action_ids.contains(*reference)
                        || operation_path_ids.contains(*reference)
                })
        })
        .cloned()
        .collect()
}

fn task_owned_frontend_ids(
    task: &TaskDefinition,
    aac: &ArchitectureArtifactContract,
    frontend: &Value,
) -> (
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
) {
    let mut views = BTreeSet::new();
    let mut actions = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut workflows = task
        .write_boundary
        .artifact_refs
        .user_flows
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut interfaces = task
        .write_boundary
        .artifact_refs
        .all_interfaces()
        .into_iter()
        .collect::<BTreeSet<_>>();
    for detail in &aac.detail_coverage {
        if !task
            .requirement_detail_refs
            .iter()
            .any(|id| id == &detail.detail_id)
        {
            continue;
        }
        views.extend(detail.artifact_refs.frontend_data_views.iter().cloned());
        actions.extend(detail.artifact_refs.frontend_actions.iter().cloned());
        paths.extend(
            detail
                .artifact_refs
                .frontend_operation_paths
                .iter()
                .cloned(),
        );
        workflows.extend(detail.artifact_refs.user_flows.iter().cloned());
        interfaces.extend(detail.artifact_refs.interfaces.iter().cloned());
    }
    for flow in &aac.user_flows {
        if string_at(flow, "flowId").is_some_and(|id| workflows.contains(&id)) {
            for step in array_at(flow, "happyPath") {
                if let Some(id) = string_at(step, "interactionRef") {
                    interfaces.insert(id);
                }
            }
        }
    }
    for path in array_at(frontend, "operationPaths") {
        let matches = string_at(path, "pathId").is_some_and(|id| paths.contains(&id))
            || string_at(path, "workflowRef").is_some_and(|id| workflows.contains(&id))
            || string_array_at(path, "interfaceRefs")
                .iter()
                .any(|id| interfaces.contains(id));
        if matches {
            if let Some(id) = string_at(path, "pathId") {
                paths.insert(id);
            }
            views.extend(string_array_at(path, "dataViewRefs"));
            actions.extend(string_array_at(path, "actionRefs"));
            workflows.extend(string_at(path, "workflowRef"));
            interfaces.extend(string_array_at(path, "interfaceRefs"));
        }
    }
    let mut surface_ids = BTreeSet::new();
    for surface in frontend_surface_values(frontend) {
        let matches = string_array_at(surface, "dataViewRefs")
            .iter()
            .any(|id| views.contains(id))
            || string_array_at(surface, "actionRefs")
                .iter()
                .any(|id| actions.contains(id))
            || string_array_at(surface, "operationPathRefs")
                .iter()
                .any(|id| paths.contains(id))
            || string_array_at(surface, "workflowRefs")
                .iter()
                .any(|id| workflows.contains(id))
            || string_array_at(surface, "interfaceRefs")
                .iter()
                .any(|id| interfaces.contains(id));
        if matches {
            if let Some(id) = string_at(surface, "surfaceId") {
                surface_ids.insert(id);
            }
            views.extend(string_array_at(surface, "dataViewRefs"));
            actions.extend(string_array_at(surface, "actionRefs"));
            paths.extend(string_array_at(surface, "operationPathRefs"));
            workflows.extend(string_array_at(surface, "workflowRefs"));
            interfaces.extend(string_array_at(surface, "interfaceRefs"));
        }
    }
    (surface_ids, views, actions, paths, workflows, interfaces)
}

fn frontend_surface_values(frontend: &Value) -> Vec<&Value> {
    let registry = frontend
        .pointer("/uiSurfaceRegistry/surfaces")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    if registry.is_empty() {
        array_at(frontend, "surfaces")
    } else {
        registry
    }
}

fn selected_frontend_scope_values(
    frontend: &Value,
    array_key: &str,
    id_key: &str,
    ids: &BTreeSet<String>,
    use_registry_for_surfaces: bool,
) -> Vec<Value> {
    if ids.is_empty() {
        return Vec::new();
    }
    let values = if use_registry_for_surfaces {
        frontend_surface_values(frontend)
    } else {
        array_at(frontend, array_key)
    };
    values
        .into_iter()
        .filter(|value| string_at(value, id_key).is_some_and(|id| ids.contains(&id)))
        .cloned()
        .collect()
}

fn derived_ui_ownership_dimensions(
    requirement: &Value,
    task_kind: &TaskKind,
    implementation_actions: &[ImplementationAction],
) -> Vec<String> {
    let mut dimensions = Vec::new();
    let scope = requirement.get("uiTaskScope").unwrap_or(&Value::Null);
    let has_surface = scope_array_has_items(scope, "surfacesInScope");
    let has_data_view = scope_array_has_items(scope, "dataViewsInScope");
    let has_action = scope_array_has_items(scope, "actionsInScope")
        || scope_array_has_items(scope, "operationPathsInScope");
    let has_state = scope_array_has_items(scope, "stateExpectation");
    let has_integration = scope_array_has_items(scope, "frontendBackendBindings");
    if has_surface {
        dimensions.push("surface".to_string());
        dimensions.push("layout".to_string());
    }
    if has_data_view {
        dimensions.push("data_view".to_string());
    }
    if has_action {
        dimensions.push("action".to_string());
    }
    if has_state || task_kind_is_frontend(task_kind) {
        dimensions.push("state".to_string());
    }
    if has_integration || has_action {
        dimensions.push("integration_feedback".to_string());
    }
    if implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateFrontendNavigation
        )
    }) {
        dimensions.push("surface".to_string());
        dimensions.push("action".to_string());
    }
    if implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::ImplementSharedClientState))
    {
        dimensions.push("state".to_string());
    }
    if implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::ImplementReactiveClientFlow))
    {
        dimensions.push("integration_feedback".to_string());
    }
    if implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::OptimizeFrontendPerformance))
    {
        dimensions.push("layout".to_string());
    }
    if implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::ImplementServerRenderedComponent
        )
    }) {
        dimensions.push("surface".to_string());
        dimensions.push("state".to_string());
    }
    if implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::ImplementServerMutation))
    {
        dimensions.push("action".to_string());
        dimensions.push("integration_feedback".to_string());
    }
    if implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::ImplementFrontendFrameworkVersionFeature
        )
    }) {
        dimensions.push("surface".to_string());
    }
    if task_kind_is_frontend(task_kind)
        || implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateUiFlow
                    | ImplementationAction::CreateOrUpdateFrontendNavigation
                    | ImplementationAction::ImplementReactiveClientFlow
                    | ImplementationAction::ImplementSharedClientState
                    | ImplementationAction::OptimizeFrontendPerformance
                    | ImplementationAction::ImplementServerRenderedComponent
                    | ImplementationAction::ImplementServerMutation
                    | ImplementationAction::ImplementFrontendFrameworkVersionFeature
                    | ImplementationAction::ImplementFrontendExperienceContract
            )
        })
    {
        dimensions.push("visual_system".to_string());
        dimensions.push("content_boundary".to_string());
    }
    unique_strings(dimensions)
}

fn task_kind_is_frontend(task_kind: &TaskKind) -> bool {
    matches!(
        task_kind,
        TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
    )
}

fn scope_array_has_items(scope: &Value, key: &str) -> bool {
    scope
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn task_can_own_requirement_detail(task: &TaskDefinition) -> bool {
    !matches!(
        task.task_kind,
        TaskKind::VerificationIncrement
            | TaskKind::RuntimeDeliveryClosure
            | TaskKind::BrowserQualityClosure
    )
}

fn requirement_detail_owner_score(
    task: &TaskDefinition,
    detail: &RequirementDetailItem,
    coverage: &ArchitectureDetailCoverageEntry,
) -> u32 {
    let artifact_score = artifact_ref_owner_score(&task.write_boundary.artifact_refs, coverage);
    let acceptance_score =
        intersection_count(&task.acceptance_refs, &detail.acceptance_refs) as u32 * 2;
    let concept_score = intersection_count(&task.concept_refs, &detail.concept_refs) as u32 * 3;
    artifact_score
        + acceptance_score
        + concept_score
        + semantic_detail_owner_score(task, detail, coverage)
        + structured_task_kind_owner_score(task, detail, coverage)
}

fn artifact_ref_owner_score(
    task_refs: &TaskArtifactRefs,
    coverage: &ArchitectureDetailCoverageEntry,
) -> u32 {
    let refs = &coverage.artifact_refs;
    let mut score = 0;
    score += intersection_count(&task_refs.interfaces, &refs.interfaces) as u32 * 7;
    score += intersection_count(&task_refs.user_flows, &refs.user_flows) as u32 * 6;
    score += intersection_count(&task_refs.state_machines, &refs.state_machines) as u32 * 6;
    score += intersection_count(&task_refs.state_machines, &refs.constraints) as u32 * 5;
    score += intersection_count(&task_refs.entities, &refs.entities) as u32 * 5;
    score += intersection_count(&task_refs.modules, &refs.modules) as u32 * 3;
    score
}

fn structured_task_kind_owner_score(
    task: &TaskDefinition,
    _detail: &RequirementDetailItem,
    coverage: &ArchitectureDetailCoverageEntry,
) -> u32 {
    let refs = &coverage.artifact_refs;
    let has_data =
        !refs.entities.is_empty() || !refs.fields.is_empty() || !refs.constraints.is_empty();
    let has_api = !refs.interfaces.is_empty();
    let has_ui = !refs.frontend_data_views.is_empty()
        || !refs.frontend_actions.is_empty()
        || !refs.frontend_operation_paths.is_empty();
    let has_flow = !refs.user_flows.is_empty();
    let has_state = !refs.state_machines.is_empty();
    let mut score: u32 = 0;
    if has_data && task_directly_owns_persistence_mapping(task) {
        score += 16;
    }
    if has_api && task_owns_interface_behavior(task) {
        score += 14;
    }
    if has_ui && task_is_frontend_task(task) {
        score += 18;
    }
    if has_flow && task_owns_business_flow_behavior(task) {
        score += 12;
    }
    if has_state
        && task
            .implementation_actions
            .iter()
            .any(|action| matches!(action, ImplementationAction::CreateOrUpdateStateMachine))
    {
        score += 14;
    }
    if !has_data && !has_api && !has_ui && !has_flow && !has_state {
        score += match task.task_kind {
            TaskKind::FeatureIncrement => 6,
            TaskKind::InterfaceIncrement if task_owns_interface_behavior(task) => 5,
            TaskKind::IntegrationIncrement => 3,
            // A frontend task is not the owner of a generic requirement
            // unless the accepted detail carries a frontend impact.
            _ => 0,
        };
    }
    score
}

fn semantic_detail_owner_score(
    task: &TaskDefinition,
    detail: &RequirementDetailItem,
    coverage: &ArchitectureDetailCoverageEntry,
) -> u32 {
    let mut score: u32 = 0;
    if (detail.impact_tags.iter().any(|tag| tag == "data_model")
        || detail.lifecycle_stage == "create")
        && task_directly_owns_persistence_mapping(task)
    {
        score += 4;
    }
    if detail.impact_tags.iter().any(|tag| tag == "business_flow")
        && task_owns_business_flow_behavior(task)
    {
        score += 4;
    }
    if detail.impact_tags.iter().any(|tag| tag == "frontend") && task_is_frontend_task(task) {
        score += 4;
    }
    if detail.impact_tags.iter().any(|tag| tag == "interface") && task_owns_interface_behavior(task)
    {
        score += 4;
    }
    if !coverage.artifact_refs.state_machines.is_empty()
        && task
            .implementation_actions
            .iter()
            .any(|action| matches!(action, ImplementationAction::CreateOrUpdateStateMachine))
    {
        score += 3;
    }
    if !coverage.artifact_refs.user_flows.is_empty() && task_owns_business_flow_behavior(task) {
        score += 3;
    }
    // These are structured requirement facts. They are the deterministic
    // fallback when AAC has not attached a concrete artifact to a detail.
    match detail.lifecycle_stage.as_str() {
        "state_change" | "approve_or_process" => {
            if task_owns_interface_behavior(task) {
                score += 10;
            }
            if task_owns_business_flow_behavior(task) {
                score += 4;
            }
        }
        "create" | "update" | "query_select" => {
            if task_directly_owns_persistence_mapping(task) {
                score += 8;
            }
            if task_owns_interface_behavior(task) {
                score += 5;
            }
        }
        _ => {}
    }
    if detail.impact_tags.iter().any(|tag| tag == "frontend") {
        if task_is_frontend_task(task) {
            score += 24;
        } else {
            score = score.saturating_sub(20);
        }
    }
    if detail.impact_tags.iter().any(|tag| tag == "data_model")
        && task_directly_owns_persistence_mapping(task)
    {
        score += 10;
    }
    score
}

fn task_owns_business_flow_behavior(task: &TaskDefinition) -> bool {
    matches!(
        task.task_kind,
        TaskKind::FeatureIncrement | TaskKind::InterfaceIncrement | TaskKind::UiFlowIncrement
    ) || task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateBusinessRule
                | ImplementationAction::CreateOrUpdateInterface
                | ImplementationAction::CreateOrUpdateUiFlow
                | ImplementationAction::CreateOrUpdateFrontendNavigation
                | ImplementationAction::WireReferenceInApiOrUi
        )
    })
}

fn task_owns_interface_behavior(task: &TaskDefinition) -> bool {
    matches!(
        task.task_kind,
        TaskKind::FeatureIncrement | TaskKind::InterfaceIncrement | TaskKind::IntegrationIncrement
    ) || task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateInterface
                | ImplementationAction::WireReferenceInApiOrUi
        )
    })
}

fn intersection_count(left: &[String], right: &[String]) -> usize {
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let right = right.iter().collect::<BTreeSet<_>>();
    left.iter().filter(|item| right.contains(item)).count()
}

fn normalize_task_verification_detail_refs(
    tasks: &mut [TaskDefinition],
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) {
    let detail_acceptance_refs = current_phase_covered_detail_acceptance_refs(pgc, aac);
    if detail_acceptance_refs.is_empty() {
        return;
    }
    for task in tasks {
        let task_detail_refs = task.requirement_detail_refs.clone();
        for detail_ref in task_detail_refs {
            let Some(acceptance_refs) = detail_acceptance_refs.get(&detail_ref) else {
                continue;
            };
            if task.verification_intents.iter().any(|intent| {
                intent
                    .requirement_detail_refs
                    .iter()
                    .any(|item| item == &detail_ref)
            }) {
                continue;
            }
            if let Some(intent_index) = verification_intent_for_detail(task, acceptance_refs)
                .or_else(|| (!task.verification_intents.is_empty()).then_some(0))
            {
                push_unique(
                    &mut task.verification_intents[intent_index].requirement_detail_refs,
                    detail_ref,
                );
            }
        }
    }
}

fn current_phase_covered_detail_acceptance_refs(
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) -> BTreeMap<String, BTreeSet<String>> {
    let covered_detail_ids = aac
        .detail_coverage
        .iter()
        .filter(|entry| matches!(entry.coverage_status, CoverageStatus::Covered))
        .map(|entry| entry.detail_id.as_str())
        .collect::<BTreeSet<_>>();
    pgc.requirement_details
        .items
        .iter()
        .filter(|detail| {
            detail.required_for_current_phase
                && covered_detail_ids.contains(detail.detail_id.as_str())
        })
        .map(|detail| {
            (
                detail.detail_id.clone(),
                detail
                    .acceptance_refs
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect()
}

fn verification_intent_for_detail(
    task: &TaskDefinition,
    acceptance_refs: &BTreeSet<String>,
) -> Option<usize> {
    if task.verification_intents.len() == 1 {
        return Some(0);
    }
    if acceptance_refs.is_empty() {
        return None;
    }
    let matches = task
        .verification_intents
        .iter()
        .enumerate()
        .filter_map(|(index, intent)| {
            intent
                .acceptance_refs
                .iter()
                .any(|item| acceptance_refs.contains(item))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    matches.first().copied()
}

fn normalize_runtime_delivery_closure_group(
    groups: &mut Vec<TaskPlanGroup>,
    tasks: &mut [TaskDefinition],
    runtime_delivery: Option<&Value>,
) {
    let Some(runtime_delivery) = runtime_delivery else {
        return;
    };
    if runtime_delivery.get("status").and_then(Value::as_str) != Some("modified") {
        return;
    }
    let closure_task_indices = tasks
        .iter()
        .enumerate()
        .filter_map(|(index, task)| {
            matches!(task.task_kind, TaskKind::RuntimeDeliveryClosure).then_some(index)
        })
        .collect::<Vec<_>>();
    if closure_task_indices.len() != 1 {
        return;
    }

    let closure_task_index = closure_task_indices[0];
    let closure_task_id = tasks[closure_task_index].task_id.clone();
    let current_group_id = tasks[closure_task_index].group_id.clone();
    let current_group_has_only_closure = groups
        .iter()
        .find(|group| group.group_id == current_group_id)
        .is_some_and(|group| group.task_ids == vec![closure_task_id.clone()]);
    let closure_group_id = if current_group_has_only_closure {
        current_group_id
    } else {
        next_runtime_closure_group_id(groups)
    };

    for group in groups.iter_mut() {
        group.task_ids.retain(|task_id| task_id != &closure_task_id);
        group
            .depends_on
            .retain(|group_id| group_id != &closure_group_id);
    }

    tasks[closure_task_index].group_id = closure_group_id.clone();
    tasks[closure_task_index].depends_on.clear();
    if !groups
        .iter()
        .any(|group| group.group_id == closure_group_id)
    {
        groups.push(TaskPlanGroup {
            group_id: closure_group_id.clone(),
            title: "Runtime delivery closure".to_string(),
            objective: "Verify the final RuntimeDeliveryContract code-level closure.".to_string(),
            depends_on: vec![],
            scope_refs: vec![],
            acceptance_refs: vec![],
            task_ids: vec![],
        });
    }

    let dependency_group_ids = groups
        .iter()
        .filter(|group| group.group_id != closure_group_id && !group.task_ids.is_empty())
        .map(|group| group.group_id.clone())
        .collect::<Vec<_>>();
    if let Some(group) = groups
        .iter_mut()
        .find(|group| group.group_id == closure_group_id)
    {
        group.task_ids = vec![closure_task_id];
        group.scope_refs.clear();
        group.acceptance_refs.clear();
        group.depends_on = dependency_group_ids;
    }
    if let Some(index) = groups
        .iter()
        .position(|group| group.group_id == closure_group_id)
    {
        let closure_group = groups.remove(index);
        groups.push(closure_group);
    }
}

fn next_runtime_closure_group_id(groups: &[TaskPlanGroup]) -> String {
    const PREFERRED: &str = "group-runtime-delivery-closure";
    let existing = groups
        .iter()
        .map(|group| group.group_id.as_str())
        .collect::<BTreeSet<_>>();
    if !existing.contains(PREFERRED) {
        return PREFERRED.to_string();
    }
    for suffix in 2.. {
        let candidate = format!("{PREFERRED}-{suffix}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("unbounded runtime closure group id suffix search should always return")
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|item| item == &value) {
        values.push(value);
    }
}

fn validate_runtime_delivery_requirements(
    tasks: &[TaskDefinition],
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    for task in tasks {
        let Some(requirement) = task.runtime_delivery_requirement.as_ref() else {
            continue;
        };
        let target = Some(task.task_id.as_str());
        if requirement.applies_to_this_task {
            if requirement
                .runtime_delivery_ref
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                issues.push(issue(
                    "RUNTIME_REQUIREMENT_INVALID",
                    "tasks[].runtimeDeliveryRequirement.runtimeDeliveryRef",
                    "Runtime-affecting tasks must reference the accepted RuntimeDeliveryContract.",
                    target,
                ));
            }
            if requirement.affected_contract_fields.is_empty() {
                issues.push(issue(
                    "RUNTIME_REQUIREMENT_INVALID",
                    "tasks[].runtimeDeliveryRequirement.affectedContractFields",
                    "Runtime-affecting tasks must list affected runtime contract fields.",
                    target,
                ));
            }
            if requirement.required_code_level_checks.is_empty() {
                issues.push(issue(
                    "RUNTIME_REQUIREMENT_INVALID",
                    "tasks[].runtimeDeliveryRequirement.requiredCodeLevelChecks",
                    "Runtime-affecting tasks must list required code-level runtime checks.",
                    target,
                ));
            }
            let boundary_text = format!(
                "{} {} {}",
                requirement
                    .required_code_level_checks
                    .iter()
                    .map(|check| format!(
                        "{} {}",
                        check.objective,
                        check.contract_field.as_deref().unwrap_or_default()
                    ))
                    .collect::<Vec<_>>()
                    .join(" "),
                requirement.evidence_expected_in_task_result.join(" "),
                requirement.forbidden_actions.join(" ")
            )
            .to_ascii_lowercase();
            if boundary_text.contains("require clean install")
                || boundary_text.contains("required clean install")
                || boundary_text.contains("require container")
                || boundary_text.contains("required container")
                || boundary_text.contains("must run docker")
                || boundary_text.contains("must run deploy")
            {
                issues.push(issue(
                    "RUNTIME_REQUIREMENT_BOUNDARY_INVALID",
                    "tasks[].runtimeDeliveryRequirement.verificationBoundary",
                    "RuntimeDeliveryRequirement must stay at code level and must not require clean install, container build, registry, or deploy success.",
                    target,
                ));
            }
        } else if requirement.reason.trim().is_empty() {
            issues.push(issue(
                "RUNTIME_REQUIREMENT_INVALID",
                "tasks[].runtimeDeliveryRequirement.reason",
                "Non-applicable runtimeDeliveryRequirement entries must explain why the task does not affect runtime delivery.",
                target,
            ));
        }
    }
    issues
}

fn validate_quality_requirement_ownership(
    tasks: &[TaskDefinition],
    engineering: &[contracts::EngineeringQualityRequirement],
    architecture: &[contracts::ArchitectureQualityRequirement],
    api: &[contracts::ApiContractRequirement],
    code: &[contracts::CodeQualityRequirement],
) -> Vec<delivery_core::RepairIssue> {
    let task_ids = tasks
        .iter()
        .map(|task| task.task_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();

    for requirement in engineering {
        validate_requirement_task_ownership(
            &mut issues,
            &task_ids,
            &requirement.requirement_id,
            &requirement.applies_to_task_ids,
            tasks,
            |task| task.engineering_quality_requirement_refs.as_slice(),
            "engineeringQualityRequirements",
        );
    }
    for requirement in architecture {
        validate_requirement_task_ownership(
            &mut issues,
            &task_ids,
            &requirement.requirement_id,
            &requirement.applies_to_task_ids,
            tasks,
            |task| task.architecture_quality_requirement_refs.as_slice(),
            "architectureQualityRequirements",
        );
    }
    for requirement in api {
        validate_requirement_task_ownership(
            &mut issues,
            &task_ids,
            &requirement.requirement_id,
            &requirement.applies_to_task_ids,
            tasks,
            |task| task.api_contract_requirement_refs.as_slice(),
            "apiContractRequirements",
        );
    }
    for requirement in code {
        validate_requirement_task_ownership(
            &mut issues,
            &task_ids,
            &requirement.requirement_id,
            &requirement.applies_to_task_ids,
            tasks,
            |task| task.code_quality_requirement_refs.as_slice(),
            "codeQualityRequirements",
        );
    }

    issues
}

fn validate_requirement_task_ownership<F>(
    issues: &mut Vec<delivery_core::RepairIssue>,
    task_ids: &BTreeSet<&str>,
    requirement_id: &str,
    applies_to_task_ids: &[String],
    tasks: &[TaskDefinition],
    refs_for_task: F,
    field_name: &str,
) where
    F: Fn(&TaskDefinition) -> &[String] + Copy,
{
    if applies_to_task_ids.is_empty() {
        issues.push(issue(
            "QUALITY_REQUIREMENT_OWNERSHIP_INVALID",
            field_name,
            "Every derived quality requirement must have at least one owning task.",
            Some(requirement_id),
        ));
        return;
    }
    for task_id in applies_to_task_ids {
        if !task_ids.contains(task_id.as_str()) {
            issues.push(issue(
                "QUALITY_REQUIREMENT_OWNERSHIP_INVALID",
                field_name,
                "Quality requirement ownership must reference an existing TaskPlan task.",
                Some(requirement_id),
            ));
            continue;
        }
        let Some(task) = tasks.iter().find(|task| task.task_id == *task_id) else {
            continue;
        };
        if !refs_for_task(task)
            .iter()
            .any(|reference| reference == requirement_id)
        {
            issues.push(issue(
                "QUALITY_REQUIREMENT_OWNERSHIP_INVALID",
                field_name,
                "A quality requirement must be present in the owning task's derived requirement refs.",
                Some(task_id),
            ));
        }
        if task.verification_intents.is_empty() {
            issues.push(issue(
                "QUALITY_REQUIREMENT_VERIFICATION_MISSING",
                "tasks[].verificationIntents",
                "Every task owning a quality requirement must provide a verification intent for its evidence.",
                Some(task_id),
            ));
        }
    }
    for task in tasks {
        if refs_for_task(task)
            .iter()
            .any(|reference| reference == requirement_id)
            && !applies_to_task_ids
                .iter()
                .any(|task_id| task_id == &task.task_id)
        {
            issues.push(issue(
                "QUALITY_REQUIREMENT_OWNERSHIP_INVALID",
                field_name,
                "A task must not claim a quality requirement that does not apply to it.",
                Some(&task.task_id),
            ));
        }
    }
}

fn validate_must_acceptance_task_coverage(
    tasks: &[TaskDefinition],
    pgc: &contracts::PlanningGenerationContract,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    for acceptance in pgc
        .phase_scope
        .acceptance_candidates
        .iter()
        .filter(|acceptance| matches!(acceptance.priority, AcceptancePriority::Must))
    {
        if tasks
            .iter()
            .any(|task| task.acceptance_refs.contains(&acceptance.id))
        {
            continue;
        }
        issues.push(issue(
            "MUST_ACCEPTANCE_NOT_COVERED",
            "tasks[].acceptanceRefs",
            "Every must acceptance candidate must be assigned to at least one TaskPlan task.",
            Some(&acceptance.id),
        ));
    }
    issues
}

fn validate_frontend_task_presence(
    tasks: &[TaskDefinition],
    aac: &ArchitectureArtifactContract,
) -> Vec<delivery_core::RepairIssue> {
    if !frontend_experience_required(aac) {
        return Vec::new();
    }
    if tasks.iter().any(|task| {
        matches!(
            task.task_kind,
            TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
        ) || task.frontend_experience_requirement.is_some()
    }) {
        return Vec::new();
    }
    vec![issue(
        "FRONTEND_TASK_REQUIRED",
        "tasks[].frontendExperienceRequirement",
        "frontendExperience.required=true requires a UI/frontend task or a task with frontendExperienceRequirement.",
        None,
    )]
}

fn validate_frontend_quality_requirements(
    tasks: &[TaskDefinition],
    aac: &ArchitectureArtifactContract,
) -> Vec<delivery_core::RepairIssue> {
    if !frontend_experience_required(aac) {
        return Vec::new();
    }
    let expected_surface_contract = frontend_ui_surface_decision_contract(aac);
    let mut issues = Vec::new();
    for task in tasks {
        let owns_frontend = matches!(
            task.task_kind,
            TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
        ) || task.frontend_experience_requirement.is_some();
        if !owns_frontend {
            continue;
        }
        let Some(requirement) = task.frontend_experience_requirement.as_ref() else {
            issues.push(issue(
                "FRONTEND_REQUIREMENT_REQUIRED",
                "tasks[].frontendExperienceRequirement",
                "UI/frontend tasks must carry frontendExperienceRequirement.",
                Some(&task.task_id),
            ));
            continue;
        };
        if requirement
            .get("frontendExperienceRef")
            .and_then(Value::as_str)
            != Some("sourceRefs.architectureArtifactContractRef#/frontendExperience")
        {
            issues.push(issue(
                "FRONTEND_REQUIREMENT_REF_INVALID",
                "tasks[].frontendExperienceRequirement.frontendExperienceRef",
                "frontendExperienceRequirement.frontendExperienceRef must point to the AAC frontendExperience.",
                Some(&task.task_id),
            ));
        }
        if expected_surface_contract.is_some()
            && requirement
                .get("uiSurfaceDecisionContractRef")
                .and_then(Value::as_str)
                != Some(
                    "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
                )
        {
            issues.push(issue(
                "FRONTEND_UI_SURFACE_DECISION_REF_REQUIRED",
                "tasks[].frontendExperienceRequirement.uiSurfaceDecisionContractRef",
                "frontendExperienceRequirement must carry the AAC uiSurfaceDecisionContract ref.",
                Some(&task.task_id),
            ));
        }
        if let Some(surface_contract) = expected_surface_contract {
            validate_ui_task_scope_contract(task, requirement, surface_contract, &mut issues);
        }
        validate_ui_ownership_dimensions(task, requirement, &mut issues);
    }
    issues
}

fn validate_ui_ownership_dimensions(
    task: &TaskDefinition,
    requirement: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(dimensions) = requirement
        .pointer("/uiTaskScope/ownershipDimensions")
        .and_then(Value::as_array)
    else {
        issues.push(issue(
            "FRONTEND_UI_OWNERSHIP_DIMENSIONS_REQUIRED",
            "tasks[].frontendExperienceRequirement.uiTaskScope.ownershipDimensions",
            "UI/frontend tasks must declare ownership dimensions so execution can compile a task-scoped uiProductionBrief.",
            Some(&task.task_id),
        ));
        return;
    };
    if dimensions.is_empty() {
        issues.push(issue(
            "FRONTEND_UI_OWNERSHIP_DIMENSIONS_REQUIRED",
            "tasks[].frontendExperienceRequirement.uiTaskScope.ownershipDimensions",
            "uiTaskScope.ownershipDimensions cannot be empty for UI/frontend tasks.",
            Some(&task.task_id),
        ));
        return;
    }
    for dimension in dimensions {
        let Some(value) = dimension.as_str() else {
            issues.push(issue(
                "FRONTEND_UI_OWNERSHIP_DIMENSION_INVALID",
                "tasks[].frontendExperienceRequirement.uiTaskScope.ownershipDimensions",
                "Each ownership dimension must be a string enum value.",
                Some(&task.task_id),
            ));
            continue;
        };
        if !UI_OWNERSHIP_DIMENSION_VALUES.contains(&value) {
            issues.push(issue(
                "FRONTEND_UI_OWNERSHIP_DIMENSION_INVALID",
                "tasks[].frontendExperienceRequirement.uiTaskScope.ownershipDimensions",
                "ownershipDimensions must use only surface, data_view, action, state, layout, visual_system, content_boundary, or integration_feedback.",
                Some(&task.task_id),
            ));
        }
    }
}

fn validate_ui_task_scope_contract(
    task: &TaskDefinition,
    requirement: &Value,
    surface_contract: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(scope) = requirement
        .get("uiTaskScope")
        .filter(|value| value.is_object())
    else {
        issues.push(issue(
            "FRONTEND_UI_TASK_SCOPE_REQUIRED",
            "tasks[].frontendExperienceRequirement.uiTaskScope",
            "Frontend tasks must carry one MCP-derived uiTaskScope projection.",
            Some(&task.task_id),
        ));
        return;
    };
    for (field, contract_key, id_key, code) in [
        (
            "regionsInScope",
            "regionModel",
            "regionId",
            "FRONTEND_UI_REGION_SCOPE_INVALID",
        ),
        (
            "actionsInContract",
            "actionModel",
            "actionId",
            "FRONTEND_UI_ACTION_SCOPE_INVALID",
        ),
        (
            "statesInContract",
            "stateModel",
            "state",
            "FRONTEND_UI_STATE_SCOPE_INVALID",
        ),
        (
            "qualityRulesInScope",
            "qualityRules",
            "ruleId",
            "FRONTEND_UI_QUALITY_RULE_SCOPE_INVALID",
        ),
    ] {
        let Some(items) = scope.get(field).and_then(Value::as_array) else {
            issues.push(issue(
                code,
                &format!("tasks[].frontendExperienceRequirement.uiTaskScope.{field}"),
                "The MCP-derived UI task scope field must be an array.",
                Some(&task.task_id),
            ));
            continue;
        };
        let allowed = contract_string_ids(surface_contract, &format!("/{contract_key}"), id_key);
        for item in items {
            let Some(id) = item.get(id_key).and_then(Value::as_str) else {
                issues.push(issue(
                    code,
                    &format!("tasks[].frontendExperienceRequirement.uiTaskScope.{field}"),
                    "Every UI task scope entry must be a structured object with its canonical id.",
                    Some(&task.task_id),
                ));
                continue;
            };
            if !allowed.is_empty() && !allowed.contains(id) {
                issues.push(issue(
                    code,
                    &format!("tasks[].frontendExperienceRequirement.uiTaskScope.{field}"),
                    "UI task scope entries must reference IDs declared by AAC uiSurfaceDecisionContract.",
                    Some(&task.task_id),
                ));
            }
        }
    }
}

fn validate_requirement_detail_assignments(
    tasks: &[TaskDefinition],
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) -> Vec<delivery_core::RepairIssue> {
    let covered_detail_ids = aac
        .detail_coverage
        .iter()
        .filter(|entry| matches!(entry.coverage_status, CoverageStatus::Covered))
        .map(|entry| entry.detail_id.clone())
        .collect::<BTreeSet<_>>();
    if covered_detail_ids.is_empty() {
        return Vec::new();
    }
    let task_detail_owners = tasks
        .iter()
        .flat_map(|task| {
            task.requirement_detail_refs
                .iter()
                .map(|detail_id| (detail_id.clone(), task.task_id.clone()))
        })
        .fold(
            BTreeMap::<String, Vec<String>>::new(),
            |mut owners, (detail, task)| {
                owners.entry(detail).or_default().push(task);
                owners
            },
        );

    let mut issues = Vec::new();
    for detail in pgc
        .requirement_details
        .items
        .iter()
        .filter(|detail| detail.required_for_current_phase)
    {
        if !covered_detail_ids.contains(&detail.detail_id) {
            continue;
        }
        let owners = task_detail_owners
            .get(&detail.detail_id)
            .cloned()
            .unwrap_or_default();
        if owners.is_empty() {
            issues.push(issue(
                "DETAIL_TASK_ASSIGNMENT_MISSING",
                "tasks[].requirementDetailRefs",
                "Every covered current-phase requirement detail must have exactly one MCP-derived implementation owner.",
                Some(&detail.detail_id),
            ));
        } else if owners.len() != 1 {
            issues.push(issue(
                "DETAIL_TASK_OWNERSHIP_CONFLICT",
                "tasks[].requirementDetailRefs",
                "A requirement detail must be owned by exactly one implementation task; verification and closure tasks must not duplicate the business owner.",
                Some(&detail.detail_id),
            ));
        }
        let verification_owned = owners.first().is_some_and(|owner_id| {
            tasks
                .iter()
                .find(|task| &task.task_id == owner_id)
                .is_some_and(|task| {
                    task.verification_intents.iter().any(|intent| {
                        intent
                            .requirement_detail_refs
                            .iter()
                            .any(|item| item == &detail.detail_id)
                    })
                })
        });
        if !verification_owned {
            issues.push(issue(
                "DETAIL_TASK_ASSIGNMENT_MISSING",
                "tasks[].verificationIntents[].requirementDetailRefs",
                "The owning implementation task must include each covered detail in an assigned verification intent.",
                Some(&detail.detail_id),
            ));
        }
    }
    issues
}

fn validate_workflow_closure_task_assignments(
    tasks: &[TaskDefinition],
    aac: &ArchitectureArtifactContract,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    for requirement in workflow_closure_requirements(aac) {
        let closure_id = requirement
            .get("closureId")
            .and_then(Value::as_str)
            .unwrap_or("workflow_closure");
        if tasks
            .iter()
            .any(|task| task_covers_workflow_closure(task, &requirement))
        {
            continue;
        }
        issues.push(issue(
            "WORKFLOW_CLOSURE_NOT_ASSIGNED",
            "tasks[].frontendExperienceRequirement",
            "Every workflow closure requirement must be assigned to a task that wires the user flow to every declared interface and verifies it with automated or runtime API evidence.",
            Some(closure_id),
        ));
    }
    issues
}

pub(crate) fn task_covers_workflow_closure(task: &TaskDefinition, requirement: &Value) -> bool {
    let Some(workflow_ref) = requirement.get("workflowRef").and_then(Value::as_str) else {
        return false;
    };
    if !task
        .write_boundary
        .artifact_refs
        .user_flows
        .iter()
        .any(|item| item == workflow_ref)
    {
        return false;
    }
    let required_interfaces = string_array_at(requirement, "interfaceRefs");
    if !required_interfaces.iter().all(|interface_ref| {
        task.write_boundary
            .artifact_refs
            .all_interfaces()
            .iter()
            .any(|item| item == interface_ref)
    }) {
        return false;
    }
    let required_acceptance = string_array_at(requirement, "acceptanceRefs");
    if !required_acceptance.iter().all(|acceptance_ref| {
        task.acceptance_refs
            .iter()
            .any(|item| item == acceptance_ref)
    }) {
        return false;
    }
    if task.frontend_experience_requirement.is_none() {
        return false;
    }
    if !task
        .implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::WireReferenceInApiOrUi))
    {
        return false;
    }
    task.verification_intents.iter().any(|intent| {
        required_acceptance.iter().all(|acceptance_ref| {
            intent
                .acceptance_refs
                .iter()
                .any(|item| item == acceptance_ref)
        }) && intent.acceptable_evidence.iter().any(|evidence| {
            matches!(
                evidence,
                VerificationEvidence::AutomatedTest | VerificationEvidence::RuntimeApiCheck
            )
        })
    })
}

fn validate_runtime_delivery_closure_task(
    groups: &[TaskPlanGroup],
    tasks: &[TaskDefinition],
    runtime_delivery: Option<&Value>,
) -> Vec<delivery_core::RepairIssue> {
    let Some(runtime_delivery) = runtime_delivery else {
        return Vec::new();
    };
    if runtime_delivery.get("status").and_then(Value::as_str) != Some("modified") {
        return Vec::new();
    }

    let mut issues = Vec::new();
    let closure_tasks = tasks
        .iter()
        .filter(|task| matches!(task.task_kind, contracts::TaskKind::RuntimeDeliveryClosure))
        .collect::<Vec<_>>();
    if closure_tasks.len() != 1 {
        issues.push(issue(
            "RUNTIME_CLOSURE_TASK_REQUIRED",
            "tasks.runtimeDeliveryClosure",
            "RuntimeDelivery status=modified requires exactly one runtime_delivery_closure task.",
            None,
        ));
        return issues;
    }
    let closure = closure_tasks[0];
    let target = Some(closure.task_id.as_str());
    let Some(requirement) = closure.runtime_delivery_requirement.as_ref() else {
        issues.push(issue(
            "RUNTIME_CLOSURE_REQUIREMENT_INVALID",
            "tasks[].runtimeDeliveryRequirement",
            "runtime_delivery_closure task must carry runtimeDeliveryRequirement.",
            target,
        ));
        return issues;
    };
    if !requirement.applies_to_this_task {
        issues.push(issue(
            "RUNTIME_CLOSURE_REQUIREMENT_INVALID",
            "tasks[].runtimeDeliveryRequirement.appliesToThisTask",
            "runtime_delivery_closure task must have runtimeDeliveryRequirement.appliesToThisTask=true.",
            target,
        ));
        return issues;
    }

    let required_fields = runtime_delivery_closure_fields(runtime_delivery);
    let required_field_set = required_fields.iter().cloned().collect::<BTreeSet<_>>();
    let affected_fields = requirement
        .affected_contract_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for field in &required_fields {
        if !affected_fields.contains(field) {
            issues.push(issue(
                "RUNTIME_CLOSURE_FIELD_MISSING",
                "tasks[].runtimeDeliveryRequirement.affectedContractFields",
                &format!("runtime_delivery_closure must include affected field {field}."),
                target,
            ));
        }
    }
    for field in &requirement.affected_contract_fields {
        if !required_field_set.contains(field) {
            issues.push(issue(
                "RUNTIME_CLOSURE_FIELD_INVALID",
                "tasks[].runtimeDeliveryRequirement.affectedContractFields",
                &format!("runtime_delivery_closure affected field {field} is not required by RuntimeDeliveryContract."),
                target,
            ));
        }
    }

    let check_id_by_field = requirement
        .required_code_level_checks
        .iter()
        .filter_map(|check| {
            check
                .contract_field
                .as_ref()
                .map(|field| (field.clone(), check.check_id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let required_check_ids = required_fields
        .iter()
        .map(|field| (field, runtime_delivery_closure_check_id(field)))
        .collect::<Vec<_>>();
    let required_check_id_set = required_check_ids
        .iter()
        .map(|(_, check_id)| check_id.clone())
        .collect::<BTreeSet<_>>();
    for (field, check_id) in &required_check_ids {
        if check_id_by_field.get(*field) != Some(check_id) {
            issues.push(issue(
                "RUNTIME_CLOSURE_CHECK_INVALID",
                "tasks[].runtimeDeliveryRequirement.requiredCodeLevelChecks",
                &format!("runtime_delivery_closure must include checkId {check_id} for {field}."),
                target,
            ));
        }
    }
    for check in &requirement.required_code_level_checks {
        let contract_field = check.contract_field.as_deref().unwrap_or_default();
        if !required_field_set.contains(contract_field)
            || !required_check_id_set.contains(&check.check_id)
        {
            issues.push(issue(
                "RUNTIME_CLOSURE_CHECK_INVALID",
                "tasks[].runtimeDeliveryRequirement.requiredCodeLevelChecks",
                "runtime_delivery_closure checkIds and contractFields must match RuntimeDeliveryContract exactly.",
                target,
            ));
        }
    }

    let Some(closure_group) = groups
        .iter()
        .find(|group| group.group_id == closure.group_id)
    else {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].groupId",
            "runtime_delivery_closure group must exist.",
            target,
        ));
        return issues;
    };
    if closure_group.task_ids != vec![closure.task_id.clone()] {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].taskIds",
            "runtime_delivery_closure group must contain exactly the closure task.",
            Some(&closure_group.group_id),
        ));
    }
    let browser_closure_group = browser_quality_closure_group(groups, tasks);
    let expected_final_group_id = browser_closure_group
        .map(|group| group.group_id.as_str())
        .unwrap_or(closure_group.group_id.as_str());
    if groups.last().map(|group| group.group_id.as_str()) != Some(expected_final_group_id) {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].position",
            "runtime_delivery_closure must be final unless an MCP-generated browser quality closure follows it.",
            Some(&closure_group.group_id),
        ));
    }
    if browser_closure_group
        .is_some_and(|group| !group.depends_on.contains(&closure_group.group_id))
    {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].dependsOn",
            "The browser quality closure must depend on runtime_delivery_closure.",
            browser_closure_group.map(|group| group.group_id.as_str()),
        ));
    }
    for group in groups.iter().filter(|group| {
        group.depends_on.contains(&closure_group.group_id)
            && Some(group.group_id.as_str())
                != browser_closure_group.map(|browser| browser.group_id.as_str())
    }) {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].dependsOn",
            "Only the MCP-generated browser quality closure may depend on runtime_delivery_closure.",
            Some(&group.group_id),
        ));
    }

    let task_group_by_id = tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task.group_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    for dependency in &closure.depends_on {
        if let Some(dependency_group) = task_group_by_id.get(dependency.as_str()) {
            if *dependency_group != closure_group.group_id.as_str() {
                issues.push(issue(
                    "RUNTIME_CLOSURE_TASK_DEPENDENCY_INVALID",
                    "tasks[].dependsOn",
                    "runtime_delivery_closure task must not depend directly on tasks in other groups; use group dependsOn.",
                    target,
                ));
            }
        }
    }

    let closure_group_dependencies = transitive_group_dependencies(groups, &closure_group.group_id);
    for runtime_task in tasks.iter().filter(|task| {
        task.task_id != closure.task_id
            && task
                .runtime_delivery_requirement
                .as_ref()
                .is_some_and(|requirement| requirement.applies_to_this_task)
    }) {
        if runtime_task.group_id == closure_group.group_id {
            if !closure.depends_on.contains(&runtime_task.task_id) {
                issues.push(issue(
                    "RUNTIME_CLOSURE_TASK_DEPENDENCY_INVALID",
                    "tasks[].dependsOn",
                    "runtime_delivery_closure must depend on runtime-affecting tasks in its own group.",
                    target,
                ));
            }
        } else if !closure_group_dependencies.contains(&runtime_task.group_id) {
            issues.push(issue(
                "RUNTIME_CLOSURE_GROUP_DEPENDENCY_INVALID",
                "groups[].dependsOn",
                "runtime_delivery_closure group must depend on every group containing runtime-affecting tasks.",
                Some(&closure_group.group_id),
            ));
        }
    }

    issues
}

fn browser_quality_closure_group<'a>(
    groups: &'a [TaskPlanGroup],
    tasks: &[TaskDefinition],
) -> Option<&'a TaskPlanGroup> {
    let closure_group_id = tasks
        .iter()
        .find(|task| matches!(task.task_kind, TaskKind::BrowserQualityClosure))
        .map(|task| task.group_id.as_str())?;
    groups
        .iter()
        .find(|group| group.group_id == closure_group_id)
}

fn normalize_browser_verification_assignments(tasks: &mut [TaskDefinition]) {
    for task in tasks {
        if !task_requires_browser_verification(task)
            || task.verification_intents.len() != 1
            || task.verification_intents[0]
                .preferred_evidence
                .iter()
                .chain(task.verification_intents[0].acceptable_evidence.iter())
                .any(|evidence| matches!(evidence, VerificationEvidence::BrowserAutomation))
        {
            continue;
        }
        task.verification_intents[0]
            .acceptable_evidence
            .push(VerificationEvidence::BrowserAutomation);
    }
}

fn validate_browser_verification_assignments(
    tasks: &[TaskDefinition],
) -> Vec<delivery_core::RepairIssue> {
    tasks
        .iter()
        .filter(|task| task_requires_explicit_browser_owner(task))
        .filter(|task| {
            !task.verification_intents.iter().any(|intent| {
                intent
                    .preferred_evidence
                    .iter()
                    .chain(intent.acceptable_evidence.iter())
                    .any(|evidence| matches!(evidence, VerificationEvidence::BrowserAutomation))
            })
        })
        .map(|task| {
            issue(
                "TASKPLAN_BROWSER_VERIFICATION_REQUIRED",
                "tasks[].verificationIntents[].acceptableEvidence",
                &format!(
                    "Task {} owns browser-verifiable UI scope. Mark the verification intent that proves rendered or interactive behavior with browser_automation; keep build, lint, unit-only, API-only, and static intents unchanged.",
                    task.task_id
                ),
                Some(&task.group_id),
            )
        })
        .collect()
}

fn task_requires_explicit_browser_owner(task: &TaskDefinition) -> bool {
    let explicitly_requests_browser_evidence = task.verification_intents.iter().any(|intent| {
        intent
            .preferred_evidence
            .iter()
            .chain(intent.acceptable_evidence.iter())
            .any(|evidence| matches!(evidence, VerificationEvidence::BrowserAutomation))
    });
    let owns_browser_suite_setup = matches!(task.task_kind, TaskKind::VerificationIncrement)
        && task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::AddOrUpdateTests | ImplementationAction::AddOrUpdateConfig
            )
        });
    explicitly_requests_browser_evidence || owns_browser_suite_setup
}

fn materialize_browser_quality_closure(
    groups: &mut Vec<TaskPlanGroup>,
    tasks: &mut Vec<TaskDefinition>,
    source_profiles: Vec<BrowserVerificationProfile>,
) -> Vec<BrowserVerificationProfile> {
    if source_profiles.is_empty() {
        return Vec::new();
    }

    const PREFERRED_TASK_ID: &str = "task-browser-quality-closure";
    const PREFERRED_GROUP_ID: &str = "group-browser-quality-closure";
    let task_id = next_unique_identifier(
        PREFERRED_TASK_ID,
        tasks.iter().map(|task| task.task_id.as_str()),
    );
    let group_id = next_unique_identifier(
        PREFERRED_GROUP_ID,
        groups.iter().map(|group| group.group_id.as_str()),
    );
    let source_task_indices = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| (task.task_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut checks = Vec::new();
    let mut verification_intents = Vec::new();
    let mut surface_refs = BTreeSet::new();
    let mut workflow_refs = BTreeSet::new();
    let mut region_refs = BTreeSet::new();
    let mut action_refs = BTreeSet::new();
    let mut state_refs = BTreeSet::new();
    let mut quality_rule_refs = BTreeSet::new();
    let mut reference_plan = BTreeMap::<String, ReferenceLoadPlanItem>::new();
    let mut has_business_flow_mode = false;
    let mut has_rendered_inspection_mode = false;
    let mut runner_source = BrowserRunnerSource::LoomManaged;
    let mut installation_id = None;

    for profile in source_profiles {
        has_business_flow_mode |= profile.mode == BrowserVerificationMode::BusinessFlow;
        has_rendered_inspection_mode |= profile.mode == BrowserVerificationMode::RenderedInspection;
        runner_source = profile.runner_source;
        installation_id = installation_id.or(profile.installation_id.clone());
        surface_refs.extend(profile.surface_refs);
        workflow_refs.extend(profile.workflow_refs);
        region_refs.extend(profile.region_refs);
        action_refs.extend(profile.action_refs);
        state_refs.extend(profile.state_refs);
        quality_rule_refs.extend(profile.quality_rule_refs);
        for item in profile.reference_load_plan {
            reference_plan.entry(item.path.clone()).or_insert(item);
        }
        let Some(task_index) = source_task_indices.get(&profile.task_id).copied() else {
            continue;
        };
        let source_task = &mut tasks[task_index];
        for source_verification_id in profile.verification_ids {
            let Some(intent) = source_task
                .verification_intents
                .iter_mut()
                .find(|intent| intent.verification_id == source_verification_id)
            else {
                continue;
            };
            let closure_verification_id = format!(
                "verify-browser-{}-{}",
                normalized_identifier(&source_task.task_id),
                normalized_identifier(&source_verification_id)
            );
            let intent_required = profile.checks.iter().any(|check| {
                check.source_verification_id == source_verification_id
                    && check.enforcement == BrowserEvidenceEnforcement::Required
            });
            verification_intents.push(VerificationIntent {
                verification_id: closure_verification_id.clone(),
                acceptance_refs: Vec::new(),
                requirement_detail_refs: Vec::new(),
                behavior: intent.behavior.clone(),
                preferred_evidence: intent_required
                    .then_some(VerificationEvidence::BrowserAutomation)
                    .into_iter()
                    .collect(),
                acceptable_evidence: vec![VerificationEvidence::BrowserAutomation],
            });
            intent
                .preferred_evidence
                .retain(|evidence| *evidence != VerificationEvidence::BrowserAutomation);
            intent
                .acceptable_evidence
                .retain(|evidence| *evidence != VerificationEvidence::BrowserAutomation);
            if intent.preferred_evidence.is_empty() && intent.acceptable_evidence.is_empty() {
                intent
                    .acceptable_evidence
                    .push(VerificationEvidence::AutomatedTest);
            }
            checks.extend(
                profile
                    .checks
                    .iter()
                    .filter(|check| check.source_verification_id == source_verification_id)
                    .cloned()
                    .map(|mut check| {
                        check.verification_id = closure_verification_id.clone();
                        check
                    }),
            );
        }
    }
    if checks.is_empty() {
        return Vec::new();
    }

    let mode = if has_business_flow_mode {
        BrowserVerificationMode::BusinessFlow
    } else if has_rendered_inspection_mode {
        BrowserVerificationMode::RenderedInspection
    } else {
        BrowserVerificationMode::SuiteSetup
    };
    let mut implementation_actions = vec![ImplementationAction::AddOrUpdateTests];
    if runner_source != BrowserRunnerSource::ExistingProject {
        implementation_actions.push(ImplementationAction::AddOrUpdateConfig);
    }
    tasks.push(TaskDefinition {
        task_id: task_id.clone(),
        group_id: group_id.clone(),
        title: "Verify browser quality closure".to_string(),
        task_kind: TaskKind::BrowserQualityClosure,
        implementation_actions,
        implementation_obligations: Vec::new(),
        objective: "Create or adapt the task-scoped browser checks and close required rendered, interaction, and workflow evidence.".to_string(),
        depends_on: Vec::new(),
        scope_refs: Vec::new(),
        acceptance_refs: Vec::new(),
        requirement_detail_refs: Vec::new(),
        write_boundary: TaskWriteBoundary {
            forbidden_paths: vec![".loom".to_string()],
            artifact_refs: TaskArtifactRefs::default(),
        },
        verification_intents: verification_intents.clone(),
        concept_refs: Vec::new(),
        concept_responsibilities: Vec::new(),
        concept_verification_intents: Vec::new(),
        frontend_experience_requirement: None,
        runtime_delivery_requirement: None,
        engineering_quality_requirement_refs: Vec::new(),
        architecture_quality_requirement_refs: Vec::new(),
        api_contract_requirement_refs: Vec::new(),
        code_quality_requirement_refs: Vec::new(),
    });
    let dependency_group_ids = groups
        .iter()
        .filter(|group| !group.task_ids.is_empty())
        .map(|group| group.group_id.clone())
        .collect::<Vec<_>>();
    groups.push(TaskPlanGroup {
        group_id: group_id.clone(),
        title: "Browser quality closure".to_string(),
        objective: "Close the phase browser evidence after implementation and runtime delivery are complete.".to_string(),
        depends_on: dependency_group_ids,
        scope_refs: Vec::new(),
        acceptance_refs: Vec::new(),
        task_ids: vec![task_id.clone()],
    });

    vec![BrowserVerificationProfile {
        profile_id: format!(
            "browser-quality-closure-{}",
            normalized_identifier(&task_id)
        ),
        task_id,
        mode,
        runner_source,
        installation_id,
        verification_ids: verification_intents
            .into_iter()
            .map(|intent| intent.verification_id)
            .collect(),
        surface_refs: surface_refs.into_iter().collect(),
        workflow_refs: workflow_refs.into_iter().collect(),
        region_refs: region_refs.into_iter().collect(),
        action_refs: action_refs.into_iter().collect(),
        state_refs: state_refs.into_iter().collect(),
        quality_rule_refs: quality_rule_refs.into_iter().collect(),
        checks,
        reference_load_plan: reference_plan.into_values().collect(),
    }]
}

fn normalized_identifier(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    value.trim_matches('-').to_string()
}

fn next_unique_identifier<'a>(preferred: &str, existing: impl Iterator<Item = &'a str>) -> String {
    let existing = existing.collect::<BTreeSet<_>>();
    if !existing.contains(preferred) {
        return preferred.to_string();
    }
    for suffix in 2.. {
        let candidate = format!("{preferred}-{suffix}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("unbounded closure identifier suffix search should always return")
}

fn transitive_group_dependencies(groups: &[TaskPlanGroup], group_id: &str) -> BTreeSet<String> {
    let by_id = groups
        .iter()
        .map(|group| (group.group_id.clone(), group))
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut stack = by_id
        .get(group_id)
        .map(|group| group.depends_on.clone())
        .unwrap_or_default();
    while let Some(dependency) = stack.pop() {
        if visited.insert(dependency.clone()) {
            if let Some(group) = by_id.get(&dependency) {
                stack.extend(group.depends_on.clone());
            }
        }
    }
    visited
}

fn validate_ref_list(
    refs: &[String],
    allowed: &BTreeSet<String>,
    code: &str,
    field_path: &str,
    target_id: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    for value in refs {
        if !allowed.contains(value) {
            issues.push(issue(
                code,
                field_path,
                "TaskPlan reference must come from request allowedRefs.",
                Some(target_id),
            ));
        }
    }
}

fn allowed_set(allowed_refs: &Value, key: &str) -> BTreeSet<String> {
    allowed_refs
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn has_group_cycle(groups: &[TaskPlanGroup]) -> bool {
    let by_id = groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    fn visit<'a>(
        id: &'a str,
        by_id: &BTreeMap<&'a str, &'a TaskPlanGroup>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        if let Some(group) = by_id.get(id) {
            for dep in &group.depends_on {
                if visit(dep, by_id, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(id);
        visited.insert(id);
        false
    }
    let mut visited = BTreeSet::new();
    for id in by_id.keys().copied() {
        if visit(id, &by_id, &mut BTreeSet::new(), &mut visited) {
            return true;
        }
    }
    false
}

fn has_cycle(tasks: &[TaskDefinition]) -> bool {
    let by_id = tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    fn visit<'a>(
        id: &'a str,
        by_id: &BTreeMap<&'a str, &'a TaskDefinition>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        if let Some(task) = by_id.get(id) {
            for dep in &task.depends_on {
                if visit(dep, by_id, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(id);
        visited.insert(id);
        false
    }
    let mut visited = BTreeSet::new();
    for id in by_id.keys().copied() {
        if visit(id, &by_id, &mut BTreeSet::new(), &mut visited) {
            return true;
        }
    }
    false
}

fn requirement_detail_transfer(
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) -> Value {
    let detail_coverage = aac
        .detail_coverage
        .iter()
        .map(|entry| {
            (
                entry.detail_id.clone(),
                serde_json::to_value(entry).unwrap_or(Value::Null),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let requirement_items = pgc
        .requirement_details
        .items
        .iter()
        .filter(|item| item.required_for_current_phase)
        .map(|item| {
            let coverage = detail_coverage.get(&item.detail_id);
            let coverage_status = coverage
                .and_then(|value| value.get("coverageStatus"))
                .cloned()
                .unwrap_or_else(|| Value::String("uncovered".to_string()));
            let artifact_hints = coverage
                .and_then(|value| value.get("artifactRefs"))
                .map(compact_requirement_artifact_hints)
                .unwrap_or_else(|| json!({}));
            let summary = if item.summary.trim().is_empty() {
                item.title.clone()
            } else {
                item.summary.clone()
            };
            json!([
                item.detail_id,
                item.kind,
                item.priority,
                item.impact_tags,
                item.quality,
                item.lifecycle_stage,
                item.scope_refs,
                item.acceptance_refs,
                item.concept_refs,
                item.frontend_refs,
                coverage_status,
                summary,
                artifact_hints,
                coverage
                    .and_then(|value| value.get("reason"))
                    .cloned()
                    .unwrap_or(Value::Null)
            ])
        })
        .collect::<Vec<_>>();
    json!({
        "authority": "planning_generation_contract_plus_architecture_artifact_contract",
        "requirementDetailAssignment": {
            "itemEncoding": "row_array",
            "itemColumns": [
                "detailId",
                "kind",
                "priority",
                "impactTags",
                "quality",
                "lifecycleStage",
                "scopeRefs",
                "acceptanceRefs",
                "conceptRefs",
                "frontendRefs",
                "coverageStatus",
                "summary",
                "artifactRefHints",
                "coverageReason"
            ],
            "items": requirement_items,
            "assignmentRule": "Every item with coverageStatus=covered must be assigned to at least one task.requirementDetailRefs entry using its detailId.",
            "verificationRule": "Every assigned covered detail must be referenced by at least one verificationIntents[].requirementDetailRefs entry that proves the concrete behavior.",
            "verificationSubsetRule": "Every verificationIntents[].requirementDetailRefs entry must also be present in the same parent task.requirementDetailRefs.",
            "insufficientAacRule": "If a required detail has coverageStatus other than covered because AAC lacks a taskable artifact, write blocked output with blockedReasonCode AAC_INSUFFICIENT instead of inventing vague tasks.",
            "artifactRefHintRule": "artifactRefHints are compact routing hints for task grouping. Use architectureDetails, acceptanceDetails, and businessFlowDetails as the authoritative source for full object shape and behavior."
        },
        "currentPhaseScope": {
            "includedRefs": pgc.phase_scope.included.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
            "deferredRefs": pgc.phase_scope.deferred.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
            "excludedRefs": pgc.phase_scope.excluded.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
            "authority": "sourceRefs.planningGenerationContractRef#/phaseScope"
        },
        "acceptanceDetails": compact_acceptance_details(&pgc.phase_scope.acceptance_candidates),
        "businessFlowDetails": compact_business_flow_details(&pgc.planning_inputs.business_flows),
        "objectOperationDetailRules": {
            "taskAssignmentRule": "Task objectives and verification intents must preserve concrete objects, operations, fields, states, blocking rules, and feedback when present.",
            "evidenceRule": "TaskResult must be able to show which concrete behavior was implemented or verified."
        },
        "architectureDetails": {
            "modules": compact_artifact_values(&aac.modules, &["moduleId", "name", "responsibilities", "layer", "scopeRefs", "acceptanceRefs"]),
            "applicationInteractions": compact_application_interactions(aac),
            "entities": compact_artifact_array(aac.data_model.get("entities"), &["entityId", "name", "fields", "relationships", "scopeRefs", "acceptanceRefs"]),
            "interfaces": aac.interfaces.iter().map(compact_api_interface_for_task_plan).collect::<Vec<_>>(),
            "userFlows": compact_artifact_values(&aac.user_flows, &["flowId", "name", "kind", "steps", "actorRefs", "scopeRefs", "acceptanceRefs"]),
            "stateMachines": compact_artifact_values(&aac.state_machines, &["stateMachineId", "name", "states", "transitions", "scopeRefs", "acceptanceRefs"]),
            "frontendOperationPathDetails": frontend_operation_path_details(aac),
            "architectureQuality": compact_architecture_quality(aac)
        },
        "workflowClosureRequirements": workflow_closure_requirements(aac),
        "conceptRefs": {
            "deliveryConceptGlossaryRef": pgc.context_refs.delivery_concept_glossary_ref,
            "phaseConceptGroundingRef": pgc.context_refs.phase_concept_grounding_ref
        },
        "taskPlanningFieldMapping": {
            "taskObjective": "Name the concrete business object, rule, flow, state, UI, API, operation path, blocking detail, or feedback detail the task owns.",
            "taskRequirementDetailRefs": "Use the detailId column from requirementDetailAssignment.items row arrays.",
            "frontendExperienceRequirement": "Use when the task owns UI surfaces, workflows, states, bindings, or operation paths.",
            "runtimeDeliveryRequirement": "Use when the task touches build, start, runtime entry, static serving, generated artifacts, or runtime surface."
        }
    })
}

fn compact_acceptance_details(values: &[contracts::AcceptanceCandidate]) -> Vec<Value> {
    values
        .iter()
        .map(|acceptance| {
            json!({
                "id": acceptance.id,
                "statement": acceptance.statement,
                "capabilityRefs": acceptance.capability_refs,
                "sourceRefs": acceptance.source_refs,
                "priority": acceptance.priority
            })
        })
        .collect()
}

fn compact_business_flow_details(values: &[Value]) -> Vec<Value> {
    values
        .iter()
        .map(|flow| {
            compact_artifact_value(flow, &["id", "name", "actors", "capabilityRefs", "summary"])
        })
        .collect()
}

fn compact_architecture_quality(aac: &ArchitectureArtifactContract) -> Value {
    json!({
        "decisions": aac.architecture_quality.decisions.iter().map(|decision| json!({
            "decisionId": &decision.decision_id,
            "category": &decision.category,
            "decision": &decision.decision,
            "ownerArtifactRefs": &decision.owner_artifact_refs,
            "verificationHints": &decision.verification_hints
        })).collect::<Vec<_>>(),
        "nfrs": aac.architecture_quality.nfrs.iter().map(|nfr| json!({
            "nfrId": &nfr.nfr_id,
            "category": &nfr.category,
            "target": &nfr.target,
            "measurement": &nfr.measurement,
            "ownerArtifactRefs": &nfr.owner_artifact_refs,
            "verificationStrategy": &nfr.verification_strategy
        })).collect::<Vec<_>>(),
        "risks": aac.architecture_quality.risks.iter().map(|risk| json!({
            "riskId": &risk.risk_id,
            "category": &risk.category,
            "impact": &risk.impact,
            "mitigation": &risk.mitigation,
            "ownerArtifactRefs": &risk.owner_artifact_refs,
            "verificationHints": &risk.verification_hints
        })).collect::<Vec<_>>()
    })
}

fn compact_artifact_values(values: &[Value], keys: &[&str]) -> Vec<Value> {
    values
        .iter()
        .map(|value| compact_artifact_value(value, keys))
        .collect()
}

fn compact_artifact_array(value: Option<&Value>, keys: &[&str]) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| compact_artifact_value(item, keys))
                    .collect()
            })
            .unwrap_or_default(),
    )
}

fn compact_artifact_value(value: &Value, keys: &[&str]) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut compact = serde_json::Map::new();
    for key in keys {
        if let Some(value) = object.get(*key).filter(|value| !value.is_null()) {
            compact.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(compact)
}

fn compact_application_interactions(aac: &ArchitectureArtifactContract) -> Vec<Value> {
    aac.engineering_boundary
        .pointer("/applicationInteractions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|interaction| {
            let mut compact = serde_json::Map::new();
            for key in [
                "interactionId",
                "providerApplicationRef",
                "consumerApplicationRefs",
                "providerModuleRef",
                "interactionType",
                "protocol",
                "interfaceRefs",
                "qualityTraits",
                "scopeRefs",
                "acceptanceRefs",
            ] {
                if let Some(value) = interaction.get(key).filter(|value| !value.is_null()) {
                    compact.insert(key.to_string(), value.clone());
                }
            }
            Value::Object(compact)
        })
        .collect()
}

fn compact_requirement_artifact_hints(artifact_refs: &Value) -> Value {
    let mut hints = serde_json::Map::new();
    for key in [
        "modules",
        "entities",
        "interfaces",
        "userFlows",
        "stateMachines",
        "constraints",
        "acceptanceMatrix",
        "frontendActions",
        "frontendDataViews",
        "frontendOperationPaths",
    ] {
        insert_string_array_hint(&mut hints, artifact_refs, key, key);
    }
    let fields = string_array_at(artifact_refs, "fields");
    if !fields.is_empty() {
        hints.insert("fieldRefs".to_string(), json!(fields));
    }
    Value::Object(hints)
}

fn insert_string_array_hint(
    hints: &mut serde_json::Map<String, Value>,
    source: &Value,
    source_key: &str,
    target_key: &str,
) {
    let values = string_array_at(source, source_key);
    if !values.is_empty() {
        hints.insert(target_key.to_string(), json!(values));
    }
}

fn frontend_experience_required(aac: &ArchitectureArtifactContract) -> bool {
    aac.frontend_experience
        .as_ref()
        .and_then(|value| value.get("required"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn frontend_ui_surface_decision_contract(aac: &ArchitectureArtifactContract) -> Option<&Value> {
    aac.frontend_experience
        .as_ref()
        .and_then(|frontend| frontend.get("uiSurfaceDecisionContract"))
}

fn frontend_experience_requirement_template(aac: &ArchitectureArtifactContract) -> Value {
    if !frontend_experience_required(aac) {
        return Value::Null;
    }
    let Some(frontend) = aac.frontend_experience.as_ref() else {
        return Value::Null;
    };
    let mut requirement = json!({
        "frontendExperienceRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience",
        "experienceLevel": frontend
            .get("experienceLevel")
            .and_then(Value::as_str)
            .unwrap_or("production_internal_product"),
        "mustSatisfy": true,
        "uiTaskScope": {
            "source": "AAC frontendExperience.uiSurfaceRegistry plus frontend surfaces, dataViews, actions, and operationPaths",
            "selectionRule": "For each frontend task, select only the surfaces, data views, actions, operation paths, states, backend/API bindings, and ownership dimensions owned by that task. Do not copy unrelated UI surfaces into the task.",
            "ownershipDimensionEnum": UI_OWNERSHIP_DIMENSION_VALUES,
            "ownershipDimensionRule": "Choose dimensions from the current task's actual UI responsibility: surface, data_view, action, state, layout, visual_system, content_boundary, integration_feedback. ownershipDimensions are not a task-splitting strategy; use them to describe what this business task owns.",
            "ownershipDimensions": ["surface", "state", "visual_system", "content_boundary"],
            "surfacesInScope": ["current-task uiSurfaceRegistry surface object"],
            "dataViewsInScope": ["current-task frontendExperience.dataViews object"],
            "actionsInScope": ["current-task frontendExperience.actions object"],
            "operationPathsInScope": ["current-task frontendExperience.operationPaths object"],
            "frontendBackendBindings": ["current-task binding between UI action/path and AAC interface when known"],
            "stateExpectation": ["loading", "success", "error", "empty", "business_blocking"],
            "regionsInScope": ["current-task uiSurfaceDecisionContract.regionModel object"],
            "actionsInContract": ["current-task uiSurfaceDecisionContract.actionModel object"],
            "statesInContract": ["current-task uiSurfaceDecisionContract.stateModel object"],
            "qualityRulesInScope": ["current-task uiSurfaceDecisionContract.qualityRules object"],
            "layoutBaseline": "current-task layout baseline from AAC UI contract",
            "informationModel": "current-task information model from AAC UI contract",
            "contentBoundary": "current-task content boundary from AAC UI contract",
            "bindingContract": "current-task UI/API binding contract from AAC"
        }
    });
    if frontend.get("uiSurfaceRegistry").is_some() {
        requirement["uiSurfaceRegistryRef"] = json!(
            "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceRegistry"
        );
    }
    if let Some(surface_contract) = frontend.get("uiSurfaceDecisionContract") {
        requirement["uiSurfaceDecisionContractRef"] = json!(
            "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract"
        );
        requirement["uiTaskScope"]["contractRef"] = json!(
            "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract"
        );
        requirement["uiTaskScope"]["patternDecision"] = surface_contract
            .get("patternDecision")
            .cloned()
            .unwrap_or(Value::Null);
    }
    requirement
}

fn normalize_ui_task_scope_contract(requirement: &mut Value, surface_contract: &Value) {
    let Some(scope) = requirement
        .get_mut("uiTaskScope")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    scope.insert(
        "source".to_string(),
        json!("MCP-derived AAC uiSurfaceDecisionContract projection"),
    );
    scope.insert("contractRef".to_string(), json!("sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract"));
    scope.insert(
        "patternDecision".to_string(),
        surface_contract
            .get("patternDecision")
            .cloned()
            .unwrap_or(Value::Null),
    );
    scope.insert(
        "regionsInScope".to_string(),
        selected_scope_objects(
            scope.get("regionsInScope"),
            surface_contract,
            "regionModel",
            "regionId",
        ),
    );
    scope.insert(
        "actionsInContract".to_string(),
        selected_scope_objects(
            scope.get("actionsInContract"),
            surface_contract,
            "actionModel",
            "actionId",
        ),
    );
    scope.insert(
        "statesInContract".to_string(),
        selected_scope_objects(
            scope.get("statesInContract"),
            surface_contract,
            "stateModel",
            "state",
        ),
    );
    scope.insert(
        "qualityRulesInScope".to_string(),
        selected_scope_objects(
            scope.get("qualityRulesInScope"),
            surface_contract,
            "qualityRules",
            "ruleId",
        ),
    );
    let region_ids = scope_object_ids(scope, "regionsInScope", "regionId");
    let action_ids = scope_object_ids(scope, "actionsInContract", "actionId");
    let state_ids = scope_object_ids(scope, "statesInContract", "state");
    let quality_rule_ids = scope_object_ids(scope, "qualityRulesInScope", "ruleId");
    scope.insert(
        "layoutBaseline".to_string(),
        scoped_layout_baseline(surface_contract.get("layoutModel"), &region_ids),
    );
    scope.insert(
        "informationModel".to_string(),
        scoped_information_model(surface_contract.get("informationModel")),
    );
    scope.insert(
        "contentBoundary".to_string(),
        scoped_content_boundary(surface_contract.get("contentBoundary")),
    );
    scope.insert(
        "bindingContract".to_string(),
        scoped_binding_contract(
            surface_contract.get("semanticFacts"),
            &region_ids,
            &action_ids,
            &state_ids,
            &quality_rule_ids,
        ),
    );
}

fn scope_object_ids(
    scope: &serde_json::Map<String, Value>,
    key: &str,
    id_key: &str,
) -> BTreeSet<String> {
    scope
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(id_key).and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn scoped_layout_baseline(layout: Option<&Value>, region_ids: &BTreeSet<String>) -> Value {
    let Some(layout) = layout.and_then(Value::as_object) else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    for key in [
        "density",
        "shell",
        "responsiveModel",
        "navigationPlacement",
        "contentPlacement",
    ] {
        if let Some(value) = layout.get(key) {
            result.insert(key.to_string(), value.clone());
        }
    }
    if let Some(primary) = layout.get("primaryWorkRegionId") {
        if primary.as_str().is_none_or(|id| region_ids.contains(id)) {
            result.insert("primaryWorkRegionId".to_string(), primary.clone());
        }
    }
    Value::Object(result)
}

fn scoped_information_model(information: Option<&Value>) -> Value {
    let Some(information) = information.and_then(Value::as_object) else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    for key in ["primaryObjects", "fields", "scanOrder", "secondaryObjects"] {
        if let Some(value) = information.get(key) {
            result.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(result)
}

fn scoped_content_boundary(content: Option<&Value>) -> Value {
    let Some(content) = content.and_then(Value::as_object) else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    for key in [
        "forbiddenUserVisibleContent",
        "allowedUserVisibleContent",
        "requiredBusinessCopy",
        "copyTone",
    ] {
        if let Some(value) = content.get(key) {
            result.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(result)
}

fn scoped_binding_contract(
    facts: Option<&Value>,
    region_ids: &BTreeSet<String>,
    action_ids: &BTreeSet<String>,
    state_ids: &BTreeSet<String>,
    quality_rule_ids: &BTreeSet<String>,
) -> Value {
    let Some(facts) = facts.and_then(Value::as_object) else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    for key in [
        "userJobs",
        "informationShapes",
        "operationModels",
        "riskFactors",
        "navigationModel",
        "devicePosture",
        "productMode",
    ] {
        if let Some(value) = facts.get(key) {
            result.insert(key.to_string(), value.clone());
        }
    }
    result.insert("regionRefs".to_string(), json!(region_ids));
    result.insert("actionRefs".to_string(), json!(action_ids));
    result.insert("stateRefs".to_string(), json!(state_ids));
    result.insert("qualityRuleRefs".to_string(), json!(quality_rule_ids));
    Value::Object(result)
}

fn selected_scope_objects(
    requested: Option<&Value>,
    contract: &Value,
    array_key: &str,
    id_key: &str,
) -> Value {
    let allowed = contract
        .get(array_key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let requested_ids = requested
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str()
                .map(str::to_string)
                .or_else(|| item.get(id_key).and_then(Value::as_str).map(str::to_string))
        })
        .collect::<BTreeSet<_>>();
    let selected = allowed
        .into_iter()
        .filter(|item| {
            item.get(id_key)
                .and_then(Value::as_str)
                .is_some_and(|id| requested_ids.contains(id))
        })
        .collect();
    Value::Array(selected)
}

fn contract_string_ids(
    surface_contract: &Value,
    contract_pointer: &str,
    contract_id_key: &str,
) -> BTreeSet<String> {
    surface_contract
        .pointer(contract_pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(contract_id_key).and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn frontend_operation_path_details(aac: &ArchitectureArtifactContract) -> Value {
    let Some(frontend) = aac.frontend_experience.as_ref() else {
        return Value::Null;
    };
    let surfaces = frontend
        .get("surfaces")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    compact_artifact_value(
                        item,
                        &[
                            "surfaceId",
                            "name",
                            "kind",
                            "purpose",
                            "dataViewRefs",
                            "actionRefs",
                            "stateRefs",
                            "operationPathRefs",
                            "workflowRefs",
                            "interfaceRefs",
                            "regionRefs",
                            "scopeRefs",
                            "acceptanceRefs",
                        ],
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let data_views = frontend
        .get("dataViews")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    compact_artifact_value(
                        item,
                        &[
                            "viewId",
                            "name",
                            "purpose",
                            "objectRefs",
                            "fieldRefs",
                            "interfaceRefs",
                            "stateRefs",
                            "surfaceRefs",
                            "scopeRefs",
                            "acceptanceRefs",
                        ],
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let actions = frontend
        .get("actions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    compact_artifact_value(
                        item,
                        &[
                            "actionId",
                            "name",
                            "label",
                            "purpose",
                            "interfaceRefs",
                            "operationPathRefs",
                            "stateRefs",
                            "surfaceRefs",
                            "workflowRefs",
                            "scopeRefs",
                            "acceptanceRefs",
                        ],
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let operation_paths = frontend
        .get("operationPaths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    compact_artifact_value(
                        item,
                        &[
                            "pathId",
                            "name",
                            "purpose",
                            "surfaceRef",
                            "workflowRef",
                            "dataViewRefs",
                            "actionRefs",
                            "stateRefs",
                            "interfaceRefs",
                            "steps",
                            "scopeRefs",
                            "acceptanceRefs",
                        ],
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let decision_contract = frontend
        .get("uiSurfaceDecisionContract")
        .map(compact_ui_surface_decision_contract)
        .unwrap_or(Value::Null);
    json!({
        "required": frontend.get("required").cloned().unwrap_or(Value::Bool(false)),
        "experienceLevel": frontend.get("experienceLevel").cloned().unwrap_or(Value::Null),
        "surfaces": surfaces,
        "dataViews": data_views,
        "actions": actions,
        "operationPaths": operation_paths,
        "uiSurfaceRegistry": compact_ui_surface_registry(frontend.get("uiSurfaceRegistry")),
        "uiSurfaceDecisionContract": decision_contract,
        "sourceRefs": frontend.get("sourceRefs").cloned().unwrap_or(Value::Null),
        "fullSource": "sourceRefs.architectureArtifactContractRef#/frontendExperience"
    })
}

fn compact_ui_surface_registry(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    if let Some(items) = value.as_array() {
        return Value::Array(
            items
                .iter()
                .map(|item| {
                    compact_artifact_value(
                        item,
                        &[
                            "surfaceId",
                            "name",
                            "regionRefs",
                            "dataViewRefs",
                            "actionRefs",
                            "stateRefs",
                            "operationPathRefs",
                            "workflowRefs",
                        ],
                    )
                })
                .collect(),
        );
    }
    compact_artifact_value(
        value,
        &[
            "surfaceRefs",
            "regionRefs",
            "dataViewRefs",
            "actionRefs",
            "stateRefs",
            "operationPathRefs",
            "workflowRefs",
        ],
    )
}

fn compact_ui_surface_decision_contract(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut compact = serde_json::Map::new();
    if let Some(value) = object.get("contractId").filter(|value| !value.is_null()) {
        compact.insert("contractId".to_string(), value.clone());
    }
    if let Some(value) = object.get("patternDecision") {
        compact.insert(
            "patternDecision".to_string(),
            compact_artifact_value(
                value,
                &[
                    "mode",
                    "knownPattern",
                    "primaryKnownPattern",
                    "customPattern",
                ],
            ),
        );
    }
    for (key, id_key, fields) in [
        (
            "regionModel",
            "regionId",
            [
                "regionId",
                "role",
                "label",
                "purpose",
                "layout",
                "dataViewRefs",
                "actionRefs",
                "stateRefs",
                "required",
            ]
            .as_slice(),
        ),
        (
            "actionModel",
            "actionId",
            [
                "actionId",
                "kind",
                "label",
                "purpose",
                "interfaceRefs",
                "operationPathRefs",
                "stateRefs",
                "feedbackRefs",
                "required",
            ]
            .as_slice(),
        ),
        (
            "stateModel",
            "state",
            [
                "state",
                "description",
                "trigger",
                "feedback",
                "blocking",
                "required",
            ]
            .as_slice(),
        ),
        (
            "qualityRules",
            "ruleId",
            ["ruleId", "category", "requirement", "verification", "scope"].as_slice(),
        ),
    ] {
        compact.insert(
            key.to_string(),
            compact_surface_contract_array(object.get(key), id_key, fields),
        );
    }
    if let Some(value) = object.get("layoutModel") {
        compact.insert(
            "layoutModel".to_string(),
            compact_artifact_value(
                value,
                &[
                    "density",
                    "shell",
                    "responsiveModel",
                    "navigationPlacement",
                    "contentPlacement",
                    "primaryWorkRegionId",
                ],
            ),
        );
    }
    if let Some(value) = object.get("informationModel") {
        compact.insert(
            "informationModel".to_string(),
            compact_artifact_value(
                value,
                &["primaryObjects", "fields", "scanOrder", "secondaryObjects"],
            ),
        );
    }
    if let Some(value) = object.get("contentBoundary") {
        compact.insert(
            "contentBoundary".to_string(),
            compact_artifact_value(
                value,
                &[
                    "forbiddenUserVisibleContent",
                    "allowedUserVisibleContent",
                    "requiredBusinessCopy",
                    "copyTone",
                ],
            ),
        );
    }
    if let Some(value) = object.get("semanticFacts") {
        compact.insert(
            "semanticFacts".to_string(),
            compact_artifact_value(
                value,
                &[
                    "userJobs",
                    "informationShapes",
                    "operationModels",
                    "riskFactors",
                    "navigationModel",
                    "devicePosture",
                    "productMode",
                ],
            ),
        );
    }
    if let Some(value) = object.get("sourceRefs").filter(|value| !value.is_null()) {
        compact.insert("sourceRefs".to_string(), value.clone());
    }
    Value::Object(compact)
}

fn compact_surface_contract_array(value: Option<&Value>, id_key: &str, fields: &[&str]) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|item| {
                let mut compact = compact_artifact_value(item, fields);
                if compact.get(id_key).is_none() {
                    if let Some(id) = item.get(id_key) {
                        compact[id_key] = id.clone();
                    }
                }
                compact
            })
            .collect(),
    )
}

pub(crate) fn workflow_closure_requirements(aac: &ArchitectureArtifactContract) -> Vec<Value> {
    let Some(frontend) = aac.frontend_experience.as_ref() else {
        return vec![];
    };
    if frontend
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        != true
    {
        return vec![];
    }

    let flow_by_id = aac
        .user_flows
        .iter()
        .filter_map(|flow| string_at(flow, "flowId").map(|id| (id, flow)))
        .collect::<BTreeMap<_, _>>();
    let interface_by_id = aac
        .interfaces
        .iter()
        .filter_map(|interface| string_at(interface, "interfaceId").map(|id| (id, interface)))
        .collect::<BTreeMap<_, _>>();
    let mut surface_refs_by_flow: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for surface in array_at(frontend, "surfaces") {
        let Some(surface_id) = string_at(surface, "surfaceId") else {
            continue;
        };
        for workflow_ref in string_array_at(surface, "workflowRefs") {
            surface_refs_by_flow
                .entry(workflow_ref)
                .or_default()
                .push(surface_id.clone());
        }
    }

    for operation_path in array_at(frontend, "operationPaths") {
        let Some(workflow_ref) = string_at(operation_path, "workflowRef") else {
            continue;
        };
        let surface_ref = string_at(operation_path, "surfaceRef");
        let refs = surface_refs_by_flow.entry(workflow_ref).or_default();
        if let Some(surface_ref) = surface_ref {
            refs.push(surface_ref);
        }
    }

    let mut requirements = Vec::new();
    for (workflow_ref, surface_refs) in surface_refs_by_flow {
        let Some(flow) = flow_by_id.get(workflow_ref.as_str()) else {
            continue;
        };
        if let Some(kind) = string_at(flow, "kind") {
            if kind != "user_interaction" {
                continue;
            }
        }
        let steps = array_at(flow, "happyPath");
        if steps.is_empty() {
            continue;
        }

        let operation_paths = array_at(frontend, "operationPaths")
            .into_iter()
            .filter(|operation_path| {
                string_at(operation_path, "workflowRef").as_deref() == Some(workflow_ref.as_str())
                    || string_at(operation_path, "surfaceRef")
                        .map(|surface_ref| surface_refs.iter().any(|id| id == &surface_ref))
                        .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        let operation_path_refs = unique_strings(
            operation_paths
                .iter()
                .filter_map(|operation_path| string_at(operation_path, "pathId"))
                .collect(),
        );
        let data_view_refs = unique_strings(
            operation_paths
                .iter()
                .flat_map(|operation_path| string_array_at(operation_path, "dataViewRefs"))
                .collect(),
        );
        let action_refs = unique_strings(
            operation_paths
                .iter()
                .flat_map(|operation_path| string_array_at(operation_path, "actionRefs"))
                .collect(),
        );

        for step in steps {
            let candidate_interface_refs = string_at(step, "interactionRef")
                .into_iter()
                .collect::<Vec<_>>();
            let executable_interfaces = candidate_interface_refs
                .iter()
                .filter_map(|interface_ref| interface_by_id.get(interface_ref.as_str()).copied())
                .filter(|interface| {
                    is_executable_interface(interface) && has_interface_shape(interface)
                })
                .collect::<Vec<_>>();
            if executable_interfaces.is_empty() {
                continue;
            }
            let interface_refs = unique_strings(
                executable_interfaces
                    .iter()
                    .filter_map(|interface| string_at(interface, "interfaceId"))
                    .collect(),
            );
            let step_id = string_at(step, "stepId").unwrap_or_else(|| "step".to_string());
            requirements.push(json!({
                "closureId": format!("closure:{workflow_ref}:{step_id}"),
                "workflowRef": workflow_ref.clone(),
                "workflowName": string_at(flow, "name").unwrap_or_else(|| workflow_ref.clone()),
                "surfaceRefs": unique_strings(surface_refs.clone()),
                "operationPathRefs": operation_path_refs.clone(),
                "dataViewRefs": data_view_refs.clone(),
                "actionRefs": action_refs.clone(),
                "moduleRefs": string_array_at(flow, "moduleRefs"),
                "acceptanceRefs": string_array_at(flow, "acceptanceRefs"),
                "interfaceRefs": interface_refs,
                "stateMachineRefs": unique_strings(string_array_at(step, "stateMachineRefs")),
                "stepRefs": [step_id.clone()],
                "entry": flow.get("entry").cloned().unwrap_or(Value::Null),
                "derivation": {
                    "source": "aac_frontend_surface_userflow_interface",
                    "rule": "Generated from AAC frontendExperience surfaces or operationPaths, structured user-flow happy-path steps, and executable interfaces with request/response shape."
                },
                "requiredDataBindingMode": "wired",
                "satisfiedDataBindingModes": ["wired"],
                "staticModePolicy": "not_satisfied",
                "knownGapPolicy": "not_satisfied_when_required_closure",
                "requiredEvidence": [
                    "user_action",
                    "declared_interface_invocation",
                    "state_or_persistence_change",
                    "success_or_blocking_feedback"
                ],
                "interfaces": executable_interfaces
                    .iter()
                    .map(|interface| {
                        json!({
                            "interfaceId": string_at(interface, "interfaceId").unwrap_or_default(),
                            "name": string_at(interface, "name").unwrap_or_default(),
                            "type": string_at(interface, "type").unwrap_or_default(),
                            "role": string_at(interface, "role"),
                            "method": string_at(interface, "method"),
                            "path": string_at(interface, "path"),
                            "requestSchema": interface.get("requestSchema").cloned().unwrap_or(Value::Array(vec![])),
                            "responseSchema": interface.get("responseSchema").cloned().unwrap_or(Value::Array(vec![])),
                            "errorSchema": interface.get("errorSchema").cloned().unwrap_or(Value::Array(vec![]))
                        })
                    })
                    .collect::<Vec<_>>()
            }));
        }
    }
    requirements
}

fn allowed_refs(
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) -> Value {
    let detail_ids = pgc
        .requirement_details
        .items
        .iter()
        .map(|item| item.detail_id.clone())
        .collect::<Vec<_>>();
    json!({
        "scopeRefs": pgc.phase_scope.included.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "deferredScopeRefs": pgc.phase_scope.deferred.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "excludedScopeRefs": pgc.phase_scope.excluded.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "acceptanceRefs": pgc.phase_scope.acceptance_candidates.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "requirementDetailIds": detail_ids,
        "moduleRefs": ids_from_values(&aac.modules, "moduleId"),
        "entityRefs": ids_from_value_array(&aac.data_model, "/entities", "entityId"),
        "interfaceRefs": ids_from_values(&aac.interfaces, "interfaceId"),
        "userFlowRefs": ids_from_values(&aac.user_flows, "flowId"),
        "stateMachineRefs": ids_from_values(&aac.state_machines, "machineId"),
        "decisionRefs": aac.architecture_quality.decisions.iter().map(|item| item.decision_id.clone()).collect::<Vec<_>>(),
        "nfrRefs": aac.architecture_quality.nfrs.iter().map(|item| item.nfr_id.clone()).collect::<Vec<_>>(),
        "riskRefs": aac.architecture_quality.risks.iter().map(|item| item.risk_id.clone()).collect::<Vec<_>>()
    })
}

fn array_at<'a>(value: &'a Value, key: &str) -> Vec<&'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn string_array_at(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn is_executable_interface(interface: &Value) -> bool {
    matches!(
        string_at(interface, "type").as_deref(),
        Some("http_api" | "service_method" | "cli_command" | "event" | "job" | "external_adapter")
    )
}

fn is_http_api_interface(interface: &Value) -> bool {
    string_at(interface, "type").as_deref() == Some("http_api")
        || string_at(interface, "kind").as_deref() == Some("http_api")
}

fn has_interface_shape(interface: &Value) -> bool {
    interface
        .get("requestSchema")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
        && interface
            .get("responseSchema")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
}

fn generation_rules(aac: &ArchitectureArtifactContract, code_quality_seed: &Value) -> Value {
    json!({
        "groupedOutputRules": [
            "First write outputContract.outlineFile.",
            "Then write one group file per outline.groups[].groupId using outputContract.groupFilePattern.",
            "Do not write the accepted final TaskPlan artifact."
        ],
        "scopeAndReferenceRules": [
            "Use only refs from allowedRefs.",
            "Do not implement deferred or excluded scope.",
            "Keep next-phase seeds in deferred scope or next-phase preview only; do not create executable tasks for them."
        ],
        "writeBoundaryRules": [
            "Every task.writeBoundary.forbiddenPaths must include .loom.",
            "Project source edits happen only during TaskExecution, not during TaskPlan generation."
        ],
        "verificationEvidenceRules": [
            "verificationIntents must use enumRefs.verificationEvidence.",
            "Each implementation task must have at least one verification intent.",
            "Every covered current-phase detailId from the detailId column in contextProjection.requirementDetailTransfer.requirementDetailAssignment.items must appear in at least one task.requirementDetailRefs.",
            "Every covered current-phase detailId assigned to a task must appear in at least one verificationIntents[].requirementDetailRefs that proves the concrete behavior.",
            "Every verificationIntents[].requirementDetailRefs item must also be present in the same parent task.requirementDetailRefs; do not reference a detail only inside a verification intent.",
            "Prefer the smallest stable verification signal that proves the user-visible behavior or contract obligation.",
            "Avoid broad snapshots or weak no-op checks as the primary verification evidence.",
            "For a task with browser-owned UI surfaces, workflows, actions, states, rendered viewport rules, or Playwright suite setup, mark each verification intent that truly requires a browser with browser_automation in acceptableEvidence. Do not attach browser_automation to build, lint, unit-only, API-only, or static verification intents."
        ],
        "detailOwnershipRules": {
            "source": "contextProjection.requirementDetailTransfer.requirementDetailAssignment.items",
            "assignmentRule": "For every covered current-phase detail row, choose the task that owns the matching artifactRefHints through task.writeBoundary.artifactRefs. Use kind, impactTags, and lifecycleStage to break ties between implementation tasks.",
            "ownerTaskBoundary": "Assign business requirement details to implementation owner tasks, not broad verification, runtime closure, or handoff tasks.",
            "verificationRule": "After assigning a detail to a task, include the same detailId in that task's verificationIntents[].requirementDetailRefs.",
            "acceptNormalization": "loom.taskPlanAcceptFile deterministically assigns each covered detail to exactly one owner task (the canonical implementation owner) and removes duplicate detail, scope, acceptance, and artifact ownership from verification or closure tasks.",
            "artifactOwnershipNormalization": "When candidate tasks repeat the same accepted artifact ref, Loom keeps the detail-derived owner; otherwise it selects one deterministic implementation owner before deriving quality and API requirements."
        },
        "conceptGroundingRules": {
            "phaseConceptGroundingRef": "sourceRefs.phaseConceptGroundingRef",
            "rule": "Bind high-risk business concepts when the current task owns their rule, state, field, or operation meaning."
        },
        "frontendExperienceRules": {
            "required": aac.frontend_experience.as_ref().and_then(|value| value.get("required")).and_then(Value::as_bool).unwrap_or(false),
            "requirementTemplate": "outputContract.frontendExperienceRequirementTemplate",
            "uiSurfaceDecisionContractSource": "outputContract.frontendExperienceRequirementTemplate.uiSurfaceDecisionContractRef",
            "rule": "When frontendExperience is required, UI responsibilities must be visible in task objective, verification intents, and frontendExperienceRequirement.",
            "taskScopeRule": "MCP derives one frontendExperienceRequirement.uiTaskScope from AAC uiSurfaceDecisionContract and task-owned refs. Select only the current task's regions, actions, states, quality rules, surfaces, data views, operation paths, bindings, layout, information model, and content boundary.",
            "ownershipDimensionRule": "ownershipDimensions describe what this business task owns; they are not a task-splitting strategy. Use surface, data_view, action, state, layout, visual_system, content_boundary, and integration_feedback only when the task changes that concern."
        },
        "workflowClosureRules": {
            "derivationAuthority": "AAC frontendExperience + userFlows + executable interfaces",
            "requirementSource": "contextProjection.requirementDetailTransfer.workflowClosureRequirements",
            "appliesWhen": "Only when workflowClosureRequirements is non-empty.",
            "taskAssignmentRule": "Assign each closure requirement to at least one executable task whose artifact refs include workflowRef and every interfaceRef.",
            "taskCoverageShape": "The task must own the user action, declared interface invocation, state or persistence change, and success or blocking feedback evidence.",
            "resultExpectation": "TaskResult frontendExperienceSelfCheck must reference closureRequirementIds and cannot mark static or unwired UI as satisfied.",
            "repairRule": "If a closure requirement is unassigned, repair TaskPlan assignment rather than routing back to AAC when AAC already declares the workflow and interfaces.",
            "rules": [
                "Use contextProjection.requirementDetailTransfer.workflowClosureRequirements as the exact workflow closure requirement list.",
                "Task implementationActions should include wire_reference_in_api_or_ui when the task closes a frontend workflow.",
                "Verification intents for closure tasks should accept automated_test or runtime_api_check evidence."
            ]
        },
        "runtimeDeliveryRules": {
            "status": aac.runtime_delivery.as_ref().and_then(|value| value.get("status")).cloned().unwrap_or(Value::String("not_applicable".to_string())),
            "rule": "Runtime-affecting tasks must carry runtimeDeliveryRequirement; final runtime closure is required when runtimeDelivery.status=modified. Loom derives the accepted RuntimeDelivery reference, contract fields, and check ids during accept, so the candidate must not invent those machine fields.",
            "closureTaskRule": "When outputContract.runtimeDeliveryClosureTaskTemplate is present, create exactly one task with taskKind=runtime_delivery_closure. Declare the closure task and its group placement; Loom materializes the complete runtimeDeliveryRequirement from the accepted RuntimeDeliveryContract even when the candidate omits that machine-owned object.",
            "closureGroupRule": "The runtime_delivery_closure task must be the only task in its group, that group must be the final outline.groups entry, no other group may depend on it, and its dependsOn must point to the previous group or groups that make runtime-affecting work transitively complete.",
            "closureTaskDependencyRule": "Do not make the runtime_delivery_closure task depend directly on tasks from other groups; express cross-group ordering through the closure group dependsOn."
        },
        "engineeringQualityRules": {
            "persistenceMappingRequirementSource": "outputContract.engineeringQualityRequirementTemplate",
            "appliesWhen": "Use this only when the task creates or changes persistence, entities, migrations, repositories, or backend API/business logic that reads or mutates persisted entities.",
            "notFor": "Do not attach persistence mapping requirements to pure frontend UI tasks, even when they call APIs.",
            "acceptNormalization": "loom.taskPlanAcceptFile deterministically materializes top-level engineeringQualityRequirements and task engineeringQualityRequirementRefs; do not duplicate full quality requirements inside every task.",
            "taskPlanningRule": "For applicable tasks, verificationIntents should prove storage schema, data-access mapping, DTO/API contract, query/sort/filter fields, and same-provider persistence behavior stay aligned."
        },
        "codeReferenceRules": {
            "authority": "MCP derives task-scoped code reference groups from the intersection of TechnicalBaseline stack signals and task-owned implementation capabilities; agents must not author referenceLoadPlan or reference group paths.",
            "frameworkCapabilityActions": {
                "frontendNavigation": ["create_or_update_frontend_navigation"],
                "reactiveClientFlow": ["implement_reactive_client_flow"],
                "sharedClientState": ["implement_shared_client_state"],
                "frontendPerformance": ["optimize_frontend_performance"],
                "serverRenderedComponent": ["implement_server_rendered_component"],
                "serverMutation": ["implement_server_mutation"],
                "frontendFrameworkVersionFeature": ["implement_frontend_framework_version_feature"],
                "mobilePlatformBehavior": ["implement_mobile_platform_behavior"],
                "clientStorage": ["implement_client_storage"],
                "languageVersionFeature": ["implement_language_version_feature"],
                "genericTypeAbstraction": ["implement_generic_type_abstraction"],
                "dependencyAbstraction": ["implement_dependency_abstraction"],
                "moduleStructure": ["refactor_module_structure"],
                "runtimePerformance": ["optimize_runtime_performance"],
                "security": ["implement_authentication_or_authorization"],
                "async": ["implement_async_processing"],
                "cache": ["implement_cache_policy"],
                "externalIntegration": ["implement_external_service_integration"],
                "resilience": ["implement_resilience_policy"],
                "serviceRoutingOrDiscovery": ["configure_service_routing_or_discovery"],
                "observability": ["implement_observability"],
                "frameworkMigration": ["migrate_framework_implementation"]
            },
            "frameworkCapabilityRule": "Use these implementationActions only when the task really owns the corresponding capability. They are cross-framework implementation facts; MCP maps them to the framework selected by TechnicalBaseline.",
            "structuredCapabilitySources": {
                "frontendNavigation": "Use create_or_update_frontend_navigation only for the task that owns route definitions, route parameters, guards, resolvers, deep links, nested navigation, or equivalent framework navigation configuration.",
                "reactiveClientFlow": "Use implement_reactive_client_flow only for a task that owns stream cancellation, ordering, fan-out, subscription lifecycle, or other reactive client behavior beyond a simple one-shot binding.",
                "sharedClientState": "Use implement_shared_client_state only for the task that owns a selected shared state container, reducer/store, effects, selectors, or a cross-surface client state lifecycle. Local component state does not use this action.",
                "frontendPerformance": "Use optimize_frontend_performance only for a task that owns a measurable client rendering, rebuild, list, image, animation, memory, startup, or interaction-latency risk and its verification evidence.",
                "serverRenderedComponent": "Use implement_server_rendered_component only for a task that owns a framework server-rendered component boundary, server/client composition, streaming, hydration, or serializable handoff.",
                "serverMutation": "Use implement_server_mutation only for a task that owns a framework server-side form/action mutation, its authorization and validation, result state, and cache/readback reconciliation.",
                "frontendFrameworkVersionFeature": "Use implement_frontend_framework_version_feature only for a frontend task that intentionally owns an API available from the accepted framework version and whose implementation or fallback differs from the repository's baseline patterns.",
                "mobilePlatformBehavior": "Use implement_mobile_platform_behavior only for a task that owns iOS/Android-specific behavior, native APIs or modules, permissions, safe-area/keyboard/status-bar integration, gestures, or hardware-back semantics.",
                "clientStorage": "Use implement_client_storage only for a task that owns client-side persistence, secure device storage, persisted drafts/preferences/session state, hydration, migration, expiry, or identity-scoped cleanup.",
                "languageVersionFeature": "Use implement_language_version_feature only for a task that intentionally owns language-standard/version APIs whose implementation or fallback differs from the repository baseline; include build configuration ownership when the declared compiler/language target changes.",
                "genericTypeAbstraction": "Use implement_generic_type_abstraction only for a task that owns a reusable generic/template/type-parameter contract with real consumers, constraints, and verification; do not infer it from prose examples or ordinary collection use.",
                "dependencyAbstraction": "Use implement_dependency_abstraction only for a task that owns a consumer-facing interface/protocol/trait/adapter seam with concrete consumers, implementations, lifecycle/error semantics, and verification; do not create it only to mirror one implementation.",
                "moduleStructure": "Use refactor_module_structure only for a task that owns module/package/project boundaries, entry-point placement, import visibility, workspace/module files, build tags, generated-code ownership, or dependency direction and verifies affected build targets.",
                "runtimePerformance": "Use optimize_runtime_performance only for a task that owns a measured CPU, allocation, memory-layout, throughput, latency, binary-size, or runtime resource bottleneck and its benchmark/profile plus correctness evidence.",
                "security": "A task owning an interface authPolicy or an application interaction with required/optional authRequirement, or an architecture-quality security ref uses implement_authentication_or_authorization. deferred_with_risk records a risk without creating current-phase authentication work.",
                "async": "A task owning an event/job application interaction uses implement_async_processing.",
                "cache": "A task owning an explicit application-cache decision, NFR, or implementation boundary uses implement_cache_policy. HTTP cachePolicy, validators, and conditional requests remain API/web behavior and do not activate an application-cache reference.",
                "externalIntegration": "A task owning an external_adapter application interaction uses implement_external_service_integration.",
                "resilience": "A task owning an application-interaction retry operationalPolicy or an applicable availability/reliability decision uses implement_resilience_policy. An HTTP interface retryPolicy alone describes caller-visible API behavior and does not activate internal retries.",
                "observability": "A task owning an observability/operability architecture-quality ref uses implement_observability.",
                "serviceRoutingOrDiscovery": "Use configure_service_routing_or_discovery only for an explicitly accepted service-routing, discovery, gateway, or centralized-config capability.",
                "frameworkMigration": "Use migrate_framework_implementation only when the task explicitly owns behavior parity or an accepted contract transition from an existing framework implementation."
            },
            "structuredCapabilityOwnershipRule": "Assign a capability action only to the task whose artifactRefs own the matching interface, provider module, decision, NFR, or risk. Do not infer capability actions from framework availability or generic task prose.",
            "unmappedStackRule": "When codeQualitySeed.unmappedSignals is non-empty, preserve the accepted stack and repository conventions; do not substitute a nearby language or framework profile.",
            "persistenceActions": {
                "schema": ["create_or_update_entity", "create_or_update_persistence", "create_entity_migration", "create_entity_crud"],
                "query": ["create_entity_repository", "create_entity_crud", "create_or_update_persistence_query", "optimize_persistence_query", "implement_analytical_query"],
                "transaction": ["create_or_update_persistence", "create_entity_repository", "create_entity_crud", "create_or_update_persistence_query", "implement_persistence_transaction"],
                "performance": ["optimize_persistence_query"],
                "analytics": ["implement_analytical_query"],
                "persistenceTest": ["add_or_update_persistence_tests"]
            },
            "dialectRule": "When the accepted persistence provider is MySQL or PostgreSQL, MCP adds only the provider overlay matching the assigned persistence subject. MariaDB is not silently treated as MySQL.",
            "mybatisPlusRule": "When the accepted dataAccess selection is MyBatis Plus, MCP emits the mybatisplus reference group only for task-owned persistence capabilities. Agents must not select MyBatis-Plus references, JPA references, MyBatis-Flex, or plain MyBatis guidance themselves.",
            "nonSelectionRule": "Do not attach framework references solely because a language or framework is present in TechnicalBaseline. Do not attach SQL or provider overlays to pure API, controller, frontend, or generic test tasks. Generic add_or_update_tests does not select database references."
        },
        "architectureQualityRules": {
            "requirementSource": "contextProjection.requirementDetailTransfer.architectureDetails.architectureQuality plus task-owned modules and interfaces",
            "architectureQualitySource": "contextProjection.requirementDetailTransfer.architectureDetails.architectureQuality",
            "referenceRule": "Do not write task.writeBoundary.artifactRefs.decisions, nfrs, or risks and do not inline full ADR, NFR, or risk objects inside tasks. Loom derives those refs from the accepted architecture ownerArtifactRefs and the task-owned modules/interfaces.",
            "assignmentRule": "Tasks must identify the modules and interfaces they own. Loom uses those artifact refs to assign every applicable architecture decision, NFR, and risk deterministically.",
            "acceptNormalization": "loom.taskPlanAcceptFile derives architecture quality artifact refs, top-level architectureQualityRequirements, and task architectureQualityRequirementRefs from accepted architecture ownership.",
            "verificationRule": "Assigned tasks must include verificationIntents whose summaries can prove the referenced architecture decision, NFR, or risk mitigation was respected."
        },
        "apiContractRules": {
            "required": aac.interfaces.iter().any(is_http_api_interface),
            "requirementSource": "outputContract.apiContractRequirementTemplate",
            "interfaceSource": "contextProjection.apiInterfaces (copied from the accepted AAC interfaces)",
            "exposureSource": "contextProjection.apiContract (copied from the accepted AAC API contract)",
            "assignmentRule": "Loom assigns each accepted interface id to one implementation owner in task.writeBoundary.artifactRefs.interfaces. Client, integration, and verification tasks receive the same contract in consumedInterfaces and the derived API requirement; they must not claim duplicate write ownership.",
            "implementationRule": "API tasks must preserve request schema, response schema, status codes, error schema, auth policy, and pagination policy declared by the AAC interface.",
            "verificationRule": "API tasks should include verification intents that prove at least the declared success path and important business or validation error path. Collection endpoints should also prove declared pagination/filter behavior.",
            "nonDuplicationRule": "Do not inline full API requirements inside every task; use interface refs and the generated task apiContractRequirementRefs."
        },
        "codeQualityRules": {
            "required": code_quality_seed.get("required").and_then(Value::as_bool).unwrap_or(false),
            "seedSource": "codeQualitySeed",
            "requirementSource": "outputContract.codeQualityRequirementTemplate",
            "assignmentRule": "loom.taskPlanAcceptFile derives task codeQualityRequirementRefs from TechnicalBaseline stack signals and task scope; do not inline full code quality requirements inside every task.",
            "referenceRule": "Use codeQualitySeed.codeStackSignals only to describe accurate task ownership. Do not write codeQualityRequirementRefs, reference groups, or reference paths; Loom derives task-scoped requirements and referenceLoadPlan during accept.",
            "nonDuplicationRule": "Do not repeat language or framework best-practice prose in task objective or verification intents; use codeQualityRequirementRefs and TaskResult codeQualityEvidence."
        }
    })
}

fn code_quality_requirement_template(code_quality_seed: &Value) -> Value {
    if code_quality_seed.is_null()
        || !code_quality_seed
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Value::Null;
    }
    json!({
        "requirementId": "code-quality-{taskId}",
        "kind": "language_implementation_quality",
        "codeStackSignalSource": "codeQualitySeed.codeStackSignals",
        "derivationAuthority": "loom.taskPlanAcceptFile derives reference groups and referenceLoadPlan from codeStackSignals plus accepted task ownership.",
        "verificationObligations": code_quality_verification_obligations(),
        "taskRefRule": "Loom attaches the generated requirement through codeQualityRequirementRefs during accept; agents must not write that field or inline language/framework reference prose inside tasks."
    })
}

fn engineering_quality_requirement_template(
    baseline: &contracts::TechnicalBaselineContract,
) -> Value {
    let stack_signals = stack_signals_from_baseline(&baseline.stack);
    if !has_persistence_quality_signal(&stack_signals) {
        return Value::Null;
    }
    json!({
        "requirementId": "eqr-persistence-mapping-001",
        "kind": "persistence_mapping",
        "stackSignals": stack_signals,
        "alignmentTargets": persistence_alignment_targets(),
        "riskFieldKinds": persistence_risk_field_kinds(),
        "verificationObligations": persistence_verification_obligations(),
        "taskRefRule": "Loom attaches this generated requirement through engineeringQualityRequirementRefs during accept; agents must not write that field or duplicate the full object in each task."
    })
}

fn api_contract_requirement_template(interfaces: &[Value]) -> Value {
    let interface_refs = interfaces
        .iter()
        .filter_map(|interface| string_at(interface, "interfaceId"))
        .collect::<Vec<_>>();
    if interface_refs.is_empty() {
        return Value::Null;
    }
    json!({
        "requirementId": "api-contract-current-001",
        "kind": "api_contract",
        "interfaceRefs": interface_refs,
        "verificationObligations": api_contract_verification_obligations(),
        "taskRefRule": "Loom attaches generated requirements through apiContractRequirementRefs during accept for API implementation, client binding, integration, and verification tasks; agents must not write that field or duplicate full API requirements inside every task."
    })
}

fn compact_api_interface_for_task_plan(interface: &Value) -> Value {
    json!({
        "interfaceId": interface.get("interfaceId").cloned().unwrap_or(Value::Null),
        "name": interface.get("name").cloned().unwrap_or(Value::Null),
        "resource": interface.get("resource").cloned().unwrap_or(Value::Null),
        "operationKind": interface.get("operationKind").cloned().unwrap_or(Value::Null),
        "method": interface.get("method").cloned().unwrap_or(Value::Null),
        "path": interface.get("path").cloned().unwrap_or(Value::Null),
        "statusCodes": interface.get("statusCodes").cloned().unwrap_or(Value::Null),
        "requestFieldCount": interface
            .get("requestSchema")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "responseFieldCount": interface
            .get("responseSchema")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "scopeRefs": interface.get("scopeRefs").cloned().unwrap_or_else(|| json!([])),
        "acceptanceRefs": interface
            .get("acceptanceRefs")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "requirementDetailRefs": interface
            .get("requirementDetailRefs")
            .cloned()
            .unwrap_or_else(|| json!([]))
    })
}

fn normalize_architecture_quality_requirements(
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
) -> Vec<ArchitectureQualityRequirement> {
    if aac.architecture_quality.decisions.is_empty()
        && aac.architecture_quality.nfrs.is_empty()
        && aac.architecture_quality.risks.is_empty()
    {
        for task in tasks {
            task.architecture_quality_requirement_refs.clear();
        }
        return vec![];
    }
    let mut requirements = Vec::new();
    let mut requirement_ids_by_task = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.iter() {
        let refs = &task.write_boundary.artifact_refs;
        let decision_refs = refs.decisions.clone();
        let nfr_refs = refs.nfrs.clone();
        let risk_refs = refs.risks.clone();
        if decision_refs.is_empty() && nfr_refs.is_empty() && risk_refs.is_empty() {
            continue;
        }
        let requirement_id = format!("aqr-{}", task.task_id);
        requirement_ids_by_task
            .entry(task.task_id.clone())
            .or_default()
            .push(requirement_id.clone());
        requirements.push(ArchitectureQualityRequirement {
            requirement_id,
            kind: "architecture_quality".to_string(),
            applies_to_task_ids: vec![task.task_id.clone()],
            decision_refs,
            nfr_refs,
            risk_refs,
            verification_obligations: vec![
                "Use task.verificationIntents as verification id source.".to_string(),
                "Record architectureQualityEvidence for every referenced architecture quality requirement."
                    .to_string(),
                "Summarize how changed code respected the referenced decision, NFR, or risk mitigation."
                    .to_string(),
            ],
        });
    }
    for task in tasks {
        task.architecture_quality_requirement_refs = requirement_ids_by_task
            .remove(&task.task_id)
            .unwrap_or_default();
    }
    requirements
}

fn normalize_architecture_quality_artifact_refs(
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
) {
    for task in tasks {
        let task_modules = task
            .write_boundary
            .artifact_refs
            .modules
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let task_interfaces = task
            .write_boundary
            .artifact_refs
            .interfaces
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        task.write_boundary.artifact_refs.decisions = aac
            .architecture_quality
            .decisions
            .iter()
            .filter(|decision| {
                owner_refs_intersect(
                    &decision.owner_artifact_refs.modules,
                    &decision.owner_artifact_refs.interfaces,
                    &task_modules,
                    &task_interfaces,
                )
            })
            .map(|decision| decision.decision_id.clone())
            .collect();
        task.write_boundary.artifact_refs.nfrs = aac
            .architecture_quality
            .nfrs
            .iter()
            .filter(|nfr| {
                owner_refs_intersect(
                    &nfr.owner_artifact_refs.modules,
                    &nfr.owner_artifact_refs.interfaces,
                    &task_modules,
                    &task_interfaces,
                )
            })
            .map(|nfr| nfr.nfr_id.clone())
            .collect();
        task.write_boundary.artifact_refs.risks = aac
            .architecture_quality
            .risks
            .iter()
            .filter(|risk| {
                owner_refs_intersect(
                    &risk.owner_artifact_refs.modules,
                    &risk.owner_artifact_refs.interfaces,
                    &task_modules,
                    &task_interfaces,
                ) || risk
                    .owner_artifact_refs
                    .decisions
                    .iter()
                    .any(|decision_id| {
                        aac.architecture_quality.decisions.iter().any(|decision| {
                            decision.decision_id == *decision_id
                                && owner_refs_intersect(
                                    &decision.owner_artifact_refs.modules,
                                    &decision.owner_artifact_refs.interfaces,
                                    &task_modules,
                                    &task_interfaces,
                                )
                        })
                    })
                    || risk.owner_artifact_refs.nfrs.iter().any(|nfr_id| {
                        aac.architecture_quality.nfrs.iter().any(|nfr| {
                            nfr.nfr_id == *nfr_id
                                && owner_refs_intersect(
                                    &nfr.owner_artifact_refs.modules,
                                    &nfr.owner_artifact_refs.interfaces,
                                    &task_modules,
                                    &task_interfaces,
                                )
                        })
                    })
            })
            .map(|risk| risk.risk_id.clone())
            .collect();
    }
}

fn owner_refs_intersect(
    owner_modules: &[String],
    owner_interfaces: &[String],
    task_modules: &BTreeSet<String>,
    task_interfaces: &BTreeSet<String>,
) -> bool {
    owner_modules
        .iter()
        .any(|owner| task_modules.contains(owner))
        || owner_interfaces
            .iter()
            .any(|owner| task_interfaces.contains(owner))
}

fn normalize_api_contract_requirements(
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
    allowed_refs: &Value,
    baseline: &contracts::TechnicalBaselineContract,
) -> Vec<ApiContractRequirement> {
    let http_api_refs = aac
        .interfaces
        .iter()
        .filter(|interface| is_http_api_interface(interface))
        .filter_map(|interface| string_at(interface, "interfaceId"))
        .collect::<BTreeSet<_>>();
    if http_api_refs.is_empty() {
        for task in tasks {
            task.api_contract_requirement_refs.clear();
        }
        return vec![];
    }
    let allowed_interface_refs = allowed_set(allowed_refs, "interfaceRefs");
    let mut requirements = Vec::new();
    let mut requirement_ids_by_task = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.iter_mut() {
        let mut interface_refs = task
            .write_boundary
            .artifact_refs
            .all_interfaces()
            .into_iter()
            .filter(|interface_ref| http_api_refs.contains(interface_ref))
            .collect::<Vec<_>>();
        if interface_refs.is_empty() && task_can_consume_api_contract(task) {
            interface_refs = aac
                .user_flows
                .iter()
                .filter(|flow| {
                    task.write_boundary
                        .artifact_refs
                        .user_flows
                        .iter()
                        .any(|flow_ref| {
                            string_at(flow, "flowId").as_deref() == Some(flow_ref.as_str())
                        })
                })
                .flat_map(|flow| {
                    array_at(flow, "happyPath")
                        .into_iter()
                        .filter_map(|step| string_at(step, "interactionRef"))
                })
                .filter(|interface_ref| {
                    http_api_refs.contains(interface_ref)
                        && allowed_interface_refs.contains(interface_ref)
                })
                .collect::<Vec<_>>();
            interface_refs.sort();
            interface_refs.dedup();
            // This is a consumer projection. The accepted owner boundary is
            // intentionally not widened by API requirement derivation.
        }
        if interface_refs.is_empty() || !task_uses_api_contract(task) {
            continue;
        }
        if !task_owns_api_contract(task) {
            for interface_ref in &interface_refs {
                push_unique(
                    &mut task.write_boundary.artifact_refs.consumed_interfaces,
                    interface_ref.clone(),
                );
            }
            task.write_boundary
                .artifact_refs
                .interfaces
                .retain(|interface_ref| !interface_refs.contains(interface_ref));
        }
        let requirement_id = format!("api-contract-{}", task.task_id);
        requirement_ids_by_task
            .entry(task.task_id.clone())
            .or_default()
            .push(requirement_id.clone());
        requirements.push(ApiContractRequirement {
            requirement_id,
            kind: "api_contract".to_string(),
            applies_to_task_ids: vec![task.task_id.clone()],
            interface_refs: interface_refs.clone(),
            security_profile_refs: interface_refs
                .iter()
                .filter_map(|interface_ref| {
                    aac.interfaces
                        .iter()
                        .find(|interface| {
                            string_at(interface, "interfaceId").as_deref() == Some(interface_ref)
                        })
                        .and_then(|interface| {
                            interface
                                .pointer("/authPolicy/securityProfileRef")
                                .and_then(Value::as_str)
                                .filter(|profile_ref| !profile_ref.trim().is_empty())
                                .map(str::to_string)
                        })
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            reference_load_plan: api_security_reference_load_plan(aac, &interface_refs, baseline),
            verification_obligations: api_contract_verification_obligations(),
        });
    }
    for task in tasks {
        task.api_contract_requirement_refs = requirement_ids_by_task
            .remove(&task.task_id)
            .unwrap_or_default();
    }
    requirements
}

fn api_security_reference_load_plan(
    aac: &ArchitectureArtifactContract,
    interface_refs: &[String],
    baseline: &contracts::TechnicalBaselineContract,
) -> Vec<ReferenceLoadPlanItem> {
    let profile_mechanisms = baseline
        .security_profiles
        .iter()
        .map(|profile| (profile.profile_id.as_str(), profile.mechanism))
        .collect::<BTreeMap<_, _>>();
    let protected_profiles = aac
        .interfaces
        .iter()
        .filter_map(|interface| {
            interface_refs
                .iter()
                .any(|interface_ref| {
                    string_at(interface, "interfaceId").as_deref() == Some(interface_ref)
                })
                .then(|| {
                    interface
                        .pointer("/authPolicy/required")
                        .and_then(Value::as_str)
                        .filter(|required| matches!(*required, "required" | "optional"))
                        .and_then(|_| interface.pointer("/authPolicy/securityProfileRef"))
                        .and_then(Value::as_str)
                        .and_then(|profile_ref| profile_mechanisms.get(profile_ref).copied())
                })
        })
        .collect::<Vec<_>>();
    if protected_profiles.is_empty() {
        return vec![];
    }
    let mut plan = vec![ReferenceLoadPlanItem {
        ref_id: "tech.api.security".to_string(),
        path: "tech/api/security.md".to_string(),
        reason: "Selected API security contract for protected interfaces.".to_string(),
    }];
    if protected_profiles
        .iter()
        .any(|mechanism| matches!(mechanism, Some(contracts::SecurityMechanism::BearerJwt)))
    {
        plan.push(ReferenceLoadPlanItem {
            ref_id: "tech.api.jwt".to_string(),
            path: "tech/api/jwt.md".to_string(),
            reason: "Selected JWT API contract for interfaces bound to a bearer JWT profile."
                .to_string(),
        });
    }
    plan
}

fn normalize_implementation_obligations(
    baseline: &contracts::TechnicalBaselineContract,
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
) {
    let stack_signals = stack_signals_from_baseline(&baseline.stack);
    for task in tasks {
        let mut obligations = Vec::new();
        let artifacts = &task.write_boundary.artifact_refs;

        for action in &task.implementation_actions {
            let Some((kind, outcome, evidence)) = implementation_obligation_for_action(*action)
            else {
                continue;
            };
            push_task_implementation_obligation(
                &mut obligations,
                task,
                kind,
                outcome,
                evidence,
                artifacts.clone(),
                obligation_source_refs(task, &[baseline.technical_baseline_id.as_str()]),
            );
        }

        if !artifacts.entities.is_empty() {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "entity_contract",
                "Implement the task-owned entities, field invariants, and serialization boundary declared by AAC.",
                vec![VerificationEvidence::AutomatedTest, VerificationEvidence::StaticCheck],
                artifact_refs_for(artifacts, "entities"),
                obligation_source_refs(task, &[]),
            );
        }
        if !artifacts.interfaces.is_empty() {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "interface_contract",
                "Implement the task-owned interfaces with the accepted method, path, schemas, status behavior, and error behavior.",
                vec![VerificationEvidence::AutomatedTest, VerificationEvidence::RuntimeApiCheck],
                artifact_refs_for(artifacts, "interfaces"),
                obligation_source_refs(task, &[]),
            );
        }
        if !artifacts.state_machines.is_empty() {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "state_transition",
                "Implement the task-owned state transitions and reject invalid transitions according to AAC.",
                vec![VerificationEvidence::AutomatedTest, VerificationEvidence::RuntimeApiCheck],
                artifact_refs_for(artifacts, "state_machines"),
                obligation_source_refs(task, &[]),
            );
        }

        if task_directly_owns_persistence_mapping(task) {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "persistence_mapping",
                &format!(
                    "Implement durable storage, data-access mapping, transaction boundaries, and readback for the accepted persistence provider{}.",
                    stack_signals
                        .get("dataAccess")
                        .map(|value| format!(" `{value}`"))
                        .unwrap_or_default()
                ),
                vec![VerificationEvidence::AutomatedTest, VerificationEvidence::RuntimeApiCheck, VerificationEvidence::StaticCheck],
                artifact_refs_for(artifacts, "entities"),
                obligation_source_refs(
                    task,
                    stack_signals
                        .get("dataAccess")
                        .into_iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .as_slice(),
                ),
            );
        }

        let owned_interfaces = aac
            .interfaces
            .iter()
            .filter(|interface| {
                string_at(interface, "interfaceId")
                    .is_some_and(|id| artifacts.interfaces.iter().any(|owned| owned == &id))
            })
            .collect::<Vec<_>>();
        // Security belongs to the interface boundary. A persistence or domain
        // task may touch a module that has secured interactions, but it must not
        // inherit the API authentication obligation merely because the module
        // is shared by that interaction.
        let interaction_requires_security = task_owns_interface_behavior(task)
            && task_owned_application_interactions(aac, task)
                .iter()
                .any(|interaction| interaction_requires_security(interaction));
        if task_has_implementation_action(
            task,
            ImplementationAction::ImplementAuthenticationOrAuthorization,
        ) || owned_interfaces
            .iter()
            .any(|interface| interface_requires_security(interface))
            || interaction_requires_security
        {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "authentication_authorization",
                "Enforce the accepted authentication and authorization policy at the task-owned boundary and verify allowed and denied behavior.",
                vec![VerificationEvidence::AutomatedTest, VerificationEvidence::RuntimeApiCheck, VerificationEvidence::StaticCheck],
                artifact_refs_for(artifacts, "interfaces"),
                obligation_source_refs(task, &[]),
            );
        }

        if task.frontend_experience_requirement.is_some() {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "frontend_experience",
                "Implement the task-owned frontend surfaces, actions, states, and feedback declared by AAC.",
                vec![VerificationEvidence::BrowserAutomation, VerificationEvidence::AutomatedTest],
                artifacts.clone(),
                obligation_source_refs(task, &[]),
            );
        }
        if task
            .runtime_delivery_requirement
            .as_ref()
            .is_some_and(|requirement| requirement.applies_to_this_task)
        {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "runtime_delivery",
                "Implement the task-owned runtime command, configuration, and service binding required by the accepted RuntimeDelivery contract.",
                vec![VerificationEvidence::StaticCheck, VerificationEvidence::RuntimeApiCheck],
                artifacts.clone(),
                obligation_source_refs(task, &[]),
            );
        }

        if !task.code_quality_requirement_refs.is_empty() {
            push_task_implementation_obligation(
                &mut obligations,
                task,
                "reference_alignment",
                "Implement the task using the MCP-selected language and framework references; reading a reference or running a build alone is not implementation evidence.",
                vec![VerificationEvidence::StaticCheck, VerificationEvidence::AutomatedTest],
                artifacts.clone(),
                obligation_source_refs(task, &[]),
            );
        }

        let obligation_snapshot = obligations.clone();
        for (index, obligation) in obligations.iter_mut().enumerate() {
            obligation.verification_ids =
                verification_ids_for_obligation(task, &obligation_snapshot, index);
        }
        obligations.sort_by(|left, right| left.obligation_id.cmp(&right.obligation_id));
        task.implementation_obligations = obligations;
    }
}

fn implementation_obligation_for_action(
    action: ImplementationAction,
) -> Option<(&'static str, &'static str, Vec<VerificationEvidence>)> {
    let evidence = |behavioral: bool| {
        if behavioral {
            vec![
                VerificationEvidence::AutomatedTest,
                VerificationEvidence::RuntimeApiCheck,
            ]
        } else {
            vec![
                VerificationEvidence::StaticCheck,
                VerificationEvidence::AutomatedTest,
            ]
        }
    };
    let result = match action {
        ImplementationAction::CreateOrUpdatePersistence
        | ImplementationAction::CreateEntityRepository
        | ImplementationAction::CreateEntityMigration
        | ImplementationAction::CreateEntityCrud
        | ImplementationAction::CreateOrUpdatePersistenceQuery
        | ImplementationAction::ImplementPersistenceTransaction
        | ImplementationAction::OptimizePersistenceQuery
        | ImplementationAction::ImplementAnalyticalQuery
        | ImplementationAction::AddOrUpdatePersistenceTests => (
            "persistence_mapping",
            "Implement the task-owned persistence operation and its provider-compatible data-access behavior.",
            evidence(true),
        ),
        ImplementationAction::CreateOrUpdateInterface => (
            "interface_contract",
            "Implement the task-owned API interface against the accepted method, schema, status, error, and security contract.",
            evidence(true),
        ),
        ImplementationAction::WireReferenceInApiOrUi => (
            "api_binding",
            "Implement the task-owned API or client binding against the accepted interface contract.",
            evidence(true),
        ),
        ImplementationAction::CreateOrUpdateStateMachine => (
            "state_machine",
            "Implement the task-owned state machine and its invalid-transition behavior.",
            evidence(true),
        ),
        ImplementationAction::CreateOrUpdateBusinessRule => (
            "business_rule",
            "Implement the task-owned business rule and its blocking/error behavior.",
            evidence(true),
        ),
        ImplementationAction::ImplementAuthenticationOrAuthorization => (
            "authentication_authorization",
            "Implement the accepted authentication and authorization policy at the task-owned boundary.",
            evidence(true),
        ),
        ImplementationAction::ImplementAsyncProcessing => (
            "async_processing",
            "Implement the task-owned asynchronous processing, completion, retry, and failure boundary.",
            evidence(true),
        ),
        ImplementationAction::ImplementCachePolicy => (
            "cache_policy",
            "Implement the accepted cache policy, invalidation behavior, and fallback behavior.",
            evidence(true),
        ),
        ImplementationAction::ImplementExternalServiceIntegration => (
            "external_integration",
            "Implement the task-owned external service integration and failure handling.",
            evidence(true),
        ),
        ImplementationAction::ImplementResiliencePolicy => (
            "resilience_policy",
            "Implement the accepted resilience policy and its bounded failure behavior.",
            evidence(true),
        ),
        ImplementationAction::ConfigureServiceRoutingOrDiscovery => (
            "service_routing",
            "Implement the accepted service routing or discovery boundary.",
            evidence(false),
        ),
        ImplementationAction::ImplementObservability => (
            "observability",
            "Implement the task-owned observability boundary without duplicating events across layers.",
            evidence(false),
        ),
        ImplementationAction::ImplementRuntimeDeliveryContract => (
            "runtime_delivery",
            "Implement the task-owned runtime delivery contract.",
            evidence(false),
        ),
        ImplementationAction::ImplementFrontendExperienceContract
        | ImplementationAction::CreateOrUpdateUiFlow
        | ImplementationAction::CreateOrUpdateFrontendNavigation
        | ImplementationAction::ImplementReactiveClientFlow
        | ImplementationAction::ImplementSharedClientState
        | ImplementationAction::OptimizeFrontendPerformance
        | ImplementationAction::ImplementServerRenderedComponent
        | ImplementationAction::ImplementServerMutation
        | ImplementationAction::ImplementFrontendFrameworkVersionFeature
        | ImplementationAction::ImplementMobilePlatformBehavior
        | ImplementationAction::ImplementClientStorage
        | ImplementationAction::CreateEntityAdminPage => (
            "frontend_experience",
            "Implement the task-owned frontend behavior and its user-visible states and feedback.",
            vec![VerificationEvidence::BrowserAutomation, VerificationEvidence::AutomatedTest],
        ),
        ImplementationAction::CreateOrUpdateEntity => (
            "entity_contract",
            "Implement the task-owned entity model and its invariants.",
            evidence(false),
        ),
        ImplementationAction::ImplementEntityLifecycle => (
            "entity_lifecycle",
            "Implement the task-owned entity lifecycle and persistence effects.",
            evidence(true),
        ),
        ImplementationAction::RefactorModuleStructure
        | ImplementationAction::OptimizeRuntimePerformance
        | ImplementationAction::ImplementLanguageVersionFeature
        | ImplementationAction::ImplementGenericTypeAbstraction
        | ImplementationAction::ImplementDependencyAbstraction
        | ImplementationAction::MigrateFrameworkImplementation
        | ImplementationAction::RefactorSupportingCode => (
            "implementation_structure",
            "Implement the task-owned code structure or framework change and keep affected behavior intact.",
            evidence(false),
        ),
        ImplementationAction::AddReferenceField
        | ImplementationAction::ValidateReferenceFormat
        | ImplementationAction::UseFixtureOrMockData
        | ImplementationAction::AddOrUpdateTests
        | ImplementationAction::AddOrUpdateConfig => return None,
    };
    Some(result)
}

fn verification_intent_matches_obligation(
    intent: &VerificationIntent,
    task: &TaskDefinition,
) -> bool {
    intent
        .acceptance_refs
        .iter()
        .any(|reference| task.acceptance_refs.iter().any(|owned| owned == reference))
        || intent.requirement_detail_refs.iter().any(|reference| {
            task.requirement_detail_refs
                .iter()
                .any(|owned| owned == reference)
        })
        || (intent.acceptance_refs.is_empty() && intent.requirement_detail_refs.is_empty())
}

fn verification_ids_for_obligation(
    task: &TaskDefinition,
    obligations: &[TaskImplementationObligation],
    obligation_index: usize,
) -> Vec<String> {
    let obligation = &obligations[obligation_index];
    let mut matching = task
        .verification_intents
        .iter()
        .filter(|intent| {
            verification_intent_matches_obligation(intent, task)
                && verification_intent_matches_scope(intent, obligation)
                && verification_intent_matches_evidence(intent, obligation)
        })
        .map(|intent| intent.verification_id.clone())
        .collect::<Vec<_>>();

    // When a task has no structured refs to distinguish its intents, use the
    // available evidence capability and stable declaration order. A single
    // unscoped intent may legitimately verify multiple obligations; unrelated
    // scoped intents must not be copied into every obligation.
    if matching.is_empty() {
        let compatible = task
            .verification_intents
            .iter()
            .filter(|intent| {
                verification_intent_matches_obligation(intent, task)
                    && verification_intent_matches_evidence(intent, obligation)
            })
            .map(|intent| intent.verification_id.clone())
            .collect::<Vec<_>>();
        matching = if compatible.len() == 1 {
            compatible
        } else {
            compatible
                .get(obligation_index)
                .or_else(|| compatible.last())
                .cloned()
                .into_iter()
                .collect()
        };
    }
    matching.sort();
    matching.dedup();
    matching
}

fn verification_intent_matches_scope(
    intent: &VerificationIntent,
    obligation: &TaskImplementationObligation,
) -> bool {
    let intent_refs = intent
        .acceptance_refs
        .iter()
        .chain(intent.requirement_detail_refs.iter())
        .collect::<BTreeSet<_>>();
    intent_refs.is_empty()
        || intent_refs.iter().any(|reference| {
            obligation
                .source_refs
                .iter()
                .any(|source| source == *reference)
        })
}

fn verification_intent_matches_evidence(
    intent: &VerificationIntent,
    obligation: &TaskImplementationObligation,
) -> bool {
    obligation.acceptable_evidence.iter().any(|expected| {
        intent
            .preferred_evidence
            .iter()
            .chain(intent.acceptable_evidence.iter())
            .any(|actual| actual == expected)
    })
}

fn obligation_source_refs(task: &TaskDefinition, extra: &[&str]) -> Vec<String> {
    task.scope_refs
        .iter()
        .chain(task.acceptance_refs.iter())
        .chain(task.requirement_detail_refs.iter())
        .chain(task.write_boundary.artifact_refs.modules.iter())
        .chain(task.write_boundary.artifact_refs.entities.iter())
        .chain(task.write_boundary.artifact_refs.interfaces.iter())
        .chain(task.write_boundary.artifact_refs.consumed_interfaces.iter())
        .chain(task.write_boundary.artifact_refs.user_flows.iter())
        .chain(task.write_boundary.artifact_refs.state_machines.iter())
        .chain(task.write_boundary.artifact_refs.decisions.iter())
        .chain(task.write_boundary.artifact_refs.nfrs.iter())
        .chain(task.write_boundary.artifact_refs.risks.iter())
        .map(String::as_str)
        .chain(extra.iter().copied())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn push_task_implementation_obligation(
    obligations: &mut Vec<TaskImplementationObligation>,
    task: &TaskDefinition,
    kind: &str,
    required_outcome: &str,
    acceptable_evidence: Vec<VerificationEvidence>,
    artifact_refs: TaskArtifactRefs,
    source_refs: Vec<String>,
) {
    let obligation_id = format!(
        "obligation-{}-{}",
        normalized_identifier(&task.task_id),
        normalized_identifier(kind)
    );
    if obligations
        .iter()
        .any(|obligation| obligation.obligation_id == obligation_id)
    {
        return;
    }
    let primary = obligations.is_empty();
    obligations.push(TaskImplementationObligation {
        obligation_id,
        kind: kind.to_string(),
        source_refs: compact_obligation_source_refs(task, kind, source_refs, primary),
        artifact_refs: compact_obligation_artifact_refs(kind, artifact_refs),
        required_outcome: required_outcome.to_string(),
        required: true,
        acceptable_evidence,
        verification_ids: Vec::new(),
        defer_policy: "must_be_satisfied_before_completed".to_string(),
    });
}

fn compact_obligation_source_refs(
    task: &TaskDefinition,
    kind: &str,
    source_refs: Vec<String>,
    primary: bool,
) -> Vec<String> {
    let relevant_artifacts =
        compact_obligation_artifact_ref_set(kind, &task.write_boundary.artifact_refs);
    let task_refs = task
        .scope_refs
        .iter()
        .chain(task.acceptance_refs.iter())
        .chain(task.requirement_detail_refs.iter())
        .collect::<BTreeSet<_>>();
    source_refs
        .into_iter()
        .filter(|value| {
            if !primary {
                return relevant_artifacts.contains(value);
            }
            task.requirement_detail_refs
                .iter()
                .any(|item| item == value)
                || task.acceptance_refs.iter().any(|item| item == value)
                || task.scope_refs.iter().any(|item| item == value)
                || relevant_artifacts.contains(value)
                || !task_refs.contains(value)
        })
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn compact_obligation_artifact_refs(kind: &str, artifacts: TaskArtifactRefs) -> TaskArtifactRefs {
    let mut compact = TaskArtifactRefs::default();
    match kind {
        "entity_contract" | "persistence_mapping" | "entity_lifecycle" => {
            compact.modules = artifacts.modules;
            compact.entities = artifacts.entities;
            compact.state_machines = artifacts.state_machines;
        }
        "interface_contract"
        | "api_binding"
        | "authentication_authorization"
        | "resilience_policy"
        | "state_transition"
        | "service_routing"
        | "external_integration"
        | "async_processing"
        | "cache_policy" => {
            compact.modules = artifacts.modules;
            compact.interfaces = artifacts.interfaces;
            compact.consumed_interfaces = artifacts.consumed_interfaces;
        }
        "frontend_experience" | "business_rule" => {
            compact.modules = artifacts.modules;
            compact.user_flows = artifacts.user_flows;
            compact.consumed_interfaces = artifacts.consumed_interfaces;
        }
        "state_machine" => {
            compact.modules = artifacts.modules;
            compact.state_machines = artifacts.state_machines;
        }
        _ => compact.modules = artifacts.modules,
    }
    compact
}

fn compact_obligation_artifact_ref_set(
    kind: &str,
    artifacts: &TaskArtifactRefs,
) -> BTreeSet<String> {
    let compact = compact_obligation_artifact_refs(kind, artifacts.clone());
    compact
        .modules
        .into_iter()
        .chain(compact.entities)
        .chain(compact.interfaces)
        .chain(compact.consumed_interfaces)
        .chain(compact.user_flows)
        .chain(compact.state_machines)
        .collect()
}

fn artifact_refs_for(artifacts: &TaskArtifactRefs, field: &str) -> TaskArtifactRefs {
    let mut selected = TaskArtifactRefs::default();
    match field {
        "entities" => selected.entities = artifacts.entities.clone(),
        "interfaces" => selected.interfaces = artifacts.interfaces.clone(),
        "state_machines" => selected.state_machines = artifacts.state_machines.clone(),
        _ => {}
    }
    selected
}

fn normalize_structured_verification_intents(
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
) {
    let persistence_task_ids = persistence_quality_task_ids(tasks)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for task in tasks.iter_mut() {
        let interface_refs = task_interface_refs_for_verification(task);
        let interfaces = aac
            .interfaces
            .iter()
            .filter(|interface| {
                string_at(interface, "interfaceId").is_some_and(|id| interface_refs.contains(&id))
            })
            .collect::<Vec<_>>();

        for interface in interfaces {
            let Some(interface_id) = string_at(interface, "interfaceId") else {
                continue;
            };
            let method = string_at(interface, "method").unwrap_or_else(|| "operation".to_string());
            let path = string_at(interface, "path").unwrap_or_else(|| interface_id.clone());
            let mut obligations = vec![(
                "success",
                format!("Verify the declared success response for {method} {path}."),
            )];
            if matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
                obligations.push((
                    "error",
                    format!("Verify the declared validation or business error response for {method} {path}."),
                ));
            }
            if interface_has_pagination(interface) {
                if task_is_frontend_task(task) {
                    obligations.extend([
                        (
                            "pagination-state",
                            format!("Verify the UI owns loading, current-page, page-size, and empty-page state for {method} {path}."),
                        ),
                        (
                            "pagination-navigation",
                            format!("Verify the UI page navigation action requests the declared page for {method} {path}."),
                        ),
                        (
                            "pagination-filter-reset",
                            format!("Verify changing the declared filter resets the UI page before requesting {method} {path}."),
                        ),
                    ]);
                } else {
                    obligations.push((
                        "pagination",
                        format!("Verify the declared pagination contract for {method} {path}."),
                    ));
                }
            }
            if interface_has_normalization(interface) {
                obligations.push((
                    "normalization",
                    format!("Verify declared input normalization before validation for {method} {path}."),
                ));
            }
            if interface_has_idempotency(interface) {
                obligations.push((
                    "duplicate",
                    format!(
                        "Verify the declared duplicate-submission behavior for {method} {path}."
                    ),
                ));
            }
            if interface_requires_auth(interface) && task_is_frontend_task(task) {
                obligations.push((
                    "permission",
                    format!(
                        "Verify permission denial feedback for the UI binding of {method} {path}."
                    ),
                ));
            }
            if persistence_task_ids.contains(&task.task_id) && is_mutating_interface(&method) {
                obligations.push((
                    "save-failure",
                    format!("Verify the declared persistence or save failure response for {method} {path}."),
                ));
            }
            for (kind, behavior) in obligations {
                let verification_id = format!(
                    "verify-api-{}-{}-{}",
                    normalized_identifier(&task.task_id),
                    normalized_identifier(&interface_id),
                    kind
                );
                if task
                    .verification_intents
                    .iter()
                    .any(|intent| intent.verification_id == verification_id)
                {
                    continue;
                }
                task.verification_intents.push(VerificationIntent {
                    verification_id,
                    acceptance_refs: string_array_at(interface, "acceptanceRefs"),
                    requirement_detail_refs: Vec::new(),
                    behavior,
                    preferred_evidence: vec![VerificationEvidence::AutomatedTest],
                    acceptable_evidence: vec![
                        VerificationEvidence::AutomatedTest,
                        VerificationEvidence::RuntimeApiCheck,
                    ],
                });
            }
        }

        if persistence_task_ids.contains(&task.task_id)
            && !task.verification_intents.iter().any(|intent| {
                intent.verification_id
                    == format!(
                        "verify-persistence-restart-{}",
                        normalized_identifier(&task.task_id)
                    )
            })
        {
            task.verification_intents.push(VerificationIntent {
                verification_id: format!(
                    "verify-persistence-restart-{}",
                    normalized_identifier(&task.task_id)
                ),
                acceptance_refs: Vec::new(),
                requirement_detail_refs: Vec::new(),
                behavior:
                    "Verify persisted state remains available after the application restarts."
                        .to_string(),
                preferred_evidence: vec![VerificationEvidence::AutomatedTest],
                acceptable_evidence: vec![
                    VerificationEvidence::AutomatedTest,
                    VerificationEvidence::RuntimeApiCheck,
                ],
            });
        }
    }
}

fn task_interface_refs_for_verification(task: &TaskDefinition) -> BTreeSet<String> {
    let mut refs = task
        .write_boundary
        .artifact_refs
        .all_interfaces()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let Some(scope) = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.pointer("/uiTaskScope"))
    else {
        return refs;
    };
    scope
        .get("frontendBackendBindings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|binding| binding.get("interfaces").and_then(Value::as_array))
        .flatten()
        .filter_map(|interface| string_at(interface, "interfaceId"))
        .for_each(|interface_id| {
            refs.insert(interface_id);
        });
    refs
}

fn interface_has_pagination(interface: &Value) -> bool {
    interface
        .pointer("/paginationPolicy/strategy")
        .and_then(Value::as_str)
        .is_some_and(|strategy| !matches!(strategy, "" | "not_applicable"))
}

fn interface_has_normalization(interface: &Value) -> bool {
    interface
        .get("requestSchema")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|field| field.get("normalization").is_some() || field.get("normalize").is_some())
}

fn interface_has_idempotency(interface: &Value) -> bool {
    interface
        .pointer("/idempotencyPolicy/required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn interface_requires_auth(interface: &Value) -> bool {
    interface
        .pointer("/authPolicy/required")
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "required" | "optional" | "deferred_with_risk"))
}

fn is_mutating_interface(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH" | "DELETE")
}

fn normalize_code_quality_requirements(
    baseline: &contracts::TechnicalBaselineContract,
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
) -> Vec<CodeQualityRequirement> {
    let mut requirements = Vec::new();
    let mut requirement_ids_by_task = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.iter() {
        let context = code_reference_task_context(aac, task);
        let Some(selection) =
            code_reference_selection_for_task_with_context(baseline, task, &context)
        else {
            continue;
        };
        if selection.reference_groups.is_empty() {
            continue;
        }
        let requirement_id = format!("code-quality-{}", task.task_id);
        requirement_ids_by_task
            .entry(task.task_id.clone())
            .or_default()
            .push(requirement_id.clone());
        requirements.push(CodeQualityRequirement {
            requirement_id,
            kind: "language_implementation_quality".to_string(),
            applies_to_task_ids: vec![task.task_id.clone()],
            stack_signals: selection.stack_signals.clone(),
            reference_load_plan: code_reference_load_plan(&selection.reference_groups),
            package_naming_policy: package_naming_policy_for_reference_groups(
                &selection.reference_groups,
            ),
            reference_groups: selection.reference_groups.clone(),
            focus_tags: selection.focus_tags.clone(),
            verification_obligations: code_quality_verification_obligations(),
        });
    }
    for task in tasks {
        task.code_quality_requirement_refs = requirement_ids_by_task
            .remove(&task.task_id)
            .unwrap_or_default();
    }
    requirements
}

fn code_reference_task_context(
    aac: &ArchitectureArtifactContract,
    task: &TaskDefinition,
) -> CodeReferenceTaskContext {
    let owned_decisions = task
        .write_boundary
        .artifact_refs
        .decisions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let application_architecture = aac.architecture_quality.decisions.iter().any(|decision| {
        owned_decisions.contains(&decision.decision_id)
            && matches!(
                decision.category.as_str(),
                "architecture_style" | "module_boundary"
            )
    });
    let owned_interfaces = task
        .write_boundary
        .artifact_refs
        .all_interfaces()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let task_interfaces = aac
        .interfaces
        .iter()
        .filter(|interface| {
            string_at(interface, "interfaceId")
                .is_some_and(|interface_id| owned_interfaces.contains(&interface_id))
        })
        .collect::<Vec<_>>();
    let task_interactions = task_owned_application_interactions(aac, task);
    let security = task_has_implementation_action(
        task,
        ImplementationAction::ImplementAuthenticationOrAuthorization,
    ) || task_interfaces
        .iter()
        .any(|interface| interface_requires_security(interface))
        || task_interactions
            .iter()
            .any(|interaction| interaction_requires_security(interaction))
        || task_quality_category_matches(aac, task, "security");
    let async_processing =
        task_has_implementation_action(task, ImplementationAction::ImplementAsyncProcessing)
            || task_interactions.iter().any(|interaction| {
                matches!(
                    interaction.get("interactionType").and_then(Value::as_str),
                    Some("event" | "job")
                )
            });
    let integration = task_has_implementation_action(
        task,
        ImplementationAction::ImplementExternalServiceIntegration,
    ) || task_interactions.iter().any(|interaction| {
        interaction.get("interactionType").and_then(Value::as_str) == Some("external_adapter")
    });
    let resilience =
        task_has_implementation_action(task, ImplementationAction::ImplementResiliencePolicy)
            || task_interactions
                .iter()
                .any(|interaction| interaction_owns_operational_policy(interaction, "retry"))
            || (integration
                && (task_quality_category_matches(aac, task, "integration")
                    || task_quality_category_matches(aac, task, "availability")
                    || task_quality_category_matches(aac, task, "reliability")));
    let request_tracing = task_interfaces
        .iter()
        .any(|interface| interface_owns_operational_policy(interface, "request_id"));
    let observability =
        task_has_implementation_action(task, ImplementationAction::ImplementObservability)
            || task_quality_category_matches(aac, task, "observability")
            || task_quality_category_matches(aac, task, "operability");
    CodeReferenceTaskContext {
        application_architecture,
        security,
        async_processing,
        integration,
        resilience,
        observability,
        request_tracing,
    }
}

fn task_has_implementation_action(task: &TaskDefinition, expected: ImplementationAction) -> bool {
    task.implementation_actions
        .iter()
        .any(|action| *action == expected)
}

fn interface_requires_security(interface: &Value) -> bool {
    interface
        .pointer("/authPolicy/required")
        .and_then(Value::as_str)
        .is_some_and(|required| matches!(required, "required" | "optional"))
}

fn interaction_requires_security(interaction: &Value) -> bool {
    matches!(
        interaction
            .pointer("/qualityTraits/authRequirement")
            .and_then(Value::as_str),
        Some("required" | "optional")
    )
}

fn interaction_owns_operational_policy(interaction: &Value, policy: &str) -> bool {
    interaction
        .pointer("/qualityTraits/operationalPolicies")
        .and_then(Value::as_array)
        .is_some_and(|policies| policies.iter().any(|item| item.as_str() == Some(policy)))
}

fn interface_owns_operational_policy(interface: &Value, policy: &str) -> bool {
    interface
        .pointer("/qualityTraits/operationalPolicies")
        .or_else(|| interface.pointer("/operationalPolicies"))
        .and_then(Value::as_array)
        .is_some_and(|policies| policies.iter().any(|item| item.as_str() == Some(policy)))
}

fn task_owned_application_interactions<'a>(
    aac: &'a ArchitectureArtifactContract,
    task: &TaskDefinition,
) -> Vec<&'a Value> {
    let module_refs = task
        .write_boundary
        .artifact_refs
        .modules
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let interface_refs = task
        .write_boundary
        .artifact_refs
        .all_interfaces()
        .into_iter()
        .collect::<BTreeSet<_>>();
    aac.engineering_boundary
        .pointer("/applicationInteractions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|interaction| {
            interaction
                .get("providerModuleRef")
                .and_then(Value::as_str)
                .is_some_and(|module_ref| module_refs.contains(module_ref))
                || string_array_at(interaction, "interfaceRefs")
                    .iter()
                    .any(|interface_ref| interface_refs.contains(interface_ref))
        })
        .collect()
}

fn task_quality_category_matches(
    aac: &ArchitectureArtifactContract,
    task: &TaskDefinition,
    category: &str,
) -> bool {
    aac.architecture_quality.decisions.iter().any(|decision| {
        decision.category == category
            && task
                .write_boundary
                .artifact_refs
                .decisions
                .contains(&decision.decision_id)
    }) || aac.architecture_quality.nfrs.iter().any(|nfr| {
        nfr.category == category && task.write_boundary.artifact_refs.nfrs.contains(&nfr.nfr_id)
    }) || aac.architecture_quality.risks.iter().any(|risk| {
        risk.category == category
            && task
                .write_boundary
                .artifact_refs
                .risks
                .contains(&risk.risk_id)
    })
}

fn code_quality_verification_obligations() -> Vec<String> {
    vec![
        "Use task.verificationIntents as verification id source.".to_string(),
        "Run the smallest available language-appropriate compile, type, lint, unit, or integration check that proves the changed code.".to_string(),
        "Record codeQualityEvidence for every assigned code quality requirement, including selected reference groups, reference files checked, changed files, commands, and known gaps.".to_string(),
    ]
}

fn task_owns_api_contract(task: &TaskDefinition) -> bool {
    if task_is_frontend_task(task) {
        return false;
    }
    if task.write_boundary.artifact_refs.interfaces.is_empty() {
        return false;
    }
    matches!(task.task_kind, TaskKind::InterfaceIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateInterface
                    | ImplementationAction::CreateEntityCrud
            )
        })
}

fn task_uses_api_contract(task: &TaskDefinition) -> bool {
    if task_owns_api_contract(task) {
        return true;
    }
    matches!(
        task.task_kind,
        TaskKind::IntegrationIncrement
            | TaskKind::VerificationIncrement
            | TaskKind::FrontendExperience
            | TaskKind::UiFlowIncrement
    ) || task
        .implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::WireReferenceInApiOrUi))
}

fn task_can_consume_api_contract(task: &TaskDefinition) -> bool {
    matches!(
        task.task_kind,
        TaskKind::IntegrationIncrement
            | TaskKind::VerificationIncrement
            | TaskKind::FrontendExperience
            | TaskKind::UiFlowIncrement
    ) || task
        .implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::WireReferenceInApiOrUi))
}

fn api_contract_verification_obligations() -> Vec<String> {
    vec![
        "Use task.verificationIntents as verification id source.".to_string(),
        "Verify at least one declared success path for each task-owned API interface or client/test binding.".to_string(),
        "Verify important validation or business-blocking error behavior for write/state-transition APIs.".to_string(),
        "For collection APIs, verify the declared pagination or filtering behavior when present.".to_string(),
        "For UI-owned collection APIs, verify pagination state, page navigation, and filter-to-page reset as separate observable behaviors.".to_string(),
        "For normalized inputs, verify trim or other declared canonicalization occurs before validation.".to_string(),
        "Record apiContractEvidence for every assigned API contract requirement.".to_string(),
    ]
}

fn normalize_engineering_quality_requirements(
    baseline: &contracts::TechnicalBaselineContract,
    tasks: &mut [TaskDefinition],
) -> Vec<EngineeringQualityRequirement> {
    let stack_signals = stack_signals_from_baseline(&baseline.stack);
    if !has_persistence_quality_signal(&stack_signals) {
        for task in tasks {
            task.engineering_quality_requirement_refs.clear();
        }
        return vec![];
    }
    let applies_to_task_ids = persistence_quality_task_ids(tasks);
    let requirement_id = "eqr-persistence-mapping-001".to_string();
    let applies_set = applies_to_task_ids.iter().cloned().collect::<BTreeSet<_>>();
    for task in tasks {
        task.engineering_quality_requirement_refs = if applies_set.contains(&task.task_id) {
            vec![requirement_id.clone()]
        } else {
            vec![]
        };
    }
    if applies_to_task_ids.is_empty() {
        return vec![];
    }
    vec![EngineeringQualityRequirement {
        requirement_id,
        kind: "persistence_mapping".to_string(),
        applies_to_task_ids,
        stack_signals,
        alignment_targets: persistence_alignment_targets(),
        risk_field_kinds: persistence_risk_field_kinds(),
        verification_obligations: persistence_verification_obligations(),
    }]
}

fn stack_signals_from_baseline(stack: &Value) -> BTreeMap<String, String> {
    let mut signals = BTreeMap::new();
    for (signal, track) in [
        ("web", "web"),
        ("backend", "backend"),
        ("persistence", "persistence"),
        ("dataAccess", "dataAccess"),
        ("externalServices", "externalServices"),
        ("migrationTool", "migrationTool"),
        ("testDatabase", "testDatabase"),
    ] {
        if let Some(value) = stack_track_selection(stack, track) {
            signals.insert(signal.to_string(), value);
        }
    }
    for (signal, keys) in [
        ("backend", &["backend", "server", "api"][..]),
        (
            "persistence",
            &["persistence", "database", "databaseProvider", "db"][..],
        ),
        ("dataAccess", &["dataAccess", "orm", "data_access"][..]),
        (
            "migrationTool",
            &["migrationTool", "migration", "migrations"][..],
        ),
        (
            "testDatabase",
            &["testDatabase", "testDb", "test_database"][..],
        ),
    ] {
        if signals.contains_key(signal) {
            continue;
        }
        if let Some(value) = first_stack_string_for_keys(stack, keys) {
            signals.insert(signal.to_string(), value);
        }
    }
    signals
}

fn stack_track_selection(stack: &Value, track: &str) -> Option<String> {
    let track_value = stack.get("tracks")?.get(track)?;
    let status = track_value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        status.as_str(),
        "not_needed" | "not_applicable" | "none" | "disabled"
    ) {
        return None;
    }
    let selection = compact_stack_value(track_value.get("selection")?)?;
    (!selection_is_absent(&selection)).then_some(selection)
}

fn first_stack_string_for_keys(stack: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = stack.get(*key).and_then(compact_stack_value) {
            if !selection_is_absent(&value) {
                return Some(value);
            }
        }
    }
    None
}

fn compact_stack_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(compact_stack_value)
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(", "))
        }
        Value::Object(object) => object
            .get("selection")
            .and_then(compact_stack_value)
            .or_else(|| object.get("name").and_then(compact_stack_value)),
        _ => None,
    }
}

fn selection_is_absent(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "no persistence",
        "no database",
        "none",
        "not needed",
        "not applicable",
        "n/a",
        "无",
        "不需要",
        "无需",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn has_persistence_quality_signal(signals: &BTreeMap<String, String>) -> bool {
    signals.contains_key("persistence")
        || signals.contains_key("dataAccess")
        || signals.contains_key("migrationTool")
}

fn persistence_quality_task_ids(tasks: &[TaskDefinition]) -> Vec<String> {
    let direct_ids = tasks
        .iter()
        .filter(|task| task_directly_owns_persistence_mapping(task))
        .map(|task| task.task_id.clone())
        .collect::<BTreeSet<_>>();
    tasks
        .iter()
        .filter(|task| {
            task_directly_owns_persistence_mapping(task)
                || (task_backend_consumes_persistence_mapping(task)
                    && task_depends_on_any(task, tasks, &direct_ids))
        })
        .map(|task| task.task_id.clone())
        .collect()
}

fn task_directly_owns_persistence_mapping(task: &TaskDefinition) -> bool {
    !task.write_boundary.artifact_refs.entities.is_empty()
        || matches!(task.task_kind, TaskKind::DataModelIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateEntity
                    | ImplementationAction::CreateOrUpdatePersistence
                    | ImplementationAction::CreateEntityMigration
                    | ImplementationAction::CreateEntityRepository
                    | ImplementationAction::CreateEntityCrud
                    | ImplementationAction::CreateOrUpdatePersistenceQuery
                    | ImplementationAction::ImplementPersistenceTransaction
                    | ImplementationAction::OptimizePersistenceQuery
                    | ImplementationAction::ImplementAnalyticalQuery
                    | ImplementationAction::ImplementEntityLifecycle
                    | ImplementationAction::AddOrUpdatePersistenceTests
            )
        })
}

fn task_backend_consumes_persistence_mapping(task: &TaskDefinition) -> bool {
    if task_is_frontend_task(task) {
        return false;
    }
    matches!(
        task.task_kind,
        TaskKind::FeatureIncrement | TaskKind::InterfaceIncrement | TaskKind::IntegrationIncrement
    ) || task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateInterface
                | ImplementationAction::CreateOrUpdateBusinessRule
                | ImplementationAction::CreateOrUpdateStateMachine
                | ImplementationAction::WireReferenceInApiOrUi
        )
    })
}

fn task_is_frontend_task(task: &TaskDefinition) -> bool {
    task.frontend_experience_requirement.is_some()
        || matches!(
            task.task_kind,
            TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
        )
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateUiFlow
                    | ImplementationAction::CreateOrUpdateFrontendNavigation
                    | ImplementationAction::ImplementReactiveClientFlow
                    | ImplementationAction::ImplementSharedClientState
                    | ImplementationAction::OptimizeFrontendPerformance
                    | ImplementationAction::ImplementServerRenderedComponent
                    | ImplementationAction::ImplementServerMutation
                    | ImplementationAction::ImplementFrontendFrameworkVersionFeature
                    | ImplementationAction::ImplementFrontendExperienceContract
                    | ImplementationAction::CreateEntityAdminPage
            )
        })
}

fn task_depends_on_any(
    task: &TaskDefinition,
    tasks: &[TaskDefinition],
    target_ids: &BTreeSet<String>,
) -> bool {
    let by_id = tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    let mut stack = task.depends_on.clone();
    let mut seen = BTreeSet::new();
    while let Some(task_id) = stack.pop() {
        if !seen.insert(task_id.clone()) {
            continue;
        }
        if target_ids.contains(&task_id) {
            return true;
        }
        if let Some(dependency) = by_id.get(task_id.as_str()) {
            stack.extend(dependency.depends_on.clone());
        }
    }
    false
}

fn persistence_alignment_targets() -> Vec<String> {
    [
        "domain_model_field",
        "storage_schema_field",
        "data_access_mapping",
        "dto_json_contract",
        "query_sort_filter_field",
        "test_database_provider",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn persistence_risk_field_kinds() -> Vec<String> {
    [
        "datetime",
        "decimal",
        "enum",
        "boolean",
        "json",
        "foreign_key",
        "unique_constraint",
        "nullable_default",
        "identifier",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn persistence_verification_obligations() -> Vec<String> {
    [
        "same_provider_persistence_test",
        "create_detail_list_roundtrip",
        "state_change_readback",
        "error_response_message_contract",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn runtime_delivery_closure_task_template(aac: &ArchitectureArtifactContract) -> Value {
    let Some(runtime_delivery) = aac.runtime_delivery.as_ref() else {
        return Value::Null;
    };
    let status = runtime_delivery.get("status").and_then(Value::as_str);
    if status != Some("modified") {
        return Value::Null;
    }
    let affected_contract_fields = runtime_delivery_closure_fields(runtime_delivery);
    let required_code_level_checks = affected_contract_fields
        .iter()
        .map(|field| runtime_delivery_closure_check(field))
        .collect::<Vec<_>>();
    json!({
        "taskKind": "runtime_delivery_closure",
        "groupPlacement": {
            "groupKind": "runtime_delivery_closure",
            "position": "final_group",
            "taskIdsRule": "The closure group taskIds array must contain exactly this one runtime_delivery_closure task.",
            "dependsOnRule": "Use group dependsOn to reference the previous group or groups that make runtime-affecting work transitively complete; no other group may depend on the closure group.",
            "taskDependsOnRule": "Keep the closure task dependsOn empty unless another task is in the same closure group; the closure group itself should carry cross-group dependencies."
        },
        "runtimeDeliveryRequirement": {
            "appliesToThisTask": true,
            "reason": "Final code-level closure for the RuntimeDeliveryContract.",
            "affectedContractFields": affected_contract_fields,
            "requiredCodeLevelChecks": required_code_level_checks,
            "evidenceExpectedInTaskResult": [],
            "forbiddenActions": []
        }
    })
}

fn runtime_delivery_closure_fields(runtime_delivery: &Value) -> Vec<String> {
    let mut fields = Vec::new();
    for phase in ["development", "verification", "deployment"] {
        if has_non_empty_string(
            runtime_delivery,
            &format!("/commands/{phase}/build/command"),
        ) || has_non_empty_string(
            runtime_delivery,
            &format!("/commands/{phase}/start/command"),
        ) {
            fields.push(format!("commands.{phase}"));
        }
    }
    if has_non_empty_array(runtime_delivery, "/runtimeSurfaces") {
        fields.push("runtimeSurfaces".to_string());
    }
    if runtime_delivery.get("httpProbes").is_some() {
        fields.push("httpProbes".to_string());
    }
    if runtime_delivery.get("frontend").is_some() {
        fields.push("frontend".to_string());
    }
    if runtime_delivery.get("api").is_some() {
        fields.push("api".to_string());
    }
    if runtime_delivery.get("environment").is_some() {
        fields.push("environment".to_string());
    }
    fields
}

fn runtime_delivery_closure_check(contract_field: &str) -> Value {
    json!({
        "objective": format!("Confirm {contract_field} is closed at code level against RuntimeDeliveryContract."),
        "acceptableEvidence": acceptable_evidence_for_runtime_closure_field(contract_field)
    })
}

fn runtime_delivery_check_for_field(contract_field: &str) -> contracts::RuntimeCodeLevelCheck {
    contracts::RuntimeCodeLevelCheck {
        check_id: runtime_delivery_check_id(contract_field),
        contract_field: Some(contract_field.to_string()),
        objective: format!(
            "Confirm {contract_field} is closed at code level against RuntimeDeliveryContract."
        ),
        acceptable_evidence: acceptable_evidence_for_runtime_closure_field(contract_field)
            .into_iter()
            .filter_map(|evidence| match evidence {
                "static_check" => Some(VerificationEvidence::StaticCheck),
                "runtime_api_check" => Some(VerificationEvidence::RuntimeApiCheck),
                "manual_command_output" => Some(VerificationEvidence::ManualCommandOutput),
                _ => None,
            })
            .collect(),
    }
}

fn runtime_delivery_check_id(contract_field: &str) -> String {
    runtime_delivery_closure_check_id(contract_field)
}

fn runtime_delivery_closure_check_id(contract_field: &str) -> String {
    let mut id = String::from("rd-closure-");
    let mut previous_dash = false;
    for ch in contract_field.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            id.push('-');
            previous_dash = true;
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    id
}

fn acceptable_evidence_for_runtime_closure_field(contract_field: &str) -> Vec<&'static str> {
    if matches!(
        contract_field,
        "httpProbes" | "runtimeSurfaces" | "api" | "frontend"
    ) {
        return vec!["static_check", "runtime_api_check", "manual_command_output"];
    }
    vec!["static_check", "manual_command_output"]
}

fn has_non_empty_string(value: &Value, pointer: &str) -> bool {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|item| !item.is_empty())
}

fn has_non_empty_array(value: &Value, pointer: &str) -> bool {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn enum_refs() -> Value {
    json!({
        "taskKind": TASK_KIND_VALUES,
        "implementationAction": IMPLEMENTATION_ACTION_VALUES,
        "verificationEvidence": VERIFICATION_EVIDENCE_VALUES,
        "uiSurfaceDecision": ui_surface_decision_enum_refs(),
        "codeQuality": code_quality_enum_refs()
    })
}

fn ids_from_values(values: &[Value], id_key: &str) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| {
            value
                .get(id_key)
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn ids_from_value_array(root: &Value, pointer: &str, id_key: &str) -> Vec<String> {
    root.pointer(pointer)
        .and_then(Value::as_array)
        .map(|items| ids_from_values(items, id_key))
        .unwrap_or_default()
}

fn ensure_latest_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    mode: TaskPlanSubmitMode,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let Some(phase) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return Ok(Some(stale_failure(
            project_root,
            "TaskPlan submit phase does not exist.".to_string(),
            mode,
        )));
    };
    if phase
        .latest_refs
        .get(mode.latest_ref_key())
        .map(String::as_str)
        != Some(request_ref)
    {
        return Ok(Some(stale_failure(
            project_root,
            "TaskPlan submit must use the active phase latest requestRef.".to_string(),
            mode,
        )));
    }
    Ok(None)
}

fn repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
    mode: TaskPlanSubmitMode,
) -> LoomMcpActionResult {
    LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
        project_root: input.project_root.clone(),
        stop_allowed: false,
        target_file,
        target_ids: authorized
            .targets
            .iter()
            .map(|target| target.target_id.clone())
            .collect(),
        issues,
        resubmit_tool: mode.resubmit_tool().to_string(),
        fix_scope: Some(mode.fix_scope().to_string()),
        read_groups: authorized.read_groups.clone(),
        agent_instruction: delivery_core::repairable_error_agent_instruction(mode.resubmit_tool()),
    })
}

fn stale_failure(
    project_root: &str,
    message: String,
    mode: TaskPlanSubmitMode,
) -> LoomMcpActionResult {
    failed(
        project_root,
        mode.stale_code(),
        message,
        mode.route_action(),
    )
}

fn failed(
    project_root: &str,
    code: &str,
    message: String,
    route_action: &str,
) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: code.to_string(),
            message,
            target_batch: Some(8),
            domain: Some("execution".to_string()),
            route_action: Some(route_action.to_string()),
            recovery_tool: Some("loom.continue".to_string()),
        },
    })
}

fn issue(
    code: &str,
    field_path: &str,
    message: &str,
    target_id: Option<&str>,
) -> delivery_core::RepairIssue {
    delivery_core::RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: target_id.map(str::to_string),
        field_path: Some(field_path.to_string()),
    }
}

fn deserialize_candidate<T>(
    value: Value,
    fallback_path: &str,
    code: &str,
    target_id: Option<&str>,
) -> Result<T, delivery_core::RepairIssue>
where
    T: serde::de::DeserializeOwned,
{
    let text = serde_json::to_string(&value).map_err(|error| {
        issue(
            code,
            fallback_path,
            &format!(
                "TaskPlan candidate JSON could not be prepared for schema validation: {error}."
            ),
            target_id,
        )
    })?;
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let field_path = if path == "." || path.is_empty() {
            fallback_path.to_string()
        } else {
            path
        };
        issue(
            code,
            &field_path,
            &format!("TaskPlan candidate JSON does not match the expected schema: {error}."),
            target_id,
        )
    })
}

fn value_to_write_target(
    value: &Value,
) -> Result<delivery_core::WriteTarget, state::store::StateError> {
    serde_json::from_value(value.clone()).map_err(state::store::StateError::from)
}

fn string_field(
    fields: &BTreeMap<String, delivery_core::FieldReadResult>,
    name: &str,
) -> Result<String, state::store::StateError> {
    fields
        .get(name)
        .and_then(|field| field.value.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!("missing request field {name}"))
        })
}

fn value_field(fields: &BTreeMap<String, delivery_core::FieldReadResult>, name: &str) -> Value {
    fields
        .get(name)
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null)
}

fn read_project_json<T: serde::de::DeserializeOwned>(
    project_root: &Path,
    relative: &str,
) -> Result<T, state::store::StateError> {
    let path = from_project_relative(project_root, relative)?;
    state::store::read_json(&path)
}

fn read_project_json_value(
    project_root: &Path,
    relative: &str,
) -> Result<Value, state::store::StateError> {
    let path = from_project_relative(project_root, relative)?;
    state::store::read_json(&path)
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}

pub(crate) fn execute_task_next_from_request(
    project_root: &str,
    request_ref: &str,
    task: &TaskDefinition,
    result_file: String,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })?;
    let submit_tool = inspected.submit_tool.ok_or_else(|| {
        state::store::StateError::StateCorrupted(
            "TaskExecution request missing submitTool".to_string(),
        )
    })?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::ExecuteTask(delivery_core::ExecuteTaskNext {
                execution_kind: delivery_core::ExecutionKind::PlannedTask,
                repair_origin: None,
                request_ref: request_ref.to_string(),
                result_file,
                task_id: task.task_id.clone(),
                group_id: Some(task.group_id.clone()),
                read_groups: inspected.read_groups,
                submit_tool,
                edit_boundary: ExecuteEditBoundary {
                    allowed_paths: vec![".".to_string()],
                    protected_paths: task.write_boundary.forbidden_paths.clone(),
                },
                verification_policy: ExecuteVerificationPolicy {
                    required_commands: vec![],
                    evidence_required: true,
                },
                repair_context: None,
                post_submit: PostSubmitAction::ContinueDelivery,
            }),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser_task(verification_count: usize) -> TaskDefinition {
        let verification_intents = (0..verification_count)
            .map(|index| {
                json!({
                    "verificationId": format!("verify-ui-{index}"),
                    "acceptanceRefs": [],
                    "requirementDetailRefs": [],
                    "behavior": format!("Verify UI behavior {index}"),
                    "preferredEvidence": ["automated_test"],
                    "acceptableEvidence": ["automated_test"]
                })
            })
            .collect::<Vec<_>>();
        serde_json::from_value(json!({
            "taskId": "task-ui",
            "groupId": "group-ui",
            "title": "Implement UI",
            "taskKind": "ui_flow_increment",
            "implementationActions": ["create_or_update_ui_flow", "add_or_update_tests"],
            "objective": "Implement the task-owned UI workflow.",
            "dependsOn": [],
            "scopeRefs": [],
            "acceptanceRefs": [],
            "requirementDetailRefs": [],
            "writeBoundary": {"forbiddenPaths": [".loom"], "artifactRefs": {}},
            "verificationIntents": verification_intents,
            "conceptRefs": [],
            "conceptResponsibilities": [],
            "conceptVerificationIntents": [],
            "frontendExperienceRequirement": {
                "uiTaskScope": {
                    "surfacesInScope": ["surface-workbench"],
                    "regionsInScope": [{"regionId": "region-main"}],
                    "qualityRulesInScope": [{"ruleId": "verify.rendered_viewports"}]
                }
            },
            "engineeringQualityRequirementRefs": [],
            "architectureQualityRequirementRefs": [],
            "apiContractRequirementRefs": [],
            "codeQualityRequirementRefs": []
        }))
        .expect("browser task")
    }

    #[test]
    fn implementation_obligations_keep_single_provenance_and_relevant_verifications() {
        let mut task = browser_task(3);
        task.task_kind = TaskKind::InterfaceIncrement;
        task.frontend_experience_requirement = None;
        task.implementation_actions = vec![
            ImplementationAction::CreateOrUpdateInterface,
            ImplementationAction::ImplementResiliencePolicy,
        ];
        task.scope_refs = vec!["scope-orders".to_string()];
        task.acceptance_refs = vec!["accept-orders".to_string()];
        task.requirement_detail_refs = vec![
            "detail-orders-create".to_string(),
            "detail-orders-retry".to_string(),
        ];
        task.write_boundary.artifact_refs.interfaces = vec!["api-orders".to_string()];

        let mut obligations = Vec::new();
        push_task_implementation_obligation(
            &mut obligations,
            &task,
            "interface_contract",
            "Implement the accepted interface.",
            vec![VerificationEvidence::AutomatedTest],
            task.write_boundary.artifact_refs.clone(),
            obligation_source_refs(&task, &[]),
        );
        push_task_implementation_obligation(
            &mut obligations,
            &task,
            "resilience_policy",
            "Implement bounded retry behavior.",
            vec![VerificationEvidence::AutomatedTest],
            task.write_boundary.artifact_refs.clone(),
            obligation_source_refs(&task, &[]),
        );

        assert_eq!(obligations[0].source_refs.len(), 5);
        assert!(obligations[0]
            .source_refs
            .contains(&"detail-orders-create".to_string()));
        assert!(!obligations[1]
            .source_refs
            .contains(&"detail-orders-create".to_string()));
        assert!(obligations[0]
            .artifact_refs
            .interfaces
            .contains(&"api-orders".to_string()));
    }

    #[test]
    fn deferred_security_does_not_create_authentication_task_signals() {
        assert!(!interface_requires_security(&json!({
            "authPolicy": {"required": "deferred_with_risk"}
        })));
        assert!(!interaction_requires_security(&json!({
            "qualityTraits": {"authRequirement": "deferred_with_risk"}
        })));
        assert!(interface_requires_security(&json!({
            "authPolicy": {"required": "required"}
        })));
        assert!(interaction_requires_security(&json!({
            "qualityTraits": {"authRequirement": "optional"}
        })));
    }

    #[test]
    fn taskplan_candidate_nulls_are_not_silently_rewritten() {
        let candidate = json!({
            "tasks": [{
                "scopeRefs": null,
                "verificationIntents": null,
                "writeBoundary": {
                    "forbiddenPaths": null,
                    "artifactRefs": {
                        "modules": null,
                        "interfaces": null
                    }
                },
                "runtimeDeliveryRequirement": {
                    "affectedContractFields": null,
                    "requiredCodeLevelChecks": null
                }
            }]
        });

        assert!(candidate["tasks"][0]["scopeRefs"].is_null());
        assert!(candidate["tasks"][0]["verificationIntents"].is_null());
        assert!(candidate["tasks"][0]["writeBoundary"]["artifactRefs"]["modules"].is_null());
    }

    #[test]
    fn architecture_quality_refs_are_derived_from_owner_artifacts() {
        let aac: ArchitectureArtifactContract = serde_json::from_value(json!({
            "schemaVersion": "1.0",
            "architectureArtifactContractId": "aac-1",
            "deliveryId": "delivery-1",
            "phaseId": "phase-1",
            "status": "ready",
            "source": {"planningGenerationContractId": "pgc-1", "technicalBaselineId": "tbr-1"},
            "engineeringBoundary": {},
            "modules": [{"moduleId": "module-orders"}],
            "dataModel": {},
            "interfaces": [],
            "userFlows": [],
            "stateMachines": [],
            "acceptanceMatrix": [],
            "detailCoverage": [],
            "architectureQuality": {
                "decisions": [{
                    "decisionId": "adr-orders",
                    "category": "module_boundary",
                    "title": "Own order behavior in one module",
                    "status": "accepted",
                    "context": "The current phase changes order behavior.",
                    "decision": "The order module owns its behavior.",
                    "alternativesConsidered": [{"name": "shared service", "tradeoff": "less ownership", "rejectedBecause": "it weakens invariants"}],
                    "consequences": {"positive": ["clear ownership"], "negative": ["explicit mapping"], "neutral": []},
                    "sourceRefs": {"scopeRefs": ["scope-1"], "acceptanceRefs": [], "requirementDetailRefs": []},
                    "ownerArtifactRefs": {"modules": ["module-orders"], "interfaces": []},
                    "verificationHints": ["review module ownership"]
                }],
                "nfrs": [{
                    "nfrId": "nfr-orders",
                    "category": "maintainability",
                    "source": "derived_minimum",
                    "target": "Order rules remain in the order module.",
                    "rationale": "Ownership prevents drift.",
                    "measurement": {"indicator": "rule location", "workloadOrCondition": "order writes", "evaluationBoundary": "review"},
                    "sourceRefs": {"scopeRefs": ["scope-1"], "acceptanceRefs": [], "requirementDetailRefs": []},
                    "architectureRefs": {"decisions": ["adr-orders"], "risks": ["risk-orders"]},
                    "ownerArtifactRefs": {"modules": ["module-orders"], "interfaces": []},
                    "verificationStrategy": "Review changed order files."
                }],
                "risks": [{
                    "riskId": "risk-orders",
                    "category": "maintainability",
                    "severity": "medium",
                    "likelihood": "medium",
                    "impact": "Rules can drift.",
                    "mitigation": "Keep one owner.",
                    "ownerArtifactRefs": {"modules": [], "interfaces": [], "decisions": ["adr-orders"], "nfrs": ["nfr-orders"]},
                    "verificationHints": ["review ownership"]
                }]
            },
            "handoff": {"readyForTaskPlan": true, "blockingReasons": [], "nextNode": "task_plan"},
            "createdAt": "2026-07-15T00:00:00Z",
            "updatedAt": "2026-07-15T00:00:00Z"
        }))
        .expect("architecture artifact");
        let compact_quality = compact_architecture_quality(&aac);
        assert_eq!(
            compact_quality["decisions"][0]["decision"],
            json!("The order module owns its behavior.")
        );
        assert_eq!(
            compact_quality["nfrs"][0]["measurement"]["evaluationBoundary"],
            json!("review")
        );
        assert_eq!(
            compact_quality["risks"][0]["mitigation"],
            json!("Keep one owner.")
        );
        assert!(compact_quality["decisions"][0]
            .get("alternativesConsidered")
            .is_none());
        let mut owned = browser_task(1);
        owned.write_boundary.artifact_refs.modules = vec!["module-orders".to_string()];
        let mut unrelated = browser_task(1);
        unrelated.task_id = "task-unrelated".to_string();
        unrelated.write_boundary.artifact_refs.modules = vec!["module-other".to_string()];
        let mut tasks = vec![owned, unrelated];

        normalize_architecture_quality_artifact_refs(&aac, &mut tasks);

        assert_eq!(
            tasks[0].write_boundary.artifact_refs.decisions,
            ["adr-orders"]
        );
        assert_eq!(tasks[0].write_boundary.artifact_refs.nfrs, ["nfr-orders"]);
        assert_eq!(tasks[0].write_boundary.artifact_refs.risks, ["risk-orders"]);
        assert!(tasks[1].write_boundary.artifact_refs.decisions.is_empty());
        assert!(tasks[1].write_boundary.artifact_refs.nfrs.is_empty());
        assert!(tasks[1].write_boundary.artifact_refs.risks.is_empty());
    }

    #[test]
    fn code_reference_context_uses_task_owned_architecture_facts() {
        let aac: ArchitectureArtifactContract = serde_json::from_value(json!({
            "schemaVersion": "1.0",
            "architectureArtifactContractId": "aac-code-context",
            "deliveryId": "delivery-1",
            "phaseId": "phase-1",
            "status": "ready",
            "source": {
                "planningGenerationContractId": "pgc-1",
                "technicalBaselineId": "tbr-1"
            },
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-payment-provider",
                    "interactionType": "external_adapter",
                    "providerModuleRef": "module-orders",
                    "interfaceRefs": ["api-orders-create"],
                    "qualityTraits": {"operationalPolicies": ["retry"]}
                }, {
                    "interactionId": "interaction-order-events",
                    "interactionType": "event",
                    "providerModuleRef": "module-orders",
                    "interfaceRefs": []
                }]
            },
            "modules": [{"moduleId": "module-orders"}],
            "dataModel": {},
            "interfaces": [{
                "interfaceId": "api-orders-create",
                "authPolicy": {
                    "required": "required",
                    "securityProfileRef": "security-api-default",
                    "actorRefs": [],
                    "permissionRefs": []
                },
                "qualityTraits": {"operationalPolicies": ["request_id"]}
            }, {
                "interfaceId": "api-cache-hints-only",
                "cachePolicy": {"strategy": "private", "validators": ["etag"]},
                "retryPolicy": {
                    "retryableStatuses": [502, 503],
                    "retryAfterHeader": true
                }
            }],
            "userFlows": [],
            "stateMachines": [],
            "acceptanceMatrix": [],
            "detailCoverage": [],
            "architectureQuality": {
                "decisions": [{
                    "decisionId": "adr-orders-module-boundary",
                    "category": "module_boundary",
                    "title": "Keep order application boundaries explicit",
                    "status": "accepted",
                    "context": "Order behavior spans HTTP, persistence, and a provider adapter.",
                    "decision": "The order module owns application orchestration behind inward-facing ports.",
                    "alternativesConsidered": [{
                        "name": "endpoint-owned orchestration",
                        "tradeoff": "fewer types but mixed transport and business concerns",
                        "rejectedBecause": "the accepted module boundary requires reusable application behavior"
                    }],
                    "consequences": {
                        "positive": ["explicit ownership"],
                        "negative": ["additional adapter mapping"],
                        "neutral": []
                    },
                    "sourceRefs": {
                        "scopeRefs": [],
                        "acceptanceRefs": [],
                        "requirementDetailRefs": []
                    },
                    "ownerArtifactRefs": {
                        "modules": ["module-orders"],
                        "interfaces": ["api-orders-create"]
                    },
                    "verificationHints": ["verify application dependency direction"]
                }],
                "nfrs": [{
                    "nfrId": "nfr-orders-observability",
                    "category": "observability",
                    "source": "confirmed_requirement",
                    "target": "Order provider calls expose correlated metrics and traces.",
                    "rationale": "External failures must be diagnosable.",
                    "measurement": {
                        "indicator": "correlated telemetry",
                        "workloadOrCondition": "provider request",
                        "evaluationBoundary": "integration verification"
                    },
                    "sourceRefs": {
                        "scopeRefs": [],
                        "acceptanceRefs": [],
                        "requirementDetailRefs": []
                    },
                    "architectureRefs": {"decisions": [], "risks": []},
                    "ownerArtifactRefs": {
                        "modules": ["module-orders"],
                        "interfaces": ["api-orders-create"]
                    },
                    "verificationStrategy": "Verify correlated telemetry at the provider boundary."
                }],
                "risks": []
            },
            "handoff": {
                "readyForTaskPlan": true,
                "blockingReasons": [],
                "nextNode": "task_plan"
            },
            "createdAt": "2026-07-15T00:00:00Z",
            "updatedAt": "2026-07-15T00:00:00Z"
        }))
        .expect("architecture artifact");
        let mut task = browser_task(1);
        task.task_kind = TaskKind::InterfaceIncrement;
        task.implementation_actions = vec![ImplementationAction::CreateOrUpdateInterface];
        task.frontend_experience_requirement = None;
        task.write_boundary.artifact_refs.modules = vec!["module-orders".to_string()];
        task.write_boundary.artifact_refs.interfaces = vec!["api-orders-create".to_string()];
        task.write_boundary.artifact_refs.decisions =
            vec!["adr-orders-module-boundary".to_string()];
        task.write_boundary.artifact_refs.nfrs = vec!["nfr-orders-observability".to_string()];

        let context = code_reference_task_context(&aac, &task);
        let projected_interactions = compact_application_interactions(&aac);
        let rules = generation_rules(&aac, &Value::Null);
        let mut api_policy_only_task = task.clone();
        api_policy_only_task
            .write_boundary
            .artifact_refs
            .modules
            .clear();
        api_policy_only_task
            .write_boundary
            .artifact_refs
            .decisions
            .clear();
        api_policy_only_task.write_boundary.artifact_refs.interfaces =
            vec!["api-cache-hints-only".to_string()];
        api_policy_only_task
            .write_boundary
            .artifact_refs
            .nfrs
            .clear();
        let api_policy_only_context = code_reference_task_context(&aac, &api_policy_only_task);

        assert!(context.application_architecture);
        assert!(context.security);
        assert!(context.async_processing);
        assert!(context.integration);
        assert!(context.resilience);
        assert!(context.observability);
        assert!(context.request_tracing);
        assert!(!api_policy_only_context.resilience);
        assert!(!api_policy_only_context.request_tracing);
        assert!(!api_policy_only_context.application_architecture);
        assert_eq!(projected_interactions.len(), 2);
        assert_eq!(
            projected_interactions[0]["interactionType"],
            json!("external_adapter")
        );
        assert_eq!(
            projected_interactions[1]["providerModuleRef"],
            json!("module-orders")
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["frontendNavigation"],
            json!(["create_or_update_frontend_navigation"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["reactiveClientFlow"],
            json!(["implement_reactive_client_flow"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["sharedClientState"],
            json!(["implement_shared_client_state"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["frontendPerformance"],
            json!(["optimize_frontend_performance"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["serverRenderedComponent"],
            json!(["implement_server_rendered_component"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["serverMutation"],
            json!(["implement_server_mutation"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]
                ["frontendFrameworkVersionFeature"],
            json!(["implement_frontend_framework_version_feature"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["mobilePlatformBehavior"],
            json!(["implement_mobile_platform_behavior"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["clientStorage"],
            json!(["implement_client_storage"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["languageVersionFeature"],
            json!(["implement_language_version_feature"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["genericTypeAbstraction"],
            json!(["implement_generic_type_abstraction"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["dependencyAbstraction"],
            json!(["implement_dependency_abstraction"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["moduleStructure"],
            json!(["refactor_module_structure"])
        );
        assert_eq!(
            rules["codeReferenceRules"]["frameworkCapabilityActions"]["runtimePerformance"],
            json!(["optimize_runtime_performance"])
        );
    }

    #[test]
    fn quality_requirements_do_not_duplicate_task_implementation_scope() {
        let requirement = code_quality_requirement_template(&json!({"required": true}));
        assert!(requirement.get("implementationObligations").is_none());
        assert!(requirement.get("verificationObligations").is_some());
    }

    #[test]
    fn implementation_obligations_use_structured_verification_refs_not_behavior_keywords() {
        let task = TaskDefinition {
            task_id: "task-owned".to_string(),
            group_id: "group-owned".to_string(),
            title: "Implement owned behavior".to_string(),
            task_kind: TaskKind::FeatureIncrement,
            implementation_actions: vec![],
            implementation_obligations: vec![],
            objective: "Implement the owned behavior".to_string(),
            depends_on: vec![],
            scope_refs: vec![],
            acceptance_refs: vec!["accept-owned".to_string()],
            requirement_detail_refs: vec!["detail-owned".to_string()],
            write_boundary: TaskWriteBoundary {
                forbidden_paths: vec![".loom".to_string()],
                artifact_refs: TaskArtifactRefs::default(),
            },
            verification_intents: vec![
                VerificationIntent {
                    verification_id: "verify-other".to_string(),
                    acceptance_refs: vec!["accept-other".to_string()],
                    requirement_detail_refs: vec!["detail-other".to_string()],
                    behavior: "This text mentions persistence and storage but belongs elsewhere."
                        .to_string(),
                    preferred_evidence: vec![VerificationEvidence::AutomatedTest],
                    acceptable_evidence: vec![VerificationEvidence::AutomatedTest],
                },
                VerificationIntent {
                    verification_id: "verify-owned".to_string(),
                    acceptance_refs: vec!["accept-owned".to_string()],
                    requirement_detail_refs: vec!["detail-owned".to_string()],
                    behavior: "Verify the owned behavior.".to_string(),
                    preferred_evidence: vec![VerificationEvidence::AutomatedTest],
                    acceptable_evidence: vec![VerificationEvidence::AutomatedTest],
                },
            ],
            concept_refs: vec![],
            concept_responsibilities: vec![],
            concept_verification_intents: vec![],
            frontend_experience_requirement: None,
            runtime_delivery_requirement: None,
            engineering_quality_requirement_refs: vec![],
            architecture_quality_requirement_refs: vec![],
            api_contract_requirement_refs: vec![],
            code_quality_requirement_refs: vec![],
        };

        let ids = task
            .verification_intents
            .iter()
            .filter(|intent| verification_intent_matches_obligation(intent, &task))
            .map(|intent| intent.verification_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["verify-owned"]);
        let obligation = TaskImplementationObligation {
            obligation_id: "obligation-owned".to_string(),
            kind: "persistence_mapping".to_string(),
            source_refs: vec!["detail-owned".to_string()],
            artifact_refs: TaskArtifactRefs::default(),
            required_outcome: "Implement the owned persistence boundary.".to_string(),
            required: true,
            acceptable_evidence: vec![VerificationEvidence::AutomatedTest],
            verification_ids: vec![],
            defer_policy: "must_be_satisfied_before_completed".to_string(),
        };
        assert_eq!(
            verification_ids_for_obligation(&task, &[obligation], 0),
            vec!["verify-owned"]
        );
        assert_eq!(
            implementation_obligation_for_action(ImplementationAction::CreateOrUpdatePersistence)
                .expect("persistence action")
                .0,
            "persistence_mapping"
        );
        assert_eq!(
            implementation_obligation_for_action(ImplementationAction::CreateOrUpdateEntity)
                .expect("entity action")
                .0,
            "entity_contract"
        );
    }

    #[test]
    fn ui_surface_decision_projection_keeps_task_contract_compact() {
        let surface_contract = json!({
            "patternDecision": {
                "mode": "known",
                "knownPattern": "collection_workbench",
                "primaryKnownPattern": null,
                "customPattern": null
            },
            "regionModel": [{
                "regionId": "region_primary"
            }],
            "actionModel": [{
                "actionId": "action_create"
            }],
            "stateModel": [{
                "state": "loading"
            }],
            "qualityRules": [{
                "ruleId": "rule_primary"
            }],
            "referencePlan": [{
                "path": "uix/core.md"
            }]
        });

        let projection = compact_ui_surface_decision_contract(&surface_contract);

        assert_eq!(
            projection
                .pointer("/patternDecision/mode")
                .and_then(Value::as_str),
            Some("known")
        );
        assert!(
            projection.get("referencePlan").is_none(),
            "TaskPlan projection must not copy the full UI reference plan"
        );
        assert!(
            projection["qualityRules"]
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "TaskPlan projection must keep the scoped quality rule ids"
        );
        assert!(
            projection["regionModel"]
                .as_array()
                .is_some_and(|items| items
                    .iter()
                    .any(|item| item["regionId"] == json!("region_primary"))),
            "TaskPlan projection must expose compact region ids"
        );
    }

    #[test]
    fn single_browser_verification_intent_is_normalized_without_agent_retry() {
        let mut tasks = vec![browser_task(1)];

        normalize_browser_verification_assignments(&mut tasks);

        assert!(tasks[0].verification_intents[0]
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation));
        assert!(validate_browser_verification_assignments(&tasks).is_empty());
    }

    #[test]
    fn generic_frontend_tests_do_not_create_browser_verification() {
        let mut task = browser_task(1);
        task.frontend_experience_requirement.as_mut().unwrap()["uiTaskScope"]
            ["qualityRulesInScope"] = json!([]);
        let mut tasks = vec![task];

        normalize_browser_verification_assignments(&mut tasks);

        assert!(!tasks[0].verification_intents[0]
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation));
        assert!(validate_browser_verification_assignments(&tasks).is_empty());
    }

    #[test]
    fn multiple_verification_intents_are_left_for_browser_closure() {
        let mut tasks = vec![browser_task(2)];

        normalize_browser_verification_assignments(&mut tasks);
        let issues = validate_browser_verification_assignments(&tasks);

        assert!(!tasks[0].verification_intents.iter().any(|intent| intent
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation)));
        assert!(
            issues.is_empty(),
            "business UI tasks are closed by the MCP browser task"
        );
    }

    #[test]
    fn browser_verification_is_moved_to_final_mcp_closure() {
        let mut tasks = vec![browser_task(1)];
        let mut groups = vec![TaskPlanGroup {
            group_id: "group-ui".to_string(),
            title: "UI".to_string(),
            objective: "Implement UI".to_string(),
            depends_on: vec![],
            scope_refs: vec![],
            acceptance_refs: vec![],
            task_ids: vec!["task-ui".to_string()],
        }];
        normalize_browser_verification_assignments(&mut tasks);
        let profiles = derive_browser_verification_profiles(
            &contracts::BrowserAutomationFacts::default(),
            &tasks,
        );

        let closure_profiles =
            materialize_browser_quality_closure(&mut groups, &mut tasks, profiles);

        assert_eq!(
            groups.last().unwrap().group_id,
            "group-browser-quality-closure"
        );
        assert_eq!(
            tasks.last().unwrap().task_id,
            "task-browser-quality-closure"
        );
        assert_eq!(closure_profiles.len(), 1);
        assert_eq!(closure_profiles[0].task_id, "task-browser-quality-closure");
        assert_eq!(closure_profiles[0].checks[0].source_task_id, "task-ui");
        assert_eq!(
            closure_profiles[0].checks[0].enforcement,
            BrowserEvidenceEnforcement::Required
        );
        assert!(!tasks[0].verification_intents[0]
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation));
        assert!(tasks.last().unwrap().verification_intents[0]
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation));
    }

    #[test]
    fn preferred_browser_evidence_remains_required_in_closure() {
        let mut tasks = vec![browser_task(1)];
        let mut groups = vec![TaskPlanGroup {
            group_id: "group-ui".to_string(),
            title: "UI".to_string(),
            objective: "Implement UI".to_string(),
            depends_on: vec![],
            scope_refs: vec![],
            acceptance_refs: vec![],
            task_ids: vec!["task-ui".to_string()],
        }];
        normalize_browser_verification_assignments(&mut tasks);
        tasks[0].verification_intents[0].preferred_evidence =
            vec![VerificationEvidence::BrowserAutomation];
        let profiles = derive_browser_verification_profiles(
            &contracts::BrowserAutomationFacts::default(),
            &tasks,
        );

        let closure_profiles =
            materialize_browser_quality_closure(&mut groups, &mut tasks, profiles);

        assert!(closure_profiles[0]
            .checks
            .iter()
            .all(|check| { check.enforcement == BrowserEvidenceEnforcement::Required }));
        assert!(matches!(
            tasks.last().unwrap().task_kind,
            TaskKind::BrowserQualityClosure
        ));
    }

    #[test]
    fn browser_closure_uses_collision_safe_ids_and_structural_detection() {
        let mut source = browser_task(1);
        source.verification_intents[0]
            .acceptable_evidence
            .push(VerificationEvidence::BrowserAutomation);
        let mut tasks = vec![
            source,
            TaskDefinition {
                task_id: "task-browser-quality-closure".to_string(),
                group_id: "group-browser-quality-closure".to_string(),
                title: "User task with reserved-looking id".to_string(),
                task_kind: TaskKind::FeatureIncrement,
                implementation_actions: vec![],
                implementation_obligations: vec![],
                objective: "Existing task".to_string(),
                depends_on: vec![],
                scope_refs: vec![],
                acceptance_refs: vec![],
                requirement_detail_refs: vec![],
                write_boundary: TaskWriteBoundary {
                    forbidden_paths: vec![],
                    artifact_refs: TaskArtifactRefs::default(),
                },
                verification_intents: vec![],
                concept_refs: vec![],
                concept_responsibilities: vec![],
                concept_verification_intents: vec![],
                frontend_experience_requirement: None,
                runtime_delivery_requirement: None,
                engineering_quality_requirement_refs: vec![],
                architecture_quality_requirement_refs: vec![],
                api_contract_requirement_refs: vec![],
                code_quality_requirement_refs: vec![],
            },
        ];
        let mut groups = vec![
            TaskPlanGroup {
                group_id: "group-ui".to_string(),
                title: "UI".to_string(),
                objective: "Implement UI".to_string(),
                depends_on: vec![],
                scope_refs: vec![],
                acceptance_refs: vec![],
                task_ids: vec!["task-ui".to_string()],
            },
            TaskPlanGroup {
                group_id: "group-browser-quality-closure".to_string(),
                title: "Existing".to_string(),
                objective: "Existing group".to_string(),
                depends_on: vec![],
                scope_refs: vec![],
                acceptance_refs: vec![],
                task_ids: vec!["task-browser-quality-closure".to_string()],
            },
        ];
        let profiles = derive_browser_verification_profiles(
            &contracts::BrowserAutomationFacts::default(),
            &tasks,
        );

        materialize_browser_quality_closure(&mut groups, &mut tasks, profiles);

        let closure = tasks
            .iter()
            .find(|task| matches!(task.task_kind, TaskKind::BrowserQualityClosure))
            .unwrap();
        assert_eq!(closure.task_id, "task-browser-quality-closure-2");
        assert_eq!(closure.group_id, "group-browser-quality-closure-2");
        assert_eq!(
            browser_quality_closure_group(&groups, &tasks).map(|group| group.group_id.as_str()),
            Some("group-browser-quality-closure-2")
        );
    }
}
