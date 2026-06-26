use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    ArchitectureArtifactContract, TaskDefinition, TaskGroupRunState, TaskPlan, TaskPlanGroup,
    TaskPlanGroupCandidateAgentWritable, TaskPlanHandoff, TaskPlanOutlineCandidateAgentWritable,
    TaskPlanPolicy, TaskPlanRun, TaskPlanRunNextAction, TaskPlanRunScheduler, TaskPlanRunStatus,
    TaskPlanRunSummary, TaskPlanScopeSnapshot, TaskPlanSource, TaskPlanStatus, TaskRunState,
    TaskRunStatus,
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
use crate::templates::{taskplan_group_result_template, taskplan_outline_result_template};

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
    let runtime_closure_template = runtime_delivery_closure_task_template(aac);
    let outline_result_template =
        taskplan_outline_result_template(request_id, delivery_id, phase_id);
    let group_result_template = taskplan_group_result_template(request_id, delivery_id, phase_id);
    json!({
        "schemaVersion": "1.0",
        "requestType": "taskplan_grouped_generation",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::TaskPlanCandidate,
        "sourceRefs": {
            "technicalBaselineRef": baseline_ref,
            "planningGenerationContractRef": planning_ref,
            "architectureArtifactContractRef": architecture_ref,
            "phaseConceptGroundingRef": pgc.context_refs.phase_concept_grounding_ref,
            "deliveryConceptGlossaryRef": pgc.context_refs.delivery_concept_glossary_ref
        },
        "contextProjection": {
            "phaseId": phase_id,
            "planningContractId": pgc.planning_contract_id,
            "architectureArtifactContractId": aac.architecture_artifact_contract_id,
            "technicalBaseline": {
                "technicalBaselineId": baseline.technical_baseline_id,
                "projectKind": baseline.project_kind,
                "stack": baseline.stack
            },
            "frontendExperienceProjection": aac.frontend_experience,
            "runtimeDeliveryProjection": aac.runtime_delivery,
            "requirementDetailTransfer": requirement_transfer
        },
        "allowedRefs": allowed_refs(pgc, aac),
        "generationRules": generation_rules(aac),
        "enumRefs": enum_refs(),
        "outputContract": {
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
            "groupResultTemplate": group_result_template,
            "runtimeDeliveryClosureTaskTemplate": runtime_closure_template
        },
        "blockedOutput": {
            "status": "blocked",
            "blockedReasonCode": "AAC_INSUFFICIENT",
            "nextNode": "architecture_artifact_repair"
        },
        "requestReadPlan": {
            "groups": taskplan_read_groups(aac, &runtime_closure_template)
        }
    })
}

fn taskplan_read_groups(
    aac: &ArchitectureArtifactContract,
    runtime_closure_template: &Value,
) -> Value {
    let mut groups = vec![
        json!({
            "groupId": "taskplan_core_context",
            "required": true,
            "purpose": "Read current phase source refs, requirement transfer, and allowed refs before writing the TaskPlan outline.",
            "whenToRead": "Read first.",
            "fields": [
                "sourceRefs",
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
                "allowedRefs"
            ]
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
            "fields": taskplan_candidate_contract_fields(runtime_closure_template)
        }),
    ];
    let optional_fields = taskplan_optional_projection_fields(aac);
    if !optional_fields.is_empty() {
        groups.push(json!({
            "groupId": "taskplan_optional_projection",
            "required": false,
            "purpose": "Read full frontend/runtime projections only when core projection is insufficient.",
            "whenToRead": "Read on demand.",
            "fields": optional_fields
        }));
    }
    Value::Array(groups)
}

fn taskplan_candidate_contract_fields(runtime_closure_template: &Value) -> Vec<&'static str> {
    let mut fields = vec![
        "enumRefs",
        "outputContract.outlineFile",
        "outputContract.groupFilePattern",
        "outputContract.pathAuthority",
        "outputContract.outlineResultTemplate",
        "outputContract.groupResultTemplate",
    ];
    if !runtime_closure_template.is_null() {
        fields.push("outputContract.runtimeDeliveryClosureTaskTemplate");
    }
    fields
}

fn taskplan_optional_projection_fields(aac: &ArchitectureArtifactContract) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if aac.frontend_experience.is_some() {
        fields.push("contextProjection.frontendExperienceProjection");
    }
    if aac.runtime_delivery.is_some() {
        fields.push("contextProjection.runtimeDeliveryProjection");
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
            "sourceRefs".to_string(),
            "contextProjection.planningContractId".to_string(),
            "contextProjection.architectureArtifactContractId".to_string(),
            "allowedRefs".to_string(),
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })?
    .fields;
    let root = Path::new(&input.project_root);
    let outline_ref = string_field(&fields, "outputContract.outlineFile")?;
    let group_pattern = string_field(&fields, "outputContract.groupFilePattern")?;
    let allowed_refs = fields
        .get("allowedRefs")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let outline: TaskPlanOutlineCandidateAgentWritable = read_project_json(root, &outline_ref)?;
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
        let candidate: TaskPlanGroupCandidateAgentWritable = read_project_json(root, &group_file)?;
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
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, outline_ref, issues, mode));
    }

    let source_refs = fields
        .get("sourceRefs")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let planning_ref = source_refs
        .get("planningGenerationContractRef")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "TaskPlan request missing sourceRefs.planningGenerationContractRef".to_string(),
            )
        })?;
    let architecture_ref = source_refs
        .get("architectureArtifactContractRef")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "TaskPlan request missing sourceRefs.architectureArtifactContractRef".to_string(),
            )
        })?;
    let baseline_ref = source_refs
        .get("technicalBaselineRef")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "TaskPlan request missing sourceRefs.technicalBaselineRef".to_string(),
            )
        })?;
    let baseline: contracts::TechnicalBaselineContract = read_project_json(root, baseline_ref)?;
    let pgc: contracts::PlanningGenerationContract = read_project_json(root, planning_ref)?;
    let aac: ArchitectureArtifactContract = read_project_json(root, architecture_ref)?;
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
            json!({
                "detailId": item.detail_id,
                "kind": item.kind,
                "title": item.title,
                "summary": item.summary,
                "priority": item.priority,
                "impactTags": item.impact_tags,
                "lifecycleStage": item.lifecycle_stage,
                "scopeRefs": item.scope_refs,
                "acceptanceRefs": item.acceptance_refs,
                "conceptRefs": item.concept_refs,
                "frontendRefs": item.frontend_refs,
                "coverage": detail_coverage.get(&item.detail_id).cloned().unwrap_or(Value::Null)
            })
        })
        .collect::<Vec<_>>();
    json!({
        "authority": "planning_generation_contract_plus_architecture_artifact_contract",
        "requirementDetailAssignment": {
            "items": requirement_items,
            "assignmentRule": "Every covered current-phase requirement detail must be assigned to task.requirementDetailRefs and verificationIntents[].requirementDetailRefs."
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
        "workflowClosureRequirements": [],
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
        "scopeRefs": pgc.phase_scope.included.iter().chain(pgc.phase_scope.deferred.iter()).chain(pgc.phase_scope.excluded.iter()).map(|item| item.id.clone()).collect::<Vec<_>>(),
        "acceptanceRefs": pgc.phase_scope.acceptance_candidates.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "requirementDetailIds": detail_ids,
        "moduleRefs": ids_from_values(&aac.modules, "moduleId"),
        "entityRefs": ids_from_value_array(&aac.data_model, "/entities", "entityId"),
        "interfaceRefs": ids_from_values(&aac.interfaces, "interfaceId"),
        "userFlowRefs": ids_from_values(&aac.user_flows, "flowId"),
        "stateMachineRefs": ids_from_values(&aac.state_machines, "machineId"),
        "decisionRefs": [],
        "riskRefs": []
    })
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
            "Do not implement deferred or excluded scope."
        ],
        "writeBoundaryRules": [
            "Every task.writeBoundary.forbiddenPaths must include .loom.",
            "Project source edits happen only during TaskExecution, not during TaskPlan generation."
        ],
        "verificationEvidenceRules": [
            "verificationIntents must use enumRefs.verificationEvidence.",
            "Each implementation task must have at least one verification intent."
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
            "rule": "When workflow closure requirements exist, assign each to an executable task that owns user action, interface invocation, state/readback, and feedback evidence."
        },
        "runtimeDeliveryRules": {
            "status": aac.runtime_delivery.as_ref().and_then(|value| value.get("status")).cloned().unwrap_or(Value::String("not_applicable".to_string())),
            "rule": "Runtime-affecting tasks must carry runtimeDeliveryRequirement; final runtime closure is required when runtimeDelivery.status=modified."
        }
    })
}

fn runtime_delivery_closure_task_template(aac: &ArchitectureArtifactContract) -> Value {
    let status = aac
        .runtime_delivery
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str);
    if status != Some("modified") {
        return Value::Null;
    }
    json!({
        "taskKind": "runtime_delivery_closure",
        "runtimeDeliveryRequirement": {
            "appliesToThisTask": true,
            "reason": "Final code-level closure for the RuntimeDeliveryContract.",
            "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
            "affectedContractFields": ["build.command", "start.command", "runtimeSurfaces", "deliveryMechanics"],
            "requiredCodeLevelChecks": []
        }
    })
}

fn enum_refs() -> Value {
    json!({
        "taskKind": [
            "feature_increment", "data_model_increment", "interface_increment", "ui_flow_increment",
            "frontend_experience", "runtime_delivery", "runtime_delivery_closure",
            "integration_increment", "verification_increment", "refactor_support", "configuration_support"
        ],
        "implementationAction": [
            "create_or_update_entity", "create_or_update_persistence", "create_or_update_interface",
            "create_or_update_ui_flow", "create_or_update_state_machine", "create_or_update_business_rule",
            "add_reference_field", "validate_reference_format", "use_fixture_or_mock_data",
            "wire_reference_in_api_or_ui", "create_entity_crud", "create_entity_repository",
            "create_entity_admin_page", "create_entity_migration", "implement_entity_lifecycle",
            "add_or_update_tests", "add_or_update_config", "implement_frontend_experience_contract",
            "implement_runtime_delivery_contract", "refactor_supporting_code"
        ],
        "verificationEvidence": [
            "automated_test", "manual_command_output", "runtime_api_check", "static_check", "agent_review_explanation"
        ]
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

fn read_project_json<T: serde::de::DeserializeOwned>(
    project_root: &Path,
    relative: &str,
) -> Result<T, state::store::StateError> {
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
