use std::{collections::BTreeSet, path::Path};

use contracts::{
    ManualReviewResolution, ReviewFinding, ReviewResult, TaskPlan, TaskPlanRun, TaskPlanRunStatus,
    TaskResult,
};
use delivery_core::{
    apply_delivery_index, ArtifactKind, DomainDispatcher, FileSubmitInput, LoomMcpActionResult,
    LoomMcpAutoRunnableResult, LoomMcpFailure, LoomMcpFailureResult, LoomMcpNextAction,
    LoomMcpRepairableErrorResult, LoomMcpUserGateResult, OperationContext, RouteAction,
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
    paths::{
        manual_review_request_file, manual_review_resolution_candidate_file,
        manual_review_resolution_file, review_latest_file, review_request_file,
        review_result_candidate_file, review_result_file, task_result_file,
    },
    task_execution::load_current_plan_and_run,
};

const REVIEW_ACTIONS: &[&str] = &[
    "done",
    "continue_to_next_phase",
    "execution_repair",
    "taskplan_repair",
    "architecture_artifact_repair",
    "manual_review",
    "needs_user_decision",
];

const REPAIR_ACTIONS: &[&str] = &[
    "execution_repair",
    "taskplan_repair",
    "architecture_artifact_repair",
    "manual_review",
    "needs_user_decision",
];

pub fn materialize_review_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match materialize_review_request_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => failed(
            project_root,
            "REVIEW_REQUEST_FAILED",
            error.to_string(),
            "review",
        ),
    }
}

fn materialize_review_request_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let (task_plan, run) = load_current_plan_and_run(root, &locator)?;
    if !matches!(
        run.status,
        TaskPlanRunStatus::Completed
            | TaskPlanRunStatus::CompletedWithNotes
            | TaskPlanRunStatus::Blocked
            | TaskPlanRunStatus::Failed
    ) {
        return Ok(failed(
            project_root,
            "TASKPLAN_RUN_NOT_TERMINAL",
            "ReviewRequest requires a terminal TaskPlanRun.".to_string(),
            "review",
        ));
    }
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
    if let Some(existing_request_ref) = phase.latest_refs.get("reviewRequestRef").cloned() {
        if state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        })
        .map(|request| request.request_kind == "review_request")
        .unwrap_or(false)
        {
            return write_review_result(project_root, &existing_request_ref);
        }
    }

    let review_id = format!("review_{}", state::store::now_millis());
    let result_file = to_project_relative(root, &review_result_candidate_file(root, &review_id))?;
    let request_file = to_project_relative(root, &review_request_file(root, &locator, &review_id))?;
    let task_results = load_task_results(root, &locator, &task_plan, &run);
    let request_root = build_review_request(
        &review_id,
        delivery_id,
        phase_id,
        &result_file,
        &task_plan,
        &run,
        &task_results,
    )?;
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: review_id.clone(),
            request_kind: "review_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    if let Some(parent) = from_project_relative(root, &result_file)?.parent() {
        state::store::ensure_dir(parent)?;
    }
    if let Some(active_phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        active_phase
            .latest_refs
            .insert("reviewRequestId".to_string(), review_id);
        active_phase
            .latest_refs
            .insert("reviewRequestRef".to_string(), stored.request_ref.clone());
        active_phase.next_action = Some(RouteAction {
            kind: RouteActionKind::Review,
            source: "review_request".to_string(),
            reason: "review_request_active".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(stored.request_ref.clone()),
            details: None,
            target_phase_id: None,
        });
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    write_review_result(project_root, &stored.request_ref)
}

fn build_review_request(
    review_id: &str,
    delivery_id: &str,
    phase_id: &str,
    result_file: &str,
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    task_results: &[TaskResult],
) -> Result<Value, state::store::StateError> {
    let schema_shape = serde_json::to_value(schema_for!(ReviewResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let allowed_refs = allowed_refs(task_plan, run, task_results);
    Ok(json!({
        "schemaVersion": "1.0",
        "requestType": "review_gate",
        "requestId": review_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::ReviewResult,
        "source": {
            "phaseId": phase_id,
            "taskPlanId": task_plan.task_plan_id,
            "taskPlanRunId": run.run_id,
            "architectureArtifactContractId": task_plan.source.architecture_artifact_contract_id,
            "technicalBaselineId": task_plan.source.technical_baseline_id
        },
        "sourceRefs": {
            "taskPlanRef": format!(".loom/deliveries/{delivery_id}/tasks/{phase_id}/taskplans/{}.json", task_plan.task_plan_id),
            "taskPlanRunRef": format!(".loom/deliveries/{delivery_id}/tasks/{phase_id}/runs/{}.json", run.run_id)
        },
        "reviewScope": {
            "type": "phase_run",
            "groupIds": run.group_states.iter().map(|state| state.group_id.clone()).collect::<Vec<_>>(),
            "acceptanceRefs": task_plan.scope_snapshot.acceptance_refs,
            "runStatus": run.status,
            "runSummary": run.summary
        },
        "reviewPacket": {
            "taskPlanId": task_plan.task_plan_id,
            "taskPlanRunId": run.run_id,
            "groups": task_plan.groups,
            "tasks": task_plan.tasks,
            "taskResults": task_results
        },
        "changeContext": {
            "mode": "current_file_content",
            "changedFiles": task_results.iter()
                .flat_map(|result| result.changed_files.iter().map(|file| json!({
                    "path": file,
                    "source": result.task_result_id
                })))
                .collect::<Vec<_>>()
        },
        "conceptReviewMatrix": build_concept_review_matrix(task_plan, task_results),
        "detailReviewMatrix": build_detail_review_matrix(task_plan, task_results),
        "reviewSignals": build_review_signals(task_plan, task_results),
        "enumRefs": {
            "decision": ["approved", "approved_with_notes", "changes_requested", "blocked", "needs_user_decision"],
            "findingSeverity": ["critical", "major", "minor", "note"],
            "severityClass": ["blocking", "warning", "info"],
            "evidenceKind": ["code", "test", "runtime", "manual", "contract", "review_limitation"],
            "failureClass": ["product_defect", "environment_blocker", "review_limitation", "contract_gap"],
            "findingCategory": [
                "functional_correctness",
                "integration_risk",
                "test_gap",
                "evidence_insufficient",
                "acceptance_not_satisfied",
                "frontend_experience",
                "architecture_design_gap",
                "task_scope_mismatch",
                "task_verification_mapping_issue",
                "environment_or_dependency",
                "review_limitation"
            ],
            "nextAction": REVIEW_ACTIONS,
            "readRefType": ["review_packet", "change_context", "changed_file", "task_result", "verification_evidence"],
            "evidenceRefType": ["task_result", "verification_result", "changed_file", "manual_note"]
        },
        "reviewRules": {
            "commonRules": [
                "Read reviewPacket, changeContext, review matrices, reviewSignals, and outputContract before writing ReviewResult.",
                "Every finding must include non-empty readRefs.",
                "Do not modify project files during review.",
                "Do not convert environment blockers into execution_repair unless another product defect finding justifies execution repair.",
                "Do not approve when reviewSignals contains unsatisfied requirement detail evidence or frontend workflow closure."
            ],
            "routingRules": {
                "actionPriority": [
                    "needs_user_decision",
                    "manual_review",
                    "architecture_artifact_repair",
                    "taskplan_repair",
                    "execution_repair",
                    "continue_to_next_phase",
                    "done"
                ]
            }
        },
        "outputContract": {
            "artifactKind": ArtifactKind::ReviewResult,
            "writeMode": "single_json",
            "submitTool": "loom.reviewAcceptFile",
            "resultFile": result_file,
            "writeTargets": [{
                "targetId": "result",
                "path": result_file,
                "required": true,
                "description": "Write the ReviewResult JSON for this phase run."
            }],
            "schemaShape": schema_shape,
            "allowedRefs": allowed_refs,
            "requiredFields": ["reviewId", "source", "decision", "findings", "coverageAssessment", "limitations", "pendingActions", "nextAction"],
            "reviewSignals": build_review_signals(task_plan, task_results),
            "routingRules": {
                "topLevelNextActionPriority": [
                    "needs_user_decision",
                    "manual_review",
                    "architecture_artifact_repair",
                    "taskplan_repair",
                    "execution_repair",
                    "continue_to_next_phase",
                    "done"
                ]
            }
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "review_scope",
                    "required": true,
                    "purpose": "Read review source identity and phase run scope.",
                    "whenToRead": "Read first.",
                    "fields": ["source", "reviewScope", "sourceRefs"]
                },
                {
                    "groupId": "review_packets",
                    "required": true,
                    "purpose": "Read task plan and task results.",
                    "whenToRead": "Read before judging implementation quality.",
                    "fields": ["reviewPacket"]
                },
                {
                    "groupId": "change_context",
                    "required": true,
                    "purpose": "Read changed file context.",
                    "whenToRead": "Read before judging implementation quality.",
                    "fields": ["changeContext"]
                },
                {
                    "groupId": "review_matrices",
                    "required": true,
                    "purpose": "Read concept, requirement detail, frontend, and runtime review signals.",
                    "whenToRead": "Read before deciding approval or repair route.",
                    "fields": ["conceptReviewMatrix", "detailReviewMatrix", "reviewSignals", "outputContract.reviewSignals"]
                },
                {
                    "groupId": "review_rules",
                    "required": true,
                    "purpose": "Read review enums and routing rules.",
                    "whenToRead": "Read before writing findings and nextAction.",
                    "fields": ["enumRefs", "reviewRules.commonRules", "reviewRules.routingRules", "outputContract.routingRules"]
                },
                {
                    "groupId": "review_write_contract",
                    "required": true,
                    "purpose": "Read ReviewResult output path and precise schema fields.",
                    "whenToRead": "Read before writing ReviewResult.",
                    "fields": [
                        "outputContract.resultFile",
                        "outputContract.writeTargets",
                        "outputContract.allowedRefs",
                        "outputContract.requiredFields",
                        "outputContract.schemaShape.properties.source",
                        "outputContract.schemaShape.properties.decision",
                        "outputContract.schemaShape.properties.findings",
                        "outputContract.schemaShape.properties.coverageAssessment",
                        "outputContract.schemaShape.properties.limitations",
                        "outputContract.schemaShape.properties.pendingActions",
                        "outputContract.schemaShape.properties.nextAction"
                    ]
                }
            ]
        }
    }))
}

pub fn accept_review_result_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    match accept_review_result_file_inner(input, authorized, dispatcher) {
        Ok(result) => result,
        Err(error) => failed(
            &input.project_root,
            "REVIEW_ACCEPT_FAILED",
            error.to_string(),
            "review_accept",
        ),
    }
}

pub fn accept_manual_review_resolution_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    match accept_manual_review_resolution_file_inner(input, authorized, dispatcher) {
        Ok(result) => result,
        Err(error) => failed(
            &input.project_root,
            "MANUAL_REVIEW_RESOLVE_FAILED",
            error.to_string(),
            "manual_review_resolve",
        ),
    }
}

fn accept_review_result_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    let target = authorized.targets.first().ok_or_else(|| {
        state::store::StateError::InvalidArgument("ReviewResult target is missing".to_string())
    })?;
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing deliveryId".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing phaseId".to_string())
    })?;
    if let Some(stale) = ensure_latest_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
        "reviewRequestRef",
        "review_accept",
    )? {
        return Ok(stale);
    }
    let root = Path::new(&input.project_root);
    let raw = state::store::read_json_value(&from_project_relative(root, &target.path)?)?;
    let result: ReviewResult = match serde_json::from_value(raw) {
        Ok(result) => result,
        Err(error) => {
            return repairable_or_fallback_manual_review(
                input,
                authorized,
                target.path.clone(),
                vec![issue(
                    "REVIEW_RESULT_SCHEMA_INVALID",
                    "$",
                    &format!("ReviewResult JSON has an invalid schema: {error}"),
                )],
            )
        }
    };
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec![
            "source".to_string(),
            "outputContract.allowedRefs".to_string(),
            "reviewSignals".to_string(),
        ],
    })?
    .fields;
    let issues = validate_review_result(&result, &authorized.request_id, &fields);
    if !issues.is_empty() {
        return repairable_or_fallback_manual_review(
            input,
            authorized,
            target.path.clone(),
            issues,
        );
    }
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let persisted = review_result_file(root, &locator, &result.review_id);
    state::store::write_json_atomic(&persisted, &result)?;
    let result_ref = to_project_relative(root, &persisted)?;
    state::store::write_json_atomic(
        &review_latest_file(root, &locator),
        &json!({
            "schemaVersion": "1.0",
            "reviewId": result.review_id,
            "reviewResultRef": result_ref,
            "updatedAt": state::store::now_string()
        }),
    )?;
    update_delivery_after_review(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &result,
        &result_ref,
    )?;
    route_after_review(
        input,
        authorized,
        &delivery_id,
        &phase_id,
        &result,
        result_ref,
        dispatcher,
    )
}

fn validate_review_result(
    result: &ReviewResult,
    request_id: &str,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let source = fields.get("source").map(|field| &field.value);
    if result.source.request_id != request_id
        || source
            .and_then(|value| value.get("phaseId"))
            .and_then(Value::as_str)
            != Some(result.source.phase_id.as_str())
        || source
            .and_then(|value| value.get("taskPlanId"))
            .and_then(Value::as_str)
            != Some(result.source.task_plan_id.as_str())
        || source
            .and_then(|value| value.get("taskPlanRunId"))
            .and_then(Value::as_str)
            != Some(result.source.task_plan_run_id.as_str())
    {
        issues.push(issue(
            "REVIEW_RESULT_REF_INVALID",
            "source",
            "ReviewResult source must match the active ReviewRequest.",
        ));
    }
    validate_review_enums(result, &mut issues);
    let allowed = fields
        .get("outputContract.allowedRefs")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    validate_review_refs(result, &allowed, &mut issues);
    validate_review_decision(result, &mut issues);
    validate_review_signals(result, fields, &mut issues);
    issues
}

fn validate_review_enums(result: &ReviewResult, issues: &mut Vec<delivery_core::RepairIssue>) {
    let decisions = [
        "approved",
        "approved_with_notes",
        "changes_requested",
        "blocked",
        "needs_user_decision",
    ];
    if !decisions.contains(&result.decision.as_str()) {
        issues.push(issue(
            "REVIEW_RESULT_ENUM_INVALID",
            "decision",
            "ReviewResult decision is not allowed.",
        ));
    }
    if !REVIEW_ACTIONS.contains(&result.next_action.r#type.as_str()) {
        issues.push(issue(
            "REVIEW_RESULT_ENUM_INVALID",
            "nextAction.type",
            "ReviewResult nextAction.type is not allowed.",
        ));
    }
    for finding in &result.findings {
        if !["critical", "major", "minor", "note"].contains(&finding.severity.as_str()) {
            issues.push(issue(
                "REVIEW_RESULT_ENUM_INVALID",
                "findings[].severity",
                "Review finding severity is not allowed.",
            ));
        }
        if !REVIEW_ACTIONS.contains(&finding.recommended_next_action.as_str()) {
            issues.push(issue(
                "REVIEW_RESULT_ENUM_INVALID",
                "findings[].recommendedNextAction",
                "Review finding recommendedNextAction is not allowed.",
            ));
        }
    }
}

fn validate_review_refs(
    result: &ReviewResult,
    allowed: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let task_ids = allowed_set(allowed, "taskIds");
    let group_ids = allowed_set(allowed, "groupIds");
    let acceptance_refs = allowed_set(allowed, "acceptanceRefs");
    let task_result_ids = allowed_set(allowed, "taskResultIds");
    let read_refs = allowed_set(allowed, "readRefs");
    let finding_ids = result
        .findings
        .iter()
        .map(|finding| finding.finding_id.clone())
        .collect::<BTreeSet<_>>();
    for finding in &result.findings {
        if finding.read_refs.is_empty() {
            issues.push(issue(
                "REVIEW_RESULT_REF_INVALID",
                "findings[].readRefs",
                "Every finding must include readRefs.",
            ));
        }
        for task_ref in &finding.task_refs {
            if !task_ids.contains(task_ref) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].taskRefs",
                    "Review finding taskRefs must use allowed task ids.",
                ));
            }
        }
        for group_ref in &finding.group_refs {
            if !group_ids.contains(group_ref) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].groupRefs",
                    "Review finding groupRefs must use allowed group ids.",
                ));
            }
        }
        for acceptance_ref in &finding.acceptance_refs {
            if !acceptance_refs.contains(acceptance_ref) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].acceptanceRefs",
                    "Review finding acceptanceRefs must use current phase acceptance refs.",
                ));
            }
        }
        for read_ref in &finding.read_refs {
            if !read_refs.contains(&read_ref.r#ref) && !task_result_ids.contains(&read_ref.r#ref) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].readRefs",
                    "Review finding readRefs must use allowed request refs, task result ids, changed file refs, or verification refs.",
                ));
            }
        }
        if finding.failure_class.as_deref() == Some("environment_blocker")
            && finding.recommended_next_action == "execution_repair"
            && !result
                .findings
                .iter()
                .any(|other| other.failure_class.as_deref() == Some("product_defect"))
        {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "findings[].failureClass",
                "Environment blockers cannot route to execution_repair without a separate product defect finding.",
            ));
        }
    }
    for action in &result.pending_actions {
        for finding_ref in &action.finding_refs {
            if !finding_ids.contains(finding_ref) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "pendingActions[].findingRefs",
                    "pendingActions findingRefs must reference current findings.",
                ));
            }
        }
        if action.r#type == result.next_action.r#type {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "pendingActions[].type",
                "pendingActions must not duplicate top-level nextAction.",
            ));
        }
    }
    for finding_ref in &result.next_action.finding_refs {
        if !finding_ids.contains(finding_ref) {
            issues.push(issue(
                "REVIEW_RESULT_REF_INVALID",
                "nextAction.findingRefs",
                "nextAction findingRefs must reference current findings.",
            ));
        }
    }
}

fn validate_review_decision(result: &ReviewResult, issues: &mut Vec<delivery_core::RepairIssue>) {
    let has_blocking = result.findings.iter().any(is_blocking_finding);
    let has_execution = result.findings.iter().any(|finding| {
        finding.recommended_next_action == "execution_repair" && is_blocking_finding(finding)
    });
    let has_blocked_route = result.findings.iter().any(|finding| {
        [
            "architecture_artifact_repair",
            "taskplan_repair",
            "manual_review",
        ]
        .contains(&finding.recommended_next_action.as_str())
            && is_blocking_finding(finding)
    });
    let has_user_decision = result.findings.iter().any(|finding| {
        finding.recommended_next_action == "needs_user_decision" && is_blocking_finding(finding)
    });
    match result.decision.as_str() {
        "approved" | "approved_with_notes"
            if has_blocking || !result.pending_actions.is_empty() =>
        {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "decision",
                "Approved ReviewResult cannot contain blocking findings or pending actions.",
            ));
        }
        "changes_requested" if !has_execution => issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "decision",
            "changes_requested requires a blocking execution_repair finding.",
        )),
        "blocked" if !has_blocked_route => issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "decision",
            "blocked requires a blocking architecture, taskplan, or manual_review finding.",
        )),
        "needs_user_decision" if !has_user_decision => issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "decision",
            "needs_user_decision requires a blocking user decision finding.",
        )),
        _ => {}
    }
    let expected = expected_top_action(result);
    if result.next_action.r#type != expected {
        issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "nextAction.type",
            "ReviewResult nextAction.type does not match routing priority.",
        ));
    }
    if result.next_action.r#type == "continue_to_next_phase"
        && result
            .next_action
            .target_phase_id
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    {
        issues.push(issue(
            "REVIEW_RESULT_REF_INVALID",
            "nextAction.targetPhaseId",
            "continue_to_next_phase requires nextAction.targetPhaseId.",
        ));
    }
}

fn validate_review_signals(
    result: &ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let signals = fields
        .get("reviewSignals")
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null);
    let unsatisfied_detail = signals
        .get("requirementDetailEvidence")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| item.get("detailSatisfied").and_then(Value::as_bool) == Some(false));
    let unsatisfied_frontend = signals
        .get("frontendWorkflowClosure")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| item.get("closureSatisfied").and_then(Value::as_bool) == Some(false));
    if matches!(result.decision.as_str(), "approved" | "approved_with_notes")
        && (unsatisfied_detail || unsatisfied_frontend)
    {
        issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "decision",
            "ReviewResult cannot approve when reviewSignals contain unsatisfied requirement detail or frontend workflow closure.",
        ));
    }
}

fn route_after_review<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    delivery_id: &str,
    phase_id: &str,
    result: &ReviewResult,
    result_ref: String,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    let next_action = review_route_action(result, &result_ref);
    if matches!(
        next_action.kind,
        RouteActionKind::ManualReview | RouteActionKind::NeedsUserDecision
    ) {
        return materialize_manual_review_request(input, authorized, result, result_ref);
    }
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
                delivery_id: delivery_id.to_string(),
                phase_id: phase_id.to_string(),
                source_tool: "loom.reviewAcceptFile".to_string(),
                accepted_artifact_ref: result_ref,
                next_action: Some(next_action),
            },
        )
        .map_err(to_state_error)
}

fn review_route_action(result: &ReviewResult, result_ref: &str) -> RouteAction {
    RouteAction {
        kind: route_kind_for_review_action(&result.next_action.r#type),
        source: "review_result".to_string(),
        reason: result.next_action.reason.clone(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(result_ref.to_string()),
        details: Some(json!({
            "reviewId": result.review_id,
            "decision": result.decision,
            "nextAction": result.next_action
        })),
        target_phase_id: result.next_action.target_phase_id.clone(),
    }
}

fn materialize_manual_review_request(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    result: &ReviewResult,
    result_ref: String,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing deliveryId".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing phaseId".to_string())
    })?;
    let root = Path::new(&input.project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let request_id = format!(
        "manual_review_{}_{}",
        safe_id(&result.review_id),
        state::store::now_millis()
    );
    let result_file = to_project_relative(
        root,
        &manual_review_resolution_candidate_file(root, &request_id),
    )?;
    let request_file = to_project_relative(
        root,
        &manual_review_request_file(root, &locator, &request_id),
    )?;
    let request_root = build_manual_review_request(
        &request_id,
        &delivery_id,
        &phase_id,
        &result_file,
        result,
        &result_ref,
    );
    let stored = state::write_native_request(
        &input.project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "manual_review_resolution_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.clone()),
            phase_id: Some(phase_id.clone()),
            root: request_root,
        },
    )?;
    if let Some(parent) = from_project_relative(root, &result_file)?.parent() {
        state::store::ensure_dir(parent)?;
    }
    update_delivery_after_manual_review_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &stored.request_ref,
        result,
        &result_ref,
    )?;
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: input.project_root.clone(),
        request_ref: stored.request_ref.clone(),
    })?;
    Ok(LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
        project_root: input.project_root.clone(),
        prompt: "Review requires user decision. Reply approve_override to continue with notes, or request_changes with the repair route and change summary.".to_string(),
        accepted_responses: vec![
            "approve_override".to_string(),
            "request_changes".to_string(),
        ],
        request_ref: Some(stored.request_ref),
        delivery_id: Some(delivery_id),
        phase_id: Some(phase_id),
        gate: Some(json!({
            "gateId": format!("manual_review_{}", result.review_id),
            "kind": "manual_review",
            "reviewResultRef": result_ref,
            "writeTargets": inspected.write_targets,
            "readGroups": inspected.read_groups,
            "submitTool": "loom.reviewResolveFile",
            "visibleReason": {
                "decision": result.decision,
                "nextAction": result.next_action,
                "blockingFindings": result.findings.iter()
                    .filter(|finding| is_blocking_finding(finding))
                    .map(|finding| json!({
                        "findingId": finding.finding_id,
                        "summary": finding.summary,
                        "recommendedNextAction": finding.recommended_next_action
                    }))
                    .collect::<Vec<_>>()
            }
        })),
    }))
}

fn build_manual_review_request(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    result_file: &str,
    result: &ReviewResult,
    result_ref: &str,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(ManualReviewResolution))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    json!({
        "schemaVersion": "1.0",
        "requestType": "manual_review_resolution",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "artifactKind": ArtifactKind::ManualReviewResolution,
        "source": {
            "reviewId": result.review_id,
            "reviewResultRef": result_ref,
            "decision": result.decision,
            "reviewNextAction": result.next_action,
            "blockingFindings": result.findings.iter()
                .filter(|finding| is_blocking_finding(finding))
                .map(|finding| json!({
                    "findingId": finding.finding_id,
                    "summary": finding.summary,
                    "recommendedNextAction": finding.recommended_next_action
                }))
                .collect::<Vec<_>>()
        },
        "manualReviewProtocol": {
            "acceptedDecisions": ["approve_override", "request_changes"],
            "approveOverrideRule": "Use approve_override only when the user explicitly accepts the review issue as non-blocking; nextAction.type must be done or continue_to_next_phase.",
            "requestChangesRule": "Use request_changes when the user asks for changes; changeRequest.route must be execution_repair, taskplan_repair, architecture_artifact_repair, or needs_user_decision.",
            "routeRules": {
                "execution_repair": "Use for code, test, local verification, or project configuration changes.",
                "taskplan_repair": "Use for task structure, task order, task coverage, or task reference problems.",
                "architecture_artifact_repair": "Use for AAC design facts, interfaces, data model, state, runtime, or coverage problems.",
                "needs_user_decision": "Use for scope, acceptance, external environment, credential, network, policy, or product decision blockers."
            }
        },
        "enumRefs": {
            "decision": ["approve_override", "request_changes"],
            "changeRequestRoute": ["execution_repair", "taskplan_repair", "architecture_artifact_repair", "needs_user_decision"],
            "nextActionType": ["done", "continue_to_next_phase", "execution_repair", "taskplan_repair", "architecture_artifact_repair", "needs_user_decision"]
        },
        "outputContract": {
            "artifactKind": ArtifactKind::ManualReviewResolution,
            "writeMode": "single_json",
            "submitTool": "loom.reviewResolveFile",
            "resultFile": result_file,
            "writeTargets": [{
                "targetId": "resolution",
                "path": result_file,
                "required": true,
                "description": "Write the ManualReviewResolution JSON after the user answers the review gate."
            }],
            "requiredFields": [
                "schemaVersion", "manualReviewResolutionId", "manualReviewRequestId",
                "deliveryId", "phaseId", "userAnswer", "decision", "changeRequest",
                "nextAction", "createdAt"
            ],
            "schemaShape": schema_shape
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "manual_review_context",
                    "required": true,
                    "purpose": "Read the review issue and allowed user decision protocol.",
                    "whenToRead": "Read after the user answers the manual review gate.",
                    "fields": ["source", "manualReviewProtocol", "enumRefs"]
                },
                {
                    "groupId": "manual_review_write_contract",
                    "required": true,
                    "purpose": "Read the authorized resolution output path and schema fields.",
                    "whenToRead": "Read before writing ManualReviewResolution.",
                    "fields": [
                        "outputContract.resultFile",
                        "outputContract.writeTargets",
                        "outputContract.requiredFields",
                        "outputContract.schemaShape.properties.manualReviewRequestId",
                        "outputContract.schemaShape.properties.decision",
                        "outputContract.schemaShape.properties.changeRequest",
                        "outputContract.schemaShape.properties.nextAction"
                    ]
                }
            ]
        }
    })
}

fn accept_manual_review_resolution_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    let target = authorized.targets.first().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "ManualReviewResolution target is missing".to_string(),
        )
    })?;
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "ManualReviewResolution request missing deliveryId".to_string(),
        )
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "ManualReviewResolution request missing phaseId".to_string(),
        )
    })?;
    if let Some(stale) = ensure_latest_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
        "manualReviewRequestRef",
        "manual_review_resolve",
    )? {
        return Ok(stale);
    }
    let root = Path::new(&input.project_root);
    let raw = state::store::read_json_value(&from_project_relative(root, &target.path)?)?;
    let resolution: ManualReviewResolution = match serde_json::from_value(raw) {
        Ok(resolution) => resolution,
        Err(error) => {
            return Ok(repairable_with_tool(
                input,
                authorized,
                target.path.clone(),
                vec![issue(
                    "MANUAL_REVIEW_RESOLUTION_SCHEMA_INVALID",
                    "$",
                    &format!("ManualReviewResolution JSON has an invalid schema: {error}"),
                )],
                "loom.reviewResolveFile",
                "manual_review_resolution_candidate_only",
            ))
        }
    };
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec!["source".to_string(), "enumRefs".to_string()],
    })?
    .fields;
    let issues = validate_manual_review_resolution(&resolution, authorized, &fields);
    if !issues.is_empty() {
        return Ok(repairable_with_tool(
            input,
            authorized,
            target.path.clone(),
            issues,
            "loom.reviewResolveFile",
            "manual_review_resolution_candidate_only",
        ));
    }
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let persisted =
        manual_review_resolution_file(root, &locator, &resolution.manual_review_resolution_id);
    state::store::write_json_atomic(&persisted, &resolution)?;
    let resolution_ref = to_project_relative(root, &persisted)?;
    let effective_action = effective_manual_review_action(&resolution);
    update_delivery_after_manual_review_resolution(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &resolution,
        &resolution_ref,
        &effective_action,
    )?;
    route_after_manual_review(
        input,
        &resolution,
        resolution_ref,
        effective_action,
        dispatcher,
    )
}

fn validate_manual_review_resolution(
    resolution: &ManualReviewResolution,
    authorized: &AuthorizedWriteSet,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if resolution.manual_review_request_id != authorized.request_id {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_REF_INVALID",
            "manualReviewRequestId",
            "ManualReviewResolution must reference the active ManualReview requestId.",
        ));
    }
    if authorized.delivery_id.as_deref() != Some(resolution.delivery_id.as_str())
        || authorized.phase_id.as_deref() != Some(resolution.phase_id.as_str())
    {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_REF_INVALID",
            "deliveryId",
            "ManualReviewResolution deliveryId and phaseId must match the active request.",
        ));
    }
    match resolution.decision.as_str() {
        "approve_override" => {
            if resolution.change_request.is_some() {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "changeRequest",
                    "approve_override requires changeRequest to be omitted or null.",
                ));
            }
            if !matches!(
                resolution.next_action.r#type.as_str(),
                "done" | "continue_to_next_phase"
            ) {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "nextAction.type",
                    "approve_override can only route to done or continue_to_next_phase.",
                ));
            }
        }
        "request_changes" => {
            let Some(change) = resolution.change_request.as_ref() else {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "changeRequest",
                    "request_changes requires changeRequest.",
                ));
                return issues;
            };
            if ![
                "execution_repair",
                "taskplan_repair",
                "architecture_artifact_repair",
                "needs_user_decision",
            ]
            .contains(&change.route.as_str())
            {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_ENUM_INVALID",
                    "changeRequest.route",
                    "changeRequest.route is not allowed.",
                ));
            }
            if resolution.next_action.r#type != change.route {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "nextAction.type",
                    "request_changes nextAction.type must match changeRequest.route.",
                ));
            }
        }
        _ => issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_ENUM_INVALID",
            "decision",
            "ManualReviewResolution decision is not allowed.",
        )),
    }
    if fields
        .get("source")
        .and_then(|field| field.value.get("reviewId"))
        .and_then(Value::as_str)
        .is_none()
    {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_REF_INVALID",
            "source",
            "ManualReview request source must include the reviewId.",
        ));
    }
    issues
}

fn effective_manual_review_action(resolution: &ManualReviewResolution) -> RouteAction {
    let (kind, reason) = if resolution.decision == "approve_override" {
        (
            route_kind_for_review_action(&resolution.next_action.r#type),
            resolution.next_action.reason.clone(),
        )
    } else {
        let change = resolution
            .change_request
            .as_ref()
            .expect("validated request_changes has changeRequest");
        (
            route_kind_for_review_action(&change.route),
            format!("{}: {}", change.route, change.reason),
        )
    };
    RouteAction {
        kind,
        source: "manual_review_resolution".to_string(),
        reason,
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(resolution.manual_review_resolution_id.clone()),
        details: Some(json!({
            "manualReviewResolutionId": resolution.manual_review_resolution_id,
            "decision": resolution.decision,
            "changeRequest": resolution.change_request,
            "nextAction": resolution.next_action
        })),
        target_phase_id: resolution.next_action.target_phase_id.clone(),
    }
}

fn route_after_manual_review<D>(
    input: &FileSubmitInput,
    resolution: &ManualReviewResolution,
    resolution_ref: String,
    mut effective_action: RouteAction,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher,
{
    effective_action.request_ref = Some(resolution_ref.clone());
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
                delivery_id: resolution.delivery_id.clone(),
                phase_id: resolution.phase_id.clone(),
                source_tool: "loom.reviewResolveFile".to_string(),
                accepted_artifact_ref: resolution_ref,
                next_action: Some(effective_action),
            },
        )
        .map_err(to_state_error)
}

fn update_delivery_after_review(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    result: &ReviewResult,
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
            .insert("reviewResult".to_string(), result_ref.to_string());
        phase.next_action = Some(RouteAction {
            kind: route_kind_for_review_action(&result.next_action.r#type),
            source: "review_result".to_string(),
            reason: result.next_action.reason.clone(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(result_ref.to_string()),
            details: Some(json!({
                "reviewId": result.review_id,
                "decision": result.decision,
                "nextAction": result.next_action
            })),
            target_phase_id: result.next_action.target_phase_id.clone(),
        });
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)
}

fn update_delivery_after_manual_review_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    result: &ReviewResult,
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
        phase.latest_refs.insert(
            "manualReviewRequestRef".to_string(),
            request_ref.to_string(),
        );
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::ManualReview,
            source: "review_result".to_string(),
            reason: result.next_action.reason.clone(),
            prompt: Some("Review requires user decision before delivery can continue.".to_string()),
            accepted_responses: vec![
                "approve_override".to_string(),
                "request_changes".to_string(),
            ],
            request_ref: Some(request_ref.to_string()),
            details: Some(json!({
                "gateId": format!("manual_review_{}", result.review_id),
                "kind": "manual_review",
                "reviewResultRef": result_ref,
                "reviewId": result.review_id,
                "decision": result.decision,
                "nextAction": result.next_action
            })),
            target_phase_id: None,
        });
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)
}

fn update_delivery_after_manual_review_resolution(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    resolution: &ManualReviewResolution,
    resolution_ref: &str,
    effective_action: &RouteAction,
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
            "manualReviewResolution".to_string(),
            resolution_ref.to_string(),
        );
        phase.next_action = Some(RouteAction {
            request_ref: Some(resolution_ref.to_string()),
            ..effective_action.clone()
        });
        phase.latest_refs.insert(
            "manualReviewEffectiveDecision".to_string(),
            resolution.decision.clone(),
        );
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)?;
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    state::store::write_json_atomic(
        &review_latest_file(Path::new(project_root), &locator),
        &json!({
            "schemaVersion": "1.0",
            "latestResolutionRef": resolution_ref,
            "effectiveDecision": resolution.decision,
            "effectiveNextAction": effective_action,
            "updatedAt": state::store::now_string()
        }),
    )
}

fn write_review_result(
    project_root: &str,
    request_ref: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })?;
    let submit_tool = inspected.submit_tool.ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing submitTool".to_string())
    })?;
    let write_targets = inspected
        .write_targets
        .iter()
        .map(value_to_write_target)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
                artifact_kind: ArtifactKind::ReviewResult,
                request_ref: request_ref.to_string(),
                write_mode: WriteMode::SingleJson,
                write_targets,
                read_groups: inspected.read_groups,
                submit_tool,
            }),
        ),
    ))
}

fn load_task_results(
    root: &Path,
    locator: &DeliveryPhaseLocator,
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
) -> Vec<TaskResult> {
    run.task_states
        .iter()
        .filter_map(|state| {
            let task_id = &state.task_id;
            let result_id = state.result_id.as_ref()?;
            let task = task_plan
                .tasks
                .iter()
                .find(|task| task.task_id == *task_id)?;
            let path = task_result_file(root, locator, &run.run_id, &task.task_id, result_id);
            state::store::read_json(&path).ok()
        })
        .collect()
}

fn build_concept_review_matrix(task_plan: &TaskPlan, task_results: &[TaskResult]) -> Vec<Value> {
    let evidence_refs = task_results
        .iter()
        .flat_map(|result| {
            result
                .concept_evidence
                .iter()
                .map(|evidence| evidence.concept_ref.clone())
        })
        .collect::<BTreeSet<_>>();
    task_plan
        .tasks
        .iter()
        .flat_map(|task| {
            task.concept_refs.iter().map(|concept_ref| {
                json!({
                    "conceptRef": concept_ref,
                    "taskId": task.task_id,
                    "status": if evidence_refs.contains(concept_ref) { "satisfied" } else { "missing_evidence" },
                    "recommendedNextAction": if evidence_refs.contains(concept_ref) { "none" } else { "execution_repair" }
                })
            })
        })
        .collect()
}

fn build_detail_review_matrix(task_plan: &TaskPlan, task_results: &[TaskResult]) -> Vec<Value> {
    let satisfied = task_results
        .iter()
        .flat_map(|result| {
            result
                .requirement_detail_evidence
                .iter()
                .filter(|evidence| evidence.status == "satisfied")
                .map(|evidence| evidence.detail_id.clone())
        })
        .collect::<BTreeSet<_>>();
    task_plan
        .tasks
        .iter()
        .flat_map(|task| {
            task.requirement_detail_refs.iter().map(|detail_id| {
                let ok = satisfied.contains(detail_id);
                json!({
                    "detailId": detail_id,
                    "taskId": task.task_id,
                    "detailSatisfied": ok,
                    "recommendedNextAction": if ok { "none" } else { "execution_repair" }
                })
            })
        })
        .collect()
}

fn build_review_signals(task_plan: &TaskPlan, task_results: &[TaskResult]) -> Value {
    let detail = build_detail_review_matrix(task_plan, task_results);
    let frontend = task_results
        .iter()
        .filter_map(|result| result.frontend_experience_self_check.as_ref())
        .map(|check| {
            json!({
                "closureSatisfied": check.get("status").and_then(Value::as_str) == Some("satisfied"),
                "recommendedNextAction": if check.get("status").and_then(Value::as_str) == Some("satisfied") { "none" } else { "execution_repair" }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "requirementDetailEvidence": detail,
        "frontendWorkflowClosure": frontend
    })
}

fn allowed_refs(task_plan: &TaskPlan, run: &TaskPlanRun, task_results: &[TaskResult]) -> Value {
    let changed_files = task_results
        .iter()
        .flat_map(|result| result.changed_files.clone())
        .collect::<BTreeSet<_>>();
    let verification_refs = task_results
        .iter()
        .flat_map(|result| {
            result.verification_results.iter().flat_map(|verification| {
                [
                    verification.verification_id.clone(),
                    format!("{}:{}", result.task_result_id, verification.verification_id),
                    format!("{}:{}", result.task_id, verification.verification_id),
                ]
            })
        })
        .collect::<BTreeSet<_>>();
    let mut read_refs = vec!["reviewPacket".to_string(), "changeContext".to_string()];
    read_refs.extend(
        task_results
            .iter()
            .map(|result| result.task_result_id.clone()),
    );
    read_refs.extend(changed_files.iter().cloned());
    read_refs.extend(verification_refs.iter().cloned());
    json!({
        "taskIds": run.task_states.iter().map(|state| state.task_id.clone()).collect::<Vec<_>>(),
        "groupIds": run.group_states.iter().map(|state| state.group_id.clone()).collect::<Vec<_>>(),
        "acceptanceRefs": task_plan.scope_snapshot.acceptance_refs,
        "taskResultIds": task_results.iter().map(|result| result.task_result_id.clone()).collect::<Vec<_>>(),
        "changedFilePaths": changed_files.iter().cloned().collect::<Vec<_>>(),
        "verificationEvidenceRefs": verification_refs.iter().cloned().collect::<Vec<_>>(),
        "readRefs": read_refs
    })
}

fn expected_top_action(result: &ReviewResult) -> String {
    for action in REPAIR_ACTIONS {
        if result.findings.iter().any(|finding| {
            finding.recommended_next_action == *action && is_blocking_finding(finding)
        }) {
            return (*action).to_string();
        }
    }
    if (result.decision == "approved" || result.decision == "approved_with_notes")
        && result.next_action.r#type == "continue_to_next_phase"
    {
        "continue_to_next_phase".to_string()
    } else if result.decision == "approved" || result.decision == "approved_with_notes" {
        "done".to_string()
    } else {
        result.next_action.r#type.clone()
    }
}

fn is_blocking_finding(finding: &ReviewFinding) -> bool {
    finding.severity_class.as_deref() == Some("blocking")
        || matches!(finding.severity.as_str(), "critical" | "major")
}

fn route_kind_for_review_action(action: &str) -> RouteActionKind {
    match action {
        "execution_repair" => RouteActionKind::ExecutionRepair,
        "taskplan_repair" => RouteActionKind::TaskplanRepair,
        "architecture_artifact_repair" => RouteActionKind::ArchitectureArtifactRepair,
        "manual_review" => RouteActionKind::ManualReview,
        "needs_user_decision" => RouteActionKind::NeedsUserDecision,
        "continue_to_next_phase" => RouteActionKind::ContinueToNextPhase,
        "done" => RouteActionKind::Done,
        _ => RouteActionKind::Review,
    }
}

fn ensure_latest_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    latest_ref_key: &str,
    route_action: &str,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let latest = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .and_then(|phase| phase.latest_refs.get(latest_ref_key))
        .map(String::as_str);
    if latest != Some(request_ref) {
        return Ok(Some(failed(
            project_root,
            "STALE_REVIEW_REQUEST",
            "Review submit must use the active phase latest Review requestRef.".to_string(),
            route_action,
        )));
    }
    Ok(None)
}

fn allowed_set(value: &Value, key: &str) -> BTreeSet<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
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
            .unwrap_or("Write the requested artifact.")
            .to_string(),
    })
}

fn repairable_or_fallback_manual_review(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing deliveryId".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("Review request missing phaseId".to_string())
    })?;
    let attempts = increment_review_invalid_attempts(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &authorized.request_id,
        &issues,
    )?;
    if attempts < 3 {
        return Ok(repairable(input, authorized, target_file, issues));
    }
    let root = Path::new(&input.project_root);
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec!["source".to_string()],
    })?
    .fields;
    let source = fields
        .get("source")
        .map(|field| field.value.clone())
        .unwrap_or_else(|| json!({}));
    let now = state::store::now_string();
    let review_id = format!("fallback_review_{}", safe_id(&authorized.request_id));
    let fallback: ReviewResult = serde_json::from_value(json!({
        "schemaVersion": "1.0",
        "reviewId": review_id,
        "source": {
            "requestId": authorized.request_id,
            "phaseId": source.get("phaseId").and_then(Value::as_str).unwrap_or(&phase_id),
            "taskPlanId": source.get("taskPlanId").and_then(Value::as_str).unwrap_or("unknown_task_plan"),
            "taskPlanRunId": source.get("taskPlanRunId").and_then(Value::as_str).unwrap_or("unknown_task_plan_run")
        },
        "decision": "blocked",
        "findings": [{
            "findingId": "fallback-review-result-invalid",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "review_limitation",
            "failureClass": "review_limitation",
            "category": "review_limitation",
            "summary": "ReviewResult remained invalid after repeated repair attempts.",
            "evidence": "Rust validator received repeated invalid ReviewResult submissions.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Fallback is based on the active ReviewRequest context."}],
            "taskRelevance": "current_phase",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "unknown",
            "recommendedNextAction": "manual_review"
        }],
        "coverageAssessment": {
            "mustAcceptance": [],
            "summary": {
                "totalMust": 0,
                "satisfied": 0,
                "insufficientEvidence": 0,
                "notSatisfied": 0,
                "notReviewed": 0
            }
        },
        "limitations": [{
            "code": "fallback_review_result_invalid",
            "summary": "ReviewResult validation failed repeatedly.",
            "impact": "Manual review is required to avoid an automated repair loop."
        }],
        "pendingActions": [],
        "nextAction": {
            "type": "manual_review",
            "reason": "Repeated invalid ReviewResult submissions require manual review.",
            "findingRefs": ["fallback-review-result-invalid"]
        },
        "createdAt": now,
        "updatedAt": now
    }))
    .map_err(state::store::StateError::Json)?;
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let persisted = review_result_file(root, &locator, &fallback.review_id);
    state::store::write_json_atomic(&persisted, &fallback)?;
    let result_ref = to_project_relative(root, &persisted)?;
    state::store::write_json_atomic(
        &review_latest_file(root, &locator),
        &json!({
            "schemaVersion": "1.0",
            "reviewId": fallback.review_id,
            "reviewResultRef": result_ref,
            "fallbackReason": "repeated_invalid_review_result",
            "updatedAt": state::store::now_string()
        }),
    )?;
    update_delivery_after_review(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &fallback,
        &result_ref,
    )?;
    materialize_manual_review_request(input, authorized, &fallback, result_ref)
}

fn increment_review_invalid_attempts(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_id: &str,
    issues: &[delivery_core::RepairIssue],
) -> Result<u32, state::store::StateError> {
    let root = Path::new(project_root);
    let file = state::paths::delivery_dir(root, delivery_id)
        .join("reviews")
        .join(phase_id)
        .join("attempts")
        .join(format!("{}.json", safe_id(request_id)));
    let mut value = if file.exists() {
        state::store::read_json_value(&file)?
    } else {
        json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "attemptCount": 0,
            "issues": []
        })
    };
    let attempts = value
        .get("attemptCount")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        + 1;
    value["attemptCount"] = json!(attempts);
    value["updatedAt"] = json!(state::store::now_string());
    value["issues"] = serde_json::to_value(issues).unwrap_or_else(|_| json!([]));
    state::store::write_json_atomic(&file, &value)?;
    Ok(attempts as u32)
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
        "loom.reviewAcceptFile",
        "review_result_candidate_only",
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
            domain: Some("review".to_string()),
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

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}
