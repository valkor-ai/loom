use std::{collections::BTreeSet, path::Path};

use contracts::{
    BrainstormContract, BrainstormStatus, ClarificationBlockName, PhaseDevelopmentMode,
    ProjectKind, RepositoryContextCandidateAgentWritable, RepositoryContextContract,
    RepositoryMode, TechnicalBaselineContract,
};
use delivery_core::{
    read_selectors_value_from_paths, ArtifactKind, DomainDispatcher, FileSubmitInput,
    LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult, LoomMcpRepairableErrorResult,
    LoomMcpUserGateResult, OperationContext, RouteAction, RouteActionKind, SubmitAcceptedEvent,
    TransitionEngine, TransitionStore,
};
use schemars::schema_for;
use serde_json::{json, Map, Value};
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
    let repository_lens = phase_repository_lens(root, &delivery, phase_id, baseline.project_kind);
    let request_root = build_request_root(
        &request_id,
        delivery_id,
        phase_id,
        &candidate_file,
        &brainstorm_contract_ref,
        &baseline,
        repository_lens,
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
    repository_lens: RepositoryLens,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(RepositoryContextCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let mut scan_purpose = json!({
        "scanPurpose": "phase_start_repository_snapshot",
        "primaryConsumer": "phase_brainstorm",
        "laterConsumers": ["PGC", "AAC", "TaskPlan"]
    });
    if !repository_lens.completed_phase_summaries.is_empty() {
        scan_purpose["completedPhaseSummaries"] =
            Value::Array(repository_lens.completed_phase_summaries.clone());
    }
    let mut generation_rules = vec![
        "Summarize repository code facts only.",
        "Do not restate or infer current phase scope, acceptance, tasks, or review conclusions.",
        "All paths must stay inside projectRoot and must not use forbidden prefixes.",
        "technologySignals and structureSignals are objects, not arrays.",
        "repoOverview.primaryApplications, structureSignals.rootPaths, structureSignals.entryPoints, existingCapabilities, relevantSurfaces, and recommendedReadRefs are arrays of objects with the fields shown in outputContract.resultTemplate.",
        "Every existingCapabilities[].surfaceRefs and recommendedReadRefs[].surfaceRefs value must reference a relevantSurfaces[].surfaceId.",
        "Use enumRefs.surfaceRelevance only for relevantSurfaces[].relevance; integration_boundary is not a surface relevance value and belongs only to recommendedReadRefs[].reason.",
        "Use architecture_boundary for code surfaces that define API, module, service, frontend/backend, or persistence boundaries.",
    ];
    if matches!(
        repository_lens.phase_development_mode,
        PhaseDevelopmentMode::IncrementalDelivery
    ) {
        generation_rules.push("When scanPurpose.completedPhaseSummaries exists, inspect and report current repository facts after those delivered phases instead of treating the repository as blank.");
    }
    let mut scan_contract_fields = vec![
        "baselineProjectKind",
        "repositoryMode",
        "phaseDevelopmentMode",
        "scanPurpose.scanPurpose",
        "scanPurpose.primaryConsumer",
        "scanPurpose.laterConsumers",
        "source.brainstormContractRef",
        "source.technicalBaselineRef",
    ];
    if !repository_lens.completed_phase_summaries.is_empty() {
        scan_contract_fields.push("scanPurpose.completedPhaseSummaries");
    }
    json!({
        "schemaVersion": "1.0",
        "requestType": "repository_context_request",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "baselineProjectKind": baseline.project_kind,
        "repositoryMode": repository_lens.repository_mode,
        "phaseDevelopmentMode": repository_lens.phase_development_mode,
        "source": {
            "brainstormContractRef": brainstorm_contract_ref,
            "technicalBaselineRef": format!(".loom/deliveries/{}/contracts/technical-baseline.json", delivery_id)
        },
        "scanPurpose": scan_purpose,
        "generationRules": generation_rules,
        "enumRefs": {
            "projectKind": ["new_project", "existing_project", "unknown"],
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
            "schemaShape": schema_shape,
            "schemaProjection": {
                "requiredTopLevelFields": [
                    "status",
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
                },
                "objectShapeRules": {
                    "technologySignals": "object with primaryLanguages, frameworks, packageManagers, buildCommands, testCommands, notes arrays",
                    "structureSignals": "object with rootPaths, entryPoints, configurationFiles arrays",
                    "repoOverview.primaryApplications[]": "objects with applicationId, name, kind, rootPath",
                    "relevantSurfaces[]": "objects with surfaceId, kind, path, summary, relevance, suggestedUse",
                    "recommendedReadRefs[]": "objects with path, reason, priority, summary, surfaceRefs",
                    "contextQuality.warnings[]": "objects with code and message; use [] only when there are no warnings",
                    "warnings[]": "objects with code and message; use [] only when there are no warnings"
                }
            },
            "resultTemplate": {
                "status": "ready",
                "repoOverview": {
                    "summary": "Short repository summary from the current phase perspective.",
                    "repositoryShape": "unknown",
                    "primaryApplications": [{
                        "applicationId": "app-main",
                        "name": "Main application",
                        "kind": "service | cli | web_app | library | unknown",
                        "rootPath": "."
                    }]
                },
                "technologySignals": {
                    "primaryLanguages": ["language-name"],
                    "frameworks": ["framework-name"],
                    "packageManagers": ["package-manager"],
                    "buildCommands": ["build command"],
                    "testCommands": ["test command"],
                    "notes": ["Short technology note. Use [] only when none."]
                },
                "structureSignals": {
                    "rootPaths": [{
                        "path": ".",
                        "role": "source_root | app_root | config_root | test_root | documentation_root"
                    }],
                    "entryPoints": [{
                        "path": "project-relative/path",
                        "kind": "module | cli | server | page | test | config | unknown",
                        "description": "Why this entry point matters."
                    }],
                    "configurationFiles": ["project-relative/config-file"]
                },
                "existingCapabilities": [{
                    "capabilityId": "cap-existing-example",
                    "name": "Existing capability name",
                    "status": "partial",
                    "summary": "Observed repository capability from the current codebase.",
                    "surfaceRefs": ["surface-example"],
                    "confidence": "medium",
                    "deliveryRelevance": "Why this matters to the overall delivery or upcoming Brainstorm."
                }],
                "relevantSurfaces": [{
                    "surfaceId": "surface-example",
                    "kind": "module",
                    "path": "project-relative/path",
                    "summary": "Surface summary.",
                    "relevance": "extension_point",
                    "suggestedUse": "inspect_or_extend"
                }],
                "recommendedReadRefs": [{
                    "path": "project-relative/path",
                    "reason": "implemented_capability",
                    "priority": "medium",
                    "summary": "Why the agent should read this file first.",
                    "surfaceRefs": ["surface-example"]
                }],
                "contextQuality": {
                    "coverage": "focused",
                    "confidence": "medium",
                    "warnings": [{
                        "code": "LOW_CONFIDENCE_REPOSITORY_SCAN",
                        "message": "Use [] only when there are no warnings."
                    }]
                },
                "warnings": [{
                    "code": "LOW_CONFIDENCE_REPOSITORY_SCAN",
                    "message": "Use [] only when there are no warnings."
                }]
            }
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "repository_context_scan_contract",
                    "required": true,
                    "purpose": "Read the repository scanning purpose and baseline lens before inspecting the repository.",
                    "whenToRead": "Read before any repository inspection.",
                    "selectors": read_selectors_value_from_paths(scan_contract_fields)
                },
                {
                    "groupId": "repository_context_generation_rules",
                    "required": true,
                    "purpose": "Read the hard safety rules and enum sets before writing RepositoryContext.",
                    "whenToRead": "Read before drafting the candidate.",
                    "selectors": read_selectors_value_from_paths([
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
                    ])
                },
                {
                    "groupId": "repository_context_write_contract",
                    "required": true,
                    "purpose": "Read the write target and schema projection before writing the candidate.",
                    "whenToRead": "Read only when ready to write RepositoryContext.",
                    "selectors": read_selectors_value_from_paths([
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
                        "outputContract.schemaProjection",
                        "outputContract.resultTemplate"
                    ])
                }
            ]
        }
    })
}

#[derive(Debug, Clone)]
struct RepositoryLens {
    repository_mode: RepositoryMode,
    phase_development_mode: PhaseDevelopmentMode,
    completed_phase_summaries: Vec<Value>,
}

fn phase_repository_lens(
    project_root: &Path,
    delivery: &delivery_core::DeliveryIndex,
    phase_id: &str,
    baseline_project_kind: ProjectKind,
) -> RepositoryLens {
    let completed_phase_count = delivery
        .phases
        .iter()
        .position(|phase| phase.phase_id == phase_id)
        .unwrap_or(0);
    let completed_phase_summaries = delivery
        .phases
        .iter()
        .take(completed_phase_count)
        .map(|phase| completed_phase_summary(project_root, phase))
        .collect::<Vec<_>>();
    if completed_phase_count > 0 {
        return RepositoryLens {
            repository_mode: RepositoryMode::ExistingProject,
            phase_development_mode: PhaseDevelopmentMode::IncrementalDelivery,
            completed_phase_summaries,
        };
    }
    match baseline_project_kind {
        ProjectKind::NewProject => RepositoryLens {
            repository_mode: RepositoryMode::EmptyProject,
            phase_development_mode: PhaseDevelopmentMode::InitialDelivery,
            completed_phase_summaries,
        },
        ProjectKind::ExistingProject => RepositoryLens {
            repository_mode: RepositoryMode::ExistingProject,
            phase_development_mode: PhaseDevelopmentMode::InitialDelivery,
            completed_phase_summaries,
        },
        ProjectKind::Unknown => RepositoryLens {
            repository_mode: RepositoryMode::Unknown,
            phase_development_mode: PhaseDevelopmentMode::Unknown,
            completed_phase_summaries,
        },
    }
}

fn completed_phase_summary(
    project_root: &Path,
    phase: &delivery_core::DeliveryPhaseState,
) -> Value {
    let mut summary = Map::new();
    summary.insert("phaseId".to_string(), json!(phase.phase_id));
    summary.insert("status".to_string(), json!("completed"));
    if let Some(brainstorm_ref) = phase.latest_refs.get("brainstormContract") {
        if let Ok(file) = from_project_relative(project_root, brainstorm_ref) {
            if let Ok(brainstorm) = state::store::read_json::<BrainstormContract>(&file) {
                summary.insert(
                    "title".to_string(),
                    json!(brainstorm.phase_plan.current.title),
                );
                summary.insert(
                    "goal".to_string(),
                    json!(brainstorm.phase_plan.current.goal),
                );
            }
        }
    }
    Value::Object(summary)
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
    let request_index =
        state::request_index::get_request_index_entry(&input.project_root, &authorized.request_id)?;
    let request_file = from_project_relative(project_root, &request_index.request_file)?;
    let request_root = state::store::read_json_value(&request_file)?;
    let candidate: RepositoryContextCandidateAgentWritable =
        match state::store::read_json_value(&candidate_file)
            .map(|mut raw| {
                normalize_repository_context_candidate_value(
                    &mut raw,
                    &request_root,
                    &input.request_ref,
                );
                raw
            })
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
    let issues = validate_repository_context(project_root, &candidate);
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

fn normalize_repository_context_candidate_value(
    raw: &mut Value,
    request: &Value,
    request_ref: &str,
) {
    let Some(object) = raw.as_object_mut() else {
        return;
    };
    object.insert(
        "source".to_string(),
        json!({
            "requestRef": request_ref,
            "brainstormContractRef": request.pointer("/source/brainstormContractRef").cloned().unwrap_or(Value::Null),
            "technicalBaselineRef": request.pointer("/source/technicalBaselineRef").cloned().unwrap_or(Value::Null)
        }),
    );
    object.insert(
        "requestLens".to_string(),
        json!({
            "projectKind": request.get("baselineProjectKind").cloned().unwrap_or(Value::String("unknown".to_string())),
            "baselineProjectKind": request.get("baselineProjectKind").cloned().unwrap_or(Value::String("unknown".to_string())),
            "repositoryMode": request.get("repositoryMode").cloned().unwrap_or(Value::String("unknown".to_string())),
            "phaseDevelopmentMode": request.get("phaseDevelopmentMode").cloned().unwrap_or(Value::String("unknown".to_string())),
            "scanPurpose": request.pointer("/scanPurpose/scanPurpose").cloned().unwrap_or(Value::String("phase_start_repository_snapshot".to_string())),
            "primaryConsumer": request.pointer("/scanPurpose/primaryConsumer").cloned().unwrap_or(Value::String("phase_brainstorm".to_string())),
            "laterConsumers": request.pointer("/scanPurpose/laterConsumers").cloned().unwrap_or_else(|| json!([]))
        }),
    );
    normalize_repo_overview(object.get_mut("repoOverview"));
    normalize_relevant_surfaces(object.get_mut("relevantSurfaces"));
    normalize_recommended_read_refs(object.get_mut("recommendedReadRefs"));
    normalize_context_quality(object.get_mut("contextQuality"));
    normalize_context_warnings(object.get_mut("warnings"), "REPOSITORY_CONTEXT_WARNING");
}

fn normalize_repo_overview(value: Option<&mut Value>) {
    let Some(object) = value.and_then(Value::as_object_mut) else {
        return;
    };
    normalize_enum_field(
        object,
        "repositoryShape",
        &[
            ("multi_app", "multi_application"),
            ("multi_application_repo", "multi_application"),
            ("single_app", "single_package"),
        ],
        &["single_package", "monorepo", "multi_application", "unknown"],
        "unknown",
    );
}

fn normalize_relevant_surfaces(value: Option<&mut Value>) {
    let Some(items) = value.and_then(Value::as_array_mut) else {
        return;
    };
    for (index, item) in items.iter_mut().enumerate() {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        if !object
            .get("surfaceId")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            object.insert(
                "surfaceId".to_string(),
                json!(format!("surface-{}", index + 1)),
            );
        }
        normalize_enum_field(
            object,
            "kind",
            &[
                ("api", "controller"),
                ("database", "data_access"),
                ("frontend", "ui"),
                ("backend", "service"),
            ],
            &[
                "entrypoint",
                "module",
                "service",
                "controller",
                "data_access",
                "ui",
                "config",
                "test",
                "script",
                "documentation",
                "unknown",
            ],
            "unknown",
        );
        normalize_enum_field(
            object,
            "relevance",
            &[
                ("integration_boundary", "architecture_boundary"),
                ("dependency_context", "architecture_boundary"),
                ("test_or_validation", "validation_surface"),
                ("risk_review", "delivery_context"),
            ],
            &[
                "implemented_capability",
                "architecture_boundary",
                "extension_point",
                "validation_surface",
                "delivery_context",
                "unrelated",
            ],
            "delivery_context",
        );
        normalize_enum_field(
            object,
            "suggestedUse",
            &[
                ("inspect", "inspect_only"),
                ("read_only", "inspect_only"),
                ("extend", "inspect_or_extend"),
                ("reuse", "reuse_existing_pattern"),
                ("avoid", "avoid_modifying"),
            ],
            &[
                "inspect_only",
                "inspect_or_extend",
                "reuse_existing_pattern",
                "avoid_modifying",
            ],
            "inspect_only",
        );
    }
}

fn normalize_recommended_read_refs(value: Option<&mut Value>) {
    let Some(items) = value.and_then(Value::as_array_mut) else {
        return;
    };
    for item in items {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        normalize_enum_field(
            object,
            "reason",
            &[
                ("architecture_boundary", "integration_boundary"),
                ("validation_surface", "test_or_validation"),
                ("delivery_context", "dependency_context"),
            ],
            &[
                "implemented_capability",
                "dependency_context",
                "integration_boundary",
                "test_or_validation",
                "risk_review",
                "extension_point",
            ],
            "dependency_context",
        );
        normalize_enum_field(
            object,
            "priority",
            &[],
            &["high", "medium", "low"],
            "medium",
        );
    }
}

fn normalize_context_quality(value: Option<&mut Value>) {
    let Some(object) = value.and_then(Value::as_object_mut) else {
        return;
    };
    normalize_enum_field(
        object,
        "coverage",
        &[
            ("complete", "broad"),
            ("comprehensive", "broad"),
            ("high", "broad"),
            ("medium", "partial"),
            ("low", "insufficient"),
        ],
        &["focused", "partial", "broad", "insufficient"],
        "partial",
    );
    normalize_enum_field(
        object,
        "confidence",
        &[("certain", "high"), ("none", "unknown")],
        &["low", "medium", "high", "unknown"],
        "unknown",
    );
    normalize_context_warnings(
        object.get_mut("warnings"),
        "REPOSITORY_CONTEXT_QUALITY_WARNING",
    );
}

fn normalize_context_warnings(value: Option<&mut Value>, default_code: &str) {
    let Some(value) = value else {
        return;
    };
    match value {
        Value::Array(items) => {
            for item in items {
                normalize_context_warning_item(item, default_code);
            }
        }
        Value::String(message) => {
            *value = json!([{ "code": default_code, "message": message.clone() }]);
        }
        _ => {
            *value = json!([]);
        }
    }
}

fn normalize_context_warning_item(item: &mut Value, default_code: &str) {
    match item {
        Value::String(message) => {
            *item = json!({ "code": default_code, "message": message.clone() });
        }
        Value::Object(object) => {
            if !object
                .get("code")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
            {
                object.insert("code".to_string(), json!(default_code));
            }
            if !object
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
            {
                object.insert("message".to_string(), json!("Repository context warning."));
            }
        }
        _ => {
            *item = json!({ "code": default_code, "message": "Repository context warning." });
        }
    }
}

fn normalize_enum_field(
    object: &mut Map<String, Value>,
    field: &str,
    aliases: &[(&str, &str)],
    allowed: &[&str],
    default_value: &str,
) {
    let candidate = object
        .get(field)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let normalized = aliases
        .iter()
        .find_map(|(from, to)| (candidate == *from).then_some(*to))
        .or_else(|| {
            allowed
                .iter()
                .copied()
                .find(|allowed| candidate == *allowed)
        })
        .unwrap_or(default_value);
    object.insert(field.to_string(), json!(normalized));
}

fn validate_repository_context(
    project_root: &Path,
    context: &RepositoryContextCandidateAgentWritable,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
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
    let mut gate = brainstorm::phase_scope_gate();
    if let Some(object) = gate.as_object_mut() {
        object.insert("gateId".to_string(), json!("phase_brainstorm_required"));
        object.insert("kind".to_string(), json!("phase_brainstorm_continuation"));
        object.insert("requestRef".to_string(), json!(request_ref));
    }
    LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
        project_root: project_root.to_string(),
        prompt: brainstorm::phase_scope_prompt(phase_id),
        accepted_responses: vec!["reply_in_chat".to_string()],
        request_ref: Some(request_ref.to_string()),
        delivery_id: Some(delivery_id.to_string()),
        phase_id: Some(phase_id.to_string()),
        gate: Some(gate),
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
