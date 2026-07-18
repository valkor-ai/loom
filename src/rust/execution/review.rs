use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

use contracts::{
    ArchitectureArtifactContract, ManualReviewResolution, ReviewFinding, ReviewNextAction,
    ReviewResult, TaskDefinition, TaskPlan, TaskPlanGroup, TaskPlanRun, TaskPlanRunStatus,
    TaskResult,
};
use delivery_core::{
    apply_delivery_index, read_selectors_value_from_paths, ArtifactKind, DeliveryLifecycleStatus,
    DomainDispatcher, FileSubmitInput, LoomMcpActionResult, LoomMcpAutoRunnableResult,
    LoomMcpFailure, LoomMcpFailureResult, LoomMcpNextAction, LoomMcpRepairableErrorResult,
    LoomMcpUserGateResult, OperationContext, RouteAction, RouteActionKind, SubmitAcceptedEvent,
    TransitionEngine, TransitionStore, WriteArtifactNext, WriteMode, WriteTarget,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{delivery_dir, from_project_relative, to_project_relative, DeliveryPhaseLocator},
    write_targets::AuthorizedWriteSet,
};

use crate::{
    api_contract::{exposure_projection, interfaces_for_refs, load_project_api_contract},
    paths::{
        manual_review_request_file, manual_review_resolution_candidate_file,
        manual_review_resolution_file, review_latest_file, review_request_file,
        review_result_candidate_file, review_result_file, task_result_file,
    },
    task_execution::{load_current_plan_and_run, save_run},
    task_plan::update_run_summary,
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
    let task_results = load_task_results(root, &locator, &task_plan, &run);
    if let Some(existing_request_ref) = phase.latest_refs.get("reviewRequestRef").cloned() {
        if state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        })
        .map(|request| request.request_kind == "review_request")
        .unwrap_or(false)
            && review_request_matches_current_task_results(
                project_root,
                &existing_request_ref,
                &task_results,
            )
        {
            return write_review_result(project_root, &existing_request_ref);
        }
    }

    let review_id = format!("review_{}", state::store::now_millis());
    let result_file = to_project_relative(root, &review_result_candidate_file(root, &review_id))?;
    let request_file = to_project_relative(root, &review_request_file(root, &locator, &review_id))?;
    let architecture_contract = phase
        .latest_refs
        .get("architectureArtifact")
        .and_then(|architecture_ref| from_project_relative(root, architecture_ref).ok())
        .and_then(|architecture_path| {
            state::store::read_json::<ArchitectureArtifactContract>(&architecture_path).ok()
        });
    let project_api_contract = architecture_contract
        .as_ref()
        .map(|architecture| load_project_api_contract(root, architecture))
        .transpose()?
        .flatten();
    let next_phase_handoff =
        brainstorm::next_phase_handoff_from_preview(project_root, delivery_id, phase_id, None)?;
    let request_root = build_review_request(
        root,
        &review_id,
        delivery_id,
        phase_id,
        &result_file,
        &task_plan,
        &run,
        &task_results,
        architecture_contract.as_ref(),
        project_api_contract.as_ref(),
        next_phase_handoff.as_ref(),
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

fn review_request_matches_current_task_results(
    project_root: &str,
    request_ref: &str,
    task_results: &[TaskResult],
) -> bool {
    if task_results.is_empty() {
        return false;
    }
    let expected = Value::Array(compact_task_result_summaries(task_results));
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "reviewPacket.taskResultSummaries".to_string(),
            "reviewPacket.taskResultSnapshotFingerprint".to_string(),
        ],
    });
    let Ok(fields) = fields else {
        return false;
    };
    let summaries_match = fields
        .fields
        .get("reviewPacket.taskResultSummaries")
        .map(|field| field.value.clone())
        .is_some_and(|actual| actual == expected);
    let fingerprint_match = fields
        .fields
        .get("reviewPacket.taskResultSnapshotFingerprint")
        .and_then(|field| field.value.as_str())
        .is_some_and(|actual| actual == task_result_snapshot_fingerprint(task_results));
    summaries_match && fingerprint_match
}

fn task_result_snapshot_fingerprint(task_results: &[TaskResult]) -> String {
    let mut snapshots = task_results
        .iter()
        .filter_map(|result| {
            let mut value = serde_json::to_value(result).ok()?;
            if let Some(object) = value.as_object_mut() {
                object.remove("createdAt");
                object.remove("updatedAt");
            }
            Some(value)
        })
        .collect::<Vec<_>>();
    snapshots.sort_by(|left, right| {
        left.get("taskResultId")
            .and_then(Value::as_str)
            .cmp(&right.get("taskResultId").and_then(Value::as_str))
    });
    delivery_core::contract_fingerprint(&Value::Array(snapshots))
}

fn build_review_request(
    project_root: &Path,
    review_id: &str,
    delivery_id: &str,
    phase_id: &str,
    result_file: &str,
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    task_results: &[TaskResult],
    architecture_contract: Option<&ArchitectureArtifactContract>,
    project_api_contract: Option<&Value>,
    next_phase_handoff: Option<&brainstorm::NextPhaseHandoff>,
) -> Result<Value, state::store::StateError> {
    let schema_shape = review_result_schema_shape();
    let review_signals = build_review_signals(task_plan, run, task_results, architecture_contract);
    let next_phase_preview = review_next_phase_preview(next_phase_handoff);
    let (change_set, change_context) =
        build_change_set(project_root, delivery_id, phase_id, review_id, task_results)?;
    let allowed_refs = allowed_refs(task_plan, run, task_results, &change_context, &change_set);
    let change_context_mode = change_context
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("current_file_content")
        .to_string();
    let concept_review_matrix = build_concept_review_matrix(task_plan, task_results);
    let detail_review_matrix = build_detail_review_matrix(task_plan, task_results);
    let engineering_quality_review_matrix =
        build_engineering_quality_review_matrix(task_plan, task_results);
    let architecture_quality_review_matrix =
        build_architecture_quality_review_matrix(task_plan, task_results, architecture_contract);
    let api_contract_review_matrix = build_api_contract_review_matrix(task_plan, task_results);
    let code_quality_review_matrix = build_code_quality_review_matrix(task_plan, task_results);
    let frontend_quality_review_matrix =
        build_frontend_quality_review_matrix(task_plan, task_results, architecture_contract);
    let review_matrix_summary = compact_review_matrix_summary(
        &concept_review_matrix,
        &detail_review_matrix,
        &engineering_quality_review_matrix,
        &architecture_quality_review_matrix,
        &api_contract_review_matrix,
        &code_quality_review_matrix,
        &frontend_quality_review_matrix,
    );
    let mut root = json!({
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
            "taskIds": run.task_states.iter().map(|state| state.task_id.clone()).collect::<Vec<_>>(),
            "groupIds": run.group_states.iter().map(|state| state.group_id.clone()).collect::<Vec<_>>(),
            "acceptanceRefs": task_plan.scope_snapshot.acceptance_refs,
            "nextPhaseId": next_phase_preview.get("suggestedPhaseId").cloned().unwrap_or(Value::Null),
            "nextPhasePreview": next_phase_preview,
            "runStatus": run.status,
            "runSummary": run.summary
        },
        "reviewPacket": {
            "taskPlanId": task_plan.task_plan_id,
            "taskPlanRunId": run.run_id,
            "groupSummaries": compact_group_summaries(&task_plan.groups),
            "taskSummaries": compact_task_summaries(&task_plan.tasks),
            "taskResultSummaries": compact_task_result_summaries(task_results),
            "taskResultSnapshotFingerprint": task_result_snapshot_fingerprint(task_results),
            "apiContractContext": compact_api_contract_context(
                task_plan,
                architecture_contract,
                project_api_contract,
            )
        },
        "changeSet": change_set,
        "changeContext": change_context,
        "conceptReviewMatrix": concept_review_matrix,
        "detailReviewMatrix": detail_review_matrix,
        "engineeringQualityReviewMatrix": engineering_quality_review_matrix,
        "architectureQualityReviewMatrix": architecture_quality_review_matrix,
        "apiContractReviewMatrix": api_contract_review_matrix,
        "codeQualityReviewMatrix": code_quality_review_matrix,
        "frontendQualityReviewMatrix": frontend_quality_review_matrix,
        "reviewMatrixSummary": review_matrix_summary,
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
                "architecture_quality",
                "api_contract",
                "code_quality",
                "task_scope_mismatch",
                "task_verification_mapping_issue",
                "environment_or_dependency",
                "review_limitation"
            ],
            "nextAction": REVIEW_ACTIONS,
            "readRefType": ["review_packet", "change_context", "diff_ref", "changed_file", "task_result", "verification_evidence"],
            "evidenceRefType": ["task_result", "verification_result", "diff_ref", "changed_file", "manual_note"]
        },
        "reviewRules": {
            "commonRules": [
                "Read reviewPacket compact groupSummaries, taskSummaries, taskResultSummaries, changeContext, review matrices, outputContract.reviewSignals, and outputContract before writing ReviewResult.",
                "Review spec fidelity and project standards as separate axes; a clean implementation can still be wrong for the confirmed contract.",
                "When reviewMatrixSummary.codeQuality or a code_quality signal needs investigation, read the optional review_code_quality_context group. Use its task-scoped referenceLoadPlan and referenceGroups only; do not scan the full tech tree or substitute a different database provider reference.",
                "Every finding must include non-empty readRefs.",
                "Write finding observations and evidence only. Loom derives findingId, pendingActions.findingRefs, nextAction.findingRefs, nextAction.targetTaskIds, and approved phase linkage from the current review signals.",
                "Every blocking finding must describe the smallest repair that satisfies the current Loom contract.",
                "Do not modify project files during review.",
                "Use compact browser check status, attempts, command, and observed outcome first. Read a referenced Playwright trace, report, or screenshot only when a failed, blocked, retried, or ambiguous check cannot be judged from the compact evidence.",
                "Do not convert environment blockers into execution_repair unless another product defect finding justifies execution repair.",
                "Do not approve when outputContract.reviewSignals contains unsatisfied requirement detail evidence, engineering quality, architecture quality, API contract, code quality, frontend workflow closure, or frontend UI quality.",
                "If outputContract.reviewSignals contains frontend_workflow_closure with missingTaskAssignment=true, route taskplan_repair unless a higher-priority blocking finding applies.",
                "If outputContract.reviewSignals contains architecture_quality with missingTaskAssignment=true, route taskplan_repair unless a higher-priority blocking finding applies.",
                "Blocking findings must cite a task, group, artifact, or file location unless the route is manual_review or needs_user_decision."
            ],
            "changeSetRules": change_set_rules(&change_context_mode),
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
            "resultTemplate": review_result_template(task_plan, run, next_phase_handoff),
            "allowedRefs": allowed_refs,
            "requiredFields": ["decision", "findings", "coverageAssessment", "limitations", "pendingActions", "nextAction"],
            "reviewSignals": {
                "items": review_signals
            },
            "changeContextMode": change_context_mode.clone(),
            "severityPolicy": review_severity_policy(),
            "routingRules": {
                "topLevelNextActionPriority": [
                    "needs_user_decision",
                    "manual_review",
                    "architecture_artifact_repair",
                    "taskplan_repair",
                    "execution_repair",
                    "continue_to_next_phase",
                    "done"
                ],
                "manualReviewPriorityRule": "manual_review outranks automatic repair only for blocking review limitations or environment blockers that prevent reliable review."
            },
            "validatorRules": review_validator_rules(&change_context_mode)
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "review_scope",
                    "required": true,
                    "purpose": "Read review source identity and phase run scope.",
                    "whenToRead": "Read first.",
                    "selectors": read_selectors_value_from_paths([
                        "source.phaseId",
                        "source.taskPlanId",
                        "source.taskPlanRunId",
                        "source.architectureArtifactContractId",
                        "source.technicalBaselineId",
                        "sourceRefs.taskPlanRef",
                        "sourceRefs.taskPlanRunRef",
                        "reviewScope.type",
                        "reviewScope.taskIds",
                        "reviewScope.groupIds",
                        "reviewScope.acceptanceRefs",
                        "reviewScope.nextPhaseId",
                        "reviewScope.nextPhasePreview.kind",
                        "reviewScope.nextPhasePreview.suggestedPhaseId",
                        "reviewScope.nextPhasePreview.reason",
                        "reviewScope.runStatus",
                        "reviewScope.runSummary"
                    ])
                },
                {
                    "groupId": "review_packets",
                    "required": true,
                    "purpose": "Read task plan and task results.",
                    "whenToRead": "Read before judging implementation quality.",
                    "selectors": read_selectors_value_from_paths([
                        "reviewPacket.taskPlanId",
                        "reviewPacket.taskPlanRunId",
                        "reviewPacket.groupSummaries",
                        "reviewPacket.taskSummaries",
                        "reviewPacket.taskResultSummaries",
                        "reviewPacket.taskResultSnapshotFingerprint",
                        "reviewPacket.apiContractContext"
                    ])
                },
                {
                    "groupId": "change_context",
                    "required": true,
                    "purpose": "Read changed file context.",
                    "whenToRead": "Read before judging implementation quality.",
                    "selectors": read_selectors_value_from_paths([
                        "changeContext.mode",
                        "changeContext.changedFiles",
                        "outputContract.changeContextMode"
                    ])
                },
                {
                    "groupId": "review_matrices",
                    "required": true,
                    "purpose": "Read compact concept, requirement detail, engineering quality, architecture quality, API contract, code quality, frontend quality, and runtime review signals.",
                    "whenToRead": "Read before deciding approval or repair route.",
                    "selectors": read_selectors_value_from_paths([
                        "reviewMatrixSummary.concept",
                        "reviewMatrixSummary.detail",
                        "reviewMatrixSummary.engineeringQuality",
                        "reviewMatrixSummary.architectureQuality",
                        "reviewMatrixSummary.apiContract",
                        "reviewMatrixSummary.codeQuality",
                        "reviewMatrixSummary.frontendQuality",
                        "outputContract.reviewSignals.items"
                    ])
                },
                {
                    "groupId": "review_code_quality_context",
                    "required": false,
                    "purpose": "Read task-scoped language, framework, and database reference evidence only when code quality requires investigation.",
                    "whenToRead": "Read when reviewMatrixSummary.codeQuality or a code_quality review signal is unsatisfied or ambiguous.",
                    "selectors": read_selectors_value_from_paths([
                        "codeQualityReviewMatrix"
                    ])
                },
                review_quality_read_group(),
                {
                    "groupId": "review_rules",
                    "required": true,
                    "purpose": "Read review enums and routing rules.",
                    "whenToRead": "Read before writing findings and nextAction.",
                    "selectors": read_selectors_value_from_paths([
                        "enumRefs.decision",
                        "enumRefs.findingSeverity",
                        "enumRefs.severityClass",
                        "enumRefs.evidenceKind",
                        "enumRefs.failureClass",
                        "enumRefs.findingCategory",
                        "enumRefs.nextAction",
                        "enumRefs.readRefType",
                        "enumRefs.evidenceRefType",
                        "reviewRules.commonRules",
                        "reviewRules.changeSetRules",
                        "reviewRules.routingRules",
                        "outputContract.routingRules",
                        "outputContract.severityPolicy",
                        "outputContract.validatorRules"
                    ])
                },
                {
                    "groupId": "review_write_contract",
                    "required": true,
                    "purpose": "Read ReviewResult output path and precise schema fields.",
                    "whenToRead": "Read before writing ReviewResult.",
                    "selectors": read_selectors_value_from_paths([
                        "outputContract.resultFile",
                        "outputContract.writeTargets",
                        "outputContract.allowedRefs.taskIds",
                        "outputContract.allowedRefs.groupIds",
                        "outputContract.allowedRefs.acceptanceRefs",
                        "outputContract.allowedRefs.taskResultIds",
                        "outputContract.allowedRefs.changedFilePaths",
                        "outputContract.allowedRefs.diffRefs",
                        "outputContract.allowedRefs.verificationEvidenceRefs",
                        "outputContract.allowedRefs.readRefs",
                        "outputContract.requiredFields",
                        "outputContract.resultTemplate",
                        "outputContract.schemaShape.properties.decision",
                        "outputContract.schemaShape.properties.findings",
                        "outputContract.schemaShape.properties.coverageAssessment",
                        "outputContract.schemaShape.properties.limitations",
                        "outputContract.schemaShape.properties.pendingActions",
                        "outputContract.schemaShape.properties.nextAction"
                    ])
                }
            ]
        }
    });
    root["reviewQualityProfile"] = review_quality_profile();
    Ok(root)
}

fn review_quality_read_group() -> Value {
    json!({
        "groupId": "review_quality_profile",
        "required": true,
        "purpose": "Read the review quality method and selected review references.",
        "whenToRead": "Read after review matrices and before writing findings.",
        "selectors": read_selectors_value_from_paths([
            "reviewQualityProfile.loadMode",
            "reviewQualityProfile.reviewMode",
            "reviewQualityProfile.reviewStageOrder",
            "reviewQualityProfile.referenceLoadPlan"
        ])
    })
}

fn review_quality_profile() -> Value {
    json!({
        "loadMode": "mcp_reference_load_plan",
        "reviewMode": "phase_run_review",
        "reviewStageOrder": [
            "spec_compliance",
            "implementation_quality",
            "evidence_quality",
            "routing_decision"
        ],
        "referenceLoadPlan": [
            {
                "refId": "rv.core",
                "path": "tech/review/core.md",
                "reason": "Risk-based review posture, scope reconstruction, inspection order, and repository-fit method."
            },
            {
                "refId": "rv.spec",
                "path": "tech/review/spec-compliance.md",
                "reason": "Current-phase requirement compliance before implementation quality review."
            },
            {
                "refId": "rv.defects",
                "path": "tech/review/defect-patterns.md",
                "reason": "Common correctness, security, persistence, reliability, performance, and maintainability defects."
            },
            {
                "refId": "rv.evidence",
                "path": "tech/review/test-evidence.md",
                "reason": "Verification and evidence sufficiency review."
            },
            {
                "refId": "rv.findings",
                "path": "tech/review/finding-quality.md",
                "reason": "Actionable finding impact, evidence, root-cause, repair ownership, and consistency guidance."
            }
        ]
    })
}

fn review_result_schema_shape() -> Value {
    json!({
        "type": "object",
        "required": [
            "decision",
            "findings",
            "coverageAssessment",
            "limitations",
            "pendingActions",
            "nextAction"
        ],
        "properties": {
            "decision": "approved | approved_with_notes | changes_requested | blocked | needs_user_decision",
            "findings": [{
                "findingType": "defect | note | limitation | contract_gap",
                "severity": "critical | major | minor | note",
                "severityClass": "blocking | warning | info",
                "evidenceKind": "code | test | runtime | manual | contract | review_limitation",
                "failureClass": "product_defect | environment_blocker | review_limitation | contract_gap",
                "category": "enumRefs.findingCategory item",
                "summary": "string",
                "evidence": "string",
                "readRefs": [{
                    "type": "enumRefs.readRefType item",
                    "ref": "allowed review read ref",
                    "reason": "string"
                }],
                "evidenceRefs": [{
                    "type": "enumRefs.evidenceRefType item",
                    "ref": "allowed task result, verification, diff, changed file, or manual ref",
                    "reason": "string"
                }],
                "groupRefs": ["allowed group id"],
                "taskRefs": ["allowed task id"],
                "acceptanceRefs": ["allowed acceptance ref"],
                "artifactRefs": {},
                "location": {},
                "taskRelevance": "direct | indirect | not_applicable",
                "scopeRelation": "within_task_changed_files | current_phase | outside_current_change_set",
                "introducedByCurrentTask": "yes | no | unknown",
                "recommendedNextAction": "enumRefs.nextAction item"
            }],
            "coverageAssessment": {
                "mustAcceptance": [{
                    "acceptanceRef": "reviewScope.acceptanceRefs item",
                    "status": "satisfied | insufficient_evidence | not_satisfied | not_reviewed",
                    "supportingTaskResults": ["outputContract.allowedRefs.taskResultIds item"],
                    "evidenceStatus": "sufficient | insufficient | missing",
                    "notes": ["string"]
                }],
                "summary": {
                    "totalMust": 0,
                    "satisfied": 0,
                    "insufficientEvidence": 0,
                    "notSatisfied": 0,
                    "notReviewed": 0
                }
            },
            "limitations": [{
                "code": "string",
                "summary": "string",
                "impact": "string"
            }],
            "pendingActions": [{
                "type": "enumRefs.nextAction item other than top-level nextAction.type",
                "reason": "string"
            }],
            "nextAction": {
                "type": "enumRefs.nextAction item",
                "reason": "string",
                "userVisibleState": "string or null"
            }
        },
        "additionalProperties": false
    })
}

fn review_result_template(
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    next_phase_handoff: Option<&brainstorm::NextPhaseHandoff>,
) -> Value {
    let first_task_result_ref = run
        .task_states
        .iter()
        .find_map(|state| state.result_id.as_deref())
        .unwrap_or("");
    let must_acceptance = task_plan
        .scope_snapshot
        .acceptance_refs
        .iter()
        .map(|acceptance_ref| {
            json!({
                "acceptanceRef": acceptance_ref,
                "status": "satisfied",
                "supportingTaskResults": [],
                "evidenceStatus": "sufficient",
                "notes": []
            })
        })
        .collect::<Vec<_>>();
    let next_action = if let Some(handoff) = next_phase_handoff {
        json!({
            "type": "continue_to_next_phase",
            "reason": handoff.reason
        })
    } else {
        json!({
            "type": "done",
            "reason": ""
        })
    };
    json!({
        "decision": "approved",
        "findings": [{
            "findingType": "note",
            "severity": "note",
            "severityClass": "info",
            "evidenceKind": "review_limitation",
            "failureClass": "review_limitation",
            "category": "review_limitation",
            "summary": "",
            "evidence": "",
            "readRefs": [{
                "type": "review_packet",
                "ref": "reviewPacket",
                "reason": ""
            }],
            "evidenceRefs": [{
                "type": "task_result",
                "ref": first_task_result_ref,
                "reason": ""
            }],
            "groupRefs": [],
            "taskRefs": [],
            "acceptanceRefs": [],
            "artifactRefs": {},
            "location": {},
            "taskRelevance": "not_applicable",
            "scopeRelation": "current_phase",
            "introducedByCurrentTask": "unknown",
            "recommendedNextAction": "done"
        }],
        "coverageAssessment": {
            "mustAcceptance": must_acceptance,
            "summary": {
                "totalMust": task_plan.scope_snapshot.acceptance_refs.len(),
                "satisfied": task_plan.scope_snapshot.acceptance_refs.len(),
                "insufficientEvidence": 0,
                "notSatisfied": 0,
                "notReviewed": 0
            }
        },
        "limitations": [],
        "pendingActions": [],
        "nextAction": next_action
    })
}

fn review_next_phase_preview(handoff: Option<&brainstorm::NextPhaseHandoff>) -> Value {
    if let Some(handoff) = handoff {
        json!({
            "kind": "candidate",
            "suggestedPhaseId": handoff.phase_id.clone(),
            "title": handoff.title.clone(),
            "goal": handoff.goal.clone(),
            "scopePreview": handoff.scope_preview.clone(),
            "reason": handoff.reason.clone()
        })
    } else {
        json!({
            "kind": "none",
            "suggestedPhaseId": Value::Null,
            "title": Value::Null,
            "goal": Value::Null,
            "scopePreview": [],
            "reason": Value::Null
        })
    }
}

fn review_artifacts_dir(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
    review_id: &str,
) -> std::path::PathBuf {
    delivery_dir(project_root, delivery_id)
        .join("reviews")
        .join(phase_id)
        .join("artifacts")
        .join(review_id)
}

fn build_change_set(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
    review_id: &str,
    task_results: &[TaskResult],
) -> Result<(Value, Value), state::store::StateError> {
    let mut source_by_path = BTreeMap::<String, String>::new();
    for result in task_results {
        for file in &result.changed_files {
            source_by_path
                .entry(file.clone())
                .or_insert_with(|| result.task_result_id.clone());
        }
    }
    let changed_files = source_by_path.keys().cloned().collect::<Vec<_>>();
    if git_diff_available(project_root) {
        let diffs_dir =
            review_artifacts_dir(project_root, delivery_id, phase_id, review_id).join("diffs");
        state::store::ensure_dir(&diffs_dir)?;
        let mut files = Vec::new();
        let mut diff_texts = Vec::new();
        for file in &changed_files {
            let diff_path = diffs_dir.join(format!("{}.diff", safe_id(file)));
            let tracked = git_tracked(project_root, file);
            let diff = git_diff_for_declared_file(project_root, file, tracked);
            state::store::write_text_atomic(&diff_path, &diff)?;
            let has_diff = !diff.trim().is_empty();
            if has_diff {
                diff_texts.push(diff);
            }
            let diff_ref = to_project_relative(project_root, &diff_path)?;
            let (insertions, deletions) =
                git_numstat(project_root, file, tracked).unwrap_or((0, 0));
            files.push(json!({
                "path": file,
                "source": source_by_path.get(file),
                "changeType": if tracked && has_diff { "modified" } else { "declared_changed" },
                "insertions": insertions,
                "deletions": deletions,
                "diffRef": diff_ref
            }));
        }
        let full_diff_path = diffs_dir.join("full.diff");
        state::store::write_text_atomic(&full_diff_path, &diff_texts.join("\n"))?;
        let full_diff_ref = to_project_relative(project_root, &full_diff_path)?;
        let change_set = json!({
            "mode": "git_diff_ref",
            "gitAvailable": true,
            "diffAvailable": true,
            "diffInline": false,
            "summary": {
                "changedFileCount": files.len()
            },
            "files": files,
            "fullDiffRef": full_diff_ref
        });
        let change_context = json!({
            "mode": "git_diff_ref",
            "changedFiles": change_set["files"],
            "fullDiffRef": full_diff_ref
        });
        Ok((change_set, change_context))
    } else {
        let files = changed_files
            .iter()
            .map(|file| {
                json!({
                    "path": file,
                    "source": source_by_path.get(file),
                    "changeType": "declared_changed"
                })
            })
            .collect::<Vec<_>>();
        Ok((
            json!({
                "mode": "current_file_content",
                "gitAvailable": false,
                "diffAvailable": false,
                "diffInline": false,
                "summary": {
                    "changedFileCount": files.len()
                },
                "files": files
            }),
            json!({
                "mode": "current_file_content",
                "changedFiles": files
            }),
        ))
    }
}

fn git_diff_available(project_root: &Path) -> bool {
    git_output(project_root, &["rev-parse", "--is-inside-work-tree"]).is_some()
        && git_output(project_root, &["rev-parse", "--verify", "HEAD"]).is_some()
}

fn git_output(project_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_output_allow_diff_exit(project_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .ok()?;
    if output.status.success() || output.status.code() == Some(1) {
        return Some(String::from_utf8_lossy(&output.stdout).to_string());
    }
    None
}

fn git_tracked(project_root: &Path, file: &str) -> bool {
    git_output(project_root, &["ls-files", "--error-unmatch", "--", file]).is_some()
}

fn git_diff_for_declared_file(project_root: &Path, file: &str, tracked: bool) -> String {
    if tracked {
        return git_output(project_root, &["diff", "HEAD", "--", file]).unwrap_or_default();
    }
    let file_path = project_root.join(file);
    if !file_path.exists() {
        return String::new();
    }
    git_output_allow_diff_exit(
        project_root,
        &["diff", "--no-index", "--", "/dev/null", file],
    )
    .filter(|diff| !diff.trim().is_empty())
    .unwrap_or_else(|| synthetic_new_file_diff(&file_path, file))
}

fn synthetic_new_file_diff(file_path: &Path, file: &str) -> String {
    let content = fs::read_to_string(file_path).unwrap_or_default();
    let line_count = content.lines().count();
    let mut diff = format!(
        "diff --git a/{file} b/{file}\nnew file mode 100644\n--- /dev/null\n+++ b/{file}\n@@ -0,0 +1,{line_count} @@\n"
    );
    for line in content.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    if !content.ends_with('\n') && !content.is_empty() {
        diff.push_str("\\ No newline at end of file\n");
    }
    diff
}

fn git_numstat(project_root: &Path, file: &str, tracked: bool) -> Option<(u32, u32)> {
    let output = if tracked {
        git_output(project_root, &["diff", "HEAD", "--numstat", "--", file])?
    } else if project_root.join(file).exists() {
        git_output_allow_diff_exit(
            project_root,
            &["diff", "--no-index", "--numstat", "--", "/dev/null", file],
        )?
    } else {
        return None;
    };
    let line = output.lines().next()?;
    let mut parts = line.split_whitespace();
    let insertions = parts.next()?.parse::<u32>().ok()?;
    let deletions = parts.next()?.parse::<u32>().ok()?;
    Some((insertions, deletions))
}

fn change_set_rules(mode: &str) -> Vec<&'static str> {
    if mode == "git_diff_ref" {
        vec![
            "Diff content is not inlined.",
            "Read changeContext first and read only per-file diffRefs needed for findings.",
            "Use fullDiffRef only when a cross-file finding cannot be supported by per-file diffRefs.",
            "Line-level findings must be based on a read diffRef or fullDiffRef.",
        ]
    } else {
        vec![
            "Only changed file paths are provided.",
            "Read changed files only when needed.",
            "Critical or major findings must be direct and within task changed files.",
            "Use notes for issues outside the current task change set.",
        ]
    }
}

fn review_severity_policy() -> Value {
    json!({
        "critical": "Blocks accepted behavior, data integrity, security, or runtime viability.",
        "major": "Breaks a must acceptance item, integration contract, or required workflow.",
        "minor": "Important but non-blocking correctness, maintainability, or evidence gap.",
        "note": "Observation that should not change routing."
    })
}

fn review_validator_rules(mode: &str) -> Value {
    json!({
        "blockingFindingsNeedActionableRefs": true,
        "coverageMustListEveryAcceptanceRef": true,
        "pendingActionFindingRefsMustMatchRecommendedNextAction": true,
        "warningOnlyFindingsCannotRouteRepair": true,
        "changeContextMode": mode,
        "currentFileContentMajorFindingRule": "When changeContextMode is current_file_content, critical/major findings must have taskRelevance=direct and scopeRelation=within_task_changed_files."
    })
}

fn manual_review_resolution_template(result: &ReviewResult) -> Value {
    let route = match result.next_action.r#type.as_str() {
        "execution_repair" | "taskplan_repair" | "architecture_artifact_repair" => {
            result.next_action.r#type.as_str()
        }
        _ => "needs_user_decision",
    };
    json!({
        "userAnswer": {
            "text": "",
            "selectedShortReply": "request_changes"
        },
        "decision": "request_changes",
        "changeRequest": {
            "summary": "",
            "route": route,
            "reason": "",
            "details": {}
        },
        "nextAction": {
            "type": route,
            "reason": ""
        }
    })
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
    let fields = read_review_submit_fields(input)?;
    let raw = state::store::read_json_value(&from_project_relative(root, &target.path)?)?;
    let normalized = normalize_review_result_machine_fields(raw, &authorized.request_id, &fields);
    let mut result: ReviewResult = match serde_json::from_value(normalized) {
        Ok(result) => result,
        Err(error) => {
            return review_result_repairable(
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
    if let Some(handoff) =
        normalize_approved_next_phase(&input.project_root, &delivery_id, &phase_id, &result)?
    {
        result.next_action.r#type = "continue_to_next_phase".to_string();
        result.next_action.target_phase_id = Some(handoff.phase_id);
        result.next_action.reason = handoff.reason;
    }
    normalize_browser_environment_review_route(&mut result, &fields);
    normalize_review_signal_targets(&mut result, &fields);
    normalize_review_linkage_fields(&mut result, &fields);
    let issues = validate_review_result(&result, &fields);
    if !issues.is_empty() {
        return review_result_repairable(input, authorized, target.path.clone(), issues);
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

fn read_review_submit_fields(
    input: &FileSubmitInput,
) -> Result<
    std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    state::store::StateError,
> {
    Ok(
        state::read_request_fields(delivery_core::ReadRequestFieldsInput {
            project_root: input.project_root.clone(),
            request_ref: input.request_ref.clone(),
            fields: vec![
                "source.phaseId".to_string(),
                "source.taskPlanId".to_string(),
                "source.taskPlanRunId".to_string(),
                "outputContract.allowedRefs.taskIds".to_string(),
                "outputContract.allowedRefs.groupIds".to_string(),
                "outputContract.allowedRefs.acceptanceRefs".to_string(),
                "outputContract.allowedRefs.taskResultIds".to_string(),
                "outputContract.allowedRefs.changedFilePaths".to_string(),
                "outputContract.allowedRefs.diffRefs".to_string(),
                "outputContract.allowedRefs.verificationEvidenceRefs".to_string(),
                "outputContract.allowedRefs.readRefs".to_string(),
                "enumRefs.readRefType".to_string(),
                "enumRefs.evidenceRefType".to_string(),
                "outputContract.changeContextMode".to_string(),
                "outputContract.reviewSignals.items".to_string(),
                "reviewScope.nextPhasePreview.kind".to_string(),
                "reviewScope.nextPhasePreview.suggestedPhaseId".to_string(),
            ],
        })?
        .fields,
    )
}

fn normalize_review_result_machine_fields(
    mut raw: Value,
    request_id: &str,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Value {
    let Some(object) = raw.as_object_mut() else {
        return raw;
    };
    object.insert("schemaVersion".to_string(), json!("1.0"));
    object.insert("reviewId".to_string(), json!(request_id));
    object.insert(
        "source".to_string(),
        json!({
            "requestId": request_id,
            "phaseId": review_field_value(fields, "source.phaseId"),
            "taskPlanId": review_field_value(fields, "source.taskPlanId"),
            "taskPlanRunId": review_field_value(fields, "source.taskPlanRunId")
        }),
    );
    let now = state::store::now_string();
    object.insert("createdAt".to_string(), json!(now.clone()));
    object.insert("updatedAt".to_string(), json!(now));
    if let Some(findings) = object.get_mut("findings").and_then(Value::as_array_mut) {
        for (index, finding) in findings.iter_mut().enumerate() {
            if let Some(finding) = finding.as_object_mut() {
                finding.insert(
                    "findingId".to_string(),
                    json!(format!("finding-{}", index + 1)),
                );
            }
        }
    }
    normalize_review_pending_actions(object);
    if let Some(next_action) = object.get_mut("nextAction").and_then(Value::as_object_mut) {
        next_action.remove("targetTaskIds");
        next_action.remove("findingRefs");
        next_action.remove("targetPhaseId");
        next_action.remove("targetNode");
    }
    raw
}

fn normalize_review_pending_actions(object: &mut serde_json::Map<String, Value>) {
    let next_action_type = object
        .get("nextAction")
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let Some(raw_items) = object
        .get("pendingActions")
        .and_then(Value::as_array)
        .cloned()
    else {
        object.insert("pendingActions".to_string(), json!([]));
        return;
    };
    let actions = raw_items
        .into_iter()
        .filter_map(|item| {
            let mut action = item.as_object()?.clone();
            let action_type = action
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            if next_action_type.as_deref() == Some(action_type.as_str()) {
                return None;
            }
            action.insert("type".to_string(), json!(action_type));
            action.remove("findingRefs");
            if !action
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
            {
                action.insert(
                    "reason".to_string(),
                    json!("Pending action was normalized from ReviewResult draft."),
                );
            }
            Some(Value::Object(action))
        })
        .collect::<Vec<_>>();
    object.insert("pendingActions".to_string(), Value::Array(actions));
}

fn normalize_review_signal_targets(
    result: &mut ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) {
    let signals = array_field(fields, "outputContract.reviewSignals.items");
    let action_type = result.next_action.r#type.as_str();
    let mut target_task_ids = Vec::new();
    for signal in signals.as_array().into_iter().flatten() {
        if signal.get("recommendedNextAction").and_then(Value::as_str) != Some(action_type) {
            continue;
        }
        target_task_ids.extend(value_string_array(signal, "taskRefs"));
    }
    target_task_ids.extend(
        result
            .findings
            .iter()
            .filter(|finding| finding.recommended_next_action == action_type)
            .flat_map(|finding| finding.task_refs.clone()),
    );
    result.next_action.target_task_ids = if matches!(
        action_type,
        "done" | "continue_to_next_phase" | "review" | "retry_browser_environment"
    ) {
        Vec::new()
    } else {
        dedupe_non_empty(target_task_ids)
    };
}

fn normalize_review_linkage_fields(
    result: &mut ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) {
    if result.next_action.r#type == "continue_to_next_phase" {
        result.next_action.target_phase_id =
            review_field_value(fields, "reviewScope.nextPhasePreview.suggestedPhaseId")
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .or_else(|| result.next_action.target_phase_id.clone());
    } else {
        result.next_action.target_phase_id = None;
    }
    let next_action_type = result.next_action.r#type.clone();
    result.next_action.finding_refs =
        if matches!(next_action_type.as_str(), "done" | "continue_to_next_phase") {
            Vec::new()
        } else {
            result
                .findings
                .iter()
                .filter(|finding| finding.recommended_next_action == next_action_type)
                .map(|finding| finding.finding_id.clone())
                .collect()
        };
    for action in &mut result.pending_actions {
        action.finding_refs = result
            .findings
            .iter()
            .filter(|finding| finding.recommended_next_action == action.r#type)
            .map(|finding| finding.finding_id.clone())
            .collect();
    }
}

fn normalize_browser_environment_review_route(
    result: &mut ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) {
    let signals = array_field(fields, "outputContract.reviewSignals.items");
    let blocked_signals = signals
        .as_array()
        .into_iter()
        .flatten()
        .filter(|signal| {
            signal.get("kind").and_then(Value::as_str) == Some("frontend_ui_quality")
                && signal.get("recommendedNextAction").and_then(Value::as_str)
                    == Some("manual_review")
                && signal
                    .pointer("/browserVerification/environmentBlocked")
                    .and_then(Value::as_bool)
                    == Some(true)
        })
        .collect::<Vec<_>>();
    if blocked_signals.is_empty() {
        return;
    }
    let task_refs = blocked_signals
        .iter()
        .flat_map(|signal| value_string_array(signal, "taskRefs"))
        .chain(blocked_signals.iter().filter_map(|signal| {
            signal
                .pointer("/browserVerification/closureTaskId")
                .and_then(Value::as_str)
                .map(str::to_string)
        }))
        .collect::<Vec<_>>();
    let evidence_refs = blocked_signals
        .iter()
        .filter_map(|signal| {
            signal
                .pointer("/browserVerification/closureTaskResultId")
                .and_then(Value::as_str)
                .map(|result_id| contracts::ReviewEvidenceRef {
                    r#type: "task_result".to_string(),
                    r#ref: result_id.to_string(),
                    reason: "MCP-generated browser environment closure result.".to_string(),
                })
        })
        .collect::<Vec<_>>();
    let finding_id = "finding-browser-environment-unavailable".to_string();
    result
        .findings
        .retain(|finding| finding.finding_id != finding_id);
    result.findings.push(ReviewFinding {
        finding_id: finding_id.clone(),
        finding_type: Some("limitation".to_string()),
        concept_ref: None,
        severity: "minor".to_string(),
        severity_class: Some("blocking".to_string()),
        evidence_kind: Some("runtime".to_string()),
        failure_class: Some("environment_blocker".to_string()),
        category: "environment_or_dependency".to_string(),
        summary: "Required browser evidence is unavailable in supported execution environments."
            .to_string(),
        evidence: "Host launch doctor and managed Playwright container smoke both failed; project code was not classified as defective."
            .to_string(),
        read_refs: vec![contracts::ReviewReadRef {
            r#type: "review_packet".to_string(),
            r#ref: "reviewPacket".to_string(),
            reason: "Compact browser closure status and environment diagnostics.".to_string(),
        }],
        evidence_refs,
        group_refs: Vec::new(),
        task_refs: dedupe_non_empty(task_refs),
        acceptance_refs: Vec::new(),
        artifact_refs: json!({}),
        location: json!({}),
        task_relevance: "indirect".to_string(),
        scope_relation: "current_phase".to_string(),
        introduced_by_current_task: "no".to_string(),
        recommended_next_action: "manual_review".to_string(),
    });
    result.decision = "blocked".to_string();
    result.next_action.r#type = "manual_review".to_string();
    result.next_action.reason =
        "Required browser evidence needs an environment retry, external evidence, or explicit waiver."
            .to_string();
    result.next_action.target_task_ids = result
        .findings
        .iter()
        .find(|finding| finding.finding_id == finding_id)
        .map(|finding| finding.task_refs.clone())
        .unwrap_or_default();
    result.next_action.finding_refs = vec![finding_id];
}

fn review_field_value(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    name: &str,
) -> Value {
    fields
        .get(name)
        .map(|field| field.value.clone())
        .unwrap_or(Value::Null)
}

fn validate_review_result(
    result: &ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    validate_review_enums(result, &mut issues);
    let allowed = json!({
        "taskIds": array_field(fields, "outputContract.allowedRefs.taskIds"),
        "groupIds": array_field(fields, "outputContract.allowedRefs.groupIds"),
        "acceptanceRefs": array_field(fields, "outputContract.allowedRefs.acceptanceRefs"),
        "taskResultIds": array_field(fields, "outputContract.allowedRefs.taskResultIds"),
        "changedFilePaths": array_field(fields, "outputContract.allowedRefs.changedFilePaths"),
        "diffRefs": array_field(fields, "outputContract.allowedRefs.diffRefs"),
        "verificationEvidenceRefs": array_field(fields, "outputContract.allowedRefs.verificationEvidenceRefs"),
        "readRefs": array_field(fields, "outputContract.allowedRefs.readRefs")
    });
    validate_review_refs(result, &allowed, fields, &mut issues);
    validate_review_coverage(result, &allowed, &mut issues);
    validate_review_decision(result, fields, &mut issues);
    validate_review_signals(result, fields, &mut issues);
    issues
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
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let task_ids = allowed_set(allowed, "taskIds");
    let group_ids = allowed_set(allowed, "groupIds");
    let acceptance_refs = allowed_set(allowed, "acceptanceRefs");
    let task_result_ids = allowed_set(allowed, "taskResultIds");
    let read_refs = allowed_set(allowed, "readRefs");
    let changed_file_refs = normalized_allowed_set(allowed, "changedFilePaths");
    let verification_refs = allowed_set(allowed, "verificationEvidenceRefs");
    let read_ref_types = string_set_field(fields, "enumRefs.readRefType");
    let evidence_ref_types = string_set_field(fields, "enumRefs.evidenceRefType");
    let change_context_mode = fields
        .get("outputContract.changeContextMode")
        .and_then(|field| field.value.as_str())
        .unwrap_or("current_file_content");
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
        if is_blocking_finding(finding)
            && !blocking_finding_has_actionable_ref(finding)
            && !["manual_review", "needs_user_decision"]
                .contains(&finding.recommended_next_action.as_str())
        {
            issues.push(issue(
                "REVIEW_RESULT_REF_INVALID",
                "findings[].refs",
                "Blocking review findings must cite a task, group, artifact, or file location unless routed to manual/user decision.",
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
            if !read_ref_types.contains(&read_ref.r#type) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].readRefs",
                    "Review finding readRefs must use allowed readRef types.",
                ));
            } else if !is_allowed_review_read_ref(
                &read_ref.r#type,
                &read_ref.r#ref,
                &read_refs,
                &task_result_ids,
                &changed_file_refs,
                &verification_refs,
            ) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].readRefs",
                    "Review finding readRefs must use allowed request refs, task result ids, changed file refs, or verification refs.",
                ));
            }
        }
        for evidence_ref in &finding.evidence_refs {
            if !evidence_ref_types.contains(&evidence_ref.r#type) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].evidenceRefs",
                    "Review finding evidenceRefs must use allowed evidenceRef types.",
                ));
            } else if !is_allowed_review_evidence_ref(
                &evidence_ref.r#type,
                &evidence_ref.r#ref,
                &read_refs,
                &task_result_ids,
                &changed_file_refs,
                &verification_refs,
            ) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "findings[].evidenceRefs",
                    "Review finding evidenceRefs must use allowed task result, verification, diff, changed file, or manual refs.",
                ));
            }
        }
        if change_context_mode == "current_file_content"
            && matches!(finding.severity.as_str(), "critical" | "major")
            && !(finding.task_relevance == "direct"
                && finding.scope_relation == "within_task_changed_files")
        {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "findings[].severity",
                "In current_file_content mode, critical or major findings must be direct and within task changed files.",
            ));
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
            } else if result
                .findings
                .iter()
                .find(|finding| finding.finding_id == *finding_ref)
                .map(|finding| finding.recommended_next_action.as_str())
                != Some(action.r#type.as_str())
            {
                issues.push(issue(
                    "REVIEW_RESULT_STATUS_INCONSISTENT",
                    "pendingActions[].findingRefs",
                    "pendingActions findingRefs must match each finding recommendedNextAction.",
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

fn validate_review_coverage(
    result: &ReviewResult,
    allowed: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let acceptance_refs = allowed_set(allowed, "acceptanceRefs");
    let task_result_ids = allowed_set(allowed, "taskResultIds");
    let covered = result
        .coverage_assessment
        .must_acceptance
        .iter()
        .map(|assessment| assessment.acceptance_ref.clone())
        .collect::<BTreeSet<_>>();
    for assessment in &result.coverage_assessment.must_acceptance {
        if !acceptance_refs.contains(&assessment.acceptance_ref) {
            issues.push(issue(
                "REVIEW_RESULT_REF_INVALID",
                "coverageAssessment.mustAcceptance[].acceptanceRef",
                "Review coverageAssessment must use current phase acceptance refs.",
            ));
        }
        for task_result_ref in &assessment.supporting_task_results {
            if !task_result_ids.contains(task_result_ref) {
                issues.push(issue(
                    "REVIEW_RESULT_REF_INVALID",
                    "coverageAssessment.mustAcceptance[].supportingTaskResults",
                    "Review coverage supportingTaskResults must use allowed task result ids.",
                ));
            }
        }
        if assessment.status != "satisfied"
            && !result
                .findings
                .iter()
                .any(|finding| finding.acceptance_refs.contains(&assessment.acceptance_ref))
        {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "coverageAssessment.mustAcceptance[]",
                "Unsatisfied acceptance coverage requires a finding that cites that acceptanceRef.",
            ));
        }
    }
    for acceptance_ref in acceptance_refs {
        if !covered.contains(&acceptance_ref) {
            issues.push(issue(
                "REVIEW_RESULT_REF_INVALID",
                "coverageAssessment.mustAcceptance",
                "Review coverageAssessment.mustAcceptance must include every current phase acceptanceRef.",
            ));
        }
    }
}

fn validate_review_decision(
    result: &ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
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
    let expected = expected_top_action(result, fields);
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
    if !result.findings.is_empty()
        && result
            .findings
            .iter()
            .all(|finding| finding.severity_class.as_deref() == Some("warning"))
        && [
            "execution_repair",
            "taskplan_repair",
            "architecture_artifact_repair",
            "manual_review",
            "needs_user_decision",
        ]
        .contains(&result.next_action.r#type.as_str())
    {
        issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "nextAction.type",
            "Warning-only review findings cannot route repair, manual review, or user decision.",
        ));
    }
}

fn validate_review_signals(
    result: &ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let signals = array_field(fields, "outputContract.reviewSignals.items");
    let unsatisfied_detail = signals.as_array().into_iter().flatten().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("requirement_detail_evidence")
            && item.get("detailSatisfied").and_then(Value::as_bool) == Some(false)
    });
    let unsatisfied_frontend = signals.as_array().into_iter().flatten().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("frontend_workflow_closure")
            && item.get("closureSatisfied").and_then(Value::as_bool) == Some(false)
    });
    let unsatisfied_frontend_quality = signals.as_array().into_iter().flatten().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("frontend_ui_quality")
            && item.get("uiQualitySatisfied").and_then(Value::as_bool) == Some(false)
    });
    let unsatisfied_architecture_quality = signals.as_array().into_iter().flatten().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("architecture_quality")
            && item
                .get("architectureQualitySatisfied")
                .and_then(Value::as_bool)
                == Some(false)
    });
    let unsatisfied_api_contract = signals.as_array().into_iter().flatten().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("api_contract")
            && item.get("apiContractSatisfied").and_then(Value::as_bool) == Some(false)
    });
    let missing_workflow_task_assignment = signals.as_array().into_iter().flatten().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("frontend_workflow_closure")
            && item.get("missingTaskAssignment").and_then(Value::as_bool) == Some(true)
            && item.get("recommendedNextAction").and_then(Value::as_str) == Some("taskplan_repair")
    });
    let missing_architecture_quality_task_assignment =
        signals.as_array().into_iter().flatten().any(|item| {
            item.get("kind").and_then(Value::as_str) == Some("architecture_quality")
                && item.get("missingTaskAssignment").and_then(Value::as_bool) == Some(true)
                && item.get("recommendedNextAction").and_then(Value::as_str)
                    == Some("taskplan_repair")
        });
    if matches!(result.decision.as_str(), "approved" | "approved_with_notes")
        && (unsatisfied_detail
            || unsatisfied_frontend
            || unsatisfied_frontend_quality
            || unsatisfied_architecture_quality
            || unsatisfied_api_contract)
    {
        issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "decision",
            "ReviewResult cannot approve when outputContract.reviewSignals contain unsatisfied requirement detail, frontend workflow closure, frontend UI quality, architecture quality, or API contract.",
        ));
    }
    validate_execution_repair_signal_targets(result, &signals, issues);
    if missing_workflow_task_assignment || missing_architecture_quality_task_assignment {
        let has_higher_priority_blocker = result.findings.iter().any(|finding| {
            is_blocking_finding(finding)
                && [
                    "needs_user_decision",
                    "manual_review",
                    "architecture_artifact_repair",
                ]
                .contains(&finding.recommended_next_action.as_str())
        });
        let has_taskplan_repair_finding = result.findings.iter().any(|finding| {
            is_blocking_finding(finding) && finding.recommended_next_action == "taskplan_repair"
        });
        if !has_higher_priority_blocker && result.next_action.r#type != "taskplan_repair" {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "nextAction.type",
                "Missing workflow closure or architecture quality task assignment must route taskplan_repair unless a higher-priority blocking finding applies.",
            ));
        }
        if !has_higher_priority_blocker && !has_taskplan_repair_finding {
            issues.push(issue(
                "REVIEW_RESULT_STATUS_INCONSISTENT",
                "findings",
                "Missing workflow closure or architecture quality task assignment requires a blocking taskplan_repair finding.",
            ));
        }
    }
}

fn validate_execution_repair_signal_targets(
    result: &ReviewResult,
    signals: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if result.next_action.r#type != "execution_repair" {
        return;
    }
    let expected_task_ids = signals
        .as_array()
        .into_iter()
        .flatten()
        .filter(|signal| {
            signal.get("recommendedNextAction").and_then(Value::as_str) == Some("execution_repair")
        })
        .flat_map(|signal| value_string_array(signal, "taskRefs"))
        .collect::<BTreeSet<_>>();
    if expected_task_ids.is_empty() {
        return;
    }
    let routed_task_ids = review_execution_repair_target_task_ids(result)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let missing_task_ids = expected_task_ids
        .difference(&routed_task_ids)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_task_ids.is_empty() {
        issues.push(issue(
            "REVIEW_RESULT_STATUS_INCONSISTENT",
            "nextAction.targetTaskIds",
            "ReviewResult execution_repair must target every task referenced by outputContract.reviewSignals execution_repair items.",
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
    let target_task_ids = review_execution_repair_target_task_ids(result);
    let next_action = route_next_action_with_target_task_ids(&result.next_action, &target_task_ids);
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
            "nextAction": next_action
        })),
        target_phase_id: result.next_action.target_phase_id.clone(),
    }
}

fn review_execution_repair_target_task_ids(result: &ReviewResult) -> Vec<String> {
    let selected_finding_refs = result
        .next_action
        .finding_refs
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut values = result.next_action.target_task_ids.clone();
    for finding in &result.findings {
        if finding.recommended_next_action != "execution_repair" {
            continue;
        }
        if !selected_finding_refs.is_empty() && !selected_finding_refs.contains(&finding.finding_id)
        {
            continue;
        }
        values.extend(finding.task_refs.clone());
    }
    dedupe_non_empty(values)
}

fn normalize_approved_next_phase(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    result: &ReviewResult,
) -> Result<Option<brainstorm::NextPhaseHandoff>, state::store::StateError> {
    if !matches!(result.decision.as_str(), "approved" | "approved_with_notes")
        || !matches!(
            result.next_action.r#type.as_str(),
            "done" | "continue_to_next_phase"
        )
    {
        return Ok(None);
    }
    materialize_approved_next_phase_from_preview(
        project_root,
        delivery_id,
        phase_id,
        &result.next_action.r#type,
    )
}

fn materialize_approved_next_phase_from_preview(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    action_type: &str,
) -> Result<Option<brainstorm::NextPhaseHandoff>, state::store::StateError> {
    if !matches!(action_type, "done" | "continue_to_next_phase") {
        return Ok(None);
    }
    // The accepted phase preview is the source of truth for the handoff. The
    // agent must not be able to select a different phase by writing a target id.
    brainstorm::materialize_next_phase_from_preview(project_root, delivery_id, phase_id, None)
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
    let browser_quality_gate = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec!["outputContract.reviewSignals.items".to_string()],
    })
    .ok()
    .and_then(|read| {
        read.fields
            .get("outputContract.reviewSignals.items")
            .cloned()
    })
    .and_then(|field| {
        field.value.as_array().and_then(|signals| {
            signals
                .iter()
                .find(|signal| {
                    signal.get("recommendedNextAction").and_then(Value::as_str)
                        == Some("manual_review")
                        && signal
                            .pointer("/browserVerification/environmentBlocked")
                            .and_then(Value::as_bool)
                            == Some(true)
                })
                .cloned()
        })
    });
    let request_root = build_manual_review_request(
        &request_id,
        &delivery_id,
        &phase_id,
        &result_file,
        result,
        &result_ref,
        browser_quality_gate.as_ref(),
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
    let browser_environment_gate = browser_quality_gate.is_some();
    Ok(LoomMcpActionResult::UserGate(LoomMcpUserGateResult::new(
        input.project_root.clone(),
        if browser_environment_gate {
            "Required browser evidence is unavailable. Retry the browser environment, submit external browser evidence, or approve a quality waiver."
        } else {
            "Review requires user decision. Reply approve_override to continue with notes, or request_changes with the repair route and change summary."
        },
        if browser_environment_gate {
            vec![
                "retry_browser_environment".to_string(),
                "submit_external_browser_evidence".to_string(),
                "approve_quality_waiver".to_string(),
            ]
        } else {
            vec![
                "approve_override".to_string(),
                "request_changes".to_string(),
            ]
        },
        Some(stored.request_ref),
        Some(delivery_id),
        Some(phase_id),
        Some(json!({
            "gateId": format!("manual_review_{}", result.review_id),
            "kind": "manual_review",
            "reviewResultRef": result_ref,
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
    )))
}

fn build_manual_review_request(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    result_file: &str,
    result: &ReviewResult,
    result_ref: &str,
    browser_quality_gate: Option<&Value>,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(ManualReviewResolution))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    if let Some(browser_quality_gate) = browser_quality_gate {
        let required_check_ids = browser_quality_gate
            .pointer("/browserVerification/requiredCheckIds")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let completion_type = if result.next_action.target_phase_id.is_some() {
            "continue_to_next_phase"
        } else {
            "done"
        };
        let common = json!({
            "userAnswer": {"text": "", "selectedShortReply": ""}
        });
        let template = |decision: &str, browser_resolution: Value, next_type: &str| {
            let mut value = common.clone();
            value["userAnswer"]["selectedShortReply"] = json!(decision);
            value["decision"] = json!(decision);
            value["changeRequest"] = Value::Null;
            if !browser_resolution.is_null() {
                value["browserQualityResolution"] = browser_resolution;
            }
            value["nextAction"] = json!({
                "type": next_type,
                "reason": "",
            });
            value
        };
        return json!({
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
                "browserQualityGate": browser_quality_gate
            },
            "manualReviewProtocol": {
                "acceptedDecisions": [
                    "retry_browser_environment",
                    "submit_external_browser_evidence",
                    "approve_quality_waiver"
                ],
                "retryRule": "Re-run MCP browser preparation after the environment or dependencies have changed; do not route through execution repair.",
                "externalEvidenceRule": "Provide one concrete evidence item for every required check id in source.browserQualityGate.browserVerification.requiredCheckIds order. Loom binds each item to its check id; evidence may cite project-relative artifacts or HTTPS CI/report URLs.",
                "waiverRule": "A quality waiver requires an explicit user reason and records the missing browser evidence as an accepted limitation."
            },
            "enumRefs": {
                "decision": ["retry_browser_environment", "submit_external_browser_evidence", "approve_quality_waiver"],
                "nextActionType": ["retry_browser_environment", "review", "done", "continue_to_next_phase"]
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
                    "description": "Write the selected browser quality resolution."
                }],
                "requiredFields": [
                    "userAnswer", "decision", "nextAction"
                ],
                "schemaShape": schema_shape,
                "resultTemplatesByDecision": {
                    "retry_browser_environment": template("retry_browser_environment", Value::Null, "retry_browser_environment"),
                    "submit_external_browser_evidence": template(
                        "submit_external_browser_evidence",
                        json!({
                            "externalEvidence": required_check_ids.iter().map(|_| json!({
                                "evidenceRefs": [],
                                "observedOutcome": "",
                                "source": ""
                            })).collect::<Vec<_>>()
                        }),
                        "review"
                    ),
                    "approve_quality_waiver": template(
                        "approve_quality_waiver",
                        json!({"waiverReason": ""}),
                        completion_type
                    )
                }
            },
            "requestReadPlan": {"groups": [
                {
                    "groupId": "browser_quality_resolution_context",
                    "required": true,
                    "purpose": "Read the blocked browser checks and selected resolution protocol.",
                    "whenToRead": "Read after the user selects a browser quality resolution.",
                    "selectors": read_selectors_value_from_paths([
                        "source.reviewId",
                        "source.reviewResultRef",
                        "source.reviewNextAction",
                        "source.browserQualityGate",
                        "manualReviewProtocol.acceptedDecisions",
                        "manualReviewProtocol.retryRule",
                        "manualReviewProtocol.externalEvidenceRule",
                        "manualReviewProtocol.waiverRule",
                        "enumRefs.decision",
                        "enumRefs.nextActionType"
                    ])
                },
                {
                    "groupId": "browser_quality_resolution_write_contract",
                    "required": true,
                    "purpose": "Read the exact output path and template for the selected decision.",
                    "whenToRead": "Read before writing the resolution.",
                    "selectors": read_selectors_value_from_paths([
                        "outputContract.resultFile",
                        "outputContract.writeTargets",
                        "outputContract.requiredFields",
                        "outputContract.resultTemplatesByDecision"
                    ])
                }
            ]}
        });
    }
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
                "userAnswer", "decision", "changeRequest", "nextAction"
            ],
            "schemaShape": schema_shape,
            "resultTemplate": manual_review_resolution_template(
                result,
            )
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "manual_review_context",
                    "required": true,
                    "purpose": "Read the review issue and allowed user decision protocol.",
                    "whenToRead": "Read after the user answers the manual review gate.",
                    "selectors": read_selectors_value_from_paths([
                        "source.reviewId",
                        "source.reviewResultRef",
                        "source.decision",
                        "source.reviewNextAction",
                        "source.blockingFindings",
                        "manualReviewProtocol.acceptedDecisions",
                        "manualReviewProtocol.approveOverrideRule",
                        "manualReviewProtocol.requestChangesRule",
                        "manualReviewProtocol.routeRules",
                        "enumRefs.decision",
                        "enumRefs.changeRequestRoute",
                        "enumRefs.nextActionType"
                    ])
                },
                {
                    "groupId": "manual_review_write_contract",
                    "required": true,
                    "purpose": "Read the authorized resolution output path and schema fields.",
                    "whenToRead": "Read before writing ManualReviewResolution.",
                    "selectors": read_selectors_value_from_paths([
                        "outputContract.resultFile",
                        "outputContract.writeTargets",
                        "outputContract.requiredFields",
                        "outputContract.resultTemplate",
                        "outputContract.schemaShape.properties.decision",
                        "outputContract.schemaShape.properties.changeRequest",
                        "outputContract.schemaShape.properties.nextAction"
                    ])
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
    let normalized = normalize_manual_review_resolution_machine_fields(
        raw,
        &authorized.request_id,
        &delivery_id,
        &phase_id,
    );
    let mut resolution: ManualReviewResolution = match serde_json::from_value(normalized) {
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
    let mut requested_fields = vec![
        "source.reviewId".to_string(),
        "source.reviewNextAction".to_string(),
        "enumRefs.decision".to_string(),
        "enumRefs.nextActionType".to_string(),
    ];
    let browser_quality_resolution = authorized
        .read_groups
        .iter()
        .any(|group| group.group_id == "browser_quality_resolution_context");
    if browser_quality_resolution {
        requested_fields.push("source.browserQualityGate".to_string());
    } else {
        requested_fields.push("enumRefs.changeRequestRoute".to_string());
    }
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: requested_fields,
    })?
    .fields;
    normalize_manual_review_resolution_links(&mut resolution, &fields);
    let issues = validate_manual_review_resolution(&resolution, &fields);
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
    let approved_next_phase = if matches!(
        resolution.decision.as_str(),
        "approve_override" | "approve_quality_waiver"
    ) {
        materialize_approved_next_phase_from_preview(
            &input.project_root,
            &delivery_id,
            &phase_id,
            &resolution.next_action.r#type,
        )?
    } else {
        None
    };
    if let Some(handoff) = approved_next_phase {
        resolution.next_action.r#type = "continue_to_next_phase".to_string();
        resolution.next_action.target_phase_id = Some(handoff.phase_id);
        resolution.next_action.reason = handoff.reason;
    } else if resolution.next_action.r#type == "continue_to_next_phase" {
        return Ok(repairable_with_tool(
            input,
            authorized,
            target.path.clone(),
            vec![issue(
                "MANUAL_REVIEW_NEXT_PHASE_UNAVAILABLE",
                "nextAction.type",
                "continue_to_next_phase requires an accepted nextPhasePreview candidate. Use done when no next phase is available.",
            )],
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
    apply_browser_quality_resolution(&input.project_root, &locator, &resolution, &fields)?;
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

fn normalize_manual_review_resolution_machine_fields(
    mut raw: Value,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Value {
    let Some(object) = raw.as_object_mut() else {
        return raw;
    };
    object.insert("schemaVersion".to_string(), json!("1.0"));
    object.insert(
        "manualReviewResolutionId".to_string(),
        json!(format!("manual-review-resolution-{request_id}")),
    );
    object.insert("manualReviewRequestId".to_string(), json!(request_id));
    object.insert("deliveryId".to_string(), json!(delivery_id));
    object.insert("phaseId".to_string(), json!(phase_id));
    object.insert("createdAt".to_string(), json!(state::store::now_string()));
    raw
}

fn normalize_manual_review_resolution_links(
    resolution: &mut ManualReviewResolution,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) {
    let source_next_action = fields
        .get("source.reviewNextAction")
        .map(|field| &field.value)
        .filter(|value| value.is_object());
    let source_next_action = source_next_action.unwrap_or(&Value::Null);
    resolution.next_action.target_task_ids =
        value_string_array(source_next_action, "targetTaskIds");
    resolution.next_action.finding_refs = value_string_array(source_next_action, "findingRefs");
    if resolution.next_action.r#type == "continue_to_next_phase" {
        resolution.next_action.target_phase_id = source_next_action
            .get("targetPhaseId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
    } else {
        resolution.next_action.target_phase_id = None;
    }
    let required_check_ids = fields
        .get("source.browserQualityGate")
        .map(|field| &field.value)
        .and_then(|gate| gate.pointer("/browserVerification/requiredCheckIds"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !required_check_ids.is_empty() {
        let raw_evidence = resolution
            .browser_quality_resolution
            .as_ref()
            .map(|value| value.external_evidence.clone())
            .unwrap_or_default();
        if let Some(browser_resolution) = resolution.browser_quality_resolution.as_mut() {
            browser_resolution.external_evidence = required_check_ids
                .iter()
                .enumerate()
                .map(|(index, check_id)| {
                    let mut evidence = raw_evidence.get(index).cloned().unwrap_or(
                        contracts::BrowserExternalEvidence {
                            check_id: String::new(),
                            evidence_refs: Vec::new(),
                            observed_outcome: String::new(),
                            source: String::new(),
                        },
                    );
                    evidence.check_id = check_id.clone();
                    evidence
                })
                .collect();
        }
    }
}

fn validate_manual_review_resolution(
    resolution: &ManualReviewResolution,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if fields
        .get("source.reviewId")
        .and_then(|field| field.value.as_str())
        .is_none()
    {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_REF_INVALID",
            "source",
            "ManualReview request source must include the reviewId.",
        ));
    }
    let browser_quality_gate = fields
        .get("source.browserQualityGate")
        .map(|field| &field.value)
        .filter(|value| value.is_object());
    if let Some(browser_quality_gate) = browser_quality_gate {
        validate_browser_quality_manual_resolution(resolution, browser_quality_gate, &mut issues);
        return issues;
    }
    if resolution.browser_quality_resolution.is_some() {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
            "browserQualityResolution",
            "Generic manual review cannot include a browser quality resolution.",
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
    issues
}

fn validate_browser_quality_manual_resolution(
    resolution: &ManualReviewResolution,
    gate: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if resolution.user_answer.selected_short_reply.as_deref() != Some(resolution.decision.as_str())
    {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
            "userAnswer.selectedShortReply",
            "Browser quality selectedShortReply must match decision.",
        ));
    }
    if resolution.change_request.is_some() {
        issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
            "changeRequest",
            "Browser quality resolution does not use generic changeRequest routing.",
        ));
    }
    match resolution.decision.as_str() {
        "retry_browser_environment" => {
            if resolution.browser_quality_resolution.is_some() {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "browserQualityResolution",
                    "retry_browser_environment does not include evidence or waiver data.",
                ));
            }
            if resolution.next_action.r#type != "retry_browser_environment" {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "nextAction.type",
                    "retry_browser_environment must use the dedicated environment retry route.",
                ));
            }
        }
        "submit_external_browser_evidence" => {
            if resolution.next_action.r#type != "review" {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "nextAction.type",
                    "External browser evidence must return to Review.",
                ));
            }
            let expected = gate
                .pointer("/browserVerification/requiredCheckIds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            let Some(browser_resolution) = resolution.browser_quality_resolution.as_ref() else {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "browserQualityResolution",
                    "External browser evidence requires browserQualityResolution.",
                ));
                return;
            };
            let actual = browser_resolution
                .external_evidence
                .iter()
                .map(|evidence| evidence.check_id.clone())
                .collect::<BTreeSet<_>>();
            if actual != expected || actual.len() != browser_resolution.external_evidence.len() {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_REF_INVALID",
                    "browserQualityResolution.externalEvidence[].checkId",
                    "External evidence must cover every required browser check exactly once.",
                ));
            }
            for evidence in &browser_resolution.external_evidence {
                if evidence.evidence_refs.is_empty()
                    || evidence.observed_outcome.trim().is_empty()
                    || evidence.source.trim().is_empty()
                    || evidence
                        .evidence_refs
                        .iter()
                        .any(|reference| !valid_external_browser_evidence_ref(reference))
                {
                    issues.push(issue(
                        "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                        "browserQualityResolution.externalEvidence",
                        "Each external browser evidence item requires valid evidenceRefs, observedOutcome, and source.",
                    ));
                }
            }
        }
        "approve_quality_waiver" => {
            if !matches!(
                resolution.next_action.r#type.as_str(),
                "done" | "continue_to_next_phase"
            ) {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "nextAction.type",
                    "Quality waiver can only complete the delivery or continue to the next phase.",
                ));
            }
            if resolution
                .browser_quality_resolution
                .as_ref()
                .and_then(|browser| browser.waiver_reason.as_deref())
                .is_none_or(|reason| reason.trim().is_empty())
            {
                issues.push(issue(
                    "MANUAL_REVIEW_RESOLUTION_STATUS_INVALID",
                    "browserQualityResolution.waiverReason",
                    "Quality waiver requires an explicit non-empty reason.",
                ));
            }
        }
        _ => issues.push(issue(
            "MANUAL_REVIEW_RESOLUTION_ENUM_INVALID",
            "decision",
            "Browser quality manual review decision is not allowed.",
        )),
    }
}

fn valid_external_browser_evidence_ref(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("https://")
        || (!value.is_empty()
            && !value.starts_with('/')
            && !value.starts_with('~')
            && !value.contains('\\')
            && !value
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            && !value.starts_with(".loom/")
            && !value.starts_with(".git/")
            && !value.starts_with("node_modules/"))
}

fn apply_browser_quality_resolution(
    project_root: &str,
    locator: &DeliveryPhaseLocator,
    resolution: &ManualReviewResolution,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> Result<(), state::store::StateError> {
    if !matches!(
        resolution.decision.as_str(),
        "retry_browser_environment" | "submit_external_browser_evidence"
    ) {
        return Ok(());
    }
    let gate = fields
        .get("source.browserQualityGate")
        .map(|field| &field.value)
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser quality resolution is missing source.browserQualityGate".to_string(),
            )
        })?;
    let closure_task_id = gate
        .pointer("/browserVerification/closureTaskId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser quality gate is missing closureTaskId".to_string(),
            )
        })?;
    let root = Path::new(project_root);
    let (task_plan, mut run) = load_current_plan_and_run(root, locator)?;
    let closure_task = task_plan
        .tasks
        .iter()
        .find(|task| task.task_id == closure_task_id)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser quality gate references a missing closure task".to_string(),
            )
        })?;

    if resolution.decision == "retry_browser_environment" {
        let _ = fs::remove_file(root.join(".loom/runtime/browser-automation/latest.json"));
        if let Some(state) = run
            .task_states
            .iter_mut()
            .find(|state| state.task_id == closure_task_id)
        {
            state.status = contracts::TaskRunStatus::Pending;
            state.result_id = None;
            state.started_at = None;
            state.finished_at = None;
        }
        if let Some(group) = run
            .group_states
            .iter_mut()
            .find(|group| group.group_id == closure_task.group_id)
        {
            group.status = contracts::TaskRunStatus::Pending;
            group.started_at = None;
            group.finished_at = None;
        }
        run.status = TaskPlanRunStatus::Running;
        run.next_action = Some(contracts::TaskPlanRunNextAction {
            r#type: "continue_execution".to_string(),
            reason: "BROWSER_ENVIRONMENT_RETRY_REQUESTED".to_string(),
            source_task_id: Some(closure_task_id.to_string()),
            target_node: "task_execution".to_string(),
        });
        run.updated_at = state::store::now_string();
        update_run_summary(&mut run);
        return save_run(root, locator, &run);
    }

    let result_id = run
        .task_states
        .iter()
        .find(|state| state.task_id == closure_task_id)
        .and_then(|state| state.result_id.clone())
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser closure result is missing for external evidence".to_string(),
            )
        })?;
    let result_path = task_result_file(root, locator, &run.run_id, closure_task_id, &result_id);
    let mut result: TaskResult = state::store::read_json(&result_path)?;
    let evidence = resolution
        .browser_quality_resolution
        .as_ref()
        .map(|browser| {
            browser
                .external_evidence
                .iter()
                .map(|evidence| (evidence.check_id.as_str(), evidence))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    for verification in &mut result.verification_results {
        for check in &mut verification.browser_checks {
            let Some(external) = evidence.get(check.check_id.as_str()) else {
                continue;
            };
            check.status = contracts::BrowserCheckStatus::Passed;
            check.command = "external_browser_evidence".to_string();
            check.attempts = 1;
            check.artifact_refs = external.evidence_refs.clone();
            check.observed_outcome = format!(
                "{} (source: {})",
                external.observed_outcome, external.source
            );
            check.blocked_reason = None;
        }
    }
    let profile = task_plan
        .browser_verification_profiles
        .iter()
        .find(|profile| profile.task_id == closure_task_id)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "browser closure profile is missing for external evidence".to_string(),
            )
        })?;
    for verification in &mut result.verification_results {
        let required_passed = profile
            .checks
            .iter()
            .filter(|check| {
                check.verification_id == verification.verification_id
                    && check.enforcement == contracts::BrowserEvidenceEnforcement::Required
            })
            .all(|expected| {
                verification.browser_checks.iter().any(|actual| {
                    actual.check_id == expected.check_id
                        && actual.status == contracts::BrowserCheckStatus::Passed
                })
            });
        if required_passed {
            verification.status = "passed".to_string();
            verification.summary =
                "Required browser evidence was supplied through the external evidence gate."
                    .to_string();
        }
    }
    let has_non_passed = result
        .verification_results
        .iter()
        .flat_map(|verification| verification.browser_checks.iter())
        .any(|check| check.status != contracts::BrowserCheckStatus::Passed);
    result.status = if has_non_passed {
        contracts::TaskResultStatus::CompletedWithNotes
    } else {
        contracts::TaskResultStatus::Completed
    };
    result.notes = if has_non_passed {
        vec!["Required checks use external evidence; supplemental browser checks remain unavailable."
            .to_string()]
    } else {
        vec!["Browser checks were closed with user-submitted external evidence.".to_string()]
    };
    result.updated_at = state::store::now_string();
    state::store::write_json_atomic(&result_path, &result)?;
    if let Some(state) = run
        .task_states
        .iter_mut()
        .find(|state| state.task_id == closure_task_id)
    {
        state.status = if has_non_passed {
            contracts::TaskRunStatus::CompletedWithNotes
        } else {
            contracts::TaskRunStatus::Completed
        };
    }
    if let Some(group) = run
        .group_states
        .iter_mut()
        .find(|group| group.group_id == closure_task.group_id)
    {
        group.status = if has_non_passed {
            contracts::TaskRunStatus::CompletedWithNotes
        } else {
            contracts::TaskRunStatus::Completed
        };
    }
    run.status = if has_non_passed {
        TaskPlanRunStatus::CompletedWithNotes
    } else {
        TaskPlanRunStatus::Completed
    };
    run.next_action = Some(contracts::TaskPlanRunNextAction {
        r#type: "review".to_string(),
        reason: "EXTERNAL_BROWSER_EVIDENCE_ACCEPTED".to_string(),
        source_task_id: Some(closure_task_id.to_string()),
        target_node: "review".to_string(),
    });
    run.updated_at = state::store::now_string();
    update_run_summary(&mut run);
    save_run(root, locator, &run)
}

fn effective_manual_review_action(resolution: &ManualReviewResolution) -> RouteAction {
    let (kind, reason) = match resolution.decision.as_str() {
        "approve_override" | "approve_quality_waiver" => (
            route_kind_for_review_action(&resolution.next_action.r#type),
            resolution.next_action.reason.clone(),
        ),
        "retry_browser_environment" => (
            RouteActionKind::ContinueExecution,
            "Retry MCP browser environment preparation.".to_string(),
        ),
        "submit_external_browser_evidence" => (
            RouteActionKind::Review,
            "Re-run Review with accepted external browser evidence.".to_string(),
        ),
        _ => {
            let change = resolution
                .change_request
                .as_ref()
                .expect("validated request_changes has changeRequest");
            (
                route_kind_for_review_action(&change.route),
                format!("{}: {}", change.route, change.reason),
            )
        }
    };
    let target_task_ids = manual_review_target_task_ids(resolution);
    let next_action =
        route_next_action_with_target_task_ids(&resolution.next_action, &target_task_ids);
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
            "nextAction": next_action
        })),
        target_phase_id: resolution.next_action.target_phase_id.clone(),
    }
}

fn route_next_action_with_target_task_ids(
    next_action: &ReviewNextAction,
    target_task_ids: &[String],
) -> ReviewNextAction {
    let mut next_action = next_action.clone();
    if next_action.target_task_ids.is_empty() && !target_task_ids.is_empty() {
        next_action.target_task_ids = target_task_ids.to_vec();
    }
    next_action
}

fn manual_review_target_task_ids(resolution: &ManualReviewResolution) -> Vec<String> {
    dedupe_non_empty(resolution.next_action.target_task_ids.clone())
}

fn dedupe_non_empty(mut values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values.retain(|value| !value.is_empty() && seen.insert(value.clone()));
    values
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
    let next_kind = route_kind_for_review_action(&result.next_action.r#type);
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        phase
            .latest_refs
            .insert("reviewResult".to_string(), result_ref.to_string());
        phase.next_action = Some(RouteAction {
            kind: next_kind.clone(),
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
    if next_kind == RouteActionKind::Done {
        delivery.status = DeliveryLifecycleStatus::Completed;
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
    if effective_action.kind == RouteActionKind::Done {
        delivery.status = DeliveryLifecycleStatus::Completed;
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

fn build_engineering_quality_review_matrix(
    task_plan: &TaskPlan,
    task_results: &[TaskResult],
) -> Vec<Value> {
    task_plan
        .engineering_quality_requirements
        .iter()
        .flat_map(|requirement| {
            requirement.applies_to_task_ids.iter().map(|task_id| {
                let result = task_results
                    .iter()
                    .find(|result| result.task_id == *task_id);
                let passed_verifications = result
                    .map(passed_verification_summaries)
                    .unwrap_or_default();
                let satisfied = result
                    .map(|result| {
                        matches!(
                            result.status,
                            contracts::TaskResultStatus::Completed
                                | contracts::TaskResultStatus::CompletedWithNotes
                        ) && !passed_verifications.is_empty()
                    })
                    .unwrap_or(false);
                json!({
                    "requirementId": requirement.requirement_id,
                    "kind": requirement.kind,
                    "taskId": task_id,
                    "taskResultId": result.map(|result| result.task_result_id.clone()),
                    "stackSignals": requirement.stack_signals,
                    "alignmentTargets": requirement.alignment_targets,
                    "riskFieldKinds": requirement.risk_field_kinds,
                    "verificationObligations": requirement.verification_obligations,
                    "passedVerificationSummaries": passed_verifications,
                    "requirementDetailEvidence": result
                        .map(compact_requirement_detail_evidence)
                        .unwrap_or_default(),
                    "qualitySatisfied": satisfied,
                    "recommendedNextAction": if satisfied { "none" } else { "execution_repair" }
                })
            })
        })
        .collect()
}

fn build_code_quality_review_matrix(
    task_plan: &TaskPlan,
    task_results: &[TaskResult],
) -> Vec<Value> {
    task_plan
        .code_quality_requirements
        .iter()
        .flat_map(|requirement| {
            requirement.applies_to_task_ids.iter().map(|task_id| {
                let result = task_results
                    .iter()
                    .find(|result| result.task_id == *task_id);
                let evidence = result.and_then(|result| {
                    result
                        .code_quality_evidence
                        .iter()
                        .find(|evidence| evidence.requirement_id == requirement.requirement_id)
                });
                let satisfied = result
                    .map(|result| {
                        matches!(
                            result.status,
                            contracts::TaskResultStatus::Completed
                                | contracts::TaskResultStatus::CompletedWithNotes
                        ) && evidence
                            .map(|evidence| {
                                let expected_paths = requirement
                                    .reference_load_plan
                                    .iter()
                                    .map(|item| item.path.as_str())
                                    .collect::<std::collections::BTreeSet<_>>();
                                let checked_paths = evidence
                                    .reference_files_checked
                                    .iter()
                                    .map(String::as_str)
                                    .collect::<std::collections::BTreeSet<_>>();
                                let files_satisfied = expected_paths.is_empty()
                                    || (expected_paths.is_subset(&checked_paths)
                                        && checked_paths.is_subset(&expected_paths));
                                evidence.status == "satisfied"
                                    && !evidence.reference_groups_checked.is_empty()
                                    && files_satisfied
                                    && !evidence.verification_ids.is_empty()
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                json!({
                    "requirementId": requirement.requirement_id,
                    "kind": requirement.kind,
                    "taskId": task_id,
                    "taskResultId": result.map(|result| result.task_result_id.clone()),
                    "referenceGroups": requirement.reference_groups,
                    "referenceLoadPlan": requirement.reference_load_plan,
                    "focusTags": requirement.focus_tags,
                    "implementationObligations": requirement.implementation_obligations,
                    "verificationObligations": requirement.verification_obligations,
                    "evidenceStatus": evidence.map(|evidence| evidence.status.clone()),
                    "referenceGroupsChecked": evidence
                        .map(|evidence| json!(evidence.reference_groups_checked.clone()))
                        .unwrap_or_else(|| json!({})),
                    "referenceFilesChecked": evidence
                        .map(|evidence| json!(evidence.reference_files_checked.clone()))
                        .unwrap_or_else(|| json!([])),
                    "knownGapCount": evidence
                        .map(|evidence| evidence.known_gaps.len())
                        .unwrap_or(0),
                    "qualitySatisfied": satisfied,
                    "recommendedNextAction": if satisfied { "none" } else { "execution_repair" }
                })
            })
        })
        .collect()
}

fn build_architecture_quality_review_matrix(
    task_plan: &TaskPlan,
    task_results: &[TaskResult],
    architecture_contract: Option<&ArchitectureArtifactContract>,
) -> Vec<Value> {
    let assigned_decisions = task_plan
        .tasks
        .iter()
        .flat_map(|task| task.write_boundary.artifact_refs.decisions.clone())
        .collect::<BTreeSet<_>>();
    let assigned_nfrs = task_plan
        .tasks
        .iter()
        .flat_map(|task| task.write_boundary.artifact_refs.nfrs.clone())
        .collect::<BTreeSet<_>>();
    let assigned_risks = task_plan
        .tasks
        .iter()
        .flat_map(|task| task.write_boundary.artifact_refs.risks.clone())
        .collect::<BTreeSet<_>>();
    let mut items = Vec::new();
    if let Some(aac) = architecture_contract {
        for decision in &aac.architecture_quality.decisions {
            if !assigned_decisions.contains(&decision.decision_id) {
                items.push(json!({
                    "requirementId": decision.decision_id,
                    "qualityKind": "decision",
                    "category": decision.category,
                    "taskId": Value::Null,
                    "taskResultId": Value::Null,
                    "missingTaskAssignment": true,
                    "qualitySatisfied": false,
                    "recommendedNextAction": "taskplan_repair"
                }));
            }
        }
        for nfr in &aac.architecture_quality.nfrs {
            if !assigned_nfrs.contains(&nfr.nfr_id) {
                items.push(json!({
                    "requirementId": nfr.nfr_id,
                    "qualityKind": "nfr",
                    "category": nfr.category,
                    "taskId": Value::Null,
                    "taskResultId": Value::Null,
                    "missingTaskAssignment": true,
                    "qualitySatisfied": false,
                    "recommendedNextAction": "taskplan_repair"
                }));
            }
        }
        for risk in &aac.architecture_quality.risks {
            if !assigned_risks.contains(&risk.risk_id) {
                items.push(json!({
                    "requirementId": risk.risk_id,
                    "qualityKind": "risk",
                    "category": risk.category,
                    "severity": risk.severity,
                    "taskId": Value::Null,
                    "taskResultId": Value::Null,
                    "missingTaskAssignment": true,
                    "qualitySatisfied": false,
                    "recommendedNextAction": "taskplan_repair"
                }));
            }
        }
    }
    for requirement in &task_plan.architecture_quality_requirements {
        for task_id in &requirement.applies_to_task_ids {
            let result = task_results
                .iter()
                .find(|result| result.task_id == *task_id);
            let evidence = result.and_then(|result| {
                result
                    .architecture_quality_evidence
                    .iter()
                    .find(|evidence| evidence.requirement_id == requirement.requirement_id)
            });
            let passed_verification_ids = result
                .map(|result| {
                    result
                        .verification_results
                        .iter()
                        .filter(|verification| verification.status == "passed")
                        .map(|verification| verification.verification_id.clone())
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let evidence_verifications = evidence
                .map(|evidence| evidence.verification_ids.clone())
                .unwrap_or_default();
            let verification_supported = !evidence_verifications.is_empty()
                && evidence_verifications
                    .iter()
                    .all(|id| passed_verification_ids.contains(id));
            let satisfied = evidence
                .map(|evidence| evidence.status == "satisfied" && verification_supported)
                .unwrap_or(false);
            items.push(json!({
                "requirementId": requirement.requirement_id,
                "qualityKind": requirement.kind,
                "taskId": task_id,
                "taskResultId": result.map(|result| result.task_result_id.clone()),
                "decisionRefs": requirement.decision_refs,
                "nfrRefs": requirement.nfr_refs,
                "riskRefs": requirement.risk_refs,
                "implementationObligations": requirement.implementation_obligations,
                "verificationObligations": requirement.verification_obligations,
                "architectureQualityEvidenceStatus": evidence.map(|evidence| evidence.status.clone()),
                "verificationSupported": verification_supported,
                "qualitySatisfied": satisfied,
                "missingTaskAssignment": false,
                "recommendedNextAction": if satisfied { "none" } else { "execution_repair" }
            }));
        }
    }
    items
}

fn build_api_contract_review_matrix(
    task_plan: &TaskPlan,
    task_results: &[TaskResult],
) -> Vec<Value> {
    task_plan
        .api_contract_requirements
        .iter()
        .flat_map(|requirement| {
            requirement.applies_to_task_ids.iter().map(|task_id| {
                let result = task_results
                    .iter()
                    .find(|result| result.task_id == *task_id);
                let evidence = result.and_then(|result| {
                    result
                        .api_contract_evidence
                        .iter()
                        .find(|evidence| evidence.requirement_id == requirement.requirement_id)
                });
                let passed_verification_ids = result
                    .map(|result| {
                        result
                            .verification_results
                            .iter()
                            .filter(|verification| verification.status == "passed")
                            .map(|verification| verification.verification_id.clone())
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                let evidence_verifications = evidence
                    .map(|evidence| evidence.verification_ids.clone())
                    .unwrap_or_default();
                let verification_supported = !evidence_verifications.is_empty()
                    && evidence_verifications
                        .iter()
                        .all(|id| passed_verification_ids.contains(id));
                let satisfied = evidence
                    .map(|evidence| {
                        evidence.status == "satisfied"
                            && evidence.known_gaps.is_empty()
                            && verification_supported
                    })
                    .unwrap_or(false);
                json!({
                    "requirementId": requirement.requirement_id,
                    "qualityKind": requirement.kind,
                    "taskId": task_id,
                    "taskResultId": result.map(|result| result.task_result_id.clone()),
                    "interfaceRefs": requirement.interface_refs,
                    "implementationObligations": requirement.implementation_obligations,
                    "verificationObligations": requirement.verification_obligations,
                    "apiContractEvidenceStatus": evidence.map(|evidence| evidence.status.clone()),
                    "verificationSupported": verification_supported,
                    "knownGapCount": evidence.map(|evidence| evidence.known_gaps.len()).unwrap_or(0),
                    "contractSatisfied": satisfied,
                    "recommendedNextAction": if satisfied { "none" } else { "execution_repair" }
                })
            })
        })
        .collect()
}

fn compact_api_contract_context(
    task_plan: &TaskPlan,
    architecture_contract: Option<&ArchitectureArtifactContract>,
    project_api_contract: Option<&Value>,
) -> Value {
    let Some(aac) = architecture_contract else {
        return Value::Null;
    };
    let interface_refs = task_plan
        .api_contract_requirements
        .iter()
        .flat_map(|requirement| requirement.interface_refs.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let interface_refs = interface_refs.into_iter().collect::<Vec<_>>();
    json!({
        "contract": exposure_projection(aac.api_contract_ref.as_deref(), project_api_contract),
        "interfaces": interfaces_for_refs(project_api_contract, &interface_refs)
            .iter()
            .map(compact_api_interface_for_review)
            .collect::<Vec<_>>()
    })
}

fn compact_api_interface_for_review(interface: &Value) -> Value {
    json!({
        "interfaceId": interface.get("interfaceId").cloned().unwrap_or(Value::Null),
        "method": interface.get("method").cloned().unwrap_or(Value::Null),
        "path": interface.get("path").cloned().unwrap_or(Value::Null),
        "operationKind": interface.get("operationKind").cloned().unwrap_or(Value::Null),
        "statusCodes": interface.get("statusCodes").cloned().unwrap_or(Value::Null),
        "requestFieldCount": interface
            .get("requestSchema")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "responseFieldCount": interface
            .get("responseSchema")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0)
    })
}

fn passed_verification_summaries(result: &TaskResult) -> Vec<Value> {
    result
        .verification_results
        .iter()
        .filter(|verification| verification.status == "passed")
        .map(|verification| {
            json!({
                "verificationId": verification.verification_id,
                "evidenceType": verification.evidence_type,
                "summary": compact_summary(&verification.summary)
            })
        })
        .collect()
}

fn compact_requirement_detail_evidence(result: &TaskResult) -> Vec<Value> {
    result
        .requirement_detail_evidence
        .iter()
        .map(|evidence| {
            json!({
                "detailId": evidence.detail_id,
                "status": evidence.status,
                "verificationIds": evidence.verification_ids,
                "summary": compact_summary(&evidence.summary)
            })
        })
        .collect()
}

fn build_frontend_quality_review_matrix(
    task_plan: &TaskPlan,
    task_results: &[TaskResult],
    architecture_contract: Option<&ArchitectureArtifactContract>,
) -> Vec<Value> {
    task_plan
        .tasks
        .iter()
        .filter_map(|task| {
            let requirement = task.frontend_experience_requirement.as_ref()?;
            let surface_contract =
                task_scoped_surface_contract_for_review(requirement, architecture_contract);
            if !surface_contract.is_object() {
                return None;
            }
            let result = task_results
                .iter()
                .find(|result| result.task_id == task.task_id);
            let self_check = result
                .and_then(|result| result.frontend_quality_self_check.as_ref())
                .map(serde_json::to_value)
                .and_then(Result::ok)
                .unwrap_or(Value::Null);
            let surface_review =
                frontend_surface_contract_review(requirement, &surface_contract, &self_check);
            let forbidden_violation_count = self_check
                .pointer("/contentBoundaryEvidence/forbiddenContentViolations")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let token_plan = requirement
                .pointer("/executionGuidance/styleAssetPlan/designTokenAssetPlan")
                .or_else(|| surface_contract.get("designTokenAssetPlan"))
                .unwrap_or(&Value::Null);
            let token_evidence = self_check
                .get("designTokenEvidence")
                .cloned()
                .unwrap_or(Value::Null);
            let token_strategy = token_plan
                .get("strategy")
                .and_then(Value::as_str)
                .unwrap_or("not_applicable");
            let token_strategy_matches = token_evidence
                .get("strategyUsed")
                .and_then(Value::as_str)
                == Some(token_strategy);
            let token_template_matches =
                token_evidence.get("templateIdUsed").unwrap_or(&Value::Null)
                    == token_plan.get("templateId").unwrap_or(&Value::Null);
            let token_asset_file_count = token_evidence
                .get("tokenAssetFiles")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let token_consumer_file_count = token_evidence
                .get("tokenConsumerFiles")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let parallel_token_system_created = token_evidence
                .get("parallelTokenSystemCreated")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let token_merge_summary_present = token_evidence
                .get("mergeSummary")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|summary| !summary.is_empty());
            let token_asset_satisfied = token_strategy_matches
                && token_template_matches
                && !parallel_token_system_created
                && (token_strategy == "not_applicable"
                    || (token_asset_file_count > 0 && token_merge_summary_present));
            let known_gap_count = self_check
                .get("knownGaps")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let expected_browser_checks = task_plan
                .browser_verification_profiles
                .iter()
                .flat_map(|profile| profile.checks.iter())
                .filter(|check| check.source_task_id == task.task_id)
                .collect::<Vec<_>>();
            let expected_browser_check_ids = expected_browser_checks
                .iter()
                .map(|check| check.check_id.clone())
                .collect::<BTreeSet<_>>();
            let required_browser_check_ids = expected_browser_checks
                .iter()
                .filter(|check| check.enforcement == contracts::BrowserEvidenceEnforcement::Required)
                .map(|check| check.check_id.clone())
                .collect::<BTreeSet<_>>();
            let passed_browser_check_ids = task_results
                .iter()
                .flat_map(|result| result.verification_results.iter())
                .flat_map(|verification| verification.browser_checks.iter())
                .filter(|check| check.status == contracts::BrowserCheckStatus::Passed)
                .map(|check| check.check_id.clone())
                .collect::<BTreeSet<_>>();
            let blocked_browser_check_ids = task_results
                .iter()
                .flat_map(|result| result.verification_results.iter())
                .flat_map(|verification| verification.browser_checks.iter())
                .filter(|check| check.status == contracts::BrowserCheckStatus::Blocked)
                .map(|check| check.check_id.clone())
                .collect::<BTreeSet<_>>();
            let browser_closure_result = task_results.iter().find(|result| {
                result
                    .verification_results
                    .iter()
                    .flat_map(|verification| verification.browser_checks.iter())
                    .any(|check| expected_browser_check_ids.contains(&check.check_id))
            });
            let browser_verification_satisfied =
                required_browser_check_ids.is_subset(&passed_browser_check_ids);
            let required_browser_environment_blocked = required_browser_check_ids
                .intersection(&blocked_browser_check_ids)
                .next()
                .is_some();
            let surface_contract_satisfied = surface_review
                .get("satisfied")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let quality_satisfied = self_check.get("status").and_then(Value::as_str)
                == Some("satisfied")
                && surface_contract_satisfied
                && token_asset_satisfied
                && browser_verification_satisfied
                && forbidden_violation_count == 0
                && known_gap_count == 0;
            Some(json!({
                "taskId": task.task_id,
                "taskResultId": result.map(|result| result.task_result_id.clone()),
                "surfaceDecisionContractRef": requirement.get("uiSurfaceDecisionContractRef").cloned().unwrap_or(Value::Null),
                "actualStatus": self_check.get("status").and_then(Value::as_str),
                "qualitySatisfied": quality_satisfied,
                "surfaceContractCoverage": surface_review,
                "designTokenAsset": {
                    "strategy": token_strategy,
                    "templateId": token_plan.get("templateId").cloned().unwrap_or(Value::Null),
                    "strategyMatches": token_strategy_matches,
                    "templateMatches": token_template_matches,
                    "tokenAssetFileCount": token_asset_file_count,
                    "tokenConsumerFileCount": token_consumer_file_count,
                    "mergeSummaryPresent": token_merge_summary_present,
                    "parallelTokenSystemCreated": parallel_token_system_created,
                    "satisfied": token_asset_satisfied
                },
                "browserVerification": {
                    "closureTaskId": browser_closure_result.map(|result| result.task_id.clone()),
                    "closureTaskResultId": browser_closure_result.map(|result| result.task_result_id.clone()),
                    "expectedCheckCount": expected_browser_check_ids.len(),
                    "requiredCheckCount": required_browser_check_ids.len(),
                    "requiredCheckIds": required_browser_check_ids.iter().cloned().collect::<Vec<_>>(),
                    "passedCheckCount": expected_browser_check_ids
                        .intersection(&passed_browser_check_ids)
                        .count(),
                    "requiredBlockedCount": required_browser_check_ids
                        .intersection(&blocked_browser_check_ids)
                        .count(),
                    "environmentBlocked": required_browser_environment_blocked,
                    "satisfied": browser_verification_satisfied
                },
                "forbiddenViolationCount": forbidden_violation_count,
                "knownGapCount": known_gap_count,
                "recommendedNextAction": if quality_satisfied {
                    "none"
                } else if required_browser_environment_blocked {
                    "manual_review"
                } else {
                    "execution_repair"
                }
            }))
        })
        .collect()
}

fn task_scoped_surface_contract_for_review(
    requirement: &Value,
    architecture_contract: Option<&ArchitectureArtifactContract>,
) -> Value {
    if let Some(contract) = requirement
        .pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
        .filter(|value| value.is_object())
    {
        return contract.clone();
    }

    let full_contract = architecture_contract
        .and_then(|contract| contract.frontend_experience.as_ref())
        .and_then(|frontend| frontend.get("uiSurfaceDecisionContract"))
        .cloned()
        .unwrap_or(Value::Null);
    if !full_contract.is_object() {
        return Value::Null;
    }

    let ownership = requirement
        .get("uiSurfaceOwnership")
        .unwrap_or(&Value::Null);
    let region_ids = string_array_field(ownership, "regionIdsInScope");
    let action_ids = string_array_field(ownership, "actionIdsInScope");
    let state_ids = string_array_field(ownership, "stateKindsInScope");
    let quality_rule_ids = string_array_field(ownership, "qualityRuleIdsInScope");
    json!({
        "contractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
        "selectionMode": if region_ids.is_empty()
            && action_ids.is_empty()
            && state_ids.is_empty()
            && quality_rule_ids.is_empty()
        {
            "all_when_task_scope_empty"
        } else {
            "task_scope"
        },
        "patternDecision": full_contract.get("patternDecision").cloned().unwrap_or(Value::Null),
        "semanticFacts": full_contract.get("semanticFacts").cloned().unwrap_or(Value::Null),
        "layoutModel": full_contract.get("layoutModel").cloned().unwrap_or(Value::Null),
        "regionsInScope": selected_surface_contract_values(&full_contract, "regionModel", "regionId", &region_ids),
        "informationModel": full_contract.get("informationModel").cloned().unwrap_or(Value::Null),
        "actionsInScope": selected_surface_contract_values(&full_contract, "actionModel", "actionId", &action_ids),
        "statesInScope": selected_surface_contract_values(&full_contract, "stateModel", "state", &state_ids),
        "compositionConstraints": full_contract.get("compositionConstraints").cloned().unwrap_or(Value::Null),
        "contentBoundary": full_contract.get("contentBoundary").cloned().unwrap_or(Value::Null),
        "qualityRulesInScope": selected_surface_contract_values(&full_contract, "qualityRules", "ruleId", &quality_rule_ids),
        "designTokenAssetPlan": full_contract.get("designTokenAssetPlan").cloned().unwrap_or(Value::Null),
        "referencePlan": full_contract.get("referencePlan").cloned().unwrap_or_else(|| json!([]))
    })
}

fn selected_surface_contract_values(
    contract: &Value,
    array_key: &str,
    id_key: &str,
    ids: &[String],
) -> Vec<Value> {
    let values = contract
        .get(array_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return values.into_iter().cloned().collect();
    }
    let selected = ids.iter().cloned().collect::<BTreeSet<_>>();
    values
        .into_iter()
        .filter(|value| {
            value
                .get(id_key)
                .and_then(Value::as_str)
                .is_some_and(|id| selected.contains(id))
        })
        .cloned()
        .collect()
}

fn frontend_surface_contract_review(
    requirement: &Value,
    surface_contract: &Value,
    self_check: &Value,
) -> Value {
    if !surface_contract.is_object() {
        return json!({
            "present": false,
            "satisfied": true
        });
    }

    let expected_ref = requirement
        .get("uiSurfaceDecisionContractRef")
        .and_then(Value::as_str)
        .or_else(|| surface_contract.get("contractRef").and_then(Value::as_str));
    let actual_ref = self_check
        .get("surfaceDecisionContractRef")
        .and_then(Value::as_str);
    let ref_matches = expected_ref
        .map(|expected| actual_ref == Some(expected))
        .unwrap_or(true);
    let region_review = surface_evidence_review(
        &object_array_string_field(surface_contract, "regionsInScope", "regionId"),
        self_check,
        "surfaceRegionEvidence",
    );
    let action_review = surface_evidence_review(
        &object_array_string_field(surface_contract, "actionsInScope", "actionId"),
        self_check,
        "surfaceActionEvidence",
    );
    let state_review = surface_evidence_review(
        &object_array_string_field(surface_contract, "statesInScope", "state"),
        self_check,
        "surfaceStateEvidence",
    );
    let quality_rule_review = surface_evidence_review(
        &object_array_string_field(surface_contract, "qualityRulesInScope", "ruleId"),
        self_check,
        "surfaceQualityRuleEvidence",
    );
    let expected_reference_plan_files = reference_plan_paths(requirement, surface_contract);
    let checked_reference_plan_files = string_array_field(self_check, "referencePlanFilesChecked");
    let missing_reference_plan_files = missing_strings(
        &expected_reference_plan_files,
        &checked_reference_plan_files,
    );
    let content_checked = self_check
        .pointer("/contentBoundaryEvidence/checked")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content_evidence_present = self_check
        .pointer("/contentBoundaryEvidence/evidence")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|evidence| !evidence.is_empty());
    let content_violation_count = self_check
        .pointer("/contentBoundaryEvidence/forbiddenContentViolations")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let content_boundary_satisfied =
        content_checked && content_evidence_present && content_violation_count == 0;
    let satisfied = ref_matches
        && review_satisfied(&region_review)
        && review_satisfied(&action_review)
        && review_satisfied(&state_review)
        && review_satisfied(&quality_rule_review)
        && missing_reference_plan_files.is_empty()
        && content_boundary_satisfied;

    json!({
        "present": true,
        "contractRefExpected": expected_ref,
        "contractRefActual": actual_ref,
        "contractRefMatches": ref_matches,
        "regions": region_review,
        "actions": action_review,
        "states": state_review,
        "qualityRules": quality_rule_review,
        "referencePlanFileCount": expected_reference_plan_files.len(),
        "referencePlanFilesCheckedCount": checked_reference_plan_files.len(),
        "missingReferencePlanFiles": missing_reference_plan_files,
        "contentBoundary": {
            "checked": content_checked,
            "evidencePresent": content_evidence_present,
            "violationCount": content_violation_count,
            "satisfied": content_boundary_satisfied
        },
        "satisfied": satisfied
    })
}

fn surface_evidence_review(
    expected_ids: &[String],
    self_check: &Value,
    evidence_field: &str,
) -> Value {
    let evidence = self_check
        .get(evidence_field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let expected_set = expected_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut checked_ids = Vec::new();
    let mut invented_ids = Vec::new();
    let mut duplicate_ids = Vec::new();
    let mut non_satisfied_ids = Vec::new();
    let mut missing_file_ids = Vec::new();
    let mut empty_evidence_ids = Vec::new();
    for item in &evidence {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            empty_evidence_ids.push("missing_id".to_string());
            continue;
        };
        checked_ids.push(id.to_string());
        if !seen.insert(id.to_string()) {
            duplicate_ids.push(id.to_string());
        }
        if !expected_set.contains(id) {
            invented_ids.push(id.to_string());
        }
        if item.get("status").and_then(Value::as_str) != Some("satisfied") {
            non_satisfied_ids.push(id.to_string());
        }
        if string_array_field(item, "files").is_empty() {
            missing_file_ids.push(id.to_string());
        }
        if item
            .get("evidence")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_none_or(str::is_empty)
        {
            empty_evidence_ids.push(id.to_string());
        }
    }
    let missing_ids = missing_strings(expected_ids, &checked_ids);
    let satisfied = missing_ids.is_empty()
        && invented_ids.is_empty()
        && duplicate_ids.is_empty()
        && non_satisfied_ids.is_empty()
        && missing_file_ids.is_empty()
        && empty_evidence_ids.is_empty();
    json!({
        "expectedIds": expected_ids,
        "checkedIds": checked_ids,
        "missingIds": missing_ids,
        "inventedIds": invented_ids,
        "duplicateIds": duplicate_ids,
        "nonSatisfiedIds": non_satisfied_ids,
        "missingFileIds": missing_file_ids,
        "emptyEvidenceIds": empty_evidence_ids,
        "satisfied": satisfied
    })
}

fn review_satisfied(review: &Value) -> bool {
    review
        .get("satisfied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn compact_review_matrix_summary(
    concept_matrix: &[Value],
    detail_matrix: &[Value],
    engineering_quality_matrix: &[Value],
    architecture_quality_matrix: &[Value],
    api_contract_matrix: &[Value],
    code_quality_matrix: &[Value],
    frontend_quality_matrix: &[Value],
) -> Value {
    json!({
        "concept": concept_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "conceptRef": item.get("conceptRef").cloned().unwrap_or(Value::Null),
                "status": item.get("status").cloned().unwrap_or(Value::Null),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>(),
        "detail": detail_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "detailId": item.get("detailId").cloned().unwrap_or(Value::Null),
                "detailSatisfied": item.get("detailSatisfied").cloned().unwrap_or(Value::Null),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>(),
        "engineeringQuality": engineering_quality_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "requirementId": item.get("requirementId").cloned().unwrap_or(Value::Null),
                "qualitySatisfied": item.get("qualitySatisfied").cloned().unwrap_or(Value::Null),
                "missingAlignmentTargetCount": item
                    .get("missingAlignmentTargets")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>(),
        "architectureQuality": architecture_quality_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "requirementId": item.get("requirementId").cloned().unwrap_or(Value::Null),
                "qualityKind": item.get("qualityKind").cloned().unwrap_or(Value::Null),
                "qualitySatisfied": item.get("qualitySatisfied").cloned().unwrap_or(Value::Null),
                "missingTaskAssignment": item.get("missingTaskAssignment").cloned().unwrap_or(Value::Bool(false)),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>(),
        "apiContract": api_contract_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "requirementId": item.get("requirementId").cloned().unwrap_or(Value::Null),
                "interfaceRefs": item.get("interfaceRefs").cloned().unwrap_or_else(|| json!([])),
                "contractSatisfied": item.get("contractSatisfied").cloned().unwrap_or(Value::Null),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>(),
        "codeQuality": code_quality_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "requirementId": item.get("requirementId").cloned().unwrap_or(Value::Null),
                "referenceGroupCount": reference_group_entry_count(
                    item.get("referenceGroups").unwrap_or(&Value::Null)
                ),
                "referenceFileCount": item
                    .get("referenceLoadPlan")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "qualitySatisfied": item.get("qualitySatisfied").cloned().unwrap_or(Value::Null),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>(),
        "frontendQuality": frontend_quality_matrix.iter().map(|item| {
            json!({
                "taskId": item.get("taskId").cloned().unwrap_or(Value::Null),
                "taskResultId": item.get("taskResultId").cloned().unwrap_or(Value::Null),
                "qualitySatisfied": item.get("qualitySatisfied").cloned().unwrap_or(Value::Null),
                "browserVerification": item.get("browserVerification").cloned().unwrap_or_else(|| json!({})),
                "surfaceContractSatisfied": item
                    .pointer("/surfaceContractCoverage/satisfied")
                    .cloned()
                    .unwrap_or(Value::Null),
                "missingSurfaceRegionCount": item
                    .pointer("/surfaceContractCoverage/regions/missingIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingSurfaceActionCount": item
                    .pointer("/surfaceContractCoverage/actions/missingIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingSurfaceStateCount": item
                    .pointer("/surfaceContractCoverage/states/missingIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingSurfaceQualityRuleCount": item
                    .pointer("/surfaceContractCoverage/qualityRules/missingIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingReferencePlanFileCount": item
                    .pointer("/surfaceContractCoverage/missingReferencePlanFiles")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "forbiddenViolationCount": item.get("forbiddenViolationCount").cloned().unwrap_or_else(|| json!(0)),
                "knownGapCount": item.get("knownGapCount").cloned().unwrap_or_else(|| json!(0)),
                "recommendedNextAction": item.get("recommendedNextAction").cloned().unwrap_or(Value::Null)
            })
        }).collect::<Vec<_>>()
    })
}

fn build_review_signals(
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    task_results: &[TaskResult],
    architecture_contract: Option<&ArchitectureArtifactContract>,
) -> Value {
    let mut signals = vec![json!({
        "signalId": "sig-task-run-summary",
        "kind": "task_run_summary",
        "status": run.status,
        "totalTasks": run.summary.total,
        "completedTasks": run.summary.completed,
        "failedTasks": run.summary.failed,
        "blockedTasks": run.summary.blocked
    })];
    for detail in build_detail_review_matrix(task_plan, task_results) {
        let detail_id = detail
            .get("detailId")
            .and_then(Value::as_str)
            .unwrap_or("detail");
        let detail_satisfied = detail
            .get("detailSatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!("sig-requirement-detail-{}", safe_signal_id(detail_id)),
            "kind": "requirement_detail_evidence",
            "detailId": detail_id,
            "taskRefs": detail.get("taskId").and_then(Value::as_str).map(|task_id| vec![task_id.to_string()]).unwrap_or_default(),
            "detailSatisfied": detail_satisfied,
            "actualStatus": if detail_satisfied { "satisfied" } else { "missing" },
            "recommendedNextAction": if detail_satisfied { "none" } else { "execution_repair" },
            "reason": if detail_satisfied {
                "Assigned TaskResult evidence reports this requirement detail as satisfied."
            } else {
                "Assigned TaskResult evidence is missing or does not report this requirement detail as satisfied."
            }
        }));
    }
    for quality in
        build_frontend_quality_review_matrix(task_plan, task_results, architecture_contract)
    {
        let task_id = quality
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap_or("task");
        let quality_satisfied = quality
            .get("qualitySatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let browser_environment_blocked = quality
            .pointer("/browserVerification/environmentBlocked")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!("sig-frontend-ui-quality-{}", safe_signal_id(task_id)),
            "kind": "frontend_ui_quality",
            "taskRefs": [task_id],
            "taskResultId": quality.get("taskResultId").cloned().unwrap_or(Value::Null),
            "uiQualitySatisfied": quality_satisfied,
            "actualStatus": quality.get("actualStatus").cloned().unwrap_or(Value::Null),
            "surfaceContractCoverage": quality.get("surfaceContractCoverage").cloned().unwrap_or_else(|| json!({})),
            "designTokenAsset": quality.get("designTokenAsset").cloned().unwrap_or_else(|| json!({})),
            "forbiddenViolationCount": quality.get("forbiddenViolationCount").cloned().unwrap_or_else(|| json!(0)),
            "knownGapCount": quality.get("knownGapCount").cloned().unwrap_or_else(|| json!(0)),
            "browserVerification": quality.get("browserVerification").cloned().unwrap_or_else(|| json!({})),
            "recommendedNextAction": if quality_satisfied {
                "none"
            } else if browser_environment_blocked {
                "manual_review"
            } else {
                "execution_repair"
            },
            "reason": if quality_satisfied {
                "TaskResult frontend quality self-check satisfies the task UI surface contract."
            } else if browser_environment_blocked {
                "Required browser evidence is unavailable on both host and managed container; this is an environment quality gate, not a product-code repair."
            } else {
                "TaskResult frontend quality self-check does not satisfy the task UI surface contract."
            }
        }));
    }
    for item in build_engineering_quality_review_matrix(task_plan, task_results) {
        let requirement_id = item
            .get("requirementId")
            .and_then(Value::as_str)
            .unwrap_or("engineering_quality");
        let task_id = item.get("taskId").and_then(Value::as_str).unwrap_or("task");
        let quality_satisfied = item
            .get("qualitySatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!(
                "sig-engineering-quality-{}-{}",
                safe_signal_id(requirement_id),
                safe_signal_id(task_id)
            ),
            "kind": "engineering_quality",
            "requirementId": requirement_id,
            "qualityKind": item.get("kind").cloned().unwrap_or(Value::Null),
            "taskRefs": [task_id],
            "taskResultId": item.get("taskResultId").cloned().unwrap_or(Value::Null),
            "qualitySatisfied": quality_satisfied,
            "alignmentTargetCount": item
                .get("alignmentTargets")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
            "riskFieldKindCount": item
                .get("riskFieldKinds")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
            "passedVerificationCount": item
                .get("passedVerificationSummaries")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
            "recommendedNextAction": if quality_satisfied { "none" } else { "execution_repair" },
            "reason": if quality_satisfied {
                "TaskResult contains passed verification evidence for the referenced engineering quality requirement."
            } else {
                "TaskResult is missing passed verification evidence for the referenced engineering quality requirement."
            }
        }));
    }
    for item in
        build_architecture_quality_review_matrix(task_plan, task_results, architecture_contract)
    {
        let requirement_id = item
            .get("requirementId")
            .and_then(Value::as_str)
            .unwrap_or("architecture_quality");
        let task_id = item.get("taskId").and_then(Value::as_str).unwrap_or("task");
        let quality_satisfied = item
            .get("qualitySatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let missing_task_assignment = item
            .get("missingTaskAssignment")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!(
                "sig-architecture-quality-{}-{}",
                safe_signal_id(requirement_id),
                safe_signal_id(task_id)
            ),
            "kind": "architecture_quality",
            "requirementId": requirement_id,
            "qualityKind": item.get("qualityKind").cloned().unwrap_or(Value::Null),
            "taskRefs": item.get("taskId").and_then(Value::as_str).map(|task_id| vec![task_id.to_string()]).unwrap_or_default(),
            "taskResultId": item.get("taskResultId").cloned().unwrap_or(Value::Null),
            "decisionRefs": item.get("decisionRefs").cloned().unwrap_or_else(|| json!([])),
            "nfrRefs": item.get("nfrRefs").cloned().unwrap_or_else(|| json!([])),
            "riskRefs": item.get("riskRefs").cloned().unwrap_or_else(|| json!([])),
            "architectureQualitySatisfied": quality_satisfied,
            "missingTaskAssignment": missing_task_assignment,
            "recommendedNextAction": if missing_task_assignment {
                "taskplan_repair"
            } else if quality_satisfied {
                "none"
            } else {
                "execution_repair"
            },
            "reason": if missing_task_assignment {
                "TaskPlan does not assign this architecture quality item to an implementation task."
            } else if quality_satisfied {
                "TaskResult contains supported architecture quality evidence for the referenced requirement."
            } else {
                "TaskResult is missing supported architecture quality evidence for the referenced requirement."
            }
        }));
    }
    for item in build_api_contract_review_matrix(task_plan, task_results) {
        let requirement_id = item
            .get("requirementId")
            .and_then(Value::as_str)
            .unwrap_or("api_contract");
        let task_id = item.get("taskId").and_then(Value::as_str).unwrap_or("task");
        let contract_satisfied = item
            .get("contractSatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!(
                "sig-api-contract-{}-{}",
                safe_signal_id(requirement_id),
                safe_signal_id(task_id)
            ),
            "kind": "api_contract",
            "requirementId": requirement_id,
            "qualityKind": item.get("qualityKind").cloned().unwrap_or(Value::Null),
            "taskRefs": [task_id],
            "taskResultId": item.get("taskResultId").cloned().unwrap_or(Value::Null),
            "interfaceRefs": item.get("interfaceRefs").cloned().unwrap_or_else(|| json!([])),
            "apiContractSatisfied": contract_satisfied,
            "knownGapCount": item.get("knownGapCount").cloned().unwrap_or_else(|| json!(0)),
            "recommendedNextAction": if contract_satisfied { "none" } else { "execution_repair" },
            "reason": if contract_satisfied {
                "TaskResult contains supported API contract evidence for the referenced requirement."
            } else {
                "TaskResult is missing supported API contract evidence for the referenced requirement."
            }
        }));
    }
    for item in build_code_quality_review_matrix(task_plan, task_results) {
        let requirement_id = item
            .get("requirementId")
            .and_then(Value::as_str)
            .unwrap_or("code_quality");
        let task_id = item.get("taskId").and_then(Value::as_str).unwrap_or("task");
        let quality_satisfied = item
            .get("qualitySatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!(
                "sig-code-quality-{}-{}",
                safe_signal_id(requirement_id),
                safe_signal_id(task_id)
            ),
            "kind": "code_quality",
            "requirementId": requirement_id,
            "qualityKind": item.get("kind").cloned().unwrap_or(Value::Null),
            "taskRefs": [task_id],
            "taskResultId": item.get("taskResultId").cloned().unwrap_or(Value::Null),
            "referenceGroupCount": reference_group_entry_count(
                item.get("referenceGroups").unwrap_or(&Value::Null)
            ),
            "referenceFileCount": item
                .get("referenceLoadPlan")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
            "referenceGroupCheckedCount": reference_group_entry_count(
                item.get("referenceGroupsChecked").unwrap_or(&Value::Null)
            ),
            "referenceFileCheckedCount": item
                .get("referenceFilesChecked")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
            "codeQualitySatisfied": quality_satisfied,
            "knownGapCount": item.get("knownGapCount").cloned().unwrap_or_else(|| json!(0)),
            "recommendedNextAction": if quality_satisfied { "none" } else { "execution_repair" },
            "reason": if quality_satisfied {
                "TaskResult contains supported code quality evidence for selected language/framework references."
            } else {
                "TaskResult is missing supported code quality evidence for selected language/framework references."
            }
        }));
    }
    if let Some(architecture_contract) = architecture_contract {
        for requirement in crate::task_plan::workflow_closure_requirements(architecture_contract) {
            if task_plan
                .tasks
                .iter()
                .any(|task| crate::task_plan::task_covers_workflow_closure(task, &requirement))
            {
                continue;
            }
            let closure_id = requirement
                .get("closureId")
                .and_then(Value::as_str)
                .unwrap_or("workflow_closure");
            signals.push(json!({
                "signalId": format!("sig-workflow-closure-missing-{}", safe_signal_id(closure_id)),
                "kind": "frontend_workflow_closure",
                "closureId": closure_id,
                "workflowRef": requirement.get("workflowRef").and_then(Value::as_str),
                "interfaceRefs": value_string_array(&requirement, "interfaceRefs"),
                "acceptanceRefs": value_string_array(&requirement, "acceptanceRefs"),
                "taskRefs": [],
                "closureSatisfied": false,
                "missingTaskAssignment": true,
                "recommendedNextAction": "taskplan_repair",
                "reason": "No TaskPlan task structurally covers this workflow closure requirement."
            }));
        }
    }
    for task in &task_plan.tasks {
        if task.frontend_experience_requirement.is_some() {
            signals.push(json!({
                "signalId": format!("sig-frontend-task-{}", safe_signal_id(&task.task_id)),
                "kind": "task_contract_presence",
                "taskId": task.task_id,
                "contractType": "frontend_experience"
            }));
        }
        if let Some(requirement) = &task.runtime_delivery_requirement {
            signals.push(json!({
                "signalId": format!("sig-runtime-task-{}", safe_signal_id(&task.task_id)),
                "kind": "task_contract_presence",
                "taskId": task.task_id,
                "contractType": "runtime_delivery",
                "isClosureTask": matches!(task.task_kind, contracts::TaskKind::RuntimeDeliveryClosure),
                "affectedContractFields": requirement.affected_contract_fields,
                "requiredCodeLevelChecks": requirement.required_code_level_checks.iter().map(|check| check.check_id.clone()).collect::<Vec<_>>()
            }));
        }
        for closure_id in frontend_closure_ids(task) {
            let result = task_results
                .iter()
                .find(|result| result.task_id == task.task_id);
            let check = result.and_then(|result| result.frontend_experience_self_check.as_ref());
            let data_binding = check
                .and_then(|check| check.get("dataBinding"))
                .unwrap_or(&Value::Null);
            let known_gap_count = data_binding
                .get("knownGaps")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0);
            let covered_closures = check
                .and_then(|check| check.get("closureRequirementIds"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<BTreeSet<_>>();
            let closure_satisfied = check
                .and_then(|check| check.get("status"))
                .and_then(Value::as_str)
                == Some("satisfied")
                && data_binding.get("mode").and_then(Value::as_str) == Some("wired")
                && known_gap_count == 0
                && covered_closures.contains(&closure_id);
            signals.push(json!({
                "signalId": format!("sig-workflow-closure-{}-{}", safe_signal_id(&closure_id), safe_signal_id(&task.task_id)),
                "kind": "frontend_workflow_closure",
                "closureId": closure_id,
                "taskRefs": [task.task_id.clone()],
                "taskResultId": result.map(|result| result.task_result_id.clone()),
                "closureSatisfied": closure_satisfied,
                "actualFrontendSelfCheckStatus": check.and_then(|check| check.get("status")).and_then(Value::as_str),
                "actualDataBindingMode": data_binding.get("mode").and_then(Value::as_str),
                "knownGapCount": known_gap_count,
                "requiredDataBindingMode": "wired",
                "recommendedNextAction": if closure_satisfied { "none" } else { "execution_repair" },
                "reason": if closure_satisfied {
                    "TaskResult self-check reports wired closure evidence with no known gaps."
                } else {
                    "Required workflow closure is not satisfied by TaskResult frontend self-check evidence."
                }
            }));
        }
    }
    for result in task_results {
        if result.runtime_delivery_evidence.is_some() {
            signals.push(json!({
                "signalId": format!("sig-runtime-evidence-{}", safe_signal_id(&result.task_result_id)),
                "kind": "task_result_evidence_presence",
                "taskResultId": result.task_result_id,
                "taskId": result.task_id,
                "evidenceType": "runtime_delivery"
            }));
        }
        if result.frontend_experience_self_check.is_some() {
            signals.push(json!({
                "signalId": format!("sig-frontend-evidence-{}", safe_signal_id(&result.task_result_id)),
                "kind": "task_result_evidence_presence",
                "taskResultId": result.task_result_id,
                "taskId": result.task_id,
                "evidenceType": "frontend_experience"
            }));
        }
        if result.frontend_quality_self_check.is_some() {
            signals.push(json!({
                "signalId": format!("sig-frontend-quality-evidence-{}", safe_signal_id(&result.task_result_id)),
                "kind": "task_result_evidence_presence",
                "taskResultId": result.task_result_id,
                "taskId": result.task_id,
                "evidenceType": "frontend_quality"
            }));
        }
    }
    Value::Array(signals)
}

fn value_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn reference_group_entry_count(value: &Value) -> usize {
    value
        .as_object()
        .into_iter()
        .flat_map(|object| object.values())
        .filter_map(Value::as_array)
        .map(Vec::len)
        .sum()
}

fn object_array_string_field(value: &Value, array_key: &str, field_key: &str) -> Vec<String> {
    value
        .get(array_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get(field_key)
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn string_array_field(value: &Value, array_key: &str) -> Vec<String> {
    value
        .get(array_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn reference_plan_paths(requirement: &Value, surface_contract: &Value) -> Vec<String> {
    let execution_paths = requirement
        .pointer("/executionGuidance/styleAssetPlan/referencePlan")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("path").and_then(Value::as_str).map(str::to_string))
        .collect::<Vec<_>>();
    if !execution_paths.is_empty() {
        return execution_paths;
    }
    surface_contract
        .get("referencePlan")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("path").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn missing_strings(expected: &[String], actual: &[String]) -> Vec<String> {
    let actual = actual.iter().collect::<BTreeSet<_>>();
    expected
        .iter()
        .filter(|item| !actual.contains(item))
        .cloned()
        .collect()
}

fn frontend_closure_ids(task: &TaskDefinition) -> Vec<String> {
    task.frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("executionGuidance"))
        .and_then(|guidance| guidance.get("closureRequirementRefs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str().map(str::to_string).or_else(|| {
                item.get("closureId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .collect()
}

fn safe_signal_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn compact_group_summaries(groups: &[TaskPlanGroup]) -> Vec<Value> {
    groups
        .iter()
        .map(|group| {
            json!({
                "groupId": group.group_id,
                "title": group.title,
                "objective": group.objective,
                "dependsOn": group.depends_on,
                "scopeRefs": group.scope_refs,
                "acceptanceRefs": group.acceptance_refs,
                "taskIds": group.task_ids
            })
        })
        .collect()
}

fn compact_task_summaries(tasks: &[TaskDefinition]) -> Vec<Value> {
    tasks
        .iter()
        .map(|task| {
            json!({
                "taskId": task.task_id,
                "groupId": task.group_id,
                "title": task.title,
                "taskKind": task.task_kind,
                "implementationActions": task.implementation_actions,
                "objective": task.objective,
                "dependsOn": task.depends_on,
                "scopeRefs": task.scope_refs,
                "acceptanceRefs": task.acceptance_refs,
                "requirementDetailRefs": task.requirement_detail_refs,
                "conceptRefs": task.concept_refs,
                "writeBoundary": task.write_boundary,
                "verificationIntents": task.verification_intents.iter().map(|intent| {
                    json!({
                        "verificationId": intent.verification_id,
                        "acceptanceRefs": intent.acceptance_refs,
                        "requirementDetailRefs": intent.requirement_detail_refs,
                        "preferredEvidence": intent.preferred_evidence,
                        "acceptableEvidence": intent.acceptable_evidence
                    })
                }).collect::<Vec<_>>(),
                "frontendExperienceRequired": task.frontend_experience_requirement.is_some(),
                "runtimeDeliveryRequired": task.runtime_delivery_requirement.is_some(),
                "engineeringQualityRequirementRefs": task.engineering_quality_requirement_refs,
                "architectureQualityRequirementRefs": task.architecture_quality_requirement_refs,
                "apiContractRequirementRefs": task.api_contract_requirement_refs,
                "codeQualityRequirementRefs": task.code_quality_requirement_refs
            })
        })
        .collect()
}

fn compact_task_result_summaries(task_results: &[TaskResult]) -> Vec<Value> {
    task_results
        .iter()
        .map(|result| {
            let mut summary = json!({
                "taskResultId": result.task_result_id,
                "taskId": result.task_id,
                "status": result.status,
                "changedFileCount": result.changed_files.len(),
                "verificationResults": result.verification_results.iter().map(|verification| {
                    json!({
                        "verificationId": verification.verification_id,
                        "status": verification.status,
                        "evidenceType": verification.evidence_type,
                        "browserChecks": verification.browser_checks.iter().map(|check| {
                            let diagnostic_artifact_refs = if check.status != contracts::BrowserCheckStatus::Passed
                                || check.attempts > 1
                            {
                                check.artifact_refs.clone()
                            } else {
                                Vec::new()
                            };
                            json!({
                                "checkId": check.check_id,
                                "status": check.status,
                                "attempts": check.attempts,
                                "retrySucceeded": check.status == contracts::BrowserCheckStatus::Passed && check.attempts > 1,
                                "artifactRefCount": check.artifact_refs.len(),
                                "diagnosticArtifactRefs": diagnostic_artifact_refs,
                                "command": compact_summary(&check.command),
                                "observedOutcome": compact_summary(&check.observed_outcome),
                                "blockedReason": check.blocked_reason
                            })
                        }).collect::<Vec<_>>()
                    })
                }).collect::<Vec<_>>(),
                "requirementDetailEvidence": result.requirement_detail_evidence.iter().map(|evidence| {
                    json!({
                        "detailId": evidence.detail_id,
                        "status": evidence.status,
                        "verificationIdCount": evidence.verification_ids.len(),
                        "evidenceRefCount": evidence.evidence_refs.len()
                    })
                }).collect::<Vec<_>>(),
                "conceptEvidence": result.concept_evidence.iter().map(|evidence| {
                    json!({
                        "conceptRef": evidence.concept_ref,
                        "evidenceType": evidence.evidence_type,
                        "refCount": evidence.refs.len()
                    })
                }).collect::<Vec<_>>(),
                "architectureQualityEvidence": result.architecture_quality_evidence.iter().map(|evidence| {
                    json!({
                        "requirementId": evidence.requirement_id,
                        "status": evidence.status,
                        "verificationIdCount": evidence.verification_ids.len()
                    })
                }).collect::<Vec<_>>(),
                "apiContractEvidence": result.api_contract_evidence.iter().map(|evidence| {
                    json!({
                        "requirementId": evidence.requirement_id,
                        "status": evidence.status,
                        "interfaceRefCount": evidence.interface_refs.len(),
                        "verificationIdCount": evidence.verification_ids.len(),
                        "knownGapCount": evidence.known_gaps.len()
                    })
                }).collect::<Vec<_>>(),
                "codeQualityEvidence": result.code_quality_evidence.iter().map(|evidence| {
                    json!({
                        "requirementId": evidence.requirement_id,
                        "status": evidence.status,
                        "referenceGroupCount": evidence
                            .reference_groups_checked
                            .values()
                            .map(Vec::len)
                            .sum::<usize>(),
                        "referenceFileCount": evidence.reference_files_checked.len(),
                        "verificationIdCount": evidence.verification_ids.len(),
                        "knownGapCount": evidence.known_gaps.len()
                    })
                }).collect::<Vec<_>>(),
                "frontendExperienceSelfCheckPresent": result.frontend_experience_self_check.is_some(),
                "frontendQualitySelfCheckPresent": result.frontend_quality_self_check.is_some(),
                "runtimeDeliveryEvidencePresent": result.runtime_delivery_evidence.is_some(),
                "blockedReasonCodes": result.blocked_reasons.iter().map(|reason| reason.code.clone()).collect::<Vec<_>>(),
                "failureCode": result.failure.as_ref().map(|failure| failure.code.clone()),
                "noChangeReasonCode": result.no_change_reason.as_ref().map(|reason| reason.code.clone())
            });
            if result.frontend_quality_self_check.is_some() {
                summary
                    .as_object_mut()
                    .expect("task result summary object")
                    .insert(
                        "frontendQualitySelfCheck".to_string(),
                        compact_frontend_quality_self_check(result),
                    );
            }
            summary
        })
        .collect()
}

fn compact_summary(value: &str) -> String {
    const LIMIT: usize = 320;
    let trimmed = value.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }
    trimmed.chars().take(LIMIT).collect::<String>()
}

fn compact_frontend_quality_self_check(result: &TaskResult) -> Value {
    let Some(self_check) = result.frontend_quality_self_check.as_ref() else {
        return json!({});
    };
    let self_check = serde_json::to_value(self_check).unwrap_or(Value::Null);
    json!({
        "present": true,
        "status": self_check.get("status").and_then(Value::as_str),
        "surfaceDecisionContractRef": self_check
            .get("surfaceDecisionContractRef")
            .and_then(Value::as_str),
        "surfaceRegionEvidenceCount": self_check
            .get("surfaceRegionEvidence")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "surfaceActionEvidenceCount": self_check
            .get("surfaceActionEvidence")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "surfaceStateEvidenceCount": self_check
            .get("surfaceStateEvidence")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "surfaceQualityRuleEvidenceCount": self_check
            .get("surfaceQualityRuleEvidence")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "referencePlanFilesCheckedCount": self_check
            .get("referencePlanFilesChecked")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "contentBoundaryEvidence": {
            "checked": self_check
                .pointer("/contentBoundaryEvidence/checked")
                .and_then(Value::as_bool),
                "violationCount": self_check
                    .pointer("/contentBoundaryEvidence/forbiddenContentViolations")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
        },
        "designTokenEvidence": self_check
            .get("designTokenEvidence")
            .map(|evidence| json!({
                "strategyUsed": evidence.get("strategyUsed").cloned().unwrap_or(Value::Null),
                "templateIdUsed": evidence.get("templateIdUsed").cloned().unwrap_or(Value::Null),
                "tokenAssetFileCount": evidence
                    .get("tokenAssetFiles")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "tokenConsumerFileCount": evidence
                    .get("tokenConsumerFiles")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "parallelTokenSystemCreated": evidence
                    .get("parallelTokenSystemCreated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "mergeSummaryPresent": evidence
                    .get("mergeSummary")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .is_some_and(|summary| !summary.is_empty())
            }))
            .unwrap_or(Value::Null),
        "forbiddenViolationCount": self_check
            .pointer("/contentBoundaryEvidence/forbiddenContentViolations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "knownGapCount": self_check
            .get("knownGaps")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0)
    })
}

fn allowed_refs(
    task_plan: &TaskPlan,
    run: &TaskPlanRun,
    task_results: &[TaskResult],
    change_context: &Value,
    change_set: &Value,
) -> Value {
    let changed_files = task_results
        .iter()
        .flat_map(|result| result.changed_files.clone())
        .collect::<BTreeSet<_>>();
    let changed_files = change_context
        .get("changedFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|file| file.get("path").and_then(Value::as_str).map(str::to_string))
        .chain(changed_files)
        .collect::<BTreeSet<_>>();
    let diff_refs = change_context
        .get("changedFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|file| {
            file.get("diffRef")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .chain(
            change_set
                .get("fullDiffRef")
                .and_then(Value::as_str)
                .map(str::to_string),
        )
        .collect::<BTreeSet<_>>();
    let mut verification_refs = task_results
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
    verification_refs.extend(task_results.iter().flat_map(|result| {
        result
            .verification_results
            .iter()
            .flat_map(|verification| verification.browser_checks.iter())
            .flat_map(|check| {
                std::iter::once(check.check_id.clone()).chain(check.artifact_refs.iter().cloned())
            })
    }));
    let mut read_refs = vec!["reviewPacket".to_string(), "changeContext".to_string()];
    read_refs.extend(
        task_results
            .iter()
            .map(|result| result.task_result_id.clone()),
    );
    read_refs.extend(changed_files.iter().cloned());
    read_refs.extend(diff_refs.iter().cloned());
    read_refs.extend(verification_refs.iter().cloned());
    json!({
        "taskIds": run.task_states.iter().map(|state| state.task_id.clone()).collect::<Vec<_>>(),
        "groupIds": run.group_states.iter().map(|state| state.group_id.clone()).collect::<Vec<_>>(),
        "acceptanceRefs": task_plan.scope_snapshot.acceptance_refs,
        "taskResultIds": task_results.iter().map(|result| result.task_result_id.clone()).collect::<Vec<_>>(),
        "changedFilePaths": changed_files.iter().cloned().collect::<Vec<_>>(),
        "diffRefs": diff_refs.iter().cloned().collect::<Vec<_>>(),
        "verificationEvidenceRefs": verification_refs.iter().cloned().collect::<Vec<_>>(),
        "readRefs": read_refs
    })
}

fn expected_top_action(
    result: &ReviewResult,
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
) -> String {
    if result.findings.iter().any(|finding| {
        finding.recommended_next_action == "needs_user_decision" && is_blocking_finding(finding)
    }) {
        return "needs_user_decision".to_string();
    }
    if result.findings.iter().any(|finding| {
        finding.recommended_next_action == "manual_review"
            && is_blocking_finding(finding)
            && (finding.category == "review_limitation"
                || finding.category == "environment_or_dependency"
                || finding.failure_class.as_deref() == Some("environment_blocker")
                || finding.severity_class.as_deref() == Some("manual_review"))
    }) {
        return "manual_review".to_string();
    }
    for action in [
        "architecture_artifact_repair",
        "taskplan_repair",
        "execution_repair",
    ] {
        if result.findings.iter().any(|finding| {
            finding.recommended_next_action == action && is_blocking_finding(finding)
        }) {
            return action.to_string();
        }
    }
    let has_next_phase = fields
        .get("reviewScope.nextPhasePreview.kind")
        .and_then(|field| field.value.as_str())
        == Some("candidate");
    if matches!(result.decision.as_str(), "approved" | "approved_with_notes") && has_next_phase {
        "continue_to_next_phase".to_string()
    } else if result.decision == "approved" || result.decision == "approved_with_notes" {
        "done".to_string()
    } else {
        result.next_action.r#type.clone()
    }
}

fn is_blocking_finding(finding: &ReviewFinding) -> bool {
    finding.severity_class.as_deref() == Some("blocking")
        || finding.severity_class.as_deref() == Some("manual_review")
        || matches!(finding.severity.as_str(), "critical" | "major")
}

fn blocking_finding_has_actionable_ref(finding: &ReviewFinding) -> bool {
    if !finding.task_refs.is_empty() || !finding.group_refs.is_empty() {
        return true;
    }
    if finding
        .artifact_refs
        .as_object()
        .map(|refs| {
            refs.values().any(|value| {
                value
                    .as_array()
                    .map(|items| !items.is_empty())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
    {
        return true;
    }
    finding
        .location
        .get("file")
        .and_then(Value::as_str)
        .map(|file| !file.trim().is_empty())
        .unwrap_or(false)
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

fn normalized_allowed_set(value: &Value, key: &str) -> BTreeSet<String> {
    allowed_set(value, key)
        .into_iter()
        .map(|item| normalize_review_file_ref(&item))
        .collect()
}

fn string_set_field(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    name: &str,
) -> BTreeSet<String> {
    fields
        .get(name)
        .map(|field| field.value.clone())
        .filter(Value::is_array)
        .and_then(|value| {
            value.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect::<BTreeSet<_>>()
            })
        })
        .unwrap_or_default()
}

fn normalize_review_file_ref(value: &str) -> String {
    value
        .trim_start_matches("changed_file:")
        .trim_start_matches("file:")
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn is_allowed_review_read_ref(
    ref_type: &str,
    ref_value: &str,
    allowed_read_refs: &BTreeSet<String>,
    task_result_ids: &BTreeSet<String>,
    changed_file_refs: &BTreeSet<String>,
    verification_refs: &BTreeSet<String>,
) -> bool {
    if allowed_read_refs.contains(ref_value) || task_result_ids.contains(ref_value) {
        return true;
    }
    match ref_type {
        "changed_file" => changed_file_refs.contains(&normalize_review_file_ref(ref_value)),
        "verification_evidence" => verification_refs.contains(ref_value),
        _ => false,
    }
}

fn is_allowed_review_evidence_ref(
    ref_type: &str,
    ref_value: &str,
    allowed_read_refs: &BTreeSet<String>,
    task_result_ids: &BTreeSet<String>,
    changed_file_refs: &BTreeSet<String>,
    verification_refs: &BTreeSet<String>,
) -> bool {
    if ref_type == "manual_note" {
        return true;
    }
    if allowed_read_refs.contains(ref_value) || task_result_ids.contains(ref_value) {
        return true;
    }
    match ref_type {
        "changed_file" => changed_file_refs.contains(&normalize_review_file_ref(ref_value)),
        "verification_result" => verification_refs.contains(ref_value),
        _ => false,
    }
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

fn review_result_repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    Ok(repairable(input, authorized, target_file, issues))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_quality_profile_routes_only_review_method_references() {
        let profile = review_quality_profile();
        let items = profile["referenceLoadPlan"].as_array().unwrap();
        let paths = items
            .iter()
            .filter_map(|item| item.get("path").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec![
                "tech/review/core.md",
                "tech/review/spec-compliance.md",
                "tech/review/defect-patterns.md",
                "tech/review/test-evidence.md",
                "tech/review/finding-quality.md",
            ]
        );
        assert!(items.iter().all(|item| {
            !item["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("route selection")
        }));
    }

    fn task_result_with_browser_check(attempts: u32) -> TaskResult {
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
                "browserChecks": [{
                    "checkId": "browser-ui-desktop",
                    "status": "passed",
                    "command": "pnpm playwright test --grep workflow",
                    "attempts": attempts,
                    "artifactRefs": ["test-results/workflow/trace.zip"],
                    "observedOutcome": "The submitted record appears in the rendered list.",
                    "blockedReason": null
                }]
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

    fn requirement() -> Value {
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
            "regionsInScope": [{"regionId": "region-main"}],
            "actionsInScope": [{"actionId": "action-submit"}],
            "statesInScope": [{"state": "empty"}],
            "qualityRulesInScope": [{"ruleId": "rule-density"}]
        })
    }

    fn self_check() -> Value {
        json!({
            "status": "satisfied",
            "surfaceDecisionContractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
            "surfaceRegionEvidence": [{
                "id": "region-main",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "evidence": "Region implemented."
            }],
            "surfaceActionEvidence": [{
                "id": "action-submit",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "evidence": "Action implemented."
            }],
            "surfaceStateEvidence": [{
                "id": "empty",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "evidence": "State implemented."
            }],
            "surfaceQualityRuleEvidence": [{
                "id": "rule-density",
                "status": "satisfied",
                "files": ["web/src/App.tsx"],
                "evidence": "Density rule applied."
            }],
            "contentBoundaryEvidence": {
                "checked": true,
                "forbiddenContentViolations": [],
                "evidence": "Only business UI copy is visible."
            },
            "referencePlanFilesChecked": ["plugins/shared/loom/references/uix/core.md"]
        })
    }

    #[test]
    fn frontend_surface_review_accepts_complete_coverage() {
        let review =
            frontend_surface_contract_review(&requirement(), &surface_contract(), &self_check());

        assert_eq!(review.get("satisfied").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn frontend_surface_review_rejects_missing_action_evidence() {
        let mut self_check = self_check();
        self_check["surfaceActionEvidence"] = json!([]);
        let review =
            frontend_surface_contract_review(&requirement(), &surface_contract(), &self_check);

        assert_eq!(
            review.get("satisfied").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            review
                .pointer("/actions/missingIds")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn compact_browser_summary_keeps_retry_visible_without_loading_success_artifacts() {
        let first_pass = compact_task_result_summaries(&[task_result_with_browser_check(1)]);
        let retried = compact_task_result_summaries(&[task_result_with_browser_check(2)]);

        assert_eq!(
            first_pass[0]["verificationResults"][0]["browserChecks"][0]["diagnosticArtifactRefs"],
            json!([])
        );
        assert_eq!(
            retried[0]["verificationResults"][0]["browserChecks"][0]["retrySucceeded"],
            json!(true)
        );
        assert_eq!(
            retried[0]["verificationResults"][0]["browserChecks"][0]["diagnosticArtifactRefs"][0],
            json!("test-results/workflow/trace.zip")
        );
    }

    #[test]
    fn review_snapshot_fingerprint_changes_when_task_result_evidence_changes() {
        let first = task_result_with_browser_check(1);
        let second = task_result_with_browser_check(2);
        assert_ne!(
            task_result_snapshot_fingerprint(&[first]),
            task_result_snapshot_fingerprint(&[second])
        );
    }

    #[test]
    fn browser_environment_signal_is_normalized_to_manual_review() {
        let mut result: ReviewResult = serde_json::from_value(json!({
            "schemaVersion": "1.0",
            "reviewId": "review-1",
            "source": {
                "requestId": "request-1",
                "phaseId": "phase-1",
                "taskPlanId": "taskplan-1",
                "taskPlanRunId": "run-1"
            },
            "decision": "approved",
            "findings": [],
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
            "limitations": [],
            "pendingActions": [],
            "nextAction": {
                "type": "done",
                "reason": "No issues.",
                "targetTaskIds": [],
                "findingRefs": []
            },
            "createdAt": "2026-07-13T10:00:00+08:00",
            "updatedAt": "2026-07-13T10:00:00+08:00"
        }))
        .unwrap();
        let mut fields = BTreeMap::new();
        fields.insert(
            "outputContract.reviewSignals.items".to_string(),
            delivery_core::FieldReadResult {
                value: json!([{
                    "kind": "frontend_ui_quality",
                    "taskRefs": ["task-ui"],
                    "recommendedNextAction": "manual_review",
                    "browserVerification": {
                        "environmentBlocked": true,
                        "closureTaskId": "task-browser-quality-closure",
                        "closureTaskResultId": "result-browser-environment"
                    }
                }]),
            },
        );

        normalize_browser_environment_review_route(&mut result, &fields);

        assert_eq!(result.decision, "blocked");
        assert_eq!(result.next_action.r#type, "manual_review");
        assert_eq!(result.findings.len(), 1);
        assert_eq!(
            result.findings[0].failure_class.as_deref(),
            Some("environment_blocker")
        );
        assert!(result.findings[0]
            .task_refs
            .contains(&"task-browser-quality-closure".to_string()));
    }

    #[test]
    fn browser_quality_manual_request_exposes_only_dedicated_resolutions() {
        let result: ReviewResult = serde_json::from_value(json!({
            "schemaVersion": "1.0",
            "reviewId": "review-1",
            "source": {"requestId": "request-1", "phaseId": "phase-1", "taskPlanId": "plan-1", "taskPlanRunId": "run-1"},
            "decision": "blocked",
            "findings": [],
            "coverageAssessment": {"mustAcceptance": [], "summary": {"totalMust": 0, "satisfied": 0, "insufficientEvidence": 0, "notSatisfied": 0, "notReviewed": 0}},
            "limitations": [],
            "pendingActions": [],
            "nextAction": {"type": "manual_review", "reason": "Browser unavailable", "targetTaskIds": [], "findingRefs": []},
            "createdAt": "2026-07-13T10:00:00+08:00",
            "updatedAt": "2026-07-13T10:00:00+08:00"
        }))
        .unwrap();
        let request = build_manual_review_request(
            "manual-1",
            "delivery-1",
            "phase-1",
            ".loom/agent-writable/manual-1/result.json",
            &result,
            ".loom/review.json",
            Some(&json!({
                "browserVerification": {
                    "environmentBlocked": true,
                    "closureTaskId": "task-browser-quality-closure",
                    "requiredCheckIds": ["check-desktop", "check-mobile"]
                }
            })),
        );

        assert_eq!(
            request["manualReviewProtocol"]["acceptedDecisions"],
            json!([
                "retry_browser_environment",
                "submit_external_browser_evidence",
                "approve_quality_waiver"
            ])
        );
        assert!(request["outputContract"]
            .get("resultTemplatesByDecision")
            .is_some());
        assert!(request["outputContract"].get("resultTemplate").is_none());
    }

    #[test]
    fn external_browser_evidence_must_cover_required_checks_exactly() {
        let gate = json!({
            "browserVerification": {
                "requiredCheckIds": ["check-desktop", "check-mobile"]
            }
        });
        let mut resolution: ManualReviewResolution = serde_json::from_value(json!({
            "schemaVersion": "1.0",
            "manualReviewResolutionId": "resolution-1",
            "manualReviewRequestId": "manual-1",
            "deliveryId": "delivery-1",
            "phaseId": "phase-1",
            "userAnswer": {"text": "CI evidence attached", "selectedShortReply": "submit_external_browser_evidence"},
            "decision": "submit_external_browser_evidence",
            "browserQualityResolution": {
                "externalEvidence": [{
                    "checkId": "check-desktop",
                    "evidenceRefs": ["https://ci.example.test/run/1"],
                    "observedOutcome": "Desktop workflow passed.",
                    "source": "CI"
                }]
            },
            "nextAction": {"type": "review", "reason": "Review evidence", "targetTaskIds": [], "findingRefs": []},
            "createdAt": "2026-07-13T10:00:00+08:00"
        }))
        .unwrap();
        let mut issues = Vec::new();

        validate_browser_quality_manual_resolution(&resolution, &gate, &mut issues);
        assert!(issues.iter().any(|issue| {
            issue.field_path.as_deref()
                == Some("browserQualityResolution.externalEvidence[].checkId")
        }));

        resolution
            .browser_quality_resolution
            .as_mut()
            .unwrap()
            .external_evidence
            .push(contracts::BrowserExternalEvidence {
                check_id: "check-mobile".to_string(),
                evidence_refs: vec!["test-results/mobile/report.html".to_string()],
                observed_outcome: "Mobile workflow passed.".to_string(),
                source: "QA workstation".to_string(),
            });
        issues.clear();
        validate_browser_quality_manual_resolution(&resolution, &gate, &mut issues);
        assert!(issues.is_empty(), "{issues:#?}");
    }
}
