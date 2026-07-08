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
    let architecture_contract = phase
        .latest_refs
        .get("architectureArtifact")
        .and_then(|architecture_ref| from_project_relative(root, architecture_ref).ok())
        .and_then(|architecture_path| {
            state::store::read_json::<ArchitectureArtifactContract>(&architecture_path).ok()
        });
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
    next_phase_handoff: Option<&brainstorm::NextPhaseHandoff>,
) -> Result<Value, state::store::StateError> {
    let schema_shape = serde_json::to_value(schema_for!(ReviewResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
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
        build_frontend_quality_review_matrix(task_plan, task_results);
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
            "taskResultSummaries": compact_task_result_summaries(task_results)
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
                "Every finding must include non-empty readRefs.",
                "Every blocking finding must describe the smallest repair that satisfies the current Loom contract.",
                "Do not modify project files during review.",
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
            "resultTemplate": review_result_template(review_id, phase_id, task_plan, run, next_phase_handoff),
            "allowedRefs": allowed_refs,
            "requiredFields": ["reviewId", "source", "decision", "findings", "coverageAssessment", "limitations", "pendingActions", "nextAction"],
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
                        "reviewPacket.taskResultSummaries"
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
                        "outputContract.schemaShape.properties.source",
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
                "reason": "Review gate posture, order, decision discipline, and route selection."
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
                "reason": "Actionable ReviewFinding severity, category, evidence, and repair route guidance."
            }
        ]
    })
}

fn review_result_template(
    review_id: &str,
    phase_id: &str,
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
            "reason": handoff.reason,
            "targetPhaseId": handoff.phase_id,
            "targetTaskIds": [],
            "findingRefs": []
        })
    } else {
        json!({
            "type": "done",
            "reason": "",
            "targetTaskIds": [],
            "findingRefs": []
        })
    };
    json!({
        "schemaVersion": "1.0",
        "reviewId": review_id,
        "source": {
            "requestId": review_id,
            "phaseId": phase_id,
            "taskPlanId": task_plan.task_plan_id,
            "taskPlanRunId": run.run_id
        },
        "decision": "approved",
        "findings": [{
            "findingId": "finding_1",
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
        "nextAction": next_action,
        "createdAt": "ISO-8601 datetime",
        "updatedAt": "ISO-8601 datetime"
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

fn manual_review_resolution_template(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    result: &ReviewResult,
) -> Value {
    let route = match result.next_action.r#type.as_str() {
        "execution_repair" | "taskplan_repair" | "architecture_artifact_repair" => {
            result.next_action.r#type.as_str()
        }
        _ => "needs_user_decision",
    };
    json!({
        "schemaVersion": "1.0",
        "manualReviewResolutionId": format!("manual-review-resolution-{request_id}"),
        "manualReviewRequestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
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
            "reason": "",
            "targetTaskIds": result.next_action.target_task_ids.clone(),
            "findingRefs": result.next_action.finding_refs.clone()
        },
        "createdAt": "ISO-8601 datetime"
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
    let raw = state::store::read_json_value(&from_project_relative(root, &target.path)?)?;
    let mut result: ReviewResult = match serde_json::from_value(raw) {
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
        ],
    })?
    .fields;
    if let Some(handoff) =
        normalize_approved_next_phase(&input.project_root, &delivery_id, &phase_id, &result)?
    {
        result.next_action.r#type = "continue_to_next_phase".to_string();
        result.next_action.target_phase_id = Some(handoff.phase_id);
        result.next_action.reason = handoff.reason;
    }
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
    if result.source.request_id != request_id
        || fields
            .get("source.phaseId")
            .and_then(|field| field.value.as_str())
            != Some(result.source.phase_id.as_str())
        || fields
            .get("source.taskPlanId")
            .and_then(|field| field.value.as_str())
            != Some(result.source.task_plan_id.as_str())
        || fields
            .get("source.taskPlanRunId")
            .and_then(|field| field.value.as_str())
            != Some(result.source.task_plan_run_id.as_str())
    {
        issues.push(issue(
            "REVIEW_RESULT_REF_INVALID",
            "source",
            "ReviewResult source must match the active ReviewRequest.",
        ));
    }
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
    brainstorm::materialize_next_phase_from_preview(
        project_root,
        delivery_id,
        phase_id,
        result.next_action.target_phase_id.as_deref(),
    )
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
            "schemaShape": schema_shape,
            "resultTemplate": manual_review_resolution_template(
                request_id,
                delivery_id,
                phase_id,
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
                        "outputContract.schemaShape.properties.manualReviewRequestId",
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
        fields: vec![
            "source.reviewId".to_string(),
            "enumRefs.decision".to_string(),
            "enumRefs.changeRequestRoute".to_string(),
            "enumRefs.nextActionType".to_string(),
        ],
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
    let mut values = resolution.next_action.target_task_ids.clone();
    if let Some(change_request) = &resolution.change_request {
        values.extend(
            change_request
                .details
                .get("targetTaskIds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_string)),
        );
    }
    dedupe_non_empty(values)
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
) -> Vec<Value> {
    task_plan
        .tasks
        .iter()
        .filter_map(|task| {
            let ui_quality_contract = task
                .frontend_experience_requirement
                .as_ref()
                .and_then(|requirement| requirement.get("uiQualityContract"))?;
            let result = task_results
                .iter()
                .find(|result| result.task_id == task.task_id);
            let self_check = result
                .and_then(|result| result.frontend_quality_self_check.as_ref())
                .map(serde_json::to_value)
                .and_then(Result::ok)
                .unwrap_or(Value::Null);
            let expected_refs = reference_groups(
                ui_quality_contract
                    .get("referenceProfile")
                    .unwrap_or(&Value::Null),
                "groups",
            );
            let checked_refs = reference_groups(&self_check, "referenceGroupsChecked");
            let expected_reference_files = reference_load_plan_paths(
                ui_quality_contract
                    .get("referenceProfile")
                    .unwrap_or(&Value::Null),
            );
            let checked_reference_files = string_array_field(&self_check, "referenceFilesChecked");
            let expected_states =
                object_array_string_field(ui_quality_contract, "requiredUiStates", "state");
            let covered_states = object_array_string_field(&self_check, "statesCovered", "state");
            let expected_rules =
                object_array_string_field(ui_quality_contract, "businessUiRules", "ruleId");
            let checked_rules =
                object_array_string_field(&self_check, "businessUiRulesChecked", "ruleId");
            let missing_reference_groups = missing_reference_groups(&expected_refs, &checked_refs);
            let missing_reference_files =
                missing_strings(&expected_reference_files, &checked_reference_files);
            let missing_ui_states = missing_strings(&expected_states, &covered_states);
            let missing_business_ui_rule_ids = missing_strings(&expected_rules, &checked_rules);
            let expected_gates = frontend_quality_gates_for_review(task, ui_quality_contract);
            let gate_review = frontend_quality_gate_review(&expected_gates, &self_check);
            let forbidden_violation_count = self_check
                .pointer("/forbiddenContentCheck/violations")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let token_plan = ui_quality_contract
                .get("designTokenAssetPlan")
                .cloned()
                .unwrap_or(Value::Null);
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
            let scenario_matches = self_check.get("scenarioKind").and_then(Value::as_str)
                == ui_quality_contract
                    .pointer("/scenario/kind")
                    .and_then(Value::as_str);
            let quality_level_matches = self_check.get("qualityLevel").and_then(Value::as_str)
                == ui_quality_contract
                    .get("qualityLevel")
                    .and_then(Value::as_str);
            let quality_satisfied = self_check.get("status").and_then(Value::as_str)
                == Some("satisfied")
                && scenario_matches
                && quality_level_matches
                && gate_review
                    .get("satisfied")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                && missing_reference_groups.is_empty()
                && missing_reference_files.is_empty()
                && missing_ui_states.is_empty()
                && missing_business_ui_rule_ids.is_empty()
                && token_asset_satisfied
                && forbidden_violation_count == 0
                && known_gap_count == 0;
            Some(json!({
                "taskId": task.task_id,
                "taskResultId": result.map(|result| result.task_result_id.clone()),
                "scenarioKind": ui_quality_contract.pointer("/scenario/kind").and_then(Value::as_str),
                "actualScenarioKind": self_check.get("scenarioKind").and_then(Value::as_str),
                "qualityLevel": ui_quality_contract.get("qualityLevel").and_then(Value::as_str),
                "actualQualityLevel": self_check.get("qualityLevel").and_then(Value::as_str),
                "actualStatus": self_check.get("status").and_then(Value::as_str),
                "qualitySatisfied": quality_satisfied,
                "missingReferenceGroups": missing_reference_groups,
                "referenceFiles": expected_reference_files,
                "referenceFilesChecked": checked_reference_files,
                "missingReferenceFiles": missing_reference_files,
                "missingUiStates": missing_ui_states,
                "missingBusinessUiRuleIds": missing_business_ui_rule_ids,
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
                "qualityGateCoverage": gate_review,
                "forbiddenViolationCount": forbidden_violation_count,
                "knownGapCount": known_gap_count,
                "recommendedNextAction": if quality_satisfied { "none" } else { "execution_repair" }
            }))
        })
        .collect()
}

fn frontend_quality_gates_for_review(
    task: &TaskDefinition,
    ui_quality_contract: &Value,
) -> Vec<Value> {
    task.frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiTaskQualityGates"))
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| {
            ui_quality_contract
                .get("qualityGates")
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default()
}

fn frontend_quality_gate_review(expected_gates: &[Value], self_check: &Value) -> Value {
    let expected = expected_gates
        .iter()
        .filter_map(|gate| {
            let gate_id = gate.get("gateId").and_then(Value::as_str)?;
            Some((
                gate_id.to_string(),
                gate.get("severity")
                    .and_then(Value::as_str)
                    .unwrap_or("must")
                    .to_string(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let expected_by_id = expected_gates
        .iter()
        .filter_map(|gate| {
            gate.get("gateId")
                .and_then(Value::as_str)
                .map(|gate_id| (gate_id.to_string(), gate))
        })
        .collect::<BTreeMap<_, _>>();
    let expected_ids = expected.keys().cloned().collect::<BTreeSet<_>>();
    let gate_results = self_check
        .get("gateResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut seen = BTreeSet::new();
    let mut checked_gate_ids = Vec::new();
    let mut duplicate_gate_ids = Vec::new();
    let mut invented_gate_ids = Vec::new();
    let mut invalid_status_gate_ids = Vec::new();
    let mut partial_gate_ids = Vec::new();
    let mut missing_status_gate_ids = Vec::new();
    let mut blocked_environment_gate_ids = Vec::new();
    let mut invalid_environment_block_gate_ids = Vec::new();
    let mut render_gate_missing_viewport_ids = Vec::new();
    let mut source_check_missing_gate_ids = Vec::new();
    let mut viewport_check_missing_gate_ids = Vec::new();
    let mut self_report_only_gate_ids = Vec::new();
    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut status_by_gate = BTreeMap::<String, String>::new();

    for result in &gate_results {
        let Some(gate_id) = result.get("gateId").and_then(Value::as_str) else {
            invalid_status_gate_ids.push("missing_gate_id".to_string());
            continue;
        };
        checked_gate_ids.push(gate_id.to_string());
        if !seen.insert(gate_id.to_string()) {
            duplicate_gate_ids.push(gate_id.to_string());
        }
        if !expected_ids.contains(gate_id) {
            invented_gate_ids.push(gate_id.to_string());
        }
        let required_evidence = expected_by_id
            .get(gate_id)
            .map(|gate| string_array_field(gate, "requiredEvidence"))
            .unwrap_or_default();
        let status = result
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("missing");
        *status_counts.entry(status.to_string()).or_insert(0) += 1;
        status_by_gate.insert(gate_id.to_string(), status.to_string());
        match status {
            "partial" => partial_gate_ids.push(gate_id.to_string()),
            "missing" => missing_status_gate_ids.push(gate_id.to_string()),
            "blocked_by_environment" => {
                blocked_environment_gate_ids.push(gate_id.to_string());
                if !is_environment_blockable_gate(gate_id, &required_evidence) {
                    invalid_environment_block_gate_ids.push(gate_id.to_string());
                }
            }
            "satisfied" => {
                if evidence_requires_source_check(&required_evidence)
                    && string_array_field(result, "sourceChecks").is_empty()
                {
                    source_check_missing_gate_ids.push(gate_id.to_string());
                }
                if evidence_requires_viewport_check(&required_evidence)
                    && string_array_field(result, "viewportsChecked").is_empty()
                {
                    viewport_check_missing_gate_ids.push(gate_id.to_string());
                }
                if is_render_or_viewport_gate(gate_id)
                    && !has_desktop_and_mobile_viewports(&string_array_field(
                        result,
                        "viewportsChecked",
                    ))
                {
                    render_gate_missing_viewport_ids.push(gate_id.to_string());
                }
                if string_array_field(result, "files").is_empty()
                    && string_array_field(result, "sourceChecks").is_empty()
                    && string_array_field(result, "viewportsChecked").is_empty()
                    && string_array_field(result, "attemptedChecks").is_empty()
                    && string_array_field(result, "fallbackEvidence").is_empty()
                {
                    self_report_only_gate_ids.push(gate_id.to_string());
                }
            }
            "not_applicable" => {}
            _ => invalid_status_gate_ids.push(gate_id.to_string()),
        }
    }

    let missing_gate_ids = expected_ids
        .iter()
        .filter(|gate_id| !seen.contains(*gate_id))
        .cloned()
        .collect::<Vec<_>>();
    let must_gate_unsatisfied_ids = expected
        .iter()
        .filter_map(|(gate_id, severity)| {
            if severity != "must" {
                return None;
            }
            let status = status_by_gate.get(gate_id).map(String::as_str);
            if status == Some("satisfied") {
                None
            } else {
                Some(gate_id.clone())
            }
        })
        .collect::<Vec<_>>();
    let satisfied = expected_ids.is_empty()
        || (missing_gate_ids.is_empty()
            && invented_gate_ids.is_empty()
            && duplicate_gate_ids.is_empty()
            && invalid_status_gate_ids.is_empty()
            && partial_gate_ids.is_empty()
            && missing_status_gate_ids.is_empty()
            && blocked_environment_gate_ids.is_empty()
            && invalid_environment_block_gate_ids.is_empty()
            && render_gate_missing_viewport_ids.is_empty()
            && source_check_missing_gate_ids.is_empty()
            && viewport_check_missing_gate_ids.is_empty()
            && self_report_only_gate_ids.is_empty()
            && must_gate_unsatisfied_ids.is_empty());

    json!({
        "expectedGateIds": expected_ids.into_iter().collect::<Vec<_>>(),
        "checkedGateIds": checked_gate_ids,
        "expectedGateCount": expected.len(),
        "checkedGateCount": gate_results.len(),
        "statusCounts": status_counts,
        "missingGateIds": missing_gate_ids,
        "inventedGateIds": invented_gate_ids,
        "duplicateGateIds": duplicate_gate_ids,
        "invalidStatusGateIds": invalid_status_gate_ids,
        "partialGateIds": partial_gate_ids,
        "missingStatusGateIds": missing_status_gate_ids,
        "mustGateUnsatisfiedIds": must_gate_unsatisfied_ids,
        "blockedEnvironmentGateIds": blocked_environment_gate_ids,
        "invalidEnvironmentBlockGateIds": invalid_environment_block_gate_ids,
        "renderGateMissingViewportIds": render_gate_missing_viewport_ids,
        "sourceCheckMissingGateIds": source_check_missing_gate_ids,
        "viewportCheckMissingGateIds": viewport_check_missing_gate_ids,
        "selfReportOnlyGateIds": self_report_only_gate_ids,
        "satisfied": satisfied
    })
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
                "scenarioKind": item.get("scenarioKind").cloned().unwrap_or(Value::Null),
                "actualScenarioKind": item.get("actualScenarioKind").cloned().unwrap_or(Value::Null),
                "qualityLevel": item.get("qualityLevel").cloned().unwrap_or(Value::Null),
                "actualQualityLevel": item.get("actualQualityLevel").cloned().unwrap_or(Value::Null),
                "missingReferenceCount": item
                    .get("missingReferenceGroups")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingQualityGateCount": item
                    .pointer("/qualityGateCoverage/missingGateIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "mustQualityGateUnsatisfiedCount": item
                    .pointer("/qualityGateCoverage/mustGateUnsatisfiedIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingUiStateCount": item
                    .get("missingUiStates")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                "missingBusinessUiRuleCount": item
                    .get("missingBusinessUiRuleIds")
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
    for quality in build_frontend_quality_review_matrix(task_plan, task_results) {
        let task_id = quality
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap_or("task");
        let quality_satisfied = quality
            .get("qualitySatisfied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        signals.push(json!({
            "signalId": format!("sig-frontend-ui-quality-{}", safe_signal_id(task_id)),
            "kind": "frontend_ui_quality",
            "taskRefs": [task_id],
            "taskResultId": quality.get("taskResultId").cloned().unwrap_or(Value::Null),
            "uiQualitySatisfied": quality_satisfied,
            "actualStatus": quality.get("actualStatus").cloned().unwrap_or(Value::Null),
            "missingReferenceGroups": quality.get("missingReferenceGroups").cloned().unwrap_or_else(|| json!([])),
            "qualityGateCoverage": quality.get("qualityGateCoverage").cloned().unwrap_or_else(|| json!({})),
            "missingUiStates": quality.get("missingUiStates").cloned().unwrap_or_else(|| json!([])),
            "missingBusinessUiRuleIds": quality.get("missingBusinessUiRuleIds").cloned().unwrap_or_else(|| json!([])),
            "forbiddenViolationCount": quality.get("forbiddenViolationCount").cloned().unwrap_or_else(|| json!(0)),
            "knownGapCount": quality.get("knownGapCount").cloned().unwrap_or_else(|| json!(0)),
            "recommendedNextAction": if quality_satisfied { "none" } else { "execution_repair" },
            "reason": if quality_satisfied {
                "TaskResult frontend quality self-check satisfies the UI quality contract."
            } else {
                "TaskResult frontend quality self-check does not satisfy the UI quality contract."
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

fn reference_groups(value: &Value, key: &str) -> Vec<(String, String)> {
    value
        .get(key)
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.iter())
        .flat_map(|(group, items)| {
            items
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str())
                .map(|item| (group.clone(), item.to_string()))
                .collect::<Vec<_>>()
        })
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

fn reference_load_plan_paths(value: &Value) -> Vec<String> {
    value
        .get("referenceLoadPlan")
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

fn is_render_or_viewport_gate(gate_id: &str) -> bool {
    gate_id.contains("render") || gate_id.contains("viewport")
}

fn is_environment_blockable_gate(gate_id: &str, required_evidence: &[String]) -> bool {
    is_render_or_viewport_gate(gate_id)
        || gate_id.contains("mobile")
        || evidence_requires_viewport_check(required_evidence)
        || required_evidence
            .iter()
            .any(|item| item == "render_or_environment_reason")
}

fn evidence_requires_source_check(required: &[String]) -> bool {
    required
        .iter()
        .any(|item| item == "source_check" || item == "responsive_source_check")
}

fn evidence_requires_viewport_check(required: &[String]) -> bool {
    required.iter().any(|item| item == "viewport_check")
}

fn has_desktop_and_mobile_viewports(viewports: &[String]) -> bool {
    let normalized = viewports
        .iter()
        .map(|viewport| viewport.to_ascii_lowercase())
        .collect::<Vec<_>>();
    normalized
        .iter()
        .any(|viewport| viewport.contains("desktop") || viewport.contains("1024"))
        && normalized.iter().any(|viewport| {
            viewport.contains("mobile") || viewport.contains("375") || viewport.contains("390")
        })
}

fn missing_reference_groups(
    expected: &[(String, String)],
    actual: &[(String, String)],
) -> Vec<Value> {
    let actual = actual.iter().collect::<BTreeSet<_>>();
    expected
        .iter()
        .filter(|item| !actual.contains(item))
        .map(|(group, item)| {
            json!({
                "group": group,
                "item": item
            })
        })
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
                        "evidenceType": verification.evidence_type
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
        "scenarioKind": self_check.get("scenarioKind").and_then(Value::as_str),
        "qualityLevel": self_check.get("qualityLevel").and_then(Value::as_str),
        "referenceGroupCheckCount": reference_groups(&self_check, "referenceGroupsChecked").len(),
        "statesCoveredCount": self_check
            .get("statesCovered")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "businessUiRuleCheckCount": self_check
            .get("businessUiRulesChecked")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "surfacesCoveredCount": self_check
            .get("surfacesCovered")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "qualityGateResultCount": self_check
            .get("gateResults")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "qualityGateStatusCounts": frontend_quality_gate_status_counts(&self_check),
        "environmentBlockedGateCount": self_check
            .get("gateResults")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|gate| {
                gate.get("status").and_then(Value::as_str) == Some("blocked_by_environment")
            })
            .count(),
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
            .pointer("/forbiddenContentCheck/violations")
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

fn frontend_quality_gate_status_counts(self_check: &Value) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for gate in self_check
        .get("gateResults")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let status = gate
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("missing");
        *counts.entry(status.to_string()).or_insert(0) += 1;
    }
    counts
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

fn repairable_or_fallback_manual_review(
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
