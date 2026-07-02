use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    ArchitectureArtifactContract, EngineeringQualityRequirement, ImplementationAction,
    TaskDefinition, TaskKind, TaskPlan, TaskPlanRun, TaskPlanRunNextAction, TaskPlanRunStatus,
    TaskRunStatus, VerificationEvidence,
};
use delivery_core::{
    apply_delivery_index, read_selectors_value_from_paths, DeliveryLifecycleStatus,
    LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult, RouteAction, RouteActionKind,
    TransitionStore,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
};

use crate::{
    paths::{
        task_execution_request_file, task_execution_result_candidate_file, task_plan_file,
        task_plan_latest_file, task_plan_run_file, task_plan_run_latest_file,
    },
    task_plan::{execute_task_next_from_request, update_run_summary},
    templates::{
        frontend_quality_self_check_applies, frontend_self_check_applies,
        runtime_delivery_evidence_applies, task_result_required_top_level_fields,
        task_result_template, FRONTEND_QUALITY_CONTRACT_READ_FIELDS,
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
    let architecture_projection = task_scoped_architecture_projection(&aac, &request_task);
    let schema_shape = serde_json::to_value(schema_for!(contracts::TaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let dependency_results = dependency_results(run, task);
    let read_groups = task_execution_read_groups(&request_task, !dependency_results.is_empty());
    let user_facing_language = pgc.planning_inputs.user_facing_language.clone();
    let execution_rules =
        task_execution_rules(result_file, &request_task, user_facing_language.clone());
    let result_rules = task_result_rules(&request_task);
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
            "verificationEvidence": ["automated_test", "manual_command_output", "runtime_api_check", "static_check", "agent_review_explanation"],
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
            "resultTemplate": task_result_template(&task_plan.task_plan_id, &request_task),
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

fn task_result_rules(task: &TaskDefinition) -> Value {
    let mut rules = vec![
        "TaskResult must include every requiredTopLevelFields entry.".to_string(),
        "If status is completed, every verification intent should have passed evidence.".to_string(),
        "If status is failed, failure is required.".to_string(),
        "TaskResult must include executionContinuity; if agent-owned long-running work release state is unknown, status cannot be completed.".to_string(),
        "changedFiles must list intended deliverable files, not incidental dependency directories, caches, logs, or generated build output.".to_string(),
        "noChangeReason must be null when changedFiles is non-empty; when changedFiles is empty and a reason is needed, noChangeReason must be an object with code and summary, never a string or array.".to_string(),
        "For completed or completed_with_notes results, every requirementDetailEvidence entry must include verificationIds that reference task.verificationIntents; do not leave verificationIds empty.".to_string(),
    ];
    if frontend_self_check_applies(task) {
        rules.push("For frontend tasks, fill frontendExperienceSelfCheck using task.frontendExperienceRequirement.executionGuidance and frontend/backend bindings when present.".to_string());
        rules.push("For browser/e2e/interactive verification, follow executionRules.interactiveVerificationProbePolicy and record evidence through existing TaskResult fields.".to_string());
    }
    if runtime_delivery_evidence_applies(task) {
        rules.push("For runtimeDeliveryRequirement tasks, include runtimeDeliveryEvidence with checkedFields, codeLevelChecks, commandsRun when commands were run, and unverifiedItems when environment prevents a check.".to_string());
        rules.push("For runtimeDeliveryEvidence.codeLevelChecks, use only the exact checkId values listed in task.runtimeDeliveryRequirement.requiredCodeLevelChecks[].checkId.".to_string());
        rules.push("If a temporary runtime/probe/server/container was started, include runtimeDeliveryEvidence.runtimeProbeCleanup; cleanup failure alone should be completed_with_notes, not failed or blocked.".to_string());
    }
    if !task.engineering_quality_requirement_refs.is_empty() {
        rules.push("For referenced engineeringQualityRequirements, verificationResults summaries must state how implementation kept the declared alignmentTargets aligned for this task.".to_string());
        rules.push("For persistence_mapping requirements, evidence must cover changed risk field kinds across domain model, storage schema or migration, data access mapping, DTO/API contract, and same-provider persistence behavior when those parts are in task scope.".to_string());
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

fn task_execution_read_groups(task: &TaskDefinition, has_dependency_results: bool) -> Value {
    let has_frontend_execution = task_has_frontend_execution(task);
    let has_frontend_requirement = task.frontend_experience_requirement.is_some();
    let needs_runtime_probe_rules = task_needs_controlled_runtime_probe_rules(task);
    let mut core_fields = vec![
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
        "sourceContext.technicalBaseline.stack",
        "sourceContext.architectureArtifactProjection.modules",
        "sourceContext.architectureArtifactProjection.entities",
        "sourceContext.architectureArtifactProjection.interfaces",
        "sourceContext.architectureArtifactProjection.userFlows",
        "sourceContext.architectureArtifactProjection.stateMachines",
        "sourceContext.acceptanceSnapshot",
        "sourceContext.requirementDetailSnapshot",
        "sourceContext.userFacingLanguage",
        "executionRules.sourceEditPreparationContract",
        "executionRules.verificationCommandSchedulingRules",
        "executionRules.userFacingLanguage",
        "executionRules.boundaryRules",
    ];
    if has_frontend_requirement {
        core_fields.extend([
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
        ]);
    }
    if frontend_quality_self_check_applies(task) {
        core_fields.extend([
            "task.frontendExperienceRequirement.executionGuidance.uiQuality",
            "task.frontendExperienceRequirement.uiQualityContractRef",
        ]);
        core_fields.extend(FRONTEND_QUALITY_CONTRACT_READ_FIELDS);
    }
    if has_frontend_execution {
        if !runtime_delivery_evidence_applies(task) {
            core_fields.push("executionRules.frontendImplementationOrganizationRules");
        }
        core_fields.push("executionRules.interactiveVerificationProbePolicy");
    }
    if needs_runtime_probe_rules {
        core_fields.push("executionRules.controlledRuntimeProbeRules");
    }
    if runtime_delivery_evidence_applies(task) {
        core_fields.extend(runtime_delivery_requirement_read_fields(task));
        core_fields.push("sourceContext.architectureArtifactProjection.runtimeDelivery");
        core_fields.push("executionRules.runtimeDeliveryExecutionRules");
    }
    if !task.engineering_quality_requirement_refs.is_empty() {
        core_fields.extend([
            "task.engineeringQualityRequirementRefs",
            "sourceContext.engineeringQualityRequirements",
            "executionRules.engineeringQualityExecutionRules",
        ]);
    }
    if has_dependency_results {
        core_fields.push("sourceContext.dependencyResults");
    }
    if !task.concept_refs.is_empty() {
        core_fields.push("task.conceptRefs");
    }
    if !task.concept_responsibilities.is_empty() {
        core_fields.push("task.conceptResponsibilities");
    }
    if !task.concept_verification_intents.is_empty() {
        core_fields.push("task.conceptVerificationIntents");
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

    Value::Array(vec![
        json!({
            "groupId": "task_execution_core",
            "required": true,
            "purpose": "Read task identity, task-scoped architecture context, and execution rules before editing.",
            "whenToRead": "Read before any source edit.",
            "selectors": read_selectors_value_from_paths(core_fields)
        }),
        json!({
            "groupId": "task_execution_result_contract",
            "required": true,
            "purpose": "Read TaskResult output file, schema fields, enum values, and completion barrier.",
            "whenToRead": "Read before writing TaskResult.",
            "selectors": read_selectors_value_from_paths(result_fields)
        }),
    ])
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

fn task_with_execution_guidance(
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
        "stateMachines": selected_values(&aac.state_machines, "machineId", &state_machine_refs, task, true)
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
            "guidanceWarnings": ["AAC frontendExperience is absent."]
        });
    };
    let closure_requirements = workflow_closure_requirements_for_task(task, aac);
    let closure_workflow_refs = closure_requirements
        .iter()
        .filter_map(|item| string_at(item, "workflowRef"))
        .collect::<Vec<_>>();
    let workflow_refs = unique_strings(
        task.write_boundary
            .artifact_refs
            .user_flows
            .iter()
            .cloned()
            .chain(closure_workflow_refs)
            .collect(),
    );
    let surface_refs = unique_strings(
        closure_requirements
            .iter()
            .flat_map(|item| string_array_at(item, "surfaceRefs"))
            .collect(),
    );
    let operation_path_refs = unique_strings(
        closure_requirements
            .iter()
            .flat_map(|item| string_array_at(item, "operationPathRefs"))
            .collect(),
    );
    let data_view_refs = unique_strings(
        closure_requirements
            .iter()
            .flat_map(|item| string_array_at(item, "dataViewRefs"))
            .collect(),
    );
    let action_refs = unique_strings(
        closure_requirements
            .iter()
            .flat_map(|item| string_array_at(item, "actionRefs"))
            .collect(),
    );
    let surfaces = array_at(frontend, "surfaces")
        .into_iter()
        .filter(|surface| {
            string_at(surface, "surfaceId")
                .map(|id| surface_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
                || string_array_at(surface, "workflowRefs")
                    .iter()
                    .any(|workflow_ref| workflow_refs.iter().any(|item| item == workflow_ref))
        })
        .cloned()
        .collect::<Vec<_>>();
    let operation_paths = array_at(frontend, "operationPaths")
        .into_iter()
        .filter(|path| {
            string_at(path, "pathId")
                .map(|id| operation_path_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
                || string_at(path, "workflowRef")
                    .map(|id| workflow_refs.iter().any(|item| item == &id))
                    .unwrap_or(false)
                || string_at(path, "surfaceRef")
                    .map(|id| surface_refs.iter().any(|item| item == &id))
                    .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let data_views = array_at(frontend, "dataViews")
        .into_iter()
        .filter(|view| {
            string_at(view, "viewId")
                .map(|id| data_view_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let actions = array_at(frontend, "actions")
        .into_iter()
        .filter(|action| {
            string_at(action, "actionId")
                .map(|id| action_refs.iter().any(|item| item == &id))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let closure_ids = closure_requirements
        .iter()
        .filter_map(|item| string_at(item, "closureId"))
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    if closure_requirements.is_empty() {
        warnings.push("No workflow closure requirement matched this task; use task refs and source context to decide whether frontend work is static, mocked, or wired.".to_string());
    }
    let mut guidance = json!({
        "schemaVersion": "1.0",
        "purpose": "Task-scoped frontend execution guidance derived from AAC and TaskPlan refs.",
        "userFacingLanguage": user_facing_language,
        "responsibility": task.objective,
        "surfacesInScope": surfaces,
        "dataViewsInScope": data_views,
        "actionsInScope": actions,
        "operationPathsInScope": operation_paths,
        "frontendBackendBindings": frontend_backend_bindings(&closure_requirements),
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
        "guidanceWarnings": warnings
    });
    let ui_quality = frontend_quality_execution_guidance(task);
    if !ui_quality.is_null() {
        guidance["uiQuality"] = ui_quality;
    }
    guidance
}

fn frontend_quality_execution_guidance(task: &TaskDefinition) -> Value {
    let Some(requirement) = task.frontend_experience_requirement.as_ref() else {
        return Value::Null;
    };
    if requirement.get("uiQualityContract").is_none() {
        return Value::Null;
    }
    json!({
        "contractRef": requirement.get("uiQualityContractRef").cloned().unwrap_or(Value::Null),
        "contractField": "task.frontendExperienceRequirement.uiQualityContract",
        "selfCheckField": "frontendQualitySelfCheck",
        "mustCover": [
            "scenario",
            "qualityLevel",
            "referenceProfile.referenceIds",
            "designTokenAssetPlan",
            "requiredUiStates",
            "businessUiRules",
            "forbiddenUserVisibleContent"
        ],
        "rule": "Implement the UI according to the uiQualityContract fields, apply designTokenAssetPlan by reusing/extending existing token assets before creating new ones, and report evidence in frontendQualitySelfCheck."
    })
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
