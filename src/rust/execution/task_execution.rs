use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

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
    let architecture_projection = task_scoped_architecture_projection(&aac, &request_task);
    let schema_shape = serde_json::to_value(schema_for!(contracts::TaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let read_groups = task_execution_read_groups(&request_task);
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
        "sourceContext": {
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
            "userFacingLanguage": &pgc.planning_inputs.user_facing_language,
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
                "Use the smallest meaningful verification signal that proves the current task's behavior or contract obligation.",
                "Do not leave long-running servers, watchers, browsers, or workers unreleased before TaskResult submission."
            ],
            "userFacingLanguage": {
                "constraint": pgc.planning_inputs.user_facing_language,
                "rule": "Preserve user-facing language from confirmed requirements in generated UI and feedback."
            },
            "boundaryRules": [
                "Execute only the current task.",
                "Do not modify Brainstorm, TechnicalBaseline, PGC, AAC, TaskPlan, or other protected Loom artifacts.",
                "Do not implement deferred scope.",
                "Use confirmed business language in user-visible UI, feedback, test names, and TaskResult evidence when applicable.",
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
            "resultTemplate": task_result_template(&task_plan.task_plan_id, &request_task),
            "resultRules": [
                "TaskResult must include every requiredTopLevelFields entry.",
                "If status is completed, every verification intent should have passed evidence.",
                "If status is failed, failure is required."
            ]
        },
        "requestReadPlan": { "groups": read_groups }
    }))
}

fn task_execution_read_groups(task: &TaskDefinition) -> Value {
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
    if task.frontend_experience_requirement.is_some() {
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
    if task.runtime_delivery_requirement.is_some() {
        core_fields.push("task.runtimeDeliveryRequirement");
        core_fields.push("sourceContext.architectureArtifactProjection.runtimeDelivery");
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
        "outputContract.schemaShape.properties.conceptEvidence",
        "outputContract.schemaShape.properties.blockedReasons",
        "outputContract.resultRules",
        "blockedOutput.blockedReasons",
        "executionRules.completionBarrier",
        "executionRules.finalResponseGuard",
        "executionRules.completionContinuityRequirement",
        "executionRules.verificationCommandSchedulingRules",
    ];
    if task.frontend_experience_requirement.is_some() {
        result_fields.push("outputContract.schemaShape.properties.frontendExperienceSelfCheck");
    }
    if task.runtime_delivery_requirement.is_some() {
        result_fields.push("outputContract.schemaShape.properties.runtimeDeliveryEvidence");
    }

    Value::Array(vec![
        json!({
            "groupId": "task_execution_core",
            "required": true,
            "purpose": "Read task identity, task-scoped architecture context, and execution rules before editing.",
            "whenToRead": "Read before any source edit.",
            "fields": core_fields
        }),
        json!({
            "groupId": "task_execution_result_contract",
            "required": true,
            "purpose": "Read TaskResult output file, schema fields, enum values, and completion barrier.",
            "whenToRead": "Read before writing TaskResult.",
            "fields": result_fields
        }),
        json!({
            "groupId": "task_execution_optional_refs",
            "required": false,
            "purpose": "Read source refs and dependency results only when task-scoped projection is insufficient.",
            "whenToRead": "Read on demand.",
            "fields": [
                "sourceRefs.technicalBaselineRef",
                "sourceRefs.planningGenerationContractRef",
                "sourceRefs.architectureArtifactContractRef",
                "sourceRefs.taskPlanRef",
                "sourceRefs.taskPlanRunRef",
                "sourceRefs.phaseConceptGroundingRef",
                "taskConceptGrounding.conceptRefs",
                "taskConceptGrounding.conceptResponsibilities",
                "taskConceptGrounding.conceptVerificationIntents",
                "sourceContext.dependencyResults"
            ]
        }),
    ])
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
    if task.runtime_delivery_requirement.is_some() {
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
                "readWhen": "No closure refs were derived for this task."
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
    json!({
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
            "sourcePaths": [
                "sourceRefs.architectureArtifactContractRef#/frontendExperience",
                "sourceRefs.architectureArtifactContractRef#/userFlows",
                "sourceRefs.architectureArtifactContractRef#/interfaces"
            ],
            "closureRequirementIds": closure_requirements.iter().filter_map(|item| string_at(item, "closureId")).collect::<Vec<_>>(),
            "readWhen": "Read these source paths only when closureRequirementRefs and frontendBackendBindings are insufficient.",
            "derivationRule": "Closure refs are derived from AAC frontendExperience surfaces or operationPaths, task userFlows, userFlow steps, and executable interfaces."
        },
        "guidanceWarnings": warnings
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
