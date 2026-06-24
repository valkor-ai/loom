use std::path::Path;

use contracts::{
    BrainstormContract, ProjectKind, TechnicalBaselineApprovalType,
    TechnicalBaselineCandidateAgentWritable, TechnicalBaselineContract, TechnicalBaselineStatus,
};
use delivery_core::{
    ArtifactKind, FileSubmitInput, LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpRepairableErrorResult, LoomMcpUserGateResult, OperationContext, RouteAction,
    RouteActionKind, SubmitAcceptedEvent, TransitionEngine, TransitionStore,
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
        repository_context_file, technical_baseline_candidate_file, technical_baseline_file,
        technical_baseline_request_file,
    },
    write_artifact_result, PlanningDomainDispatcher,
};

pub fn materialize_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match materialize_request_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: project_root.to_string(),
            error: LoomMcpFailure {
                code: "TECHNICAL_BASELINE_REQUEST_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("planning".to_string()),
                route_action: Some("technical_baseline_request".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn materialize_request_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
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
                "phase {} does not exist in delivery {}",
                phase_id, delivery_id
            ))
        })?;
    if let Some(existing_request_ref) = phase
        .latest_refs
        .get("technicalBaselineRequestRef")
        .cloned()
    {
        let inspected = state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        });
        if inspected
            .as_ref()
            .map(|request| request.request_kind == "technical_baseline_request")
            .unwrap_or(false)
        {
            return write_artifact_result(
                project_root,
                &existing_request_ref,
                ArtifactKind::TechnicalBaselineCandidate,
            );
        }
    }
    let brainstorm_ref = phase
        .latest_refs
        .get("brainstormContract")
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest brainstormContract ref is missing".to_string(),
            )
        })?
        .clone();
    let brainstorm = read_brainstorm_contract(root, &brainstorm_ref)?;
    let project_kind = infer_project_kind(root);
    let request_id = format!("tbr_{}", state::store::now_millis());
    let candidate_file = to_project_relative(
        root,
        &technical_baseline_candidate_file(root, &locator, &request_id),
    )?;
    let request_file = to_project_relative(
        root,
        &technical_baseline_request_file(root, &locator, &request_id),
    )?;
    let baseline_exists = technical_baseline_file(root, delivery_id).exists();
    let request_root = build_request_root(
        &brainstorm,
        delivery_id,
        phase_id,
        &request_id,
        &candidate_file,
        project_kind,
        baseline_exists,
    );
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "technical_baseline_request".to_string(),
            request_file: Some(request_file),
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(phase_id.to_string()),
            root: request_root,
        },
    )?;
    if let Some(active_phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        active_phase
            .latest_refs
            .insert("technicalBaselineRequestId".to_string(), request_id);
        active_phase.latest_refs.insert(
            "technicalBaselineRequestRef".to_string(),
            stored.request_ref.clone(),
        );
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    write_artifact_result(
        project_root,
        &stored.request_ref,
        ArtifactKind::TechnicalBaselineCandidate,
    )
}

fn build_request_root(
    brainstorm: &BrainstormContract,
    delivery_id: &str,
    phase_id: &str,
    request_id: &str,
    candidate_file: &str,
    project_kind: ProjectKind,
    baseline_exists: bool,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(TechnicalBaselineCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let selection_guidance = if matches!(project_kind, ProjectKind::Greenfield) {
        Some(json!({
            "userFacingConfirmationProtocol": {
                "required": true,
                "showToUser": [
                    "recommendation_basis",
                    "recommended_stack",
                    "adjustable_range",
                    "confirmation_rule"
                ],
                "confirmationRule": "Do not submit the final TechnicalBaseline candidate until the user explicitly confirms the final technology baseline."
            },
            "recommendationRules": [
                "Base the first recommendation on the full Brainstorm scope and deferred roadmap hints, not only on the current phase slice.",
                "Keep build, test, local run, and deployment mechanics out of the first-screen technology confirmation unless the user explicitly asks for them."
            ]
        }))
    } else {
        None
    };
    let repo_evidence = json!({
        "detectedProjectKind": project_kind,
        "baselineExists": baseline_exists,
        "repositoryContextExists": false
    });
    json!({
        "schemaVersion": "1.0",
        "requestType": "technical_baseline_request",
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "requestId": request_id,
        "projectKind": project_kind,
        "operation": if matches!(project_kind, ProjectKind::ExistingProject) {
            "infer_existing_project_baseline"
        } else {
            "recommend_greenfield_baseline"
        },
        "brainstormLens": {
            "summary": brainstorm.summary,
            "scope": brainstorm.scope,
            "acceptance": brainstorm.acceptance,
            "frontendExperience": brainstorm.frontend_experience,
            "userFacingLanguage": brainstorm.delivery_context.user_facing_language,
            "sourceRefs": brainstorm.sources.iter().map(|source| source.source_id.clone()).collect::<Vec<_>>()
        },
        "currentPhaseLens": {
            "phaseId": brainstorm.phase_plan.current.phase_id,
            "title": brainstorm.phase_plan.current.title,
            "goal": brainstorm.phase_plan.current.goal,
            "includedScopeRefs": brainstorm.phase_plan.current.scope_refs,
            "acceptanceRefs": brainstorm.phase_plan.current.acceptance_refs,
        },
        "decisionNeeds": technical_baseline_decision_needs(project_kind, baseline_exists),
        "constraints": {
            "mustUse": [],
            "mustAvoid": [],
            "userPreferences": [],
            "deploymentPreference": "local_first"
        },
        "repoEvidence": repo_evidence,
        "selectionGuidance": selection_guidance,
        "enumRefs": {
            "projectKind": ["greenfield", "existing_project", "unknown"],
            "status": ["draft", "needs_user_confirmation", "auto_accepted", "confirmed", "blocked", "superseded"],
            "source": ["user_specified", "user_confirmed", "detected_from_repo", "agent_inferred_from_repo_signals", "agent_recommended_for_greenfield"],
            "scope": ["project", "roadmap", "phase_override"],
            "approvalType": ["user_confirmed", "policy_auto_accept", "manual_override", "none"],
            "confidence": ["low", "medium", "high", "unknown"]
        },
        "rules": {
            "context": [
                "Use the confirmed Brainstorm scope as the product-scope authority.",
                "Do not rewrite or weaken the confirmed Brainstorm scope, acceptance, or frontend target while choosing the technology baseline."
            ],
            "candidatePolicy": [
                "Write only the TechnicalBaseline candidate JSON.",
                "Do not write accepted baseline files directly.",
                "Use needs_user_confirmation plus approval.type=none when the baseline still needs explicit user confirmation."
            ]
        },
        "outputContract": {
            "artifactKind": ArtifactKind::TechnicalBaselineCandidate,
            "writeMode": "single_json",
            "submitTool": "loom.technicalBaselineAcceptFile",
            "writeTargets": [{
                "targetId": "candidate",
                "path": candidate_file,
                "required": true,
                "description": "Write the TechnicalBaseline candidate JSON."
            }],
            "schemaShape": schema_shape,
            "schemaProjection": {
                "requiredTopLevelFields": [
                    "status",
                    "source",
                    "projectKind",
                    "scope",
                    "stack",
                    "approval",
                    "confidence"
                ]
            }
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "technical_baseline_context",
                    "required": true,
                    "purpose": "Read the confirmed Brainstorm scope, acceptance, frontend target, current phase lens, and baseline decision needs before drafting the baseline.",
                    "whenToRead": "Read before producing any TechnicalBaseline recommendation.",
                    "fields": [
                        "brainstormLens.summary",
                        "brainstormLens.scope",
                        "brainstormLens.acceptance",
                        "brainstormLens.frontendExperience",
                        "brainstormLens.userFacingLanguage",
                        "currentPhaseLens",
                        "decisionNeeds",
                        "constraints"
                    ]
                },
                {
                    "groupId": "technical_baseline_repo_evidence",
                    "required": false,
                    "purpose": "Read repository evidence before inferring an existing-project baseline or deciding whether reuse applies.",
                    "whenToRead": "Read for existing_project or when repository continuity matters.",
                    "fields": [
                        "projectKind",
                        "repoEvidence"
                    ]
                },
                {
                    "groupId": "technical_baseline_selection_guidance",
                    "required": false,
                    "purpose": "Read the greenfield confirmation discipline before asking the user to confirm the baseline.",
                    "whenToRead": "Read only when the projectKind is greenfield or the baseline still needs explicit user confirmation.",
                    "fields": [
                        "selectionGuidance",
                        "rules.context",
                        "rules.candidatePolicy"
                    ]
                },
                {
                    "groupId": "technical_baseline_write_contract",
                    "required": true,
                    "purpose": "Read the candidate schema and write target before writing the TechnicalBaseline candidate.",
                    "whenToRead": "Read only when ready to write the candidate file.",
                    "fields": [
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
                        "outputContract.schemaProjection",
                        "enumRefs.projectKind",
                        "enumRefs.status",
                        "enumRefs.source",
                        "enumRefs.scope",
                        "enumRefs.approvalType",
                        "enumRefs.confidence"
                    ]
                }
            ]
        }
    })
}

pub fn accept_technical_baseline_file(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
) -> LoomMcpActionResult {
    match accept_technical_baseline_file_inner(input, authorized) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "TECHNICAL_BASELINE_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("planning".to_string()),
                route_action: Some("technical_baseline_accept".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn accept_technical_baseline_file_inner(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let Some(target) = authorized.targets.first() else {
        return Ok(repairable(
            input,
            authorized,
            String::new(),
            vec![issue(
                "TARGET_MISSING",
                "candidate",
                "No authorized TechnicalBaseline target was written.",
            )],
        ));
    };
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("authorized deliveryId is missing".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("authorized phaseId is missing".to_string())
    })?;
    if let Some(result) = ensure_latest_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
        "technicalBaselineRequestRef",
    )? {
        return Ok(result);
    }
    let project_root = Path::new(&input.project_root);
    let candidate_file = from_project_relative(project_root, &target.path)?;
    let raw = state::store::read_json_value(&candidate_file)?;
    let candidate: TechnicalBaselineCandidateAgentWritable =
        match serde_json::from_value(raw.clone()) {
            Ok(candidate) => candidate,
            Err(error) => {
                return Ok(repairable(
                    input,
                    authorized,
                    target.path.clone(),
                    vec![issue(
                        "TECHNICAL_BASELINE_SCHEMA_INVALID",
                        "candidate",
                        &format!("TechnicalBaseline candidate JSON has an invalid schema: {error}"),
                    )],
                ));
            }
        };

    let issues = validate_candidate(&candidate);
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, target.path.clone(), issues));
    }
    if matches!(candidate.project_kind, ProjectKind::Unknown) {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "TechnicalBaseline projectKind is unknown. Ask the user whether this phase continues an existing project or starts a greenfield project, then rewrite the same candidate with the confirmed projectKind.".to_string(),
            "project_kind_confirmation".to_string(),
        ));
    }
    if matches!(candidate.project_kind, ProjectKind::Greenfield)
        && candidate.approval.r#type != TechnicalBaselineApprovalType::UserConfirmed
    {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "Greenfield TechnicalBaseline must be explicitly confirmed by the user before planning can continue. Present the recommended stack, capture corrections, then rewrite the same candidate with approval.type=user_confirmed.".to_string(),
            "greenfield_baseline_confirmation".to_string(),
        ));
    }
    if candidate.requires_user_confirmation.unwrap_or(false)
        || matches!(
            candidate.status,
            TechnicalBaselineStatus::NeedsUserConfirmation
        )
    {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "TechnicalBaseline still requires explicit user confirmation. Present the baseline change or recommendation, then rewrite the same candidate with the confirmed baseline.".to_string(),
            "technical_baseline_confirmation".to_string(),
        ));
    }

    let now = state::store::now_string();
    let persisted = TechnicalBaselineContract {
        schema_version: "1.0".to_string(),
        technical_baseline_id: format!("tb_{}_{}", phase_id, state::store::now_millis()),
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
        status: candidate.status,
        source: candidate.source,
        project_kind: candidate.project_kind,
        scope: candidate.scope,
        stack: candidate.stack,
        constraints: candidate.constraints,
        evidence: candidate.evidence,
        approval: candidate.approval,
        confidence: candidate.confidence,
        requires_user_confirmation: candidate.requires_user_confirmation,
        reasoning_summary: candidate.reasoning_summary,
        alternatives: candidate.alternatives,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let baseline_file = technical_baseline_file(project_root, &delivery_id);
    state::store::write_json_atomic(&baseline_file, &persisted)?;
    let baseline_ref = to_project_relative(project_root, &baseline_file)?;

    let store = FileTransitionStore;
    let mut delivery = store
        .load_delivery_index(&input.project_root, &delivery_id)
        .map_err(to_state_error)?;
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        phase.latest_refs.insert(
            "technicalBaselineRequestRef".to_string(),
            input.request_ref.clone(),
        );
        phase
            .latest_refs
            .insert("technicalBaseline".to_string(), baseline_ref.clone());
    }
    delivery.updated_at = now;
    store
        .save_delivery_index(&input.project_root, &delivery)
        .map_err(to_state_error)?;

    let next_action = if matches!(persisted.project_kind, ProjectKind::ExistingProject)
        && !repository_context_file(
            project_root,
            &DeliveryPhaseLocator {
                delivery_id: delivery_id.clone(),
                phase_id: phase_id.clone(),
            },
        )
        .exists()
    {
        RouteAction {
            kind: RouteActionKind::RepositoryContextRequest,
            source: "technical_baseline_accept".to_string(),
            reason: "technical_baseline_ready_existing_project".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        }
    } else {
        RouteAction {
            kind: RouteActionKind::PlanningContractCreate,
            source: "technical_baseline_accept".to_string(),
            reason: "technical_baseline_ready".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        }
    };

    let engine = TransitionEngine {
        store: FileTransitionStore,
        dispatcher: PlanningDomainDispatcher,
    };
    engine
        .advance_after_submit(
            OperationContext {
                project_root: input.project_root.clone(),
            },
            SubmitAcceptedEvent {
                delivery_id,
                phase_id,
                source_tool: "loom.technicalBaselineAcceptFile".to_string(),
                accepted_artifact_ref: format!(
                    "{}/targets/{}",
                    input.request_ref, target.target_id
                ),
                next_action: Some(next_action),
            },
        )
        .map_err(to_state_error)
}

fn validate_candidate(
    candidate: &TechnicalBaselineCandidateAgentWritable,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if !candidate.stack.is_object() {
        issues.push(issue(
            "TECHNICAL_BASELINE_STACK_INVALID",
            "stack",
            "stack must be a JSON object that describes the selected technology baseline.",
        ));
    }
    if candidate.approval.r#type == TechnicalBaselineApprovalType::None
        && matches!(candidate.status, TechnicalBaselineStatus::Confirmed)
    {
        issues.push(issue(
            "TECHNICAL_BASELINE_APPROVAL_INVALID",
            "approval.type",
            "A confirmed TechnicalBaseline cannot keep approval.type=none.",
        ));
    }
    issues
}

fn technical_baseline_decision_needs(
    project_kind: ProjectKind,
    baseline_exists: bool,
) -> Vec<String> {
    let mut needs = Vec::new();
    if matches!(project_kind, ProjectKind::Greenfield) {
        needs.push("confirm_greenfield_stack".to_string());
    }
    if baseline_exists {
        needs.push("check_previous_baseline_reuse".to_string());
    }
    if matches!(project_kind, ProjectKind::Unknown) {
        needs.push("confirm_project_kind".to_string());
    }
    needs
}

fn infer_project_kind(project_root: &Path) -> ProjectKind {
    let markers = [
        "package.json",
        "tsconfig.json",
        "pom.xml",
        "build.gradle",
        "pyproject.toml",
        "go.mod",
        "Cargo.toml",
        "requirements.txt",
        "app",
        "src",
        "frontend",
        "backend",
    ];
    if markers
        .iter()
        .any(|marker| project_root.join(marker).exists())
    {
        ProjectKind::ExistingProject
    } else {
        ProjectKind::Greenfield
    }
}

fn read_brainstorm_contract(
    project_root: &Path,
    relative_ref: &str,
) -> Result<BrainstormContract, state::store::StateError> {
    let absolute = from_project_relative(project_root, relative_ref)?;
    state::store::read_json(&absolute)
}

fn ensure_latest_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    latest_ref_key: &str,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    if delivery.active_phase_id != phase_id {
        return Ok(Some(stale_failure(
            project_root,
            "TechnicalBaseline submit must bind to the active phase.".to_string(),
        )));
    }
    let Some(phase) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return Ok(Some(stale_failure(
            project_root,
            format!("delivery {} is missing phase {}", delivery_id, phase_id),
        )));
    };
    if phase.latest_refs.get(latest_ref_key).map(String::as_str) != Some(request_ref) {
        return Ok(Some(stale_failure(
            project_root,
            "TechnicalBaseline submit must use the active phase latest requestRef.".to_string(),
        )));
    }
    Ok(None)
}

fn technical_baseline_user_gate(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    prompt: String,
    gate_id: String,
) -> LoomMcpActionResult {
    LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
        project_root: input.project_root.clone(),
        prompt,
        accepted_responses: vec!["reply_in_chat".to_string()],
        request_ref: Some(input.request_ref.clone()),
        delivery_id: authorized.delivery_id.clone(),
        phase_id: authorized.phase_id.clone(),
        gate: Some(json!({
            "gateId": gate_id,
            "kind": "technical_baseline_confirmation"
        })),
    })
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
        resubmit_tool: "loom.technicalBaselineAcceptFile".to_string(),
        fix_scope: Some("technical_baseline_candidate_only".to_string()),
        read_groups: authorized.read_groups.clone(),
    })
}

fn stale_failure(project_root: &str, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: "STALE_TECHNICAL_BASELINE_REQUEST".to_string(),
            message,
            target_batch: Some(8),
            domain: Some("planning".to_string()),
            route_action: Some("technical_baseline_accept".to_string()),
            recovery_tool: Some("loom.continue".to_string()),
        },
    })
}

fn issue(code: &str, field_path: &str, message: &str) -> delivery_core::RepairIssue {
    delivery_core::RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("candidate".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}
