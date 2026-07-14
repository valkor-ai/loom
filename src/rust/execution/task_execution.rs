use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    ApiContractRequirement, ArchitectureArtifactContract, ArchitectureQualityRequirement,
    BrowserVerificationProfile, EngineeringQualityRequirement, ImplementationAction,
    TaskAttemptState, TaskDefinition, TaskKind, TaskPlan, TaskPlanRun, TaskPlanRunNextAction,
    TaskPlanRunStatus, TaskResult, TaskRunStatus, VerificationEvidence,
};
use delivery_core::{
    apply_delivery_index, read_selectors_value_from_paths, DeliveryLifecycleStatus,
    LoomMcpActionResult, LoomMcpAutoRunnableResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpNextAction, RouteAction, RouteActionKind, RunLoomToolNext, TransitionStore,
};
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
};

use crate::{
    paths::{
        task_execution_request_file, task_execution_result_candidate_file, task_plan_file,
        task_plan_latest_file, task_plan_run_file, task_plan_run_latest_file, task_result_file,
    },
    task_plan::{
        execute_task_next_from_request, update_run_summary, UI_OWNERSHIP_DIMENSION_VALUES,
    },
    templates::{
        code_quality_execution_context, code_quality_requirements_for_task,
        frontend_quality_self_check_applies, frontend_self_check_applies,
        runtime_delivery_evidence_applies, task_result_required_top_level_fields,
        task_result_schema_shape, task_result_template_with_code_quality,
    },
};

pub fn continue_execution(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match continue_execution_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: project_root.to_string(),
            error: LoomMcpFailure {
                code: "TASK_EXECUTION_MATERIALIZE_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("execution".to_string()),
                route_action: Some("continue_execution".to_string()),
                recovery_tool: Some("loom.continue".to_string()),
            },
        }),
    }
}

fn continue_execution_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let (task_plan, mut run) = load_current_plan_and_run(root, &locator)?;
    let Some(task_id) = running_or_ready_task_id(&run) else {
        update_route_for_review(project_root, delivery_id, phase_id)?;
        return Ok(crate::review::materialize_review_request(
            project_root,
            delivery_id,
            phase_id,
        ));
    };
    let task = task_plan
        .tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "TaskPlanRun references missing task {task_id}"
            ))
        })?;
    if matches!(task.task_kind, TaskKind::BrowserQualityClosure)
        && crate::browser::browser_runtime_preparation_state(root)
            == crate::browser::BrowserRuntimePreparationState::Unavailable
    {
        return close_unavailable_browser_environment(
            project_root,
            &locator,
            &task_plan,
            &mut run,
            &task,
        );
    }
    if matches!(task.task_kind, TaskKind::BrowserQualityClosure)
        && crate::browser::browser_runtime_preparation_state(root)
            == crate::browser::BrowserRuntimePreparationState::NeedsPreparation
    {
        return materialize_browser_runtime_prepare_action(
            project_root,
            &locator,
            &task_plan,
            &task,
        );
    }
    if let Some(existing) =
        existing_execution_next_if_current(project_root, delivery_id, phase_id, &task)?
    {
        return Ok(existing);
    }

    let now = state::store::now_string();
    if let Some(state) = run
        .task_states
        .iter_mut()
        .find(|state| state.task_id == task.task_id)
    {
        if state.status == TaskRunStatus::Pending {
            state.status = TaskRunStatus::Running;
            state.started_at = Some(now.clone());
        }
    }
    if let Some(group) = run
        .group_states
        .iter_mut()
        .find(|group| group.group_id == task.group_id)
    {
        if group.status == TaskRunStatus::Pending {
            group.status = TaskRunStatus::Running;
            group.started_at = Some(now.clone());
        }
    }
    run.status = TaskPlanRunStatus::Running;
    run.scheduler.started_at.get_or_insert(now.clone());
    run.next_action = Some(TaskPlanRunNextAction {
        r#type: "continue_execution".to_string(),
        reason: "TASK_READY".to_string(),
        source_task_id: Some(task.task_id.clone()),
        target_node: "task_execution".to_string(),
    });
    run.updated_at = now;
    update_run_summary(&mut run);
    save_run(root, &locator, &run)?;

    let request_id = format!(
        "exec_{}_{}",
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
    let request_root = build_execution_request(
        project_root,
        &locator,
        &request_id,
        &result_file,
        &task_plan,
        &run,
        &task,
    )?;
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "task_execution_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    if let Some(parent) = from_project_relative(root, &result_file)?.parent() {
        state::store::ensure_dir(parent)?;
    }
    update_route_for_execution(
        project_root,
        delivery_id,
        phase_id,
        &stored.request_ref,
        &result_file,
        &task,
        &run,
    )?;
    execute_task_next_from_request(project_root, &stored.request_ref, &task, result_file)
}

fn materialize_browser_runtime_prepare_action(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    task_plan: &TaskPlan,
    task: &TaskDefinition,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let profile = task_plan
        .browser_verification_profiles
        .iter()
        .find(|profile| profile.task_id == task.task_id)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser quality closure is missing its verification profile".to_string(),
            )
        })?;
    let request_id = format!("browser_runtime_prepare_{}", state::store::now_millis());
    let request_file = to_project_relative(
        root,
        &task_execution_request_file(root, locator, &request_id),
    )?;
    let request_root = json!({
        "schemaVersion": "1.0",
        "requestType": "browser_runtime_prepare",
        "source": {
            "taskPlanId": task_plan.task_plan_id,
            "taskId": task.task_id,
            "profileId": profile.profile_id
        },
        "browserRuntimePreparation": {
            "projectTargets": crate::browser::browser_runtime_targets(root),
            "requestedBrowsers": ["chromium"],
            "policy": "Resolve exact project versions, try host launch, then managed container fallback."
        },
        "requestReadPlan": {"groups": [{
            "groupId": "browser_runtime_prepare_context",
            "required": true,
            "purpose": "Read the exact project targets and runtime fallback policy.",
            "whenToRead": "Read before calling loom.browserRuntimePrepare.",
            "selectors": read_selectors_value_from_paths([
                "source.taskPlanId",
                "source.taskId",
                "source.profileId",
                "browserRuntimePreparation.projectTargets",
                "browserRuntimePreparation.requestedBrowsers",
                "browserRuntimePreparation.policy"
            ])
        }]}
    });
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id,
            request_kind: "browser_runtime_prepare_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(locator.delivery_id.clone()),
            phase_id: Some(locator.phase_id.clone()),
            root: request_root,
        },
    )?;
    update_route_for_browser_runtime_prepare(project_root, locator, &stored.request_ref, task)?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::RunLoomTool(RunLoomToolNext {
                tool_name: "loom.browserRuntimePrepare".to_string(),
                request_ref: stored.request_ref,
                read_groups: stored.read_groups,
                retry_tool: "loom.continue".to_string(),
            }),
        ),
    ))
}

fn close_unavailable_browser_environment(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    task_plan: &TaskPlan,
    run: &mut TaskPlanRun,
    task: &TaskDefinition,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let profile = task_plan
        .browser_verification_profiles
        .iter()
        .find(|profile| profile.task_id == task.task_id)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser quality closure is missing its MCP verification profile".to_string(),
            )
        })?;
    let runtime_state =
        state::store::read_json_value(&root.join(".loom/runtime/browser-automation/latest.json"))?;
    let diagnostic = runtime_state
        .pointer("/runtime/runtimes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|runtime| {
            runtime
                .get("doctorChecks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|check| check.get("status").and_then(Value::as_str) == Some("failed"))
        .filter_map(|check| check.get("summary").and_then(Value::as_str))
        .take(4)
        .collect::<Vec<_>>()
        .join(" ");
    let blocked_reason = if diagnostic.is_empty() {
        "Browser launch is unavailable on both the host and Loom-managed container.".to_string()
    } else {
        diagnostic
    };
    let verification_results = task
        .verification_intents
        .iter()
        .map(|intent| {
            json!({
                "verificationId": intent.verification_id,
                "status": "inconclusive",
                "evidenceType": "browser_automation",
                "summary": "Browser evidence could not run because both supported execution environments are unavailable.",
                "browserChecks": profile.checks.iter()
                    .filter(|check| check.verification_id == intent.verification_id)
                    .map(|check| json!({
                        "checkId": check.check_id,
                        "status": "blocked",
                        "command": "",
                        "attempts": 0,
                        "artifactRefs": [],
                        "observedOutcome": "",
                        "blockedReason": blocked_reason.clone()
                    }))
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let now = state::store::now_string();
    let result_id = format!("system-browser-environment-{}", state::store::now_millis());
    let result: TaskResult = serde_json::from_value(json!({
        "schemaVersion": "1.0",
        "taskResultId": result_id,
        "taskId": task.task_id,
        "taskPlanId": task_plan.task_plan_id,
        "status": "completed_with_notes",
        "changedFiles": [],
        "noChangeReason": {
            "code": "ENVIRONMENT_CHECK_ONLY",
            "summary": "MCP closed the browser environment check without changing project files."
        },
        "verificationResults": verification_results,
        "executionContinuity": {
            "taskResultSubmittedAfterVerification": true,
            "agentOwnedLongRunningWork": "none",
            "notes": ["Browser environment failure was classified by MCP and did not enter execution repair."]
        },
        "notes": [blocked_reason],
        "createdAt": now.clone(),
        "updatedAt": now.clone()
    }))
    .map_err(state::store::StateError::Json)?;
    let result_path = task_result_file(
        root,
        locator,
        &run.run_id,
        &task.task_id,
        &result.task_result_id,
    );
    state::store::write_json_atomic(&result_path, &result)?;

    if let Some(state) = run
        .task_states
        .iter_mut()
        .find(|state| state.task_id == task.task_id)
    {
        state.status = TaskRunStatus::CompletedWithNotes;
        state.result_id = Some(result.task_result_id.clone());
        state.finished_at = Some(now.clone());
        state.attempts.push(TaskAttemptState {
            attempt: state.attempts.len() as u32 + 1,
            result_id: result.task_result_id.clone(),
            status: TaskRunStatus::CompletedWithNotes,
        });
    }
    if let Some(group) = run
        .group_states
        .iter_mut()
        .find(|group| group.group_id == task.group_id)
    {
        group.status = TaskRunStatus::CompletedWithNotes;
        group.finished_at = Some(now.clone());
    }
    update_run_summary(run);
    run.status = if run.summary.pending == 0 && run.summary.running == 0 {
        TaskPlanRunStatus::CompletedWithNotes
    } else {
        TaskPlanRunStatus::Running
    };
    run.next_action = Some(TaskPlanRunNextAction {
        r#type: "review".to_string(),
        reason: "BROWSER_ENVIRONMENT_REQUIRES_REVIEW".to_string(),
        source_task_id: Some(task.task_id.clone()),
        target_node: "review".to_string(),
    });
    run.updated_at = now;
    save_run(root, locator, run)?;
    update_route_for_review(project_root, &locator.delivery_id, &locator.phase_id)?;
    Ok(crate::review::materialize_review_request(
        project_root,
        &locator.delivery_id,
        &locator.phase_id,
    ))
}

fn existing_execution_next_if_current(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    task: &TaskDefinition,
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
    if action.kind != RouteActionKind::ContinueExecution
        || action.source != "task_execution_request"
    {
        return Ok(None);
    }
    if action
        .details
        .as_ref()
        .and_then(|details| details.get("taskId"))
        .and_then(Value::as_str)
        != Some(task.task_id.as_str())
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
    let Some(result_file) = phase.latest_refs.get("taskExecutionResultFile") else {
        return Ok(None);
    };
    execute_task_next_from_request(project_root, request_ref, task, result_file.clone()).map(Some)
}

fn build_execution_request(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
    result_file: &str,
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    task: &TaskDefinition,
) -> Result<Value, state::store::StateError> {
    let root = Path::new(project_root);
    let baseline_ref = format!(
        ".loom/deliveries/{}/contracts/technical-baseline.json",
        locator.delivery_id
    );
    let planning_ref = format!(
        ".loom/deliveries/{}/contracts/planning/{}/pgc.json",
        locator.delivery_id, locator.phase_id
    );
    let architecture_ref = format!(
        ".loom/deliveries/{}/contracts/architecture/{}/aac.json",
        locator.delivery_id, locator.phase_id
    );
    let baseline: contracts::TechnicalBaselineContract = read_project_json(root, &baseline_ref)?;
    let pgc: contracts::PlanningGenerationContract = read_project_json(root, &planning_ref)?;
    let aac: ArchitectureArtifactContract = read_project_json(root, &architecture_ref)?;
    let task_plan_ref = to_project_relative(
        root,
        &task_plan_file(root, locator, &task_plan.task_plan_id),
    )?;
    let run_ref = to_project_relative(root, &task_plan_run_file(root, locator, &run.run_id))?;
    let request_task = task_with_execution_guidance(
        task.clone(),
        &aac,
        &pgc.planning_inputs.user_facing_language,
    );
    let engineering_quality_requirements =
        task_scoped_engineering_quality_requirements(task_plan, &request_task);
    let architecture_quality_requirements =
        task_scoped_architecture_quality_requirements(task_plan, &request_task);
    let api_contract_requirements = task_scoped_api_contract_requirements(task_plan, &request_task);
    let code_quality_requirements = code_quality_requirements_for_task(task_plan, &request_task);
    let browser_verification_profile =
        browser_verification_profile_for_task(task_plan, &request_task);
    let browser_verification_context = browser_verification_profile
        .map(|profile| browser_verification_context(root, task_plan, profile));
    let architecture_projection = task_scoped_architecture_projection(&aac, &request_task);
    let schema_shape = task_result_schema_shape(&request_task, browser_verification_profile);
    let dependency_results = dependency_results(run, task);
    let read_groups = task_execution_read_groups(
        &request_task,
        !dependency_results.is_empty(),
        &architecture_projection,
        browser_verification_context.is_some(),
    );
    let user_facing_language = pgc.planning_inputs.user_facing_language.clone();
    let mut execution_rules =
        task_execution_rules(result_file, &request_task, user_facing_language.clone());
    if browser_verification_context.is_some() {
        execution_rules["browserVerificationRules"] = browser_verification_rules();
    }
    let result_rules = task_result_rules(&request_task, browser_verification_profile.is_some());
    let mut source_context = json!({
        "technicalBaseline": {
            "projectKind": baseline.project_kind,
            "stack": baseline.stack
        },
        "architectureArtifactProjection": architecture_projection,
        "acceptanceSnapshot": pgc.phase_scope.acceptance_candidates.iter()
            .filter(|acceptance| request_task.acceptance_refs.iter().any(|id| id == &acceptance.id))
            .collect::<Vec<_>>(),
        "requirementDetailSnapshot": pgc.requirement_details.items.iter()
            .filter(|detail| request_task.requirement_detail_refs.iter().any(|id| id == &detail.detail_id))
            .collect::<Vec<_>>(),
        "userFacingLanguage": user_facing_language
    });
    if !dependency_results.is_empty() {
        source_context["dependencyResults"] = json!(dependency_results);
    }
    if !engineering_quality_requirements.is_empty() {
        source_context["engineeringQualityRequirements"] = json!(engineering_quality_requirements);
    }
    if !architecture_quality_requirements.is_empty() {
        source_context["architectureQualityRequirements"] =
            json!(architecture_quality_requirements);
    }
    if !api_contract_requirements.is_empty() {
        source_context["apiContractRequirements"] = json!(api_contract_requirements);
    }
    if !code_quality_requirements.is_empty() {
        source_context["codeQualityExecutionContext"] =
            code_quality_execution_context(&code_quality_requirements);
    }
    if let Some(browser_verification_context) = browser_verification_context {
        source_context["browserVerificationContext"] = browser_verification_context;
    }
    Ok(json!({
        "schemaVersion": "1.0",
        "requestType": "execute_task",
        "requestId": request_id,
        "artifactKind": delivery_core::ArtifactKind::TaskResult,
        "source": {
            "taskPlanId": task_plan.task_plan_id,
            "taskId": task.task_id,
            "groupId": task.group_id,
            "technicalBaselineId": baseline.technical_baseline_id,
            "architectureArtifactContractId": aac.architecture_artifact_contract_id,
            "taskPlanRunId": run.run_id
        },
        "sourceRefs": {
            "technicalBaselineRef": baseline_ref,
            "planningGenerationContractRef": planning_ref,
            "architectureArtifactContractRef": architecture_ref,
            "taskPlanRef": task_plan_ref,
            "taskPlanRunRef": run_ref,
            "phaseConceptGroundingRef": pgc.context_refs.phase_concept_grounding_ref
        },
        "task": &request_task,
        "sourceContext": source_context,
        "executionRules": execution_rules,
        "enumRefs": {
            "taskResultStatus": ["completed", "completed_with_notes", "blocked", "failed"],
            "verificationStatus": ["passed", "not_run", "failed", "inconclusive"],
            "verificationEvidence": ["automated_test", "browser_automation", "manual_command_output", "runtime_api_check", "static_check", "agent_review_explanation"],
            "selfRepairStopReason": ["not_attempted", "verification_passed", "blocked_condition_detected", "same_failure_repeated_without_progress", "hard_attempt_limit_reached", "repair_requires_contract_change", "repair_requires_scope_expansion"]
        },
        "outputContract": {
            "artifactKind": delivery_core::ArtifactKind::TaskResult,
            "writeMode": "single_json",
            "submitTool": "loom.recordTaskResultFile",
            "resultFile": result_file,
            "writeTargets": [{
                "targetId": "result",
                "path": result_file,
                "required": true,
                "description": "Write the TaskResult JSON for this planned task."
            }],
            "requiredTopLevelFields": task_result_required_top_level_fields(&request_task),
            "blockedReasonOptions": [
                {"code": "DESIGN_INSUFFICIENT", "nextNode": "architecture_artifact_repair"},
                {"code": "TASKPLAN_INVALID", "nextNode": "taskplan_repair"},
                {"code": "DEPENDENCY_NOT_READY", "nextNode": "wait_dependency"}
            ],
            "schemaShape": schema_shape,
            "resultTemplate": task_result_template_with_code_quality(
                &request_task,
                &code_quality_requirements,
                browser_verification_profile,
            ),
            "resultRules": result_rules
        },
        "requestReadPlan": { "groups": read_groups }
    }))
}

pub(crate) fn task_execution_rules(
    result_file: &str,
    task: &TaskDefinition,
    user_facing_language: Option<contracts::UserFacingLanguageConstraint>,
) -> Value {
    let mut rules = json!({
        "sourceEditPreparationContract": source_edit_preparation_contract(result_file),
        "completionBarrier": {
            "resultFile": result_file,
            "submitTool": "loom.recordTaskResultFile",
            "rule": "The task is not complete until TaskResult exists at outputContract.resultFile and loom.recordTaskResultFile succeeds."
        },
        "finalResponseGuard": {
            "mustNotReportProgressBeforeSubmit": true,
            "rule": "Do not stop with a progress-only summary before submitting TaskResult."
        },
        "completionContinuityRequirement": {
            "rule": "Verification method is agent-chosen, but it must return control before TaskResult submission.",
            "forbiddenOutcome": "Do not leave this task waiting on a long-running command, browser session, interactive tool, server, watcher, worker, progress summary, or handoff note.",
            "requiredCloseout": "Write TaskResult and run loom.recordTaskResultFile in this same task turn unless a declared stop condition is reached.",
            "taskResultField": "executionContinuity",
            "statusRule": "If agent-owned long-running work was started and its release state is unknown, use completed_with_notes with notes unless an independent failure or blocked condition remains."
        },
        "verificationCommandSchedulingRules": verification_command_scheduling_rules(),
        "userFacingLanguage": {
            "constraint": user_facing_language,
            "rule": "Preserve the confirmed user-facing language in generated UI, validation, feedback, test labels, and TaskResult evidence when applicable."
        },
        "boundaryRules": [
            "Execute only the current task.",
            "Do not modify Brainstorm, TechnicalBaseline, PGC, AAC, TaskPlan, or other protected Loom artifacts.",
            "Do not implement deferred scope.",
            "Use confirmed business language in user-visible UI, feedback, test names, and TaskResult evidence when applicable.",
            "Write TaskResult JSON only to outputContract.resultFile."
        ]
    });
    let Some(object) = rules.as_object_mut() else {
        return rules;
    };
    if task_has_frontend_execution(task) {
        if !runtime_delivery_evidence_applies(task) {
            object.insert(
                "frontendImplementationOrganizationRules".to_string(),
                frontend_implementation_organization_rules(),
            );
        }
        object.insert(
            "interactiveVerificationProbePolicy".to_string(),
            interactive_verification_probe_policy(),
        );
    }
    if task_needs_controlled_runtime_probe_rules(task) {
        object.insert(
            "controlledRuntimeProbeRules".to_string(),
            controlled_runtime_probe_rules(),
        );
    }
    if runtime_delivery_evidence_applies(task) {
        object.insert(
            "runtimeDeliveryExecutionRules".to_string(),
            runtime_delivery_execution_rules(),
        );
    }
    if !task.engineering_quality_requirement_refs.is_empty() {
        object.insert(
            "engineeringQualityExecutionRules".to_string(),
            engineering_quality_execution_rules(task),
        );
    }
    if !task.architecture_quality_requirement_refs.is_empty() {
        object.insert(
            "architectureQualityExecutionRules".to_string(),
            architecture_quality_execution_rules(task),
        );
    }
    if !task.api_contract_requirement_refs.is_empty() {
        object.insert(
            "apiContractExecutionRules".to_string(),
            api_contract_execution_rules(task),
        );
    }
    if !task.code_quality_requirement_refs.is_empty() {
        object.insert(
            "codeQualityExecutionRules".to_string(),
            code_quality_execution_rules(task),
        );
    }
    rules
}

fn source_edit_preparation_contract(result_file: &str) -> Value {
    json!({
        "schemaVersion": "1.0",
        "contractKind": "source_edit_preparation",
        "resultFile": result_file,
        "requiredWritePlanFields": {
            "targetPath": "Concrete project-relative or absolute path to create, replace, edit, or write as result artifact.",
            "writeKind": ["create", "replace", "edit", "multi_edit", "artifact_result"],
            "contentBasis": [
                "task.objective",
                "task.acceptanceRefs and sourceContext.acceptanceSnapshot",
                "task.requirementDetailRefs and sourceContext.requirementDetailSnapshot",
                "task.frontendExperienceRequirement when present",
                "task.runtimeDeliveryRequirement when present",
                "current source file contents for source edits"
            ],
            "writePayloadReady": "true only when complete file content or a complete edit set has been formed before invoking the write method"
        },
        "sequence": [
            "Read required requestReadPlan groups and current source files that will be changed.",
            "Form an internal write plan with targetPath, writeKind, contentBasis, and writePayloadReady=true.",
            "Invoke file write/edit only after path and payload are complete.",
            "If a write tool rejects missing or invalid path/content/edit arguments, rebuild complete arguments before retrying.",
            "If targetPath or payload cannot be determined within this task boundary, write a failed or blocked TaskResult and submit it."
        ],
        "forbiddenOutcomes": [
            "Do not invoke write/edit tools with missing path, content, or edit arguments.",
            "Do not repeat a malformed write/edit tool call.",
            "Do not begin a write while writePayloadReady is false.",
            "Do not ask the user how to continue when the uncertainty can be represented as a failed or blocked TaskResult.",
            "Do not stop with a progress-only summary after source edits."
        ]
    })
}

pub(crate) fn verification_command_scheduling_rules() -> Value {
    json!([
        "Run verification commands serially by default; only read-only inspection commands may be parallelized.",
        "Do not issue multiple tool calls in the same response for commands that may install dependencies, build artifacts, run tests, start or probe runtimes, clean outputs, generate code, format files, mutate caches, or write files.",
        "For write-producing verification commands, run one command, wait for it to finish, inspect the result, then decide the next command.",
        "Treat commands as write-producing when unsure, including install, build, clean, test, e2e, lint with cache/fix, format with write, codegen, dev/start/preview servers, runtime checks, and commands that may write node_modules, dist, build, coverage, cache, reports, logs, or lockfiles.",
        "When a temporary runtime is running for a bounded probe, run only readiness, HTTP, API, or browser probes against that runtime until cleanup is complete.",
        "Record verification commands in TaskResult in the actual order they completed."
    ])
}

pub(crate) fn controlled_runtime_probe_rules() -> Value {
    json!([
        "Never run long-lived runtime or server commands as foreground blocking verification commands.",
        "This applies to commands that listen on a port, serve requests, watch files, open preview/dev servers, start workers, start queues, or keep the process alive.",
        "If a runtime probe is needed, start only a task-owned temporary runtime in the background with a bounded readiness window, record pid, port, and command when available, run the probe, then stop that task-owned runtime before writing TaskResult.",
        "If the runtime reports ready or listening, do not wait for natural process exit; probe the ready target and close out.",
        "If the environment cannot safely start, probe, and clean up a temporary runtime, skip the live probe and record static/code-level evidence plus unverifiedItems or completed_with_notes.",
        "Runtime probe cleanup failure, unknown cleanup, or not-safe cleanup is non-blocking by itself; record runtimeProbeCleanup and use completed_with_notes unless an independent defect remains."
    ])
}

fn frontend_implementation_organization_rules() -> Value {
    json!([
        "For frontend tasks, organize implementation by responsibility boundaries rather than one giant mixed file.",
        "Use the project's existing frontend structure when present.",
        "When adding a frontend module to an existing app, add it as a reachable entry, route, tab, or navigation item in the existing app shell.",
        "Do not replace, hide, or remove existing reachable module entries unless the requirement explicitly asks for that replacement or removal.",
        "Follow sourceContext.userFacingLanguage or executionRules.userFacingLanguage for user-visible copy; do not translate code identifiers, API paths, database fields, enum values, package names, framework terms, or internal artifact ids.",
        "Make UI/view, API or service interaction, state or feedback handling, and verification evidence distinguishable.",
        "Do not force every responsibility into a separate file for small tasks, and do not collapse multiple frontend responsibilities into an unmaintainable single blob."
    ])
}

fn interactive_verification_probe_policy() -> Value {
    json!({
        "appliesWhen": "The task uses browser, e2e, interactive UI, runtime UI, or API-backed UI verification.",
        "deriveProbePlanFrom": [
            "task.verificationIntents[].behavior",
            "task.frontendExperienceRequirement.executionGuidance.surfacesInScope",
            "task.frontendExperienceRequirement.executionGuidance.workflowsInScope",
            "task.frontendExperienceRequirement.executionGuidance.actionsInScope",
            "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
            "task.runtimeDeliveryRequirement.requiredCodeLevelChecks"
        ],
        "requiredExecutionPattern": [
            "Before running browser, e2e, or interactive code, derive the smallest applicable probe plan from the current task fields.",
            "Select only probes that match the current task responsibility; do not run probes for absent surfaces, workflows, actions, bindings, or runtime checks.",
            "Each probe must verify one interaction target: one verification intent, workflow step, user action, frontend/backend binding, or runtime check.",
            "Each probe must be bounded, return before the next probe starts, and produce an observable result such as reachable page, visible state, response status, result message, list/detail change, or state transition.",
            "Do not bundle multiple business workflows into one browser/e2e script."
        ],
        "failureProgressRule": [
            "When a probe fails, the next attempt must be smaller, more specific, reset tool context, or change the failure condition.",
            "Continue while new observable evidence appears or the failure signature changes.",
            "Stop retrying that verification method when the same failure signature repeats without new observable evidence."
        ],
        "taskResultEvidence": [
            "Record successful probe facts in verificationResults[].summary for the matching verificationId.",
            "Record runtime command or probe evidence in runtimeDeliveryEvidence.commandsRun when runtimeDeliveryEvidence applies.",
            "Record remaining unverified responsibility in notes or runtimeDeliveryEvidence.unverifiedItems according to TaskResult rules."
        ]
    })
}

fn runtime_delivery_execution_rules() -> Value {
    json!({
        "readRuntimeDeliveryRequirement": true,
        "verificationBoundary": "code_level_only",
        "mustKeepContractAndCodeAligned": true,
        "mayEditApplicationCode": true,
        "mayEditPackageScripts": true,
        "mayEditDeployGeneratedFiles": false,
        "mayEditRuntimeDeliveryContract": false,
        "mustRecordRuntimeDeliveryEvidence": true,
        "mustRecordRuntimeProbeCleanupWhenTemporaryRuntimeStarted": true,
        "foregroundRuntimeCommandsForbidden": true,
        "controlledRuntimeProbeRulesField": "executionRules.controlledRuntimeProbeRules",
        "runtimeProbeCleanupFailureSeverity": "completed_with_notes_only",
        "selfRepairWhenCodeLevelCheckFails": true
    })
}

fn task_result_rules(task: &TaskDefinition, has_browser_verification: bool) -> Value {
    let mut rules = vec![
        "TaskResult must include every requiredTopLevelFields entry.".to_string(),
        "If status is completed, every verification intent should have passed evidence.".to_string(),
        "If status is failed, failure is required.".to_string(),
        "TaskResult must include executionContinuity; if agent-owned long-running work release state is unknown, status cannot be completed.".to_string(),
        "changedFiles must list intended deliverable files, not incidental dependency directories, caches, logs, or generated build output.".to_string(),
        "noChangeReason must be null when changedFiles is non-empty; when changedFiles is empty and a reason is needed, noChangeReason must be an object with code and summary, never a string or array.".to_string(),
        "For completed or completed_with_notes results, provide substantive status, evidenceRefs, and summary for every requirementDetailEvidence entry; Loom derives detailId and verificationIds from the task contract.".to_string(),
    ];
    if frontend_self_check_applies(task) {
        rules.push("For frontend tasks, fill frontendExperienceSelfCheck using task.frontendExperienceRequirement.executionGuidance and frontend/backend bindings when present; Loom derives closureRequirementIds from the assigned closure contract.".to_string());
        rules.push("For browser/e2e/interactive verification, follow executionRules.interactiveVerificationProbePolicy and record evidence through existing TaskResult fields.".to_string());
    }
    if frontend_quality_self_check_applies(task) {
        rules.push("For frontendQualitySelfCheck, provide substantive status, files, evidence, contentBoundaryEvidence, referencePlanFilesChecked, and token evidence for the task-scoped uiProductionBrief.surfaceDecisionContract. Submit surface evidence arrays in the contract order; Loom derives surfaceDecisionContractRef and every evidence id. Do not leave replace_with_* values in submitted results.".to_string());
    }
    if has_browser_verification {
        rules.push("Record every sourceContext.browserVerificationContext.profile.checks outcome under verificationResults[].browserChecks in the profile order. Loom derives verificationId and checkId; do not write or copy those linkage fields, and do not paste trace, screenshot, or report contents into TaskResult prose.".to_string());
        rules.push("Passed browser checks require the exact command, attempt count, and concise observed outcome. Blocked checks require a concrete blockedReason. Keep retry success visible with attempts greater than one.".to_string());
    }
    if runtime_delivery_evidence_applies(task) {
        rules.push("For runtimeDeliveryRequirement tasks, include runtimeDeliveryEvidence with checkedFields, codeLevelChecks, commandsRun when commands were run, and unverifiedItems when environment prevents a check.".to_string());
        rules.push("For runtimeDeliveryEvidence.codeLevelChecks, report status and evidence in the task.runtimeDeliveryRequirement.requiredCodeLevelChecks order. Loom derives requirementRef, checkedFields, checkId, and contractField.".to_string());
        rules.push("If a temporary runtime/probe/server/container was started, include runtimeDeliveryEvidence.runtimeProbeCleanup; cleanup failure alone should be completed_with_notes, not failed or blocked.".to_string());
    }
    if !task.engineering_quality_requirement_refs.is_empty() {
        rules.push("For referenced engineeringQualityRequirements, verificationResults summaries must state how implementation kept the declared alignmentTargets aligned for this task.".to_string());
        rules.push("For persistence_mapping requirements, evidence must cover changed risk field kinds across domain model, storage schema or migration, data access mapping, DTO/API contract, and same-provider persistence behavior when those parts are in task scope.".to_string());
    }
    if !task.architecture_quality_requirement_refs.is_empty() {
        rules.push("For referenced architectureQualityRequirements, provide one architectureQualityEvidence entry per assigned requirement in task order; Loom derives requirementId and verificationIds. Summaries must state how changed files respected the referenced decision, NFR, or risk mitigation.".to_string());
    }
    if !task.api_contract_requirement_refs.is_empty() {
        rules.push("For referenced apiContractRequirements, provide one apiContractEvidence entry per assigned requirement in task order; Loom derives requirementId, interfaceRefs, and verificationIds. Summaries must state how changed files implemented or preserved the referenced API interfaces.".to_string());
    }
    if !task.code_quality_requirement_refs.is_empty() {
        rules.push("For referenced codeQualityExecutionContext entries, provide one codeQualityEvidence entry per assigned requirement in task order; Loom derives requirementId and verificationIds. referenceFilesChecked must list exactly the files read from sourceContext.codeQualityExecutionContext[].referenceLoadPlan, and summaries must state how changed files followed selected language/framework references and existing repository style.".to_string());
        rules.push("referenceLoadPlan paths are Loom installed reference paths, not project source paths; resolve them under the current Loom skill reference root before editing or writing codeQualityEvidence.".to_string());
    }
    json!(rules)
}

fn engineering_quality_execution_rules(task: &TaskDefinition) -> Value {
    json!({
        "appliesToRequirementRefs": task.engineering_quality_requirement_refs,
        "requirementSource": "sourceContext.engineeringQualityRequirements",
        "scopeRule": "Apply only the listed requirements whose appliesToTaskIds include this task; do not create new requirements inside TaskResult.",
        "implementationRules": [
            "Before editing persistence-affecting code, compare sourceContext.engineeringQualityRequirements[].alignmentTargets against this task's changed entities, schema or migrations, repositories, DTOs, API payloads, query fields, and tests.",
            "Keep field type semantics aligned across code, storage, data access, serialization, and tests; do not rely on provider defaults for declared riskFieldKinds.",
            "Use the actual stackSignals from the request as evidence to choose provider-compatible mappings; do not hardcode assumptions from an unrelated stack."
        ],
        "verificationRules": [
            "Use task.verificationIntents as the verification id source.",
            "When implementation touches persistence, prefer same-provider tests or runtime checks over mock-only evidence.",
            "Record concise alignment evidence in verificationResults[].summary and requirementDetailEvidence[].summary."
        ]
    })
}

fn architecture_quality_execution_rules(task: &TaskDefinition) -> Value {
    json!({
        "appliesToRequirementRefs": task.architecture_quality_requirement_refs,
        "requirementSource": "sourceContext.architectureQualityRequirements",
        "architectureSource": "sourceContext.architectureArtifactProjection.architectureQuality",
        "scopeRule": "Apply only the listed requirements whose appliesToTaskIds include this task; do not create new architecture requirements inside TaskResult.",
        "implementationRules": [
            "Before editing, compare sourceContext.architectureQualityRequirements against the task-owned modules, interfaces, data model, runtime surfaces, and workflows.",
            "Respect referenced decisions, implement referenced risk mitigations when in scope, and keep referenced NFRs observable through code or verification evidence.",
            "Do not expand architecture scope beyond the current task to satisfy an unrelated decision, NFR, or risk."
        ],
        "verificationRules": [
            "Use task.verificationIntents as the verification id source.",
            "Record concise evidence in architectureQualityEvidence for every referenced architecture quality requirement.",
            "Verification summaries must state how the changed files respected the referenced decision, NFR, or risk mitigation."
        ]
    })
}

fn api_contract_execution_rules(task: &TaskDefinition) -> Value {
    json!({
        "appliesToRequirementRefs": task.api_contract_requirement_refs,
        "requirementSource": "sourceContext.apiContractRequirements",
        "interfaceSource": "sourceContext.architectureArtifactProjection.interfaces",
        "scopeRule": "Apply only the listed API contract requirements whose appliesToTaskIds include this task; do not create new API requirements inside TaskResult.",
        "implementationRules": [
            "Before editing API or client binding code, compare sourceContext.apiContractRequirements with the task-owned AAC interfaces.",
            "Keep method, path, request schema, response schema, status code categories, error schema, auth policy, and pagination policy aligned with the AAC interface.",
            "Do not replace business errors with generic 500 responses or silent success.",
            "Do not add versioned paths or OpenAPI files unless the AAC interface or requirement explicitly declares them."
        ],
        "verificationRules": [
            "Use task.verificationIntents as the verification id source.",
            "Record concise evidence in apiContractEvidence for every referenced API contract requirement.",
            "For write or state-transition APIs, verification should cover one success path and one validation or business-blocking error path when feasible.",
            "For collection APIs, verification should cover declared pagination or filtering behavior when present."
        ]
    })
}

fn code_quality_execution_rules(task: &TaskDefinition) -> Value {
    json!({
        "appliesToRequirementRefs": task.code_quality_requirement_refs,
        "requirementSource": "sourceContext.codeQualityExecutionContext",
        "scopeRule": "Apply only listed code quality requirements whose appliesToTaskIds include this task; do not create new language or framework requirements inside TaskResult.",
        "referenceLoadRule": "Load only files listed in sourceContext.codeQualityExecutionContext[].referenceLoadPlan. These paths are relative to the installed Loom skill references root, not the project workspace. Do not derive paths from referenceGroups, scan the tech/code or tech/backend trees, or load external language/framework skills.",
        "referencePathResolution": {
            "pathMeaning": "Loom installed reference path",
            "projectWorkspacePath": false,
            "codexAndClaudeHint": "Resolve as references/<path> next to the active Loom SKILL.md.",
            "opencodeHint": "Resolve as ../references/loom/<path> from the active OpenCode loom command/plugin files."
        },
        "implementationRules": [
            "Before editing, compare selected language/framework references with existing repository patterns and prefer the existing project convention when both are valid.",
            "Keep API, UI, architecture, runtime, and persistence obligations in their dedicated contracts; use code quality requirements for language/framework implementation discipline only.",
            "When a code quality requirement contains packageNamingPolicy, production source package declarations must follow that policy; placeholder package roots are not acceptable deliverable code.",
            "When a selected language or framework reference is not applicable to a changed file but the requirement is still satisfied, explain the non-applicability in the summary without adding knownGaps.",
            "If a selected Loom reference cannot be loaded from the installed reference root, do not mark codeQualityEvidence satisfied; report the unresolved reference as a blocking contract problem instead of treating the project workspace as missing source files."
        ],
        "verificationRules": [
            "Use task.verificationIntents as the verification id source.",
            "Run the smallest available compile, type, lint, unit, or integration check that proves the changed code.",
            "Record selected reference groups, reference files checked, changed files, commands, and remaining gaps in codeQualityEvidence."
        ]
    })
}

pub(crate) fn browser_verification_profile_for_task<'a>(
    task_plan: &'a TaskPlan,
    task: &TaskDefinition,
) -> Option<&'a BrowserVerificationProfile> {
    task_plan
        .browser_verification_profiles
        .iter()
        .find(|profile| profile.task_id == task.task_id)
}

pub(crate) fn browser_verification_context(
    project_root: &Path,
    task_plan: &TaskPlan,
    profile: &BrowserVerificationProfile,
) -> Value {
    let installation = profile
        .installation_id
        .as_ref()
        .and_then(|installation_id| {
            task_plan
                .browser_automation_facts
                .installations
                .iter()
                .find(|installation| &installation.installation_id == installation_id)
        });
    let runtime = state::store::read_json_value(
        &project_root.join(".loom/runtime/browser-automation/latest.json"),
    )
    .ok()
    .filter(|value| {
        matches!(
            value.get("status").and_then(Value::as_str),
            Some("ready" | "partial")
        )
    })
    .map(|value| {
        json!({
            "status": value.get("status").cloned().unwrap_or(Value::Null),
            "projectTargets": value.get("projectTargets").cloned().unwrap_or_else(|| json!([])),
            "runtimeEnvironments": value.get("runtimeEnvironments").cloned().unwrap_or_else(|| json!([]))
        })
    });
    json!({
        "profile": profile,
        "projectRunner": installation,
        "baselineSelection": task_plan.browser_automation_facts.baseline_selection,
        "runtime": runtime
    })
}

pub(crate) fn browser_verification_rules() -> Value {
    json!({
        "profileAuthority": "sourceContext.browserVerificationContext.profile",
        "referenceLoadRule": "Read only files listed in sourceContext.browserVerificationContext.profile.referenceLoadPlan. Paths are relative to the installed Loom skill references root; do not browse sibling test references or load an external Playwright skill.",
        "scopeRule": "Run only profile.checks in this MCP-generated browser quality closure and keep each check bounded to its source task, source verification, viewport, backend mode, and enforcement.",
        "runnerRule": "Reuse sourceContext.browserVerificationContext.projectRunner when present. Do not replace an existing project runner or install a second project-local Playwright stack.",
        "runnerBootstrapRule": "When projectRunner is absent and profile.runnerSource is baseline_selected or loom_managed, create the first project-owned Playwright dependency/config only for this closure, pin @playwright/test to the exact resolvedVersion supplied by runtime.runtimeEnvironments, and update the project lockfile. The shared runner remains a preparation/doctor asset and is never copied into the project.",
        "runtimeAuthority": "MCP prepared sourceContext.browserVerificationContext.runtime before creating this execution request. Do not call loom.browserRuntimePrepare from inside the task, install browsers ad hoc, or edit shared cache state.",
        "runtimeExecutionRule": "Select the runtime environment whose requested/resolved version matches the project runner. For host backend, apply browserEnvironment to the project-local runner. For managed_container backend, use its commandPrefix and browserEnvironment without copying shared assets into the project; when the tested service runs on the host, translate loopback base URLs to managedContainer.hostGateway while preserving the actual port.",
        "environmentFailureRule": "Use blocked only when the supplied host/container browser environment cannot launch or execute, and include the exact environment diagnostic; Loom classifies that outside generic execution repair. Application startup, API, selector, assertion, and workflow failures are product evidence and must remain failed.",
        "resultRule": "Record browser outcomes through verificationResults[].browserChecks; do not paste Playwright reports, traces, screenshots, or console logs into prose fields. MCP correlates closure checks to source UI tasks."
    })
}

fn task_execution_read_groups(
    task: &TaskDefinition,
    has_dependency_results: bool,
    architecture_projection: &Value,
    has_browser_verification: bool,
) -> Value {
    let has_frontend_execution = task_has_frontend_execution(task);
    let has_frontend_requirement = task.frontend_experience_requirement.is_some();
    let needs_runtime_probe_rules = task_needs_controlled_runtime_probe_rules(task);
    let core_fields = vec![
        "source.taskPlanId",
        "source.taskId",
        "source.groupId",
        "source.technicalBaselineId",
        "source.architectureArtifactContractId",
        "source.taskPlanRunId",
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
        "sourceContext.technicalBaseline.projectKind",
        "sourceContext.userFacingLanguage",
        "executionRules.sourceEditPreparationContract",
        "executionRules.boundaryRules",
    ];
    let mut scope_fields = vec![
        "sourceContext.acceptanceSnapshot",
        "sourceContext.requirementDetailSnapshot",
        "executionRules.verificationCommandSchedulingRules",
        "executionRules.userFacingLanguage",
    ];
    if has_dependency_results {
        scope_fields.push("sourceContext.dependencyResults");
    }

    let mut architecture_fields = Vec::new();
    if projection_array_has_items(architecture_projection, "modules") {
        architecture_fields.push("sourceContext.architectureArtifactProjection.modules");
    }
    if projection_array_has_items(architecture_projection, "entities") {
        architecture_fields.push("sourceContext.architectureArtifactProjection.entities");
    }
    if projection_array_has_items(architecture_projection, "interfaces") {
        architecture_fields.push("sourceContext.architectureArtifactProjection.interfaces");
    }
    if projection_array_has_items(architecture_projection, "userFlows") {
        architecture_fields.push("sourceContext.architectureArtifactProjection.userFlows");
    }
    if projection_array_has_items(architecture_projection, "stateMachines") {
        architecture_fields.push("sourceContext.architectureArtifactProjection.stateMachines");
    }
    if projection_architecture_quality_has_items(architecture_projection) {
        architecture_fields
            .push("sourceContext.architectureArtifactProjection.architectureQuality");
    }

    let mut frontend_fields = Vec::new();
    if has_frontend_requirement {
        frontend_fields.extend([
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
            "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief",
            "task.frontendExperienceRequirement.executionGuidance.styleAssetPlan",
            "task.frontendExperienceRequirement.uiSurfaceDecisionContractRef",
            "task.frontendExperienceRequirement.uiSurfaceOwnership",
        ]);
    }
    if has_frontend_execution {
        if !runtime_delivery_evidence_applies(task) {
            frontend_fields.push("executionRules.frontendImplementationOrganizationRules");
        }
        frontend_fields.push("executionRules.interactiveVerificationProbePolicy");
    }

    let mut runtime_fields = Vec::new();
    if needs_runtime_probe_rules {
        runtime_fields.push("executionRules.controlledRuntimeProbeRules");
    }
    if runtime_delivery_evidence_applies(task) {
        runtime_fields.extend(runtime_delivery_requirement_read_fields(task));
        runtime_fields.push("sourceContext.architectureArtifactProjection.runtimeDelivery");
        runtime_fields.push("executionRules.runtimeDeliveryExecutionRules");
    }

    let browser_fields = if has_browser_verification {
        vec![
            "sourceContext.browserVerificationContext",
            "executionRules.browserVerificationRules",
        ]
    } else {
        vec![]
    };

    let mut quality_fields = Vec::new();
    if !task.engineering_quality_requirement_refs.is_empty() {
        quality_fields.extend([
            "task.engineeringQualityRequirementRefs",
            "sourceContext.engineeringQualityRequirements",
            "executionRules.engineeringQualityExecutionRules",
        ]);
    }
    if !task.architecture_quality_requirement_refs.is_empty() {
        quality_fields.extend([
            "task.architectureQualityRequirementRefs",
            "sourceContext.architectureQualityRequirements",
            "executionRules.architectureQualityExecutionRules",
        ]);
    }
    if !task.api_contract_requirement_refs.is_empty() {
        quality_fields.extend([
            "task.apiContractRequirementRefs",
            "sourceContext.apiContractRequirements",
            "executionRules.apiContractExecutionRules",
        ]);
    }
    if !task.code_quality_requirement_refs.is_empty() {
        quality_fields.extend([
            "task.codeQualityRequirementRefs",
            "sourceContext.codeQualityExecutionContext",
            "executionRules.codeQualityExecutionRules",
        ]);
    }
    let mut concept_fields = Vec::new();
    if !task.concept_refs.is_empty() {
        concept_fields.push("task.conceptRefs");
    }
    if !task.concept_responsibilities.is_empty() {
        concept_fields.push("task.conceptResponsibilities");
    }
    if !task.concept_verification_intents.is_empty() {
        concept_fields.push("task.conceptVerificationIntents");
    }

    let mut result_fields = vec![
        "enumRefs.taskResultStatus",
        "enumRefs.verificationStatus",
        "enumRefs.verificationEvidence",
        "enumRefs.selfRepairStopReason",
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
        "outputContract.schemaShape.properties.blockedReasons",
        "outputContract.resultRules",
        "outputContract.blockedReasonOptions",
        "executionRules.completionBarrier",
        "executionRules.finalResponseGuard",
        "executionRules.completionContinuityRequirement",
        "executionRules.verificationCommandSchedulingRules",
    ];
    if frontend_self_check_applies(task) {
        result_fields.push("outputContract.schemaShape.properties.frontendExperienceSelfCheck");
    }
    if frontend_quality_self_check_applies(task) {
        result_fields.push("outputContract.schemaShape.properties.frontendQualitySelfCheck");
    }
    if runtime_delivery_evidence_applies(task) {
        result_fields.push("outputContract.schemaShape.properties.runtimeDeliveryEvidence");
    }
    if !task.concept_refs.is_empty() {
        result_fields.push("outputContract.schemaShape.properties.conceptEvidence");
    }
    if !task.architecture_quality_requirement_refs.is_empty() {
        result_fields.push("outputContract.schemaShape.properties.architectureQualityEvidence");
    }
    if !task.api_contract_requirement_refs.is_empty() {
        result_fields.push("outputContract.schemaShape.properties.apiContractEvidence");
    }
    if !task.code_quality_requirement_refs.is_empty() {
        result_fields.push("outputContract.schemaShape.properties.codeQualityEvidence");
    }

    let mut groups = vec![
        json!({
            "groupId": "task_execution_core",
            "required": true,
            "purpose": "Read task identity, edit boundary, and source edit rules before editing.",
            "whenToRead": "Read before any source edit.",
            "selectors": read_selectors_value_from_paths(core_fields)
        }),
        json!({
            "groupId": "task_execution_scope_context",
            "required": true,
            "purpose": "Read task-scoped acceptance, requirement detail, dependency, and language context.",
            "whenToRead": "Read before deciding implementation scope.",
            "selectors": read_selectors_value_from_paths(scope_fields)
        }),
    ];
    if !architecture_fields.is_empty() {
        groups.push(json!({
            "groupId": "task_execution_architecture_context",
            "required": true,
            "purpose": "Read only task-owned architecture artifacts needed for this task.",
            "whenToRead": "Read before editing architecture-owned code.",
            "selectors": read_selectors_value_from_paths(architecture_fields)
        }));
    }
    if !frontend_fields.is_empty() {
        groups.push(json!({
            "groupId": "task_execution_frontend_context",
            "required": true,
            "purpose": "Read task-owned frontend guidance and UI quality contract.",
            "whenToRead": "Read before editing frontend surfaces.",
            "selectors": read_selectors_value_from_paths(frontend_fields)
        }));
    }
    if !runtime_fields.is_empty() {
        groups.push(json!({
            "groupId": "task_execution_runtime_context",
            "required": true,
            "purpose": "Read runtime delivery and controlled probe rules only when this task needs runtime evidence.",
            "whenToRead": "Read before runtime-impacting edits or probes.",
            "selectors": read_selectors_value_from_paths(runtime_fields)
        }));
    }
    if !browser_fields.is_empty() {
        groups.push(json!({
            "groupId": "task_execution_browser_verification",
            "required": true,
            "purpose": "Read the MCP-derived browser checks, selected runner facts, and task-scoped Playwright reference plan.",
            "whenToRead": "Read before creating, changing, or running browser verification.",
            "selectors": read_selectors_value_from_paths(browser_fields)
        }));
    }
    if !quality_fields.is_empty() {
        groups.push(json!({
            "groupId": "task_execution_quality_context",
            "required": true,
            "purpose": "Read only quality contracts assigned to this task.",
            "whenToRead": "Read before applying engineering, architecture, API, or code quality obligations.",
            "selectors": read_selectors_value_from_paths(quality_fields)
        }));
    }
    if !concept_fields.is_empty() {
        groups.push(json!({
            "groupId": "task_execution_concept_context",
            "required": true,
            "purpose": "Read concept responsibilities assigned to this task.",
            "whenToRead": "Read before implementing concept-sensitive behavior.",
            "selectors": read_selectors_value_from_paths(concept_fields)
        }));
    }
    groups.push(
        json!({
            "groupId": "task_execution_result_contract",
            "required": true,
            "purpose": "Read TaskResult output file, schema fields, enum values, and completion barrier.",
            "whenToRead": "Read before writing TaskResult.",
            "selectors": read_selectors_value_from_paths(result_fields)
        }),
    );
    Value::Array(groups)
}

fn projection_array_has_items(projection: &Value, key: &str) -> bool {
    projection
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn projection_architecture_quality_has_items(projection: &Value) -> bool {
    let Some(quality) = projection.get("architectureQuality") else {
        return false;
    };
    ["decisions", "nfrs", "risks"].iter().any(|key| {
        quality
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    })
}

fn task_scoped_engineering_quality_requirements(
    task_plan: &TaskPlan,
    task: &TaskDefinition,
) -> Vec<EngineeringQualityRequirement> {
    if task.engineering_quality_requirement_refs.is_empty() {
        return vec![];
    }
    let refs = task
        .engineering_quality_requirement_refs
        .iter()
        .collect::<BTreeSet<_>>();
    task_plan
        .engineering_quality_requirements
        .iter()
        .filter(|requirement| refs.contains(&requirement.requirement_id))
        .cloned()
        .collect()
}

fn task_scoped_architecture_quality_requirements(
    task_plan: &TaskPlan,
    task: &TaskDefinition,
) -> Vec<ArchitectureQualityRequirement> {
    if task.architecture_quality_requirement_refs.is_empty() {
        return vec![];
    }
    let refs = task
        .architecture_quality_requirement_refs
        .iter()
        .collect::<BTreeSet<_>>();
    task_plan
        .architecture_quality_requirements
        .iter()
        .filter(|requirement| refs.contains(&requirement.requirement_id))
        .cloned()
        .collect()
}

fn task_scoped_api_contract_requirements(
    task_plan: &TaskPlan,
    task: &TaskDefinition,
) -> Vec<ApiContractRequirement> {
    if task.api_contract_requirement_refs.is_empty() {
        return vec![];
    }
    let refs = task
        .api_contract_requirement_refs
        .iter()
        .collect::<BTreeSet<_>>();
    task_plan
        .api_contract_requirements
        .iter()
        .filter(|requirement| refs.contains(&requirement.requirement_id))
        .cloned()
        .collect()
}

fn task_has_frontend_execution(task: &TaskDefinition) -> bool {
    task.frontend_experience_requirement.is_some()
        || matches!(
            task.task_kind,
            TaskKind::UiFlowIncrement | TaskKind::FrontendExperience
        )
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateUiFlow
                    | ImplementationAction::ImplementFrontendExperienceContract
            )
        })
}

fn task_needs_controlled_runtime_probe_rules(task: &TaskDefinition) -> bool {
    runtime_delivery_evidence_applies(task)
        || task_has_frontend_execution(task)
        || task.verification_intents.iter().any(|intent| {
            intent
                .preferred_evidence
                .iter()
                .chain(intent.acceptable_evidence.iter())
                .any(|evidence| *evidence == VerificationEvidence::RuntimeApiCheck)
        })
}

pub(crate) fn runtime_delivery_requirement_read_fields(task: &TaskDefinition) -> Vec<&'static str> {
    let Some(requirement) = task.runtime_delivery_requirement.as_ref() else {
        return vec![];
    };
    let mut fields = vec![
        "task.runtimeDeliveryRequirement.appliesToThisTask",
        "task.runtimeDeliveryRequirement.reason",
    ];
    if requirement.runtime_delivery_ref.is_some() {
        fields.push("task.runtimeDeliveryRequirement.runtimeDeliveryRef");
    }
    if !requirement.affected_contract_fields.is_empty() {
        fields.push("task.runtimeDeliveryRequirement.affectedContractFields");
    }
    if !requirement.required_code_level_checks.is_empty() {
        fields.push("task.runtimeDeliveryRequirement.requiredCodeLevelChecks");
    }
    if !requirement.evidence_expected_in_task_result.is_empty() {
        fields.push("task.runtimeDeliveryRequirement.evidenceExpectedInTaskResult");
    }
    if !requirement.forbidden_actions.is_empty() {
        fields.push("task.runtimeDeliveryRequirement.forbiddenActions");
    }
    if requirement.source.is_some() {
        fields.push("task.runtimeDeliveryRequirement.source");
    }
    if requirement.deployment_failure_ref.is_some() {
        fields.push("task.runtimeDeliveryRequirement.deploymentFailureRef");
    }
    fields
}

pub(crate) fn task_with_phase_execution_guidance(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    task: TaskDefinition,
) -> Result<TaskDefinition, state::store::StateError> {
    if task.frontend_experience_requirement.is_none() {
        return Ok(task);
    }
    let planning_ref = format!(
        ".loom/deliveries/{}/contracts/planning/{}/pgc.json",
        locator.delivery_id, locator.phase_id
    );
    let architecture_ref = format!(
        ".loom/deliveries/{}/contracts/architecture/{}/aac.json",
        locator.delivery_id, locator.phase_id
    );
    let pgc: contracts::PlanningGenerationContract =
        read_project_json(project_root, &planning_ref)?;
    let aac: ArchitectureArtifactContract = read_project_json(project_root, &architecture_ref)?;
    Ok(task_with_execution_guidance(
        task,
        &aac,
        &pgc.planning_inputs.user_facing_language,
    ))
}

pub(crate) fn task_with_execution_guidance(
    mut task: TaskDefinition,
    aac: &ArchitectureArtifactContract,
    user_facing_language: &Option<contracts::UserFacingLanguageConstraint>,
) -> TaskDefinition {
    if task.frontend_experience_requirement.is_none() {
        return task;
    }
    let guidance = build_frontend_execution_guidance(&task, aac, user_facing_language);
    let Some(requirement) = task.frontend_experience_requirement.as_mut() else {
        return task;
    };
    let Some(requirement_object) = requirement.as_object_mut() else {
        *requirement = json!({ "executionGuidance": guidance });
        return task;
    };
    requirement_object.insert("executionGuidance".to_string(), guidance);
    task
}

fn task_scoped_architecture_projection(
    aac: &ArchitectureArtifactContract,
    task: &TaskDefinition,
) -> Value {
    let refs = &task.write_boundary.artifact_refs;
    let selected_user_flows =
        selected_values(&aac.user_flows, "flowId", &refs.user_flows, task, true);
    let interface_refs_from_flows = selected_user_flows
        .iter()
        .flat_map(|flow| string_array_at(flow, "interfaceRefs"))
        .collect::<Vec<_>>();
    let state_machine_refs_from_flows = selected_user_flows
        .iter()
        .flat_map(|flow| {
            array_at(flow, "steps")
                .into_iter()
                .flat_map(|step| string_array_at(step, "stateMachineRefs"))
        })
        .collect::<Vec<_>>();
    let interface_refs = unique_strings(
        refs.interfaces
            .iter()
            .cloned()
            .chain(interface_refs_from_flows)
            .collect(),
    );
    let state_machine_refs = unique_strings(
        refs.state_machines
            .iter()
            .cloned()
            .chain(state_machine_refs_from_flows)
            .collect(),
    );
    let mut projection = json!({
        "compaction": {
            "mode": "task_scoped_artifact_projection",
            "rule": "This projection includes only artifacts selected by task.writeBoundary.artifactRefs, directly linked workflow refs, or task scope/acceptance refs."
        },
        "modules": selected_values(&aac.modules, "moduleId", &refs.modules, task, true),
        "entities": selected_entities(&aac.data_model, &refs.entities, task),
        "interfaces": selected_values(&aac.interfaces, "interfaceId", &interface_refs, task, true),
        "userFlows": selected_user_flows,
        "stateMachines": selected_values(&aac.state_machines, "machineId", &state_machine_refs, task, true),
        "architectureQuality": {
            "decisions": aac.architecture_quality.decisions.iter()
                .filter(|decision| refs.decisions.iter().any(|item| item == &decision.decision_id))
                .cloned()
                .collect::<Vec<_>>(),
            "nfrs": aac.architecture_quality.nfrs.iter()
                .filter(|nfr| refs.nfrs.iter().any(|item| item == &nfr.nfr_id))
                .cloned()
                .collect::<Vec<_>>(),
            "risks": aac.architecture_quality.risks.iter()
                .filter(|risk| refs.risks.iter().any(|item| item == &risk.risk_id))
                .cloned()
                .collect::<Vec<_>>()
        }
    });
    if runtime_delivery_evidence_applies(task) {
        projection["runtimeDelivery"] = aac.runtime_delivery.clone().unwrap_or(Value::Null);
    }
    projection
}

fn build_frontend_execution_guidance(
    task: &TaskDefinition,
    aac: &ArchitectureArtifactContract,
    user_facing_language: &Option<contracts::UserFacingLanguageConstraint>,
) -> Value {
    let Some(frontend) = aac.frontend_experience.as_ref() else {
        return json!({
            "schemaVersion": "1.0",
            "purpose": "No AAC frontendExperience is present for this task.",
            "userFacingLanguage": user_facing_language,
            "responsibility": task.objective,
            "surfacesInScope": [],
            "dataViewsInScope": [],
            "actionsInScope": [],
            "operationPathsInScope": [],
            "frontendBackendBindings": [],
            "dataBindingExpectation": {
                "allowedModes": ["wired", "mocked_with_reason", "static_only_with_reason", "not_applicable"]
            },
            "closureRequirementRefs": [],
            "workflowClosureDetailSource": {
                "closureRequirementIds": [],
                "derivationRule": "No AAC frontendExperience is present for this task."
            },
            "uiProductionBrief": Value::Null,
            "styleAssetPlan": Value::Null,
            "guidanceWarnings": ["AAC frontendExperience is absent."]
        });
    };
    let closure_requirements = workflow_closure_requirements_for_task(task, aac);
    let task_scope = frontend_task_scope(task, aac, frontend, &closure_requirements);
    let surfaces = selected_frontend_surfaces(frontend, &task_scope.surface_refs);
    let operation_paths = selected_frontend_values(
        frontend,
        "operationPaths",
        "pathId",
        &task_scope.operation_path_refs,
    );
    let data_views =
        selected_frontend_values(frontend, "dataViews", "viewId", &task_scope.data_view_refs);
    let actions =
        selected_frontend_values(frontend, "actions", "actionId", &task_scope.action_refs);
    let mut frontend_backend_bindings = frontend_backend_bindings(&closure_requirements);
    if frontend_backend_bindings.is_empty() {
        frontend_backend_bindings = frontend_backend_bindings_from_scope(aac, &task_scope);
    }
    let closure_ids = closure_requirements
        .iter()
        .filter_map(|item| string_at(item, "closureId"))
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    if closure_requirements.is_empty() {
        warnings.push("No workflow closure requirement matched this task; UI scope was derived from AAC uiSurfaceRegistry, detail coverage, task refs, and frontend operation paths.".to_string());
    }
    if surfaces.is_empty() && task_has_frontend_execution(task) {
        warnings.push("No task-specific UI surface matched this task; use uiProductionBrief to keep any implementation business-surface scoped and avoid unrelated UI expansion.".to_string());
    }
    json!({
        "schemaVersion": "1.0",
        "purpose": "Task-scoped frontend execution guidance derived from AAC and TaskPlan refs.",
        "userFacingLanguage": user_facing_language,
        "responsibility": task.objective,
        "surfacesInScope": surfaces,
        "dataViewsInScope": data_views,
        "actionsInScope": actions,
        "operationPathsInScope": operation_paths,
        "frontendBackendBindings": frontend_backend_bindings,
        "dataBindingExpectation": {
            "allowedModes": ["wired", "mocked_with_reason", "static_only_with_reason", "not_applicable"],
            "requiredModeForSatisfaction": if closure_requirements.is_empty() { Value::Null } else { json!("wired") },
            "closureRequirementIds": closure_ids,
            "staticModePolicy": if closure_requirements.is_empty() { Value::Null } else { json!("not_satisfied") },
            "knownGapPolicy": if closure_requirements.is_empty() { Value::Null } else { json!("not_satisfied_when_required_closure") }
        },
        "closureRequirementRefs": workflow_closure_requirement_execution_view(&closure_requirements),
        "workflowClosureDetailSource": {
            "closureRequirementIds": closure_requirements.iter().filter_map(|item| string_at(item, "closureId")).collect::<Vec<_>>(),
            "detailAuthority": "Use closureRequirementRefs, frontendBackendBindings, and sourceContext.architectureArtifactProjection from this request.",
            "derivationRule": "Closure refs are derived from AAC frontendExperience surfaces or operationPaths, task userFlows, userFlow steps, and executable interfaces."
        },
        "uiProductionBrief": ui_production_brief(task, frontend, &task_scope, user_facing_language),
        "styleAssetPlan": style_asset_plan(frontend),
        "guidanceWarnings": warnings
    })
}

#[derive(Default)]
struct FrontendTaskScope {
    ownership_dimensions: Vec<String>,
    surface_refs: Vec<String>,
    surface_region_refs: Vec<String>,
    surface_action_refs: Vec<String>,
    data_view_refs: Vec<String>,
    action_refs: Vec<String>,
    operation_path_refs: Vec<String>,
    workflow_refs: Vec<String>,
    interface_refs: Vec<String>,
    state_refs: Vec<String>,
    quality_rule_refs: Vec<String>,
}

fn frontend_task_scope(
    task: &TaskDefinition,
    aac: &ArchitectureArtifactContract,
    frontend: &Value,
    closure_requirements: &[Value],
) -> FrontendTaskScope {
    let mut scope = FrontendTaskScope::default();
    push_unique_strings(
        &mut scope.workflow_refs,
        task.write_boundary.artifact_refs.user_flows.clone(),
    );
    push_unique_strings(
        &mut scope.interface_refs,
        task.write_boundary.artifact_refs.interfaces.clone(),
    );
    for requirement in closure_requirements {
        push_unique(
            &mut scope.workflow_refs,
            string_at(requirement, "workflowRef"),
        );
        push_unique_strings(
            &mut scope.surface_refs,
            string_array_at(requirement, "surfaceRefs"),
        );
        push_unique_strings(
            &mut scope.operation_path_refs,
            string_array_at(requirement, "operationPathRefs"),
        );
        push_unique_strings(
            &mut scope.data_view_refs,
            string_array_at(requirement, "dataViewRefs"),
        );
        push_unique_strings(
            &mut scope.action_refs,
            string_array_at(requirement, "actionRefs"),
        );
        push_unique_strings(
            &mut scope.interface_refs,
            string_array_at(requirement, "interfaceRefs"),
        );
    }
    if let Some(requirement) = task.frontend_experience_requirement.as_ref() {
        push_unique_strings(
            &mut scope.surface_refs,
            scope_refs_from_requirement(
                requirement,
                "/uiTaskScope/surfacesInScope",
                &["surfaceId"],
            ),
        );
        push_unique_strings(
            &mut scope.data_view_refs,
            scope_refs_from_requirement(requirement, "/uiTaskScope/dataViewsInScope", &["viewId"]),
        );
        push_unique_strings(
            &mut scope.action_refs,
            scope_refs_from_requirement(requirement, "/uiTaskScope/actionsInScope", &["actionId"]),
        );
        push_unique_strings(
            &mut scope.operation_path_refs,
            scope_refs_from_requirement(
                requirement,
                "/uiTaskScope/operationPathsInScope",
                &["pathId"],
            ),
        );
        push_unique_strings(
            &mut scope.state_refs,
            scope_refs_from_requirement(requirement, "/uiTaskScope/stateExpectation", &["state"]),
        );
        push_unique_strings(
            &mut scope.surface_region_refs,
            string_array_at_pointer(requirement, "/uiSurfaceOwnership/regionIdsInScope"),
        );
        push_unique_strings(
            &mut scope.surface_action_refs,
            string_array_at_pointer(requirement, "/uiSurfaceOwnership/actionIdsInScope"),
        );
        push_unique_strings(
            &mut scope.state_refs,
            string_array_at_pointer(requirement, "/uiSurfaceOwnership/stateKindsInScope"),
        );
        push_unique_strings(
            &mut scope.quality_rule_refs,
            string_array_at_pointer(requirement, "/uiSurfaceOwnership/qualityRuleIdsInScope"),
        );
        push_unique_strings(
            &mut scope.ownership_dimensions,
            ownership_dimensions_from_requirement(requirement),
        );
    }
    for detail in &aac.detail_coverage {
        if !task
            .requirement_detail_refs
            .iter()
            .any(|detail_id| detail_id == &detail.detail_id)
        {
            continue;
        }
        push_unique_strings(
            &mut scope.data_view_refs,
            detail.artifact_refs.frontend_data_views.clone(),
        );
        push_unique_strings(
            &mut scope.action_refs,
            detail.artifact_refs.frontend_actions.clone(),
        );
        push_unique_strings(
            &mut scope.operation_path_refs,
            detail.artifact_refs.frontend_operation_paths.clone(),
        );
        push_unique_strings(
            &mut scope.workflow_refs,
            detail.artifact_refs.user_flows.clone(),
        );
        push_unique_strings(
            &mut scope.interface_refs,
            detail.artifact_refs.interfaces.clone(),
        );
    }
    for _ in 0..2 {
        enrich_scope_from_operation_paths(frontend, &mut scope);
        enrich_scope_from_surfaces(frontend, &mut scope);
    }
    if task_has_frontend_execution(task)
        && scope.surface_refs.is_empty()
        && scope.data_view_refs.is_empty()
        && scope.action_refs.is_empty()
        && scope.operation_path_refs.is_empty()
    {
        push_unique_strings(
            &mut scope.surface_refs,
            registry_surface_values(frontend)
                .into_iter()
                .filter_map(|surface| string_at(surface, "surfaceId"))
                .chain(
                    array_at(frontend, "surfaces")
                        .into_iter()
                        .filter_map(|surface| string_at(surface, "surfaceId")),
                )
                .collect(),
        );
        push_unique_strings(
            &mut scope.operation_path_refs,
            array_at(frontend, "operationPaths")
                .into_iter()
                .filter_map(|path| string_at(path, "pathId"))
                .collect(),
        );
        push_unique_strings(
            &mut scope.data_view_refs,
            array_at(frontend, "dataViews")
                .into_iter()
                .filter_map(|view| string_at(view, "viewId"))
                .collect(),
        );
        push_unique_strings(
            &mut scope.action_refs,
            array_at(frontend, "actions")
                .into_iter()
                .filter_map(|action| string_at(action, "actionId"))
                .collect(),
        );
    }
    if scope.state_refs.is_empty() {
        if let Some(surface_contract) = frontend.get("uiSurfaceDecisionContract") {
            push_unique_strings(
                &mut scope.state_refs,
                object_array_field(surface_contract, "stateModel", "state"),
            );
        }
    }
    scope.surface_refs = unique_strings(scope.surface_refs);
    scope.surface_region_refs = unique_strings(scope.surface_region_refs);
    scope.surface_action_refs = unique_strings(scope.surface_action_refs);
    scope.data_view_refs = unique_strings(scope.data_view_refs);
    scope.action_refs = unique_strings(scope.action_refs);
    scope.operation_path_refs = unique_strings(scope.operation_path_refs);
    scope.workflow_refs = unique_strings(scope.workflow_refs);
    scope.interface_refs = unique_strings(scope.interface_refs);
    scope.state_refs = unique_strings(scope.state_refs);
    scope.quality_rule_refs = unique_strings(scope.quality_rule_refs);
    if scope.ownership_dimensions.is_empty() && task_has_frontend_execution(task) {
        scope.ownership_dimensions = derived_execution_ownership_dimensions(&scope);
    }
    scope.ownership_dimensions = unique_strings(scope.ownership_dimensions);
    scope
}

fn ownership_dimensions_from_requirement(requirement: &Value) -> Vec<String> {
    requirement
        .pointer("/uiTaskScope/ownershipDimensions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|dimension| UI_OWNERSHIP_DIMENSION_VALUES.contains(dimension))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn derived_execution_ownership_dimensions(scope: &FrontendTaskScope) -> Vec<String> {
    let mut dimensions = Vec::new();
    if !scope.surface_refs.is_empty() || !scope.surface_region_refs.is_empty() {
        dimensions.push("surface".to_string());
        dimensions.push("layout".to_string());
    }
    if !scope.data_view_refs.is_empty() {
        dimensions.push("data_view".to_string());
    }
    if !scope.action_refs.is_empty()
        || !scope.surface_action_refs.is_empty()
        || !scope.operation_path_refs.is_empty()
    {
        dimensions.push("action".to_string());
        dimensions.push("integration_feedback".to_string());
    }
    if !scope.state_refs.is_empty() {
        dimensions.push("state".to_string());
    }
    dimensions.push("visual_system".to_string());
    dimensions.push("content_boundary".to_string());
    unique_strings(dimensions)
}

fn scope_refs_from_requirement(
    requirement: &Value,
    pointer: &str,
    id_keys: &[&str],
) -> Vec<String> {
    requirement
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        id_keys.iter().find_map(|key| {
                            item.get(*key).and_then(Value::as_str).map(str::to_string)
                        })
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn enrich_scope_from_operation_paths(frontend: &Value, scope: &mut FrontendTaskScope) {
    for path in array_at(frontend, "operationPaths") {
        let path_id = string_at(path, "pathId");
        let surface_ref = string_at(path, "surfaceRef");
        let workflow_ref = string_at(path, "workflowRef");
        let interface_refs = string_array_at(path, "interfaceRefs");
        let matches_scope = path_id
            .as_ref()
            .map(|id| scope.operation_path_refs.iter().any(|item| item == id))
            .unwrap_or(false)
            || surface_ref
                .as_ref()
                .map(|id| scope.surface_refs.iter().any(|item| item == id))
                .unwrap_or(false)
            || workflow_ref
                .as_ref()
                .map(|id| scope.workflow_refs.iter().any(|item| item == id))
                .unwrap_or(false)
            || interface_refs
                .iter()
                .any(|id| scope.interface_refs.iter().any(|item| item == id));
        if !matches_scope {
            continue;
        }
        push_unique(&mut scope.operation_path_refs, path_id);
        push_unique(&mut scope.surface_refs, surface_ref);
        push_unique(&mut scope.workflow_refs, workflow_ref);
        push_unique_strings(&mut scope.interface_refs, interface_refs);
        push_unique_strings(
            &mut scope.data_view_refs,
            string_array_at(path, "dataViewRefs"),
        );
        push_unique_strings(&mut scope.action_refs, string_array_at(path, "actionRefs"));
    }
}

fn enrich_scope_from_surfaces(frontend: &Value, scope: &mut FrontendTaskScope) {
    for surface in registry_surface_values(frontend)
        .into_iter()
        .chain(array_at(frontend, "surfaces"))
    {
        let surface_id = string_at(surface, "surfaceId");
        let workflow_refs = string_array_at(surface, "workflowRefs");
        let data_view_refs = string_array_at(surface, "dataViewRefs");
        let action_refs = string_array_at(surface, "actionRefs");
        let operation_path_refs = string_array_at(surface, "operationPathRefs");
        let interface_refs = string_array_at(surface, "interfaceRefs");
        let matches_scope = surface_id
            .as_ref()
            .map(|id| scope.surface_refs.iter().any(|item| item == id))
            .unwrap_or(false)
            || workflow_refs
                .iter()
                .any(|id| scope.workflow_refs.iter().any(|item| item == id))
            || data_view_refs
                .iter()
                .any(|id| scope.data_view_refs.iter().any(|item| item == id))
            || action_refs
                .iter()
                .any(|id| scope.action_refs.iter().any(|item| item == id))
            || operation_path_refs
                .iter()
                .any(|id| scope.operation_path_refs.iter().any(|item| item == id))
            || interface_refs
                .iter()
                .any(|id| scope.interface_refs.iter().any(|item| item == id));
        if !matches_scope {
            continue;
        }
        push_unique(&mut scope.surface_refs, surface_id);
        push_unique_strings(&mut scope.workflow_refs, workflow_refs);
        push_unique_strings(&mut scope.data_view_refs, data_view_refs);
        push_unique_strings(&mut scope.action_refs, action_refs);
        push_unique_strings(&mut scope.operation_path_refs, operation_path_refs);
        push_unique_strings(&mut scope.interface_refs, interface_refs);
        push_unique_strings(&mut scope.state_refs, string_array_at(surface, "stateRefs"));
    }
}

fn selected_frontend_surfaces(frontend: &Value, ids: &[String]) -> Vec<Value> {
    let registry = selected_from_values(registry_surface_values(frontend), "surfaceId", ids);
    if registry.is_empty() {
        selected_frontend_values(frontend, "surfaces", "surfaceId", ids)
    } else {
        registry
    }
}

fn selected_frontend_values(
    frontend: &Value,
    array_key: &str,
    id_key: &str,
    ids: &[String],
) -> Vec<Value> {
    selected_from_values(array_at(frontend, array_key), id_key, ids)
}

fn selected_from_values(values: Vec<&Value>, id_key: &str, ids: &[String]) -> Vec<Value> {
    if ids.is_empty() {
        return vec![];
    }
    values
        .into_iter()
        .filter(|value| {
            string_at(value, id_key)
                .map(|id| ids.iter().any(|item| item == &id))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn registry_surface_values(frontend: &Value) -> Vec<&Value> {
    frontend
        .pointer("/uiSurfaceRegistry/surfaces")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn frontend_backend_bindings_from_scope(
    aac: &ArchitectureArtifactContract,
    scope: &FrontendTaskScope,
) -> Vec<Value> {
    aac.interfaces
        .iter()
        .filter(|interface| {
            string_at(interface, "interfaceId")
                .map(|id| scope.interface_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
        })
        .filter(|interface| is_executable_interface(interface))
        .map(|interface| {
            let interface_id = string_at(interface, "interfaceId").unwrap_or_default();
            json!({
                "bindingId": format!("ui-binding:{interface_id}"),
                "workflowRefs": scope.workflow_refs.clone(),
                "operationPathRefs": scope.operation_path_refs.clone(),
                "interfaces": [compact_interface_binding(interface)],
                "completionRule": "Wire the task-owned UI action or surface to this AAC-declared interface when the task owns the interaction."
            })
        })
        .collect()
}

fn ui_production_brief(
    task: &TaskDefinition,
    frontend: &Value,
    scope: &FrontendTaskScope,
    user_facing_language: &Option<contracts::UserFacingLanguageConstraint>,
) -> Value {
    let surface_contract = frontend
        .get("uiSurfaceDecisionContract")
        .unwrap_or(&Value::Null);
    let surfaces = selected_frontend_surfaces(frontend, &scope.surface_refs);
    let data_views =
        selected_frontend_values(frontend, "dataViews", "viewId", &scope.data_view_refs);
    let ownership_dimensions = if scope.ownership_dimensions.is_empty() {
        derived_execution_ownership_dimensions(scope)
    } else {
        scope.ownership_dimensions.clone()
    };
    json!({
        "schemaVersion": "1.1",
        "briefKind": "task_ui_production_brief",
        "appliesTo": {
            "ownershipDimensions": ownership_dimensions,
            "surfaceIds": scope.surface_refs.clone(),
            "surfaceRoles": unique_strings(surfaces.iter().map(surface_role).collect()),
            "dataViewIds": scope.data_view_refs.clone(),
            "actionIds": scope.action_refs.clone(),
            "operationPathIds": scope.operation_path_refs.clone()
        },
        "productIntent": product_intent(task, &surfaces, &data_views),
        "surfaceDecisionContract": surface_decision_contract_projection(surface_contract, scope),
        "layoutContract": layout_contract(&surfaces, surface_contract),
        "informationContract": information_contract(surface_contract, &surfaces, &data_views),
        "actionContract": action_contract(surface_contract, scope, &surfaces),
        "stateContract": state_contract(surface_contract, &surfaces, scope),
        "visualContract": visual_contract(surface_contract, &surfaces),
        "contentBoundary": content_boundary(surface_contract, user_facing_language)
    })
}

fn product_intent(task: &TaskDefinition, surfaces: &[Value], data_views: &[Value]) -> Value {
    json!({
        "userRole": surface_model_string(surfaces, "/productIntent/userRole")
            .unwrap_or_else(|| "task user".to_string()),
        "businessObject": surface_model_string(surfaces, "/productIntent/businessObject")
            .unwrap_or_else(|| {
                compact_join(
                    unique_strings(data_views.iter().filter_map(value_display_name).collect()),
                    "task-owned business object",
                )
            }),
        "primaryJob": surface_model_string(surfaces, "/productIntent/primaryJob")
            .unwrap_or_else(|| task.objective.clone()),
        "successOutcome": surface_model_string(surfaces, "/productIntent/successOutcome")
            .unwrap_or_else(|| "The user can complete the task-owned workflow and see the updated business state.".to_string())
    })
}

fn surface_decision_contract_projection(contract: &Value, scope: &FrontendTaskScope) -> Value {
    if !contract.is_object() {
        return Value::Null;
    }
    let regions = selected_surface_contract_values(
        contract,
        "regionModel",
        "regionId",
        &scope.surface_region_refs,
    );
    let actions = selected_surface_contract_values(
        contract,
        "actionModel",
        "actionId",
        &scope.surface_action_refs,
    );
    let states =
        selected_surface_contract_values(contract, "stateModel", "state", &scope.state_refs);
    let rules = selected_surface_contract_values(
        contract,
        "qualityRules",
        "ruleId",
        &scope.quality_rule_refs,
    );
    json!({
        "contractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
        "selectionMode": if scope.surface_region_refs.is_empty()
            && scope.surface_action_refs.is_empty()
            && scope.state_refs.is_empty()
            && scope.quality_rule_refs.is_empty()
        {
            "all_when_task_scope_empty"
        } else {
            "task_scope"
        },
        "patternDecision": contract.get("patternDecision").cloned().unwrap_or(Value::Null),
        "semanticFacts": contract.get("semanticFacts").cloned().unwrap_or(Value::Null),
        "layoutModel": contract.get("layoutModel").cloned().unwrap_or(Value::Null),
        "regionsInScope": regions,
        "informationModel": contract.get("informationModel").cloned().unwrap_or(Value::Null),
        "actionsInScope": actions,
        "statesInScope": states,
        "compositionConstraints": contract.get("compositionConstraints").cloned().unwrap_or(Value::Null),
        "contentBoundary": contract.get("contentBoundary").cloned().unwrap_or(Value::Null),
        "qualityRulesInScope": rules
    })
}

fn selected_surface_contract_values(
    contract: &Value,
    array_key: &str,
    id_key: &str,
    ids: &[String],
) -> Vec<Value> {
    let values = array_at(contract, array_key);
    if ids.is_empty() {
        return values.into_iter().cloned().collect();
    }
    selected_from_values(values, id_key, ids)
}

fn layout_contract(surfaces: &[Value], surface_contract: &Value) -> Value {
    let layout_model = surface_contract.get("layoutModel").unwrap_or(&Value::Null);
    let composition = surface_contract
        .get("compositionConstraints")
        .unwrap_or(&Value::Null);
    let layout_baseline = string_at(layout_model, "layoutBaseline")
        .or_else(|| surface_model_string(surfaces, "/visualModel/layoutBaseline"))
        .unwrap_or_else(|| "custom_product_layout".to_string());
    let density = surface_model_string(surfaces, "/visualModel/density")
        .or_else(|| string_at(layout_model, "density"))
        .unwrap_or_else(|| "balanced".to_string());
    let required_regions = non_empty_or(
        unique_strings(
            string_array_at(composition, "requiredComposition")
                .into_iter()
                .chain(
                    surface_model_array(surfaces, "/compositionModel/requiredRegions")
                        .unwrap_or_default(),
                )
                .collect(),
        ),
        default_required_composition,
    );
    let forbidden_regions = non_empty_or(
        unique_strings(
            string_array_at(composition, "forbiddenComposition")
                .into_iter()
                .chain(
                    surface_model_array(surfaces, "/compositionModel/forbiddenRegions")
                        .unwrap_or_default(),
                )
                .collect(),
        ),
        default_forbidden_composition,
    );
    json!({
        "layoutBaseline": layout_baseline,
        "density": density,
        "requiredRegions": required_regions,
        "forbiddenRegions": forbidden_regions,
        "responsiveBehavior": surface_responsive_behavior(layout_model, surfaces),
        "primaryRegion": string_at(layout_model, "primaryWorkRegionId")
            .or_else(|| surface_model_string(surfaces, "/compositionModel/primaryRegion"))
            .unwrap_or_else(|| "task-relevant data, form, detail, or action region".to_string()),
        "supportingRegions": surface_model_array(surfaces, "/compositionModel/supportingRegions")
            .unwrap_or_else(|| vec!["navigation/context".to_string(), "feedback".to_string()])
    })
}

fn information_contract(
    surface_contract: &Value,
    surfaces: &[Value],
    data_views: &[Value],
) -> Value {
    let information_model = surface_contract
        .get("informationModel")
        .unwrap_or(&Value::Null);
    let data_view_names =
        unique_strings(data_views.iter().filter_map(value_display_name).collect());
    json!({
        "primaryObjects": string_array_at(information_model, "primaryObjects"),
        "mustShow": string_array_at(information_model, "fields")
            .into_iter()
            .chain(surface_model_array(surfaces, "/informationModel/mustShow").unwrap_or_default())
            .collect::<Vec<_>>(),
        "scanPriority": string_array_at(information_model, "scanOrder")
            .into_iter()
            .chain(surface_model_array(surfaces, "/informationModel/scanPriority").unwrap_or_default())
            .collect::<Vec<_>>(),
        "identityFields": string_array_at(information_model, "identityFields")
            .into_iter()
            .chain(surface_model_array(surfaces, "/informationModel/identityFields").unwrap_or_default())
            .collect::<Vec<_>>(),
        "statusFields": string_array_at(information_model, "statusFields")
            .into_iter()
            .chain(surface_model_array(surfaces, "/informationModel/statusFields").unwrap_or_default())
            .collect::<Vec<_>>(),
        "longContentPolicy": string_at(information_model, "longContentPolicy")
            .or_else(|| surface_model_string(surfaces, "/informationModel/longContentPolicy"))
            .unwrap_or_else(|| "Long labels, notes, and identifiers must wrap, truncate with access to full value, or move into detail views without breaking layout.".to_string()),
        "dataViews": data_view_names
    })
}

fn action_contract(
    surface_contract: &Value,
    scope: &FrontendTaskScope,
    surfaces: &[Value],
) -> Value {
    let contract_actions = selected_surface_contract_values(
        surface_contract,
        "actionModel",
        "actionId",
        &scope.surface_action_refs,
    );
    json!({
        "actionsInScope": contract_actions,
        "primaryActions": unique_strings(
            selected_surface_contract_values(surface_contract, "actionModel", "actionId", &scope.surface_action_refs)
                .iter()
                .filter_map(|action| string_at(action, "label"))
                .chain(surface_model_array(surfaces, "/actionModel/primaryActions").unwrap_or_default())
                .collect()
        ),
        "contextualActions": surface_model_array(surfaces, "/actionModel/contextualActions")
            .unwrap_or_default(),
        "dangerousActions": surface_model_array(surfaces, "/actionModel/dangerousActions")
            .unwrap_or_default(),
        "placementRule": surface_model_string(surfaces, "/actionModel/placementRule")
            .unwrap_or_else(|| "Place actions where the user makes the decision, keeping affected object identity visible.".to_string()),
        "postSuccessUpdate": selected_surface_contract_values(surface_contract, "actionModel", "actionId", &scope.surface_action_refs)
            .iter()
            .find_map(|action| string_at(action, "postSuccessUpdate"))
            .or_else(|| surface_model_string(surfaces, "/actionModel/postSuccessUpdate"))
            .unwrap_or_else(|| "Update the affected row, detail, count, state, or route; do not rely only on a toast.".to_string())
    })
}

fn state_contract(
    surface_contract: &Value,
    surfaces: &[Value],
    scope: &FrontendTaskScope,
) -> Value {
    let state_refs = if scope.state_refs.is_empty() {
        object_array_field(surface_contract, "stateModel", "state")
    } else {
        scope.state_refs.clone()
    };
    let states_in_scope =
        selected_surface_contract_values(surface_contract, "stateModel", "state", &state_refs);
    json!({
        "statesInScope": state_refs,
        "stateModelsInScope": states_in_scope,
        "loading": state_rule(surfaces, "loading", "Near the region or control waiting for data or mutation."),
        "empty": state_rule(surfaces, "empty", "In the data/form region with filters and business next action when applicable."),
        "error": state_rule(surfaces, "error", "Near the affected region with recovery path and without stack traces."),
        "success": state_rule(surfaces, "success", "Inline object update plus short confirmation when useful."),
        "business_blocking": state_rule(surfaces, "business_blocking", "Near the blocked field, row, detail, or action with product-language reason."),
        "validation": state_rule(surfaces, "validation", "Near the field and in a summary for longer forms."),
        "disabled": state_rule(surfaces, "disabled", "On or near disabled controls with unlock reason when actionable.")
    })
}

fn visual_contract(surface_contract: &Value, surfaces: &[Value]) -> Value {
    let composition = surface_contract
        .get("compositionConstraints")
        .unwrap_or(&Value::Null);
    let density = surface_model_string(surfaces, "/visualModel/density")
        .or_else(|| {
            surface_contract
                .pointer("/layoutModel/density")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "balanced".to_string());
    json!({
        "tokenPolicy": surface_model_string(surfaces, "/visualModel/tokenPolicy")
            .unwrap_or_else(|| "Use existing or planned semantic tokens before page-local styling; do not create a second token system beside an existing one.".to_string()),
        "componentPolicy": surface_model_string(surfaces, "/visualModel/componentPolicy")
            .unwrap_or_else(|| "Use task-fit components for data, forms, details, actions, feedback, and navigation instead of decorative capability cards.".to_string()),
        "densityRule": format!("Use {density} density consistently for spacing, row height, control sizing, and information grouping."),
        "antiDemoRules": string_array_at(composition, "antiDemoRules")
            .into_iter()
            .chain(surface_model_array(surfaces, "/visualModel/antiDemoRules").unwrap_or_default())
            .collect::<Vec<_>>()
    })
}

fn content_boundary(
    surface_contract: &Value,
    user_facing_language: &Option<contracts::UserFacingLanguageConstraint>,
) -> Value {
    let surface_boundary = surface_contract
        .get("contentBoundary")
        .unwrap_or(&Value::Null);
    json!({
        "userFacingLanguage": user_facing_language
            .as_ref()
            .map(|constraint| constraint.rule.clone())
            .unwrap_or_else(|| "Use the project's confirmed user-facing language and product vocabulary.".to_string()),
        "allowedUserVisibleContent": surface_boundary
            .get("allowedUserVisibleContent")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "forbiddenUserVisibleContent": surface_boundary
            .get("forbiddenUserVisibleContent")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "copyRule": surface_boundary
            .get("copyRule")
            .and_then(Value::as_str)
            .unwrap_or("Write product copy for the user's business task. Do not expose runtime commands, technical stack explanations, delivery progress, verification instructions, internal workflow terms, generated artifact ids, or validator language unless the product itself is a developer/runtime tool.")
    })
}

fn surface_model_string(surfaces: &[Value], pointer: &str) -> Option<String> {
    surfaces
        .iter()
        .find_map(|surface| surface.pointer(pointer).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn surface_model_array(surfaces: &[Value], pointer: &str) -> Option<Vec<String>> {
    let values = unique_strings(
        surfaces
            .iter()
            .flat_map(|surface| {
                surface
                    .pointer(pointer)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
            })
            .collect(),
    );
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn responsive_behavior(surfaces: &[Value]) -> String {
    let mut parts = Vec::new();
    if let Some(value) = surface_model_string(surfaces, "/responsiveModel/desktop") {
        parts.push(format!("desktop: {value}"));
    }
    if let Some(value) = surface_model_string(surfaces, "/responsiveModel/tablet") {
        parts.push(format!("tablet: {value}"));
    }
    if let Some(value) = surface_model_string(surfaces, "/responsiveModel/mobile") {
        parts.push(format!("mobile: {value}"));
    }
    if parts.is_empty() {
        "Keep task order, object identity, primary action, and scoped feedback usable across required viewports; use cards, drill-down, stacking, or horizontal overflow only when they preserve the task.".to_string()
    } else {
        parts.join("; ")
    }
}

fn surface_responsive_behavior(layout_model: &Value, surfaces: &[Value]) -> String {
    let mut parts = Vec::new();
    for posture in ["desktop", "tablet", "mobile"] {
        if let Some(intent) = layout_model
            .pointer(&format!("/{posture}/layoutIntent"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            parts.push(format!("{posture}: {intent}"));
        }
    }
    if parts.is_empty() {
        responsive_behavior(surfaces)
    } else {
        parts.join("; ")
    }
}

fn state_rule(surfaces: &[Value], state: &str, fallback: &str) -> String {
    surface_model_string(surfaces, &format!("/statePlacementModel/{state}"))
        .unwrap_or_else(|| fallback.to_string())
}

fn style_asset_plan(frontend: &Value) -> Value {
    let surface_contract = frontend
        .get("uiSurfaceDecisionContract")
        .unwrap_or(&Value::Null);
    json!({
        "designTokenAssetPlan": surface_contract.get("designTokenAssetPlan").cloned().unwrap_or(Value::Null),
        "semanticTokenPolicy": surface_contract.get("semanticTokenPolicy").cloned().unwrap_or(Value::Null),
        "referencePlan": surface_contract.get("referencePlan").cloned().unwrap_or_else(|| json!([])),
        "implementationRule": "Load only UIX files listed in referencePlan when this task changes user-visible frontend code. Use designTokenAssetPlan as the single token asset authority for this task."
    })
}

fn surface_role(surface: &Value) -> String {
    string_at(surface, "surfaceRole")
        .or_else(|| string_at(surface, "role"))
        .unwrap_or_else(|| "page".to_string())
}

fn value_display_name(value: &Value) -> Option<String> {
    string_at(value, "label")
        .or_else(|| string_at(value, "name"))
        .or_else(|| string_at(value, "title"))
}

fn default_required_composition() -> Vec<String> {
    vec![
        "business navigation or local context".to_string(),
        "task-relevant data view, form, table, detail, or action area".to_string(),
        "local loading, empty, error, success, and business-blocking states where applicable"
            .to_string(),
    ]
}

fn default_forbidden_composition() -> Vec<String> {
    vec![
        "surface composition unrelated to the task-owned business workflow".to_string(),
        "decorative or explanatory sections that displace required data, actions, states, or feedback"
            .to_string(),
    ]
}

fn object_array_field(value: &Value, array_key: &str, field_key: &str) -> Vec<String> {
    value
        .get(array_key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get(field_key)
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn compact_join(values: Vec<String>, fallback: &str) -> String {
    if values.is_empty() {
        fallback.to_string()
    } else {
        values.join("; ")
    }
}

fn push_unique(values: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value {
        if !value.trim().is_empty() && !values.iter().any(|item| item == &value) {
            values.push(value);
        }
    }
}

fn push_unique_strings(values: &mut Vec<String>, next: Vec<String>) {
    for value in next {
        push_unique(values, Some(value));
    }
}

fn workflow_closure_requirements_for_task(
    task: &TaskDefinition,
    aac: &ArchitectureArtifactContract,
) -> Vec<Value> {
    workflow_closure_requirements(aac)
        .into_iter()
        .filter(|requirement| task_matches_workflow_closure(task, requirement))
        .collect()
}

fn task_matches_workflow_closure(task: &TaskDefinition, requirement: &Value) -> bool {
    let refs = &task.write_boundary.artifact_refs;
    let workflow_ref = string_at(requirement, "workflowRef");
    let workflow_matches = workflow_ref
        .as_ref()
        .map(|workflow_ref| refs.user_flows.iter().any(|item| item == workflow_ref))
        .unwrap_or(false);
    let interface_refs = string_array_at(requirement, "interfaceRefs");
    let interface_matches = !interface_refs.is_empty()
        && interface_refs
            .iter()
            .any(|interface_ref| refs.interfaces.iter().any(|item| item == interface_ref));
    let acceptance_refs = string_array_at(requirement, "acceptanceRefs");
    let acceptance_matches = acceptance_refs.is_empty()
        || acceptance_refs.iter().any(|acceptance_ref| {
            task.acceptance_refs
                .iter()
                .any(|item| item == acceptance_ref)
        });
    task.frontend_experience_requirement.is_some()
        && acceptance_matches
        && (workflow_matches || interface_matches)
}

fn workflow_closure_requirements(aac: &ArchitectureArtifactContract) -> Vec<Value> {
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
        if let Some(surface_ref) = string_at(operation_path, "surfaceRef") {
            surface_refs_by_flow
                .entry(workflow_ref)
                .or_default()
                .push(surface_ref);
        }
    }
    let mut requirements = Vec::new();
    for (workflow_ref, surface_refs) in surface_refs_by_flow {
        let Some(flow) = flow_by_id.get(workflow_ref.as_str()) else {
            continue;
        };
        if string_at(flow, "kind").as_deref() != Some("user_interaction") {
            continue;
        }
        let operation_paths = array_at(frontend, "operationPaths")
            .into_iter()
            .filter(|path| {
                string_at(path, "workflowRef").as_deref() == Some(workflow_ref.as_str())
                    || string_at(path, "surfaceRef")
                        .map(|surface_ref| surface_refs.iter().any(|item| item == &surface_ref))
                        .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        let operation_path_refs = unique_strings(
            operation_paths
                .iter()
                .filter_map(|path| string_at(path, "pathId"))
                .collect(),
        );
        let data_view_refs = unique_strings(
            operation_paths
                .iter()
                .flat_map(|path| string_array_at(path, "dataViewRefs"))
                .collect(),
        );
        let action_refs = unique_strings(
            operation_paths
                .iter()
                .flat_map(|path| string_array_at(path, "actionRefs"))
                .collect(),
        );
        for step in array_at(flow, "steps") {
            let step_id = string_at(step, "stepId").unwrap_or_else(|| "step".to_string());
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
                "interfaceRefs": unique_strings(executable_interfaces.iter().filter_map(|interface| string_at(interface, "interfaceId")).collect()),
                "stateMachineRefs": unique_strings(string_array_at(step, "stateMachineRefs")),
                "stepRefs": [step_id.clone()],
                "requiredDataBindingMode": "wired",
                "requiredEvidence": [
                    "user_action",
                    "declared_interface_invocation",
                    "state_or_persistence_change",
                    "success_or_blocking_feedback"
                ],
                "interfaces": executable_interfaces.iter().map(|interface| compact_interface_binding(interface)).collect::<Vec<_>>()
            }));
        }
    }
    requirements
}

fn workflow_closure_requirement_execution_view(requirements: &[Value]) -> Vec<Value> {
    requirements
        .iter()
        .map(|requirement| {
            json!({
                "closureId": string_at(requirement, "closureId").unwrap_or_default(),
                "workflowRef": string_at(requirement, "workflowRef").unwrap_or_default(),
                "workflowName": string_at(requirement, "workflowName").unwrap_or_default(),
                "surfaceRefs": string_array_at(requirement, "surfaceRefs"),
                "operationPathRefs": string_array_at(requirement, "operationPathRefs"),
                "dataViewRefs": string_array_at(requirement, "dataViewRefs"),
                "actionRefs": string_array_at(requirement, "actionRefs"),
                "acceptanceRefs": string_array_at(requirement, "acceptanceRefs"),
                "interfaceRefs": string_array_at(requirement, "interfaceRefs"),
                "stateMachineRefs": string_array_at(requirement, "stateMachineRefs"),
                "requiredDataBindingMode": "wired",
                "requiredEvidence": requirement.get("requiredEvidence").cloned().unwrap_or(Value::Array(vec![])),
                "evidenceRule": "Evidence must cover user action, declared interface invocation, state or persistence change, and success or blocking feedback."
            })
        })
        .collect()
}

fn frontend_backend_bindings(requirements: &[Value]) -> Vec<Value> {
    requirements
        .iter()
        .flat_map(|requirement| {
            let workflow_ref = string_at(requirement, "workflowRef").unwrap_or_default();
            let workflow_name = string_at(requirement, "workflowName").unwrap_or_default();
            let step_ref = string_array_at(requirement, "stepRefs")
                .into_iter()
                .next()
                .unwrap_or_default();
            array_at(requirement, "interfaces")
                .into_iter()
                .map(move |interface| {
                    json!({
                        "bindingId": format!("{workflow_ref}:{step_ref}"),
                        "workflowRef": workflow_ref,
                        "workflowName": workflow_name,
                        "stepRef": step_ref,
                        "interfaces": [interface.clone()],
                        "completionRule": "Wire the user action to this AAC-declared interface and verify readback or feedback."
                    })
                })
        })
        .collect()
}

fn compact_interface_binding(interface: &Value) -> Value {
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
}

fn selected_values(
    values: &[Value],
    id_key: &str,
    explicit_refs: &[String],
    task: &TaskDefinition,
    include_scope_acceptance_match: bool,
) -> Vec<Value> {
    values
        .iter()
        .filter(|value| {
            string_at(value, id_key)
                .map(|id| explicit_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
                || (include_scope_acceptance_match && artifact_matches_task_scope(value, task))
        })
        .cloned()
        .collect()
}

fn selected_entities(
    data_model: &Value,
    explicit_refs: &[String],
    task: &TaskDefinition,
) -> Vec<Value> {
    data_model
        .pointer("/entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entity| {
            string_at(entity, "entityId")
                .map(|id| explicit_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
                || artifact_matches_task_scope(entity, task)
        })
        .cloned()
        .collect()
}

fn artifact_matches_task_scope(value: &Value, task: &TaskDefinition) -> bool {
    let scope_match = string_array_at(value, "scopeRefs")
        .iter()
        .any(|scope_ref| task.scope_refs.iter().any(|item| item == scope_ref));
    let acceptance_match = string_array_at(value, "acceptanceRefs")
        .iter()
        .any(|acceptance_ref| {
            task.acceptance_refs
                .iter()
                .any(|item| item == acceptance_ref)
        });
    scope_match || acceptance_match
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

fn string_array_at_pointer(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
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

fn non_empty_or<F>(values: Vec<String>, fallback: F) -> Vec<String>
where
    F: FnOnce() -> Vec<String>,
{
    if values.is_empty() {
        fallback()
    } else {
        values
    }
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

fn running_or_ready_task_id(run: &TaskPlanRun) -> Option<String> {
    if let Some(running) = run
        .task_states
        .iter()
        .find(|state| state.status == TaskRunStatus::Running)
    {
        return Some(running.task_id.clone());
    }
    let completed = run
        .task_states
        .iter()
        .filter(|state| {
            matches!(
                state.status,
                TaskRunStatus::Completed | TaskRunStatus::CompletedWithNotes
            )
        })
        .map(|state| state.task_id.clone())
        .collect::<BTreeSet<_>>();
    run.task_states
        .iter()
        .find(|state| {
            state.status == TaskRunStatus::Pending
                && state.depends_on.iter().all(|dep| completed.contains(dep))
        })
        .map(|state| state.task_id.clone())
}

fn dependency_results(run: &TaskPlanRun, task: &TaskDefinition) -> Vec<Value> {
    run.task_states
        .iter()
        .filter(|state| task.depends_on.iter().any(|dep| dep == &state.task_id))
        .map(|state| {
            json!({
                "taskId": state.task_id,
                "status": state.status,
                "resultId": state.result_id
            })
        })
        .collect()
}

pub(crate) fn load_current_plan_and_run(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
) -> Result<(TaskPlan, TaskPlanRun), state::store::StateError> {
    let latest_plan: Value =
        state::store::read_json(&task_plan_latest_file(project_root, locator))?;
    let plan_ref = latest_plan
        .get("taskPlanRef")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "taskplans/latest.json missing taskPlanRef".to_string(),
            )
        })?;
    let latest_run: Value =
        state::store::read_json(&task_plan_run_latest_file(project_root, locator))?;
    let run_ref = latest_run
        .get("runRef")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("runs/latest.json missing runRef".to_string())
        })?;
    Ok((
        read_project_json(project_root, plan_ref)?,
        read_project_json(project_root, run_ref)?,
    ))
}

pub(crate) fn save_run(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    run: &TaskPlanRun,
) -> Result<(), state::store::StateError> {
    state::store::write_json_atomic(&task_plan_run_file(project_root, locator, &run.run_id), run)?;
    state::store::write_json_atomic(
        &task_plan_run_latest_file(project_root, locator),
        &json!({
            "schemaVersion": "1.0",
            "taskPlanRunId": run.run_id,
            "runRef": to_project_relative(project_root, &task_plan_run_file(project_root, locator, &run.run_id))?,
            "taskPlanId": run.task_plan_id,
            "updatedAt": run.updated_at
        }),
    )
}

fn update_route_for_execution(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    result_file: &str,
    task: &TaskDefinition,
    run: &TaskPlanRun,
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
            "taskExecutionRequestRef".to_string(),
            request_ref.to_string(),
        );
        phase.latest_refs.insert(
            "taskExecutionResultFile".to_string(),
            result_file.to_string(),
        );
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::ContinueExecution,
            source: "task_execution_request".to_string(),
            reason: "task_execution_request_created".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(request_ref.to_string()),
            details: Some(json!({
                "taskId": task.task_id,
                "groupId": task.group_id,
                "taskPlanRunId": run.run_id
            })),
            target_phase_id: None,
        });
    }
    delivery.status = DeliveryLifecycleStatus::Executing;
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

fn update_route_for_browser_runtime_prepare(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    request_ref: &str,
    task: &TaskDefinition,
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
        phase.latest_refs.insert(
            "browserRuntimePrepareRequestRef".to_string(),
            request_ref.to_string(),
        );
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::ContinueExecution,
            source: "browser_runtime_prepare_request".to_string(),
            reason: "browser_runtime_prepare_required".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(request_ref.to_string()),
            details: Some(json!({
                "taskId": task.task_id,
                "groupId": task.group_id
            })),
            target_phase_id: None,
        });
    }
    delivery.status = DeliveryLifecycleStatus::Executing;
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)
}

fn update_route_for_review(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
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
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::Review,
            source: "task_plan_run".to_string(),
            reason: "taskplan_run_ready_for_review".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        });
    }
    delivery.status = DeliveryLifecycleStatus::Reviewing;
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

fn safe_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_production_brief_surface_projection_is_task_scoped() {
        let contract = json!({
            "patternDecision": {
                "mode": "known",
                "knownPattern": "collection_workbench"
            },
            "semanticFacts": {
                "userJobs": ["browse"]
            },
            "layoutModel": {
                "density": "workbench_dense"
            },
            "regionModel": [
                { "regionId": "region_primary", "purpose": "primary work" },
                { "regionId": "region_secondary", "purpose": "secondary work" }
            ],
            "informationModel": {
                "primaryObjects": ["request"]
            },
            "actionModel": [
                { "actionId": "action_create", "label": "Create" },
                { "actionId": "action_archive", "label": "Archive" }
            ],
            "stateModel": [
                { "state": "loading" },
                { "state": "error" }
            ],
            "compositionConstraints": {
                "antiDemoRules": ["no_internal_process_copy"]
            },
            "contentBoundary": {
                "forbiddenUserVisibleContent": ["runtime_commands"]
            },
            "qualityRules": [
                { "ruleId": "rule_primary", "expectation": "primary" },
                { "ruleId": "rule_secondary", "expectation": "secondary" }
            ]
        });
        let scope = FrontendTaskScope {
            surface_region_refs: vec!["region_primary".to_string()],
            surface_action_refs: vec!["action_create".to_string()],
            state_refs: vec!["loading".to_string()],
            quality_rule_refs: vec!["rule_primary".to_string()],
            ..FrontendTaskScope::default()
        };

        let projection = surface_decision_contract_projection(&contract, &scope);

        assert_eq!(
            projection.get("selectionMode").and_then(Value::as_str),
            Some("task_scope")
        );
        assert_eq!(
            projection
                .pointer("/regionsInScope/0/regionId")
                .and_then(Value::as_str),
            Some("region_primary")
        );
        assert_eq!(
            projection
                .pointer("/actionsInScope/0/actionId")
                .and_then(Value::as_str),
            Some("action_create")
        );
        assert_eq!(
            projection
                .pointer("/qualityRulesInScope/0/ruleId")
                .and_then(Value::as_str),
            Some("rule_primary")
        );
        assert_eq!(
            projection
                .get("regionsInScope")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1),
            "projection must not copy unrelated regions when task scope is explicit"
        );
    }
}
