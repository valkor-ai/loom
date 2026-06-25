use std::path::Path;

use contracts::{
    TaskDefinition, TaskKind, TaskPlanRunNextAction, TaskPlanRunStatus, TaskResult,
    TaskResultStatus, TaskRunStatus, VerificationEvidence,
};
use delivery_core::{
    apply_delivery_index, DeliveryLifecycleStatus, FileSubmitInput, LoomMcpActionResult,
    LoomMcpFailure, LoomMcpFailureResult, LoomMcpRepairableErrorResult, RouteAction,
    RouteActionKind, TransitionStore,
};
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
};

use crate::{
    paths::task_result_file,
    task_execution::{continue_execution, load_current_plan_and_run, save_run},
    task_plan::update_run_summary,
};

pub fn accept_task_result_file(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
) -> LoomMcpActionResult {
    match accept_task_result_file_inner(input, authorized) {
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

fn accept_task_result_file_inner(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
) -> Result<LoomMcpActionResult, state::store::StateError> {
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
    if let Some(stale) = ensure_latest_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
    )? {
        return Ok(stale);
    }
    let target = authorized.targets.first().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "TaskResult submit requires a result target.".to_string(),
        )
    })?;
    let root = Path::new(&input.project_root);
    let raw_result = read_project_json_value(root, &target.path)?;
    let result: TaskResult = match serde_json::from_value(raw_result.clone()) {
        Ok(result) => result,
        Err(error) => {
            return Ok(repairable(
                input,
                authorized,
                target.path.clone(),
                vec![issue(
                    "TASK_RESULT_SCHEMA_INVALID",
                    "$",
                    &format!("TaskResult JSON has an invalid schema: {error}"),
                )],
            ))
        }
    };
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "source.taskPlanRunId".to_string(),
            "task".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.requiredTopLevelFields".to_string(),
            "blockedOutput".to_string(),
        ],
    })?
    .fields;
    let task_plan_id = string_field(&fields, "source.taskPlanId")?;
    let task_id = string_field(&fields, "source.taskId")?;
    let run_id = string_field(&fields, "source.taskPlanRunId")?;
    let result_file = string_field(&fields, "outputContract.resultFile")?;
    let task: TaskDefinition = fields
        .get("task")
        .map(|field| serde_json::from_value(field.value.clone()))
        .transpose()
        .map_err(state::store::StateError::Json)?
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("missing task field".to_string())
        })?;
    let required_top_level_fields =
        string_vec_field(&fields, "outputContract.requiredTopLevelFields")?;
    let blocked_output = fields
        .get("blockedOutput")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let issues = validate_result(
        &raw_result,
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
        return Ok(repairable(input, authorized, target.path.clone(), issues));
    }
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
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
    update_delivery_after_result(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &run,
        &persisted_ref,
    )?;

    if matches!(run.status, TaskPlanRunStatus::Running) {
        continue_execution(&input.project_root, &delivery_id, &phase_id).into_result()
    } else {
        Ok(LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "not_implemented_for_batch".to_string(),
                message: "review and repair routing are recorded, but their domain handlers belong to batch 9.".to_string(),
                target_batch: Some(9),
                domain: Some("execution".to_string()),
                route_action: run.next_action.as_ref().map(|action| action.r#type.clone()),
                recovery_tool: Some("loom.continue".to_string()),
            },
        }))
    }
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
    issues
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
    let mut required_detail_ids = task.requirement_detail_refs.clone();
    for intent in &task.verification_intents {
        for detail_id in &intent.requirement_detail_refs {
            if !required_detail_ids.contains(detail_id) {
                required_detail_ids.push(detail_id.clone());
            }
        }
    }
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
                "Blocked reason must match the request blockedOutput mapping.",
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

fn update_delivery_after_result(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    run: &contracts::TaskPlanRun,
    result_ref: &str,
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
        let kind = match run
            .next_action
            .as_ref()
            .map(|action| action.r#type.as_str())
        {
            Some("review") => RouteActionKind::Review,
            Some("execution_repair") => RouteActionKind::ExecutionRepair,
            _ => RouteActionKind::ContinueExecution,
        };
        phase.next_action = Some(RouteAction {
            kind,
            source: "task_result".to_string(),
            reason: run
                .next_action
                .as_ref()
                .map(|action| action.reason.clone())
                .unwrap_or_else(|| "task_result_recorded".to_string()),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(result_ref.to_string()),
            details: Some(json!({
                "taskPlanRunId": run.run_id,
                "runStatus": run.status,
                "summary": run.summary
            })),
            target_phase_id: None,
        });
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
    LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
        project_root: input.project_root.clone(),
        target_file,
        target_ids: authorized
            .targets
            .iter()
            .map(|target| target.target_id.clone())
            .collect(),
        issues,
        resubmit_tool: "loom.recordTaskResultFile".to_string(),
        fix_scope: Some("task_result_candidate_only".to_string()),
        read_groups: authorized.read_groups.clone(),
    })
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

trait IntoStateResult {
    fn into_result(self) -> Result<LoomMcpActionResult, state::store::StateError>;
}

impl IntoStateResult for LoomMcpActionResult {
    fn into_result(self) -> Result<LoomMcpActionResult, state::store::StateError> {
        Ok(self)
    }
}
