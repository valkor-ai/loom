use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    forbidden_jvm_package_prefixes, BrowserCheckStatus, BrowserVerificationProfile,
    CodeQualityEvidence, CodeQualityRequirement, TaskDefinition, TaskKind, TaskPlanRunNextAction,
    TaskPlanRunStatus, TaskResult, TaskResultStatus, TaskRunStatus, VerificationEvidence,
};
use delivery_core::{
    read_selectors_value_from_paths, ArtifactKind, DeliveryLifecycleStatus, DomainDispatcher,
    FileSubmitInput, LoomMcpActionResult, LoomMcpAutoRunnableResult, LoomMcpFailure,
    LoomMcpFailureResult, LoomMcpNextAction, LoomMcpRepairableErrorResult, OperationContext,
    RouteAction, RouteActionKind, SubmitAcceptedEvent, TransitionEngine, TransitionStore,
    WriteArtifactNext, WriteMode, WriteTarget,
};
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
};

use crate::{
    paths::task_result_file,
    task_execution::{
        load_current_plan_and_run, save_run,
        task_projection_runtime_delivery_requirement_read_fields,
        task_with_phase_execution_guidance,
    },
    task_plan::update_run_summary,
    templates::{
        api_contract_evidence_applies, architecture_quality_evidence_applies,
        code_quality_evidence_applies, code_quality_execution_context,
        code_quality_requirements_for_task, frontend_quality_self_check_applies,
        frontend_self_check_applies, runtime_delivery_evidence_applies, task_result_contract,
        task_result_required_top_level_fields, task_result_schema_shape,
        task_result_template_with_code_quality,
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
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .collect::<BTreeSet<_>>();
    let mut fields_to_read = vec![
        "source.taskPlanId".to_string(),
        "source.taskId".to_string(),
        "source.taskPlanRunId".to_string(),
    ];
    for output_field in [
        "outputContract.resultFile",
        "outputContract.blockedReasonOptions",
    ] {
        if output_field == "outputContract.resultFile" || allowed_read_fields.contains(output_field)
        {
            fields_to_read.push(output_field.to_string());
        }
    }
    let fields =
        read_request_fields_chunked(&input.project_root, &input.request_ref, fields_to_read)?;
    let task_plan_id = string_field(&fields, "source.taskPlanId")?;
    let task_id = string_field(&fields, "source.taskId")?;
    let run_id = string_field(&fields, "source.taskPlanRunId")?;
    let result_file = string_field(&fields, "outputContract.resultFile")?;
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let (current_task_plan, current_run) = load_current_plan_and_run(root, &locator)?;
    let task = current_task_plan
        .tasks
        .iter()
        .find(|candidate| candidate.task_id == task_id)
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "TaskResult request references task {task_id}, but the current TaskPlan has no canonical definition for it"
            ))
        })?;
    // TaskPlan is the canonical typed task contract. The request read groups are an
    // agent-facing projection and may legitimately contain nullable optional values;
    // rebuilding TaskDefinition from that projection makes repair sensitive to those
    // presentation details and can turn a valid request into a terminal state error.
    let task = task_with_phase_execution_guidance(root, &locator, task)?;
    let browser_profile = current_task_plan
        .browser_verification_profiles
        .iter()
        .find(|profile| profile.task_id == task.task_id)
        .cloned();
    let code_quality_requirements = code_quality_requirements_for_task(&current_task_plan, &task);
    let authoritative_result_contract =
        task_result_contract(&task, &code_quality_requirements, browser_profile.as_ref());
    let required_top_level_fields = authoritative_result_contract["requiredTopLevelFields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let blocked_output = json!({
        "blockedReasons": array_field(&fields, "outputContract.blockedReasonOptions")
    });
    let normalized_result = normalize_task_result_machine_fields(
        raw_result.clone(),
        &authorized.request_id,
        &task_plan_id,
        &task_id,
        &task,
        browser_profile.as_ref(),
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
    let mut issues = validate_non_applicable_evidence_fields(&raw_result, &task);
    issues.extend(validate_result(
        root,
        &normalized_result,
        &result,
        &task,
        browser_profile.as_ref(),
        &current_task_plan.engineering_quality_requirements,
        &code_quality_requirements,
        &required_top_level_fields,
        &blocked_output,
        &result_file,
        &target.path,
    ));
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
                blocked_output,
                submitted_result: normalized_result.clone(),
                previous_changed_files,
                code_quality_requirements,
                browser_profile,
            }),
        );
    }
    let mut run = current_run;
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
    let canonical_value =
        canonical_task_result_value(&normalized_result, &result, &required_top_level_fields)?;
    state::store::write_json_atomic(&persisted, &canonical_value)?;
    let persisted_value = state::store::read_json_value(&persisted)?;
    let _: TaskResult = serde_json::from_value(persisted_value.clone()).map_err(|error| {
        state::store::StateError::StateCorrupted(format!(
            "canonical TaskResult JSON is invalid: {error}"
        ))
    })?;
    for field in &required_top_level_fields {
        if !task_result_required_field_applies_to_status(field, &result.status) {
            continue;
        }
        if persisted_value.get(field).is_none() {
            return Err(state::store::StateError::StateCorrupted(format!(
                "canonical TaskResult lost required field {field}"
            )));
        }
    }
    state::lifecycle_store::finalize_agent_candidate(root, &target.path)?;
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
    let Some(next_action) = route_action_for_task_result(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &run,
        &result,
        &input.request_ref,
        &persisted_ref,
    )?
    else {
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
    project_root: &Path,
    raw_result: &Value,
    result: &TaskResult,
    task: &TaskDefinition,
    browser_profile: Option<&BrowserVerificationProfile>,
    engineering_quality_requirements: &[contracts::EngineeringQualityRequirement],
    code_quality_requirements: &[CodeQualityRequirement],
    required_top_level_fields: &[String],
    blocked_output: &Value,
    expected_file: &str,
    actual_file: &str,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    for field in required_top_level_fields {
        if raw_result.get(field).is_none()
            && task_result_required_field_applies_to_status(field, &result.status)
        {
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
    validate_verification_provenance(project_root, result, &mut issues);
    validate_implementation_obligation_results(project_root, result, task, &mut issues);
    validate_stack_conformance(
        project_root,
        result,
        task,
        engineering_quality_requirements,
        &mut issues,
    );
    validate_browser_verification_results(result, browser_profile, &mut issues);
    validate_requirement_detail_evidence(result, task, &mut issues);
    validate_concept_evidence(result, task, &mut issues);
    validate_runtime_delivery_evidence(result, task, &mut issues);
    validate_frontend_experience_self_check(result, task, &mut issues);
    validate_frontend_quality_self_check(result, task, &mut issues);
    validate_architecture_quality_evidence(result, task, &mut issues);
    validate_api_contract_evidence(result, task, &mut issues);
    validate_code_quality_evidence(result, task, code_quality_requirements, &mut issues);
    validate_jvm_package_names(
        project_root,
        result,
        task,
        code_quality_requirements,
        &mut issues,
    );
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

fn validate_jvm_package_names(
    project_root: &Path,
    result: &TaskResult,
    task: &TaskDefinition,
    code_quality_requirements: &[CodeQualityRequirement],
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) || !task_has_jvm_package_policy(task, code_quality_requirements)
    {
        return;
    }
    for relative in &result.changed_files {
        if !is_jvm_production_source_path(relative) {
            continue;
        }
        let Ok(path) = from_project_relative(project_root, relative) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(package_name) = extract_jvm_package_name(&content) else {
            continue;
        };
        let Some(forbidden_prefix) = forbidden_package_prefix(&package_name) else {
            continue;
        };
        issues.push(issue(
            "TASK_RESULT_CODE_QUALITY_INVALID",
            "changedFiles",
            &format!(
                "Production JVM source file {relative} declares placeholder package `{package_name}` using forbidden prefix `{forbidden_prefix}`. Use an existing package root, build group metadata, confirmed organization/project namespace, or fallback app.<project_slug>/app.generated."
            ),
        ));
    }
}

fn task_has_jvm_package_policy(
    task: &TaskDefinition,
    code_quality_requirements: &[CodeQualityRequirement],
) -> bool {
    let requirement_refs = task
        .code_quality_requirement_refs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    code_quality_requirements.iter().any(|requirement| {
        requirement_refs.contains(requirement.requirement_id.as_str())
            && requirement.package_naming_policy.is_some()
    })
}

fn is_jvm_production_source_path(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/");
    (normalized.ends_with(".java") || normalized.ends_with(".kt"))
        && (normalized.contains("/src/main/") || normalized.starts_with("src/main/"))
}

fn extract_jvm_package_name(content: &str) -> Option<String> {
    for line in content.lines().take(120) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.is_empty() {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("package ") else {
            continue;
        };
        let package = rest
            .trim()
            .trim_end_matches(';')
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches(';')
            .to_string();
        if !package.is_empty() {
            return Some(package);
        }
    }
    None
}

fn forbidden_package_prefix(package_name: &str) -> Option<&'static str> {
    let package = package_name.to_ascii_lowercase();
    forbidden_jvm_package_prefixes()
        .into_iter()
        .find(|prefix| package == *prefix || package.starts_with(&format!("{prefix}.")))
}

fn task_result_required_field_applies_to_status(field: &str, status: &TaskResultStatus) -> bool {
    if !matches!(status, TaskResultStatus::Failed | TaskResultStatus::Blocked) {
        return true;
    }
    !matches!(
        field,
        "frontendExperienceSelfCheck"
            | "frontendQualitySelfCheck"
            | "runtimeDeliveryEvidence"
            | "conceptEvidence"
            | "architectureQualityEvidence"
            | "apiContractEvidence"
            | "codeQualityEvidence"
    )
}

fn normalize_task_result_machine_fields(
    mut raw_result: Value,
    request_id: &str,
    task_plan_id: &str,
    task_id: &str,
    task: &TaskDefinition,
    browser_profile: Option<&BrowserVerificationProfile>,
) -> Value {
    let Some(object) = raw_result.as_object_mut() else {
        return raw_result;
    };

    let now = state::store::now_string();
    object.insert("schemaVersion".to_string(), json!("1.0"));
    object.insert(
        "taskResultId".to_string(),
        json!(format!("taskresult-{}", safe_id(request_id))),
    );
    object.insert("taskPlanId".to_string(), json!(task_plan_id));
    object.insert("taskId".to_string(), json!(task_id));
    if !object
        .get("changedFiles")
        .and_then(Value::as_array)
        .is_some()
    {
        object.insert("changedFiles".to_string(), json!([]));
    }
    normalize_string_array_field(object, "changedFiles");
    if !object.contains_key("noChangeReason")
        || !object
            .get("noChangeReason")
            .is_some_and(|value| value.is_null() || value.is_object())
    {
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
    normalize_string_array_field(object, "notes");
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
    object.insert("createdAt".to_string(), json!(now.clone()));
    object.insert("updatedAt".to_string(), json!(now));

    normalize_verification_result_machine_fields(object, task, browser_profile);
    normalize_implementation_obligation_results(object, task);
    normalize_browser_environment_blocked_result(object, browser_profile);

    let detail_ids = required_requirement_detail_ids(task);
    normalize_requirement_detail_evidence_machine_fields(object, task, &detail_ids);
    normalize_verification_provenance(object);

    remove_non_applicable_evidence_fields(object, task);
    normalize_applicable_evidence_array_fields(object, task);
    remove_incomplete_status_evidence_fields(object);

    normalize_concept_evidence_machine_fields(object, task);
    normalize_quality_evidence_machine_fields(object, task);

    if let Some(requirement) = &task.runtime_delivery_requirement {
        if requirement.applies_to_this_task {
            normalize_runtime_delivery_evidence(object, requirement);
        }
    }

    if let Some(requirement) = &task.frontend_experience_requirement {
        normalize_frontend_experience_self_check(object, requirement);
    }
    normalize_frontend_quality_self_check_shape(
        object,
        task.frontend_experience_requirement.as_ref(),
    );
    if let Some(continuity) = object
        .get_mut("executionContinuity")
        .and_then(Value::as_object_mut)
    {
        normalize_string_array_field(continuity, "notes");
    }
    project_task_result_to_canonical_fields(object);

    raw_result
}

fn normalize_implementation_obligation_results(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
) {
    let existing = object
        .get("implementationObligationResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut unkeyed = Vec::new();
    let mut by_id = BTreeMap::new();
    for value in existing {
        let Some(item) = value.as_object() else {
            unkeyed.push(serde_json::Map::new());
            continue;
        };
        let Some(id) = item.get("obligationId").and_then(Value::as_str) else {
            unkeyed.push(item.clone());
            continue;
        };
        if by_id.insert(id.to_string(), item.clone()).is_some() {
            unkeyed.push(item.clone());
        }
    }
    let normalized = task
        .implementation_obligations
        .iter()
        .map(|obligation| {
            let mut item = by_id
                .remove(&obligation.obligation_id)
                .or_else(|| {
                    if unkeyed.is_empty() {
                        None
                    } else {
                        Some(unkeyed.remove(0))
                    }
                })
                .unwrap_or_default();
            item.insert("obligationId".to_string(), json!(obligation.obligation_id));
            item.insert(
                "verificationIds".to_string(),
                json!(obligation.verification_ids),
            );
            if !matches!(
                item.get("status").and_then(Value::as_str),
                Some("satisfied" | "partial" | "blocked" | "not_verified")
            ) {
                item.insert("status".to_string(), json!("not_verified"));
            }
            normalize_string_array_field(&mut item, "verificationIds");
            normalize_string_array_field(&mut item, "evidenceRefs");
            if let Some(evidence_refs) = item.get_mut("evidenceRefs").and_then(Value::as_array_mut)
            {
                evidence_refs.retain(|reference| {
                    reference
                        .as_str()
                        .is_some_and(|reference| !is_generated_output_path(reference))
                });
            }
            if !item.get("summary").is_some_and(Value::is_string) {
                item.insert("summary".to_string(), json!(""));
            }
            retain_object_fields(
                &mut item,
                &[
                    "obligationId",
                    "status",
                    "verificationIds",
                    "evidenceRefs",
                    "summary",
                ],
            );
            Value::Object(item)
        })
        .collect::<Vec<_>>();
    object.insert(
        "implementationObligationResults".to_string(),
        Value::Array(normalized),
    );
}

fn project_task_result_to_canonical_fields(object: &mut serde_json::Map<String, Value>) {
    retain_object_fields(
        object,
        &[
            "schemaVersion",
            "taskResultId",
            "taskId",
            "taskPlanId",
            "status",
            "changedFiles",
            "noChangeReason",
            "verificationResults",
            "implementationObligationResults",
            "selfRepairSummary",
            "failure",
            "executionContinuity",
            "notes",
            "frontendExperienceSelfCheck",
            "frontendQualitySelfCheck",
            "runtimeDeliveryEvidence",
            "requirementDetailEvidence",
            "conceptEvidence",
            "architectureQualityEvidence",
            "apiContractEvidence",
            "codeQualityEvidence",
            "blockedReasons",
            "createdAt",
            "updatedAt",
        ],
    );
    project_object_fields(object, "noChangeReason", &["code", "summary"]);
    project_object_fields(
        object,
        "selfRepairSummary",
        &[
            "attempted",
            "attemptCount",
            "stopReason",
            "progressObserved",
        ],
    );
    project_object_fields(object, "failure", &["code", "summary"]);
    project_object_fields(
        object,
        "executionContinuity",
        &[
            "taskResultSubmittedAfterVerification",
            "agentOwnedLongRunningWork",
            "notes",
        ],
    );
    project_object_array_fields(
        object,
        "blockedReasons",
        &["code", "nextNode", "message", "details"],
    );
}

fn retain_object_fields(object: &mut serde_json::Map<String, Value>, fields: &[&str]) {
    object.retain(|key, _| fields.iter().any(|allowed| *allowed == key));
}

fn project_object_fields(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
    allowed_fields: &[&str],
) {
    if let Some(value) = object.get_mut(field).and_then(Value::as_object_mut) {
        retain_object_fields(value, allowed_fields);
    }
}

fn project_object_array_fields(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
    allowed_fields: &[&str],
) {
    if let Some(items) = object.get_mut(field).and_then(Value::as_array_mut) {
        for item in items {
            if let Some(item) = item.as_object_mut() {
                retain_object_fields(item, allowed_fields);
            }
        }
    }
}

fn canonical_task_result_value(
    normalized: &Value,
    result: &TaskResult,
    required_top_level_fields: &[String],
) -> Result<Value, state::store::StateError> {
    let mut canonical = serde_json::to_value(result).map_err(state::store::StateError::Json)?;
    let Some(object) = canonical.as_object_mut() else {
        return Err(state::store::StateError::StateCorrupted(
            "canonical TaskResult must be a JSON object".to_string(),
        ));
    };
    for field in required_top_level_fields {
        if !task_result_required_field_applies_to_status(field, &result.status) {
            continue;
        }
        if object.contains_key(field) {
            continue;
        }
        let value = normalized
            .get(field)
            .cloned()
            .unwrap_or_else(|| canonical_empty_task_result_field(field));
        object.insert(field.clone(), value);
    }
    let _: TaskResult =
        serde_json::from_value(canonical.clone()).map_err(state::store::StateError::Json)?;
    Ok(canonical)
}

fn canonical_empty_task_result_field(field: &str) -> Value {
    match field {
        "noChangeReason" | "failure" => Value::Null,
        _ => Value::Array(vec![]),
    }
}

fn normalize_verification_provenance(object: &mut serde_json::Map<String, Value>) {
    let detail_evidence = object
        .get("requirementDetailEvidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(verifications) = object
        .get_mut("verificationResults")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for verification in verifications {
        let Some(item) = verification.as_object_mut() else {
            continue;
        };
        let verification_id = item
            .get("verificationId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut provenance = item
            .get("provenance")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut evidence_refs = provenance
            .get("evidenceRefs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if evidence_refs.is_empty() {
            evidence_refs = detail_evidence
                .iter()
                .filter(|detail| {
                    detail
                        .get("verificationIds")
                        .and_then(Value::as_array)
                        .is_some_and(|ids| {
                            ids.iter().any(|id| id.as_str() == Some(&verification_id))
                        })
                })
                .flat_map(|detail| {
                    detail
                        .get("evidenceRefs")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                })
                .collect();
        }
        provenance.insert("evidenceRefs".to_string(), Value::Array(evidence_refs));
        provenance
            .entry("testCaseRefs".to_string())
            .or_insert_with(|| Value::Array(vec![]));
        provenance
            .entry("command".to_string())
            .or_insert(Value::Null);
        provenance
            .entry("exitCode".to_string())
            .or_insert(Value::Null);
        normalize_string_array_field(&mut provenance, "evidenceRefs");
        normalize_string_array_field(&mut provenance, "changedFiles");
        normalize_string_array_field(&mut provenance, "testCaseRefs");
        retain_object_fields(
            &mut provenance,
            &[
                "evidenceRefs",
                "changedFiles",
                "testCaseRefs",
                "command",
                "exitCode",
            ],
        );
        item.insert("provenance".to_string(), Value::Object(provenance));
    }
}

fn remove_incomplete_status_evidence_fields(object: &mut serde_json::Map<String, Value>) {
    if matches!(
        object.get("status").and_then(Value::as_str),
        Some("failed" | "blocked")
    ) {
        for field in [
            "conceptEvidence",
            "architectureQualityEvidence",
            "apiContractEvidence",
            "codeQualityEvidence",
        ] {
            object.remove(field);
        }
    }
}

fn normalize_browser_environment_blocked_result(
    object: &mut serde_json::Map<String, Value>,
    browser_profile: Option<&BrowserVerificationProfile>,
) {
    let Some(profile) = browser_profile else {
        return;
    };
    let required_check_ids = profile
        .checks
        .iter()
        .filter(|check| check.enforcement == contracts::BrowserEvidenceEnforcement::Required)
        .map(|check| check.check_id.as_str())
        .collect::<BTreeSet<_>>();
    if required_check_ids.is_empty() {
        return;
    }
    let Some(verifications) = object
        .get_mut("verificationResults")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let mut seen_required = BTreeSet::new();
    let mut has_environment_blocker = false;
    let mut has_product_failure_or_missing_check = false;
    for verification in verifications.iter_mut() {
        let mut verification_environment_blocked = false;
        for check in verification
            .get("browserChecks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(check_id) = check.get("checkId").and_then(Value::as_str) else {
                continue;
            };
            if !required_check_ids.contains(check_id) {
                continue;
            }
            seen_required.insert(check_id.to_string());
            match check.get("status").and_then(Value::as_str) {
                Some("passed") => {}
                Some("blocked")
                    if check
                        .get("blockedReason")
                        .and_then(Value::as_str)
                        .is_some_and(|reason| !reason.trim().is_empty()) =>
                {
                    has_environment_blocker = true;
                    verification_environment_blocked = true;
                }
                _ => has_product_failure_or_missing_check = true,
            }
        }
        if verification_environment_blocked {
            verification["status"] = json!("inconclusive");
            verification["evidenceType"] = json!("browser_automation");
            verification["summary"] = json!(
                "Required browser evidence was blocked by the supplied execution environment."
            );
        }
    }
    if seen_required.len() != required_check_ids.len()
        || !has_environment_blocker
        || has_product_failure_or_missing_check
    {
        return;
    }
    object.insert("status".to_string(), json!("completed_with_notes"));
    object.insert("failure".to_string(), Value::Null);
    object.insert("blockedReasons".to_string(), json!([]));
    let notes = object
        .entry("notes".to_string())
        .or_insert_with(|| json!([]));
    let Some(notes) = notes.as_array_mut() else {
        return;
    };
    let note = "Required browser evidence is environment-blocked and must be resolved in Review.";
    if !notes.iter().any(|item| item.as_str() == Some(note)) {
        notes.push(json!(note));
    }
}

fn remove_non_applicable_evidence_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
) {
    if !frontend_self_check_applies(task)
        && object
            .get("frontendExperienceSelfCheck")
            .is_some_and(Value::is_null)
    {
        object.remove("frontendExperienceSelfCheck");
    }
    if !frontend_quality_self_check_applies(task)
        && object
            .get("frontendQualitySelfCheck")
            .is_some_and(Value::is_null)
    {
        object.remove("frontendQualitySelfCheck");
    }
    if !runtime_delivery_evidence_applies(task)
        && object
            .get("runtimeDeliveryEvidence")
            .is_some_and(Value::is_null)
    {
        object.remove("runtimeDeliveryEvidence");
    }
    if task.concept_refs.is_empty() && object.get("conceptEvidence").is_some_and(Value::is_null) {
        object.remove("conceptEvidence");
    }
    if !architecture_quality_evidence_applies(task)
        && object
            .get("architectureQualityEvidence")
            .is_some_and(Value::is_null)
    {
        object.remove("architectureQualityEvidence");
    }
    if !api_contract_evidence_applies(task)
        && object
            .get("apiContractEvidence")
            .is_some_and(Value::is_null)
    {
        object.remove("apiContractEvidence");
    }
    if !code_quality_evidence_applies(task)
        && object
            .get("codeQualityEvidence")
            .is_some_and(Value::is_null)
    {
        object.remove("codeQualityEvidence");
    }
}

fn validate_non_applicable_evidence_fields(
    raw_result: &Value,
    task: &TaskDefinition,
) -> Vec<delivery_core::RepairIssue> {
    let Some(object) = raw_result.as_object() else {
        return Vec::new();
    };
    let fields = [
        (
            frontend_self_check_applies(task),
            "frontendExperienceSelfCheck",
        ),
        (
            frontend_quality_self_check_applies(task),
            "frontendQualitySelfCheck",
        ),
        (
            runtime_delivery_evidence_applies(task),
            "runtimeDeliveryEvidence",
        ),
        (!task.concept_refs.is_empty(), "conceptEvidence"),
        (
            architecture_quality_evidence_applies(task),
            "architectureQualityEvidence",
        ),
        (api_contract_evidence_applies(task), "apiContractEvidence"),
        (code_quality_evidence_applies(task), "codeQualityEvidence"),
    ];
    fields
        .into_iter()
        .filter(|(applies, field)| !applies && object.get(*field).is_some_and(|value| !value.is_null()))
        .map(|(_, field)| issue(
            "TASK_RESULT_EVIDENCE_NOT_APPLICABLE",
            field,
            "TaskResult must not include evidence fields when the canonical task contract does not assign that evidence type; remove the field instead of submitting an unconsumed value.",
        ))
        .collect()
}

fn normalize_applicable_evidence_array_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
) {
    for (applies, field) in [
        (!task.concept_refs.is_empty(), "conceptEvidence"),
        (
            architecture_quality_evidence_applies(task),
            "architectureQualityEvidence",
        ),
        (api_contract_evidence_applies(task), "apiContractEvidence"),
        (code_quality_evidence_applies(task), "codeQualityEvidence"),
    ] {
        if applies && !object.get(field).is_some_and(Value::is_array) {
            object.insert(field.to_string(), json!([]));
        }
    }
}

fn normalize_frontend_quality_self_check_shape(
    object: &mut serde_json::Map<String, Value>,
    requirement: Option<&Value>,
) {
    let Some(self_check) = object
        .get_mut("frontendQualitySelfCheck")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(requirement) = requirement {
        let contract = requirement
            .pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
            .unwrap_or(&Value::Null);
        if let Some(contract_ref) = requirement
            .get("uiSurfaceDecisionContractRef")
            .and_then(Value::as_str)
            .or_else(|| contract.get("contractRef").and_then(Value::as_str))
        {
            self_check.insert(
                "surfaceDecisionContractRef".to_string(),
                json!(contract_ref),
            );
        }
        normalize_frontend_surface_evidence_array(
            self_check,
            "surfaceRegionEvidence",
            contract,
            "regionsInScope",
            "regionId",
        );
        normalize_frontend_surface_evidence_array(
            self_check,
            "surfaceActionEvidence",
            contract,
            "actionsInScope",
            "actionId",
        );
        normalize_frontend_surface_evidence_array(
            self_check,
            "surfaceStateEvidence",
            contract,
            "statesInScope",
            "state",
        );
        normalize_frontend_surface_evidence_array(
            self_check,
            "surfaceQualityRuleEvidence",
            contract,
            "qualityRulesInScope",
            "ruleId",
        );
    } else {
        self_check.remove("surfaceDecisionContractRef");
    }
    normalize_string_array_field(self_check, "referencePlanFilesChecked");
    normalize_string_array_field(self_check, "evidenceRefs");
    normalize_string_array_field(self_check, "knownGaps");
    if !self_check
        .get("contentBoundaryEvidence")
        .is_some_and(Value::is_object)
    {
        self_check.insert(
            "contentBoundaryEvidence".to_string(),
            json!({
                "checked": true,
                "allowedContentExamples": [],
                "forbiddenContentViolations": [],
                "evidence": ""
            }),
        );
    } else if let Some(content) = self_check
        .get_mut("contentBoundaryEvidence")
        .and_then(Value::as_object_mut)
    {
        if !content.get("checked").is_some_and(Value::is_boolean) {
            content.insert("checked".to_string(), json!(true));
        }
        normalize_string_array_field(content, "allowedContentExamples");
        normalize_string_array_field(content, "forbiddenContentViolations");
    }
    if self_check
        .get("designTokenEvidence")
        .is_some_and(|value| !value.is_null() && !value.is_object())
    {
        self_check.remove("designTokenEvidence");
    }
    project_frontend_quality_self_check(self_check);
}

fn project_frontend_quality_self_check(self_check: &mut serde_json::Map<String, Value>) {
    retain_object_fields(
        self_check,
        &[
            "status",
            "surfaceDecisionContractRef",
            "evidenceRefs",
            "surfaceRegionEvidence",
            "surfaceActionEvidence",
            "surfaceStateEvidence",
            "surfaceQualityRuleEvidence",
            "contentBoundaryEvidence",
            "referencePlanFilesChecked",
            "designTokenEvidence",
            "knownGaps",
        ],
    );
    project_object_fields(
        self_check,
        "contentBoundaryEvidence",
        &[
            "checked",
            "allowedContentExamples",
            "forbiddenContentViolations",
            "evidence",
        ],
    );
    project_object_fields(
        self_check,
        "designTokenEvidence",
        &[
            "strategyUsed",
            "templateIdUsed",
            "tokenAssetFiles",
            "tokenConsumerFiles",
            "existingTokenSystemReused",
            "parallelTokenSystemCreated",
            "mergeSummary",
        ],
    );
}

fn normalize_concept_evidence_machine_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
) {
    if task.concept_refs.is_empty()
        || matches!(
            object.get("status").and_then(Value::as_str),
            Some("failed" | "blocked")
        )
    {
        return;
    }
    let raw_items = object
        .get("conceptEvidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let normalized = task
        .concept_refs
        .iter()
        .enumerate()
        .map(|(index, concept_ref)| {
            let raw = ordered_machine_item(
                &raw_items,
                index,
                "conceptRef",
                concept_ref,
                &mut used,
                false,
            );
            let raw_object = raw.as_object();
            let mut item = serde_json::Map::new();
            item.insert("conceptRef".to_string(), json!(concept_ref));
            item.insert(
                "evidenceType".to_string(),
                raw_object
                    .and_then(|value| value.get("evidenceType"))
                    .filter(|value| value.is_string())
                    .cloned()
                    .unwrap_or_else(|| json!("code")),
            );
            item.insert(
                "refs".to_string(),
                raw_object
                    .and_then(|value| value.get("refs"))
                    .filter(|value| value.is_array())
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
            normalize_string_array_field(&mut item, "refs");
            item.insert(
                "summary".to_string(),
                raw_object
                    .and_then(|value| value.get("summary"))
                    .filter(|value| value.is_string())
                    .cloned()
                    .unwrap_or_else(|| json!("")),
            );
            Value::Object(item)
        })
        .collect::<Vec<_>>();
    object.insert("conceptEvidence".to_string(), Value::Array(normalized));
}

fn normalize_quality_evidence_machine_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
) {
    if matches!(
        object.get("status").and_then(Value::as_str),
        Some("failed" | "blocked")
    ) {
        return;
    }
    // Quality evidence is linked to the verification evidence that actually
    // passed for this result. Linking every task intent makes a valid
    // completed_with_notes result fail when one unrelated or environment-bound
    // intent is not passed, and it falsely broadens the provenance of each
    // quality requirement.
    let verification_ids = object
        .get("verificationResults")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|verification| verification.get("status").and_then(Value::as_str) == Some("passed"))
        .filter_map(|verification| {
            verification
                .get("verificationId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let interface_refs = task.write_boundary.artifact_refs.all_interfaces();
    normalize_quality_evidence_array(
        object,
        "architectureQualityEvidence",
        &task.architecture_quality_requirement_refs,
        &verification_ids,
        &[],
    );
    normalize_quality_evidence_array(
        object,
        "apiContractEvidence",
        &task.api_contract_requirement_refs,
        &verification_ids,
        &interface_refs,
    );
    normalize_quality_evidence_array(
        object,
        "codeQualityEvidence",
        &task.code_quality_requirement_refs,
        &verification_ids,
        &[],
    );
}

fn normalize_quality_evidence_array(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
    requirement_ids: &[String],
    verification_ids: &[String],
    interface_refs: &[String],
) {
    if requirement_ids.is_empty() {
        return;
    }
    let raw_items = object
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let normalized = requirement_ids
        .iter()
        .enumerate()
        .map(|(index, requirement_id)| {
            let raw = ordered_machine_item(
                &raw_items,
                index,
                "requirementId",
                requirement_id,
                &mut used,
                false,
            );
            let raw_object = raw.as_object();
            let mut item = serde_json::Map::new();
            item.insert("requirementId".to_string(), json!(requirement_id));
            item.insert("verificationIds".to_string(), json!(verification_ids));
            item.insert(
                "status".to_string(),
                raw_object
                    .and_then(|value| value.get("status"))
                    .filter(|value| value.as_str().is_some_and(|status| !status.is_empty()))
                    .cloned()
                    .unwrap_or_else(|| json!("not_verified")),
            );
            if field == "apiContractEvidence" {
                item.insert("interfaceRefs".to_string(), json!(interface_refs));
            }
            if field == "codeQualityEvidence" {
                item.insert(
                    "referenceGroupsChecked".to_string(),
                    raw_object
                        .and_then(|value| value.get("referenceGroupsChecked"))
                        .filter(|value| value.is_object())
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                );
                item.insert(
                    "referenceFilesChecked".to_string(),
                    raw_object
                        .and_then(|value| value.get("referenceFilesChecked"))
                        .filter(|value| value.is_array())
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                );
                normalize_string_array_field(&mut item, "referenceFilesChecked");
                normalize_string_array_map_field(&mut item, "referenceGroupsChecked");
            }
            if matches!(field, "apiContractEvidence" | "codeQualityEvidence") {
                item.insert(
                    "knownGaps".to_string(),
                    raw_object
                        .and_then(|value| value.get("knownGaps"))
                        .filter(|value| value.is_array())
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                );
                normalize_string_array_field(&mut item, "knownGaps");
            }
            item.insert(
                "summary".to_string(),
                raw_object
                    .and_then(|value| value.get("summary"))
                    .filter(|value| value.is_string())
                    .cloned()
                    .unwrap_or_else(|| json!("")),
            );
            Value::Object(item)
        })
        .collect::<Vec<_>>();
    object.insert(field.to_string(), Value::Array(normalized));
}

fn normalize_frontend_surface_evidence_array(
    self_check: &mut serde_json::Map<String, Value>,
    field: &str,
    contract: &Value,
    contract_array_field: &str,
    id_field: &str,
) {
    let expected_ids = contract
        .get(contract_array_field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(id_field).and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let raw_items = self_check
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let normalized = expected_ids
        .iter()
        .enumerate()
        .map(|(index, expected_id)| {
            let raw = ordered_machine_item(&raw_items, index, "id", expected_id, &mut used, false);
            let raw_object = raw.as_object();
            let mut item = serde_json::Map::new();
            item.insert("id".to_string(), json!(expected_id));
            item.insert(
                "status".to_string(),
                raw_object
                    .and_then(|value| value.get("status"))
                    .filter(|value| value.as_str().is_some_and(|status| !status.is_empty()))
                    .cloned()
                    .unwrap_or_else(|| json!("missing")),
            );
            item.insert(
                "files".to_string(),
                raw_object
                    .and_then(|value| value.get("files"))
                    .filter(|value| value.is_array())
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
            normalize_string_array_field(&mut item, "files");
            item.insert(
                "evidence".to_string(),
                raw_object
                    .and_then(|value| value.get("evidence"))
                    .filter(|value| value.is_string())
                    .cloned()
                    .unwrap_or_else(|| json!("")),
            );
            Value::Object(item)
        })
        .collect::<Vec<_>>();
    self_check.insert(field.to_string(), Value::Array(normalized));
}

fn normalize_string_array_field(object: &mut serde_json::Map<String, Value>, field: &str) {
    let Some(items) = object.get(field).and_then(Value::as_array).cloned() else {
        object.insert(field.to_string(), json!([]));
        return;
    };
    object.insert(
        field.to_string(),
        Value::Array(items.into_iter().filter(Value::is_string).collect()),
    );
}

fn normalize_string_array_map_field(object: &mut serde_json::Map<String, Value>, field: &str) {
    let Some(values) = object.get(field).and_then(Value::as_object) else {
        object.insert(field.to_string(), json!({}));
        return;
    };
    let normalized = values
        .iter()
        .map(|(key, value)| {
            let items = value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            (key.clone(), json!(items))
        })
        .collect::<serde_json::Map<_, _>>();
    object.insert(field.to_string(), Value::Object(normalized));
}

fn ordered_machine_item(
    raw_items: &[Value],
    index: usize,
    machine_field: &str,
    expected_value: &str,
    used: &mut BTreeSet<usize>,
    drop_mismatched_id: bool,
) -> Value {
    let exact_index = raw_items.iter().enumerate().find_map(|(item_index, item)| {
        (!used.contains(&item_index)
            && item
                .get(machine_field)
                .and_then(Value::as_str)
                .is_some_and(|value| value == expected_value))
        .then_some(item_index)
    });
    let item_index = exact_index
        .or_else(|| (index < raw_items.len() && !used.contains(&index)).then_some(index));
    let Some(item_index) = item_index else {
        return json!({});
    };
    used.insert(item_index);
    let raw = &raw_items[item_index];
    let mismatched_id = raw
        .get(machine_field)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty() && value != expected_value);
    if drop_mismatched_id && mismatched_id {
        json!({})
    } else {
        raw.clone()
    }
}

fn normalize_verification_result_machine_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
    browser_profile: Option<&BrowserVerificationProfile>,
) {
    let raw_items = object
        .get("verificationResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let mut normalized = Vec::new();
    for (index, intent) in task.verification_intents.iter().enumerate() {
        let raw = ordered_machine_item(
            &raw_items,
            index,
            "verificationId",
            &intent.verification_id,
            &mut used,
            false,
        );
        let raw_object = raw.as_object();
        let mut item = serde_json::Map::new();
        item.insert(
            "verificationId".to_string(),
            json!(intent.verification_id.clone()),
        );
        item.insert(
            "status".to_string(),
            raw_object
                .and_then(|value| value.get("status"))
                .filter(|value| value.as_str().is_some_and(|status| !status.is_empty()))
                .cloned()
                .unwrap_or_else(|| json!("not_run")),
        );
        let submitted_evidence_type = raw_object
            .and_then(|value| value.get("evidenceType"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let evidence_allowed = submitted_evidence_type.as_ref().is_some_and(|candidate| {
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
        } else if let Some(evidence_type) = submitted_evidence_type {
            item.insert("evidenceType".to_string(), json!(evidence_type));
        }
        if let Some(browser_checks) = raw_object
            .and_then(|value| value.get("browserChecks"))
            .filter(|value| value.is_array())
            .cloned()
        {
            item.insert("browserChecks".to_string(), browser_checks);
        }
        normalize_browser_check_machine_fields(&mut item, &intent.verification_id, browser_profile);
        item.insert(
            "summary".to_string(),
            raw_object
                .and_then(|value| value.get("summary"))
                .filter(|value| value.as_str().is_some_and(|summary| !summary.is_empty()))
                .cloned()
                .unwrap_or_else(|| {
                    json!("Verification result was not reported before TaskResult submission.")
                }),
        );
        if let Some(provenance) = raw_object
            .and_then(|value| value.get("provenance"))
            .filter(|value| value.is_object())
            .cloned()
        {
            item.insert("provenance".to_string(), provenance);
        }
        normalized.push(Value::Object(item));
    }
    object.insert("verificationResults".to_string(), Value::Array(normalized));
}

fn normalize_browser_check_machine_fields(
    verification: &mut serde_json::Map<String, Value>,
    verification_id: &str,
    browser_profile: Option<&BrowserVerificationProfile>,
) {
    let Some(profile) = browser_profile else {
        verification.remove("browserChecks");
        return;
    };
    let expected = profile
        .checks
        .iter()
        .filter(|check| check.verification_id == verification_id)
        .collect::<Vec<_>>();
    if expected.is_empty() {
        verification.remove("browserChecks");
        return;
    }
    verification.insert("evidenceType".to_string(), json!("browser_automation"));
    let raw_items = verification
        .get("browserChecks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut used = BTreeSet::new();
    let mut normalized = Vec::new();
    for (index, check) in expected.into_iter().enumerate() {
        let raw = ordered_machine_item(
            &raw_items,
            index,
            "checkId",
            &check.check_id,
            &mut used,
            true,
        );
        let mut item = raw.as_object().cloned().unwrap_or_default();
        item.remove("checkId");
        item.insert("checkId".to_string(), json!(check.check_id));
        if !matches!(
            item.get("status").and_then(Value::as_str),
            Some("passed" | "failed" | "blocked" | "not_run")
        ) {
            item.insert("status".to_string(), json!("not_run"));
        }
        if !item.get("command").is_some_and(Value::is_string) {
            item.insert("command".to_string(), json!(""));
        }
        if item
            .get("attempts")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > u32::MAX as u64)
        {
            item.insert("attempts".to_string(), json!(0));
        }
        normalize_string_array_field(&mut item, "artifactRefs");
        if !item.get("observedOutcome").is_some_and(Value::is_string) {
            item.insert("observedOutcome".to_string(), json!(""));
        }
        if !item
            .get("blockedReason")
            .is_some_and(|value| value.is_null() || value.is_string())
        {
            item.insert("blockedReason".to_string(), Value::Null);
        }
        retain_object_fields(
            &mut item,
            &[
                "checkId",
                "status",
                "command",
                "attempts",
                "artifactRefs",
                "observedOutcome",
                "blockedReason",
            ],
        );
        normalized.push(Value::Object(item));
    }
    verification.insert("browserChecks".to_string(), Value::Array(normalized));
}

fn normalize_requirement_detail_evidence_machine_fields(
    object: &mut serde_json::Map<String, Value>,
    task: &TaskDefinition,
    detail_ids: &[String],
) {
    let verification_values = object
        .get("verificationResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let normalized = detail_ids
        .iter()
        .map(|detail_id| {
            let verification_ids = task
                .verification_intents
                .iter()
                .filter(|intent| {
                    intent
                        .requirement_detail_refs
                        .iter()
                        .any(|id| id == detail_id)
                })
                .map(|intent| intent.verification_id.clone())
                .collect::<Vec<_>>();
            let evidence = verification_values
                .iter()
                .filter(|verification| {
                    verification
                        .get("verificationId")
                        .and_then(Value::as_str)
                        .is_some_and(|id| verification_ids.iter().any(|expected| expected == id))
                })
                .collect::<Vec<_>>();
            let status = if !evidence.is_empty()
                && evidence.iter().all(|verification| {
                    verification.get("status").and_then(Value::as_str) == Some("passed")
                }) {
                "satisfied"
            } else if evidence.iter().any(|verification| {
                matches!(
                    verification.get("status").and_then(Value::as_str),
                    Some("failed" | "blocked" | "inconclusive")
                )
            }) {
                "partial"
            } else {
                "not_verified"
            };
            let evidence_refs = evidence
                .iter()
                .flat_map(|verification| {
                    verification
                        .pointer("/provenance/evidenceRefs")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .chain(
                            verification
                                .pointer("/provenance/changedFiles")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten(),
                        )
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                })
                .collect::<BTreeSet<_>>();
            let summary = evidence
                .iter()
                .filter_map(|verification| verification.get("summary").and_then(Value::as_str))
                .find(|summary| !summary.trim().is_empty())
                .unwrap_or("Evidence is derived from the task-scoped verification results.");
            json!({
                "detailId": detail_id,
                "status": status,
                "verificationIds": verification_ids,
                "evidenceRefs": evidence_refs.into_iter().collect::<Vec<_>>(),
                "summary": summary
            })
        })
        .collect::<Vec<_>>();
    object.insert(
        "requirementDetailEvidence".to_string(),
        Value::Array(normalized),
    );
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
    normalize_string_array_field(evidence, "checkedFields");
    normalize_string_array_field(evidence, "commandsRun");
    normalize_string_array_field(evidence, "unverifiedItems");
    if evidence
        .get("requirementRef")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        evidence.remove("requirementRef");
    }
    if evidence
        .get("runtimeProbeCleanup")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        evidence.remove("runtimeProbeCleanup");
    }
    let raw_checks = evidence
        .get("codeLevelChecks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if raw_checks.is_empty() {
        evidence.insert("codeLevelChecks".to_string(), json!([]));
    } else {
        let mut used = BTreeSet::new();
        let checks = requirement
            .required_code_level_checks
            .iter()
            .enumerate()
            .take(raw_checks.len())
            .map(|(index, required)| {
                let raw = ordered_machine_item(
                    &raw_checks,
                    index,
                    "checkId",
                    &required.check_id,
                    &mut used,
                    false,
                );
                let raw_object = raw.as_object();
                let mut check = serde_json::Map::new();
                check.insert("checkId".to_string(), json!(required.check_id));
                if let Some(contract_field) = &required.contract_field {
                    check.insert("contractField".to_string(), json!(contract_field));
                }
                check.insert(
                    "status".to_string(),
                    raw_object
                        .and_then(|value| value.get("status"))
                        .filter(|value| value.is_string())
                        .cloned()
                        .unwrap_or_else(|| json!("not_applicable")),
                );
                check.insert(
                    "evidence".to_string(),
                    raw_object
                        .and_then(|value| value.get("evidence"))
                        .filter(|value| value.is_string())
                        .cloned()
                        .unwrap_or_else(|| json!("")),
                );
                Value::Object(check)
            })
            .collect::<Vec<_>>();
        evidence.insert("codeLevelChecks".to_string(), Value::Array(checks));
    }
    retain_object_fields(
        evidence,
        &[
            "requirementRef",
            "checkedFields",
            "codeLevelChecks",
            "commandsRun",
            "unverifiedItems",
            "runtimeProbeCleanup",
        ],
    );
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
    if !closure_ids.is_empty() {
        self_check.insert("closureRequirementIds".to_string(), json!(closure_ids));
    }
    normalize_string_array_field(self_check, "evidenceRefs");
    retain_object_fields(
        self_check,
        &[
            "status",
            "dataBinding",
            "evidenceRefs",
            "closureRequirementIds",
        ],
    );
    if let Some(data_binding) = self_check
        .get_mut("dataBinding")
        .and_then(Value::as_object_mut)
    {
        retain_object_fields(data_binding, &["mode", "knownGaps"]);
        normalize_string_array_field(data_binding, "knownGaps");
    }
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
    let mut seen_verification_ids = BTreeSet::new();
    for verification in &result.verification_results {
        if !seen_verification_ids.insert(verification.verification_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_REF_INVALID",
                "verificationResults[].verificationId",
                "TaskResult verificationResults must contain each verification id at most once.",
            ));
        }
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

fn validate_verification_provenance(
    project_root: &Path,
    result: &TaskResult,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    for verification in &result.verification_results {
        if verification.status != "passed" {
            continue;
        }
        let traceable = verification.provenance.as_ref().is_some_and(|provenance| {
            !provenance.evidence_refs.is_empty()
                || !provenance.test_case_refs.is_empty()
                || (provenance
                    .command
                    .as_deref()
                    .is_some_and(|command| !command.trim().is_empty())
                    && provenance.exit_code.is_some())
                || verification
                    .browser_checks
                    .iter()
                    .any(|check| !check.artifact_refs.is_empty())
        });
        if !traceable {
            issues.push(issue(
                "TASK_RESULT_VERIFICATION_PROVENANCE_INVALID",
                "verificationResults[].provenance",
                "Passed verification must cite a concrete evidence ref, test case, browser artifact, or command with exit code.",
            ));
        }
        if verification.evidence_type == Some(VerificationEvidence::AutomatedTest)
            && verification
                .provenance
                .as_ref()
                .is_none_or(|provenance| provenance.test_case_refs.is_empty())
        {
            issues.push(issue(
                "TASK_RESULT_VERIFICATION_PROVENANCE_INVALID",
                "verificationResults[].provenance.testCaseRefs",
                "Automated test evidence must identify the test file, test case, or check id that produced the result; a build command alone is not test evidence.",
            ));
        }
        if let Some(provenance) = verification.provenance.as_ref() {
            for reference in &provenance.test_case_refs {
                if reference.contains('/') || reference.contains('\\') {
                    if !is_safe_project_relative_path(reference)
                        || !from_project_relative(project_root, reference)
                            .ok()
                            .is_some_and(|path| path.exists())
                    {
                        issues.push(issue(
                            "TASK_RESULT_VERIFICATION_PROVENANCE_INVALID",
                            "verificationResults[].provenance.testCaseRefs",
                            "Test case file references must point to existing project files.",
                        ));
                    }
                }
            }
        }
    }
}

fn validate_browser_verification_results(
    result: &TaskResult,
    browser_profile: Option<&BrowserVerificationProfile>,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(profile) = browser_profile else {
        return;
    };
    let expected = profile
        .checks
        .iter()
        .map(|check| (check.check_id.as_str(), check.verification_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    for verification in &result.verification_results {
        for check in &verification.browser_checks {
            if actual
                .insert(
                    check.check_id.as_str(),
                    verification.verification_id.as_str(),
                )
                .is_some()
            {
                issues.push(issue(
                    "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                    "verificationResults[].browserChecks[].checkId",
                    "Browser check ids must not be duplicated across verificationResults.",
                ));
            }
            if expected.get(check.check_id.as_str()).copied()
                != Some(verification.verification_id.as_str())
            {
                issues.push(issue(
                    "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                    "verificationResults[].browserChecks[].checkId",
                    "Each browser check must remain under the verificationId assigned by the MCP-derived browser profile.",
                ));
            }
            for artifact_ref in &check.artifact_refs {
                if !is_safe_project_relative_path(artifact_ref) {
                    issues.push(issue(
                        "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                        "verificationResults[].browserChecks[].artifactRefs",
                        "Browser artifact refs must use safe project-relative paths.",
                    ));
                }
            }
            match check.status {
                BrowserCheckStatus::Passed => {
                    if check.attempts == 0 {
                        issues.push(issue(
                            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                            "verificationResults[].browserChecks[].attempts",
                            "Passed browser checks must report at least one attempt.",
                        ));
                    }
                    if check.command.trim().is_empty() {
                        issues.push(issue(
                            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                            "verificationResults[].browserChecks[].command",
                            "Passed browser checks must report the exact command used.",
                        ));
                    }
                    if check.observed_outcome.trim().is_empty() {
                        issues.push(issue(
                            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                            "verificationResults[].browserChecks[].observedOutcome",
                            "Passed browser checks must report a concise observed outcome.",
                        ));
                    }
                    if check
                        .blocked_reason
                        .as_deref()
                        .is_some_and(|reason| !reason.trim().is_empty())
                    {
                        issues.push(issue(
                            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                            "verificationResults[].browserChecks[].blockedReason",
                            "Passed browser checks cannot retain a blocked reason.",
                        ));
                    }
                }
                BrowserCheckStatus::Blocked => {
                    if check
                        .blocked_reason
                        .as_deref()
                        .is_none_or(|reason| reason.trim().is_empty())
                    {
                        issues.push(issue(
                            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                            "verificationResults[].browserChecks[].blockedReason",
                            "Blocked browser checks must report a concrete environment or dependency reason.",
                        ));
                    }
                }
                BrowserCheckStatus::Failed => {
                    if check.observed_outcome.trim().is_empty() {
                        issues.push(issue(
                            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                            "verificationResults[].browserChecks[].observedOutcome",
                            "Failed browser checks must report the observed failure outcome.",
                        ));
                    }
                }
                BrowserCheckStatus::NotRun => {}
            }
        }
        let all_required_browser_checks_passed = verification.browser_checks.iter().all(|result| {
            profile
                .checks
                .iter()
                .find(|check| check.check_id == result.check_id)
                .is_none_or(|check| {
                    check.enforcement != contracts::BrowserEvidenceEnforcement::Required
                        || result.status == BrowserCheckStatus::Passed
                })
        });
        if verification.status == "passed"
            && !verification.browser_checks.is_empty()
            && !all_required_browser_checks_passed
        {
            issues.push(issue(
                "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
                "verificationResults[].status",
                "A verification result cannot be passed while one of its required browser checks is not passed.",
            ));
        }
    }
    let expected_ids = expected.keys().copied().collect::<BTreeSet<_>>();
    let actual_ids = actual.keys().copied().collect::<BTreeSet<_>>();
    if expected_ids != actual_ids {
        issues.push(issue(
            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
            "verificationResults[].browserChecks",
            "Browser checks must cover exactly the check ids in the MCP-derived browser profile.",
        ));
    }
    let required_non_passed = result
        .verification_results
        .iter()
        .flat_map(|verification| verification.browser_checks.iter())
        .filter(|result| {
            result.status != BrowserCheckStatus::Passed
                && profile.checks.iter().any(|check| {
                    check.check_id == result.check_id
                        && check.enforcement == contracts::BrowserEvidenceEnforcement::Required
                })
        })
        .collect::<Vec<_>>();
    let invalid_completed_browser_status = match result.status {
        TaskResultStatus::Completed => !required_non_passed.is_empty(),
        TaskResultStatus::CompletedWithNotes => required_non_passed
            .iter()
            .any(|check| check.status != BrowserCheckStatus::Blocked),
        TaskResultStatus::Failed | TaskResultStatus::Blocked => false,
    };
    if invalid_completed_browser_status {
        issues.push(issue(
            "TASK_RESULT_BROWSER_VERIFICATION_INVALID",
            "verificationResults[].browserChecks[].status",
            "completed must pass every required browser check. completed_with_notes may carry required checks only when they are explicitly environment-blocked; failed and not_run required checks remain incomplete product verification.",
        ));
    }
}

fn validate_implementation_obligation_results(
    project_root: &Path,
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if task.implementation_obligations.is_empty() {
        if !result.implementation_obligation_results.is_empty() {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults",
                "This task has no implementation obligations in its canonical TaskPlan.",
            ));
        }
        return;
    }

    let intent_ids = task
        .verification_intents
        .iter()
        .map(|intent| intent.verification_id.as_str())
        .collect::<BTreeSet<_>>();
    let passed_by_id = result
        .verification_results
        .iter()
        .filter(|verification| verification.status == "passed")
        .map(|verification| (verification.verification_id.as_str(), verification))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for obligation in &task.implementation_obligations {
        let Some(evidence) = result
            .implementation_obligation_results
            .iter()
            .find(|item| item.obligation_id == obligation.obligation_id)
        else {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults",
                &format!(
                    "Missing implementation result for required obligation {}.",
                    obligation.obligation_id
                ),
            ));
            continue;
        };
        if !seen.insert(evidence.obligation_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults[].obligationId",
                "Each implementation obligation must have exactly one result.",
            ));
        }
        if !matches!(
            evidence.status.as_str(),
            "satisfied" | "partial" | "blocked" | "not_verified"
        ) {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults[].status",
                "Implementation obligation status must be satisfied, partial, blocked, or not_verified.",
            ));
        }
        for verification_id in &evidence.verification_ids {
            if !intent_ids.contains(verification_id.as_str())
                || (!obligation.verification_ids.is_empty()
                    && !obligation
                        .verification_ids
                        .iter()
                        .any(|expected| expected == verification_id))
            {
                issues.push(issue(
                    "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                    "implementationObligationResults[].verificationIds",
                    "Implementation evidence must reference verification ids assigned to the same obligation.",
                ));
            }
        }
        if evidence.status == "satisfied" {
            if evidence.evidence_refs.is_empty() {
                issues.push(issue(
                    "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                    "implementationObligationResults[].evidenceRefs",
                    "Satisfied implementation obligations must cite concrete evidence references.",
                ));
            }
            let requires_behavioral_proof = matches!(
                obligation.kind.as_str(),
                "persistence_mapping"
                    | "persistence_implementation"
                    | "data_access_framework"
                    | "interface_contract"
                    | "api_binding"
                    | "state_transition"
                    | "state_machine"
                    | "business_rule"
                    | "authentication_authorization"
                    | "async_processing"
                    | "cache_policy"
                    | "external_integration"
                    | "resilience_policy"
                    | "entity_lifecycle"
            );
            if requires_behavioral_proof
                && !evidence.verification_ids.iter().any(|verification_id| {
                    passed_by_id
                        .get(verification_id.as_str())
                        .and_then(|verification| verification.evidence_type)
                        .is_some_and(|evidence_type| {
                            matches!(
                                evidence_type,
                                VerificationEvidence::AutomatedTest
                                    | VerificationEvidence::RuntimeApiCheck
                                    | VerificationEvidence::BrowserAutomation
                            )
                        })
                })
            {
                issues.push(issue(
                    "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                    "implementationObligationResults[].verificationIds",
                    "This obligation requires passed behavioral evidence; a build or reference-read result cannot satisfy it.",
                ));
            }
            validate_obligation_evidence_refs(project_root, result, evidence, issues);
            if evidence.summary.trim().is_empty() {
                issues.push(issue(
                    "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                    "implementationObligationResults[].summary",
                    "Satisfied implementation obligations must explain the concrete implementation evidence; a status without a summary is not a completion claim.",
                ));
            }
        }
    }

    for evidence in &result.implementation_obligation_results {
        if !task
            .implementation_obligations
            .iter()
            .any(|obligation| obligation.obligation_id == evidence.obligation_id)
        {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults[].obligationId",
                "TaskResult must not invent implementation obligation ids.",
            ));
        }
    }

    if matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        for obligation in task
            .implementation_obligations
            .iter()
            .filter(|item| item.required)
        {
            let satisfied = result
                .implementation_obligation_results
                .iter()
                .find(|item| item.obligation_id == obligation.obligation_id)
                .is_some_and(|item| item.status == "satisfied");
            if !satisfied {
                issues.push(issue(
                    "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INCOMPLETE",
                    "status",
                    &format!(
                        "Task cannot be completed while required obligation {} is not satisfied.",
                        obligation.obligation_id
                    ),
                ));
            }
        }
    }
}

fn validate_obligation_evidence_refs(
    project_root: &Path,
    result: &TaskResult,
    evidence: &contracts::TaskImplementationObligationResult,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let changed_files = result.changed_files.iter().collect::<BTreeSet<_>>();
    let mut has_changed_source = false;
    for reference in &evidence.evidence_refs {
        if !is_safe_project_relative_path(reference) {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults[].evidenceRefs",
                "Implementation evidence references must be safe project-relative paths.",
            ));
            continue;
        }
        if is_generated_output_path(reference) {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults[].evidenceRefs",
                "Implementation evidence must cite source or test files, not build output, caches, reports, or dependency directories.",
            ));
        }
        if changed_files.contains(reference) {
            has_changed_source = true;
        } else if !from_project_relative(project_root, reference)
            .ok()
            .is_some_and(|path| path.is_file())
        {
            issues.push(issue(
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
                "implementationObligationResults[].evidenceRefs",
                "Implementation evidence references must point to existing project files or the task's changedFiles.",
            ));
        }
    }
    if !has_changed_source {
        issues.push(issue(
            "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID",
            "implementationObligationResults[].evidenceRefs",
            "Satisfied implementation obligations must cite at least one file changed by this task; inherited files alone cannot prove ownership or completion.",
        ));
    }
}

fn validate_stack_conformance(
    project_root: &Path,
    result: &TaskResult,
    task: &TaskDefinition,
    engineering_quality_requirements: &[contracts::EngineeringQualityRequirement],
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(requirement) = engineering_quality_requirements.iter().find(|requirement| {
        requirement
            .applies_to_task_ids
            .iter()
            .any(|id| id == &task.task_id)
    }) else {
        return;
    };
    let Some(persistence_obligation) = task
        .implementation_obligations
        .iter()
        .find(|obligation| obligation.kind == "persistence_mapping")
    else {
        return;
    };
    let files = result
        .changed_files
        .iter()
        .filter_map(|file| from_project_relative(project_root, file).ok())
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect::<Vec<_>>();
    let content = files.join("\n").to_ascii_lowercase();
    if content.contains("inmemory")
        || content.contains("in_memory")
        || content.contains("hashmap<")
        || content.contains("hashmap::")
    {
        issues.push(issue(
            "TASK_RESULT_STACK_CONFORMANCE_INVALID",
            "changedFiles",
            "A task with an accepted durable persistence contract must not complete with an in-memory HashMap repository or equivalent transient store.",
        ));
    }

    let required_signals = requirement
        .stack_signals
        .iter()
        .filter(|(key, _)| matches!(key.as_str(), "persistence" | "dataAccess"))
        .map(|(key, value)| (key.clone(), value.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let missing_signals = required_signals
        .iter()
        .filter_map(|(key, value)| {
            let markers = if key == "dataAccess" {
                if value.contains("mybatis") {
                    vec!["mybatis", "basemapper", "@tableid"]
                } else if value.contains("jpa") || value.contains("hibernate") {
                    vec![
                        "jakarta.persistence",
                        "javax.persistence",
                        "@entity",
                        "jparepository",
                    ]
                } else if value.contains("prisma") {
                    vec!["prisma"]
                } else if value.contains("sqlalchemy") || value.contains("sqlmodel") {
                    vec!["sqlalchemy", "sqlmodel"]
                } else if value.contains("entity framework") || value.contains("ef core") {
                    vec!["entityframework", "dbcontext"]
                } else if value.contains("gorm") {
                    vec!["gorm"]
                } else {
                    vec!["repository", "query", "sql"]
                }
            } else if value.contains("sqlite") {
                vec!["sqlite", "jdbc:sqlite"]
            } else if value.contains("postgres") {
                vec!["postgres", "postgresql"]
            } else if value.contains("mysql") {
                vec!["mysql"]
            } else {
                Vec::new()
            };
            if markers.is_empty() || markers.iter().any(|marker| content.contains(*marker)) {
                None
            } else {
                Some(format!("{key}={value}"))
            }
        })
        .collect::<Vec<_>>();
    if !missing_signals.is_empty() {
        issues.push(issue(
            "TASK_RESULT_STACK_CONFORMANCE_INVALID",
            "changedFiles",
            &format!(
                "Persistence obligation {} must show accepted provider evidence for {} in changed source or configuration files.",
                persistence_obligation.obligation_id,
                missing_signals.join(", ")
            ),
        ));
    }
}

pub(crate) fn is_generated_output_path(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            "target"
                | "dist"
                | "build"
                | "coverage"
                | ".cache"
                | "cache"
                | "reports"
                | "report"
                | "logs"
                | "log"
                | "test-results"
                | "playwright-report"
                | ".playwright"
                | ".pytest_cache"
                | ".next"
                | "out"
                | "node_modules"
        )
    })
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
    let passed_verification_ids = result
        .verification_results
        .iter()
        .filter(|verification| verification.status == "passed")
        .map(|verification| verification.verification_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_detail_ids = BTreeSet::new();
    for evidence in &result.requirement_detail_evidence {
        if !seen_detail_ids.insert(evidence.detail_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence[].detailId",
                "Requirement detail evidence must contain each assigned detail id at most once.",
            ));
        }
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
        if evidence.status == "satisfied"
            && !evidence
                .verification_ids
                .iter()
                .all(|id| passed_verification_ids.contains(id.as_str()))
        {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence[].verificationIds",
                "Satisfied requirement detail evidence must link only to passed verification results.",
            ));
        }
        if evidence.status == "satisfied" && evidence.evidence_refs.is_empty() {
            issues.push(issue(
                "TASK_RESULT_DETAIL_EVIDENCE_INVALID",
                "requirementDetailEvidence[].evidenceRefs",
                "Satisfied requirement detail evidence must cite at least one concrete evidence reference.",
            ));
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

fn validate_architecture_quality_evidence(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if matches!(
        result.status,
        TaskResultStatus::Failed | TaskResultStatus::Blocked
    ) {
        return;
    }
    if !architecture_quality_evidence_applies(task) {
        if !result.architecture_quality_evidence.is_empty() {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence",
                "TaskResult must not include architectureQualityEvidence when the task has no architectureQualityRequirementRefs.",
            ));
        }
        return;
    }
    let requirement_refs = task
        .architecture_quality_requirement_refs
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let verification_ids = task
        .verification_intents
        .iter()
        .map(|intent| intent.verification_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let passed_verification_ids = result
        .verification_results
        .iter()
        .filter(|verification| verification.status == "passed")
        .map(|verification| verification.verification_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_requirement_ids = BTreeSet::new();
    for evidence in &result.architecture_quality_evidence {
        if !seen_requirement_ids.insert(evidence.requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence[].requirementId",
                "Architecture quality evidence must contain each requirement id at most once.",
            ));
        }
        if !requirement_refs.contains(evidence.requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence[].requirementId",
                "architectureQualityEvidence.requirementId must reference task.architectureQualityRequirementRefs.",
            ));
        }
        if evidence.verification_ids.is_empty() {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence[].verificationIds",
                "architectureQualityEvidence must link to verification results.",
            ));
        }
        for verification_id in &evidence.verification_ids {
            if !verification_ids.contains(verification_id.as_str()) {
                issues.push(issue(
                    "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                    "architectureQualityEvidence[].verificationIds",
                    "architectureQualityEvidence verificationIds must reference task verification intents.",
                ));
            }
        }
        if evidence.status == "satisfied"
            && !evidence
                .verification_ids
                .iter()
                .all(|id| passed_verification_ids.contains(id.as_str()))
        {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence[].verificationIds",
                "Satisfied architecture quality evidence must link only to passed verification results.",
            ));
        }
        if evidence.summary.trim().is_empty() {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence[].summary",
                "architectureQualityEvidence summary must explain how the task respected the referenced architecture quality requirement.",
            ));
        }
    }
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        return;
    }
    for requirement_id in &task.architecture_quality_requirement_refs {
        let Some(evidence) = result
            .architecture_quality_evidence
            .iter()
            .find(|evidence| &evidence.requirement_id == requirement_id)
        else {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence",
                "Completed TaskResult must include architectureQualityEvidence for every assigned architecture quality requirement.",
            ));
            continue;
        };
        if evidence.status != "satisfied" {
            issues.push(issue(
                "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID",
                "architectureQualityEvidence[].status",
                "Completed or completed_with_notes TaskResult architectureQualityEvidence must be satisfied.",
            ));
        }
    }
}

fn validate_api_contract_evidence(
    result: &TaskResult,
    task: &TaskDefinition,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if matches!(
        result.status,
        TaskResultStatus::Failed | TaskResultStatus::Blocked
    ) {
        return;
    }
    if !api_contract_evidence_applies(task) {
        if !result.api_contract_evidence.is_empty() {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence",
                "TaskResult must not include apiContractEvidence when the task has no apiContractRequirementRefs.",
            ));
        }
        return;
    }
    let requirement_refs = task
        .api_contract_requirement_refs
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let task_interface_refs = task
        .write_boundary
        .artifact_refs
        .interfaces
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let verification_ids = task
        .verification_intents
        .iter()
        .map(|intent| intent.verification_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let passed_verification_ids = result
        .verification_results
        .iter()
        .filter(|verification| verification.status == "passed")
        .map(|verification| verification.verification_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_requirement_ids = BTreeSet::new();
    for evidence in &result.api_contract_evidence {
        if !seen_requirement_ids.insert(evidence.requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].requirementId",
                "API contract evidence must contain each requirement id at most once.",
            ));
        }
        if !requirement_refs.contains(evidence.requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].requirementId",
                "apiContractEvidence.requirementId must reference task.apiContractRequirementRefs.",
            ));
        }
        if evidence.verification_ids.is_empty() {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].verificationIds",
                "apiContractEvidence must link to verification results.",
            ));
        }
        for verification_id in &evidence.verification_ids {
            if !verification_ids.contains(verification_id.as_str()) {
                issues.push(issue(
                    "TASK_RESULT_API_CONTRACT_INVALID",
                    "apiContractEvidence[].verificationIds",
                    "apiContractEvidence verificationIds must reference task verification intents.",
                ));
            }
        }
        if evidence.status == "satisfied"
            && !evidence
                .verification_ids
                .iter()
                .all(|id| passed_verification_ids.contains(id.as_str()))
        {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].verificationIds",
                "Satisfied API contract evidence must link only to passed verification results.",
            ));
        }
        for interface_ref in &evidence.interface_refs {
            if !task_interface_refs.is_empty()
                && !task_interface_refs.contains(interface_ref.as_str())
            {
                issues.push(issue(
                    "TASK_RESULT_API_CONTRACT_INVALID",
                    "apiContractEvidence[].interfaceRefs",
                    "apiContractEvidence interfaceRefs must reference task writeBoundary interface refs.",
                ));
            }
        }
        if evidence.summary.trim().is_empty() {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].summary",
                "apiContractEvidence summary must explain how the task implemented or preserved the referenced API contract.",
            ));
        }
    }
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        return;
    }
    for requirement_id in &task.api_contract_requirement_refs {
        let Some(evidence) = result
            .api_contract_evidence
            .iter()
            .find(|evidence| &evidence.requirement_id == requirement_id)
        else {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence",
                "Completed TaskResult must include apiContractEvidence for every assigned API contract requirement.",
            ));
            continue;
        };
        if evidence.status != "satisfied" {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].status",
                "Completed or completed_with_notes TaskResult apiContractEvidence must be satisfied.",
            ));
        }
        if !evidence.known_gaps.is_empty() {
            issues.push(issue(
                "TASK_RESULT_API_CONTRACT_INVALID",
                "apiContractEvidence[].knownGaps",
                "Completed or completed_with_notes TaskResult apiContractEvidence cannot contain known gaps.",
            ));
        }
    }
}

fn validate_code_quality_evidence(
    result: &TaskResult,
    task: &TaskDefinition,
    code_quality_requirements: &[CodeQualityRequirement],
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if matches!(
        result.status,
        TaskResultStatus::Failed | TaskResultStatus::Blocked
    ) {
        return;
    }
    if !code_quality_evidence_applies(task) {
        if !result.code_quality_evidence.is_empty() {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence",
                "TaskResult must not include codeQualityEvidence when the task has no codeQualityRequirementRefs.",
            ));
        }
        return;
    }
    let requirement_refs = task
        .code_quality_requirement_refs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let requirements_by_id = code_quality_requirements
        .iter()
        .filter(|requirement| requirement_refs.contains(requirement.requirement_id.as_str()))
        .map(|requirement| (requirement.requirement_id.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    for requirement_id in &task.code_quality_requirement_refs {
        if !requirements_by_id.contains_key(requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "sourceContext.codeQualityExecutionContext",
                "TaskResult validation requires sourceContext.codeQualityExecutionContext for every task.codeQualityRequirementRefs item.",
            ));
        }
    }
    let verification_ids = task
        .verification_intents
        .iter()
        .map(|intent| intent.verification_id.as_str())
        .collect::<BTreeSet<_>>();
    let passed_verification_ids = result
        .verification_results
        .iter()
        .filter(|verification| verification.status == "passed")
        .map(|verification| verification.verification_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_requirement_ids = BTreeSet::new();
    for evidence in &result.code_quality_evidence {
        if !seen_requirement_ids.insert(evidence.requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].requirementId",
                "Code quality evidence must contain each requirement id at most once.",
            ));
        }
        if !requirement_refs.contains(evidence.requirement_id.as_str()) {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].requirementId",
                "codeQualityEvidence.requirementId must reference task.codeQualityRequirementRefs.",
            ));
        }
        if evidence.reference_groups_checked.is_empty() {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].referenceGroupsChecked",
                "codeQualityEvidence must record the selected language/framework reference groups checked for this task.",
            ));
        }
        if let Some(requirement) = requirements_by_id.get(evidence.requirement_id.as_str()) {
            validate_code_quality_reference_groups(evidence, requirement, issues);
            validate_code_quality_reference_files(evidence, requirement, issues);
        }
        if evidence.verification_ids.is_empty() {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].verificationIds",
                "codeQualityEvidence must link to verification results.",
            ));
        }
        for verification_id in &evidence.verification_ids {
            if !verification_ids.contains(verification_id.as_str()) {
                issues.push(issue(
                    "TASK_RESULT_CODE_QUALITY_INVALID",
                    "codeQualityEvidence[].verificationIds",
                    "codeQualityEvidence verificationIds must reference task verification intents.",
                ));
            }
        }
        if evidence.status == "satisfied"
            && !evidence
                .verification_ids
                .iter()
                .all(|id| passed_verification_ids.contains(id.as_str()))
        {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].verificationIds",
                "Satisfied code quality evidence must link only to passed verification results.",
            ));
        }
        if evidence.summary.trim().is_empty() {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].summary",
                "codeQualityEvidence summary must explain how changed files followed selected code references and repository style.",
            ));
        }
    }
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        return;
    }
    for requirement_id in &task.code_quality_requirement_refs {
        let Some(evidence) = result
            .code_quality_evidence
            .iter()
            .find(|evidence| &evidence.requirement_id == requirement_id)
        else {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence",
                "Completed TaskResult must include codeQualityEvidence for every assigned code quality requirement.",
            ));
            continue;
        };
        if evidence.status != "satisfied" {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].status",
                "Completed or completed_with_notes TaskResult codeQualityEvidence must be satisfied.",
            ));
        }
        if !evidence.known_gaps.is_empty() {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].knownGaps",
                "Completed or completed_with_notes TaskResult codeQualityEvidence cannot contain known gaps.",
            ));
        }
    }
}

fn validate_code_quality_reference_groups(
    evidence: &CodeQualityEvidence,
    requirement: &CodeQualityRequirement,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if requirement.reference_groups.is_empty() {
        return;
    }
    let expected = requirement
        .reference_groups
        .iter()
        .map(|(language, groups)| {
            (
                language.as_str(),
                groups.iter().map(String::as_str).collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let checked = evidence
        .reference_groups_checked
        .iter()
        .map(|(language, groups)| {
            (
                language.as_str(),
                groups.iter().map(String::as_str).collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (language, expected_groups) in &expected {
        let Some(checked_groups) = checked.get(language) else {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].referenceGroupsChecked",
                "codeQualityEvidence.referenceGroupsChecked must include every selected language/framework group key from the assigned code quality requirement.",
            ));
            continue;
        };
        for group in expected_groups {
            if !checked_groups.contains(group) {
                issues.push(issue(
                    "TASK_RESULT_CODE_QUALITY_INVALID",
                    "codeQualityEvidence[].referenceGroupsChecked",
                    "codeQualityEvidence.referenceGroupsChecked must include every selected group from the assigned code quality requirement.",
                ));
            }
        }
    }
    for (language, checked_groups) in &checked {
        let Some(expected_groups) = expected.get(language) else {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].referenceGroupsChecked",
                "codeQualityEvidence.referenceGroupsChecked must not add language/framework group keys that were not selected by the assigned code quality requirement.",
            ));
            continue;
        };
        for group in checked_groups {
            if !expected_groups.contains(group) {
                issues.push(issue(
                    "TASK_RESULT_CODE_QUALITY_INVALID",
                    "codeQualityEvidence[].referenceGroupsChecked",
                    "codeQualityEvidence.referenceGroupsChecked must not add groups that were not selected by the assigned code quality requirement.",
                ));
            }
        }
    }
}

fn validate_code_quality_reference_files(
    evidence: &CodeQualityEvidence,
    requirement: &CodeQualityRequirement,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let expected_paths = requirement
        .reference_load_plan
        .iter()
        .map(|item| item.path.as_str())
        .collect::<BTreeSet<_>>();
    if expected_paths.is_empty() {
        if !evidence.reference_files_checked.is_empty() {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].referenceFilesChecked",
                "codeQualityEvidence.referenceFilesChecked must be empty when the assigned code quality requirement has no referenceLoadPlan.",
            ));
        }
        return;
    }
    if evidence.reference_files_checked.is_empty() {
        issues.push(issue(
            "TASK_RESULT_CODE_QUALITY_INVALID",
            "codeQualityEvidence[].referenceFilesChecked",
            "codeQualityEvidence.referenceFilesChecked must list the files from sourceContext.codeQualityExecutionContext[].referenceLoadPlan that were read for this task.",
        ));
        return;
    }
    let checked_paths = evidence
        .reference_files_checked
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for expected_path in &expected_paths {
        if !checked_paths.contains(expected_path) {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].referenceFilesChecked",
                "codeQualityEvidence.referenceFilesChecked must include every path selected by sourceContext.codeQualityExecutionContext[].referenceLoadPlan.",
            ));
        }
    }
    for checked_path in &checked_paths {
        if !expected_paths.contains(checked_path) {
            issues.push(issue(
                "TASK_RESULT_CODE_QUALITY_INVALID",
                "codeQualityEvidence[].referenceFilesChecked",
                "codeQualityEvidence.referenceFilesChecked must not include files outside sourceContext.codeQualityExecutionContext[].referenceLoadPlan.",
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
    if let Some(cleanup) = evidence.runtime_probe_cleanup.as_deref() {
        let invalid_cleanup = cleanup.trim().is_empty();
        if invalid_cleanup {
            issues.push(issue(
                "TASK_RESULT_RUNTIME_EVIDENCE_INVALID",
                "runtimeDeliveryEvidence.runtimeProbeCleanup",
                "runtimeDeliveryEvidence.runtimeProbeCleanup must be null or a non-empty cleanup outcome.",
            ));
        }
    }
    if evidence
        .commands_run
        .iter()
        .any(|command| command.trim().is_empty())
    {
        issues.push(issue(
            "TASK_RESULT_RUNTIME_EVIDENCE_INVALID",
            "runtimeDeliveryEvidence.commandsRun",
            "runtimeDeliveryEvidence.commandsRun must be an array of non-empty command strings.",
        ));
    }
    for field in &requirement.affected_contract_fields {
        if !evidence.checked_fields.contains(field) {
            issues.push(issue(
                "TASK_RESULT_REF_INVALID",
                "runtimeDeliveryEvidence.checkedFields",
                "runtimeDeliveryEvidence.checkedFields must include every affected runtime contract field.",
            ));
        }
    }
    let result_check_ids = evidence
        .code_level_checks
        .iter()
        .filter_map(|check| check.check_id.as_deref())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
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
    let covered = self_check
        .closure_requirement_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for closure_id in closure_ids {
        if !covered.contains(&closure_id) {
            issues.push(issue(
                "TASK_RESULT_WORKFLOW_CLOSURE_INVALID",
                "frontendExperienceSelfCheck.closureRequirementIds",
                "frontendExperienceSelfCheck must cover every required workflow closure id.",
            ));
        }
    }
    if self_check.status == "satisfied"
        && (self_check.data_binding.mode != "wired"
            || !self_check.data_binding.known_gaps.is_empty()
            || self_check.evidence_refs.is_empty()
            || self_check
                .evidence_refs
                .iter()
                .any(|reference| reference.trim().is_empty()))
    {
        issues.push(issue(
            "TASK_RESULT_WORKFLOW_CLOSURE_INVALID",
            "frontendExperienceSelfCheck.dataBinding",
            "Satisfied frontendExperienceSelfCheck requires wired dataBinding, no known gaps, and at least one concrete evidence ref.",
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
    let frontend_requirement = task.frontend_experience_requirement.as_ref();
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
    let surface_contract = frontend_requirement
        .and_then(|requirement| {
            requirement.pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
        })
        .unwrap_or(&Value::Null);
    if surface_contract.is_object() {
        if let Some(requirement) = frontend_requirement {
            validate_surface_decision_contract_evidence(
                &self_check,
                requirement,
                surface_contract,
                issues,
            );
            validate_design_token_evidence(&self_check, requirement, issues);
        }
    } else {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract",
            "Frontend quality validation requires the task-scoped uiSurfaceDecisionContract in uiProductionBrief.",
        ));
    }
    let completed = matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    );
    let self_check_status = self_check.get("status").and_then(Value::as_str);
    if self_check_status == Some("satisfied")
        && non_empty_string_array_at(&self_check, "evidenceRefs").is_empty()
    {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.evidenceRefs",
            "Satisfied frontendQualitySelfCheck must cite at least one concrete evidence ref so Review can trace the accepted quality result.",
        ));
    }
    let violations = self_check
        .pointer("/contentBoundaryEvidence/forbiddenContentViolations")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    let gaps = self_check
        .get("knownGaps")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    if completed && self_check_status != Some("satisfied") {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.status",
            "Completed or completed_with_notes frontend quality tasks must submit satisfied frontendQualitySelfCheck; use blocked or failed when quality evidence cannot be completed.",
        ));
    }
    if self_check_status == Some("satisfied") || completed {
        if violations > 0 || gaps > 0 {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.status",
                "Completed or satisfied frontendQualitySelfCheck cannot contain forbidden content violations or known gaps.",
            ));
        }
    }
}

fn validate_surface_decision_contract_evidence(
    self_check: &Value,
    requirement: &Value,
    surface_contract: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let expected_ref = requirement
        .get("uiSurfaceDecisionContractRef")
        .and_then(Value::as_str)
        .or_else(|| surface_contract.get("contractRef").and_then(Value::as_str));
    if let Some(expected_ref) = expected_ref {
        if self_check
            .get("surfaceDecisionContractRef")
            .and_then(Value::as_str)
            != Some(expected_ref)
        {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                "frontendQualitySelfCheck.surfaceDecisionContractRef",
                "frontendQualitySelfCheck.surfaceDecisionContractRef must match the task uiSurfaceDecisionContractRef.",
            ));
        }
    }

    validate_surface_contract_evidence_array(
        self_check,
        "surfaceRegionEvidence",
        object_array_id_set(surface_contract, "regionsInScope", "regionId"),
        "region",
        issues,
    );
    validate_surface_contract_evidence_array(
        self_check,
        "surfaceActionEvidence",
        object_array_id_set(surface_contract, "actionsInScope", "actionId"),
        "action",
        issues,
    );
    validate_surface_contract_evidence_array(
        self_check,
        "surfaceStateEvidence",
        object_array_id_set(surface_contract, "statesInScope", "state"),
        "state",
        issues,
    );
    validate_surface_contract_evidence_array(
        self_check,
        "surfaceQualityRuleEvidence",
        object_array_id_set(surface_contract, "qualityRulesInScope", "ruleId"),
        "quality rule",
        issues,
    );
    validate_content_boundary_evidence(self_check, issues);
    validate_reference_plan_files_checked(self_check, requirement, issues);
}

fn validate_surface_contract_evidence_array(
    self_check: &Value,
    field: &str,
    expected_ids: BTreeSet<String>,
    label: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    const VALID_STATUSES: &[&str] = &["satisfied", "partial", "missing", "blocked_by_environment"];

    if expected_ids.is_empty() {
        return;
    }
    let Some(items) = self_check.get(field).and_then(Value::as_array) else {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            &format!("frontendQualitySelfCheck.{field}"),
            &format!(
                "frontendQualitySelfCheck.{field} must prove every task-scoped UI surface {label}."
            ),
        ));
        return;
    };
    if items.is_empty() {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            &format!("frontendQualitySelfCheck.{field}"),
            &format!("frontendQualitySelfCheck.{field} must not be empty when the surface contract declares task-scoped {label}s."),
        ));
        return;
    }
    let overall_satisfied = self_check.get("status").and_then(Value::as_str) == Some("satisfied");
    let mut seen = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].id"),
                &format!("Each frontendQualitySelfCheck.{field} entry must include the task-scoped {label} id."),
            ));
            continue;
        };
        if !seen.insert(id.to_string()) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].id"),
                &format!(
                    "frontendQualitySelfCheck.{field} must not duplicate {label} evidence ids."
                ),
            ));
        }
        if !expected_ids.contains(id) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].id"),
                &format!("frontendQualitySelfCheck.{field} cannot invent {label} ids outside the task-scoped uiSurfaceDecisionContract."),
            ));
        }
        let status = item
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !VALID_STATUSES.contains(&status) {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].status"),
                &format!("frontendQualitySelfCheck.{field}.status must be one of satisfied, partial, missing, or blocked_by_environment."),
            ));
        }
        let evidence_present = item
            .get("evidence")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|evidence| !evidence.is_empty());
        if !evidence_present {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].evidence"),
                &format!("Each frontendQualitySelfCheck.{field} entry must include concrete evidence for the {label}."),
            ));
        }
        if status == "satisfied" && string_array_at(item, "files").is_empty() {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].files"),
                &format!("Satisfied frontendQualitySelfCheck.{field} entries must cite concrete UI files."),
            ));
        }
        if matches!(status, "satisfied" | "blocked_by_environment")
            && value_field_contains_placeholder(item)
        {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}]"),
                &format!("Satisfied or environment-blocked frontendQualitySelfCheck.{field} entries must replace template placeholders with concrete evidence."),
            ));
        }
        if overall_satisfied && matches!(status, "partial" | "missing" | "blocked_by_environment") {
            issues.push(issue(
                "TASK_RESULT_FRONTEND_QUALITY_INVALID",
                &format!("frontendQualitySelfCheck.{field}[{index}].status"),
                &format!("frontendQualitySelfCheck.status cannot be satisfied while {label} evidence is partial, missing, or environment-blocked."),
            ));
        }
    }
    if !expected_ids.is_subset(&seen) {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            &format!("frontendQualitySelfCheck.{field}"),
            &format!("frontendQualitySelfCheck.{field} must include every task-scoped uiSurfaceDecisionContract {label} id."),
        ));
    }
}

fn validate_content_boundary_evidence(
    self_check: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(content) = self_check.get("contentBoundaryEvidence") else {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.contentBoundaryEvidence",
            "frontendQualitySelfCheck.contentBoundaryEvidence is required when a uiSurfaceDecisionContract is present.",
        ));
        return;
    };
    if content.get("checked").and_then(Value::as_bool) != Some(true) {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.contentBoundaryEvidence.checked",
            "frontendQualitySelfCheck.contentBoundaryEvidence.checked must be true after checking the surface content boundary.",
        ));
    }
    let evidence_present = content
        .get("evidence")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|evidence| !evidence.is_empty());
    if !evidence_present {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.contentBoundaryEvidence.evidence",
            "frontendQualitySelfCheck.contentBoundaryEvidence.evidence must explain how user-visible content respected the contract boundary.",
        ));
    }
    let violations = content
        .get("forbiddenContentViolations")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    if self_check.get("status").and_then(Value::as_str) == Some("satisfied") && violations > 0 {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.contentBoundaryEvidence.forbiddenContentViolations",
            "Satisfied frontendQualitySelfCheck cannot contain content boundary violations.",
        ));
    }
}

fn validate_reference_plan_files_checked(
    self_check: &Value,
    requirement: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let expected_files = reference_plan_paths(requirement);
    if expected_files.is_empty() {
        return;
    }
    let checked_files = string_set_from_array(self_check, "referencePlanFilesChecked");
    if !expected_files.is_subset(&checked_files) {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.referencePlanFilesChecked",
            "frontendQualitySelfCheck.referencePlanFilesChecked must include every task-scoped styleAssetPlan.referencePlan path read for the UI task.",
        ));
    }
}

fn validate_design_token_evidence(
    self_check: &Value,
    requirement: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let plan = requirement
        .pointer("/executionGuidance/styleAssetPlan/designTokenAssetPlan")
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
            "designTokenEvidence.strategyUsed must match task.frontendExperienceRequirement.executionGuidance.styleAssetPlan.designTokenAssetPlan.strategy.",
        ));
    }
    let expected_template = plan.get("templateId").unwrap_or(&Value::Null);
    let actual_template = evidence.get("templateIdUsed").unwrap_or(&Value::Null);
    if expected_template != actual_template {
        issues.push(issue(
            "TASK_RESULT_FRONTEND_QUALITY_INVALID",
            "frontendQualitySelfCheck.designTokenEvidence.templateIdUsed",
            "designTokenEvidence.templateIdUsed must match task.frontendExperienceRequirement.executionGuidance.styleAssetPlan.designTokenAssetPlan.templateId.",
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
    if matches!(
        task.task_kind,
        TaskKind::VerificationIncrement | TaskKind::BrowserQualityClosure
    ) {
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

fn non_empty_string_array_at(value: &Value, field: &str) -> Vec<String> {
    string_array_at(value, field)
        .into_iter()
        .filter(|item| !item.trim().is_empty())
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
        VerificationEvidence::BrowserAutomation => "browser_automation",
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
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    run: &contracts::TaskPlanRun,
    result: &TaskResult,
    request_ref: &str,
    result_ref: &str,
) -> Result<Option<RouteAction>, state::store::StateError> {
    if let Some(next_repair) = queued_execution_repair_action(
        project_root,
        delivery_id,
        phase_id,
        &result.task_id,
        request_ref,
    )? {
        return Ok(Some(next_repair));
    }
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

fn queued_execution_repair_action(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    completed_task_id: &str,
    request_ref: &str,
) -> Result<Option<RouteAction>, state::store::StateError> {
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
    if phase
        .latest_refs
        .get("taskExecutionRequestRef")
        .map(String::as_str)
        != Some(request_ref)
    {
        return Ok(None);
    }
    let Some(action) = phase.next_action.as_ref() else {
        return Ok(None);
    };
    if action.kind != RouteActionKind::ExecutionRepair
        || action.source != "delivery_execution_repair"
    {
        return Ok(None);
    }
    let Some(details) = action.details.as_ref() else {
        return Ok(None);
    };
    if details.get("currentTargetTaskId").and_then(Value::as_str) != Some(completed_task_id) {
        return Ok(None);
    }
    let mut pending_target_task_ids = string_array_at(details, "pendingTargetTaskIds");
    let mut seen = BTreeSet::new();
    pending_target_task_ids
        .retain(|task_id| task_id != completed_task_id && seen.insert(task_id.clone()));
    if pending_target_task_ids.is_empty() {
        return Ok(None);
    }
    let origin = details
        .get("origin")
        .and_then(Value::as_str)
        .unwrap_or("review_result")
        .to_string();
    let source_ref = details
        .get("sourceRef")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| action.request_ref.clone());
    let finding_refs = string_array_at(details, "findingRefs");
    let mut next_details = json!({
        "origin": origin,
        "targetTaskIds": pending_target_task_ids
    });
    if let Some(source_ref) = source_ref.as_ref() {
        next_details["sourceRef"] = json!(source_ref);
    }
    if !finding_refs.is_empty() {
        next_details["findingRefs"] = json!(finding_refs);
    }
    Ok(Some(RouteAction {
        kind: RouteActionKind::ExecutionRepair,
        source: "delivery_execution_repair_queue".to_string(),
        reason: format!("{origin}_repair_next_target"),
        prompt: None,
        accepted_responses: vec![],
        request_ref: source_ref,
        details: Some(next_details),
        target_phase_id: None,
    }))
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
    blocked_output: Value,
    submitted_result: Value,
    previous_changed_files: Vec<String>,
    code_quality_requirements: Vec<CodeQualityRequirement>,
    browser_profile: Option<BrowserVerificationProfile>,
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
    materialize_task_result_repair(
        input,
        &delivery_id,
        &phase_id,
        input.request_ref.clone(),
        target_file,
        issues,
        context,
    )
}

pub(crate) fn refresh_stale_task_result_repair_action(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let request_id = request_id_from_ref(request_ref)?;
    let repair_fields = read_request_fields_chunked(
        project_root,
        request_ref,
        vec![
            "source.taskExecutionRequestRef".to_string(),
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "source.taskPlanRunId".to_string(),
            "source.originalResultFile".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.blockedReasonOptions".to_string(),
        ],
    )?;
    // Do not inspect repairContract.issueConflicts here. Large issue arrays are
    // deliberately reprojected into indexed read-group fields, so requesting
    // the unsplit parent field can make an otherwise valid repair request fail
    // with FIELD_NOT_ALLOWED. The current canonical TaskDefinition and result
    // are authoritative; validation below rebuilds the issue set directly.
    let source_task_id = string_field(&repair_fields, "source.taskId")?;
    let source_task_execution_request_ref =
        string_field(&repair_fields, "source.taskExecutionRequestRef")?;
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let (current_task_plan, _) = load_current_plan_and_run(root, &locator)?;
    let task = current_task_plan
        .tasks
        .iter()
        .find(|task| task.task_id == source_task_id)
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "TaskResult repair source task {} is not in the current TaskPlan",
                source_task_id
            ))
        })?;
    if !frontend_quality_self_check_applies(&task)
        || task
            .frontend_experience_requirement
            .as_ref()
            .and_then(|requirement| {
                requirement.pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
            })
            .is_some_and(Value::is_object)
    {
        return Ok(None);
    }

    let hydrated_task = task_with_phase_execution_guidance(root, &locator, task)?;
    if hydrated_task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| {
            requirement.pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
        })
        .is_none_or(|value| !value.is_object())
    {
        return Ok(None);
    }

    let task_plan_id = string_field(&repair_fields, "source.taskPlanId")?;
    let task_id = string_field(&repair_fields, "source.taskId")?;
    let run_id = string_field(&repair_fields, "source.taskPlanRunId")?;
    let target_file = repair_fields
        .get("source.originalResultFile")
        .and_then(|field| field.value.as_str())
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "TaskResult repair request is missing source.originalResultFile.".to_string(),
            )
        })?
        .to_string();
    let result_file = string_field(&repair_fields, "outputContract.resultFile")?;
    // Rebuild this machine-owned list from the current task contract. Repair
    // requests can be created from an older execution request whose declared
    // fields predate a contract fix; carrying that stale list would recreate
    // the same impossible repair contract.
    let required_top_level_fields = task_result_required_top_level_fields(&hydrated_task)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let blocked_output = json!({
        "blockedReasons": value_field(&repair_fields, "outputContract.blockedReasonOptions")
    });
    let code_quality_requirements =
        code_quality_requirements_for_task(&current_task_plan, &hydrated_task);
    let browser_profile = current_task_plan
        .browser_verification_profiles
        .iter()
        .find(|profile| profile.task_id == hydrated_task.task_id)
        .cloned();

    let raw_result = read_project_json_value(root, &target_file)?;
    let normalized_result = normalize_task_result_machine_fields(
        raw_result,
        &request_id,
        &task_plan_id,
        &task_id,
        &hydrated_task,
        browser_profile.as_ref(),
    );
    let (issues, previous_changed_files) =
        match serde_json::from_value::<TaskResult>(normalized_result.clone()) {
            Ok(result) => {
                let previous_changed_files =
                    previous_persisted_changed_files(root, &locator, &run_id, &result);
                (
                    validate_result(
                        root,
                        &normalized_result,
                        &result,
                        &hydrated_task,
                        browser_profile.as_ref(),
                        &current_task_plan.engineering_quality_requirements,
                        &code_quality_requirements,
                        &required_top_level_fields,
                        &blocked_output,
                        &result_file,
                        &target_file,
                    ),
                    previous_changed_files,
                )
            }
            Err(error) => (
                vec![issue(
                    "TASK_RESULT_SCHEMA_INVALID",
                    "$",
                    &format!("TaskResult JSON has an invalid schema: {error}"),
                )],
                Vec::new(),
            ),
        };
    if issues.is_empty() {
        return Ok(None);
    }
    if !issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_FRONTEND_QUALITY_INVALID")
    {
        // This refresh path exists for stale frontend-quality projections. A
        // repair request with unrelated issues must remain the active request;
        // recreating it here would change the requestRef on every continue.
        return Ok(None);
    }
    let context = RepairContextInput {
        task_plan_id,
        task_id,
        run_id,
        task: hydrated_task,
        result_file,
        blocked_output,
        submitted_result: normalized_result,
        previous_changed_files,
        code_quality_requirements,
        browser_profile,
    };
    let input = FileSubmitInput {
        project_root: project_root.to_string(),
        request_ref: source_task_execution_request_ref.clone(),
        written_target_ids: None,
    };
    materialize_task_result_repair(
        &input,
        delivery_id,
        phase_id,
        source_task_execution_request_ref,
        target_file,
        issues,
        context,
    )
    .map(Some)
}

fn materialize_task_result_repair(
    input: &FileSubmitInput,
    delivery_id: &str,
    phase_id: &str,
    source_task_execution_request_ref: String,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
    context: RepairContextInput,
) -> Result<LoomMcpActionResult, state::store::StateError> {
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
    let code_quality_contract = context.code_quality_requirements.clone();
    let result_contract = task_result_contract(
        &context.task,
        &code_quality_contract,
        context.browser_profile.as_ref(),
    );
    let mut schema_shape = result_contract["schemaShape"].clone();
    let mut result_template = task_result_repair_template(&context, &issues);
    let required_top_level_fields = task_result_required_top_level_fields(&context.task)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut contract_recovery = None;
    if let Some(missing_fields) = task_result_repair_contract_missing_fields(
        &context.task,
        &required_top_level_fields,
        &schema_shape,
        &result_template,
    ) {
        // This request is generated from the same TaskDefinition that validates
        // the result. A stale or partially materialized contract is therefore
        // recoverable inside Loom and must never become a terminal workflow
        // failure. Rebuild both projections from the canonical task contract.
        schema_shape = task_result_schema_shape(&context.task, context.browser_profile.as_ref());
        result_template = task_result_template_with_code_quality(
            &context.task,
            &code_quality_contract,
            context.browser_profile.as_ref(),
        );
        merge_submitted_task_result_fields(
            &mut result_template,
            &context.submitted_result,
            &issues,
        );
        ensure_repair_contract_fields(
            &required_top_level_fields,
            &mut schema_shape,
            &mut result_template,
        );
        contract_recovery = Some(json!({
            "mode": "canonical_rebuild",
            "fields": missing_fields,
            "authority": "task_definition_and_task_result_contract"
        }));
    }
    let browser_repair_reference_load_plan = if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_BROWSER_VERIFICATION_INVALID")
    {
        json!([{
                "refId": "test.pw.reliability",
                "path": "tech/test/playwright/reliability.md",
                "reason": "Interpret retry, failure, blocked, and artifact evidence without hiding flaky behavior."
        }])
    } else {
        json!([])
    };
    let mut context_fields = vec![
        "source.taskPlanId",
        "source.taskId",
        "source.taskPlanRunId",
        "source.taskExecutionRequestRef",
        "source.originalResultFile",
        "taskProjection.taskId",
        "taskProjection.groupId",
        "taskProjection.title",
        "taskProjection.taskKind",
        "taskProjection.objective",
        "taskProjection.acceptanceRefs",
        "taskProjection.requirementDetailRefs",
        "taskProjection.implementationObligations",
        "taskProjection.verificationIntents",
        "outputContract.blockedReasonOptions",
        "repairContract.profile",
        "repairContract.issueConflicts",
        "repairContract.minimalRepairRules",
    ];
    if browser_repair_reference_load_plan
        .as_array()
        .is_some_and(|items| !items.is_empty())
    {
        context_fields.push("repairContract.referenceLoadPlan");
    }
    if !context.task.concept_refs.is_empty() {
        context_fields.push("taskProjection.conceptRefs");
    }
    if architecture_quality_evidence_applies(&context.task) {
        context_fields.push("taskProjection.architectureQualityRequirementRefs");
    }
    if api_contract_evidence_applies(&context.task) {
        context_fields.push("taskProjection.apiContractRequirementRefs");
    }
    if code_quality_evidence_applies(&context.task) {
        context_fields.push("taskProjection.codeQualityRequirementRefs");
        context_fields.push("sourceContext.codeQualityExecutionContext");
    }
    if frontend_self_check_applies(&context.task) {
        context_fields.push(
            "taskProjection.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
        );
    }
    if frontend_quality_self_check_applies(&context.task) {
        context_fields.extend([
            "taskProjection.frontendExperienceRequirement.executionGuidance.uiProductionBrief",
            "taskProjection.frontendExperienceRequirement.executionGuidance.styleAssetPlan",
            "taskProjection.frontendExperienceRequirement.uiSurfaceDecisionContractRef",
        ]);
    }
    if runtime_delivery_evidence_applies(&context.task) {
        context_fields.extend(task_projection_runtime_delivery_requirement_read_fields(
            &context.task,
        ));
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
        "outputContract.schemaShape.properties.implementationObligationResults",
        "outputContract.schemaShape.properties.selfRepairSummary",
        "outputContract.schemaShape.properties.failure",
        "outputContract.schemaShape.properties.executionContinuity",
        "outputContract.schemaShape.properties.notes",
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
    if architecture_quality_evidence_applies(&context.task) {
        write_contract_fields
            .push("outputContract.schemaShape.properties.architectureQualityEvidence");
    }
    if api_contract_evidence_applies(&context.task) {
        write_contract_fields.push("outputContract.schemaShape.properties.apiContractEvidence");
    }
    if code_quality_evidence_applies(&context.task) {
        write_contract_fields.push("outputContract.schemaShape.properties.codeQualityEvidence");
    }
    let mut root_value = json!({
        "schemaVersion": "1.0",
        "requestType": "task_result_repair",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
            "artifactKind": ArtifactKind::TaskResultRepair,
            "source": {
            "taskExecutionRequestRef": source_task_execution_request_ref,
            "taskPlanId": context.task_plan_id,
            "taskId": context.task_id,
            "taskPlanRunId": context.run_id,
            "originalResultFile": target_file
        },
        "taskProjection": task_projection(&context.task),
        "repairContract": {
            "profile": "minimal_task_result_repair",
            "issueConflicts": task_result_issue_conflicts(&context, &issues),
            "minimalRepairRules": task_result_minimal_repair_rules(&issues),
            "referenceLoadPlan": browser_repair_reference_load_plan
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
            "requiredTopLevelFields": required_top_level_fields,
            "blockedReasonOptions": context.blocked_output
                .get("blockedReasons")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "schemaShape": schema_shape,
            "resultTemplate": result_template,
            "resultRules": [
                "The replacement must be a TaskResult JSON, not a repair summary.",
                "implementationObligationResults must contain exactly one entry for each canonical obligation in the supplied order; Loom derives obligationId and verificationIds.",
                "Runtime, frontend, and concept evidence must follow the original output contract; requirement-detail evidence is derived by Loom from verification results."
            ]
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "task_result_repair_context",
                    "required": true,
                    "purpose": "Read the original TaskResult validation issues and task contract.",
                    "whenToRead": "Read before rewriting TaskResult.",
                    "selectors": read_selectors_value_from_paths(context_fields)
                },
                {
                    "groupId": "task_result_repair_write_contract",
                    "required": true,
                    "purpose": "Read the TaskResult replacement output contract.",
                    "whenToRead": "Read before writing replacement TaskResult.",
                    "selectors": read_selectors_value_from_paths(write_contract_fields)
                }
            ]
        }
    });
    if let Some(recovery) = contract_recovery {
        root_value["repairContract"]["canonicalRebuild"] = recovery;
    }
    if !context.code_quality_requirements.is_empty() {
        root_value["sourceContext"] = json!({
            "codeQualityExecutionContext": code_quality_execution_context(&context.code_quality_requirements)
        });
    }
    let stored = state::write_native_request(
        &input.project_root,
        state::NativeRequestInput {
            request_id,
            request_kind: "task_result_repair".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
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

fn task_projection(task: &TaskDefinition) -> Value {
    let full = serde_json::to_value(task).unwrap_or_else(|_| json!({}));
    let mut projection = serde_json::Map::new();
    projection.insert(
        "projectionKind".to_string(),
        json!("task_scoped_repair_contract"),
    );
    for key in [
        "taskId",
        "groupId",
        "title",
        "taskKind",
        "implementationActions",
        "implementationObligations",
        "objective",
        "dependsOn",
        "scopeRefs",
        "acceptanceRefs",
        "requirementDetailRefs",
        "conceptRefs",
        "engineeringQualityRequirementRefs",
        "architectureQualityRequirementRefs",
        "apiContractRequirementRefs",
        "codeQualityRequirementRefs",
    ] {
        if let Some(value) = full.get(key) {
            projection.insert(key.to_string(), value.clone());
        }
    }
    projection.insert(
        "writeBoundary".to_string(),
        compact_write_boundary(full.get("writeBoundary")),
    );
    projection.insert(
        "verificationIntents".to_string(),
        compact_verification_intents(full.get("verificationIntents")),
    );
    projection.insert(
        "implementationObligations".to_string(),
        compact_implementation_obligations(full.get("implementationObligations")),
    );
    if let Some(requirement) = full.get("frontendExperienceRequirement") {
        projection.insert(
            "frontendExperienceRequirement".to_string(),
            compact_frontend_experience_requirement(requirement),
        );
    }
    if let Some(requirement) = full.get("runtimeDeliveryRequirement") {
        projection.insert(
            "runtimeDeliveryRequirement".to_string(),
            compact_runtime_delivery_requirement(requirement),
        );
    }
    Value::Object(projection)
}

fn compact_write_boundary(value: Option<&Value>) -> Value {
    let Some(object) = value.and_then(Value::as_object) else {
        return json!({"forbiddenPaths": [".loom"], "artifactRefs": {}});
    };
    json!({
        "forbiddenPaths": object.get("forbiddenPaths").cloned().unwrap_or_else(|| json!([".loom"])),
        "artifactRefs": object.get("artifactRefs").cloned().unwrap_or_else(|| json!({}))
    })
}

fn compact_verification_intents(value: Option<&Value>) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|intent| {
                compact_object_fields(
                    intent,
                    &[
                        "verificationId",
                        "acceptanceRefs",
                        "requirementDetailRefs",
                        "behavior",
                        "preferredEvidence",
                        "acceptableEvidence",
                    ],
                )
            })
            .collect(),
    )
}

fn compact_implementation_obligations(value: Option<&Value>) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|obligation| {
                compact_object_fields(
                    obligation,
                    &[
                        "obligationId",
                        "kind",
                        "sourceRefs",
                        "artifactRefs",
                        "requiredOutcome",
                        "required",
                        "acceptableEvidence",
                        "verificationIds",
                        "deferPolicy",
                    ],
                )
            })
            .collect(),
    )
}

fn compact_frontend_experience_requirement(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut projection = compact_object_fields(
        value,
        &[
            "frontendExperienceRef",
            "experienceLevel",
            "mustSatisfy",
            "uiSurfaceRegistryRef",
            "uiSurfaceDecisionContractRef",
        ],
    );
    let Some(projection_object) = projection.as_object_mut() else {
        return projection;
    };
    let guidance = object
        .get("executionGuidance")
        .and_then(Value::as_object)
        .or_else(|| Some(object));
    let Some(guidance) = guidance else {
        return projection;
    };
    let mut compact_guidance = serde_json::Map::new();
    if let Some(value) = guidance.get("closureRequirementRefs") {
        compact_guidance.insert("closureRequirementRefs".to_string(), value.clone());
    }
    if let Some(value) = guidance.get("uiTaskScope") {
        compact_guidance.insert("uiTaskScope".to_string(), compact_ui_task_scope(value));
    }
    if let Some(value) = guidance.get("uiProductionBrief") {
        compact_guidance.insert(
            "uiProductionBrief".to_string(),
            compact_ui_production_brief(value),
        );
    }
    if let Some(value) = guidance.get("styleAssetPlan") {
        compact_guidance.insert(
            "styleAssetPlan".to_string(),
            compact_style_asset_plan(value),
        );
    }
    projection_object.insert(
        "executionGuidance".to_string(),
        Value::Object(compact_guidance),
    );
    projection
}

fn compact_ui_task_scope(value: &Value) -> Value {
    let mut projection = compact_object_fields(
        value,
        &[
            "ownershipDimensions",
            "surfacesInScope",
            "dataViewsInScope",
            "actionsInScope",
            "operationPathsInScope",
            "frontendBackendBindings",
            "stateExpectation",
            "regionsInScope",
            "actionsInContract",
            "statesInContract",
            "qualityRulesInScope",
            "layoutBaseline",
            "informationModel",
            "contentBoundary",
            "bindingContract",
            "contractRef",
            "patternDecision",
        ],
    );
    if let Some(object) = value.as_object() {
        for (source_key, item_keys) in [
            (
                "surfacesInScope",
                ["surfaceId", "name", "kind", "purpose", "role"].as_slice(),
            ),
            (
                "dataViewsInScope",
                ["viewId", "name", "purpose", "fieldRefs", "interfaceRefs"].as_slice(),
            ),
            (
                "actionsInScope",
                [
                    "actionId",
                    "name",
                    "label",
                    "purpose",
                    "interfaceRefs",
                    "operationPathRefs",
                ]
                .as_slice(),
            ),
            (
                "operationPathsInScope",
                [
                    "pathId",
                    "name",
                    "purpose",
                    "interfaceRefs",
                    "actionRefs",
                    "stateRefs",
                ]
                .as_slice(),
            ),
            (
                "regionsInScope",
                [
                    "regionId",
                    "name",
                    "role",
                    "layout",
                    "informationModel",
                    "density",
                ]
                .as_slice(),
            ),
            (
                "actionsInContract",
                [
                    "actionId",
                    "name",
                    "label",
                    "purpose",
                    "interfaceRefs",
                    "operationPathRefs",
                ]
                .as_slice(),
            ),
            (
                "statesInContract",
                ["state", "stateId", "trigger", "userMeaning"].as_slice(),
            ),
            (
                "qualityRulesInScope",
                ["ruleId", "category", "requirement", "verification"].as_slice(),
            ),
        ] {
            if let Some(items) = object.get(source_key) {
                projection[source_key] = compact_array_items(items, item_keys);
            }
        }
        if let Some(bindings) = object.get("frontendBackendBindings") {
            projection["frontendBackendBindings"] = Value::Array(
                bindings
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|binding| {
                        let mut compact = compact_object_fields(
                            binding,
                            &[
                                "bindingId",
                                "workflowRefs",
                                "operationPathRefs",
                                "completionRule",
                            ],
                        );
                        if let Some(interfaces) = binding.get("interfaces") {
                            compact["interfaces"] = compact_array_items(
                                interfaces,
                                &["interfaceId", "name", "type", "method", "path", "role"],
                            );
                        }
                        compact
                    })
                    .collect(),
            );
        }
    }
    projection
}

fn compact_ui_production_brief(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut projection = compact_object_fields(
        value,
        &[
            "schemaVersion",
            "briefKind",
            "appliesTo",
            "productIntent",
            "layoutContract",
            "informationContract",
            "actionContract",
            "stateContract",
            "visualContract",
            "contentBoundary",
        ],
    );
    if let Some(contract) = object.get("surfaceDecisionContract") {
        projection["surfaceDecisionContract"] = compact_ui_surface_contract(contract);
    }
    projection
}

fn compact_ui_surface_contract(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut projection = compact_object_fields(
        value,
        &[
            "contractRef",
            "selectionMode",
            "patternDecision",
            "semanticFacts",
            "layoutModel",
            "contentBoundary",
            "compositionConstraints",
            "layoutBaseline",
            "informationModel",
        ],
    );
    for (source_key, target_key, item_keys) in [
        (
            "regionsInScope",
            "regionsInScope",
            [
                "regionId",
                "name",
                "role",
                "layout",
                "informationModel",
                "density",
            ]
            .as_slice(),
        ),
        (
            "actionsInScope",
            "actionsInScope",
            [
                "actionId",
                "name",
                "label",
                "purpose",
                "interfaceRefs",
                "operationPathRefs",
            ]
            .as_slice(),
        ),
        (
            "statesInScope",
            "statesInScope",
            ["state", "stateId", "trigger", "userMeaning"].as_slice(),
        ),
        (
            "qualityRulesInScope",
            "qualityRulesInScope",
            ["ruleId", "category", "requirement", "verification"].as_slice(),
        ),
    ] {
        if let Some(value) = object.get(source_key) {
            projection[target_key] = compact_array_items(value, item_keys);
        }
    }
    projection
}

fn compact_style_asset_plan(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut projection =
        compact_object_fields(value, &["source", "strategy", "designTokenAssetPlan"]);
    if let Some(reference_plan) = object.get("referencePlan") {
        projection["referencePlan"] =
            compact_array_items(reference_plan, &["refId", "path", "reason", "required"]);
    }
    projection
}

fn compact_runtime_delivery_requirement(value: &Value) -> Value {
    compact_object_fields(
        value,
        &[
            "appliesToThisTask",
            "reason",
            "runtimeDeliveryRef",
            "source",
            "deploymentFailureRef",
            "affectedContractFields",
            "requiredCodeLevelChecks",
            "evidenceExpectedInTaskResult",
            "forbiddenActions",
        ],
    )
}

fn compact_array_items(value: &Value, keys: &[&str]) -> Value {
    Value::Array(
        value
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| compact_object_fields(item, keys))
            .collect(),
    )
}

fn compact_object_fields(value: &Value, keys: &[&str]) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut compact = serde_json::Map::new();
    for key in keys {
        if let Some(value) = object.get(*key).filter(|value| !value.is_null()) {
            compact.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(compact)
}

fn task_result_repair_contract_missing_fields(
    task: &TaskDefinition,
    required_top_level_fields: &[String],
    schema_shape: &Value,
    result_template: &Value,
) -> Option<Vec<String>> {
    let required = required_top_level_fields
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let schema_properties = schema_shape.get("properties").and_then(Value::as_object);
    let template_object = result_template.as_object();
    let missing = task_result_required_top_level_fields(task)
        .into_iter()
        .filter(|field| {
            !required.contains(field)
                || !schema_properties.is_some_and(|properties| properties.contains_key(*field))
                || !template_object.is_some_and(|template| template.contains_key(*field))
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!missing.is_empty()).then_some(missing)
}

fn ensure_repair_contract_fields(
    required_top_level_fields: &[String],
    schema_shape: &mut Value,
    result_template: &mut Value,
) {
    let Some(properties) = schema_shape
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(template) = result_template.as_object() else {
        return;
    };
    for field in required_top_level_fields {
        if properties.contains_key(field) {
            continue;
        }
        let Some(value) = template.get(field) else {
            continue;
        };
        let shape = match value {
            Value::Array(_) => json!([{}]),
            Value::Object(_) => json!({"shape": "object"}),
            Value::Null => json!({"shape": "value or null"}),
            Value::Bool(_) => json!("boolean"),
            Value::Number(_) => json!("number"),
            Value::String(_) => json!("string"),
        };
        properties.insert(field.clone(), shape);
    }
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
    let mut seen = BTreeSet::new();
    issues
        .iter()
        .filter(|issue| {
            seen.insert((
                issue.code.clone(),
                issue.field_path.clone().unwrap_or_default(),
            ))
        })
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
            if issue.code == "TASK_RESULT_BROWSER_VERIFICATION_INVALID" {
                return task_result_browser_verification_conflict(context, base);
            }
            if issue.code == "TASK_RESULT_RUNTIME_CHECK_ID_INVALID" {
                return task_result_runtime_conflict(context, base);
            }
            if issue.code == "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID" {
                return task_result_architecture_quality_conflict(context, base);
            }
            if issue.code == "TASK_RESULT_API_CONTRACT_INVALID" {
                return task_result_api_contract_conflict(context, base);
            }
            if issue.code == "TASK_RESULT_CODE_QUALITY_INVALID" {
                return task_result_code_quality_conflict(context, base);
            }
            if matches!(
                issue.code.as_str(),
                "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INVALID"
                    | "TASK_RESULT_IMPLEMENTATION_OBLIGATION_INCOMPLETE"
            ) {
                return task_result_implementation_obligation_conflict(context, base);
            }
            base
        })
        .collect()
}

fn task_result_implementation_obligation_conflict(
    context: &RepairContextInput,
    mut base: Value,
) -> Value {
    let expected = context
        .task
        .implementation_obligations
        .iter()
        .map(|obligation| {
            json!({
                "obligationId": obligation.obligation_id,
                "kind": obligation.kind,
                "required": obligation.required,
                "acceptableEvidence": obligation.acceptable_evidence,
                "verificationIds": obligation.verification_ids
            })
        })
        .collect::<Vec<_>>();
    let current = context
        .submitted_result
        .get("implementationObligationResults")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    json!({
                        "obligationId": item.get("obligationId").cloned().unwrap_or(Value::Null),
                        "status": item.get("status").cloned().unwrap_or(Value::Null),
                        "verificationIds": item.get("verificationIds").cloned().unwrap_or_else(|| json!([])),
                        "evidenceRefCount": item.get("evidenceRefs").and_then(Value::as_array).map(|refs| refs.len()).unwrap_or(0)
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    base["expectedImplementationObligations"] = json!(expected);
    base["current"] = json!({
        "implementationObligationResults": current,
        "passedVerifications": context
            .submitted_result
            .get("verificationResults")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("status").and_then(Value::as_str) == Some("passed"))
                    .map(|item| {
                        json!({
                            "verificationId": item.get("verificationId"),
                            "evidenceType": item.get("evidenceType")
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    base["validRepairChoices"] = json!([
        "Keep implementationObligationResults in the canonical obligation order and do not add, remove, rename, or reorder entries to change their meaning; Loom derives obligationId and verificationIds.",
        "Fill evidenceRefs with concrete project-relative implementation or verification evidence; do not author linkage fields.",
        "Mark an obligation satisfied only when the cited verification passed and its evidence capability proves the requiredOutcome. A build or reference read alone cannot satisfy a behavioral obligation.",
        "Keep status completed only when every required obligation is satisfied; otherwise submit the actual incomplete status and gap.",
    ]);
    base
}

fn task_result_browser_verification_conflict(
    context: &RepairContextInput,
    mut base: Value,
) -> Value {
    let unresolved = context
        .submitted_result
        .get("verificationResults")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|verification| {
            let verification_id = verification
                .get("verificationId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            verification
                .get("browserChecks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|check| browser_check_value_needs_repair(check))
                .map(move |check| {
                    json!({
                        "verificationId": verification_id,
                        "checkId": check.get("checkId").and_then(Value::as_str),
                        "status": check.get("status").and_then(Value::as_str),
                        "attempts": check.get("attempts").and_then(Value::as_u64),
                        "blockedReason": check.get("blockedReason").cloned().unwrap_or(Value::Null),
                        "observedOutcome": check.get("observedOutcome").and_then(Value::as_str)
                    })
                })
        })
        .collect::<Vec<_>>();
    let unresolved_ids = unresolved
        .iter()
        .filter_map(|check| check.get("checkId").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let expected = context
        .browser_profile
        .as_ref()
        .map(|profile| {
            profile
                .checks
                .iter()
                .filter(|check| unresolved_ids.contains(check.check_id.as_str()))
                .map(|check| {
                    json!({
                        "checkId": check.check_id,
                        "verificationId": check.verification_id,
                        "viewportRef": check.viewport_ref,
                        "backendMode": check.backend_mode
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    base["currentUnresolvedChecks"] = json!(unresolved);
    base["expectedChecks"] = json!(expected);
    base["validRepairChoices"] = json!([
        "Preserve actual browser outcomes. Repair only check ids, parent verification mapping, command, attempts, artifact refs, observed outcome, or blocked reason fields that were recorded incorrectly.",
        "If a required browser check did not pass, keep it failed, blocked, or not_run and change the TaskResult status accordingly; never claim a pass to satisfy the contract."
    ]);
    base
}

fn browser_check_value_needs_repair(check: &Value) -> bool {
    let status = check.get("status").and_then(Value::as_str);
    if status != Some("passed") {
        return true;
    }
    check.get("attempts").and_then(Value::as_u64) == Some(0)
        || check
            .get("command")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        || check
            .get("observedOutcome")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        || check
            .get("blockedReason")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        || check
            .get("artifactRefs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|path| !is_safe_project_relative_path(path))
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
    let frontend_requirement = context.task.frontend_experience_requirement.as_ref();
    let surface_contract = frontend_requirement
        .and_then(|requirement| {
            requirement.pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
        })
        .cloned()
        .unwrap_or(Value::Null);
    let self_check = context
        .submitted_result
        .get("frontendQualitySelfCheck")
        .cloned()
        .unwrap_or(Value::Null);
    let expected_region_ids = object_array_id_set(&surface_contract, "regionsInScope", "regionId");
    let checked_region_ids = object_array_id_set(&self_check, "surfaceRegionEvidence", "id");
    let expected_action_ids = object_array_id_set(&surface_contract, "actionsInScope", "actionId");
    let checked_action_ids = object_array_id_set(&self_check, "surfaceActionEvidence", "id");
    let expected_surface_state_ids =
        object_array_id_set(&surface_contract, "statesInScope", "state");
    let checked_surface_state_ids = object_array_id_set(&self_check, "surfaceStateEvidence", "id");
    let expected_surface_quality_rule_ids =
        object_array_id_set(&surface_contract, "qualityRulesInScope", "ruleId");
    let checked_surface_quality_rule_ids =
        object_array_id_set(&self_check, "surfaceQualityRuleEvidence", "id");
    let expected_reference_plan_files = frontend_requirement
        .map(reference_plan_paths)
        .unwrap_or_default();
    let checked_reference_plan_files =
        string_set_from_array(&self_check, "referencePlanFilesChecked");
    let design_token_plan = frontend_requirement
        .and_then(|requirement| {
            requirement.pointer("/executionGuidance/styleAssetPlan/designTokenAssetPlan")
        })
        .unwrap_or(&Value::Null);
    base["current"] = json!({
        "status": self_check.get("status").and_then(Value::as_str),
        "designTokenEvidence": design_token_evidence_summary(&self_check),
        "surfaceDecisionContractRef": self_check
            .get("surfaceDecisionContractRef")
            .and_then(Value::as_str),
        "surfaceRegionIdsCovered": checked_region_ids,
        "surfaceActionIdsCovered": checked_action_ids,
        "surfaceStateIdsCovered": checked_surface_state_ids,
        "surfaceQualityRuleIdsCovered": checked_surface_quality_rule_ids,
        "referencePlanFilesCheckedCount": checked_reference_plan_files.len(),
        "contentBoundaryEvidence": {
            "checked": self_check
                .pointer("/contentBoundaryEvidence/checked")
                .and_then(Value::as_bool),
            "violationCount": self_check
                .pointer("/contentBoundaryEvidence/forbiddenContentViolations")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0),
            "evidencePresent": self_check
                .pointer("/contentBoundaryEvidence/evidence")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|evidence| !evidence.is_empty())
        },
        "knownGapsCount": self_check
            .get("knownGaps")
            .and_then(Value::as_array)
            .map(|items| items.len())
    });
    base["expected"] = json!({
        "surfaceDecisionContractRef": frontend_requirement
            .and_then(|requirement| requirement.get("uiSurfaceDecisionContractRef"))
            .and_then(Value::as_str)
            .or_else(|| surface_contract.get("contractRef").and_then(Value::as_str)),
        "surfaceRegionIdsInScope": expected_region_ids,
        "missingSurfaceRegionIds": string_set_difference(&expected_region_ids, &checked_region_ids),
        "surfaceActionIdsInScope": expected_action_ids,
        "missingSurfaceActionIds": string_set_difference(&expected_action_ids, &checked_action_ids),
        "surfaceStateIdsInScope": expected_surface_state_ids,
        "missingSurfaceStateIds": string_set_difference(&expected_surface_state_ids, &checked_surface_state_ids),
        "surfaceQualityRuleIdsInScope": expected_surface_quality_rule_ids,
        "missingSurfaceQualityRuleIds": string_set_difference(
            &expected_surface_quality_rule_ids,
            &checked_surface_quality_rule_ids
        ),
        "referencePlanFileCount": expected_reference_plan_files.len(),
        "missingReferencePlanFiles": string_set_difference(
            &expected_reference_plan_files,
            &checked_reference_plan_files
        ),
        "designTokenAssetPlan": design_token_plan_summary(design_token_plan),
        "forbiddenUserVisibleContentCount": surface_contract
            .pointer("/contentBoundary/forbiddenUserVisibleContent")
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0)
    });
    base["validRepairChoices"] = json!([
        "When task.frontendExperienceRequirement.uiSurfaceDecisionContractRef is present, make frontendQualitySelfCheck prove surfaceRegionEvidence, surfaceActionEvidence, surfaceStateEvidence, surfaceQualityRuleEvidence, contentBoundaryEvidence, and referencePlanFilesChecked from the task-scoped uiProductionBrief.",
        "For satisfied surface evidence, cite concrete UI files and non-empty evidence; do not leave replace_with_* placeholders.",
        "If any task-scoped surface contract item remains partial, missing, or blocked_by_environment, keep frontendQualitySelfCheck.status below satisfied and record the specific gap.",
        "Use frontendQualitySelfCheck.designTokenEvidence to prove the task styleAssetPlan.designTokenAssetPlan without creating a parallel token system."
    ]);
    base
}

fn reference_plan_paths(requirement: &Value) -> BTreeSet<String> {
    requirement
        .pointer("/executionGuidance/styleAssetPlan/referencePlan")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn string_set_from_array(value: &Value, field: &str) -> BTreeSet<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn string_set_difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}

fn value_field_contains_placeholder(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("replace_with_"),
        Value::Array(items) => items.iter().any(value_field_contains_placeholder),
        Value::Object(object) => object.values().any(value_field_contains_placeholder),
        _ => false,
    }
}

fn object_array_id_set(value: &Value, array_field: &str, id_field: &str) -> BTreeSet<String> {
    value
        .get(array_field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(id_field).and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn design_token_evidence_summary(self_check: &Value) -> Value {
    let evidence = self_check
        .get("designTokenEvidence")
        .unwrap_or(&Value::Null);
    json!({
        "present": !evidence.is_null(),
        "strategyUsed": evidence.get("strategyUsed").and_then(Value::as_str),
        "templateIdUsed": evidence.get("templateIdUsed").cloned().unwrap_or(Value::Null),
        "tokenAssetFileCount": evidence.get("tokenAssetFiles").and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
        "tokenConsumerFileCount": evidence.get("tokenConsumerFiles").and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
        "existingTokenSystemReused": evidence.get("existingTokenSystemReused").and_then(Value::as_bool),
        "parallelTokenSystemCreated": evidence.get("parallelTokenSystemCreated").and_then(Value::as_bool),
        "mergeSummaryPresent": evidence.get("mergeSummary").and_then(Value::as_str).map(str::trim).is_some_and(|summary| !summary.is_empty())
    })
}

fn design_token_plan_summary(design_token_plan: &Value) -> Value {
    let plan = design_token_plan
        .get("designTokenAssetPlan")
        .unwrap_or(&Value::Null);
    json!({
        "strategy": plan.get("strategy").and_then(Value::as_str),
        "templateId": plan.get("templateId").cloned().unwrap_or(Value::Null),
        "targetFiles": plan.get("targetFiles").cloned().unwrap_or_else(|| json!([])),
        "mergePolicy": plan.get("mergePolicy").and_then(Value::as_str),
        "duplicationPolicy": plan.get("duplicationPolicy").and_then(Value::as_str)
    })
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

fn task_result_architecture_quality_conflict(
    context: &RepairContextInput,
    mut base: Value,
) -> Value {
    base["expectedArchitectureQualityRequirementRefs"] =
        json!(context.task.architecture_quality_requirement_refs);
    base["current"] = json!({
        "architectureQualityEvidence": compact_task_result_evidence_entries(
            context.submitted_result.get("architectureQualityEvidence"),
            "requirementId",
        )
    });
    base["validRepairChoices"] = json!([
        "If the implementation satisfies the referenced architecture quality requirements, add architectureQualityEvidence entries for every task.architectureQualityRequirementRefs item and cite task verificationIds.",
        "If evidence is missing or the implementation has a real architecture quality gap, keep status below completed or record the gap instead of claiming satisfied evidence."
    ]);
    base
}

fn task_result_api_contract_conflict(context: &RepairContextInput, mut base: Value) -> Value {
    base["expectedApiContractRequirementRefs"] = json!(context.task.api_contract_requirement_refs);
    base["current"] = json!({
        "apiContractEvidence": compact_task_result_evidence_entries(
            context.submitted_result.get("apiContractEvidence"),
            "requirementId",
        )
    });
    base["validRepairChoices"] = json!([
        "If the implementation satisfies the referenced API contract requirements, add apiContractEvidence entries for every task.apiContractRequirementRefs item and cite task verificationIds.",
        "If API behavior or evidence is missing, keep status below completed or record the gap instead of claiming satisfied evidence."
    ]);
    base
}

fn task_result_code_quality_conflict(context: &RepairContextInput, mut base: Value) -> Value {
    base["expectedCodeQualityRequirementRefs"] = json!(context.task.code_quality_requirement_refs);
    base["expectedCodeQualityExecutionContext"] =
        code_quality_execution_context(&context.code_quality_requirements);
    base["expectedReferenceLoadPlan"] = json!(context
        .code_quality_requirements
        .iter()
        .map(|requirement| {
            json!({
                "requirementId": requirement.requirement_id,
                "referenceGroups": requirement.reference_groups,
                "referenceLoadPlan": requirement.reference_load_plan
            })
        })
        .collect::<Vec<_>>());
    base["current"] = json!({
        "codeQualityEvidence": compact_task_result_evidence_entries(
            context.submitted_result.get("codeQualityEvidence"),
            "requirementId",
        )
    });
    base["validRepairChoices"] = json!([
        "If the implementation satisfies the referenced code quality requirements, add codeQualityEvidence entries for every task.codeQualityRequirementRefs item and cite task verificationIds.",
        "If selected language or framework reference evidence or verification is missing, keep status below completed or record the gap instead of claiming satisfied evidence."
    ]);
    base
}

fn compact_task_result_evidence_entries(value: Option<&Value>, id_key: &str) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|entry| {
                let object = entry.as_object();
                let mut compact = serde_json::Map::new();
                compact.insert(
                    id_key.to_string(),
                    object
                        .and_then(|item| item.get(id_key))
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                compact.insert(
                    "status".to_string(),
                    object
                        .and_then(|item| item.get("status"))
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                compact.insert(
                    "verificationIdCount".to_string(),
                    json!(object
                        .and_then(|item| item.get("verificationIds"))
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)),
                );
                compact.insert(
                    "evidencePresent".to_string(),
                    json!(object
                        .and_then(|item| item.get("evidence"))
                        .is_some_and(|value| !value.is_null())),
                );
                compact.insert(
                    "knownGapCount".to_string(),
                    json!(object
                        .and_then(|item| item.get("knownGaps"))
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)),
                );
                Value::Object(compact)
            })
            .collect(),
    )
}

fn task_result_minimal_repair_rules(issues: &[delivery_core::RepairIssue]) -> Vec<&'static str> {
    let mut rules = vec![
        "Repair the same TaskResult JSON file only.",
        "Do not edit project source files for TaskResult contract repair.",
        "Submit evidence arrays in the order supplied by the TaskResult template; Loom derives verificationId and other relationship fields before validation.",
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
        rules.push("Keep runtimeDeliveryEvidence.codeLevelChecks in the request order and repair status/evidence only; Loom derives check ids and contract fields.");
        rules.push("Use the code-level check evidence field for a concise outcome; do not add an uncontracted reason field.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_FRONTEND_QUALITY_INVALID")
    {
        rules.push("When uiSurfaceDecisionContractRef is present, frontendQualitySelfCheck must prove the task-scoped surfaceRegionEvidence, surfaceActionEvidence, surfaceStateEvidence, surfaceQualityRuleEvidence, contentBoundaryEvidence, and referencePlanFilesChecked from uiProductionBrief.");
        rules.push("A satisfied frontendQualitySelfCheck must include at least one non-empty evidenceRefs entry; this is the trace from the persisted TaskResult to concrete source, test, command, or browser evidence used by Review.");
        rules.push("frontendQualitySelfCheck must not include removed legacy UI quality self-check fields such as scenarioKind, referenceFilesChecked, statesCovered, surfacesCovered, or gateResults.");
        rules.push("frontendQualitySelfCheck.status=satisfied is valid only when contentBoundaryEvidence has no forbidden content violations and knownGaps is empty.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_BROWSER_VERIFICATION_INVALID")
    {
        rules.push("Keep browser check entries in the request order and repair command, attempts, status, observed outcome, and blockedReason only; Loom derives check ids and verification ownership.");
        rules.push("Do not turn failed, blocked, or not-run browser evidence into passed evidence. Passed checks require the actual command, attempts, and observed outcome; blocked checks require a concrete blockedReason.");
        rules.push("Keep retry-only success visible by preserving attempts greater than one; do not flatten it into a first-attempt pass.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_ARCHITECTURE_QUALITY_INVALID")
    {
        rules.push("architectureQualityEvidence must cover every task.architectureQualityRequirementRefs item when the task is completed or completed_with_notes.");
        rules.push("Repair architectureQualityEvidence content in task requirement order; Loom derives requirementId and verificationIds.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_API_CONTRACT_INVALID")
    {
        rules.push("apiContractEvidence must cover every task.apiContractRequirementRefs item when the task is completed or completed_with_notes.");
        rules.push("Repair apiContractEvidence content in task requirement order; Loom derives requirementId, interfaceRefs, and verificationIds.");
    }
    if issues
        .iter()
        .any(|issue| issue.code == "TASK_RESULT_CODE_QUALITY_INVALID")
    {
        rules.push("codeQualityEvidence must cover every task.codeQualityRequirementRefs item when the task is completed or completed_with_notes.");
        rules.push("codeQualityEvidence.referenceGroupsChecked must exactly match the selected language/framework groups for the assigned code quality requirement.");
        rules.push("codeQualityEvidence.referenceFilesChecked must exactly list files from sourceContext.codeQualityExecutionContext[].referenceLoadPlan that were read for the task.");
        rules.push("Repair codeQualityEvidence content in task requirement order; Loom derives requirementId and verificationIds.");
    }
    rules
}

fn task_result_repair_template(
    context: &RepairContextInput,
    issues: &[delivery_core::RepairIssue],
) -> Value {
    let mut template = task_result_template_with_code_quality(
        &context.task,
        &context.code_quality_requirements,
        context.browser_profile.as_ref(),
    );
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
        if key == "requirementDetailEvidence" {
            continue;
        }
        if conflicted_fields.contains(key.as_str()) {
            continue;
        }
        if !template_object.contains_key(key) {
            continue;
        }
        let Some(agent_value) = project_agent_owned_task_result_field(key, submitted_value) else {
            continue;
        };
        if keeps_template_array_shape(template_object.get(key), &agent_value) {
            continue;
        }
        template_object.insert(key.clone(), agent_value);
    }
}

fn project_agent_owned_task_result_field(key: &str, value: &Value) -> Option<Value> {
    if key == "frontendExperienceSelfCheck" {
        let mut projected =
            compact_object_fields(value, &["status", "dataBinding", "evidenceRefs"]);
        if let Some(binding) = projected.get_mut("dataBinding") {
            *binding = compact_object_fields(binding, &["mode", "knownGaps"]);
        }
        return Some(projected);
    }
    if key == "frontendQualitySelfCheck" {
        let mut projected = compact_object_fields(
            value,
            &[
                "status",
                "evidenceRefs",
                "surfaceRegionEvidence",
                "surfaceActionEvidence",
                "surfaceStateEvidence",
                "surfaceQualityRuleEvidence",
                "contentBoundaryEvidence",
                "referencePlanFilesChecked",
                "designTokenEvidence",
                "knownGaps",
            ],
        );
        for field in [
            "surfaceRegionEvidence",
            "surfaceActionEvidence",
            "surfaceStateEvidence",
            "surfaceQualityRuleEvidence",
        ] {
            if let Some(items) = projected.get_mut(field) {
                *items = compact_array_items(items, &["status", "files", "evidence"]);
            }
        }
        if let Some(content) = projected.get_mut("contentBoundaryEvidence") {
            *content = compact_object_fields(
                content,
                &[
                    "checked",
                    "allowedContentExamples",
                    "forbiddenContentViolations",
                    "evidence",
                ],
            );
        }
        if let Some(tokens) = projected.get_mut("designTokenEvidence") {
            *tokens = compact_object_fields(
                tokens,
                &[
                    "strategyUsed",
                    "templateIdUsed",
                    "tokenAssetFiles",
                    "tokenConsumerFiles",
                    "existingTokenSystemReused",
                    "parallelTokenSystemCreated",
                    "mergeSummary",
                ],
            );
        }
        return Some(projected);
    }
    if key == "runtimeDeliveryEvidence" {
        let mut projected = compact_object_fields(
            value,
            &[
                "codeLevelChecks",
                "commandsRun",
                "unverifiedItems",
                "runtimeProbeCleanup",
            ],
        );
        if let Some(checks) = projected.get_mut("codeLevelChecks") {
            *checks = compact_array_items(checks, &["status", "evidence"]);
        }
        return Some(projected);
    }
    if !matches!(
        key,
        "verificationResults"
            | "implementationObligationResults"
            | "conceptEvidence"
            | "architectureQualityEvidence"
            | "apiContractEvidence"
            | "codeQualityEvidence"
    ) {
        return Some(value.clone());
    }
    let items = value.as_array()?;
    let projected = match key {
        "verificationResults" => items
            .iter()
            .map(|item| {
                let mut item = compact_object_fields(
                    item,
                    &[
                        "status",
                        "evidenceType",
                        "summary",
                        "provenance",
                        "browserChecks",
                    ],
                );
                if let Some(provenance) = item.get_mut("provenance") {
                    *provenance = compact_object_fields(
                        provenance,
                        &[
                            "evidenceRefs",
                            "changedFiles",
                            "testCaseRefs",
                            "command",
                            "exitCode",
                        ],
                    );
                }
                if let Some(checks) = item.get_mut("browserChecks") {
                    *checks = compact_array_items(
                        checks,
                        &[
                            "status",
                            "command",
                            "attempts",
                            "artifactRefs",
                            "observedOutcome",
                            "blockedReason",
                        ],
                    );
                }
                item
            })
            .collect::<Vec<_>>(),
        "implementationObligationResults" => items
            .iter()
            .map(|item| compact_object_fields(item, &["status", "evidenceRefs", "summary"]))
            .collect::<Vec<_>>(),
        "conceptEvidence" => items
            .iter()
            .map(|item| compact_object_fields(item, &["evidenceType", "refs", "summary"]))
            .collect::<Vec<_>>(),
        "architectureQualityEvidence" => items
            .iter()
            .map(|item| compact_object_fields(item, &["status", "summary"]))
            .collect::<Vec<_>>(),
        "apiContractEvidence" => items
            .iter()
            .map(|item| compact_object_fields(item, &["status", "knownGaps", "summary"]))
            .collect::<Vec<_>>(),
        "codeQualityEvidence" => items
            .iter()
            .map(|item| {
                compact_object_fields(
                    item,
                    &[
                        "status",
                        "referenceGroupsChecked",
                        "referenceFilesChecked",
                        "knownGaps",
                        "summary",
                    ],
                )
            })
            .collect::<Vec<_>>(),
        _ => unreachable!("known task result field was not projected"),
    };
    Some(Value::Array(projected))
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
    let mut status = store.load_status(project_root).map_err(to_state_error)?;
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
    delivery.status = DeliveryLifecycleStatus::Executing;
    delivery.updated_at = state::store::now_string();
    store
        .commit_delivery_state(project_root, &delivery, &mut status)
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
    let next_is_execution = next_action
        .map(|action| {
            matches!(
                action.kind,
                RouteActionKind::ContinueExecution | RouteActionKind::ExecutionRepair
            )
        })
        .unwrap_or(false);
    delivery.status = if next_is_execution
        || matches!(
            run.status,
            TaskPlanRunStatus::Running | TaskPlanRunStatus::NotStarted
        ) {
        DeliveryLifecycleStatus::Executing
    } else {
        DeliveryLifecycleStatus::Reviewing
    };
    delivery.updated_at = state::store::now_string();
    store
        .commit_delivery_state(project_root, &delivery, &mut status)
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
        stop_allowed: false,
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
        agent_instruction: delivery_core::repairable_error_agent_instruction(resubmit_tool),
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

fn request_id_from_ref(request_ref: &str) -> Result<String, state::store::StateError> {
    request_ref
        .split("/requests/")
        .nth(1)
        .filter(|value| !value.is_empty() && !value.contains('/'))
        .map(str::to_string)
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(format!("invalid requestRef: {request_ref}"))
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

fn read_project_json_value(
    project_root: &Path,
    relative: &str,
) -> Result<Value, state::store::StateError> {
    let path = from_project_relative(project_root, relative)?;
    state::store::read_json_value(&path)
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::from_core_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser_profile() -> BrowserVerificationProfile {
        BrowserVerificationProfile {
            profile_id: "browser-task-ui".to_string(),
            task_id: "task-ui".to_string(),
            mode: contracts::BrowserVerificationMode::RenderedInspection,
            runner_source: contracts::BrowserRunnerSource::LoomManaged,
            installation_id: None,
            verification_ids: vec!["verify-ui".to_string()],
            surface_refs: vec![],
            workflow_refs: vec![],
            region_refs: vec![],
            action_refs: vec![],
            state_refs: vec![],
            quality_rule_refs: vec![],
            checks: vec![contracts::BrowserVerificationCheck {
                check_id: "browser-ui-desktop".to_string(),
                verification_id: "verify-ui".to_string(),
                source_task_id: "task-ui".to_string(),
                source_verification_id: "verify-ui".to_string(),
                enforcement: contracts::BrowserEvidenceEnforcement::Supplemental,
                viewport_ref: "desktop_primary".to_string(),
                backend_mode: contracts::BrowserBackendMode::NotApplicable,
            }],
            reference_load_plan: vec![],
        }
    }

    fn browser_task_result(check: Value) -> TaskResult {
        serde_json::from_value(json!({
            "schemaVersion": "1.0",
            "taskResultId": "result-ui",
            "taskId": "task-ui",
            "taskPlanId": "taskplan",
            "status": "completed",
            "changedFiles": ["web/src/App.tsx"],
            "verificationResults": [{
                "verificationId": "verify-ui",
                "status": "passed",
                "evidenceType": "automated_test",
                "summary": "Browser verification completed.",
                "browserChecks": [check]
            }],
            "executionContinuity": {
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none"
            },
            "createdAt": "2026-07-13T10:00:00+08:00",
            "updatedAt": "2026-07-13T10:00:00+08:00"
        }))
        .expect("browser TaskResult")
    }

    fn surface_requirement() -> Value {
        json!({
            "uiSurfaceDecisionContractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
            "executionGuidance": {
                "styleAssetPlan": {
                    "referencePlan": [{
                        "path": "plugins/shared/loom/references/uix/core.md"
                    }]
                }
            }
        })
    }

    fn surface_contract() -> Value {
        json!({
            "contractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
            "regionsInScope": [{
                "regionId": "region-main"
            }],
            "actionsInScope": [{
                "actionId": "action-submit"
            }],
            "statesInScope": [{
                "state": "empty"
            }],
            "qualityRulesInScope": [{
                "ruleId": "rule-density"
            }]
        })
    }

    fn complete_surface_self_check() -> Value {
        json!({
            "status": "satisfied",
            "surfaceDecisionContractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
            "surfaceRegionEvidence": [{
                "id": "region-main",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "states": ["empty"],
                "actions": ["action-submit"],
                "evidence": "Main region implements the scoped workbench area."
            }],
            "surfaceActionEvidence": [{
                "id": "action-submit",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "states": ["empty"],
                "actions": ["action-submit"],
                "evidence": "Submit action is available in the task-owned form."
            }],
            "surfaceStateEvidence": [{
                "id": "empty",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "states": ["empty"],
                "actions": [],
                "evidence": "Empty state is rendered before records exist."
            }],
            "surfaceQualityRuleEvidence": [{
                "id": "rule-density",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "states": ["empty"],
                "actions": [],
                "evidence": "Layout uses compact internal-product density."
            }],
            "contentBoundaryEvidence": {
                "checked": true,
                "allowedContentExamples": ["business labels only"],
                "forbiddenContentViolations": [],
                "evidence": "No runtime commands or delivery notes are visible."
            },
            "referencePlanFilesChecked": ["plugins/shared/loom/references/uix/core.md"]
        })
    }

    fn compact_projection_task() -> TaskDefinition {
        TaskDefinition {
            task_id: "task-ui".to_string(),
            group_id: "group-ui".to_string(),
            title: "Implement the workbench".to_string(),
            task_kind: TaskKind::FrontendExperience,
            implementation_actions: vec![],
            implementation_obligations: vec![],
            objective: "Implement the task-owned workbench flow.".to_string(),
            depends_on: vec![],
            scope_refs: vec!["scope-ui".to_string()],
            acceptance_refs: vec!["accept-ui".to_string()],
            requirement_detail_refs: vec!["detail-ui".to_string()],
            write_boundary: contracts::TaskWriteBoundary {
                forbidden_paths: vec![".loom".to_string()],
                artifact_refs: contracts::TaskArtifactRefs::default(),
            },
            verification_intents: vec![],
            concept_refs: vec![],
            concept_responsibilities: vec![],
            concept_verification_intents: vec![],
            frontend_experience_requirement: Some(json!({
                "uiSurfaceDecisionContractRef": "surface-contract",
                "executionGuidance": {
                    "closureRequirementRefs": ["closure:workbench:submit"],
                    "uiProductionBrief": {
                        "appliesTo": {"surfaceIds": ["surface-workbench"]},
                        "surfaceDecisionContract": {
                            "contractRef": "surface-contract",
                            "semanticFacts": {"businessObject": "ticket"},
                            "regionsInScope": [{"regionId": "region-main", "name": "Main"}],
                            "actionsInScope": [{"actionId": "action-submit", "label": "Submit"}],
                            "statesInScope": [{"state": "loading"}],
                            "qualityRulesInScope": [{"ruleId": "rule-density"}],
                            "unrelatedLargeSource": "do not copy this source"
                        }
                    },
                    "styleAssetPlan": {
                        "referencePlan": [{"path": "uix/core.md", "reason": "UI quality"}],
                        "designTokenAssetPlan": {"strategy": "reuse_existing"}
                    }
                }
            })),
            runtime_delivery_requirement: None,
            engineering_quality_requirement_refs: vec![],
            architecture_quality_requirement_refs: vec![],
            api_contract_requirement_refs: vec![],
            code_quality_requirement_refs: vec![],
        }
    }

    #[test]
    fn task_result_repair_projection_keeps_task_contract_without_full_task_copy() {
        let mut task = compact_projection_task();
        task.implementation_obligations = vec![serde_json::from_value(json!({
            "obligationId": "obligation-task-ui-frontend-experience",
            "kind": "frontend_experience",
            "sourceRefs": ["detail-ui"],
            "artifactRefs": {},
            "requiredOutcome": "Implement the task-owned frontend surface.",
            "required": true,
            "acceptableEvidence": ["browser_automation"],
            "verificationIds": ["verify-ui"],
            "deferPolicy": "must_be_satisfied_before_completed"
        }))
        .expect("implementation obligation")];
        let projection = task_projection(&task);

        assert_eq!(projection["projectionKind"], "task_scoped_repair_contract");
        assert_eq!(
            projection["implementationObligations"][0]["obligationId"],
            "obligation-task-ui-frontend-experience"
        );
        assert!(projection.get("conceptResponsibilities").is_none());
        assert!(projection
            .pointer("/frontendExperienceRequirement/executionGuidance/uiProductionBrief/surfaceDecisionContract/regionsInScope/0/regionId")
            .is_some());
        assert!(projection
            .pointer("/frontendExperienceRequirement/executionGuidance/styleAssetPlan/referencePlan/0/path")
            .is_some());
        assert!(projection
            .pointer("/frontendExperienceRequirement/executionGuidance/uiProductionBrief/surfaceDecisionContract/unrelatedLargeSource")
            .is_none());
    }

    #[test]
    fn implementation_obligation_normalization_matches_ids_instead_of_positions() {
        let mut task = compact_projection_task();
        task.implementation_obligations = serde_json::from_value(json!([
            {
                "obligationId": "obligation-a",
                "kind": "business_rule",
                "artifactRefs": {},
                "requiredOutcome": "Implement rule A.",
                "required": true,
                "acceptableEvidence": ["automated_test"],
                "verificationIds": ["verify-a"],
                "deferPolicy": "must_be_satisfied_before_completed"
            },
            {
                "obligationId": "obligation-b",
                "kind": "state_transition",
                "artifactRefs": {},
                "requiredOutcome": "Implement state B.",
                "required": true,
                "acceptableEvidence": ["runtime_api_check"],
                "verificationIds": ["verify-b"],
                "deferPolicy": "must_be_satisfied_before_completed"
            }
        ]))
        .expect("implementation obligations");
        let mut object = json!({
            "implementationObligationResults": [
                {"obligationId": "obligation-b", "status": "satisfied", "summary": "B"},
                {"obligationId": "obsolete", "status": "partial", "summary": "obsolete"},
                {"obligationId": "obligation-a", "status": "satisfied", "summary": "A"}
            ]
        });

        normalize_implementation_obligation_results(
            object.as_object_mut().expect("result object"),
            &task,
        );

        assert_eq!(
            object["implementationObligationResults"][0]["obligationId"],
            "obligation-a"
        );
        assert_eq!(object["implementationObligationResults"][0]["summary"], "A");
        assert_eq!(
            object["implementationObligationResults"][1]["obligationId"],
            "obligation-b"
        );
        assert_eq!(object["implementationObligationResults"][1]["summary"], "B");
        assert_eq!(
            object["implementationObligationResults"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn task_result_repair_contract_rebuild_has_no_terminal_failure_path() {
        let task = compact_projection_task();
        let required = task_result_required_top_level_fields(&task)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut schema = json!({"type": "object", "properties": {}});
        let mut template = json!({
            "status": "completed",
            "frontendQualitySelfCheck": {}
        });

        assert!(
            task_result_repair_contract_missing_fields(&task, &required, &schema, &template)
                .is_some()
        );
        let canonical_schema = task_result_schema_shape(&task, None);
        let canonical_template = task_result_template_with_code_quality(&task, &[], None);
        schema = canonical_schema;
        template = canonical_template;
        ensure_repair_contract_fields(&required, &mut schema, &mut template);

        assert!(
            task_result_repair_contract_missing_fields(&task, &required, &schema, &template)
                .is_none()
        );
    }

    #[test]
    fn quality_evidence_links_only_to_passed_verifications() {
        let mut task = compact_projection_task();
        task.api_contract_requirement_refs = vec!["api-1".to_string()];
        task.code_quality_requirement_refs = vec!["code-1".to_string()];
        task.verification_intents = serde_json::from_value(json!([
            {
                "verificationId": "verify-passed",
                "behavior": "A passed contract check",
                "acceptableEvidence": ["automated_test"]
            },
            {
                "verificationId": "verify-not-run",
                "behavior": "An unrelated check not run",
                "acceptableEvidence": ["manual_command_output"]
            }
        ]))
        .expect("verification intents");
        let mut result = json!({
            "status": "completed_with_notes",
            "verificationResults": [
                {"verificationId": "verify-passed", "status": "passed"},
                {"verificationId": "verify-not-run", "status": "not_run"}
            ],
            "apiContractEvidence": [{"status": "satisfied", "summary": "API evidence"}],
            "codeQualityEvidence": [{"status": "satisfied", "summary": "Code evidence"}]
        });

        normalize_quality_evidence_machine_fields(
            result.as_object_mut().expect("result object"),
            &task,
        );

        assert_eq!(
            result["apiContractEvidence"][0]["verificationIds"],
            json!(["verify-passed"])
        );
        assert_eq!(
            result["codeQualityEvidence"][0]["verificationIds"],
            json!(["verify-passed"])
        );
    }

    #[test]
    fn frontend_surface_contract_evidence_accepts_complete_task_scope() {
        let mut issues = Vec::new();
        validate_surface_decision_contract_evidence(
            &complete_surface_self_check(),
            &surface_requirement(),
            &surface_contract(),
            &mut issues,
        );

        assert!(issues.is_empty(), "{issues:#?}");
    }

    #[test]
    fn frontend_surface_contract_evidence_rejects_missing_quality_rule() {
        let mut self_check = complete_surface_self_check();
        self_check["surfaceQualityRuleEvidence"] = json!([]);
        let mut issues = Vec::new();

        validate_surface_decision_contract_evidence(
            &self_check,
            &surface_requirement(),
            &surface_contract(),
            &mut issues,
        );

        assert!(
            issues.iter().any(|issue| issue.field_path.as_deref()
                == Some("frontendQualitySelfCheck.surfaceQualityRuleEvidence")),
            "{issues:#?}"
        );
    }

    #[test]
    fn frontend_surface_evidence_projection_discards_legacy_unconsumed_fields() {
        let mut self_check = json!({
            "surfaceRegionEvidence": [{
                "id": "region_primary",
                "status": "satisfied",
                "files": ["src/App.tsx"],
                "evidence": "The primary region is implemented.",
                "states": ["loading", "success"],
                "actions": ["create"],
                "summary": "Legacy summary with no review consumer."
            }]
        })
        .as_object_mut()
        .cloned()
        .expect("self check object");

        normalize_frontend_surface_evidence_array(
            &mut self_check,
            "surfaceRegionEvidence",
            &surface_contract(),
            "regionsInScope",
            "regionId",
        );

        let evidence = &self_check["surfaceRegionEvidence"][0];
        assert_eq!(evidence["id"], json!("region-main"));
        assert_eq!(evidence["status"], json!("satisfied"));
        assert_eq!(evidence["files"], json!(["src/App.tsx"]));
        assert_eq!(
            evidence["evidence"],
            json!("The primary region is implemented.")
        );
        assert!(evidence.get("states").is_none());
        assert!(evidence.get("actions").is_none());
        assert!(evidence.get("summary").is_none());
    }

    #[test]
    fn browser_check_normalization_adds_contract_shape_without_claiming_evidence() {
        let profile = browser_profile();
        let mut verification = serde_json::Map::new();

        normalize_browser_check_machine_fields(&mut verification, "verify-ui", Some(&profile));

        assert_eq!(
            verification["browserChecks"][0]["checkId"],
            "browser-ui-desktop"
        );
        assert_eq!(verification["browserChecks"][0]["status"], "not_run");
        assert_eq!(verification["browserChecks"][0]["attempts"], 0);
        assert_eq!(verification["browserChecks"][0]["observedOutcome"], "");
        assert_eq!(verification["evidenceType"], "browser_automation");
    }

    #[test]
    fn browser_check_normalization_does_not_rebind_wrong_id_evidence_by_position() {
        let profile = browser_profile();
        let mut verification = json!({
            "browserChecks": [{
                "checkId": "browser-unrelated",
                "status": "passed",
                "command": "pnpm playwright test unrelated.spec.ts",
                "attempts": 1,
                "artifactRefs": [],
                "observedOutcome": "An unrelated check passed.",
                "blockedReason": null
            }]
        })
        .as_object()
        .cloned()
        .unwrap();

        normalize_browser_check_machine_fields(&mut verification, "verify-ui", Some(&profile));

        assert_eq!(
            verification["browserChecks"][0]["checkId"],
            "browser-ui-desktop"
        );
        assert_eq!(verification["browserChecks"][0]["status"], "not_run");
        assert_eq!(verification["browserChecks"][0]["attempts"], 0);
    }

    #[test]
    fn browser_verification_accepts_visible_retry_success() {
        let profile = browser_profile();
        let result = browser_task_result(json!({
            "checkId": "browser-ui-desktop",
            "status": "passed",
            "command": "pnpm playwright test --grep workflow",
            "attempts": 2,
            "artifactRefs": ["test-results/workflow/trace.zip"],
            "observedOutcome": "The submitted record appears in the rendered list.",
            "blockedReason": null
        }));
        let mut issues = Vec::new();

        validate_browser_verification_results(&result, Some(&profile), &mut issues);

        assert!(issues.is_empty(), "{issues:#?}");
        assert_eq!(result.verification_results[0].browser_checks[0].attempts, 2);
    }

    #[test]
    fn browser_verification_rejects_completed_not_run_check() {
        let mut profile = browser_profile();
        profile.checks[0].enforcement = contracts::BrowserEvidenceEnforcement::Required;
        let result = browser_task_result(json!({
            "checkId": "browser-ui-desktop",
            "status": "not_run",
            "command": "",
            "attempts": 0,
            "artifactRefs": [],
            "observedOutcome": "",
            "blockedReason": null
        }));
        let mut issues = Vec::new();

        validate_browser_verification_results(&result, Some(&profile), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue.code == "TASK_RESULT_BROWSER_VERIFICATION_INVALID"
                && issue.field_path.as_deref()
                    == Some("verificationResults[].browserChecks[].status")
        }));
    }

    #[test]
    fn required_environment_blocker_is_normalized_to_reviewable_completion() {
        let mut profile = browser_profile();
        profile.checks[0].enforcement = contracts::BrowserEvidenceEnforcement::Required;
        let mut value = serde_json::to_value(browser_task_result(json!({
            "checkId": "browser-ui-desktop",
            "status": "blocked",
            "command": "",
            "attempts": 1,
            "artifactRefs": [],
            "observedOutcome": "",
            "blockedReason": "Chromium stopped launching after runtime preparation."
        })))
        .unwrap();
        value["status"] = json!("failed");
        value["failure"] = json!({
            "code": "BROWSER_LAUNCH_FAILED",
            "summary": "Chromium did not launch."
        });
        let object = value.as_object_mut().unwrap();

        normalize_browser_environment_blocked_result(object, Some(&profile));

        assert_eq!(object["status"], "completed_with_notes");
        assert!(object["failure"].is_null());
        assert_eq!(object["verificationResults"][0]["status"], "inconclusive");
        let result: TaskResult = serde_json::from_value(value).unwrap();
        let mut issues = Vec::new();
        validate_browser_verification_results(&result, Some(&profile), &mut issues);
        assert!(issues.is_empty(), "{issues:#?}");
    }

    #[test]
    fn failed_browser_assertion_is_not_reclassified_as_environment_blocker() {
        let mut profile = browser_profile();
        profile.checks[0].enforcement = contracts::BrowserEvidenceEnforcement::Required;
        let mut value = serde_json::to_value(browser_task_result(json!({
            "checkId": "browser-ui-desktop",
            "status": "failed",
            "command": "pnpm playwright test",
            "attempts": 1,
            "artifactRefs": [],
            "observedOutcome": "The submit action returned no success feedback.",
            "blockedReason": null
        })))
        .unwrap();
        value["status"] = json!("failed");
        let object = value.as_object_mut().unwrap();

        normalize_browser_environment_blocked_result(object, Some(&profile));

        assert_eq!(object["status"], "failed");
    }

    #[test]
    fn browser_verification_allows_completed_supplemental_gap() {
        let profile = browser_profile();
        let result = browser_task_result(json!({
            "checkId": "browser-ui-desktop",
            "status": "blocked",
            "command": "",
            "attempts": 0,
            "artifactRefs": [],
            "observedOutcome": "",
            "blockedReason": "Browser execution is unavailable in this environment."
        }));
        let mut issues = Vec::new();

        validate_browser_verification_results(&result, Some(&profile), &mut issues);

        assert!(issues.is_empty(), "{issues:#?}");
    }
}
