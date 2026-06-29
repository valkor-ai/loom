use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

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
    task_execution::{
        load_current_plan_and_run, runtime_delivery_requirement_read_fields, save_run,
        task_execution_rules,
    },
    task_plan::update_run_summary,
    templates::{
        runtime_delivery_requirement_template, task_result_template,
        taskplan_group_result_template, taskplan_outline_result_template,
    },
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
        RouteActionKind::TaskplanRepair => {
            if let Some(existing) = existing_active_repair_action(
                project_root,
                delivery_id,
                phase_id,
                action,
                "activeRepairActionRef",
                ArtifactKind::TaskplanRepair,
                WriteMode::TaskplanGrouped,
            ) {
                return existing;
            }
            materialize_taskplan_repair(project_root, delivery_id, phase_id, action.request_ref.clone())
                .unwrap_or_else(|error| {
                    failed(
                        project_root,
                        "TASKPLAN_REPAIR_FAILED",
                        error.to_string(),
                        "taskplan_repair",
                    )
                })
        }
        RouteActionKind::ArchitectureArtifactRepair => {
            if let Some(existing) = existing_active_repair_action(
                project_root,
                delivery_id,
                phase_id,
                action,
                "activeRepairActionRef",
                ArtifactKind::ArchitectureArtifactRepair,
                WriteMode::ArchitectureSection,
            ) {
                return existing;
            }
            materialize_architecture_repair(project_root, delivery_id, phase_id, action.request_ref.clone())
                .unwrap_or_else(|error| {
                    failed(
                        project_root,
                        "ARCHITECTURE_REPAIR_FAILED",
                        error.to_string(),
                        "architecture_artifact_repair",
                    )
                })
        }
        RouteActionKind::TaskResultRepair => existing_active_repair_action(
            project_root,
            delivery_id,
            phase_id,
            action,
            "activeTaskResultRepairActionRef",
            ArtifactKind::TaskResultRepair,
            WriteMode::SingleJson,
        )
        .unwrap_or_else(|| {
            failed(
                project_root,
                "ACTIVE_TASK_RESULT_REPAIR_NOT_FOUND",
                "The active TaskResult correction action is missing or stale. Run loom.continue after the original TaskResult validation failure recreates the active repair state.".to_string(),
                "task_result_repair",
            )
        }),
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
    D: DomainDispatcher + Copy,
{
    if authorized.artifact_kind == ArtifactKind::TaskResultRepair {
        return crate::task_result::accept_task_result_repair_file(input, authorized, dispatcher);
    }
    if authorized.artifact_kind == ArtifactKind::TaskplanRepair {
        return crate::task_plan::accept_task_plan_repair_file(input, authorized, dispatcher);
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
        state::store::StateError::InvalidArgument("Repair action missing deliveryId".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Repair action missing phaseId".to_string())
    })?;
    if let Some(stale) = ensure_latest_repair_action(
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
    if let Some(existing) = existing_delivery_execution_repair_next_if_current(
        project_root,
        delivery_id,
        phase_id,
        &task,
        attempt_count,
    )? {
        return Ok(existing);
    }
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
        source_ref.clone(),
        finding_refs.clone(),
        attempt_count,
    );
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "delivery_execution_repair".to_string(),
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
    delivery_execution_repair_next(
        project_root,
        stored.request_ref,
        result_file,
        &task,
        repair_origin,
        origin,
        source_ref,
        finding_refs,
        attempt_count,
    )
}

fn existing_delivery_execution_repair_next_if_current(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    task: &TaskDefinition,
    attempt_count: u32,
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
        return Ok(None);
    };
    let Some(action) = phase.next_action.as_ref() else {
        return Ok(None);
    };
    if action.kind != RouteActionKind::ExecutionRepair
        || action.source != "delivery_execution_repair"
    {
        return Ok(None);
    }
    let Some(request_ref) = phase
        .latest_refs
        .get("taskExecutionRequestRef")
        .or(action.request_ref.as_ref())
    else {
        return Ok(None);
    };
    let Some(result_file) = action
        .details
        .as_ref()
        .and_then(|details| details.get("resultFile"))
        .and_then(Value::as_str)
        .or_else(|| {
            phase
                .latest_refs
                .get("taskExecutionResultFile")
                .map(String::as_str)
        })
    else {
        return Ok(None);
    };
    let origin = action
        .details
        .as_ref()
        .and_then(|details| details.get("origin"))
        .and_then(Value::as_str)
        .unwrap_or("task_failure");
    let source_ref = action
        .details
        .as_ref()
        .and_then(|details| details.get("sourceRef"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let finding_refs = action
        .details
        .as_ref()
        .and_then(|details| details.get("findingRefs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    delivery_execution_repair_next(
        project_root,
        request_ref.to_string(),
        result_file.to_string(),
        task,
        repair_origin(origin),
        origin,
        source_ref,
        finding_refs,
        attempt_count,
    )
    .map(Some)
}

fn delivery_execution_repair_next(
    project_root: &str,
    request_ref: String,
    result_file: String,
    task: &TaskDefinition,
    repair_origin: RepairOrigin,
    origin: &str,
    source_ref: Option<String>,
    finding_refs: Vec<String>,
    attempt_count: u32,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.clone(),
    })?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::ExecuteTask(ExecuteTaskNext {
                execution_kind: ExecutionKind::DeliveryExecutionRepair,
                repair_origin: Some(repair_origin.clone()),
                request_ref,
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
                    repair_origin,
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
                    user_change_summary: user_change_summary(
                        Path::new(project_root),
                        origin,
                        source_ref.as_deref(),
                    ),
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
    source_ref: Option<String>,
    finding_refs: Vec<String>,
    attempt_count: u32,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(contracts::TaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let result_template = task_result_template(&task_plan.task_plan_id, task);
    let mut execution_rules = task_execution_rules(result_file, task, None);
    if let Some(object) = execution_rules.as_object_mut() {
        object.insert(
            "boundaryRules".to_string(),
            json!([
                "Use repairContext as the failure boundary; do not repair unrelated issues.",
                "Do not edit Brainstorm, TechnicalBaseline, PGC, AAC, TaskPlan, ReviewResult, ManualReviewResolution, or .loom state.",
                "Do not expand scope.",
                "Write TaskResult JSON only to outputContract.resultFile."
            ]),
        );
        if let Some(barrier) = object
            .get_mut("completionBarrier")
            .and_then(Value::as_object_mut)
        {
            barrier.insert(
                "rule".to_string(),
                json!("The repair is not complete until TaskResult exists at outputContract.resultFile and loom.recordTaskResultFile succeeds."),
            );
        }
        if let Some(rules) = object
            .get_mut("verificationCommandSchedulingRules")
            .and_then(Value::as_array_mut)
        {
            rules.push(json!(
                "Run the verification needed for the repaired task before TaskResult submission."
            ));
            rules.push(json!(
                "When a failing signal is available in repairContext, rerun that signal or the closest stable equivalent after the fix."
            ));
        }
    }
    let mut repair_core_fields = vec![
        "source.deliveryId",
        "source.phaseId",
        "source.taskPlanId",
        "source.taskPlanRunId",
        "source.taskId",
        "source.groupId",
        "task.taskId",
        "task.groupId",
        "task.title",
        "task.taskKind",
        "task.implementationActions",
        "task.objective",
        "task.dependsOn",
        "task.scopeRefs",
        "task.acceptanceRefs",
        "task.requirementDetailRefs",
        "task.writeBoundary.forbiddenPaths",
        "task.writeBoundary.artifactRefs",
        "task.verificationIntents",
        "repairContext.sourceTaskId",
        "repairContext.repairOrigin",
        "repairContext.attemptCount",
        "repairContext.sourceRef",
        "repairContext.findingRefs",
        "executionRules.completionBarrier",
        "executionRules.finalResponseGuard",
        "executionRules.completionContinuityRequirement",
        "executionRules.verificationCommandSchedulingRules",
        "executionRules.boundaryRules",
    ];
    if !task.concept_refs.is_empty() {
        repair_core_fields.push("task.conceptRefs");
    }
    if !task.concept_responsibilities.is_empty() {
        repair_core_fields.push("task.conceptResponsibilities");
    }
    if !task.concept_verification_intents.is_empty() {
        repair_core_fields.push("task.conceptVerificationIntents");
    }
    if task.frontend_experience_requirement.is_some() {
        repair_core_fields.extend([
            "task.frontendExperienceRequirement.executionGuidance.schemaVersion",
            "task.frontendExperienceRequirement.executionGuidance.purpose",
            "task.frontendExperienceRequirement.executionGuidance.userFacingLanguage",
            "task.frontendExperienceRequirement.executionGuidance.responsibility",
            "task.frontendExperienceRequirement.executionGuidance.surfacesInScope",
            "task.frontendExperienceRequirement.executionGuidance.dataViewsInScope",
            "task.frontendExperienceRequirement.executionGuidance.actionsInScope",
            "task.frontendExperienceRequirement.executionGuidance.operationPathsInScope",
            "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
            "task.frontendExperienceRequirement.executionGuidance.dataBindingExpectation",
            "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
            "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource",
            "task.frontendExperienceRequirement.executionGuidance.guidanceWarnings",
            "executionRules.frontendImplementationOrganizationRules",
            "executionRules.interactiveVerificationProbePolicy",
            "executionRules.controlledRuntimeProbeRules",
        ]);
    }
    if task.runtime_delivery_requirement.is_some() {
        repair_core_fields.extend(runtime_delivery_requirement_read_fields(task));
        repair_core_fields.extend([
            "executionRules.controlledRuntimeProbeRules",
            "executionRules.runtimeDeliveryExecutionRules",
        ]);
    }
    if execution_rules
        .get("frontendImplementationOrganizationRules")
        .is_some()
    {
        repair_core_fields.push("executionRules.frontendImplementationOrganizationRules");
    }
    if execution_rules
        .get("interactiveVerificationProbePolicy")
        .is_some()
    {
        repair_core_fields.push("executionRules.interactiveVerificationProbePolicy");
    }
    if execution_rules.get("controlledRuntimeProbeRules").is_some() {
        repair_core_fields.push("executionRules.controlledRuntimeProbeRules");
    }
    if execution_rules
        .get("runtimeDeliveryExecutionRules")
        .is_some()
    {
        repair_core_fields.push("executionRules.runtimeDeliveryExecutionRules");
    }
    let mut seen_repair_core_fields = BTreeSet::new();
    repair_core_fields.retain(|field| seen_repair_core_fields.insert(*field));
    let mut repair_result_fields = vec![
        "enumRefs.taskResultStatus",
        "enumRefs.verificationStatus",
        "outputContract.resultFile",
        "outputContract.requiredTopLevelFields",
        "outputContract.resultTemplate",
        "outputContract.schemaShape.properties.status",
        "outputContract.schemaShape.properties.changedFiles",
        "outputContract.schemaShape.properties.noChangeReason",
        "outputContract.schemaShape.properties.verificationResults",
        "outputContract.schemaShape.properties.selfRepairSummary",
        "outputContract.schemaShape.properties.failure",
        "outputContract.schemaShape.properties.executionContinuity",
        "outputContract.schemaShape.properties.notes",
        "outputContract.schemaShape.properties.requirementDetailEvidence",
        "outputContract.schemaShape.properties.conceptEvidence",
        "outputContract.schemaShape.properties.blockedReasons",
        "outputContract.resultRules",
        "outputContract.blockedReasonOptions",
        "executionRules.completionBarrier",
    ];
    if task.frontend_experience_requirement.is_some() {
        repair_result_fields
            .push("outputContract.schemaShape.properties.frontendExperienceSelfCheck");
    }
    if task.runtime_delivery_requirement.is_some() {
        repair_result_fields.push("outputContract.schemaShape.properties.runtimeDeliveryEvidence");
    }
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
            "sourceTaskId": task.task_id,
            "repairOrigin": repair_origin,
            "attemptCount": attempt_count,
            "sourceRef": source_ref,
            "findingRefs": finding_refs
        },
        "executionRules": execution_rules,
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
            "blockedReasonOptions": [
                {"code": "DESIGN_INSUFFICIENT", "nextNode": "architecture_artifact_repair"},
                {"code": "TASKPLAN_INVALID", "nextNode": "taskplan_repair"},
                {"code": "DEPENDENCY_NOT_READY", "nextNode": "wait_dependency"}
            ],
            "schemaShape": schema_shape,
            "resultTemplate": result_template,
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
                    "fields": repair_core_fields
                },
                {
                    "groupId": "repair_result_contract",
                    "required": true,
                    "purpose": "Read TaskResult write contract before submitting repair result.",
                    "whenToRead": "Read before writing TaskResult.",
                    "fields": repair_result_fields
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
    materialize_taskplan_repair_action(project_root, delivery_id, phase_id, source_ref)
}

pub fn materialize_architecture_repair(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    source_ref: Option<String>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    materialize_architecture_repair_action(project_root, delivery_id, phase_id, source_ref)
}

fn materialize_taskplan_repair_action(
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

    let source_refs = json!({
        "technicalBaselineRef": value_field(&core_fields, "sourceRefs.technicalBaselineRef"),
        "planningGenerationContractRef": value_field(&core_fields, "sourceRefs.planningGenerationContractRef"),
        "architectureArtifactContractRef": value_field(&core_fields, "sourceRefs.architectureArtifactContractRef"),
        "phaseConceptGroundingRef": value_field(&core_fields, "sourceRefs.phaseConceptGroundingRef"),
        "deliveryConceptGlossaryRef": value_field(&core_fields, "sourceRefs.deliveryConceptGlossaryRef")
    });
    let allowed_refs = json!({
        "scopeRefs": value_field(&core_fields, "allowedRefs.scopeRefs"),
        "acceptanceRefs": value_field(&core_fields, "allowedRefs.acceptanceRefs"),
        "deferredScopeRefs": value_field(&core_fields, "allowedRefs.deferredScopeRefs"),
        "excludedScopeRefs": value_field(&core_fields, "allowedRefs.excludedScopeRefs"),
        "requirementDetailIds": value_field(&core_fields, "allowedRefs.requirementDetailIds"),
        "moduleRefs": value_field(&core_fields, "allowedRefs.moduleRefs"),
        "entityRefs": value_field(&core_fields, "allowedRefs.entityRefs"),
        "interfaceRefs": value_field(&core_fields, "allowedRefs.interfaceRefs"),
        "userFlowRefs": value_field(&core_fields, "allowedRefs.userFlowRefs"),
        "stateMachineRefs": value_field(&core_fields, "allowedRefs.stateMachineRefs"),
        "decisionRefs": value_field(&core_fields, "allowedRefs.decisionRefs"),
        "riskRefs": value_field(&core_fields, "allowedRefs.riskRefs")
    });
    let mut context_projection = json!({
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
    let enum_refs = json!({
        "taskKind": value_field(&contract_fields, "enumRefs.taskKind"),
        "implementationAction": value_field(&contract_fields, "enumRefs.implementationAction"),
        "verificationEvidence": value_field(&contract_fields, "enumRefs.verificationEvidence")
    });
    let frontend_projection = optional_field_value(
        &optional_fields,
        "contextProjection.frontendExperienceProjection",
        "outputContract.frontendExperienceProjection",
    );
    let runtime_projection = optional_field_value(
        &optional_fields,
        "contextProjection.runtimeDeliveryProjection",
        "outputContract.runtimeDeliveryProjection",
    );
    let mut runtime_requirement_template = value_field(
        &contract_fields,
        "outputContract.runtimeDeliveryRequirementTemplate",
    );
    if runtime_requirement_template.is_null() {
        runtime_requirement_template = runtime_delivery_requirement_template(
            (!runtime_projection.is_null()).then_some(&runtime_projection),
        );
    }
    let runtime_closure_template = value_field(
        &contract_fields,
        "outputContract.runtimeDeliveryClosureTaskTemplate",
    );
    context_projection["frontendExperienceProjection"] = frontend_projection.clone();
    context_projection["runtimeDeliveryProjection"] = runtime_projection.clone();
    let optional_projection_fields =
        taskplan_repair_optional_projection_fields(&frontend_projection, &runtime_projection);
    let schema_shape = serde_json::to_value(schema_for!(TaskPlanOutlineCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let group_schema = serde_json::to_value(schema_for!(TaskPlanGroupCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));

    let mut request_root = json!({
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
            "outlineResultTemplate": taskplan_outline_result_template(&request_id, delivery_id, phase_id),
            "groupResultTemplate": taskplan_group_result_template(&request_id, delivery_id, phase_id)
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "taskplan_core_context",
                    "required": true,
                    "purpose": "Read current phase source refs, requirement transfer, and allowed refs before writing the replacement TaskPlan outline.",
                    "whenToRead": "Read first.",
                    "fields": [
                        "sourceRefs.technicalBaselineRef",
                        "sourceRefs.planningGenerationContractRef",
                        "sourceRefs.architectureArtifactContractRef",
                        "sourceRefs.phaseConceptGroundingRef",
                        "sourceRefs.deliveryConceptGlossaryRef",
                        "repairContext.sourceTaskPlanRequestRef",
                        "repairContext.sourceRef",
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
                        "allowedRefs.riskRefs"
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
                        "enumRefs.taskKind",
                        "enumRefs.implementationAction",
                        "enumRefs.verificationEvidence",
                        "outputContract.outlineFile",
                        "outputContract.groupFilePattern",
                        "outputContract.pathAuthority",
                        "outputContract.outlineResultTemplate",
                        "outputContract.groupResultTemplate"
                    ]
                }
            ]
        }
    });
    if !runtime_requirement_template.is_null() {
        request_root
            .pointer_mut("/outputContract")
            .and_then(Value::as_object_mut)
            .expect("taskplan repair outputContract")
            .insert(
                "runtimeDeliveryRequirementTemplate".to_string(),
                runtime_requirement_template,
            );
        request_root
            .pointer_mut("/requestReadPlan/groups/2/fields")
            .and_then(Value::as_array_mut)
            .expect("taskplan repair write contract fields")
            .push(json!("outputContract.runtimeDeliveryRequirementTemplate"));
    }
    if !runtime_closure_template.is_null() {
        request_root
            .pointer_mut("/outputContract")
            .and_then(Value::as_object_mut)
            .expect("taskplan repair outputContract")
            .insert(
                "runtimeDeliveryClosureTaskTemplate".to_string(),
                runtime_closure_template,
            );
        request_root
            .pointer_mut("/requestReadPlan/groups/2/fields")
            .and_then(Value::as_array_mut)
            .expect("taskplan repair write contract fields")
            .push(json!("outputContract.runtimeDeliveryClosureTaskTemplate"));
    }
    if !optional_projection_fields.is_empty() {
        request_root
            .pointer_mut("/requestReadPlan/groups")
            .and_then(Value::as_array_mut)
            .expect("taskplan repair action requestReadPlan groups")
            .push(json!({
                "groupId": "taskplan_optional_projection",
                "required": false,
                "purpose": "Read full frontend/runtime projections only when core projection is insufficient.",
                "whenToRead": "Read on demand.",
                "fields": optional_projection_fields
            }));
    }
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "taskplan_repair".to_string(),
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
    update_latest_repair_action(
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

fn materialize_architecture_repair_action(
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
    let source_refs = json!({
        "planningContractRef": value_field(&core_fields, "sourceRefs.planningContractRef"),
        "technicalBaselineRef": value_field(&core_fields, "sourceRefs.technicalBaselineRef"),
        "brainstormContractRef": value_field(&core_fields, "sourceRefs.brainstormContractRef"),
        "repositoryContextRef": value_field(&core_fields, "sourceRefs.repositoryContextRef"),
        "deliveryConceptGlossaryRef": value_field(&core_fields, "sourceRefs.deliveryConceptGlossaryRef"),
        "phaseConceptGroundingRef": value_field(&core_fields, "sourceRefs.phaseConceptGroundingRef"),
        "confirmedFrontendExperienceRef": value_field(&core_fields, "sourceRefs.confirmedFrontendExperienceRef"),
        "currentFrontendExperienceRef": value_field(&core_fields, "sourceRefs.currentFrontendExperienceRef"),
        "previousRuntimeDeliveryRef": value_field(&core_fields, "sourceRefs.previousRuntimeDeliveryRef")
    });
    let allowed_refs = json!({
        "scopeRefs": value_field(&core_fields, "allowedRefs.scopeRefs"),
        "acceptanceRefs": value_field(&core_fields, "allowedRefs.acceptanceRefs"),
        "deferredScopeRefs": value_field(&core_fields, "allowedRefs.deferredScopeRefs"),
        "excludedScopeRefs": value_field(&core_fields, "allowedRefs.excludedScopeRefs"),
        "requirementDetailIds": value_field(&core_fields, "allowedRefs.requirementDetailIds")
    });
    let planning_value = source_refs
        .get("planningContractRef")
        .and_then(Value::as_str)
        .map(|planning_ref| read_project_json_value(root, planning_ref))
        .transpose()?;
    let frontend_experience_details = planning_value
        .as_ref()
        .and_then(|value| value.pointer("/planningInputs/frontendExperience"))
        .cloned()
        .or_else(|| {
            frontend_fields
                .get("contextProjection.requirementDetailTransfer.frontendExperienceDetails")
                .map(|field| field.value.clone())
        })
        .unwrap_or(Value::Null);
    let user_facing_language = planning_value
        .as_ref()
        .and_then(|value| value.pointer("/planningInputs/userFacingLanguage"))
        .cloned()
        .or_else(|| {
            frontend_fields
                .get("contextProjection.requirementDetailTransfer.userFacingLanguage")
                .map(|field| field.value.clone())
        })
        .unwrap_or(Value::Null);
    let context_projection = json!({
        "phaseScope": {
            "phaseName": value_field(&core_fields, "contextProjection.phaseScope.phaseName"),
            "phaseGoal": value_field(&core_fields, "contextProjection.phaseScope.phaseGoal"),
            "included": value_field(&core_fields, "contextProjection.phaseScope.included"),
            "deferred": value_field(&core_fields, "contextProjection.phaseScope.deferred"),
            "excluded": value_field(&core_fields, "contextProjection.phaseScope.excluded"),
            "acceptanceCandidates": value_field(&core_fields, "contextProjection.phaseScope.acceptanceCandidates")
        },
        "phaseId": field_value(&core_fields, "contextProjection.phaseId")?,
        "planningContractId": field_value(&core_fields, "contextProjection.planningContractId")?,
        "technicalBaseline": {
            "technicalBaselineId": value_field(&core_fields, "contextProjection.technicalBaseline.technicalBaselineId"),
            "status": value_field(&core_fields, "contextProjection.technicalBaseline.status"),
            "scope": value_field(&core_fields, "contextProjection.technicalBaseline.scope"),
            "summary": value_field(&core_fields, "contextProjection.technicalBaseline.summary"),
            "mustFollow": value_field(&core_fields, "contextProjection.technicalBaseline.mustFollow")
        },
        "requirementDetailTransfer": {
            "requirementDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.requirementDetails")?,
            "acceptanceDetails": field_value(&core_fields, "contextProjection.requirementDetailTransfer.acceptanceDetails")?,
            "businessFlows": field_value(&core_fields, "contextProjection.requirementDetailTransfer.businessFlows")?,
            "frontendExperienceDetails": frontend_experience_details,
            "userFacingLanguage": user_facing_language
        }
    });
    let frontend_experience_source = if frontend_fields.is_empty() {
        frontend_experience_source_from_source_refs(&source_refs)
    } else {
        json!({
            "confirmedFrontendExperienceRef": value_field(&frontend_fields, "frontendExperienceSource.confirmedFrontendExperienceRef"),
            "currentFrontendExperienceRef": value_field(&frontend_fields, "frontendExperienceSource.currentFrontendExperienceRef"),
            "repositoryContextRef": value_field(&frontend_fields, "frontendExperienceSource.repositoryContextRef"),
            "authorityRule": value_field(&frontend_fields, "frontendExperienceSource.authorityRule")
        })
    };
    let runtime_authority = if source_refs
        .get("previousRuntimeDeliveryRef")
        .and_then(Value::as_str)
        .is_some()
    {
        json!("A previous runtime delivery exists in sourceRefs.previousRuntimeDeliveryRef. Use runtimeDelivery.status=unchanged only when copying that ref exactly; otherwise use modified or not_applicable.")
    } else {
        json!("No previous runtime delivery exists for this phase. runtimeDelivery.status must be modified or not_applicable; do not use unchanged and do not write basis.previousRuntimeDeliveryRef.")
    };
    let section_outputs = build_architecture_repair_section_outputs(
        root,
        &request_id,
        delivery_id,
        phase_id,
        &frontend_experience_source,
        &context_projection,
    )?;
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
            "groups": architecture_repair_read_groups(ArchitectureSectionGroup::Foundation)
        }
    });
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "architecture_artifact_repair".to_string(),
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
    update_latest_repair_action(
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

fn value_field(fields: &BTreeMap<String, delivery_core::FieldReadResult>, field: &str) -> Value {
    fields
        .get(field)
        .map(|result| result.value.clone())
        .unwrap_or(Value::Null)
}

fn frontend_experience_source_from_source_refs(source_refs: &Value) -> Value {
    let mut object = serde_json::Map::new();
    for key in [
        "confirmedFrontendExperienceRef",
        "currentFrontendExperienceRef",
        "repositoryContextRef",
    ] {
        if let Some(value) = source_refs.get(key) {
            object.insert(key.to_string(), value.clone());
        }
    }
    object.insert(
        "authorityRule".to_string(),
        json!("Use confirmed/current frontend refs as the frontend_experience authority. RepositoryContext and TechnicalBaseline are implementation facts only."),
    );
    Value::Object(object)
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

fn read_project_json_value(
    project_root: &Path,
    relative: &str,
) -> Result<Value, state::store::StateError> {
    state::store::read_json_value(&from_project_relative(project_root, relative)?)
}

fn optional_field_value(
    fields: &BTreeMap<String, delivery_core::FieldReadResult>,
    primary: &str,
    migration_fallback: &str,
) -> Value {
    fields
        .get(primary)
        .or_else(|| fields.get(migration_fallback))
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null)
}

fn taskplan_repair_optional_projection_fields(
    frontend_projection: &Value,
    runtime_projection: &Value,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if !frontend_projection.is_null() {
        fields.push("contextProjection.frontendExperienceProjection");
    }
    if !runtime_projection.is_null() {
        fields.push("contextProjection.runtimeDeliveryProjection");
    }
    fields
}

fn architecture_repair_read_groups(section: ArchitectureSectionGroup) -> Value {
    let mut groups = vec![
        json!({
            "groupId": "architecture_core_context",
            "required": true,
            "purpose": "Read the current-phase planning authority, repair context, and allowed refs before generating the replacement Architecture section.",
            "whenToRead": "Read before drafting any replacement Architecture section candidate.",
            "fields": [
                "sourceRefs.planningContractRef",
                "sourceRefs.technicalBaselineRef",
                "sourceRefs.brainstormContractRef",
                "sourceRefs.repositoryContextRef",
                "sourceRefs.deliveryConceptGlossaryRef",
                "sourceRefs.phaseConceptGroundingRef",
                "sourceRefs.confirmedFrontendExperienceRef",
                "sourceRefs.currentFrontendExperienceRef",
                "sourceRefs.previousRuntimeDeliveryRef",
                "repairContext.sourceArchitectureRequestRef",
                "repairContext.sourceRef",
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
                "allowedRefs.requirementDetailIds"
            ]
        }),
        json!({
            "groupId": "architecture_section_contract",
            "required": true,
            "purpose": "Read the current section contract, schema projection, and write target before writing the replacement section candidate.",
            "whenToRead": "Read immediately before writing the current replacement Architecture section candidate.",
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

fn build_architecture_repair_section_outputs(
    project_root: &Path,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    frontend_experience_source: &Value,
    context_projection: &Value,
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
                "resultTemplate": architecture_repair_section_result_template(
                    request_id,
                    delivery_id,
                    phase_id,
                    *section,
                    frontend_experience_source,
                    context_projection
                ),
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

fn architecture_repair_section_result_template(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    section: ArchitectureSectionGroup,
    frontend_experience_source: &Value,
    context_projection: &Value,
) -> Value {
    json!({
        "schemaVersion": "1.0",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "section": section,
        "status": "ready",
        "content": architecture_repair_section_content_template(
            section,
            frontend_experience_source,
            context_projection
        ),
        "blockedReasons": [],
        "createdAt": "ISO-8601 datetime"
    })
}

fn architecture_repair_section_content_template(
    section: ArchitectureSectionGroup,
    frontend_experience_source: &Value,
    context_projection: &Value,
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
        ArchitectureSectionGroup::RuntimeDelivery => json!({
            "runtimeDelivery": {
                "status": "modified",
                "basis": {
                    "technicalBaselineRef": ""
                },
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
        }),
        ArchitectureSectionGroup::Coverage => coverage_content_template(context_projection),
    }
}

fn coverage_content_template(context_projection: &Value) -> Value {
    let acceptance_matrix = context_projection
        .pointer("/requirementDetailTransfer/acceptanceDetails")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|acceptance| {
                    json!({
                        "acceptanceId": acceptance.get("id").cloned().unwrap_or(Value::Null),
                        "priority": acceptance.get("priority").cloned().unwrap_or_else(|| json!("must")),
                        "statement": acceptance.get("statement").cloned().unwrap_or(Value::Null),
                        "coverageStatus": "covered",
                        "reason": "",
                        "coverage": [acceptance_coverage_artifact_template()],
                        "verificationHints": [{
                            "kind": "manual",
                            "description": ""
                        }]
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let detail_coverage = context_projection
        .pointer("/requirementDetailTransfer/requirementDetails/items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|detail| {
                    json!({
                        "detailId": detail.get("detailId").cloned().unwrap_or(Value::Null),
                        "coverageStatus": "covered",
                        "artifactRefs": detail_coverage_artifact_refs_template(),
                        "reason": ""
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
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

fn acceptance_coverage_artifact_template() -> Value {
    json!({
        "type": "modules",
        "refs": [],
        "description": ""
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
        phase.latest_refs.insert(
            "taskExecutionResultFile".to_string(),
            result_file.to_string(),
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

fn existing_active_repair_action(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    action: &RouteAction,
    latest_ref_key: &str,
    artifact_kind: ArtifactKind,
    write_mode: WriteMode,
) -> Option<LoomMcpActionResult> {
    let store = FileTransitionStore;
    let delivery = match store.load_delivery_index(project_root, delivery_id) {
        Ok(delivery) => delivery,
        Err(error) => {
            return Some(failed(
                project_root,
                "REPAIR_ROUTE_STATE_READ_FAILED",
                error.to_string(),
                "repair",
            ));
        }
    };
    let Some(phase) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return Some(failed(
            project_root,
            "REPAIR_PHASE_NOT_FOUND",
            format!("Phase {phase_id} was not found for delivery {delivery_id}."),
            "repair",
        ));
    };
    let latest = phase.latest_refs.get(latest_ref_key).map(String::as_str);
    if latest.is_none()
        && action.source != "repair_action"
        && action.kind != RouteActionKind::TaskResultRepair
    {
        return None;
    }
    let request_ref = action.request_ref.as_deref().or(latest)?;
    if latest != Some(request_ref) {
        return Some(failed(
            project_root,
            "STALE_REPAIR_ACTION",
            "Continue found a stale repair action requestRef; rerun loom.continue after the active phase state is refreshed.".to_string(),
            "repair",
        ));
    }
    Some(existing_write_artifact_next(
        project_root,
        request_ref,
        artifact_kind,
        write_mode,
    ))
}

fn existing_write_artifact_next(
    project_root: &str,
    request_ref: &str,
    artifact_kind: ArtifactKind,
    write_mode: WriteMode,
) -> LoomMcpActionResult {
    let inspected = match state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    }) {
        Ok(inspected) => inspected,
        Err(error) => {
            return failed(
                project_root,
                "REPAIR_REQUEST_INSPECT_FAILED",
                error.to_string(),
                "repair",
            );
        }
    };
    let write_targets = match inspected
        .write_targets
        .iter()
        .map(value_to_write_target)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(targets) => targets,
        Err(error) => {
            return failed(
                project_root,
                "REPAIR_WRITE_TARGET_INVALID",
                error.to_string(),
                "repair",
            );
        }
    };
    LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
        project_root.to_string(),
        LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
            artifact_kind,
            request_ref: request_ref.to_string(),
            write_mode,
            write_targets,
            read_groups: inspected.read_groups,
            submit_tool: "loom.repairSubmitFile".to_string(),
        }),
    ))
}

fn update_latest_repair_action(
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
            .insert("activeRepairActionRef".to_string(), request_ref.to_string());
        phase.next_action = Some(RouteAction {
            kind: match repair_type {
                "taskplan_repair" => RouteActionKind::TaskplanRepair,
                "architecture_artifact_repair" => RouteActionKind::ArchitectureArtifactRepair,
                "task_result_repair" => RouteActionKind::TaskResultRepair,
                _ => RouteActionKind::ExecutionRepair,
            },
            source: "repair_action".to_string(),
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

fn ensure_latest_repair_action(
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
        .and_then(|phase| phase.latest_refs.get("activeRepairActionRef"))
        .map(String::as_str);
    if latest != Some(request_ref) {
        return Ok(Some(failed(
            project_root,
            "STALE_REPAIR_ACTION",
            "Repair submit must use the active phase repair action requestRef.".to_string(),
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
