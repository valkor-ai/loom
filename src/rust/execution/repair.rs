use std::{collections::BTreeMap, path::Path};

use contracts::{
    ArchitectureSectionCandidateAgentWritable, ArchitectureSectionGroup, TaskDefinition, TaskPlan,
    TaskPlanGroupCandidateAgentWritable, TaskPlanOutlineCandidateAgentWritable, TaskPlanRun,
    TaskRunStatus,
};
use delivery_core::{
    ArtifactKind, DomainDispatcher, ExecuteEditBoundary, ExecuteTaskNext,
    ExecuteVerificationPolicy, ExecutionKind, FileSubmitInput, LoomMcpActionResult,
    LoomMcpAutoRunnableResult, LoomMcpFailure, LoomMcpFailureResult, LoomMcpNextAction,
    LoomMcpRepairableErrorResult, OperationContext, PostSubmitAction, ReadFieldGroupInput,
    RepairContext, RepairOrigin, RouteAction, RouteActionKind, SubmitAcceptedEvent,
    TransitionEngine, TransitionStore, WriteArtifactNext, WriteMode, WriteTarget,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
};

use crate::{
    paths::{
        task_execution_request_file, task_execution_result_candidate_file, task_plan_group_pattern,
        task_plan_outline_candidate_file,
    },
    task_execution::{load_current_plan_and_run, save_run},
    task_plan::update_run_summary,
};

const ARCHITECTURE_SECTION_ORDER: [ArchitectureSectionGroup; 6] = [
    ArchitectureSectionGroup::Foundation,
    ArchitectureSectionGroup::DomainContract,
    ArchitectureSectionGroup::Behavior,
    ArchitectureSectionGroup::FrontendExperience,
    ArchitectureSectionGroup::RuntimeDelivery,
    ArchitectureSectionGroup::Coverage,
];

pub fn dispatch_repair_route(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    action: &RouteAction,
) -> LoomMcpActionResult {
    match action.kind {
        RouteActionKind::TaskplanRepair => materialize_taskplan_repair(
            project_root,
            delivery_id,
            phase_id,
            action.request_ref.clone(),
        )
        .unwrap_or_else(|error| {
            failed(
                project_root,
                "TASKPLAN_REPAIR_FAILED",
                error.to_string(),
                "taskplan_repair",
            )
        }),
        RouteActionKind::ArchitectureArtifactRepair => materialize_architecture_repair(
            project_root,
            delivery_id,
            phase_id,
            action.request_ref.clone(),
        )
        .unwrap_or_else(|error| {
            failed(
                project_root,
                "ARCHITECTURE_REPAIR_FAILED",
                error.to_string(),
                "architecture_artifact_repair",
            )
        }),
        RouteActionKind::TaskResultRepair => failed(
            project_root,
            "TASK_RESULT_REPAIR_REQUIRES_TASK_EXECUTION_REQUEST",
            "TaskResult repair must be created from the original TaskExecutionRequest after TaskResult validation fails.".to_string(),
            "task_result_repair",
        ),
        _ => failed(
            project_root,
            "REPAIR_ROUTE_UNSUPPORTED",
            format!("Unsupported repair route {:?}", action.kind),
            "repair",
        ),
    }
}

pub fn accept_repair_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    if authorized.artifact_kind == ArtifactKind::TaskResultRepair {
        return crate::task_result::accept_task_result_repair_file(input, authorized);
    }
    if authorized.artifact_kind == ArtifactKind::TaskplanRepair {
        return crate::task_plan::accept_task_plan_repair_file(input, authorized);
    }
    match accept_repair_file_inner(input, authorized, dispatcher) {
        Ok(result) => result,
        Err(error) => failed(
            &input.project_root,
            "REPAIR_SUBMIT_FAILED",
            error.to_string(),
            "repair_submit",
        ),
    }
}

fn accept_repair_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    let target = authorized.targets.first().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Repair target is missing".to_string())
    })?;
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Repair request missing deliveryId".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Repair request missing phaseId".to_string())
    })?;
    if let Some(stale) = ensure_latest_repair_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
    )? {
        return Ok(stale);
    }
    let root = Path::new(&input.project_root);
    let repair_value = state::store::read_json_value(&from_project_relative(root, &target.path)?)?;
    let issues = validate_repair_artifact(&repair_value, authorized.artifact_kind);
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, target.path.clone(), issues));
    }
    let repair_id = repair_value
        .get("repairId")
        .and_then(Value::as_str)
        .unwrap_or("repair")
        .to_string();
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let persisted = state::paths::delivery_dir(root, &delivery_id)
        .join("repairs")
        .join(&phase_id)
        .join("results")
        .join(format!("{}.json", safe_id(&repair_id)));
    state::store::write_json_atomic(&persisted, &repair_value)?;
    let repair_ref = to_project_relative(root, &persisted)?;
    let next_action =
        repair_submit_next_action(authorized.artifact_kind, repair_ref.clone(), &repair_value);
    update_delivery_after_repair_submit(
        &input.project_root,
        &locator,
        &repair_ref,
        &next_action,
        authorized.artifact_kind,
    )?;
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
                source_tool: "loom.repairSubmitFile".to_string(),
                accepted_artifact_ref: repair_ref,
                next_action: Some(next_action),
            },
        )
        .map_err(to_state_error)
}

pub fn materialize_delivery_execution_repair(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    origin: &str,
    source_ref: Option<String>,
    finding_refs: Vec<String>,
) -> LoomMcpActionResult {
    match materialize_delivery_execution_repair_inner(
        project_root,
        delivery_id,
        phase_id,
        origin,
        source_ref,
        finding_refs,
    ) {
        Ok(result) => result,
        Err(error) => failed(
            project_root,
            "EXECUTION_REPAIR_REQUEST_FAILED",
            error.to_string(),
            "execution_repair",
        ),
    }
}

fn materialize_delivery_execution_repair_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    origin: &str,
    source_ref: Option<String>,
    finding_refs: Vec<String>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let (task_plan, mut run) = load_current_plan_and_run(root, &locator)?;
    let task = repair_source_task(&task_plan, &run).ok_or_else(|| {
        state::store::StateError::StateCorrupted(
            "No task is available for delivery execution repair.".to_string(),
        )
    })?;
    let attempt_count = run
        .task_states
        .iter()
        .find(|state| state.task_id == task.task_id)
        .map(|state| state.attempts.len() as u32)
        .unwrap_or(0);
    let now = state::store::now_string();
    if let Some(state) = run
        .task_states
        .iter_mut()
        .find(|state| state.task_id == task.task_id)
    {
        state.status = TaskRunStatus::Running;
        state.started_at.get_or_insert(now.clone());
    }
    run.status = contracts::TaskPlanRunStatus::Running;
    run.updated_at = now;
    update_run_summary(&mut run);
    save_run(root, &locator, &run)?;

    let request_id = format!(
        "repair_exec_{}_{}",
        safe_id(&task.task_id),
        state::store::now_millis()
    );
    let project = state::initialize_project(project_root)?;
    let repair_request_ref = format!(
        "loom://projects/{}/requests/{request_id}",
        project.project_id
    );
    let result_file = to_project_relative(
        root,
        &task_execution_result_candidate_file(root, &request_id),
    )?;
    let request_file = to_project_relative(
        root,
        &task_execution_request_file(root, &locator, &request_id),
    )?;
    let repair_origin = repair_origin(origin);
    let request_root = build_repair_execution_request(
        &request_id,
        delivery_id,
        phase_id,
        &result_file,
        &task_plan,
        &run,
        &task,
        repair_origin.clone(),
        &repair_request_ref,
        source_ref.clone(),
        finding_refs.clone(),
        attempt_count,
    );
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "delivery_execution_repair_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    if let Some(parent) = from_project_relative(root, &result_file)?.parent() {
        state::store::ensure_dir(parent)?;
    }
    update_latest_execution_request(
        project_root,
        delivery_id,
        phase_id,
        &stored.request_ref,
        &result_file,
        origin,
        source_ref.clone(),
        finding_refs.clone(),
    )?;
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: stored.request_ref.clone(),
    })?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::ExecuteTask(ExecuteTaskNext {
                execution_kind: ExecutionKind::DeliveryExecutionRepair,
                repair_origin: Some(repair_origin.clone()),
                request_ref: stored.request_ref,
                result_file,
                task_id: task.task_id.clone(),
                group_id: Some(task.group_id.clone()),
                read_groups: inspected.read_groups,
                submit_tool: "loom.recordTaskResultFile".to_string(),
                edit_boundary: ExecuteEditBoundary {
                    allowed_paths: vec![".".to_string()],
                    protected_paths: vec![
                        ".loom".to_string(),
                        "Brainstorm".to_string(),
                        "TechnicalBaseline".to_string(),
                        "PGC".to_string(),
                        "AAC".to_string(),
                        "TaskPlan".to_string(),
                        "ReviewResult".to_string(),
                    ],
                },
                verification_policy: ExecuteVerificationPolicy {
                    required_commands: vec![],
                    evidence_required: true,
                },
                repair_context: Some(RepairContext {
                    repair_origin: repair_origin.clone(),
                    repair_request_ref,
                    source_task_id: task.task_id.clone(),
                    issues: vec![origin.to_string()],
                    review_result_ref: if origin == "review_result" {
                        source_ref.clone()
                    } else {
                        None
                    },
                    finding_refs,
                    manual_review_resolution_ref: if origin == "manual_review_resolution" {
                        source_ref.clone()
                    } else {
                        None
                    },
                    user_change_summary: user_change_summary(root, origin, source_ref.as_deref()),
                    failed_task_result_ref: if origin == "task_failure" {
                        source_ref.clone()
                    } else {
                        None
                    },
                    attempt_count: if origin == "task_failure" {
                        Some(attempt_count)
                    } else {
                        None
                    },
                    deployment_failure_ref: None,
                    failed_contract_fields: vec![],
                    required_code_level_checks: vec![],
                }),
                post_submit: PostSubmitAction::ContinueDelivery,
            }),
        ),
    ))
}

fn build_repair_execution_request(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    result_file: &str,
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    task: &TaskDefinition,
    repair_origin: RepairOrigin,
    repair_request_ref: &str,
    source_ref: Option<String>,
    finding_refs: Vec<String>,
    attempt_count: u32,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(contracts::TaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    json!({
        "schemaVersion": "1.0",
        "requestType": "delivery_execution_repair",
        "requestId": request_id,
        "artifactKind": ArtifactKind::TaskResult,
        "executionKind": "delivery_execution_repair",
        "source": {
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "taskPlanId": task_plan.task_plan_id,
            "taskPlanRunId": run.run_id,
            "taskId": task.task_id,
            "groupId": task.group_id
        },
        "task": task,
        "repairContext": {
            "repairRequestRef": repair_request_ref,
            "sourceTaskId": task.task_id,
            "repairOrigin": repair_origin,
            "attemptCount": attempt_count,
            "sourceRef": source_ref,
            "findingRefs": finding_refs
        },
        "executionRules": {
            "sourceEditPreparationContract": {
                "rule": "Repair only the execution issue for the current task. Do not modify Loom contracts or protected artifacts."
            },
            "completionBarrier": {
                "resultFile": result_file,
                "submitTool": "loom.recordTaskResultFile",
                "rule": "The repair is not complete until TaskResult exists at outputContract.resultFile and loom.recordTaskResultFile succeeds."
            },
            "finalResponseGuard": {
                "mustNotReportProgressBeforeSubmit": true
            },
            "completionContinuityRequirement": {
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none | started_and_released | unknown"
            },
            "verificationCommandSchedulingRules": [
                "Run the verification needed for the repaired task before TaskResult submission."
            ],
            "rules": [
                "Do not edit Brainstorm, TechnicalBaseline, PGC, AAC, TaskPlan, ReviewResult, ManualReviewResolution, or .loom state.",
                "Do not expand scope.",
                "Write TaskResult JSON only to outputContract.resultFile."
            ]
        },
        "enumRefs": {
            "taskResultStatus": ["completed", "completed_with_notes", "blocked", "failed"],
            "verificationStatus": ["passed", "not_run", "failed", "inconclusive"]
        },
        "outputContract": {
            "artifactKind": ArtifactKind::TaskResult,
            "writeMode": "single_json",
            "submitTool": "loom.recordTaskResultFile",
            "resultFile": result_file,
            "writeTargets": [{
                "targetId": "result",
                "path": result_file,
                "required": true,
                "description": "Write the TaskResult JSON for this delivery execution repair."
            }],
            "requiredTopLevelFields": [
                "schemaVersion", "taskResultId", "taskId", "taskPlanId", "status",
                "changedFiles", "noChangeReason", "verificationResults", "selfRepairSummary",
                "failure", "executionContinuity", "notes", "frontendExperienceSelfCheck",
                "runtimeDeliveryEvidence", "requirementDetailEvidence", "conceptEvidence",
                "blockedReasons", "createdAt", "updatedAt"
            ],
            "schemaShape": schema_shape,
            "resultRules": [
                "TaskResult must include every requiredTopLevelFields entry.",
                "If status is completed, evidence must show the repair was verified."
            ]
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "repair_execution_core",
                    "required": true,
                    "purpose": "Read task, repair context, and execution rules before editing.",
                    "whenToRead": "Read before source edits.",
                    "fields": ["source", "task", "repairContext", "executionRules"]
                },
                {
                    "groupId": "repair_result_contract",
                    "required": true,
                    "purpose": "Read TaskResult write contract before submitting repair result.",
                    "whenToRead": "Read before writing TaskResult.",
                    "fields": [
                        "enumRefs",
                        "outputContract.resultFile",
                        "outputContract.requiredTopLevelFields",
                        "outputContract.schemaShape.properties.status",
                        "outputContract.schemaShape.properties.changedFiles",
                        "outputContract.schemaShape.properties.noChangeReason",
                        "outputContract.schemaShape.properties.verificationResults",
                        "outputContract.schemaShape.properties.selfRepairSummary",
                        "outputContract.schemaShape.properties.failure",
                        "outputContract.schemaShape.properties.executionContinuity",
                        "outputContract.schemaShape.properties.notes",
                        "outputContract.schemaShape.properties.frontendExperienceSelfCheck",
                        "outputContract.schemaShape.properties.runtimeDeliveryEvidence",
                        "outputContract.schemaShape.properties.requirementDetailEvidence",
                        "outputContract.schemaShape.properties.conceptEvidence",
                        "outputContract.schemaShape.properties.blockedReasons",
                        "outputContract.resultRules",
                        "executionRules.completionBarrier"
                    ]
                }
            ]
        }
    })
}

pub fn materialize_taskplan_repair(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    source_ref: Option<String>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    materialize_taskplan_repair_request(project_root, delivery_id, phase_id, source_ref)
}

pub fn materialize_architecture_repair(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    source_ref: Option<String>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    materialize_architecture_repair_request(project_root, delivery_id, phase_id, source_ref)
}

fn materialize_taskplan_repair_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    source_ref: Option<String>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let original_request_ref =
        latest_phase_ref(project_root, delivery_id, phase_id, "taskPlanRequestRef")?;
    let request_id = format!("taskplan_repair_{}", state::store::now_millis());
    let outline_file =
        to_project_relative(root, &task_plan_outline_candidate_file(root, &request_id))?;
    let group_file_pattern =
        to_project_relative(root, &task_plan_group_pattern(root, &request_id))?;
    let request_file = to_project_relative(
        root,
        &state::paths::delivery_dir(root, delivery_id)
            .join("repairs")
            .join(phase_id)
            .join("requests")
            .join(format!("{request_id}.json")),
    )?;

    let core_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "taskplan_core_context".to_string(),
    })?
    .fields;
    let rule_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "taskplan_generation_rules".to_string(),
    })?
    .fields;
    let contract_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "taskplan_candidate_contract".to_string(),
    })?
    .fields;
    let optional_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "taskplan_optional_projection".to_string(),
    })
    .map(|result| result.fields)
    .unwrap_or_default();

    let source_refs = field_value(&core_fields, "sourceRefs")?;
    let allowed_refs = field_value(&core_fields, "allowedRefs")?;
    let context_projection = json!({
        "phaseId": field_value(&core_fields, "contextProjection.phaseId")?,
        "planningContractId": field_value(&core_fields, "contextProjection.planningContractId")?,
        "architectureArtifactContractId": field_value(&core_fields, "contextProjection.architectureArtifactContractId")?,
        "requirementDetailTransfer": {
            "requirementDetailAssignment": field_value(&core_fields, "contextProjection.requirementDetailTransfer.requirementDetailAssignment")?,
            "currentPhaseScope": field_value(&core_fields, "contextProjection.requirementDetailTransfer.currentPhaseScope")?,
            "acceptanceDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.acceptanceDetails")?,
            "businessFlowDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.businessFlowDetails")?,
            "objectOperationDetailRules": field_value(&core_fields, "contextProjection.requirementDetailTransfer.objectOperationDetailRules")?,
            "architectureDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.architectureDetails")?,
            "workflowClosureRequirements": field_value(&core_fields, "contextProjection.requirementDetailTransfer.workflowClosureRequirements")?,
            "conceptRefs": field_value(&core_fields, "contextProjection.requirementDetailTransfer.conceptRefs")?,
            "taskPlanningFieldMapping": field_value(&core_fields, "contextProjection.requirementDetailTransfer.taskPlanningFieldMapping")?
        }
    });
    let generation_rules = json!({
        "groupedOutputRules": field_value(&rule_fields, "generationRules.groupedOutputRules")?,
        "scopeAndReferenceRules": field_value(&rule_fields, "generationRules.scopeAndReferenceRules")?,
        "writeBoundaryRules": field_value(&rule_fields, "generationRules.writeBoundaryRules")?,
        "verificationEvidenceRules": field_value(&rule_fields, "generationRules.verificationEvidenceRules")?,
        "conceptGroundingRules": field_value(&rule_fields, "generationRules.conceptGroundingRules")?,
        "frontendExperienceRules": field_value(&rule_fields, "generationRules.frontendExperienceRules")?,
        "workflowClosureRules": field_value(&rule_fields, "generationRules.workflowClosureRules")?,
        "runtimeDeliveryRules": field_value(&rule_fields, "generationRules.runtimeDeliveryRules")?
    });
    let enum_refs = field_value(&contract_fields, "enumRefs")?;
    let frontend_projection = optional_fields
        .get("outputContract.frontendExperienceProjection")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let runtime_projection = optional_fields
        .get("outputContract.runtimeDeliveryProjection")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let schema_shape = serde_json::to_value(schema_for!(TaskPlanOutlineCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let group_schema = serde_json::to_value(schema_for!(TaskPlanGroupCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));

    let request_root = json!({
        "schemaVersion": "1.0",
        "requestType": "taskplan_repair",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::TaskplanRepair,
        "sourceRefs": source_refs,
        "repairContext": {
            "sourceTaskPlanRequestRef": original_request_ref,
            "sourceRef": source_ref
        },
        "contextProjection": context_projection,
        "allowedRefs": allowed_refs,
        "generationRules": generation_rules,
        "enumRefs": enum_refs,
        "outputContract": {
            "artifactKind": ArtifactKind::TaskplanRepair,
            "writeMode": WriteMode::TaskplanGrouped,
            "submitTool": "loom.repairSubmitFile",
            "outlineFile": outline_file,
            "groupFilePattern": group_file_pattern,
            "writeTargets": [
                {
                    "targetId": "outline",
                    "path": outline_file,
                    "required": true,
                    "description": "Write the replacement TaskPlan outline JSON."
                },
                {
                    "targetId": "groups",
                    "path": group_file_pattern,
                    "required": false,
                    "description": "Write one replacement TaskPlan group JSON for each outline.groups[].groupId."
                }
            ],
            "pathAuthority": {
                "currentRequestOnly": true,
                "currentRequestId": request_id,
                "rule": "Only this replacement contract's outlineFile and groupFilePattern are authorized TaskPlan targets."
            },
            "outlineSchemaShape": schema_shape,
            "groupSchemaShape": group_schema,
            "outlineResultTemplate": {
                "schemaVersion": "1.0",
                "requestId": request_id,
                "deliveryId": delivery_id,
                "phaseId": phase_id,
                "status": "ready",
                "taskPlanId": format!("taskplan-{phase_id}-repair"),
                "groups": []
            },
            "groupResultTemplate": {
                "schemaVersion": "1.0",
                "requestId": request_id,
                "deliveryId": delivery_id,
                "phaseId": phase_id,
                "status": "ready",
                "group": "TaskPlanGroup matching one outline group",
                "tasks": []
            },
            "frontendExperienceProjection": frontend_projection,
            "runtimeDeliveryProjection": runtime_projection
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "taskplan_core_context",
                    "required": true,
                    "purpose": "Read current phase source refs, requirement transfer, and allowed refs before writing the replacement TaskPlan outline.",
                    "whenToRead": "Read first.",
                    "fields": [
                        "sourceRefs",
                        "repairContext",
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
                },
                {
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
                },
                {
                    "groupId": "taskplan_repair_write_contract",
                    "required": true,
                    "purpose": "Read output paths, schema shapes, and enum refs before writing replacement candidates.",
                    "whenToRead": "Read before writing output files.",
                    "fields": [
                        "enumRefs",
                        "outputContract.outlineFile",
                        "outputContract.groupFilePattern",
                        "outputContract.pathAuthority",
                        "outputContract.outlineResultTemplate",
                        "outputContract.groupResultTemplate"
                    ]
                },
                {
                    "groupId": "taskplan_optional_projection",
                    "required": false,
                    "purpose": "Read full frontend/runtime projections only when core projection is insufficient.",
                    "whenToRead": "Read on demand.",
                    "fields": [
                        "outputContract.frontendExperienceProjection",
                        "outputContract.runtimeDeliveryProjection"
                    ]
                }
            ]
        }
    });
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "taskplan_repair_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    if let Some(parent) = from_project_relative(root, &outline_file)?.parent() {
        state::store::ensure_dir(parent)?;
    }
    if let Some(parent) = from_project_relative(root, &group_file_pattern)?.parent() {
        state::store::ensure_dir(parent)?;
    }
    update_latest_repair_request(
        project_root,
        &locator,
        &stored.request_ref,
        "taskplan_repair",
    )?;
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: stored.request_ref.clone(),
    })?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
                artifact_kind: ArtifactKind::TaskplanRepair,
                request_ref: stored.request_ref,
                write_mode: WriteMode::TaskplanGrouped,
                write_targets: inspected
                    .write_targets
                    .iter()
                    .map(value_to_write_target)
                    .collect::<Result<Vec<_>, _>>()?,
                read_groups: inspected.read_groups,
                submit_tool: "loom.repairSubmitFile".to_string(),
            }),
        ),
    ))
}

fn materialize_architecture_repair_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    source_ref: Option<String>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let original_request_ref = latest_phase_ref(
        project_root,
        delivery_id,
        phase_id,
        "architectureRequestRef",
    )?;
    let request_id = format!("architecture_repair_{}", state::store::now_millis());
    let request_file = to_project_relative(
        root,
        &state::paths::delivery_dir(root, delivery_id)
            .join("repairs")
            .join(phase_id)
            .join("requests")
            .join(format!("{request_id}.json")),
    )?;
    let core_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "architecture_core_context".to_string(),
    })?
    .fields;
    let frontend_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "architecture_frontend_context".to_string(),
    })
    .map(|result| result.fields)
    .unwrap_or_default();
    let runtime_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: original_request_ref.clone(),
        group_id: "architecture_runtime_context".to_string(),
    })
    .map(|result| result.fields)
    .unwrap_or_default();
    let source_refs = field_value(&core_fields, "sourceRefs")?;
    let allowed_refs = field_value(&core_fields, "allowedRefs")?;
    let context_projection = json!({
        "phaseScope": field_value(&core_fields, "contextProjection.phaseScope")?,
        "phaseId": field_value(&core_fields, "contextProjection.phaseId")?,
        "planningContractId": field_value(&core_fields, "contextProjection.planningContractId")?,
        "technicalBaseline": field_value(&core_fields, "contextProjection.technicalBaseline")?,
        "requirementDetailTransfer": {
            "requirementDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.requirementDetails")?,
            "acceptanceDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.acceptanceDetails")?,
            "businessFlows": field_value(&core_fields, "contextProjection.requirementDetailTransfer.businessFlows")?,
            "frontendExperienceDetails": frontend_fields
                .get("contextProjection.requirementDetailTransfer.frontendExperienceDetails")
                .map(|field| field.value.clone())
                .unwrap_or(Value::Null),
            "userFacingLanguage": frontend_fields
                .get("contextProjection.requirementDetailTransfer.userFacingLanguage")
                .map(|field| field.value.clone())
                .unwrap_or(Value::Null)
        }
    });
    let frontend_experience_source = frontend_fields
        .get("frontendExperienceSource")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let runtime_authority = runtime_fields
        .get("rules.runtimeDeliveryAuthority")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let section_outputs = build_architecture_repair_section_outputs(root, &request_id)?;
    let candidate_files = section_outputs
        .iter()
        .filter_map(|output| {
            output
                .get("candidateFile")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let current_output = section_outputs.first().cloned().ok_or_else(|| {
        state::store::StateError::StateCorrupted(
            "architecture repair section outputs are empty".to_string(),
        )
    })?;
    let candidate_schema =
        serde_json::to_value(schema_for!(ArchitectureSectionCandidateAgentWritable))
            .unwrap_or_else(|_| json!({ "type": "object" }));
    let request_root = json!({
        "schemaVersion": "1.0",
        "requestType": "architecture_artifact_repair",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::ArchitectureArtifactRepair,
        "sourceRefs": source_refs,
        "repairContext": {
            "sourceArchitectureRequestRef": original_request_ref,
            "sourceRef": source_ref
        },
        "contextProjection": context_projection,
        "frontendExperienceSource": frontend_experience_source,
        "allowedRefs": allowed_refs,
        "sectionState": {
            "order": ARCHITECTURE_SECTION_ORDER,
            "currentSection": current_output["section"].clone(),
            "completedSections": []
        },
        "sectionOutputs": section_outputs,
        "currentSectionContract": current_output,
        "enumRefs": {
            "section": ARCHITECTURE_SECTION_ORDER,
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
            "runtimeDeliveryAuthority": runtime_authority
        },
        "outputContract": {
            "artifactKind": ArtifactKind::ArchitectureArtifactRepair,
            "writeMode": WriteMode::ArchitectureSection,
            "submitTool": "loom.repairSubmitFile",
            "writeTargets": [{
                "targetId": section_name(ArchitectureSectionGroup::Foundation),
                "path": current_output["candidateFile"].clone(),
                "required": true,
                "description": "Write the replacement foundation Architecture section candidate JSON."
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
                "requiredContentKeys": required_architecture_content_keys(ArchitectureSectionGroup::Foundation)
            }
        },
        "postSubmit": {
            "nextAction": RouteAction {
                kind: RouteActionKind::ArchitectureArtifactContract,
                source: "architecture_artifact_repair".to_string(),
                reason: "architecture_repair_section_ready".to_string(),
                prompt: None,
                accepted_responses: vec![],
                request_ref: None,
                details: None,
                target_phase_id: None
            }
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "architecture_core_context",
                    "required": true,
                    "purpose": "Read the current-phase planning authority and allowed refs before generating the replacement Architecture section.",
                    "whenToRead": "Read before drafting any replacement Architecture section candidate.",
                    "fields": [
                        "sourceRefs",
                        "repairContext",
                        "contextProjection.phaseScope",
                        "contextProjection.phaseId",
                        "contextProjection.planningContractId",
                        "contextProjection.technicalBaseline",
                        "contextProjection.requirementDetailTransfer.requirementDetails",
                        "contextProjection.requirementDetailTransfer.acceptanceDetails",
                        "contextProjection.requirementDetailTransfer.businessFlows",
                        "allowedRefs"
                    ]
                },
                {
                    "groupId": "architecture_section_contract",
                    "required": true,
                    "purpose": "Read the current section contract, schema projection, and write target before writing the replacement section candidate.",
                    "whenToRead": "Read immediately before writing the current replacement Architecture section candidate.",
                    "fields": [
                        "sectionState.currentSection",
                        "currentSectionContract",
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
                        "outputContract.schemaProjection",
                        "enumRefs.section",
                        "enumRefs.status",
                        "enumRefs.coverageStatus",
                        "enumRefs.acceptancePriority"
                    ]
                },
                {
                    "groupId": "architecture_frontend_context",
                    "required": false,
                    "purpose": "Read the frontend authority refs only when generating frontend_experience.",
                    "whenToRead": "Read when sectionState.currentSection is frontend_experience or when frontend authority affects another section.",
                    "fields": [
                        "frontendExperienceSource",
                        "contextProjection.requirementDetailTransfer.frontendExperienceDetails",
                        "contextProjection.requirementDetailTransfer.userFacingLanguage"
                    ]
                },
                {
                    "groupId": "architecture_runtime_context",
                    "required": false,
                    "purpose": "Read runtime delivery authority fields before generating runtime_delivery.",
                    "whenToRead": "Read when sectionState.currentSection is runtime_delivery.",
                    "fields": [
                        "rules.runtimeDeliveryAuthority"
                    ]
                }
            ]
        }
    });
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "architecture_artifact_repair_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    for candidate_file in candidate_files {
        if let Some(parent) = from_project_relative(root, &candidate_file)?.parent() {
            state::store::ensure_dir(parent)?;
        }
    }
    update_latest_repair_request(
        project_root,
        &locator,
        &stored.request_ref,
        "architecture_artifact_repair",
    )?;
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: stored.request_ref.clone(),
    })?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
                artifact_kind: ArtifactKind::ArchitectureArtifactRepair,
                request_ref: stored.request_ref,
                write_mode: WriteMode::ArchitectureSection,
                write_targets: inspected
                    .write_targets
                    .iter()
                    .map(value_to_write_target)
                    .collect::<Result<Vec<_>, _>>()?,
                read_groups: inspected.read_groups,
                submit_tool: "loom.repairSubmitFile".to_string(),
            }),
        ),
    ))
}

fn latest_phase_ref(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    key: &str,
) -> Result<String, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .and_then(|phase| phase.latest_refs.get(key))
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "phase {phase_id} is missing latestRefs.{key}"
            ))
        })
}

fn field_value(
    fields: &BTreeMap<String, delivery_core::FieldReadResult>,
    field: &str,
) -> Result<Value, state::store::StateError> {
    fields
        .get(field)
        .map(|result| result.value.clone())
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "replacement source field {field} is missing"
            ))
        })
}

fn build_architecture_repair_section_outputs(
    project_root: &Path,
    request_id: &str,
) -> Result<Vec<Value>, state::store::StateError> {
    let schema_shape = serde_json::to_value(schema_for!(ArchitectureSectionCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    Ok(ARCHITECTURE_SECTION_ORDER
        .iter()
        .map(|section| {
            let candidate_file = project_root
                .join(".loom")
                .join("agent-writable")
                .join(request_id)
                .join(format!("architecture-{}.json", section_name(*section)));
            Ok(json!({
                "section": section,
                "candidateFile": to_project_relative(project_root, &candidate_file)?,
                "schemaRef": format!("rust-contract://ArchitectureSectionCandidateAgentWritable/{}", section_name(*section)),
                "schemaShape": schema_shape.clone(),
                "enumRefs": {
                    "section": ARCHITECTURE_SECTION_ORDER,
                    "status": ["ready", "blocked"],
                    "coverageStatus": ["covered", "partial", "not_applicable", "deferred", "uncovered"],
                    "acceptancePriority": ["must", "should", "could"]
                },
                "generationRules": [
                    format!("Write only the {} section candidate for this request.", section_name(*section)),
                    "Do not write the final AAC JSON; Rust assembles it after coverage submit."
                ]
            }))
        })
        .collect::<Result<Vec<_>, state::store::StateError>>()?)
}

fn section_name(section: ArchitectureSectionGroup) -> &'static str {
    match section {
        ArchitectureSectionGroup::Foundation => "foundation",
        ArchitectureSectionGroup::DomainContract => "domain_contract",
        ArchitectureSectionGroup::Behavior => "behavior",
        ArchitectureSectionGroup::FrontendExperience => "frontend_experience",
        ArchitectureSectionGroup::RuntimeDelivery => "runtime_delivery",
        ArchitectureSectionGroup::Coverage => "coverage",
    }
}

fn required_architecture_content_keys(section: ArchitectureSectionGroup) -> Vec<&'static str> {
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

fn repair_source_task(task_plan: &TaskPlan, run: &TaskPlanRun) -> Option<TaskDefinition> {
    let preferred = run
        .task_states
        .iter()
        .find(|state| matches!(state.status, TaskRunStatus::Failed | TaskRunStatus::Blocked))
        .or_else(|| {
            run.task_states
                .iter()
                .find(|state| state.result_id.is_some())
        })
        .or_else(|| run.task_states.first())?;
    task_plan
        .tasks
        .iter()
        .find(|task| task.task_id == preferred.task_id)
        .cloned()
}

fn update_latest_execution_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    result_file: &str,
    origin: &str,
    source_ref: Option<String>,
    finding_refs: Vec<String>,
) -> Result<(), state::store::StateError> {
    let store = FileTransitionStore;
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        phase.latest_refs.insert(
            "taskExecutionRequestRef".to_string(),
            request_ref.to_string(),
        );
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::ExecutionRepair,
            source: "delivery_execution_repair".to_string(),
            reason: format!("{origin}_repair"),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(request_ref.to_string()),
            details: Some(json!({
                "resultFile": result_file,
                "origin": origin,
                "sourceRef": source_ref,
                "findingRefs": finding_refs
            })),
            target_phase_id: None,
        });
    }
    delivery.status = delivery_core::DeliveryLifecycleStatus::Executing;
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)
}

fn update_latest_repair_request(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    request_ref: &str,
    repair_type: &str,
) -> Result<(), state::store::StateError> {
    let store = FileTransitionStore;
    let mut delivery = store
        .load_delivery_index(project_root, &locator.delivery_id)
        .map_err(to_state_error)?;
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == locator.phase_id)
    {
        phase
            .latest_refs
            .insert("repairRequestRef".to_string(), request_ref.to_string());
        phase.next_action = Some(RouteAction {
            kind: match repair_type {
                "taskplan_repair" => RouteActionKind::TaskplanRepair,
                "architecture_artifact_repair" => RouteActionKind::ArchitectureArtifactRepair,
                "task_result_repair" => RouteActionKind::TaskResultRepair,
                _ => RouteActionKind::ExecutionRepair,
            },
            source: "repair_request".to_string(),
            reason: repair_type.to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(request_ref.to_string()),
            details: None,
            target_phase_id: None,
        });
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)
}

fn ensure_latest_repair_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let latest = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .and_then(|phase| phase.latest_refs.get("repairRequestRef"))
        .map(String::as_str);
    if latest != Some(request_ref) {
        return Ok(Some(failed(
            project_root,
            "STALE_REPAIR_REQUEST",
            "Repair submit must use the active phase latest repairRequestRef.".to_string(),
            "repair_submit",
        )));
    }
    Ok(None)
}

fn validate_repair_artifact(
    value: &Value,
    artifact_kind: ArtifactKind,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if value.get("schemaVersion").and_then(Value::as_str) != Some("1.0") {
        issues.push(issue(
            "REPAIR_ARTIFACT_SCHEMA_INVALID",
            "schemaVersion",
            "Repair artifact schemaVersion must be 1.0.",
        ));
    }
    if value
        .get("repairId")
        .and_then(Value::as_str)
        .map(str::is_empty)
        .unwrap_or(true)
    {
        issues.push(issue(
            "REPAIR_ARTIFACT_SCHEMA_INVALID",
            "repairId",
            "Repair artifact repairId is required.",
        ));
    }
    if value
        .get("summary")
        .and_then(Value::as_str)
        .map(str::is_empty)
        .unwrap_or(true)
    {
        issues.push(issue(
            "REPAIR_ARTIFACT_SCHEMA_INVALID",
            "summary",
            "Repair artifact summary is required.",
        ));
    }
    let status = value.get("status").and_then(Value::as_str);
    if !matches!(status, Some("ready") | Some("blocked")) {
        issues.push(issue(
            "REPAIR_ARTIFACT_ENUM_INVALID",
            "status",
            "Repair artifact status must be ready or blocked.",
        ));
    }
    if status == Some("blocked") {
        issues.push(issue(
            "REPAIR_ARTIFACT_BLOCKED",
            "status",
            "Repair artifact is blocked and cannot be submitted as a completed repair route.",
        ));
    }
    let expected = expected_repair_next_action(artifact_kind);
    let actual = value
        .pointer("/nextAction/type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if actual != expected {
        issues.push(issue(
            "REPAIR_ARTIFACT_ROUTE_INVALID",
            "nextAction.type",
            &format!("Repair artifact nextAction.type must be {expected}."),
        ));
    }
    issues
}

fn expected_repair_next_action(artifact_kind: ArtifactKind) -> &'static str {
    match artifact_kind {
        ArtifactKind::TaskResultRepair => "execution_repair",
        ArtifactKind::TaskplanRepair => "taskplan_generation",
        ArtifactKind::ArchitectureArtifactRepair => "architecture_artifact_contract",
        ArtifactKind::DeployExecutionRepairResult => "deploy_execution_repair",
        _ => "unknown",
    }
}

fn repair_submit_next_action(
    artifact_kind: ArtifactKind,
    repair_ref: String,
    repair_value: &Value,
) -> RouteAction {
    let reason = repair_value
        .pointer("/nextAction/reason")
        .and_then(Value::as_str)
        .or_else(|| repair_value.get("summary").and_then(Value::as_str))
        .unwrap_or("repair_submit_accepted")
        .to_string();
    let kind = match artifact_kind {
        ArtifactKind::TaskResultRepair => RouteActionKind::ExecutionRepair,
        ArtifactKind::TaskplanRepair => RouteActionKind::TaskplanGeneration,
        ArtifactKind::ArchitectureArtifactRepair => RouteActionKind::ArchitectureArtifactContract,
        _ => RouteActionKind::ExecutionRepair,
    };
    RouteAction {
        kind,
        source: "repair_submit".to_string(),
        reason,
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(repair_ref),
        details: Some(json!({
            "repairArtifactKind": artifact_kind,
            "repair": repair_value
        })),
        target_phase_id: None,
    }
}

fn update_delivery_after_repair_submit(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    repair_ref: &str,
    next_action: &RouteAction,
    artifact_kind: ArtifactKind,
) -> Result<(), state::store::StateError> {
    let store = FileTransitionStore;
    let mut status = store.load_status(project_root).map_err(to_state_error)?;
    let mut delivery = store
        .load_delivery_index(project_root, &locator.delivery_id)
        .map_err(to_state_error)?;
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == locator.phase_id)
    {
        phase
            .latest_refs
            .insert("repairAcceptedRef".to_string(), repair_ref.to_string());
        phase.latest_refs.insert(
            "repairAcceptedKind".to_string(),
            serde_json::to_value(artifact_kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| "repair".to_string()),
        );
        phase.next_action = Some(next_action.clone());
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    delivery_core::apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)
}

fn value_to_write_target(value: &Value) -> Result<WriteTarget, state::store::StateError> {
    Ok(WriteTarget {
        target_id: value
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                state::store::StateError::InvalidArgument(
                    "write target missing targetId".to_string(),
                )
            })?
            .to_string(),
        path: value
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                state::store::StateError::InvalidArgument("write target missing path".to_string())
            })?
            .to_string(),
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("Write repair artifact.")
            .to_string(),
    })
}

fn safe_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn repair_origin(origin: &str) -> RepairOrigin {
    match origin {
        "review_result" => RepairOrigin::ReviewResult,
        "manual_review_resolution" => RepairOrigin::ManualReviewResolution,
        _ => RepairOrigin::TaskFailure,
    }
}

fn user_change_summary(
    project_root: &Path,
    origin: &str,
    source_ref: Option<&str>,
) -> Option<String> {
    if origin != "manual_review_resolution" {
        return None;
    }
    let source_ref = source_ref?;
    let source_file = from_project_relative(project_root, source_ref).ok()?;
    let value = state::store::read_json_value(&source_file).ok()?;
    value
        .pointer("/changeRequest/summary")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/userAnswer/text").and_then(Value::as_str))
        .map(str::to_string)
}

fn repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
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
        resubmit_tool: "loom.repairSubmitFile".to_string(),
        fix_scope: Some("repair_artifact_candidate_only".to_string()),
        read_groups: authorized.read_groups.clone(),
    })
}

fn issue(code: &str, field_path: &str, message: &str) -> delivery_core::RepairIssue {
    delivery_core::RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("repair".to_string()),
        field_path: Some(field_path.to_string()),
    }
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
            target_batch: Some(9),
            domain: Some("repair".to_string()),
            route_action: Some(route_action.to_string()),
            recovery_tool: Some("loom.continue".to_string()),
        },
    })
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}
