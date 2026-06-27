use std::{collections::BTreeSet, path::Path};

use contracts::{
    BrainstormContract, BrainstormStatus, ClarificationBlockName,
    RepositoryContextCandidateAgentWritable, RepositoryContextContract, TechnicalBaselineContract,
};
use delivery_core::{
    ArtifactKind, DomainDispatcher, FileSubmitInput, LoomMcpActionResult, LoomMcpFailure,
    LoomMcpFailureResult, LoomMcpRepairableErrorResult, LoomMcpUserGateResult, OperationContext,
    RouteAction, RouteActionKind, SubmitAcceptedEvent, TransitionEngine, TransitionStore,
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
        repository_context_candidate_file, repository_context_file,
        repository_context_request_file, technical_baseline_file,
    },
    write_artifact_result,
};

const FORBIDDEN_PREFIXES: &[&str] = &[".git/", ".loom/", "node_modules/"];

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
                code: "REPOSITORY_CONTEXT_REQUEST_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("planning".to_string()),
                route_action: Some("repository_context_request".to_string()),
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
        .get("repositoryContextRequestRef")
        .cloned()
    {
        let inspected = state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        });
        if inspected
            .as_ref()
            .map(|request| request.request_kind == "repository_context_request")
            .unwrap_or(false)
        {
            return write_artifact_result(
                project_root,
                &existing_request_ref,
                ArtifactKind::RepositoryContextCandidate,
            );
        }
    }
    let brainstorm_contract_ref = phase
        .latest_refs
        .get("brainstormContract")
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest brainstormContract ref is missing".to_string(),
            )
        })?
        .clone();
    let baseline = read_baseline(root, delivery_id)?;
    let request_id = format!("repoctx_{}", state::store::now_millis());
    let candidate_file = to_project_relative(
        root,
        &repository_context_candidate_file(root, &locator, &request_id),
    )?;
    let request_file = to_project_relative(
        root,
        &repository_context_request_file(root, &locator, &request_id),
    )?;
    let request_root = build_request_root(
        &request_id,
        delivery_id,
        phase_id,
        &candidate_file,
        &brainstorm_contract_ref,
        &baseline,
    );
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "repository_context_request".to_string(),
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
            .insert("repositoryContextRequestId".to_string(), request_id);
        active_phase.latest_refs.insert(
            "repositoryContextRequestRef".to_string(),
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
        ArtifactKind::RepositoryContextCandidate,
    )
}

fn build_request_root(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    candidate_file: &str,
    brainstorm_contract_ref: &str,
    baseline: &TechnicalBaselineContract,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(RepositoryContextCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    json!({
        "schemaVersion": "1.0",
        "requestType": "repository_context_request",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "baselineProjectKind": baseline.project_kind,
        "repositoryMode": "existing_project",
        "phaseDevelopmentMode": "initial_delivery",
        "source": {
            "brainstormContractRef": brainstorm_contract_ref,
            "technicalBaselineRef": format!(".loom/deliveries/{}/contracts/technical-baseline.json", delivery_id)
        },
        "scanPurpose": {
            "scanPurpose": "phase_start_repository_snapshot",
            "primaryConsumer": "phase_brainstorm",
            "laterConsumers": ["PGC", "AAC", "TaskPlan"]
        },
        "generationRules": [
            "Summarize repository code facts only.",
            "Do not restate or infer current phase scope, acceptance, tasks, or review conclusions.",
            "All paths must stay inside projectRoot and must not use forbidden prefixes."
        ],
        "enumRefs": {
            "projectKind": ["greenfield", "existing_project", "unknown"],
            "repositoryMode": ["empty_project", "existing_project", "unknown"],
            "phaseDevelopmentMode": ["initial_delivery", "incremental_delivery", "unknown"],
            "repositoryShape": ["single_package", "monorepo", "multi_application", "unknown"],
            "capabilityStatus": ["implemented", "partial", "missing", "unknown"],
            "relevantSurfaceKind": ["entrypoint", "module", "service", "controller", "data_access", "ui", "config", "test", "script", "documentation", "unknown"],
            "surfaceRelevance": ["implemented_capability", "architecture_boundary", "extension_point", "validation_surface", "delivery_context", "unrelated"],
            "suggestedUse": ["inspect_only", "inspect_or_extend", "reuse_existing_pattern", "avoid_modifying"],
            "recommendedReadReason": ["implemented_capability", "dependency_context", "integration_boundary", "test_or_validation", "risk_review", "extension_point"],
            "readPriority": ["high", "medium", "low"],
            "contextCoverage": ["focused", "partial", "broad", "insufficient"],
            "confidence": ["low", "medium", "high", "unknown"]
        },
        "outputContract": {
            "artifactKind": ArtifactKind::RepositoryContextCandidate,
            "writeMode": "single_json",
            "submitTool": "loom.repositoryContextAcceptFile",
            "writeTargets": [{
                "targetId": "candidate",
                "path": candidate_file,
                "required": true,
                "description": "Write the RepositoryContext candidate JSON."
            }],
            "bindingRules": [
                "source.requestRef must equal the current requestRef passed to loom.repositoryContextAcceptFile.",
                "Do not write source.requestId; requestRef is the required field."
            ],
            "schemaShape": schema_shape,
            "schemaProjection": {
                "requiredTopLevelFields": [
                    "status",
                    "source",
                    "requestLens",
                    "repoOverview",
                    "technologySignals",
                    "structureSignals",
                    "relevantSurfaces",
                    "contextQuality"
                ],
                "enumFields": {
                    "requestLens.projectKind": "enumRefs.projectKind",
                    "requestLens.baselineProjectKind": "enumRefs.projectKind",
                    "requestLens.repositoryMode": "enumRefs.repositoryMode",
                    "requestLens.phaseDevelopmentMode": "enumRefs.phaseDevelopmentMode",
                    "repoOverview.repositoryShape": "enumRefs.repositoryShape",
                    "existingCapabilities[].status": "enumRefs.capabilityStatus",
                    "existingCapabilities[].confidence": "enumRefs.confidence",
                    "relevantSurfaces[].kind": "enumRefs.relevantSurfaceKind",
                    "relevantSurfaces[].relevance": "enumRefs.surfaceRelevance",
                    "relevantSurfaces[].suggestedUse": "enumRefs.suggestedUse",
                    "recommendedReadRefs[].reason": "enumRefs.recommendedReadReason",
                    "recommendedReadRefs[].priority": "enumRefs.readPriority",
                    "contextQuality.coverage": "enumRefs.contextCoverage",
                    "contextQuality.confidence": "enumRefs.confidence"
                }
            },
            "resultTemplate": {
                "status": "ready",
                "source": {
                    "requestRef": "{current requestRef}",
                    "brainstormContractRef": brainstorm_contract_ref,
                    "technicalBaselineRef": format!(".loom/deliveries/{}/contracts/technical-baseline.json", delivery_id)
                },
                "requestLens": {
                    "projectKind": baseline.project_kind,
                    "baselineProjectKind": baseline.project_kind,
                    "repositoryMode": "existing_project",
                    "phaseDevelopmentMode": "initial_delivery",
                    "scanPurpose": "phase_start_repository_snapshot",
                    "primaryConsumer": "phase_brainstorm",
                    "laterConsumers": ["PGC", "AAC", "TaskPlan"]
                },
                "repoOverview": {
                    "summary": "",
                    "repositoryShape": "unknown",
                    "primaryApplications": []
                },
                "technologySignals": {
                    "primaryLanguages": [],
                    "frameworks": [],
                    "packageManagers": [],
                    "buildCommands": [],
                    "testCommands": [],
                    "notes": []
                },
                "structureSignals": {
                    "rootPaths": [],
                    "entryPoints": [],
                    "configurationFiles": []
                },
                "existingCapabilities": [],
                "relevantSurfaces": [],
                "recommendedReadRefs": [],
                "contextQuality": {
                    "coverage": "focused",
                    "confidence": "medium",
                    "warnings": []
                },
                "warnings": []
            }
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "repository_context_scan_contract",
                    "required": true,
                    "purpose": "Read the repository scanning purpose and baseline lens before inspecting the repository.",
                    "whenToRead": "Read before any repository inspection.",
                    "fields": [
                        "baselineProjectKind",
                        "repositoryMode",
                        "phaseDevelopmentMode",
                        "scanPurpose.scanPurpose",
                        "scanPurpose.primaryConsumer",
                        "scanPurpose.laterConsumers",
                        "source.brainstormContractRef",
                        "source.technicalBaselineRef"
                    ]
                },
                {
                    "groupId": "repository_context_generation_rules",
                    "required": true,
                    "purpose": "Read the hard safety rules and enum sets before writing RepositoryContext.",
                    "whenToRead": "Read before drafting the candidate.",
                    "fields": [
                        "generationRules",
                        "enumRefs.projectKind",
                        "enumRefs.repositoryMode",
                        "enumRefs.phaseDevelopmentMode",
                        "enumRefs.repositoryShape",
                        "enumRefs.capabilityStatus",
                        "enumRefs.relevantSurfaceKind",
                        "enumRefs.surfaceRelevance",
                        "enumRefs.suggestedUse",
                        "enumRefs.recommendedReadReason",
                        "enumRefs.readPriority",
                        "enumRefs.contextCoverage",
                        "enumRefs.confidence"
                    ]
                },
                {
                    "groupId": "repository_context_write_contract",
                    "required": true,
                    "purpose": "Read the write target and schema projection before writing the candidate.",
                    "whenToRead": "Read only when ready to write RepositoryContext.",
                    "fields": [
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
                        "outputContract.bindingRules",
                        "outputContract.schemaProjection",
                        "outputContract.resultTemplate"
                    ]
                }
            ]
        }
    })
}

pub fn accept_repository_context_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher + Clone,
{
    match accept_repository_context_file_inner(input, authorized, dispatcher) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "REPOSITORY_CONTEXT_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("planning".to_string()),
                route_action: Some("repository_context_accept".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn accept_repository_context_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher + Clone,
{
    let Some(target) = authorized.targets.first() else {
        return Ok(repairable(
            input,
            authorized,
            String::new(),
            vec![issue(
                "TARGET_MISSING",
                "candidate",
                "No authorized RepositoryContext target was written.",
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
        "repositoryContextRequestRef",
    )? {
        return Ok(result);
    }
    let project_root = Path::new(&input.project_root);
    let candidate_file = from_project_relative(project_root, &target.path)?;
    let candidate: RepositoryContextCandidateAgentWritable =
        match state::store::read_json_value(&candidate_file)
            .and_then(|raw| serde_json::from_value(raw).map_err(state::store::StateError::Json))
        {
            Ok(candidate) => candidate,
            Err(error) => {
                return Ok(repairable(
                    input,
                    authorized,
                    target.path.clone(),
                    vec![issue(
                        "REPOSITORY_CONTEXT_SCHEMA_INVALID",
                        "candidate",
                        &format!("RepositoryContext candidate JSON has an invalid schema: {error}"),
                    )],
                ));
            }
        };
    let issues = validate_repository_context(project_root, &candidate, &input.request_ref);
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, target.path.clone(), issues));
    }
    let now = state::store::now_string();
    let persisted = RepositoryContextContract {
        schema_version: "1.0".to_string(),
        repository_context_id: format!("repoctx_{}_{}", phase_id, state::store::now_millis()),
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
        status: candidate.status,
        source: candidate.source,
        request_lens: candidate.request_lens,
        repo_overview: candidate.repo_overview,
        technology_signals: candidate.technology_signals,
        structure_signals: candidate.structure_signals,
        existing_capabilities: candidate.existing_capabilities,
        relevant_surfaces: candidate.relevant_surfaces,
        recommended_read_refs: candidate.recommended_read_refs,
        context_quality: candidate.context_quality,
        warnings: candidate.warnings,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let context_file = repository_context_file(project_root, &locator);
    state::store::write_json_atomic(&context_file, &persisted)?;
    let context_ref = to_project_relative(project_root, &context_file)?;

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
            "repositoryContextRequestRef".to_string(),
            input.request_ref.clone(),
        );
        phase
            .latest_refs
            .insert("latestRepositoryContext".to_string(), context_ref.clone());
    }
    delivery.updated_at = now;
    store
        .save_delivery_index(&input.project_root, &delivery)
        .map_err(to_state_error)?;

    if !current_phase_scope_confirmed(&input.project_root, &delivery_id, &phase_id)? {
        if let Some(phase_brainstorm) = brainstorm::materialize_phase_brainstorm_from_preview(
            &input.project_root,
            &delivery_id,
            &phase_id,
            Some(&context_ref),
        )? {
            return Ok(phase_brainstorm_user_gate(
                &input.project_root,
                &delivery_id,
                &phase_id,
                &phase_brainstorm.request_ref,
            ));
        }
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
                delivery_id,
                phase_id,
                source_tool: "loom.repositoryContextAcceptFile".to_string(),
                accepted_artifact_ref: format!(
                    "{}/targets/{}",
                    input.request_ref, target.target_id
                ),
                next_action: Some(RouteAction {
                    kind: RouteActionKind::PlanningContractCreate,
                    source: "repository_context_accept".to_string(),
                    reason: "repository_context_ready".to_string(),
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: None,
                    details: None,
                    target_phase_id: None,
                }),
            },
        )
        .map_err(to_state_error)
}

fn read_baseline(
    project_root: &Path,
    delivery_id: &str,
) -> Result<TechnicalBaselineContract, state::store::StateError> {
    state::store::read_json(&technical_baseline_file(project_root, delivery_id))
}

fn current_phase_scope_confirmed(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Result<bool, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let Some(phase) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return Ok(false);
    };
    let Some(brainstorm_ref) = phase.latest_refs.get("brainstormContract") else {
        return Ok(false);
    };
    let root = Path::new(project_root);
    let brainstorm: BrainstormContract =
        state::store::read_json(&from_project_relative(root, brainstorm_ref)?)?;
    if !matches!(brainstorm.status, BrainstormStatus::Confirmed) {
        return Ok(false);
    }
    if brainstorm.phase_id != phase_id || brainstorm.phase_plan.current.phase_id != phase_id {
        return Ok(false);
    }
    Ok(brainstorm
        .clarification_progress
        .as_ref()
        .map(|progress| {
            progress.confirmed_blocks.iter().any(|block| {
                block.block == ClarificationBlockName::PhaseScope && block.confirmed_by_user
            })
        })
        .unwrap_or(false))
}

fn validate_repository_context(
    project_root: &Path,
    context: &RepositoryContextCandidateAgentWritable,
    request_ref: &str,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if context.source.request_ref != request_ref {
        issues.push(issue(
            "REQUEST_REF_MISMATCH",
            "source.requestRef",
            "RepositoryContext source.requestRef must match the accepted request.",
        ));
    }
    let surface_ids = context
        .relevant_surfaces
        .iter()
        .map(|surface| surface.surface_id.clone())
        .collect::<BTreeSet<_>>();
    for surface in &context.relevant_surfaces {
        validate_relative_path(
            project_root,
            &surface.path,
            "relevantSurfaces.path",
            &mut issues,
        );
    }
    for root_path in &context.structure_signals.root_paths {
        validate_relative_path(
            project_root,
            &root_path.path,
            "structureSignals.rootPaths.path",
            &mut issues,
        );
    }
    for entry in &context.structure_signals.entry_points {
        validate_relative_path(
            project_root,
            &entry.path,
            "structureSignals.entryPoints.path",
            &mut issues,
        );
    }
    for file in &context.structure_signals.configuration_files {
        validate_relative_path(
            project_root,
            file,
            "structureSignals.configurationFiles",
            &mut issues,
        );
    }
    for capability in &context.existing_capabilities {
        for surface_ref in &capability.surface_refs {
            if !surface_ids.contains(surface_ref) {
                issues.push(issue(
                    "SURFACE_REF_INVALID",
                    "existingCapabilities.surfaceRefs",
                    "existingCapabilities.surfaceRefs must reference relevantSurfaces.surfaceId.",
                ));
                break;
            }
        }
    }
    for reference in &context.recommended_read_refs {
        validate_relative_path(
            project_root,
            &reference.path,
            "recommendedReadRefs.path",
            &mut issues,
        );
        for surface_ref in &reference.surface_refs {
            if !surface_ids.contains(surface_ref) {
                issues.push(issue(
                    "READ_REF_SURFACE_INVALID",
                    "recommendedReadRefs.surfaceRefs",
                    "recommendedReadRefs.surfaceRefs must reference relevantSurfaces.surfaceId.",
                ));
                break;
            }
        }
    }
    issues
}

fn validate_relative_path(
    project_root: &Path,
    relative: &str,
    field: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if relative.trim().is_empty() {
        issues.push(issue(
            "PATH_REQUIRED",
            field,
            "RepositoryContext paths must not be empty.",
        ));
        return;
    }
    if FORBIDDEN_PREFIXES
        .iter()
        .any(|prefix| relative.starts_with(prefix))
    {
        issues.push(issue(
            "FORBIDDEN_PATH_PREFIX",
            field,
            "RepositoryContext paths must not point into .git/, .loom/, or node_modules/.",
        ));
        return;
    }
    if from_project_relative(project_root, relative).is_err() {
        issues.push(issue(
            "PATH_OUTSIDE_PROJECT",
            field,
            "RepositoryContext paths must stay inside projectRoot.",
        ));
    }
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
            "RepositoryContext submit must bind to the active phase.".to_string(),
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
            "RepositoryContext submit must use the active phase latest requestRef.".to_string(),
        )));
    }
    Ok(None)
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
        resubmit_tool: "loom.repositoryContextAcceptFile".to_string(),
        fix_scope: Some("repository_context_candidate_only".to_string()),
        read_groups: authorized.read_groups.clone(),
    })
}

fn phase_brainstorm_user_gate(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
) -> LoomMcpActionResult {
    LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
        project_root: project_root.to_string(),
        prompt: "RepositoryContext accepted. Read the generated Brainstorm request, query request-scoped knowledge, and continue the progressive clarification for this phase.".to_string(),
        accepted_responses: vec!["reply_in_chat".to_string()],
        request_ref: Some(request_ref.to_string()),
        delivery_id: Some(delivery_id.to_string()),
        phase_id: Some(phase_id.to_string()),
        gate: Some(json!({
            "gateId": "phase_brainstorm_required",
            "kind": "phase_brainstorm_continuation",
            "requestRef": request_ref
        })),
    })
}

fn stale_failure(project_root: &str, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: "STALE_REPOSITORY_CONTEXT_REQUEST".to_string(),
            message,
            target_batch: Some(8),
            domain: Some("planning".to_string()),
            route_action: Some("repository_context_accept".to_string()),
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
