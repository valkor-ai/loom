use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    AcceptancePriority, ArchitectureArtifactContract, CoverageStatus, ImplementationAction,
    TaskDefinition, TaskGroupRunState, TaskKind, TaskPlan, TaskPlanGroup,
    TaskPlanGroupCandidateAgentWritable, TaskPlanHandoff, TaskPlanOutlineCandidateAgentWritable,
    TaskPlanPolicy, TaskPlanRun, TaskPlanRunNextAction, TaskPlanRunScheduler, TaskPlanRunStatus,
    TaskPlanRunSummary, TaskPlanScopeSnapshot, TaskPlanSource, TaskPlanStatus, TaskRunState,
    TaskRunStatus, VerificationEvidence,
};
use delivery_core::{
    apply_delivery_index, ArtifactKind, DeliveryLifecycleStatus, DomainDispatcher,
    ExecuteEditBoundary, ExecuteVerificationPolicy, FileSubmitInput, LoomMcpActionResult,
    LoomMcpAutoRunnableResult, LoomMcpFailure, LoomMcpFailureResult, LoomMcpNextAction,
    LoomMcpRepairableErrorResult, OperationContext, PostSubmitAction, RouteAction, RouteActionKind,
    SubmitAcceptedEvent, TransitionEngine, TransitionStore,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
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
    "manual_command_output",
    "runtime_api_check",
    "static_check",
    "agent_review_explanation",
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
    let source_refs = taskplan_source_refs(baseline_ref, planning_ref, architecture_ref, pgc);
    let outline_result_template =
        taskplan_outline_result_template(request_id, delivery_id, phase_id);
    let group_result_template = taskplan_group_result_template(request_id, delivery_id, phase_id);
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
    json!({
        "schemaVersion": "1.0",
        "requestType": "taskplan_grouped_generation",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::TaskPlanCandidate,
        "sourceRefs": source_refs,
        "contextProjection": {
            "phaseId": phase_id,
            "planningContractId": pgc.planning_contract_id,
            "architectureArtifactContractId": aac.architecture_artifact_contract_id,
            "technicalBaseline": {
                "technicalBaselineId": baseline.technical_baseline_id,
                "projectKind": baseline.project_kind,
                "stack": baseline.stack
            },
            "requirementDetailTransfer": requirement_transfer
        },
        "allowedRefs": allowed_refs(pgc, aac),
        "generationRules": generation_rules(aac),
        "enumRefs": enum_refs(),
        "outputContract": output_contract,
        "requestReadPlan": {
            "groups": taskplan_read_groups(
                &source_refs,
                &runtime_requirement_template,
                &runtime_closure_template
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
    runtime_requirement_template: &Value,
    runtime_closure_template: &Value,
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
    core_fields.extend([
        "contextProjection.phaseId",
        "contextProjection.planningContractId",
        "contextProjection.architectureArtifactContractId",
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
        "allowedRefs.riskRefs",
    ]);
    let groups = vec![
        json!({
            "groupId": "taskplan_core_context",
            "required": true,
            "purpose": "Read current phase source refs, requirement transfer, and allowed refs before writing the TaskPlan outline.",
            "whenToRead": "Read first.",
            "fields": core_fields
        }),
        json!({
            "groupId": "taskplan_generation_rules",
            "required": true,
            "purpose": "Read grouping, reference, verification, frontend, workflow, and runtime rules.",
            "whenToRead": "Read after core context and before writing group files.",
            "fields": [
                "generationRules.groupedOutputRules",
                "generationRules.scopeAndReferenceRules",
                "generationRules.writeBoundaryRules",
                "generationRules.verificationEvidenceRules",
                "generationRules.conceptGroundingRules",
                "generationRules.frontendExperienceRules",
                "generationRules.workflowClosureRules",
                "generationRules.runtimeDeliveryRules"
            ]
        }),
        json!({
            "groupId": "taskplan_candidate_contract",
            "required": true,
            "purpose": "Read output paths, schema shapes, and enum refs before writing candidates.",
            "whenToRead": "Read before writing output files.",
            "fields": taskplan_candidate_contract_fields(
                runtime_requirement_template,
                runtime_closure_template
            )
        }),
    ];
    Value::Array(groups)
}

fn taskplan_candidate_contract_fields(
    runtime_requirement_template: &Value,
    runtime_closure_template: &Value,
) -> Vec<&'static str> {
    let mut fields = vec![
        "enumRefs.taskKind",
        "enumRefs.implementationAction",
        "enumRefs.verificationEvidence",
        "outputContract.outlineFile",
        "outputContract.groupFilePattern",
        "outputContract.pathAuthority",
        "outputContract.outlineResultTemplate",
        "outputContract.groupResultTemplate",
    ];
    if !runtime_requirement_template.is_null() {
        fields.push("outputContract.runtimeDeliveryRequirementTemplate");
    }
    if !runtime_closure_template.is_null() {
        fields.push("outputContract.runtimeDeliveryClosureTaskTemplate");
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
        "riskRefs": value_field(&fields, "allowedRefs.riskRefs")
    });
    let outline_value = read_project_json_value(root, &outline_ref)?;
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
    let mut issues = validate_outline(&outline, &authorized.request_id, &delivery_id, &phase_id);
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
        let group_value = read_project_json_value(root, &group_file)?;
        let candidate: TaskPlanGroupCandidateAgentWritable = match deserialize_candidate(
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
        issues.extend(validate_group_candidate(
            &candidate,
            group,
            &authorized.request_id,
            &delivery_id,
            &phase_id,
        ));
        groups.push(candidate.group.clone());
        tasks.extend(candidate.tasks);
    }
    issues.extend(validate_taskplan_graph(&groups, &tasks));
    issues.extend(validate_taskplan_refs(&groups, &tasks, &allowed_refs));
    issues.extend(validate_runtime_delivery_requirements(&tasks));
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, outline_ref, issues, mode));
    }

    let planning_ref = string_field(&fields, "sourceRefs.planningGenerationContractRef")?;
    let architecture_ref = string_field(&fields, "sourceRefs.architectureArtifactContractRef")?;
    let baseline_ref = string_field(&fields, "sourceRefs.technicalBaselineRef")?;
    let baseline: contracts::TechnicalBaselineContract = read_project_json(root, &baseline_ref)?;
    let pgc: contracts::PlanningGenerationContract = read_project_json(root, &planning_ref)?;
    let aac: ArchitectureArtifactContract = read_project_json(root, &architecture_ref)?;
    issues.extend(validate_must_acceptance_task_coverage(&tasks, &pgc));
    issues.extend(validate_frontend_task_presence(&tasks, &aac));
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

fn validate_outline(
    outline: &TaskPlanOutlineCandidateAgentWritable,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if outline.request_id != request_id {
        issues.push(issue(
            "REQUEST_ID_MISMATCH",
            "outline.requestId",
            "TaskPlan outline requestId must match the active request.",
            Some("outline"),
        ));
    }
    if outline.delivery_id != delivery_id {
        issues.push(issue(
            "DELIVERY_ID_MISMATCH",
            "outline.deliveryId",
            "TaskPlan outline deliveryId must match the active delivery.",
            Some("outline"),
        ));
    }
    if outline.phase_id != phase_id {
        issues.push(issue(
            "PHASE_ID_MISMATCH",
            "outline.phaseId",
            "TaskPlan outline phaseId must match the active phase.",
            Some("outline"),
        ));
    }
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
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let target = Some(candidate.group.group_id.as_str());
    if candidate.request_id != request_id {
        issues.push(issue(
            "REQUEST_ID_MISMATCH",
            "group.requestId",
            "TaskPlan group requestId must match the active request.",
            target,
        ));
    }
    if candidate.delivery_id != delivery_id {
        issues.push(issue(
            "DELIVERY_ID_MISMATCH",
            "group.deliveryId",
            "TaskPlan group deliveryId must match the active delivery.",
            target,
        ));
    }
    if candidate.phase_id != phase_id {
        issues.push(issue(
            "PHASE_ID_MISMATCH",
            "group.phaseId",
            "TaskPlan group phaseId must match the active phase.",
            target,
        ));
    }
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
    if aac
        .frontend_experience
        .as_ref()
        .and_then(|value| value.get("required"))
        .and_then(Value::as_bool)
        != Some(true)
    {
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
    if groups.last().map(|group| group.group_id.as_str()) != Some(closure_group.group_id.as_str()) {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].position",
            "runtime_delivery_closure group must be the final TaskPlan group.",
            Some(&closure_group.group_id),
        ));
    }
    for group in groups
        .iter()
        .filter(|group| group.depends_on.contains(&closure_group.group_id))
    {
        issues.push(issue(
            "RUNTIME_CLOSURE_GROUP_INVALID",
            "groups[].dependsOn",
            "No TaskPlan group may depend on the final runtime_delivery_closure group.",
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
            json!({
                "detailId": item.detail_id,
                "kind": item.kind,
                "title": item.title,
                "summary": item.summary,
                "priority": item.priority,
                "impactTags": item.impact_tags,
                "lifecycleStage": item.lifecycle_stage,
                "quality": item.quality,
                "scopeRefs": item.scope_refs,
                "acceptanceRefs": item.acceptance_refs,
                "conceptRefs": item.concept_refs,
                "frontendRefs": item.frontend_refs,
                "coverageStatus": coverage
                    .and_then(|value| value.get("coverageStatus"))
                    .cloned()
                    .unwrap_or_else(|| Value::String("uncovered".to_string())),
                "artifactRefs": coverage
                    .and_then(|value| value.get("artifactRefs"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "coverageReason": coverage
                    .and_then(|value| value.get("reason"))
                    .cloned()
                    .unwrap_or(Value::Null)
            })
        })
        .collect::<Vec<_>>();
    json!({
        "authority": "planning_generation_contract_plus_architecture_artifact_contract",
        "requirementDetailAssignment": {
            "items": requirement_items,
            "assignmentRule": "Every item with coverageStatus=covered must be assigned to at least one task.requirementDetailRefs entry using its detailId.",
            "verificationRule": "Every assigned covered detail should be referenced by at least one verificationIntents[].requirementDetailRefs entry that proves the concrete behavior.",
            "insufficientAacRule": "If a required detail has coverageStatus other than covered because AAC lacks a taskable artifact, write blocked output with blockedReasonCode AAC_INSUFFICIENT instead of inventing vague tasks."
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
            "frontendOperationPathDetails": aac.frontend_experience
        },
        "workflowClosureRequirements": workflow_closure_requirements(aac),
        "conceptRefs": {
            "deliveryConceptGlossaryRef": pgc.context_refs.delivery_concept_glossary_ref,
            "phaseConceptGroundingRef": pgc.context_refs.phase_concept_grounding_ref
        },
        "taskPlanningFieldMapping": {
            "taskObjective": "Name the concrete business object, rule, flow, state, UI, API, operation path, blocking detail, or feedback detail the task owns.",
            "taskRequirementDetailRefs": "Use detailId values from requirementDetailAssignment.items.",
            "frontendExperienceRequirement": "Use when the task owns UI surfaces, workflows, states, bindings, or operation paths.",
            "runtimeDeliveryRequirement": "Use when the task touches build, start, runtime entry, static serving, generated artifacts, or runtime surface."
        }
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
        "decisionRefs": ids_from_value_array(&aac.risks_and_decisions, "/decisions", "decisionId"),
        "riskRefs": ids_from_value_array(&aac.risks_and_decisions, "/risks", "riskId")
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

fn generation_rules(aac: &ArchitectureArtifactContract) -> Value {
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
            "Prefer the smallest stable verification signal that proves the user-visible behavior or contract obligation.",
            "Avoid broad snapshots or weak no-op checks as the primary verification evidence."
        ],
        "conceptGroundingRules": {
            "phaseConceptGroundingRef": "sourceRefs.phaseConceptGroundingRef",
            "rule": "Bind high-risk business concepts when the current task owns their rule, state, field, or operation meaning."
        },
        "frontendExperienceRules": {
            "required": aac.frontend_experience.as_ref().and_then(|value| value.get("required")).and_then(Value::as_bool).unwrap_or(false),
            "rule": "When frontendExperience is required, UI responsibilities must be visible in task objective, verification intents, and frontendExperienceRequirement."
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
            "rule": "Runtime-affecting tasks must carry runtimeDeliveryRequirement; final runtime closure is required when runtimeDelivery.status=modified."
        }
    })
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
        "runtimeDeliveryRequirement": {
            "appliesToThisTask": true,
            "reason": "Final code-level closure for the RuntimeDeliveryContract.",
            "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
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
        "checkId": runtime_delivery_closure_check_id(contract_field),
        "contractField": contract_field,
        "objective": format!("Confirm {contract_field} is closed at code level against RuntimeDeliveryContract."),
        "acceptableEvidence": acceptable_evidence_for_runtime_closure_field(contract_field)
    })
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
        "verificationEvidence": VERIFICATION_EVIDENCE_VALUES
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
