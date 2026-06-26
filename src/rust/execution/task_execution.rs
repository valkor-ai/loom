use std::{collections::BTreeSet, path::Path};

use contracts::{
    ArchitectureArtifactContract, TaskDefinition, TaskPlan, TaskPlanRun, TaskPlanRunNextAction,
    TaskPlanRunStatus, TaskRunStatus,
};
use delivery_core::{
    apply_delivery_index, DeliveryLifecycleStatus, LoomMcpActionResult, LoomMcpFailure,
    LoomMcpFailureResult, RouteAction, RouteActionKind, TransitionStore,
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
    templates::task_result_template,
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
    let schema_shape = serde_json::to_value(schema_for!(contracts::TaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
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
        "task": task,
        "sourceContext": {
            "technicalBaseline": {
                "projectKind": baseline.project_kind,
                "stack": baseline.stack
            },
            "architectureArtifactProjection": {
                "modules": aac.modules,
                "interfaces": aac.interfaces,
                "userFlows": aac.user_flows,
                "stateMachines": aac.state_machines,
                "frontendExperience": aac.frontend_experience,
                "runtimeDelivery": aac.runtime_delivery
            },
            "acceptanceSnapshot": pgc.phase_scope.acceptance_candidates.iter()
                .filter(|acceptance| task.acceptance_refs.iter().any(|id| id == &acceptance.id))
                .collect::<Vec<_>>(),
            "requirementDetailSnapshot": pgc.requirement_details.items.iter()
                .filter(|detail| task.requirement_detail_refs.iter().any(|id| id == &detail.detail_id))
                .collect::<Vec<_>>(),
            "userFacingLanguage": pgc.planning_inputs.user_facing_language,
            "dependencyResults": dependency_results(run, task)
        },
        "executionRules": {
            "sourceEditPreparationContract": {
                "rule": "Before source edits, read task, sourceContext, executionRules, and outputContract through requestReadPlan groups."
            },
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
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none | started_and_released | unknown"
            },
            "verificationCommandSchedulingRules": [
                "Verification method is agent-chosen and must return control before TaskResult submission.",
                "Do not leave long-running servers, watchers, browsers, or workers unreleased before TaskResult submission."
            ],
            "userFacingLanguage": {
                "constraint": pgc.planning_inputs.user_facing_language,
                "rule": "Preserve user-facing language from confirmed requirements in generated UI and feedback."
            },
            "rules": [
                "Execute only the current task.",
                "Do not modify Brainstorm, TechnicalBaseline, PGC, AAC, TaskPlan, or other protected Loom artifacts.",
                "Do not implement deferred scope.",
                "Write TaskResult JSON only to outputContract.resultFile."
            ]
        },
        "taskConceptGrounding": {
            "conceptRefs": task.concept_refs,
            "conceptResponsibilities": task.concept_responsibilities,
            "conceptVerificationIntents": task.concept_verification_intents
        },
        "blockedOutput": {
            "blockedReasons": [
                {"code": "DESIGN_INSUFFICIENT", "nextNode": "architecture_artifact_repair"},
                {"code": "TASKPLAN_INVALID", "nextNode": "taskplan_repair"},
                {"code": "DEPENDENCY_NOT_READY", "nextNode": "wait_dependency"}
            ]
        },
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
            "requiredTopLevelFields": [
                "schemaVersion", "taskResultId", "taskId", "taskPlanId", "status",
                "changedFiles", "noChangeReason", "verificationResults", "selfRepairSummary",
                "failure", "executionContinuity", "notes", "frontendExperienceSelfCheck",
                "runtimeDeliveryEvidence", "requirementDetailEvidence", "conceptEvidence",
                "blockedReasons", "createdAt", "updatedAt"
            ],
            "schemaShape": schema_shape,
            "resultTemplate": task_result_template(&task_plan.task_plan_id, task),
            "resultRules": [
                "TaskResult must include every requiredTopLevelFields entry.",
                "If status is completed, every verification intent should have passed evidence.",
                "If status is failed, failure is required."
            ]
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "task_execution_core",
                    "required": true,
                    "purpose": "Read task identity, task contract, source context, and execution rules before editing.",
                    "whenToRead": "Read before any source edit.",
                    "fields": [
                        "source.taskPlanId",
                        "source.taskId",
                        "source.groupId",
                        "source.technicalBaselineId",
                        "source.architectureArtifactContractId",
                        "source.taskPlanRunId",
                        "task",
                        "sourceContext.technicalBaseline",
                        "sourceContext.architectureArtifactProjection",
                        "sourceContext.acceptanceSnapshot",
                        "sourceContext.requirementDetailSnapshot",
                        "sourceContext.userFacingLanguage",
                        "executionRules"
                    ]
                },
                {
                    "groupId": "task_execution_result_contract",
                    "required": true,
                    "purpose": "Read TaskResult output file, schema, enum values, and completion barrier.",
                    "whenToRead": "Read before writing TaskResult.",
                    "fields": [
                        "enumRefs",
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
                        "outputContract.schemaShape.properties.frontendExperienceSelfCheck",
                        "outputContract.schemaShape.properties.runtimeDeliveryEvidence",
                        "outputContract.schemaShape.properties.requirementDetailEvidence",
                        "outputContract.schemaShape.properties.conceptEvidence",
                        "outputContract.schemaShape.properties.blockedReasons",
                        "outputContract.resultRules",
                        "blockedOutput",
                        "executionRules.completionBarrier",
                        "executionRules.finalResponseGuard",
                        "executionRules.completionContinuityRequirement",
                        "executionRules.verificationCommandSchedulingRules"
                    ]
                },
                {
                    "groupId": "task_execution_optional_refs",
                    "required": false,
                    "purpose": "Read source refs and concept grounding only when implementation needs more source detail.",
                    "whenToRead": "Read on demand.",
                    "fields": [
                        "sourceRefs",
                        "taskConceptGrounding",
                        "sourceContext.dependencyResults"
                    ]
                }
            ]
        }
    }))
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
