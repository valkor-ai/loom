use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    TaskDefinition, TaskKind, TaskPlanRunNextAction, TaskPlanRunStatus, TaskResult,
    TaskResultStatus, TaskRunStatus, VerificationEvidence,
};
use delivery_core::{
    apply_delivery_index, ArtifactKind, DeliveryLifecycleStatus, DomainDispatcher, FileSubmitInput,
    LoomMcpActionResult, LoomMcpAutoRunnableResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpNextAction, LoomMcpRepairableErrorResult, OperationContext, RouteAction,
    RouteActionKind, SubmitAcceptedEvent, TransitionEngine, TransitionStore, WriteArtifactNext,
    WriteMode, WriteTarget,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
};

use crate::{
    paths::task_result_file,
    task_execution::{
        load_current_plan_and_run, runtime_delivery_requirement_read_fields, save_run,
    },
    task_plan::update_run_summary,
    templates::{
        frontend_quality_self_check_applies, frontend_self_check_applies,
        runtime_delivery_evidence_applies, task_result_template,
        FRONTEND_QUALITY_CONTRACT_READ_FIELDS,
    },
};

pub fn accept_task_result_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    match accept_task_result_file_inner(input, authorized, false, dispatcher) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "TASK_RESULT_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("execution".to_string()),
                route_action: Some("record_task_result".to_string()),
                recovery_tool: Some("loom.continue".to_string()),
            },
        }),
    }
}

pub fn accept_task_result_repair_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    match accept_task_result_file_inner(input, authorized, true, dispatcher) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "TASK_RESULT_REPAIR_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(9),
                domain: Some("execution".to_string()),
                route_action: Some("task_result_repair_submit".to_string()),
                recovery_tool: Some("loom.continue".to_string()),
            },
        }),
    }
}

fn accept_task_result_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    repair_submit: bool,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskResult request is missing deliveryId".to_string(),
        )
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskResult request is missing phaseId".to_string(),
        )
    })?;
    if repair_submit {
        if let Some(stale) = ensure_latest_task_result_repair_action(
            &input.project_root,
            &delivery_id,
            &phase_id,
            &input.request_ref,
        )? {
            return Ok(stale);
        }
    } else {
        if let Some(stale) = ensure_latest_request(
            &input.project_root,
            &delivery_id,
            &phase_id,
            &input.request_ref,
        )? {
            return Ok(stale);
        }
    }
    let target = authorized.targets.first().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskResult submit requires a result target.".to_string(),
        )
    })?;
    let root = Path::new(&input.project_root);
    let raw_result = read_project_json_value(root, &target.path)?;
    let allowed_read_fields = authorized
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut fields_to_read = vec![
        "source.taskPlanId".to_string(),
        "source.taskId".to_string(),
        "source.taskPlanRunId".to_string(),
        "task.taskId".to_string(),
        "task.taskKind".to_string(),
        "task.acceptanceRefs".to_string(),
        "task.requirementDetailRefs".to_string(),
        "task.verificationIntents".to_string(),
        "outputContract.resultFile".to_string(),
        "outputContract.requiredTopLevelFields".to_string(),
    ];
    for optional_field in [
        "task.conceptRefs",
        "outputContract.blockedReasonOptions",
        "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
    ] {
        if allowed_read_fields.contains(optional_field) {
            fields_to_read.push(optional_field.to_string());
        }
    }
    for optional_field in FRONTEND_QUALITY_CONTRACT_READ_FIELDS {
        if allowed_read_fields.contains(optional_field) {
            fields_to_read.push(optional_field.to_string());
        }
    }
    if allowed_read_fields.contains("task.runtimeDeliveryRequirement") {
        fields_to_read.push("task.runtimeDeliveryRequirement".to_string());
    } else {
        for runtime_field in [
            "task.runtimeDeliveryRequirement.appliesToThisTask",
            "task.runtimeDeliveryRequirement.reason",
            "task.runtimeDeliveryRequirement.runtimeDeliveryRef",
            "task.runtimeDeliveryRequirement.affectedContractFields",
            "task.runtimeDeliveryRequirement.requiredCodeLevelChecks",
            "task.runtimeDeliveryRequirement.evidenceExpectedInTaskResult",
            "task.runtimeDeliveryRequirement.forbiddenActions",
            "task.runtimeDeliveryRequirement.source",
            "task.runtimeDeliveryRequirement.deploymentFailureRef",
        ] {
            if allowed_read_fields.contains(runtime_field) {
                fields_to_read.push(runtime_field.to_string());
            }
        }
    }
    let fields =
        read_request_fields_chunked(&input.project_root, &input.request_ref, fields_to_read)?;
    let task_plan_id = string_field(&fields, "source.taskPlanId")?;
    let task_id = string_field(&fields, "source.taskId")?;
    let run_id = string_field(&fields, "source.taskPlanRunId")?;
    let result_file = string_field(&fields, "outputContract.resultFile")?;
    let frontend_experience_requirement = frontend_experience_requirement_from_fields(&fields);
    let task: TaskDefinition = serde_json::from_value(json!({
        "taskId": value_field(&fields, "task.taskId"),
        "groupId": "",
        "title": "",
        "taskKind": value_field(&fields, "task.taskKind"),
        "implementationActions": [],
        "objective": "",
        "dependsOn": [],
        "scopeRefs": [],
        "acceptanceRefs": array_field(&fields, "task.acceptanceRefs"),
        "requirementDetailRefs": array_field(&fields, "task.requirementDetailRefs"),
        "writeBoundary": {
            "forbiddenPaths": [],
            "artifactRefs": {}
        },
        "verificationIntents": array_field(&fields, "task.verificationIntents"),
        "conceptRefs": array_field(&fields, "task.conceptRefs"),
        "conceptResponsibilities": [],
        "conceptVerificationIntents": [],
        "frontendExperienceRequirement": frontend_experience_requirement,
        "runtimeDeliveryRequirement": runtime_delivery_requirement_from_fields(&fields)
    }))
    .map_err(state::store::StateError::Json)?;
    let required_top_level_fields =
        string_vec_field(&fields, "outputContract.requiredTopLevelFields")?;
    let blocked_output = json!({
        "blockedReasons": array_field(&fields, "outputContract.blockedReasonOptions")
    });
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let normalized_result = normalize_task_result_machine_fields(
        raw_result.clone(),
        &authorized.request_id,
        &task_plan_id,
        &task_id,
        &task,
    );
    let result: TaskResult = match serde_json::from_value(normalized_result.clone()) {
        Ok(result) => result,
        Err(error) => {
            return repair_task_result_or_error(
                input,
                authorized,
                target.path.clone(),
                vec![issue(
                    "TASK_RESULT_SCHEMA_INVALID",
                    "$",
                    &format!("TaskResult JSON has an invalid schema: {error}"),
                )],
                repair_submit,
                None,
            )
        }
    };
    let previous_changed_files = previous_persisted_changed_files(root, &locator, &run_id, &result);
    let issues = validate_result(
        &normalized_result,
        &result,
        &task,
        &required_top_level_fields,
        &blocked_output,
        &task_plan_id,
        &task_id,
        &result_file,
        &target.path,
    );
    if !issues.is_empty() {
        return repair_task_result_or_error(
            input,
            authorized,
            target.path.clone(),
            issues,
            repair_submit,
            Some(RepairContextInput {
                task_plan_id,
                task_id,
                run_id,
                task,
                result_file,
                required_top_level_fields,
                blocked_output,
                submitted_result: normalized_result.clone(),
                previous_changed_files,
            }),
        );
    }
    let (_task_plan, mut run) = load_current_plan_and_run(root, &locator)?;
    if run.run_id != run_id {
        return Ok(failed(
            &input.project_root,
            "TASKPLAN_RUN_MISMATCH",
            "TaskResult request does not match current TaskPlanRun.".to_string(),
            "record_task_result",
        ));
    }
    let persisted = task_result_file(
        root,
        &locator,
        &run.run_id,
        &result.task_id,
        &result.task_result_id,
    );
    state::store::write_json_atomic(&persisted, &result)?;
    let persisted_ref = to_project_relative(root, &persisted)?;
    let now = state::store::now_string();
    if let Some(state) = run
        .task_states
        .iter_mut()
        .find(|state| state.task_id == result.task_id)
    {
        state.status = match result.status {
            TaskResultStatus::Completed => TaskRunStatus::Completed,
            TaskResultStatus::CompletedWithNotes => TaskRunStatus::CompletedWithNotes,
            TaskResultStatus::Blocked => TaskRunStatus::Blocked,
            TaskResultStatus::Failed => TaskRunStatus::Failed,
        };
        state.result_id = Some(result.task_result_id.clone());
        state.finished_at = Some(now.clone());
        state.attempts.push(contracts::TaskAttemptState {
            attempt: state.attempts.len() as u32 + 1,
            result_id: result.task_result_id.clone(),
            status: state.status,
        });
    }
    for group in &mut run.group_states {
        let states = run
            .task_states
            .iter()
            .filter(|state| state.group_id.as_deref() == Some(group.group_id.as_str()))
            .collect::<Vec<_>>();
        if states
            .iter()
            .any(|state| state.status == TaskRunStatus::Failed)
        {
            group.status = TaskRunStatus::Failed;
            group.finished_at = Some(now.clone());
        } else if states
            .iter()
            .any(|state| state.status == TaskRunStatus::Blocked)
        {
            group.status = TaskRunStatus::Blocked;
            group.finished_at = Some(now.clone());
        } else if !states.is_empty()
            && states.iter().all(|state| {
                matches!(
                    state.status,
                    TaskRunStatus::Completed | TaskRunStatus::CompletedWithNotes
                )
            })
        {
            group.status = if states
                .iter()
                .any(|state| state.status == TaskRunStatus::CompletedWithNotes)
            {
                TaskRunStatus::CompletedWithNotes
            } else {
                TaskRunStatus::Completed
            };
            group.finished_at = Some(now.clone());
        }
    }
    update_run_summary(&mut run);
    run.status = if run.summary.failed > 0 {
        TaskPlanRunStatus::Failed
    } else if run.summary.blocked > 0 {
        TaskPlanRunStatus::Blocked
    } else if run.summary.pending == 0 && run.summary.running == 0 {
        if run.summary.completed_with_notes > 0 {
            TaskPlanRunStatus::CompletedWithNotes
        } else {
            TaskPlanRunStatus::Completed
        }
    } else {
        TaskPlanRunStatus::Running
    };
    run.next_action = next_action_for_run(&run);
    run.updated_at = now;
    save_run(root, &locator, &run)?;
    let Some(next_action) = route_action_for_task_result(&run, &result, &persisted_ref)? else {
        update_delivery_after_result(
            &input.project_root,
            &delivery_id,
            &phase_id,
            &run,
            &persisted_ref,
            None,
        )?;
        return Ok(failed(
            &input.project_root,
            "BLOCKED_TASK_CANNOT_ROUTE_EXECUTION_REPAIR",
            "Blocked TaskResult must route to taskplan_repair, architecture_artifact_repair, or needs_user_decision instead of execution_repair.".to_string(),
            "blocked_task_result",
        ));
    };
    update_delivery_after_result(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &run,
        &persisted_ref,
        Some(&next_action),
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
                source_tool: if repair_submit {
                    "loom.repairSubmitFile".to_string()
                } else {
                    "loom.recordTaskResultFile".to_string()
                },
                accepted_artifact_ref: persisted_ref,
                next_action: Some(next_action),
            },
        )
        .map_err(to_state_error)
}

fn validate_result(
    raw_result: &Value,
    result: &TaskResult,
    task: &TaskDefinition,
    required_top_level_fields: &[String],
    blocked_output: &Value,
    task_plan_id: &str,
    task_id: &str,
    expected_file: &str,
    actual_file: &str,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    for field in required_top_level_fields {
        if raw_result.get(field).is_none() {
            issues.push(issue(
                "TASK_RESULT_REQUIRED_FIELD_MISSING",
                field,
                "TaskResult must include every outputContract.requiredTopLevelFields entry.",
            ));
        }
    }
    if expected_file != actual_file {
        issues.push(issue(
            "RESULT_FILE_MISMATCH",
            "outputContract.resultFile",
            "TaskResult must be written to the request resultFile.",
        ));
    }
    if result.task_plan_id != task_plan_id {
        issues.push(issue(
            "TASKPLAN_ID_MISMATCH",
            "taskPlanId",
            "TaskResult taskPlanId must match the TaskExecution request.",
        ));
    }
    if result.task_id != task_id {
        issues.push(issue(
            "TASK_ID_MISMATCH",
            "taskId",
            "TaskResult taskId must match the TaskExecution request.",
        ));
    }
    for file in &result.changed_files {
        if !is_safe_project_relative_path(file) {
            issues.push(issue(
                "TASK_RESULT_PATH_INVALID",
                "changedFiles",
                "TaskResult changedFiles must use safe project-relative source paths.",
            ));
        }
    }
    if matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) && result.changed_files.is_empty()
        && !allows_empty_changed_files(task, result)
    {
        issues.push(issue(
            "TASK_RESULT_STATUS_INCONSISTENT",
            "changedFiles",
            "Completed TaskResult must include changedFiles unless the task is verification-only or noChangeReason explicitly allows no code changes.",
        ));
    }
    if !matches!(result.status, TaskResultStatus::Failed) && result.failure.is_some() {
        issues.push(issue(
            "FAILURE_MUST_BE_NULL",
            "failure",
            "Non-failed TaskResult must not include failure details.",
        ));
    }
    if matches!(result.status, TaskResultStatus::Failed) && result.failure.is_none() {
        issues.push(issue(
            "FAILURE_REQUIRED",
            "failure",
            "Failed TaskResult must include failure.",
        ));
    }
    validate_self_repair(result, &mut issues);
    validate_verification_results(result, task, &mut issues);
    validate_requirement_detail_evidence(result, task, &mut issues);
    validate_concept_evidence(result, task, &mut issues);
    validate_runtime_delivery_evidence(result, task, &mut issues);
    validate_frontend_experience_self_check(result, task, &mut issues);
    validate_frontend_quality_self_check(result, task, &mut issues);
    validate_blocked_reasons(result, blocked_output, &mut issues);
    if result
        .execution_continuity
        .task_result_submitted_after_verification
        == false
    {
        issues.push(issue(
            "EXECUTION_CONTINUITY_REQUIRED",
            "executionContinuity.taskResultSubmittedAfterVerification",
            "TaskResult must confirm verification returned control before submission.",
        ));
    }
    if result.execution_continuity.agent_owned_long_running_work == "unknown"
        && matches!(result.status, TaskResultStatus::Completed)
    {
        issues.push(issue(
            "EXECUTION_CONTINUITY_REQUIRED",
            "executionContinuity.agentOwnedLongRunningWork",
            "Completed TaskResult must not leave agent-owned long-running work as unknown.",
        ));
    }
    if result.execution_continuity.agent_owned_long_running_work == "unknown"
        && result.notes.is_empty()
        && result.execution_continuity.notes.is_empty()
    {
        issues.push(issue(
            "EXECUTION_CONTINUITY_REQUIRED",
            "executionContinuity.notes",
            "TaskResult must explain unknown agent-owned long-running work in notes or executionContinuity.notes.",
        ));
    }
    issues
}

fn normalize_task_result_machine_fields(
    mut raw_result: Value,
    request_id: &str,
    task_plan_id: &str,
    task_id: &str,
    task: &TaskDefinition,
) -> Value {
    let Some(object) = raw_result.as_object_mut() else {
        return raw_result;
    };

    let now = state::store::now_string();
    object.insert("schemaVersion".to_string(), json!("1.0"));
    if !object
        .get("taskResultId")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
    {
        object.insert(
            "taskResultId".to_string(),
            json!(format!("taskresult-{}", safe_id(request_id))),
        );
    }
    object.insert("taskPlanId".to_string(), json!(task_plan_id));
    object.insert("taskId".to_string(), json!(task_id));
    if !object
        .get("changedFiles")
        .and_then(Value::as_array)
        .is_some()
    {
        object.insert("changedFiles".to_string(), json!([]));
    }
    if !object.contains_key("noChangeReason") {
        object.insert("noChangeReason".to_string(), Value::Null);
    }
    if !object.contains_key("selfRepairSummary") {
        object.insert(
            "selfRepairSummary".to_string(),
            json!({
                "attempted": false,
                "attemptCount": 0,
                "stopReason": "not_attempted",
                "progressObserved": false
            }),
        );
    }
    if object
        .get("status")
        .and_then(Value::as_str)
        .map(|status| status != "failed")
        .unwrap_or(false)
    {
        object.insert("failure".to_string(), Value::Null);
    }
    if !object.get("notes").and_then(Value::as_array).is_some() {
        object.insert("notes".to_string(), json!([]));
    }
    if object
        .get("status")
        .and_then(Value::as_str)
        .map(|status| status != "blocked")
        .unwrap_or(false)
    {
        object.insert("blockedReasons".to_string(), json!([]));
    } else if !object
        .get("blockedReasons")
        .and_then(Value::as_array)
        .is_some()
    {
        object.insert("blockedReasons".to_string(), json!([]));
    }
    if !object
        .get("createdAt")
        .and_then(Value::as_str)
        .is_some_and(is_iso_datetime_string)
    {
        object.insert("createdAt".to_string(), json!(now.clone()));
    }
    if !object
        .get("updatedAt")
        .and_then(Value::as_str)
        .is_some_and(is_iso_datetime_string)
    {
        object.insert("updatedAt".to_string(), json!(now));
    }

    normalize_verification_result_machine_fields(object, task);

    let detail_ids = required_requirement_detail_ids(task);
    normalize_requirement_detail_evidence_machine_fields(object, task, &detail_ids);

    if let Some(concepts) = object
        .get_mut("conceptEvidence")
        .and_then(Value::as_array_mut)
    {
        normalize_indexed_object_string_field(concepts, "conceptRef", &task.concept_refs);
    }

    if let Some(requirement) = &task.runtime_delivery_requirement {
        if requirement.applies_to_this_task {
            normalize_runtime_delivery_evidence(object, requirement);
        }
    }

    if let Some(requirement) = &task.frontend_experience_requirement {
        normalize_frontend_experience_self_check(object, requirement);
    }

    raw_result
}

fn is_iso_datetime_string(value: &str) -> bool {
    value.contains('T')
        && (value.ends_with('Z') || value.contains('+') || value.rsplit_once('-').is_some())
        && !value.contains("ISO-8601")
}

fn normalize_verification_result_machine_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
) {
    let raw_items = object
        .get("verificationResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let mut normalized = Vec::new();
    for (index, intent) in task.verification_intents.iter().enumerate() {
        let matching_index = raw_items
            .iter()
            .enumerate()
            .find(|(item_index, item)| {
                !used.contains(item_index)
                    && item
                        .get("verificationId")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value == intent.verification_id)
            })
            .map(|(item_index, _)| item_index)
            .or_else(|| (index < raw_items.len()).then_some(index));
        let raw = matching_index
            .and_then(|item_index| {
                used.insert(item_index);
                raw_items.get(item_index).cloned()
            })
            .unwrap_or_else(|| json!({}));
        let mut item = raw.as_object().cloned().unwrap_or_default();
        item.insert(
            "verificationId".to_string(),
            json!(intent.verification_id.clone()),
        );
        if !item
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            item.insert("status".to_string(), json!("not_run"));
        }
        let evidence_type = item
            .get("evidenceType")
            .and_then(Value::as_str)
            .map(str::to_string);
        let evidence_allowed = evidence_type.as_ref().is_some_and(|candidate| {
            intent
                .acceptable_evidence
                .iter()
                .any(|allowed| verification_evidence_name(*allowed) == candidate)
        });
        if !evidence_allowed {
            if let Some(first) = intent.acceptable_evidence.first() {
                item.insert(
                    "evidenceType".to_string(),
                    json!(verification_evidence_name(*first)),
                );
            } else {
                item.remove("evidenceType");
            }
        }
        if !item
            .get("summary")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            item.insert(
                "summary".to_string(),
                json!("Verification result was not reported before TaskResult submission."),
            );
        }
        normalized.push(Value::Object(item));
    }
    for (index, item) in raw_items.into_iter().enumerate() {
        if !used.contains(&index) {
            normalized.push(item);
        }
    }
    object.insert("verificationResults".to_string(), Value::Array(normalized));
}

fn normalize_requirement_detail_evidence_machine_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
    detail_ids: &[String],
) {
    let raw_items = object
        .get("requirementDetailEvidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let mut normalized = Vec::new();
    for (index, detail_id) in detail_ids.iter().enumerate() {
        let matching_index = raw_items
            .iter()
            .enumerate()
            .find(|(item_index, item)| {
                !used.contains(item_index)
                    && item
                        .get("detailId")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value == detail_id)
            })
            .map(|(item_index, _)| item_index)
            .or_else(|| (index < raw_items.len()).then_some(index));
        let raw = matching_index
            .and_then(|item_index| {
                used.insert(item_index);
                raw_items.get(item_index).cloned()
            })
            .unwrap_or_else(|| json!({}));
        let mut item = raw.as_object().cloned().unwrap_or_default();
        item.insert("detailId".to_string(), json!(detail_id));
        if !item
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            item.insert("status".to_string(), json!("not_verified"));
        }
        let detail_verification_ids = verification_ids_for_detail(task, detail_id);
        if !detail_verification_ids.is_empty() {
            item.insert(
                "verificationIds".to_string(),
                json!(detail_verification_ids),
            );
        } else if !item
            .get("verificationIds")
            .and_then(Value::as_array)
            .is_some()
        {
            item.insert("verificationIds".to_string(), json!([]));
        }
        if !item.get("evidenceRefs").and_then(Value::as_array).is_some() {
            item.insert("evidenceRefs".to_string(), json!([]));
        }
        if !item
            .get("summary")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            item.insert(
                "summary".to_string(),
                json!("Requirement detail evidence was not reported before TaskResult submission."),
            );
        }
        normalized.push(Value::Object(item));
    }
    for (index, item) in raw_items.into_iter().enumerate() {
        if !used.contains(&index) {
            normalized.push(item);
        }
    }
    object.insert(
        "requirementDetailEvidence".to_string(),
        Value::Array(normalized),
    );
}

fn normalize_indexed_object_string_field(items: &mut [Value], field: &str, values: &[String]) {
    for (index, item) in items.iter_mut().enumerate() {
        let Some(value) = values.get(index) else {
            continue;
        };
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        object.insert(field.to_string(), json!(value));
    }
}

fn normalize_runtime_delivery_evidence(
    result_object: &mut serde_json::Map<String, Value>,
    requirement: &contracts::TaskRuntimeDeliveryRequirement,
) {
    let Some(evidence) = result_object
        .get_mut("runtimeDeliveryEvidence")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(runtime_delivery_ref) = &requirement.runtime_delivery_ref {
        evidence.insert("requirementRef".to_string(), json!(runtime_delivery_ref));
    }
    if !requirement.affected_contract_fields.is_empty() {
        evidence.insert(
            "checkedFields".to_string(),
            json!(requirement.affected_contract_fields),
        );
    }
    let required_checks = &requirement.required_code_level_checks;
    let Some(checks) = evidence
        .get_mut("codeLevelChecks")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for (index, check) in checks.iter_mut().enumerate() {
        let Some(required) = required_checks.get(index) else {
            continue;
        };
        let Some(check_object) = check.as_object_mut() else {
            continue;
        };
        check_object.insert("checkId".to_string(), json!(required.check_id));
        if let Some(contract_field) = &required.contract_field {
            check_object.insert("contractField".to_string(), json!(contract_field));
        }
    }
}

fn normalize_frontend_experience_self_check(
    result_object: &mut serde_json::Map<String, Value>,
    requirement: &Value,
) {
    let Some(self_check) = result_object
        .get_mut("frontendExperienceSelfCheck")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let closure_ids = workflow_closure_requirement_ids(requirement);
    if closure_ids.is_empty() {
        return;
    }
    self_check.insert("closureRequirementIds".to_string(), json!(closure_ids));
}

fn validate_self_repair(result: &TaskResult, issues: &mut Vec<delivery_core::RepairIssue>) {
    if matches!(result.status, TaskResultStatus::Failed) && result.self_repair_summary.is_none() {
        issues.push(issue(
            "SELF_REPAIR_SUMMARY_REQUIRED",
            "selfRepairSummary",
            "Failed TaskResult must include selfRepairSummary.",
        ));
    }
    let Some(summary) = &result.self_repair_summary else {
        return;
    };
    if !summary.attempted
        && (summary.attempt_count != 0
            || summary.stop_reason != "not_attempted"
            || summary.progress_observed)
    {
        issues.push(issue(
            "SELF_REPAIR_SUMMARY_INVALID",
            "selfRepairSummary",
            "When selfRepairSummary.attempted is false, attemptCount must be 0, stopReason must be not_attempted, and progressObserved must be false.",
        ));
    }
    if summary.attempted
        && (summary.attempt_count == 0
            || summary.attempt_count > 8
            || summary.stop_reason == "not_attempted")
    {
        issues.push(issue(
            "SELF_REPAIR_SUMMARY_INVALID",
            "selfRepairSummary",
            "Attempted selfRepairSummary must include a bounded positive attemptCount and a real stopReason.",
        ));
    }
}

fn validate_verification_results(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let intents = task
        .verification_intents
        .iter()
        .map(|intent| (intent.verification_id.as_str(), intent))
        .collect::<std::collections::BTreeMap<_, _>>();
    for verification in &result.verification_results {
        let Some(intent) = intents.get(verification.verification_id.as_str()) else {
            issues.push(issue(
                "TASK_RESULT_REF_INVALID",
                "verificationResults",
                "TaskResult verificationResults must reference task.verificationIntents.",
            ));
            continue;
        };
        if let Some(evidence_type) = verification.evidence_type {
            let evidence = verification_evidence_name(evidence_type);
            let allowed = intent
                .acceptable_evidence
                .iter()
                .any(|item| verification_evidence_name(*item) == evidence);
            if !allowed {
                issues.push(issue(
                    "INVALID_VERIFICATION_INTENT",
                    "verificationResults[].evidenceType",
                    "verificationResults[].evidenceType must be allowed by the matching verification intent.",
                ));
            }
        }
    }
    if matches!(result.status, TaskResultStatus::Completed) {
        for intent in &task.verification_intents {
            let passed = result.verification_results.iter().any(|verification| {
                verification.verification_id == intent.verification_id
                    && verification.status == "passed"
            });
            if !passed {
                issues.push(issue(
                    "TASK_RESULT_STATUS_INCONSISTENT",
                    "verificationResults",
                    "Completed TaskResult must include passed evidence for every verification intent.",
                ));
            }
        }
    }
    if !matches!(result.status, TaskResultStatus::Failed)
        && result
            .verification_results
            .iter()
            .any(|verification| verification.status == "failed")
    {
        issues.push(issue(
            "TASK_RESULT_STATUS_INCONSISTENT",
            "verificationResults",
            "Non-failed TaskResult must not contain failed verification results.",
        ));
    }
    if matches!(result.status, TaskResultStatus::CompletedWithNotes)
        && result.notes.is_empty()
        && result.verification_results.iter().any(|verification| {
            verification.status == "not_run" || verification.status == "inconclusive"
        })
    {
        issues.push(issue(
            "TASK_RESULT_STATUS_INCONSISTENT",
            "notes",
            "completed_with_notes TaskResult must explain not_run or inconclusive verification results.",
        ));
    }
}

fn validate_requirement_detail_evidence(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let required_detail_ids = required_requirement_detail_ids(task);
    if required_detail_ids.is_empty() {
        return;
    }
    let verification_ids = task
        .verification_intents
        .iter()
        .map(|intent| intent.verification_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for evidence in &result.requirement_detail_evidence {
        if !required_detail_ids.contains(&evidence.detail_id) {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence[].detailId",
                "Requirement detail evidence must reference details assigned to the task.",
            ));
        }
        if evidence.verification_ids.is_empty() {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence[].verificationIds",
                "Requirement detail evidence must link to verification results.",
            ));
        }
        for verification_id in &evidence.verification_ids {
            if !verification_ids.contains(verification_id.as_str()) {
                issues.push(issue(
                    "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                    "requirementDetailEvidence[].verificationIds",
                    "Requirement detail evidence verificationIds must reference task verification intents.",
                ));
            }
        }
    }
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        return;
    }
    for detail_id in &required_detail_ids {
        let Some(evidence) = result
            .requirement_detail_evidence
            .iter()
            .find(|evidence| &evidence.detail_id == detail_id)
        else {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence",
                "Completed TaskResult must include requirementDetailEvidence for every assigned detail.",
            ));
            continue;
        };
        if matches!(result.status, TaskResultStatus::Completed) && evidence.status != "satisfied" {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence[].status",
                "Completed TaskResult requirement detail evidence must be satisfied.",
            ));
        }
    }
}

fn required_requirement_detail_ids(task: &TaskDefinition) -> Vec<String> {
    let mut required_detail_ids = task.requirement_detail_refs.clone();
    for intent in &task.verification_intents {
        for detail_id in &intent.requirement_detail_refs {
            if !required_detail_ids.contains(detail_id) {
                required_detail_ids.push(detail_id.clone());
            }
        }
    }
    required_detail_ids
}

fn verification_ids_for_detail(task: &TaskDefinition, detail_id: &str) -> Vec<String> {
    task.verification_intents
        .iter()
        .filter(|intent| {
            intent
                .requirement_detail_refs
                .iter()
                .any(|id| id == detail_id)
        })
        .map(|intent| intent.verification_id.clone())
        .collect()
}

fn validate_concept_evidence(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if task.concept_refs.is_empty() {
        return;
    }
    for concept_ref in &task.concept_refs {
        if !result
            .concept_evidence
            .iter()
            .any(|evidence| &evidence.concept_ref == concept_ref)
        {
            issues.push(issue(
                "TASK_RESULT_REF_INVALID",
                "conceptEvidence",
                "TaskResult conceptEvidence must cover every task.conceptRefs entry.",
            ));
        }
    }
    for evidence in &result.concept_evidence {
        if !task.concept_refs.contains(&evidence.concept_ref) {
            issues.push(issue(
                "TASK_RESULT_REF_INVALID",
                "conceptEvidence[].conceptRef",
                "TaskResult conceptEvidence must not invent concept refs outside the task.",
            ));
        }
    }
}

fn validate_runtime_delivery_evidence(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(requirement) = &task.runtime_delivery_requirement else {
        return;
    };
    if !requirement.applies_to_this_task {
        return;
    }
    let Some(evidence) = &result.runtime_delivery_evidence else {
        issues.push(issue(
            "TASK_RESULT_REF_INVALID",
            "runtimeDeliveryEvidence",
            "TaskResult must include runtimeDeliveryEvidence when task.runtimeDeliveryRequirement applies.",
        ));
        return;
    };
    let checked_fields = string_array_at(evidence, "checkedFields");
    for field in &requirement.affected_contract_fields {
        if !checked_fields.contains(field) {
            issues.push(issue(
                "TASK_RESULT_REF_INVALID",
                "runtimeDeliveryEvidence.checkedFields",
                "runtimeDeliveryEvidence.checkedFields must include every affected runtime contract field.",
            ));
        }
    }
    let result_check_ids = object_array_string_field(evidence, "codeLevelChecks", "checkId");
    let allowed_check_ids = requirement
        .required_code_level_checks
        .iter()
        .map(|check| check.check_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for check_id in &result_check_ids {
        if !allowed_check_ids.contains(check_id) {
            issues.push(issue(
                "TASK_RESULT_RUNTIME_CHECK_ID_INVALID",
                "runtimeDeliveryEvidence.codeLevelChecks[].checkId",
                "runtimeDeliveryEvidence codeLevelChecks must use task runtime checkIds.",
            ));
        }
    }
    for check_id in allowed_check_ids {
        if !result_check_ids.contains(&check_id) {
            issues.push(issue(
                "TASK_RESULT_RUNTIME_CHECK_ID_INVALID",
                "runtimeDeliveryEvidence.codeLevelChecks",
                "runtimeDeliveryEvidence must include every required runtime code-level check.",
            ));
        }
    }
}

fn validate_frontend_experience_self_check(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if !frontend_self_check_applies(task) {
        return;
    }
    let Some(requirement) = &task.frontend_experience_requirement else {
        return;
    };
    let closure_ids = workflow_closure_requirement_ids(requirement);
    if closure_ids.is_empty()
        && !matches!(
            task.task_kind,
            TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
        )
    {
        return;
    }
    let Some(self_check) = &result.frontend_experience_self_check else {
        if matches!(
            result.status,
            TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
        ) {
            issues.push(issue(
                "TASK_RESULT_WORKFLOW_CLOSURE_INVALID",
                "frontendExperienceSelfCheck",
                "TaskResult must include frontendExperienceSelfCheck for frontend workflow tasks.",
            ));
        }
        return;
    };
    let covered = string_array_at(self_check, "closureRequirementIds");
    for closure_id in closure_ids {
        if !covered.contains(&closure_id) {
            issues.push(issue(
                "TASK_RESULT_WORKFLOW_CLOSURE_INVALID",
                "frontendExperienceSelfCheck.closureRequirementIds",
                "frontendExperienceSelfCheck must cover every required workflow closure id.",
            ));
        }
    }
    let status = self_check.get("status").and_then(Value::as_str);
    let data_binding = self_check.get("dataBinding").unwrap_or(&Value::Null);
    let binding_mode = data_binding.get("mode").and_then(Value::as_str);
    let known_gaps = data_binding
        .get("knownGaps")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    if status == Some("satisfied") && (binding_mode != Some("wired") || known_gaps > 0) {
        issues.push(issue(
            "TASK_RESULT_WORKFLOW_CLOSURE_INVALID",
            "frontendExperienceSelfCheck.dataBinding",
            "Satisfied frontendExperienceSelfCheck requires wired dataBinding and no known gaps.",
        ));
    }
}

fn validate_frontend_quality_self_check(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if !frontend_quality_self_check_applies(task) {
        return;
    }
    let ui_quality_contract = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiQualityContract"))
        .unwrap_or(&Value::Null);
    let Some(self_check_model) = &result.frontend_quality_self_check else {
        if matches!(
            result.status,
            TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
        ) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck",
                "TaskResult must include frontendQualitySelfCheck for frontend quality tasks.",
            ));
        }
        return;
    };
    let self_check = serde_json::to_value(self_check_model).unwrap_or(Value::Null);
    if self_check.get("scenarioKind").and_then(Value::as_str)
        != ui_quality_contract
            .pointer("/scenario/kind")
            .and_then(Value::as_str)
    {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.scenarioKind",
            "frontendQualitySelfCheck.scenarioKind must match uiQualityContract.scenario.kind.",
        ));
    }
    if self_check.get("qualityLevel").and_then(Value::as_str)
        != ui_quality_contract
            .get("qualityLevel")
            .and_then(Value::as_str)
    {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.qualityLevel",
            "frontendQualitySelfCheck.qualityLevel must match uiQualityContract.qualityLevel.",
        ));
    }
    let checked_refs = string_array_at(&self_check, "referenceIdsChecked");
    for reference_id in string_array_at(
        ui_quality_contract
            .get("referenceProfile")
            .unwrap_or(&Value::Null),
        "referenceIds",
    ) {
        if !checked_refs.contains(&reference_id) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.referenceIdsChecked",
                "frontendQualitySelfCheck must cover every uiQualityContract reference id.",
            ));
            break;
        }
    }
    let covered_states = self_check
        .get("statesCovered")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("state").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for state in ui_quality_contract
        .get("requiredUiStates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("state").and_then(Value::as_str))
    {
        if !covered_states.iter().any(|item| item == state) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.statesCovered",
                "frontendQualitySelfCheck must cover every uiQualityContract required UI state.",
            ));
            break;
        }
    }
    let checked_rules = self_check
        .get("businessUiRulesChecked")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("ruleId").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for rule_id in ui_quality_contract
        .get("businessUiRules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("ruleId").and_then(Value::as_str))
    {
        if !checked_rules.iter().any(|item| item == rule_id) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.businessUiRulesChecked",
                "frontendQualitySelfCheck must cover every uiQualityContract business UI rule.",
            ));
            break;
        }
    }
    validate_design_token_evidence(&self_check, ui_quality_contract, issues);
    if self_check.get("status").and_then(Value::as_str) == Some("satisfied") {
        let violations = self_check
            .pointer("/forbiddenContentCheck/violations")
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0);
        let gaps = self_check
            .get("knownGaps")
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0);
        if violations > 0 || gaps > 0 {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.status",
                "Satisfied frontendQualitySelfCheck cannot contain forbidden content violations or known gaps.",
            ));
        }
    }
}

fn validate_design_token_evidence(
    self_check: &Value,
    ui_quality_contract: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let plan = ui_quality_contract
        .get("designTokenAssetPlan")
        .unwrap_or(&Value::Null);
    let strategy = plan
        .get("strategy")
        .and_then(Value::as_str)
        .unwrap_or("not_applicable");
    let Some(evidence) = self_check.get("designTokenEvidence") else {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.designTokenEvidence",
            "frontendQualitySelfCheck must include designTokenEvidence for UI quality tasks.",
        ));
        return;
    };
    if evidence.get("strategyUsed").and_then(Value::as_str) != Some(strategy) {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.designTokenEvidence.strategyUsed",
            "designTokenEvidence.strategyUsed must match uiQualityContract.designTokenAssetPlan.strategy.",
        ));
    }
    let expected_template = plan.get("templateId").unwrap_or(&Value::Null);
    let actual_template = evidence.get("templateIdUsed").unwrap_or(&Value::Null);
    if expected_template != actual_template {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.designTokenEvidence.templateIdUsed",
            "designTokenEvidence.templateIdUsed must match uiQualityContract.designTokenAssetPlan.templateId.",
        ));
    }
    let satisfied = self_check.get("status").and_then(Value::as_str) == Some("satisfied");
    if satisfied
        && evidence
            .get("parallelTokenSystemCreated")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.designTokenEvidence.parallelTokenSystemCreated",
            "Satisfied frontendQualitySelfCheck cannot create a parallel token system.",
        ));
    }
    if satisfied && strategy != "not_applicable" {
        if evidence
            .get("tokenAssetFiles")
            .and_then(Value::as_array)
            .map(|items| items.is_empty())
            .unwrap_or(true)
        {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.designTokenEvidence.tokenAssetFiles",
                "Satisfied frontendQualitySelfCheck requires tokenAssetFiles for the active designTokenAssetPlan.",
            ));
        }
        if evidence
            .get("mergeSummary")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.designTokenEvidence.mergeSummary",
                "Satisfied frontendQualitySelfCheck requires mergeSummary explaining how token assets were reused, extended, or created.",
            ));
        }
    }
}

fn validate_blocked_reasons(
    result: &TaskResult,
    blocked_output: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if !matches!(result.status, TaskResultStatus::Blocked) {
        if !result.blocked_reasons.is_empty() {
            issues.push(issue(
                "TASK_RESULT_BLOCKED_MAPPING_INVALID",
                "blockedReasons",
                "Only blocked TaskResult may include blockedReasons.",
            ));
        }
        return;
    }
    if result.blocked_reasons.is_empty() {
        issues.push(issue(
            "TASK_RESULT_BLOCKED_MAPPING_INVALID",
            "blockedReasons",
            "Blocked TaskResult must include at least one blocked reason.",
        ));
        return;
    }
    let allowed = blocked_output
        .get("blockedReasons")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("code")?.as_str()?.to_string(),
                item.get("nextNode")?.as_str()?.to_string(),
            ))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for blocked in &result.blocked_reasons {
        if allowed.get(&blocked.code) != Some(&blocked.next_node) {
            issues.push(issue(
                "TASK_RESULT_BLOCKED_MAPPING_INVALID",
                "blockedReasons",
                "Blocked reason must match the request blocked reason options.",
            ));
        }
    }
}

fn allows_empty_changed_files(task: &TaskDefinition, result: &TaskResult) -> bool {
    if matches!(task.task_kind, TaskKind::VerificationIncrement) {
        return true;
    }
    matches!(
        result
            .no_change_reason
            .as_ref()
            .map(|reason| reason.code.as_str()),
        Some("NO_CODE_CHANGE_REQUIRED")
            | Some("VERIFICATION_ONLY_TASK")
            | Some("ENVIRONMENT_CHECK_ONLY")
    )
}

fn is_safe_project_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.contains('\\')
        && !path.contains('\0')
        && !path.contains(':')
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !path.starts_with(".loom/")
        && !path.starts_with(".git/")
        && !path.starts_with("node_modules/")
}

fn workflow_closure_requirement_ids(requirement: &Value) -> Vec<String> {
    let Some(guidance) = requirement.get("executionGuidance") else {
        return vec![];
    };
    if let Some(items) = guidance
        .get("closureRequirementRefs")
        .and_then(Value::as_array)
    {
        return items
            .iter()
            .filter_map(|item| {
                item.as_str().map(str::to_string).or_else(|| {
                    item.get("closureId")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            })
            .collect();
    }
    guidance
        .get("workflowClosureRequirements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("closureId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn string_array_at(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn object_array_string_field(
    value: &Value,
    array_field: &str,
    string_field: &str,
) -> std::collections::BTreeSet<String> {
    value
        .get(array_field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get(string_field)
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn verification_evidence_name(evidence: VerificationEvidence) -> &'static str {
    match evidence {
        VerificationEvidence::AutomatedTest => "automated_test",
        VerificationEvidence::ManualCommandOutput => "manual_command_output",
        VerificationEvidence::RuntimeApiCheck => "runtime_api_check",
        VerificationEvidence::StaticCheck => "static_check",
        VerificationEvidence::AgentReviewExplanation => "agent_review_explanation",
    }
}

fn next_action_for_run(run: &contracts::TaskPlanRun) -> Option<TaskPlanRunNextAction> {
    match run.status {
        TaskPlanRunStatus::Running | TaskPlanRunStatus::NotStarted => Some(TaskPlanRunNextAction {
            r#type: "continue_execution".to_string(),
            reason: "NEXT_TASK_READY".to_string(),
            source_task_id: None,
            target_node: "task_execution".to_string(),
        }),
        TaskPlanRunStatus::Completed | TaskPlanRunStatus::CompletedWithNotes => {
            Some(TaskPlanRunNextAction {
                r#type: "review".to_string(),
                reason: "RUN_READY_FOR_REVIEW".to_string(),
                source_task_id: None,
                target_node: "review".to_string(),
            })
        }
        TaskPlanRunStatus::Blocked => Some(TaskPlanRunNextAction {
            r#type: "execution_repair".to_string(),
            reason: "TASK_BLOCKED".to_string(),
            source_task_id: None,
            target_node: "execution_repair".to_string(),
        }),
        TaskPlanRunStatus::Failed => Some(TaskPlanRunNextAction {
            r#type: "execution_repair".to_string(),
            reason: "TASK_FAILED".to_string(),
            source_task_id: None,
            target_node: "execution_repair".to_string(),
        }),
    }
}

fn route_action_for_task_result(
    run: &contracts::TaskPlanRun,
    result: &TaskResult,
    result_ref: &str,
) -> Result<Option<RouteAction>, state::store::StateError> {
    match run.status {
        TaskPlanRunStatus::Running | TaskPlanRunStatus::NotStarted => Ok(Some(RouteAction {
            kind: RouteActionKind::ContinueExecution,
            source: "task_result".to_string(),
            reason: run
                .next_action
                .as_ref()
                .map(|action| action.reason.clone())
                .unwrap_or_else(|| "NEXT_TASK_READY".to_string()),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(result_ref.to_string()),
            details: Some(task_result_route_details(run)),
            target_phase_id: None,
        })),
        TaskPlanRunStatus::Completed | TaskPlanRunStatus::CompletedWithNotes => {
            Ok(Some(RouteAction {
                kind: RouteActionKind::Review,
                source: "task_result".to_string(),
                reason: run
                    .next_action
                    .as_ref()
                    .map(|action| action.reason.clone())
                    .unwrap_or_else(|| "RUN_READY_FOR_REVIEW".to_string()),
                prompt: None,
                accepted_responses: vec![],
                request_ref: Some(result_ref.to_string()),
                details: Some(task_result_route_details(run)),
                target_phase_id: None,
            }))
        }
        TaskPlanRunStatus::Failed => {
            let attempt_count = run
                .task_states
                .iter()
                .find(|state| state.task_id == result.task_id)
                .map(|state| state.attempts.len())
                .unwrap_or(0);
            if attempt_count <= 3 {
                Ok(Some(RouteAction {
                    kind: RouteActionKind::ExecutionRepair,
                    source: "task_result".to_string(),
                    reason: "TASK_FAILED".to_string(),
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: Some(result_ref.to_string()),
                    details: Some(task_result_route_details(run)),
                    target_phase_id: None,
                }))
            } else {
                Ok(Some(RouteAction {
                    kind: RouteActionKind::Review,
                    source: "task_result".to_string(),
                    reason: "TASK_FAILED_RETRY_LIMIT_REACHED".to_string(),
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: Some(result_ref.to_string()),
                    details: Some(task_result_route_details(run)),
                    target_phase_id: None,
                }))
            }
        }
        TaskPlanRunStatus::Blocked => route_blocked_task_result(result, result_ref),
    }
}

fn route_blocked_task_result(
    result: &TaskResult,
    result_ref: &str,
) -> Result<Option<RouteAction>, state::store::StateError> {
    let next_node = result
        .blocked_reasons
        .iter()
        .map(|reason| reason.next_node.as_str())
        .find(|node| !node.is_empty())
        .unwrap_or("needs_user_decision");
    match next_node {
        "taskplan_repair" => Ok(Some(RouteAction {
            kind: RouteActionKind::TaskplanRepair,
            source: "task_result".to_string(),
            reason: "TASK_BLOCKED".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(result_ref.to_string()),
            details: Some(blocked_task_route_details(result)),
            target_phase_id: None,
        })),
        "architecture_artifact_repair" => Ok(Some(RouteAction {
            kind: RouteActionKind::ArchitectureArtifactRepair,
            source: "task_result".to_string(),
            reason: "TASK_BLOCKED".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(result_ref.to_string()),
            details: Some(blocked_task_route_details(result)),
            target_phase_id: None,
        })),
        "execution_repair" => Ok(None),
        _ => Ok(Some(RouteAction {
            kind: RouteActionKind::NeedsUserDecision,
            source: "task_result".to_string(),
            reason: "TASK_BLOCKED".to_string(),
            prompt: Some(
                result
                    .blocked_reasons
                    .first()
                    .map(|reason| reason.message.clone())
                    .unwrap_or_else(|| {
                        "TaskResult is blocked and requires user decision.".to_string()
                    }),
            ),
            accepted_responses: vec!["confirm".to_string(), "request_changes".to_string()],
            request_ref: Some(result_ref.to_string()),
            details: Some(blocked_task_route_details(result)),
            target_phase_id: None,
        })),
    }
}

fn task_result_route_details(run: &contracts::TaskPlanRun) -> Value {
    json!({
        "taskPlanRunId": run.run_id,
        "runStatus": run.status,
        "summary": run.summary
    })
}

fn blocked_task_route_details(result: &TaskResult) -> Value {
    json!({
        "kind": "task_result_blocked",
        "taskResultId": result.task_result_id,
        "blockedReasons": result.blocked_reasons
    })
}

struct RepairContextInput {
    task_plan_id: String,
    task_id: String,
    run_id: String,
    task: TaskDefinition,
    result_file: String,
    required_top_level_fields: Vec<String>,
    blocked_output: Value,
    submitted_result: Value,
    previous_changed_files: Vec<String>,
}

fn repair_task_result_or_error(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
    repair_submit: bool,
    context: Option<RepairContextInput>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    if repair_submit {
        return Ok(repairable_with_tool(
            input,
            authorized,
            target_file,
            issues,
            "loom.repairSubmitFile",
            "task_result_repair_candidate_only",
        ));
    }
    let Some(context) = context else {
        return Ok(repairable(input, authorized, target_file, issues));
    };
    materialize_task_result_repair(input, authorized, target_file, issues, context)
}

fn materialize_task_result_repair(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
    context: RepairContextInput,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskResult repair action missing deliveryId".to_string(),
        )
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskResult repair action missing phaseId".to_string(),
        )
    })?;
    let root = Path::new(&input.project_root);
    let request_id = format!(
        "task_result_repair_{}_{}",
        safe_id(&context.task_id),
        state::store::now_millis()
    );
    let request_file = to_project_relative(
        root,
        &state::paths::delivery_dir(root, &delivery_id)
            .join("repairs")
            .join(&phase_id)
            .join("requests")
            .join(format!("{request_id}.json")),
    )?;
    let schema_shape = serde_json::to_value(schema_for!(TaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let result_template = task_result_repair_template(&context, &issues);
    let mut context_fields = vec![
        "source.taskPlanId",
        "source.taskId",
        "source.taskPlanRunId",
        "source.taskExecutionRequestRef",
        "source.originalResultFile",
        "task.taskId",
        "task.groupId",
        "task.title",
        "task.taskKind",
        "task.objective",
        "task.acceptanceRefs",
        "task.requirementDetailRefs",
        "task.verificationIntents",
        "outputContract.blockedReasonOptions",
        "repairContract.profile",
        "repairContract.issueConflicts",
        "repairContract.minimalRepairRules",
    ];
    if !context.task.concept_refs.is_empty() {
        context_fields.push("task.conceptRefs");
    }
    if frontend_self_check_applies(&context.task) {
        context_fields
            .push("task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs");
    }
    if frontend_quality_self_check_applies(&context.task) {
        context_fields.extend([
            "task.frontendExperienceRequirement.executionGuidance.uiQuality",
            "task.frontendExperienceRequirement.uiQualityContractRef",
        ]);
        context_fields.extend(FRONTEND_QUALITY_CONTRACT_READ_FIELDS);
    }
    if runtime_delivery_evidence_applies(&context.task) {
        context_fields.extend(runtime_delivery_requirement_read_fields(&context.task));
    }
    let mut write_contract_fields = vec![
        "outputContract.resultFile",
        "outputContract.writeTargets",
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
        "outputContract.schemaShape.properties.blockedReasons",
        "outputContract.resultRules",
    ];
    if frontend_self_check_applies(&context.task) {
        write_contract_fields
            .push("outputContract.schemaShape.properties.frontendExperienceSelfCheck");
    }
    if frontend_quality_self_check_applies(&context.task) {
        write_contract_fields
            .push("outputContract.schemaShape.properties.frontendQualitySelfCheck");
    }
    if runtime_delivery_evidence_applies(&context.task) {
        write_contract_fields.push("outputContract.schemaShape.properties.runtimeDeliveryEvidence");
    }
    if !context.task.concept_refs.is_empty() {
        write_contract_fields.push("outputContract.schemaShape.properties.conceptEvidence");
    }
    let root_value = json!({
        "schemaVersion": "1.0",
        "requestType": "task_result_repair",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::TaskResultRepair,
        "source": {
            "taskExecutionRequestRef": input.request_ref,
            "taskPlanId": context.task_plan_id,
            "taskId": context.task_id,
            "taskPlanRunId": context.run_id,
            "originalResultFile": target_file
        },
        "task": context.task.clone(),
        "repairContract": {
            "profile": "minimal_task_result_repair",
            "issueConflicts": task_result_issue_conflicts(&context, &issues),
            "minimalRepairRules": task_result_minimal_repair_rules(&issues)
        },
        "outputContract": {
            "artifactKind": ArtifactKind::TaskResultRepair,
            "writeMode": "single_json",
            "submitTool": "loom.repairSubmitFile",
            "resultFile": context.result_file,
            "writeTargets": [{
                "targetId": "result",
                "path": context.result_file,
                "required": true,
                "description": "Rewrite the TaskResult JSON for the original task execution request."
            }],
            "requiredTopLevelFields": context.required_top_level_fields,
            "blockedReasonOptions": context.blocked_output
                .get("blockedReasons")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "schemaShape": schema_shape,
            "resultTemplate": result_template,
            "resultRules": [
                "The replacement must be a TaskResult JSON, not a repair summary.",
                "Runtime, frontend, requirement detail, and concept evidence must follow the original output contract."
            ]
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "task_result_repair_context",
                    "required": true,
                    "purpose": "Read the original TaskResult validation issues and task contract.",
                    "whenToRead": "Read before rewriting TaskResult.",
                    "fields": context_fields
                },
                {
                    "groupId": "task_result_repair_write_contract",
                    "required": true,
                    "purpose": "Read the TaskResult replacement output contract.",
                    "whenToRead": "Read before writing replacement TaskResult.",
                    "fields": write_contract_fields
                }
            ]
        }
    });
    let stored = state::write_native_request(
        &input.project_root,
        state::NativeRequestInput {
            request_id,
            request_kind: "task_result_repair".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.clone()),
            phase_id: Some(phase_id.clone()),
            root: root_value,
        },
    )?;
    update_latest_task_result_repair_action(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &stored.request_ref,
    )?;
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: input.project_root.clone(),
        request_ref: stored.request_ref.clone(),
    })?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            input.project_root.clone(),
            LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
                artifact_kind: ArtifactKind::TaskResultRepair,
                request_ref: stored.request_ref,
                write_mode: WriteMode::SingleJson,
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

fn previous_persisted_changed_files(
    root: &Path,
    locator: &DeliveryPhaseLocator,
    run_id: &str,
    result: &TaskResult,
) -> Vec<String> {
    let path = task_result_file(
        root,
        locator,
        run_id,
        &result.task_id,
        &result.task_result_id,
    );
    if !path.exists() {
        return Vec::new();
    }
    state::store::read_json::<TaskResult>(&path)
        .map(|previous| previous.changed_files)
        .unwrap_or_default()
}

fn task_result_issue_conflicts(
    context: &RepairContextInput,
    issues: &[delivery_core::RepairIssue],
) -> Vec<Value> {
    issues
        .iter()
        .map(|issue| {
            let base = json!({
                "code": issue.code,
                "fieldPath": issue.field_path,
                "message": issue.message
            });
            if issue.code == "TASK_RESULT_WORKFLOW_CLOSURE_INVALID" {
                return task_result_workflow_conflict(context, base);
            }
            if issue.code == "TASK_RESULT_FRONTEND_QUALITY_INVALID" {
                return task_result_frontend_quality_conflict(context, base);
            }
            if issue.code == "TASK_RESULT_RUNTIME_CHECK_ID_INVALID" {
                return task_result_runtime_conflict(context, base);
            }
            base
        })
        .collect()
}

fn task_result_workflow_conflict(context: &RepairContextInput, mut base: Value) -> Value {
    let self_check = context
        .submitted_result
        .get("frontendExperienceSelfCheck")
        .cloned()
        .unwrap_or(Value::Null);
    let data_binding = self_check
        .get("dataBinding")
        .cloned()
        .unwrap_or(Value::Null);
    let known_gaps_count = data_binding
        .get("knownGaps")
        .and_then(Value::as_array)
        .map(|items| items.len());
    let expected_closure_ids = context
        .task
        .frontend_experience_requirement
        .as_ref()
        .map(workflow_closure_requirement_ids)
        .unwrap_or_default();
    base["current"] = json!({
        "frontendExperienceSelfCheckStatus": self_check.get("status").and_then(Value::as_str),
        "dataBindingMode": data_binding.get("mode").and_then(Value::as_str),
        "knownGapsCount": known_gaps_count
    });
    base["expectedForSatisfied"] = json!({
        "frontendExperienceSelfCheckStatus": "satisfied",
        "dataBindingMode": "wired",
        "knownGaps": [],
        "closureRequirementIds": expected_closure_ids
    });
    base["validRepairChoices"] = json!([
        "If the implementation and evidence are actually wired, repair frontendExperienceSelfCheck.dataBinding.mode to wired, clear knownGaps, and cite evidence.",
        "If wired evidence is missing, do not claim satisfied; report the remaining gap through frontendExperienceSelfCheck and the normal TaskResult status."
    ]);
    base
}

fn task_result_frontend_quality_conflict(context: &RepairContextInput, mut base: Value) -> Value {
    let ui_quality_contract = context
        .task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiQualityContract"))
        .cloned()
        .unwrap_or(Value::Null);
    let self_check = context
        .submitted_result
        .get("frontendQualitySelfCheck")
        .cloned()
        .unwrap_or(Value::Null);
    base["current"] = json!({
        "status": self_check.get("status").and_then(Value::as_str),
        "scenarioKind": self_check.get("scenarioKind").and_then(Value::as_str),
        "qualityLevel": self_check.get("qualityLevel").and_then(Value::as_str),
        "referenceIdsChecked": string_array_at(&self_check, "referenceIdsChecked"),
        "designTokenEvidence": self_check.get("designTokenEvidence").cloned().unwrap_or(Value::Null),
        "knownGapsCount": self_check
            .get("knownGaps")
            .and_then(Value::as_array)
            .map(|items| items.len())
    });
    base["expected"] = json!({
        "scenarioKind": ui_quality_contract.pointer("/scenario/kind").and_then(Value::as_str),
        "qualityLevel": ui_quality_contract.get("qualityLevel").and_then(Value::as_str),
        "referenceIds": string_array_at(
            ui_quality_contract
                .get("referenceProfile")
                .unwrap_or(&Value::Null),
            "referenceIds"
        ),
        "requiredUiStates": ui_quality_contract.get("requiredUiStates").cloned().unwrap_or_else(|| json!([])),
        "businessUiRules": ui_quality_contract.get("businessUiRules").cloned().unwrap_or_else(|| json!([])),
        "designTokenAssetPlan": ui_quality_contract
            .get("designTokenAssetPlan")
            .cloned()
            .unwrap_or(Value::Null),
        "forbiddenUserVisibleContent": ui_quality_contract
            .get("forbiddenUserVisibleContent")
            .cloned()
            .unwrap_or_else(|| json!([]))
    });
    base["validRepairChoices"] = json!([
        "If the implemented UI satisfies the contract, repair frontendQualitySelfCheck to match task.frontendExperienceRequirement.uiQualityContract and cite evidence.",
        "If the UI still has quality gaps, keep status below satisfied and record the specific gaps without claiming completion."
    ]);
    base
}

fn task_result_runtime_conflict(context: &RepairContextInput, mut base: Value) -> Value {
    let required_check_ids = context
        .task
        .runtime_delivery_requirement
        .as_ref()
        .map(|requirement| {
            requirement
                .required_code_level_checks
                .iter()
                .map(|check| check.check_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let submitted_check_ids = context
        .submitted_result
        .get("runtimeDeliveryEvidence")
        .map(|evidence| object_array_string_field(evidence, "codeLevelChecks", "checkId"))
        .unwrap_or_default();
    base["current"] = json!({
        "runtimeCheckIds": submitted_check_ids
    });
    base["expectedRuntimeCheckIds"] = json!(required_check_ids);
    base["validRepairChoices"] = json!([
        "Use exactly the task.runtimeDeliveryRequirement.requiredCodeLevelChecks[].checkId values.",
        "If a code-level check does not apply, record it with status not_applicable and a non-empty reason."
    ]);
    base
}

fn task_result_minimal_repair_rules(issues: &[delivery_core::RepairIssue]) -> Vec<&'static str> {
    let mut rules = vec![
        "Repair the same TaskResult JSON file only.",
        "Do not edit project source files for TaskResult contract repair.",
        "Use exact verificationResults[].verificationId values from task.verificationIntents.",
        "Never combine selfRepairSummary.attempted=false with stopReason verification_passed.",
    ];
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_WORKFLOW_CLOSURE_INVALID")
    {
        rules.push("frontendExperienceSelfCheck.status=satisfied is valid only when dataBinding.mode=wired and knownGaps is empty.");
        rules.push("If wired evidence is missing, do not claim satisfied; report the remaining gap through frontendExperienceSelfCheck and TaskResult status.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_RUNTIME_CHECK_ID_INVALID")
    {
        rules.push("RuntimeDeliveryEvidence codeLevelChecks must use only required check ids from the request.");
        rules.push("For passed runtime checks, omit reason; use a non-empty reason only for failed, blocked, or not_applicable checks.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_FRONTEND_QUALITY_INVALID")
    {
        rules.push("frontendQualitySelfCheck must match the task uiQualityContract scenario, quality level, references, required states, and business UI rules.");
        rules.push("frontendQualitySelfCheck.status=satisfied is valid only when forbidden content violations and knownGaps are empty.");
    }
    rules
}

fn task_result_repair_template(
    context: &RepairContextInput,
    issues: &[delivery_core::RepairIssue],
) -> Value {
    let mut template = task_result_template(&context.task_plan_id, &context.task);
    merge_submitted_task_result_fields(&mut template, &context.submitted_result, issues);
    if changed_files_issue(issues)
        && context
            .previous_changed_files
            .iter()
            .any(|path| !path.is_empty())
        && template
            .get("changedFiles")
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty())
    {
        template["changedFiles"] = json!(context.previous_changed_files);
        template["noChangeReason"] = Value::Null;
    }
    template
}

fn merge_submitted_task_result_fields(
    template: &mut Value,
    submitted: &Value,
    issues: &[delivery_core::RepairIssue],
) {
    let (Some(template_object), Some(submitted_object)) =
        (template.as_object_mut(), submitted.as_object())
    else {
        return;
    };
    let conflicted_fields = issue_top_level_fields(issues);
    for (key, submitted_value) in submitted_object {
        if conflicted_fields.contains(key.as_str()) {
            continue;
        }
        if !template_object.contains_key(key) {
            continue;
        }
        if keeps_template_array_shape(template_object.get(key), submitted_value) {
            continue;
        }
        template_object.insert(key.clone(), submitted_value.clone());
    }
}

fn issue_top_level_fields(issues: &[delivery_core::RepairIssue]) -> BTreeSet<&str> {
    issues
        .iter()
        .filter_map(|issue| issue.field_path.as_deref())
        .filter_map(|path| path.split(['.', '[']).next())
        .filter(|field| !field.is_empty())
        .collect()
}

fn keeps_template_array_shape(template_value: Option<&Value>, submitted_value: &Value) -> bool {
    matches!(
        (template_value.and_then(Value::as_array), submitted_value.as_array()),
        (Some(template_items), Some(submitted_items))
            if !template_items.is_empty() && submitted_items.is_empty()
    )
}

fn changed_files_issue(issues: &[delivery_core::RepairIssue]) -> bool {
    issues.iter().any(|issue| {
        issue.field_path.as_deref() == Some("changedFiles")
            || issue.code == "TASK_RESULT_STATUS_INCONSISTENT"
    })
}

fn update_latest_task_result_repair_action(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
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
            "activeTaskResultRepairActionRef".to_string(),
            request_ref.to_string(),
        );
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::TaskResultRepair,
            source: "task_result_validation".to_string(),
            reason: "task_result_contract_invalid".to_string(),
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

fn update_delivery_after_result(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    run: &contracts::TaskPlanRun,
    result_ref: &str,
    next_action: Option<&RouteAction>,
) -> Result<(), state::store::StateError> {
    let store = FileTransitionStore;
    let mut status = store.load_status(project_root).map_err(to_state_error)?;
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        phase
            .latest_refs
            .insert("latestTaskResult".to_string(), result_ref.to_string());
        phase.latest_refs.remove("activeTaskResultRepairActionRef");
        phase.next_action = next_action.cloned();
    }
    delivery.status = if matches!(
        run.status,
        TaskPlanRunStatus::Running | TaskPlanRunStatus::NotStarted
    ) {
        DeliveryLifecycleStatus::Executing
    } else {
        DeliveryLifecycleStatus::Reviewing
    };
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)?;
    Ok(())
}

fn ensure_latest_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
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
        return Ok(Some(failed(
            project_root,
            "STALE_TASK_EXECUTION_REQUEST",
            "TaskExecution submit phase does not exist.".to_string(),
            "record_task_result",
        )));
    };
    if phase
        .latest_refs
        .get("taskExecutionRequestRef")
        .map(String::as_str)
        != Some(request_ref)
    {
        return Ok(Some(failed(
            project_root,
            "STALE_TASK_EXECUTION_REQUEST",
            "TaskResult submit must use the active phase latest TaskExecution requestRef."
                .to_string(),
            "record_task_result",
        )));
    }
    Ok(None)
}

fn ensure_latest_task_result_repair_action(
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
        .and_then(|phase| phase.latest_refs.get("activeTaskResultRepairActionRef"))
        .map(String::as_str);
    if latest != Some(request_ref) {
        return Ok(Some(failed(
            project_root,
            "STALE_TASK_RESULT_REPAIR_ACTION",
            "TaskResult repair submit must use the active phase task result repair action requestRef."
                .to_string(),
            "task_result_repair_submit",
        )));
    }
    Ok(None)
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

fn issue(code: &str, field_path: &str, message: &str) -> delivery_core::RepairIssue {
    delivery_core::RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("result".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

fn repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
) -> LoomMcpActionResult {
    repairable_with_tool(
        input,
        authorized,
        target_file,
        issues,
        "loom.recordTaskResultFile",
        "task_result_candidate_only",
    )
}

fn repairable_with_tool(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
    resubmit_tool: &str,
    fix_scope: &str,
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
        resubmit_tool: resubmit_tool.to_string(),
        fix_scope: Some(fix_scope.to_string()),
        read_groups: authorized.read_groups.clone(),
    })
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
            .unwrap_or("Write TaskResult.")
            .to_string(),
    })
}

fn read_request_fields_chunked(
    project_root: &str,
    request_ref: &str,
    mut fields: Vec<String>,
) -> Result<BTreeMap<String, delivery_core::FieldReadResult>, state::store::StateError> {
    let mut seen = BTreeSet::new();
    fields.retain(|field| seen.insert(field.clone()));
    let mut merged = BTreeMap::new();
    for chunk in fields.chunks(20) {
        let read = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
            project_root: project_root.to_string(),
            request_ref: request_ref.to_string(),
            fields: chunk.to_vec(),
        })?;
        merged.extend(read.fields);
    }
    Ok(merged)
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

fn string_field(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
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

fn string_vec_field(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    name: &str,
) -> Result<Vec<String>, state::store::StateError> {
    fields
        .get(name)
        .and_then(|field| field.value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!("missing request field {name}"))
        })
}

fn value_field(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    name: &str,
) -> Value {
    fields
        .get(name)
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null)
}

fn array_field(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    name: &str,
) -> Value {
    fields
        .get(name)
        .map(|field| field.value.clone())
        .filter(Value::is_array)
        .unwrap_or_else(|| json!([]))
}

fn runtime_delivery_requirement_from_fields(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Value {
    let whole = value_field(fields, "task.runtimeDeliveryRequirement");
    if !whole.is_null() {
        return whole;
    }
    if !fields.contains_key("task.runtimeDeliveryRequirement.appliesToThisTask") {
        return Value::Null;
    }
    let mut requirement = json!({
        "appliesToThisTask": value_field(fields, "task.runtimeDeliveryRequirement.appliesToThisTask"),
        "reason": value_field(fields, "task.runtimeDeliveryRequirement.reason"),
        "affectedContractFields": array_field(fields, "task.runtimeDeliveryRequirement.affectedContractFields"),
        "requiredCodeLevelChecks": array_field(fields, "task.runtimeDeliveryRequirement.requiredCodeLevelChecks"),
        "evidenceExpectedInTaskResult": array_field(fields, "task.runtimeDeliveryRequirement.evidenceExpectedInTaskResult"),
        "forbiddenActions": array_field(fields, "task.runtimeDeliveryRequirement.forbiddenActions")
    });
    for (field, key) in [
        (
            "task.runtimeDeliveryRequirement.runtimeDeliveryRef",
            "runtimeDeliveryRef",
        ),
        ("task.runtimeDeliveryRequirement.source", "source"),
        (
            "task.runtimeDeliveryRequirement.deploymentFailureRef",
            "deploymentFailureRef",
        ),
    ] {
        let value = value_field(fields, field);
        if !value.is_null() {
            requirement[key] = value;
        }
    }
    requirement
}

fn frontend_experience_requirement_from_fields(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Value {
    let has_closure = fields.contains_key(
        "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
    );
    let has_ui_quality =
        fields.contains_key("task.frontendExperienceRequirement.uiQualityContract.scenario");
    if !has_closure && !has_ui_quality {
        return Value::Null;
    }
    let mut requirement = json!({
        "executionGuidance": {}
    });
    if has_closure {
        requirement["executionGuidance"]["closureRequirementRefs"] = array_field(
            fields,
            "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
        );
    }
    let ui_quality_guidance = value_field(
        fields,
        "task.frontendExperienceRequirement.executionGuidance.uiQuality",
    );
    if !ui_quality_guidance.is_null() {
        requirement["executionGuidance"]["uiQuality"] = ui_quality_guidance;
    }
    let ui_quality_ref = value_field(
        fields,
        "task.frontendExperienceRequirement.uiQualityContractRef",
    );
    if !ui_quality_ref.is_null() {
        requirement["uiQualityContractRef"] = ui_quality_ref;
    }
    if has_ui_quality {
        requirement["uiQualityContract"] = json!({
            "scenario": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.scenario"),
            "qualityLevel": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.qualityLevel"),
            "surfacePolicy": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.surfacePolicy"),
            "layoutBaseline": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.layoutBaseline"),
            "density": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.density"),
            "semanticTokenPolicy": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.semanticTokenPolicy"),
            "referenceProfile": {
                "referenceIds": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceIds"),
                "loadMode": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.loadMode")
            },
            "designTokenAssetPlan": {
                "strategy": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.strategy"),
                "templateId": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"),
                "targetFiles": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.targetFiles"),
                "existingStyleEvidence": {
                    "tailwindConfigRefs": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.tailwindConfigRefs"),
                    "tokenFileRefs": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.tokenFileRefs"),
                    "globalStyleRefs": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.globalStyleRefs"),
                    "componentThemeRefs": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.componentThemeRefs"),
                    "summary": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.summary")
                },
                "mergePolicy": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.mergePolicy"),
                "duplicationPolicy": value_field(fields, "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.duplicationPolicy")
            },
            "requiredUiStates": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.requiredUiStates"),
            "businessUiRules": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.businessUiRules"),
            "forbiddenUserVisibleContent": array_field(fields, "task.frontendExperienceRequirement.uiQualityContract.forbiddenUserVisibleContent")
        });
    }
    requirement
}

fn read_project_json_value(
    project_root: &Path,
    relative: &str,
) -> Result<Value, state::store::StateError> {
    let path = from_project_relative(project_root, relative)?;
    state::store::read_json_value(&path)
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}
