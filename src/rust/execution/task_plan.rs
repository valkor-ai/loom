use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    architecture::ArchitectureDetailCoverageEntry, build_code_quality_seed, code_quality_enum_refs,
    code_reference_load_plan, code_reference_selection_for_task, execution::TaskArtifactRefs,
    package_naming_policy_for_reference_groups, planning::RequirementDetailItem,
    ui_surface_decision_enum_refs, AcceptancePriority, ApiContractRequirement,
    ArchitectureArtifactContract, ArchitectureQualityRequirement, BrowserEvidenceEnforcement,
    BrowserRunnerSource, BrowserVerificationMode, BrowserVerificationProfile,
    CodeQualityRequirement, CoverageStatus, EngineeringQualityRequirement, ImplementationAction,
    ReferenceLoadPlanItem, TaskDefinition, TaskGroupRunState, TaskKind, TaskPlan, TaskPlanGroup,
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
    "implement_entity_lifecycle",
    "add_or_update_tests",
    "add_or_update_config",
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
    let architecture_quality_template = architecture_quality_requirement_template(aac);
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
    if !architecture_quality_template.is_null() {
        output_contract["architectureQualityRequirementTemplate"] =
            architecture_quality_template.clone();
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
                &architecture_quality_template,
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
    architecture_quality_template: &Value,
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
        "contextProjection.requirementDetailTransfer.requirementDetailAssignment",
        "contextProjection.requirementDetailTransfer.currentPhaseScope",
        "contextProjection.requirementDetailTransfer.acceptanceDetails",
        "contextProjection.requirementDetailTransfer.businessFlowDetails",
        "contextProjection.requirementDetailTransfer.objectOperationDetailRules",
        "contextProjection.requirementDetailTransfer.architectureDetails",
        "contextProjection.requirementDetailTransfer.workflowClosureRequirements",
        "contextProjection.requirementDetailTransfer.conceptRefs",
        "contextProjection.requirementDetailTransfer.taskPlanningFieldMapping",
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
            "codeQualitySeed.techReferenceProfile.loadMode",
            "codeQualitySeed.techReferenceProfile.groups.code",
            "codeQualitySeed.techReferenceProfile.referenceLoadPlan",
            "codeQualitySeed.generationRules",
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
            "purpose": "Read current phase source refs, requirement transfer, and allowed refs before writing the TaskPlan outline.",
            "whenToRead": "Read first.",
            "selectors": read_selectors_value_from_paths(core_fields)
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
                architecture_quality_template,
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
    architecture_quality_template: &Value,
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
    if !architecture_quality_template.is_null() {
        fields.push("outputContract.architectureQualityRequirementTemplate");
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
    let engineering_quality_requirements =
        normalize_engineering_quality_requirements(&baseline, &mut tasks);
    let architecture_quality_requirements =
        normalize_architecture_quality_requirements(&aac, &mut tasks);
    let api_contract_requirements =
        normalize_api_contract_requirements(&aac, &mut tasks, &allowed_refs);
    let code_quality_requirements = normalize_code_quality_requirements(&baseline, &mut tasks);
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
    normalize_frontend_experience_requirements(tasks, aac);
    normalize_verification_detail_parent_refs(tasks);
    normalize_missing_requirement_detail_task_refs(tasks, pgc, aac);
    normalize_task_verification_detail_refs(tasks, pgc, aac);
    normalize_runtime_delivery_closure_group(groups, tasks, aac.runtime_delivery.as_ref());
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

    for task in tasks {
        let Some(requirement) = task.runtime_delivery_requirement.as_mut() else {
            continue;
        };
        if !requirement.applies_to_this_task {
            requirement.runtime_delivery_ref = None;
            requirement.affected_contract_fields.clear();
            requirement.required_code_level_checks.clear();
            continue;
        }

        requirement.runtime_delivery_ref = Some(runtime_ref.to_string());
        let is_closure = matches!(task.task_kind, TaskKind::RuntimeDeliveryClosure);
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
        let Some(requirement) = task.frontend_experience_requirement.as_mut() else {
            let mut normalized = template.clone();
            normalize_ui_task_scope(
                &mut normalized,
                &template,
                &task_kind,
                &implementation_actions,
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
        normalize_ui_task_scope(requirement, &template, &task_kind, &implementation_actions);
        if let Some(surface_contract) = expected_surface_contract.as_ref() {
            requirement["uiSurfaceDecisionContractRef"] = json!(
                "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract"
            );
            normalize_ui_surface_ownership(requirement, surface_contract);
            if let Some(object) = requirement.as_object_mut() {
                object.remove("uiQualityContractRef");
                object.remove("uiQualityContract");
                object.remove("uiTaskQualityGates");
            }
        }
    }
}

fn normalize_ui_task_scope(
    requirement: &mut Value,
    template: &Value,
    task_kind: &TaskKind,
    implementation_actions: &[ImplementationAction],
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
    let mut dimensions = existing_ui_ownership_dimensions(requirement);
    push_unique_strings(
        &mut dimensions,
        derived_ui_ownership_dimensions(requirement, task_kind, implementation_actions),
    );
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

fn existing_ui_ownership_dimensions(requirement: &Value) -> Vec<String> {
    requirement
        .pointer("/uiTaskScope/ownershipDimensions")
        .and_then(Value::as_array)
        .map(|items| {
            unique_strings(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|dimension| UI_OWNERSHIP_DIMENSION_VALUES.contains(dimension))
                    .map(str::to_string)
                    .collect(),
            )
        })
        .unwrap_or_default()
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
    if task_kind_is_frontend(task_kind)
        || implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateUiFlow
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

fn normalize_verification_detail_parent_refs(tasks: &mut [TaskDefinition]) {
    for task in tasks {
        let intent_detail_refs = task
            .verification_intents
            .iter()
            .flat_map(|intent| intent.requirement_detail_refs.iter().cloned())
            .collect::<Vec<_>>();
        for detail_ref in intent_detail_refs {
            push_unique(&mut task.requirement_detail_refs, detail_ref);
        }
    }
}

fn normalize_missing_requirement_detail_task_refs(
    tasks: &mut [TaskDefinition],
    pgc: &contracts::PlanningGenerationContract,
    aac: &ArchitectureArtifactContract,
) {
    let coverage_by_detail = aac
        .detail_coverage
        .iter()
        .filter(|entry| matches!(entry.coverage_status, CoverageStatus::Covered))
        .map(|entry| (entry.detail_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if coverage_by_detail.is_empty() {
        return;
    }
    let mut assigned_detail_ids = tasks
        .iter()
        .flat_map(|task| task.requirement_detail_refs.iter().cloned())
        .collect::<BTreeSet<_>>();
    for detail in pgc
        .requirement_details
        .items
        .iter()
        .filter(|detail| detail.required_for_current_phase)
    {
        if assigned_detail_ids.contains(&detail.detail_id) {
            continue;
        }
        let Some(coverage) = coverage_by_detail.get(detail.detail_id.as_str()) else {
            continue;
        };
        if let Some(task_index) = infer_requirement_detail_owner_task(tasks, detail, coverage) {
            push_unique(
                &mut tasks[task_index].requirement_detail_refs,
                detail.detail_id.clone(),
            );
            assigned_detail_ids.insert(detail.detail_id.clone());
        }
    }
}

fn infer_requirement_detail_owner_task(
    tasks: &[TaskDefinition],
    detail: &RequirementDetailItem,
    coverage: &ArchitectureDetailCoverageEntry,
) -> Option<usize> {
    let scored = tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| task_can_own_requirement_detail(task))
        .filter_map(|(index, task)| {
            let score = requirement_detail_owner_score(task, detail, coverage);
            (score > 0).then_some((index, score))
        })
        .collect::<Vec<_>>();
    let max_score = scored.iter().map(|(_, score)| *score).max()?;
    let winners = scored
        .into_iter()
        .filter(|(_, score)| *score == max_score)
        .collect::<Vec<_>>();
    if winners.len() == 1 {
        Some(winners[0].0)
    } else {
        None
    }
}

fn task_can_own_requirement_detail(task: &TaskDefinition) -> bool {
    !matches!(
        task.task_kind,
        TaskKind::VerificationIncrement | TaskKind::RuntimeDeliveryClosure
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
    if artifact_score == 0 && acceptance_score == 0 && concept_score == 0 {
        return 0;
    }
    artifact_score
        + acceptance_score
        + concept_score
        + semantic_detail_owner_score(task, detail, coverage)
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

fn semantic_detail_owner_score(
    task: &TaskDefinition,
    detail: &RequirementDetailItem,
    coverage: &ArchitectureDetailCoverageEntry,
) -> u32 {
    let mut score = 0;
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
            scope_refs: tasks[closure_task_index].scope_refs.clone(),
            acceptance_refs: tasks[closure_task_index].acceptance_refs.clone(),
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
        if group.scope_refs.is_empty() {
            group.scope_refs = tasks[closure_task_index].scope_refs.clone();
        }
        if group.acceptance_refs.is_empty() {
            group.acceptance_refs = tasks[closure_task_index].acceptance_refs.clone();
        }
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
            validate_ui_surface_ownership(task, requirement, surface_contract, &mut issues);
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
    let task_detail_ids = tasks
        .iter()
        .flat_map(|task| task.requirement_detail_refs.iter().cloned())
        .collect::<BTreeSet<_>>();
    let verification_detail_ids = tasks
        .iter()
        .flat_map(|task| {
            task.verification_intents
                .iter()
                .flat_map(|intent| intent.requirement_detail_refs.iter().cloned())
        })
        .collect::<BTreeSet<_>>();

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
        if !task_detail_ids.contains(&detail.detail_id) {
            issues.push(issue(
                "DETAIL_TASK_ASSIGNMENT_MISSING",
                "tasks[].requirementDetailRefs",
                "Every covered current-phase requirement detail must be assigned to at least one task.",
                Some(&detail.detail_id),
            ));
        }
        if !verification_detail_ids.contains(&detail.detail_id) {
            issues.push(issue(
                "DETAIL_TASK_ASSIGNMENT_MISSING",
                "tasks[].verificationIntents[].requirementDetailRefs",
                "Every covered current-phase requirement detail must be assigned to at least one verification intent.",
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
            .interfaces
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
        .filter(|task| task_requires_browser_verification(task))
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
    let mut scope_refs = BTreeSet::new();
    let mut acceptance_refs = BTreeSet::new();
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
        scope_refs.extend(source_task.scope_refs.iter().cloned());
        acceptance_refs.extend(source_task.acceptance_refs.iter().cloned());
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
            acceptance_refs.extend(intent.acceptance_refs.iter().cloned());
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
        objective: "Create or adapt the task-scoped browser checks and close required rendered, interaction, and workflow evidence.".to_string(),
        depends_on: Vec::new(),
        scope_refs: scope_refs.iter().cloned().collect(),
        acceptance_refs: acceptance_refs.iter().cloned().collect(),
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
        scope_refs: scope_refs.into_iter().collect(),
        acceptance_refs: acceptance_refs.into_iter().collect(),
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
            "included": pgc.phase_scope.included,
            "deferred": pgc.phase_scope.deferred,
            "excluded": pgc.phase_scope.excluded
        },
        "acceptanceDetails": pgc.phase_scope.acceptance_candidates,
        "businessFlowDetails": pgc.planning_inputs.business_flows,
        "objectOperationDetailRules": {
            "taskAssignmentRule": "Task objectives and verification intents must preserve concrete objects, operations, fields, states, blocking rules, and feedback when present.",
            "evidenceRule": "TaskResult must be able to show which concrete behavior was implemented or verified."
        },
        "architectureDetails": {
            "modules": aac.modules,
            "entities": aac.data_model.get("entities").cloned().unwrap_or(Value::Array(vec![])),
            "interfaces": aac.interfaces,
            "userFlows": aac.user_flows,
            "stateMachines": aac.state_machines,
            "frontendOperationPathDetails": frontend_operation_path_details(aac)
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
        hints.insert(
            "fieldRefs".to_string(),
            json!({
                "count": fields.len(),
                "examples": fields.iter().take(6).cloned().collect::<Vec<_>>(),
                "fullSource": "contextProjection.requirementDetailTransfer.architectureDetails.entities"
            }),
        );
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
            "stateExpectation": ["loading", "success", "error", "empty", "business_blocking"]
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
        requirement["uiSurfaceOwnership"] = ui_surface_ownership_template(surface_contract);
    }
    requirement
}

fn ui_surface_ownership_template(surface_contract: &Value) -> Value {
    json!({
        "source": "AAC frontendExperience.uiSurfaceDecisionContract",
        "selectionRule": "Select only the UI contract regions, actions, states, and quality rules this task owns. Do not copy unrelated regions or rules into the task.",
        "patternDecision": {
            "mode": surface_contract.pointer("/patternDecision/mode").cloned().unwrap_or(Value::Null),
            "knownPattern": surface_contract.pointer("/patternDecision/knownPattern").cloned().unwrap_or(Value::Null),
            "primaryKnownPattern": surface_contract.pointer("/patternDecision/primaryKnownPattern").cloned().unwrap_or(Value::Null),
            "customPattern": surface_contract.pointer("/patternDecision/customPattern").cloned().unwrap_or(Value::Null)
        },
        "availableRegionIds": contract_string_ids(surface_contract, "/regionModel", "regionId").into_iter().collect::<Vec<_>>(),
        "availableActionIds": contract_string_ids(surface_contract, "/actionModel", "actionId").into_iter().collect::<Vec<_>>(),
        "availableStateKinds": contract_string_ids(surface_contract, "/stateModel", "state").into_iter().collect::<Vec<_>>(),
        "availableQualityRuleIds": contract_string_ids(surface_contract, "/qualityRules", "ruleId").into_iter().collect::<Vec<_>>(),
        "regionIdsInScope": [],
        "actionIdsInScope": [],
        "stateKindsInScope": [],
        "qualityRuleIdsInScope": [],
        "contentBoundaryApplies": true,
        "responsiveCoverageRequired": true
    })
}

fn normalize_ui_surface_ownership(requirement: &mut Value, surface_contract: &Value) {
    if !requirement
        .get("uiSurfaceOwnership")
        .is_some_and(|scope| scope.is_object())
    {
        requirement["uiSurfaceOwnership"] = ui_surface_ownership_template(surface_contract);
    }
}

fn validate_ui_surface_ownership(
    task: &TaskDefinition,
    requirement: &Value,
    surface_contract: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(ownership) = requirement
        .get("uiSurfaceOwnership")
        .filter(|value| value.is_object())
    else {
        issues.push(issue(
            "FRONTEND_UI_SURFACE_OWNERSHIP_REQUIRED",
            "tasks[].frontendExperienceRequirement.uiSurfaceOwnership",
            "Frontend tasks must declare the task-owned uiSurfaceDecisionContract regions, actions, states, and quality rules.",
            Some(&task.task_id),
        ));
        return;
    };
    validate_surface_scope_ids(
        task,
        ownership,
        "regionIdsInScope",
        surface_contract,
        "/regionModel",
        "regionId",
        "FRONTEND_UI_SURFACE_REGION_SCOPE_INVALID",
        issues,
    );
    validate_surface_scope_ids(
        task,
        ownership,
        "actionIdsInScope",
        surface_contract,
        "/actionModel",
        "actionId",
        "FRONTEND_UI_SURFACE_ACTION_SCOPE_INVALID",
        issues,
    );
    validate_surface_scope_ids(
        task,
        ownership,
        "stateKindsInScope",
        surface_contract,
        "/stateModel",
        "state",
        "FRONTEND_UI_SURFACE_STATE_SCOPE_INVALID",
        issues,
    );
    validate_surface_scope_ids(
        task,
        ownership,
        "qualityRuleIdsInScope",
        surface_contract,
        "/qualityRules",
        "ruleId",
        "FRONTEND_UI_SURFACE_RULE_SCOPE_INVALID",
        issues,
    );
}

fn validate_surface_scope_ids(
    task: &TaskDefinition,
    ownership: &Value,
    ownership_key: &str,
    surface_contract: &Value,
    contract_pointer: &str,
    contract_id_key: &str,
    issue_code: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(items) = ownership.get(ownership_key).and_then(Value::as_array) else {
        issues.push(issue(
            issue_code,
            &format!("tasks[].frontendExperienceRequirement.uiSurfaceOwnership.{ownership_key}"),
            "uiSurfaceOwnership scope fields must be arrays.",
            Some(&task.task_id),
        ));
        return;
    };
    let allowed = contract_string_ids(surface_contract, contract_pointer, contract_id_key);
    for item in items {
        let Some(value) = item.as_str() else {
            issues.push(issue(
                issue_code,
                &format!(
                    "tasks[].frontendExperienceRequirement.uiSurfaceOwnership.{ownership_key}"
                ),
                "uiSurfaceOwnership scope entries must be strings.",
                Some(&task.task_id),
            ));
            return;
        };
        if !allowed.is_empty() && !allowed.contains(value) {
            issues.push(issue(
                issue_code,
                &format!(
                    "tasks[].frontendExperienceRequirement.uiSurfaceOwnership.{ownership_key}"
                ),
                "uiSurfaceOwnership scope entries must reference ids declared by AAC uiSurfaceDecisionContract.",
                Some(&task.task_id),
            ));
            return;
        }
    }
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
    json!({
        "required": frontend.get("required").cloned().unwrap_or(Value::Bool(false)),
        "experienceLevel": frontend.get("experienceLevel").cloned().unwrap_or(Value::Null),
        "surfaces": frontend.get("surfaces").cloned().unwrap_or(Value::Array(vec![])),
        "dataViews": frontend.get("dataViews").cloned().unwrap_or(Value::Array(vec![])),
        "actions": frontend.get("actions").cloned().unwrap_or(Value::Array(vec![])),
        "operationPaths": frontend.get("operationPaths").cloned().unwrap_or(Value::Array(vec![])),
        "uiSurfaceRegistry": frontend.get("uiSurfaceRegistry").cloned().unwrap_or(Value::Null),
        "uiSurfaceDecisionContract": frontend.get("uiSurfaceDecisionContract").cloned().unwrap_or(Value::Null),
        "sourceRefs": frontend.get("sourceRefs").cloned().unwrap_or(Value::Null)
    })
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
        let steps = array_at(flow, "steps");
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
            let mut candidate_interface_refs = string_array_at(step, "interfaceRefs");
            if candidate_interface_refs.is_empty() {
                candidate_interface_refs = string_array_at(flow, "interfaceRefs");
            }
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
                    "rule": "Generated from AAC frontendExperience surfaces or operationPaths, user interaction flow steps, and executable interfaces with request/response shape."
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

fn push_unique_strings(values: &mut Vec<String>, next: Vec<String>) {
    values.extend(next);
    *values = unique_strings(std::mem::take(values));
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
            "acceptNormalization": "loom.taskPlanAcceptFile deterministically fills a missing detail owner only when exactly one owner task can be inferred; ambiguous ownership remains repairable."
        },
        "conceptGroundingRules": {
            "phaseConceptGroundingRef": "sourceRefs.phaseConceptGroundingRef",
            "rule": "Bind high-risk business concepts when the current task owns their rule, state, field, or operation meaning."
        },
        "frontendExperienceRules": {
            "required": aac.frontend_experience.as_ref().and_then(|value| value.get("required")).and_then(Value::as_bool).unwrap_or(false),
            "requirementTemplate": "outputContract.frontendExperienceRequirementTemplate",
            "uiSurfaceDecisionContractSource": "outputContract.frontendExperienceRequirementTemplate.uiSurfaceDecisionContractRef",
            "uiSurfaceOwnershipSource": "outputContract.frontendExperienceRequirementTemplate.uiSurfaceOwnership",
            "rule": "When frontendExperience is required, UI responsibilities must be visible in task objective, verification intents, and frontendExperienceRequirement.",
            "taskScopeRule": "Tasks that own UI surfaces, workflows, states, bindings, data views, actions, layout, visual system, or content boundary must fill frontendExperienceRequirement.uiSurfaceOwnership from AAC uiSurfaceDecisionContract and uiTaskScope from related frontend arrays. Select only the current task's regions, actions, states, quality rules, surfaces, data views, operation paths, bindings, and ownershipDimensions.",
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
            "rule": "Runtime-affecting tasks must carry runtimeDeliveryRequirement; final runtime closure is required when runtimeDelivery.status=modified.",
            "closureTaskRule": "When outputContract.runtimeDeliveryClosureTaskTemplate is present, create exactly one task with taskKind=runtime_delivery_closure and copy its runtimeDeliveryRequirement scope and evidence expectations from that template. Loom derives runtimeDeliveryRef, contractField, and checkId values during accept.",
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
        "architectureQualityRules": {
            "requirementSource": "outputContract.architectureQualityRequirementTemplate",
            "architectureQualitySource": "sourceRefs.architectureArtifactContractRef#/architectureQuality",
            "referenceRule": "Use task.writeBoundary.artifactRefs.decisions, nfrs, and risks to assign current-phase architecture quality obligations. Do not inline full ADR, NFR, or risk objects inside tasks.",
            "assignmentRule": "Every current-phase decision, NFR, and risk that affects implementation or verification should be assigned to at least one task that owns the related module, interface, data model, runtime surface, or workflow.",
            "acceptNormalization": "loom.taskPlanAcceptFile deterministically materializes top-level architectureQualityRequirements and task architectureQualityRequirementRefs from task artifact refs.",
            "verificationRule": "Assigned tasks must include verificationIntents whose summaries can prove the referenced architecture decision, NFR, or risk mitigation was respected."
        },
        "apiContractRules": {
            "required": aac.interfaces.iter().any(is_http_api_interface),
            "requirementSource": "outputContract.apiContractRequirementTemplate",
            "interfaceSource": "contextProjection.apiInterfaces (copied from the accepted AAC interfaces)",
            "exposureSource": "contextProjection.apiContract (copied from the accepted AAC API contract)",
            "assignmentRule": "Tasks that create or change current-phase HTTP API implementations, frontend/client bindings, integration tests, or verification flows must include the accepted interface ids in task.writeBoundary.artifactRefs.interfaces. Loom derives one task-scoped API contract requirement for every such task; frontend/client tasks consume the contract without becoming API implementation owners.",
            "implementationRule": "API tasks must preserve request schema, response schema, status codes, error schema, auth policy, and pagination policy declared by the AAC interface.",
            "verificationRule": "API tasks should include verification intents that prove at least the declared success path and important business or validation error path. Collection endpoints should also prove declared pagination/filter behavior.",
            "nonDuplicationRule": "Do not inline full API requirements inside every task; use interface refs and the generated task apiContractRequirementRefs."
        },
        "codeQualityRules": {
            "required": code_quality_seed.get("required").and_then(Value::as_bool).unwrap_or(false),
            "seedSource": "codeQualitySeed",
            "requirementSource": "outputContract.codeQualityRequirementTemplate",
            "assignmentRule": "loom.taskPlanAcceptFile derives task codeQualityRequirementRefs from TechnicalBaseline stack signals and task scope; do not inline full code quality requirements inside every task.",
            "referenceRule": "Use codeQualitySeed.codeStackSignals to choose task scope and implementation practices. Do not write codeQualityRequirementRefs or inline reference paths; Loom derives the task-scoped requirement and referenceLoadPlan during accept. Groups are semantic evidence labels, not path maps.",
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
        "referenceGroupSource": "codeQualitySeed.techReferenceProfile.groups.code",
        "referenceLoadPlanSource": "Generated by loom.taskPlanAcceptFile from selected reference groups; agents must read only files listed in each requirement referenceLoadPlan.",
        "packageNamingPolicySource": "codeQualitySeed.packageNamingPolicy",
        "implementationObligations": code_quality_implementation_obligations(),
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

fn architecture_quality_requirement_template(aac: &ArchitectureArtifactContract) -> Value {
    if aac.architecture_quality.decisions.is_empty()
        && aac.architecture_quality.nfrs.is_empty()
        && aac.architecture_quality.risks.is_empty()
    {
        return Value::Null;
    }
    json!({
        "requirementId": "aqr-current-001",
        "kind": "architecture_quality",
        "decisionRefs": aac.architecture_quality.decisions.iter().map(|item| item.decision_id.clone()).collect::<Vec<_>>(),
        "nfrRefs": aac.architecture_quality.nfrs.iter().map(|item| item.nfr_id.clone()).collect::<Vec<_>>(),
        "riskRefs": aac.architecture_quality.risks.iter().map(|item| item.risk_id.clone()).collect::<Vec<_>>(),
        "implementationObligations": [
            "Respect the referenced architecture decision in changed modules, interfaces, data model, runtime, and workflow code.",
            "Implement the referenced risk mitigation when the task owns the affected artifact.",
            "Keep the referenced NFR verifiable through task verification evidence when it applies to changed code."
        ],
        "verificationObligations": [
            "Use task.verificationIntents as the verification id source.",
            "TaskResult architectureQualityEvidence must cite requirementId and verificationIds for the task-owned architecture quality refs.",
            "Verification summaries should state how implementation respected the referenced decision, NFR, or risk mitigation."
        ],
        "taskRefRule": "Loom attaches generated requirements through architectureQualityRequirementRefs during accept; agents must not write that field or duplicate full decisions, NFRs, or risks inside every task."
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
        "implementationObligations": api_contract_implementation_obligations(),
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
            implementation_obligations: vec![
                "Respect referenced architecture decisions in the task-owned implementation."
                    .to_string(),
                "Implement referenced risk mitigations when the task owns affected code or configuration."
                    .to_string(),
                "Keep referenced NFRs verifiable through task verification evidence.".to_string(),
            ],
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

fn normalize_api_contract_requirements(
    aac: &ArchitectureArtifactContract,
    tasks: &mut [TaskDefinition],
    allowed_refs: &Value,
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
            .interfaces
            .iter()
            .filter(|interface_ref| http_api_refs.contains(*interface_ref))
            .cloned()
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
                .flat_map(|flow| string_array_at(flow, "interfaceRefs"))
                .filter(|interface_ref| {
                    http_api_refs.contains(interface_ref)
                        && allowed_interface_refs.contains(interface_ref)
                })
                .collect::<Vec<_>>();
            interface_refs.sort();
            interface_refs.dedup();
            if !interface_refs.is_empty() {
                task.write_boundary.artifact_refs.interfaces = interface_refs.clone();
            }
        }
        if interface_refs.is_empty() || !task_uses_api_contract(task) {
            continue;
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
            interface_refs,
            implementation_obligations: api_contract_implementation_obligations(),
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

fn normalize_code_quality_requirements(
    baseline: &contracts::TechnicalBaselineContract,
    tasks: &mut [TaskDefinition],
) -> Vec<CodeQualityRequirement> {
    let mut requirements = Vec::new();
    let mut requirement_ids_by_task = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.iter() {
        let Some(selection) = code_reference_selection_for_task(baseline, task) else {
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
            implementation_obligations: code_quality_implementation_obligations_for_selection(
                &selection,
            ),
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

fn code_quality_implementation_obligations() -> Vec<String> {
    vec![
        "Load only the files listed in sourceContext.codeQualityExecutionContext[].referenceLoadPlan for this task; do not scan the whole tech/code or tech/backend trees or infer group-to-file mappings.".to_string(),
        "Follow existing repository structure and style before introducing new language or framework patterns.".to_string(),
        "Keep API, UI, architecture, and persistence obligations in their own dedicated quality contracts; use code quality only for language and framework implementation discipline.".to_string(),
    ]
}

fn code_quality_implementation_obligations_for_selection(
    selection: &contracts::CodeReferenceSelection,
) -> Vec<String> {
    let mut obligations = code_quality_implementation_obligations();
    if package_naming_policy_for_reference_groups(&selection.reference_groups).is_some() {
        obligations.push("For JVM production source, choose a professional base package from existing package roots, build group metadata, or confirmed organization/project identity; if none exists, use app.<project_slug> and only fall back to app.generated when no stable project slug can be derived.".to_string());
        obligations.push("Do not create or keep placeholder package roots such as com.example, org.example, com.company, com.demo, org.demo, com.sample, or org.sample in production source.".to_string());
    }
    obligations
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

fn api_contract_implementation_obligations() -> Vec<String> {
    vec![
        "Implement or preserve the AAC-declared method, path, resource, request schema, response schema, status codes, and error schema for task-owned HTTP APIs or task-owned client/test bindings.".to_string(),
        "Return actionable validation or business-blocking errors instead of silent success or generic server errors.".to_string(),
        "Apply pagination, filtering, auth, and contract file obligations only when the AAC interface declares them.".to_string(),
        "Keep frontend/client bindings aligned with the declared API response and error shapes when the task owns the binding.".to_string(),
    ]
}

fn api_contract_verification_obligations() -> Vec<String> {
    vec![
        "Use task.verificationIntents as verification id source.".to_string(),
        "Verify at least one declared success path for each task-owned API interface or client/test binding.".to_string(),
        "Verify important validation or business-blocking error behavior for write/state-transition APIs.".to_string(),
        "For collection APIs, verify the declared pagination or filtering behavior when present.".to_string(),
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
    matches!(task.task_kind, TaskKind::DataModelIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateEntity
                    | ImplementationAction::CreateOrUpdatePersistence
                    | ImplementationAction::CreateEntityMigration
                    | ImplementationAction::CreateEntityRepository
                    | ImplementationAction::CreateEntityCrud
            )
        })
}

fn task_backend_consumes_persistence_mapping(task: &TaskDefinition) -> bool {
    if task_is_frontend_task(task) || task.write_boundary.artifact_refs.entities.is_empty() {
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
    if has_non_empty_string(runtime_delivery, "/build/command") {
        fields.push("build.command".to_string());
    }
    if has_non_empty_string(runtime_delivery, "/start/command") {
        fields.push("start.command".to_string());
    }
    if has_non_empty_array(runtime_delivery, "/runtimeSurfaces") {
        fields.push("runtimeSurfaces".to_string());
    }
    if runtime_delivery.get("httpProbes").is_some() {
        fields.push("httpProbes".to_string());
    }
    if runtime_delivery
        .pointer("/deliveryMechanics/staticAssets")
        .is_some()
    {
        fields.push("deliveryMechanics.staticAssets".to_string());
    }
    if runtime_delivery.pointer("/deliveryMechanics/api").is_some() {
        fields.push("deliveryMechanics.api".to_string());
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
    if runtime_delivery_has_codegen(runtime_delivery) {
        fields.push("deliveryMechanics.codegen".to_string());
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

fn runtime_delivery_has_codegen(runtime_delivery: &Value) -> bool {
    let Some(codegen) = runtime_delivery.pointer("/deliveryMechanics/codegen") else {
        return false;
    };
    codegen
        .get("required")
        .and_then(Value::as_str)
        .is_some_and(|required| required != "no")
        || has_non_empty_array(codegen, "/commands")
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
                "uiTaskScope": {"surfacesInScope": ["surface-workbench"]},
                "uiSurfaceOwnership": {
                    "regionIdsInScope": ["region-main"],
                    "qualityRuleIdsInScope": ["verify.rendered_viewports"]
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
    fn ui_surface_ownership_template_keeps_task_scope_compact() {
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

        let ownership = ui_surface_ownership_template(&surface_contract);

        assert_eq!(
            ownership
                .pointer("/patternDecision/mode")
                .and_then(Value::as_str),
            Some("known")
        );
        assert!(
            ownership.get("referencePlan").is_none(),
            "TaskPlan ownership must not copy the full UI reference plan"
        );
        assert!(
            ownership.get("qualityRules").is_none(),
            "TaskPlan ownership must not copy the full UI rule list"
        );
        assert!(
            ownership
                .get("availableRegionIds")
                .and_then(Value::as_array)
                .is_some_and(|items| items
                    .iter()
                    .any(|item| item.as_str() == Some("region_primary"))),
            "TaskPlan ownership must expose available region ids"
        );
        assert!(
            ownership
                .get("regionIdsInScope")
                .and_then(Value::as_array)
                .is_some_and(|items| items.is_empty()),
            "TaskPlan ownership scope starts empty so the agent selects task-owned ids"
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
        task.frontend_experience_requirement.as_mut().unwrap()["uiSurfaceOwnership"]
            ["qualityRuleIdsInScope"] = json!([]);
        let mut tasks = vec![task];

        normalize_browser_verification_assignments(&mut tasks);

        assert!(!tasks[0].verification_intents[0]
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation));
        assert!(validate_browser_verification_assignments(&tasks).is_empty());
    }

    #[test]
    fn multiple_verification_intents_require_explicit_browser_owner() {
        let mut tasks = vec![browser_task(2)];

        normalize_browser_verification_assignments(&mut tasks);
        let issues = validate_browser_verification_assignments(&tasks);

        assert!(!tasks[0].verification_intents.iter().any(|intent| intent
            .acceptable_evidence
            .contains(&VerificationEvidence::BrowserAutomation)));
        assert!(issues
            .iter()
            .any(|issue| issue.code == "TASKPLAN_BROWSER_VERIFICATION_REQUIRED"));
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
