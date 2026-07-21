use std::{collections::BTreeSet, fs, path::Path};

use contracts::{
    BrainstormContract, ProjectKind, SecurityAlgorithm, SecurityKeySource, SecurityMechanism,
    SecurityRequirement, SecurityRequirementApplicability, SecurityTransport,
    TechnicalBaselineApprovalType, TechnicalBaselineCandidateAgentWritable,
    TechnicalBaselineContract, TechnicalBaselineStatus,
};
use delivery_core::{
    read_selectors_value_from_paths, ArtifactKind, DeliveryIndex, DomainDispatcher,
    FileSubmitInput, LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpRepairableErrorResult, LoomMcpUserGateResult, OperationContext, ReadRequestFieldsInput,
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
        repository_context_file, technical_baseline_candidate_file, technical_baseline_file,
        technical_baseline_request_file,
    },
    write_artifact_result,
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
    let request_id = format!("tbr_{}", state::store::now_millis());
    let candidate_file = to_project_relative(
        root,
        &technical_baseline_candidate_file(root, &locator, &request_id),
    )?;
    let request_file = to_project_relative(
        root,
        &technical_baseline_request_file(root, &locator, &request_id),
    )?;
    let previous_baseline_file = technical_baseline_file(root, delivery_id);
    let previous_baseline = if previous_baseline_file.exists() {
        let previous_baseline_ref = to_project_relative(root, &previous_baseline_file)?;
        let previous: TechnicalBaselineContract = state::store::read_json(&previous_baseline_file)?;
        Some((previous_baseline_ref, previous))
    } else {
        None
    };
    let project_kind =
        infer_project_kind_for_baseline(root, &delivery, phase_id, previous_baseline.is_some());
    let request_root = build_request_root(
        root,
        &brainstorm,
        delivery_id,
        phase_id,
        &request_id,
        &candidate_file,
        project_kind,
        previous_baseline.as_ref(),
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
    project_root: &Path,
    brainstorm: &BrainstormContract,
    delivery_id: &str,
    phase_id: &str,
    request_id: &str,
    candidate_file: &str,
    project_kind: ProjectKind,
    previous_baseline: Option<&(String, TechnicalBaselineContract)>,
) -> Value {
    let schema_shape = serde_json::to_value(schema_for!(TechnicalBaselineCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let baseline_exists = previous_baseline.is_some();
    let previous_baseline_context = previous_baseline.map(|(previous_ref, previous)| {
        json!({
            "previousBaselineRef": previous_ref,
            "technicalBaselineId": previous.technical_baseline_id,
            "status": previous.status,
            "projectKind": previous.project_kind,
            "scope": previous.scope,
            "stack": previous.stack,
            "constraints": previous.constraints,
            "confidence": previous.confidence,
            "securityProfiles": previous.security_profiles,
            "updatedAt": previous.updated_at
        })
    });
    let selection_guidance = technical_baseline_selection_guidance(project_kind, baseline_exists);
    let repo_evidence =
        technical_baseline_repo_evidence(project_root, project_kind, baseline_exists);
    let baseline_context_fields =
        technical_baseline_context_fields(brainstorm, previous_baseline.is_some());
    let repo_evidence_fields = technical_baseline_repo_evidence_fields(project_kind);
    let mut security_selection_fields = vec!["selectionGuidance"];
    if !matches!(
        brainstorm.security_requirement.applies,
        SecurityRequirementApplicability::NotApplicable
    ) {
        security_selection_fields.extend(["securityRequirement", "securityProfileGuidance"]);
    }
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
            "recommend_new_project_baseline"
        },
        "brainstormLens": {
            "summary": brainstorm.summary,
            "scopeIndex": {
                "includedIds": scope_item_ids(&brainstorm.scope.included),
                "includedLabels": scope_item_labels(&brainstorm.scope.included),
                "deferredIds": scope_item_ids(&brainstorm.scope.deferred),
                "deferredLabels": scope_item_labels(&brainstorm.scope.deferred),
                "excludedIds": scope_item_ids(&brainstorm.scope.excluded),
                "excludedLabels": scope_item_labels(&brainstorm.scope.excluded),
                "assumptionTexts": brainstorm.scope.assumptions.iter().map(|item| item.text.clone()).collect::<Vec<_>>()
            },
            "domainModel": brainstorm.domain_model.as_ref().map(|domain_model| json!({
                "capabilityNames": domain_model.capability_groups.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                "businessFlowNames": domain_model.business_flows.iter().map(|item| item.name.clone()).collect::<Vec<_>>()
            })),
            "acceptanceIndex": brainstorm.acceptance.iter().map(|acceptance| json!({
                "id": acceptance.id,
                "priority": acceptance.priority,
                "capabilityRefs": acceptance.capability_refs,
                "sourceRefs": acceptance.source_refs
            })).collect::<Vec<_>>(),
            "frontendTarget": brainstorm.frontend_experience.as_ref().map(|frontend_experience| json!({
                "required": frontend_experience.required,
                "kind": frontend_experience.kind,
                "experienceLevel": frontend_experience.experience_level,
                "audienceNames": frontend_experience.audiences.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                "surfaceNames": frontend_experience.surfaces.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                "operationNames": frontend_experience.operation_paths.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                "operationGoals": frontend_experience.operation_paths.iter().map(|item| item.user_goal.clone()).collect::<Vec<_>>()
            })),
            "userFacingLanguage": brainstorm.delivery_context.user_facing_language,
            "roadmapSignal": {
                "required": brainstorm.roadmap.required,
                "currentPhaseId": brainstorm.roadmap.current_phase_id,
                "phaseIds": brainstorm.roadmap.phases.iter().map(|phase| phase.phase_id.clone()).collect::<Vec<_>>(),
                "phaseTitles": brainstorm.roadmap.phases.iter().map(|phase| phase.title.clone().or_else(|| phase.name.clone()).unwrap_or_else(|| phase.phase_id.clone())).collect::<Vec<_>>(),
                "phaseGoals": brainstorm.roadmap.phases.iter().map(|phase| phase.goal.clone().unwrap_or_default()).collect::<Vec<_>>(),
                "nextPhasePreview": next_phase_preview_summary(&brainstorm.phase_plan.next_phase_preview)
            },
            "sourceRefs": brainstorm.sources.iter().map(|source| source.source_id.clone()).collect::<Vec<_>>()
        },
        "currentPhaseLens": {
            "phaseId": brainstorm.phase_plan.current.phase_id,
            "title": brainstorm.phase_plan.current.title,
            "goal": brainstorm.phase_plan.current.goal,
            "includedScopeRefs": brainstorm.phase_plan.current.scope_refs,
            "acceptanceRefs": brainstorm.phase_plan.current.acceptance_refs,
        },
        "securityRequirement": brainstorm.security_requirement,
        "securityProfileGuidance": security_profile_guidance(
            &brainstorm.security_requirement.applies,
            baseline_exists,
        ),
        "decisionNeeds": technical_baseline_decision_needs(project_kind, baseline_exists),
        "previousBaselineContext": previous_baseline_context,
        "constraints": {
            "mustUse": [],
            "mustAvoid": [],
            "userPreferences": [],
            "deploymentPreference": "local_first"
        },
        "repoEvidence": repo_evidence,
        "selectionGuidance": selection_guidance,
        "enumRefs": {
            "projectKind": ["new_project", "existing_project", "unknown"],
            "status": ["draft", "needs_user_confirmation", "auto_accepted", "confirmed", "blocked", "superseded"],
            "source": ["user_specified", "user_confirmed", "detected_from_repo", "agent_inferred_from_repo_signals", "agent_recommended_for_new_project"],
            "scope": ["project", "roadmap", "phase_override"],
            "approvalType": ["user_confirmed", "policy_auto_accept", "manual_override", "none"],
            "confidence": ["low", "medium", "high", "unknown"],
            "securityMechanism": ["none", "bearer_jwt"],
            "securityAlgorithm": ["RS256", "ES256", "EdDSA", "HS256"],
            "securityKeySource": ["existing_idp", "environment_secret", "file_mounted_key", "kms", "user_specified", "not_applicable"],
            "securityTransport": ["bearer_header", "same_origin_cookie", "mutual_tls", "not_applicable"]
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
                    "securityProfiles",
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
                    "purpose": "Read the confirmed Brainstorm scope, acceptance ids, frontend target, current phase lens, and baseline decision needs before drafting the baseline.",
                    "whenToRead": "Read before producing any TechnicalBaseline recommendation.",
                    "selectors": read_selectors_value_from_paths(baseline_context_fields)
                },
                {
                    "groupId": "technical_baseline_repo_evidence",
                    "required": false,
                    "purpose": "Read repository evidence before inferring an existing-project baseline or deciding whether reuse applies.",
                    "whenToRead": "Read for existing_project or when repository continuity matters.",
                    "selectors": read_selectors_value_from_paths(repo_evidence_fields)
                },
                {
                    "groupId": "technical_baseline_selection_guidance",
                    "required": selection_guidance.is_some()
                        || !matches!(
                            brainstorm.security_requirement.applies,
                            SecurityRequirementApplicability::NotApplicable
                        ),
                    "purpose": "Read the new-project confirmation discipline before asking the user to confirm the baseline.",
                    "whenToRead": "Read only when the projectKind is new_project or the baseline still needs explicit user confirmation.",
                    "selectors": read_selectors_value_from_paths(security_selection_fields)
                },
                {
                    "groupId": "technical_baseline_write_contract",
                    "required": true,
                    "purpose": "Read the candidate schema and write target before writing the TechnicalBaseline candidate.",
                    "whenToRead": "Read only when ready to write the candidate file.",
                    "selectors": read_selectors_value_from_paths([
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
                        "outputContract.schemaProjection",
                        "enumRefs.projectKind",
                        "enumRefs.status",
                        "enumRefs.source",
                        "enumRefs.scope",
                        "enumRefs.approvalType",
                        "enumRefs.confidence",
                        "enumRefs.securityMechanism",
                        "enumRefs.securityAlgorithm",
                        "enumRefs.securityKeySource",
                        "enumRefs.securityTransport"
                    ])
                }
            ]
        }
    })
}

fn security_profile_guidance(
    applicability: &SecurityRequirementApplicability,
    has_previous_baseline: bool,
) -> Value {
    let protected_scope = !matches!(
        applicability,
        SecurityRequirementApplicability::NotApplicable
    );
    let mut guidance = json!({
        "applies": protected_scope,
        "authority": "securityRequirement from the accepted Brainstorm contract",
        "profileRequired": protected_scope,
        "agentFields": []
    });
    if protected_scope {
        guidance["existingBaselineRule"] = json!(if has_previous_baseline {
            "Reuse an existing accepted security profile when it satisfies the current requirement; do not change its algorithm without explicit user confirmation."
        } else {
            "Select one concrete profile during TechnicalBaseline confirmation when the requirement is protected."
        });
        guidance["newProjectDefault"] = json!({
            "mechanism": SecurityMechanism::BearerJwt,
            "algorithm": SecurityAlgorithm::Rs256,
            "keySource": SecurityKeySource::EnvironmentSecret,
            "transport": SecurityTransport::BearerHeader,
            "rule": "RS256 is a recommendation for a new project only; the agent must present it for user confirmation and must not silently add alternatives."
        });
        guidance["algorithmPolicy"] = json!([
            "Write only the algorithm selected by the accepted TechnicalBaseline profile.",
            "Do not derive an algorithm from an incoming token header.",
            "Do not treat HS256 as a default; use it only when the user or existing identity contract explicitly selects it.",
            "Keep the algorithm closed to the profile selection; do not add a second algorithm for flexibility."
        ]);
        guidance["agentFields"] = json!([
            "securityProfiles[].profileId",
            "securityProfiles[].name",
            "securityProfiles[].mechanism",
            "securityProfiles[].algorithm",
            "securityProfiles[].keySource",
            "securityProfiles[].transport",
            "securityProfiles[].issuer",
            "securityProfiles[].audiences",
            "securityProfiles[].claims",
            "securityProfiles[].sourceRefs",
            "securityProfiles[].rationale"
        ]);
    } else {
        guidance["profilePolicy"] = json!(
            "No authentication profile applies to the accepted scope; securityProfiles must remain an empty array."
        );
    }
    guidance
}

fn scope_item_ids(items: &[contracts::ScopeItem]) -> Vec<String> {
    items.iter().map(|item| item.id.clone()).collect()
}

fn scope_item_labels(items: &[contracts::ScopeItem]) -> Vec<String> {
    items.iter().map(|item| item.label.clone()).collect()
}

fn next_phase_preview_summary(preview: &contracts::NextPhasePreview) -> Value {
    match preview {
        contracts::NextPhasePreview::Candidate {
            suggested_phase_id,
            title,
            goal,
            scope_preview,
            reason,
        } => json!({
            "kind": "candidate",
            "suggestedPhaseId": suggested_phase_id,
            "title": title,
            "goal": goal,
            "scopePreview": scope_preview,
            "reason": reason
        }),
        contracts::NextPhasePreview::None { reason } => json!({
            "kind": "none",
            "suggestedPhaseId": Value::Null,
            "title": Value::Null,
            "goal": Value::Null,
            "scopePreview": [],
            "reason": reason
        }),
    }
}

fn technical_baseline_context_fields(
    brainstorm: &BrainstormContract,
    has_previous_baseline: bool,
) -> Vec<&'static str> {
    let mut fields = vec![
        "brainstormLens.summary.title",
        "brainstormLens.summary.oneLine",
        "brainstormLens.summary.complexity",
        "brainstormLens.scopeIndex.includedIds",
        "brainstormLens.scopeIndex.includedLabels",
        "brainstormLens.scopeIndex.deferredIds",
        "brainstormLens.scopeIndex.deferredLabels",
        "brainstormLens.scopeIndex.excludedIds",
        "brainstormLens.scopeIndex.excludedLabels",
        "brainstormLens.scopeIndex.assumptionTexts",
        "brainstormLens.acceptanceIndex",
        "brainstormLens.userFacingLanguage",
        "brainstormLens.roadmapSignal.required",
        "brainstormLens.roadmapSignal.currentPhaseId",
        "brainstormLens.roadmapSignal.phaseIds",
        "brainstormLens.roadmapSignal.phaseTitles",
        "brainstormLens.roadmapSignal.phaseGoals",
        "brainstormLens.roadmapSignal.nextPhasePreview.kind",
        "brainstormLens.roadmapSignal.nextPhasePreview.suggestedPhaseId",
        "brainstormLens.roadmapSignal.nextPhasePreview.title",
        "brainstormLens.roadmapSignal.nextPhasePreview.goal",
        "brainstormLens.roadmapSignal.nextPhasePreview.scopePreview",
        "brainstormLens.roadmapSignal.nextPhasePreview.reason",
        "currentPhaseLens.phaseId",
        "currentPhaseLens.title",
        "currentPhaseLens.goal",
        "currentPhaseLens.includedScopeRefs",
        "currentPhaseLens.acceptanceRefs",
        "decisionNeeds",
        "securityRequirement",
        "securityProfileGuidance",
        "constraints.mustUse",
        "constraints.mustAvoid",
        "constraints.userPreferences",
        "constraints.deploymentPreference",
    ];
    if brainstorm.summary.business_goal.is_some() {
        fields.insert(2, "brainstormLens.summary.businessGoal");
    }
    if brainstorm.domain_model.is_some() {
        fields.extend([
            "brainstormLens.domainModel.capabilityNames",
            "brainstormLens.domainModel.businessFlowNames",
        ]);
    }
    if brainstorm.frontend_experience.is_some() {
        fields.extend([
            "brainstormLens.frontendTarget.required",
            "brainstormLens.frontendTarget.kind",
            "brainstormLens.frontendTarget.experienceLevel",
            "brainstormLens.frontendTarget.audienceNames",
            "brainstormLens.frontendTarget.surfaceNames",
            "brainstormLens.frontendTarget.operationNames",
            "brainstormLens.frontendTarget.operationGoals",
        ]);
    }
    if has_previous_baseline {
        fields.extend([
            "previousBaselineContext.previousBaselineRef",
            "previousBaselineContext.technicalBaselineId",
            "previousBaselineContext.status",
            "previousBaselineContext.projectKind",
            "previousBaselineContext.scope",
            "previousBaselineContext.stack",
            "previousBaselineContext.constraints",
            "previousBaselineContext.confidence",
            "previousBaselineContext.securityProfiles",
        ]);
    }
    fields
}

fn technical_baseline_repo_evidence(
    project_root: &Path,
    project_kind: ProjectKind,
    baseline_exists: bool,
) -> Value {
    let mut evidence = json!({
        "detectedProjectKind": project_kind,
        "baselineExists": baseline_exists,
        "repositoryContextExists": false
    });
    if matches!(project_kind, ProjectKind::ExistingProject) {
        evidence["signals"] = compact_repo_signals(project_root);
    }
    evidence
}

fn technical_baseline_repo_evidence_fields(project_kind: ProjectKind) -> Vec<&'static str> {
    let mut fields = vec![
        "projectKind",
        "repoEvidence.detectedProjectKind",
        "repoEvidence.baselineExists",
        "repoEvidence.repositoryContextExists",
    ];
    if matches!(project_kind, ProjectKind::ExistingProject) {
        fields.extend([
            "repoEvidence.signals.manifests",
            "repoEvidence.signals.packageManagers",
            "repoEvidence.signals.languages",
            "repoEvidence.signals.frameworks",
            "repoEvidence.signals.sourceRoots",
        ]);
    }
    fields
}

fn compact_repo_signals(project_root: &Path) -> Value {
    let mut signals = RepoSignalSummary::default();
    collect_node_signals(project_root, &mut signals);
    collect_java_signals(project_root, &mut signals);
    collect_python_signals(project_root, &mut signals);
    collect_rust_signals(project_root, &mut signals);
    collect_go_signals(project_root, &mut signals);
    for path in [
        "src", "app", "web", "service", "backend", "frontend", "tests",
    ] {
        if project_root.join(path).exists() {
            signals.source_roots.insert(path.to_string());
        }
    }
    signals.to_json()
}

#[derive(Default)]
struct RepoSignalSummary {
    manifests: BTreeSet<String>,
    package_managers: BTreeSet<String>,
    languages: BTreeSet<String>,
    frameworks: BTreeSet<String>,
    source_roots: BTreeSet<String>,
}

impl RepoSignalSummary {
    fn to_json(&self) -> Value {
        json!({
            "manifests": sorted_values(&self.manifests),
            "packageManagers": sorted_values(&self.package_managers),
            "languages": sorted_values(&self.languages),
            "frameworks": sorted_values(&self.frameworks),
            "sourceRoots": sorted_values(&self.source_roots)
        })
    }
}

fn sorted_values(values: &BTreeSet<String>) -> Vec<String> {
    values.iter().cloned().collect()
}

fn collect_node_signals(project_root: &Path, signals: &mut RepoSignalSummary) {
    let package_file = project_root.join("package.json");
    if !package_file.exists() {
        return;
    }
    signals.manifests.insert("package.json".to_string());
    if project_root.join("package-lock.json").exists() {
        signals.package_managers.insert("npm".to_string());
    }
    if project_root.join("pnpm-lock.yaml").exists() {
        signals.package_managers.insert("pnpm".to_string());
    }
    if project_root.join("yarn.lock").exists() {
        signals.package_managers.insert("yarn".to_string());
    }
    if signals.package_managers.is_empty() {
        signals.package_managers.insert("npm".to_string());
    }
    signals.languages.insert("JavaScript".to_string());
    if project_root.join("tsconfig.json").exists() {
        signals.languages.insert("TypeScript".to_string());
        signals.manifests.insert("tsconfig.json".to_string());
    }
    let Ok(package) = state::store::read_json_value(&package_file) else {
        return;
    };
    let dependencies = package_dependencies(&package);
    if dependencies.contains("typescript") {
        signals.languages.insert("TypeScript".to_string());
    }
    for (dependency, framework) in [
        ("next", "Next.js"),
        ("react", "React"),
        ("vue", "Vue"),
        ("svelte", "Svelte"),
        ("vite", "Vite"),
        ("express", "Express"),
        ("fastify", "Fastify"),
        ("@nestjs/core", "NestJS"),
    ] {
        if dependencies.contains(dependency) {
            signals.frameworks.insert(framework.to_string());
        }
    }
}

fn package_dependencies(package: &Value) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();
    for key in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(values) = package.get(key).and_then(Value::as_object) {
            dependencies.extend(values.keys().cloned());
        }
    }
    dependencies
}

fn collect_java_signals(project_root: &Path, signals: &mut RepoSignalSummary) {
    let pom_file = project_root.join("pom.xml");
    if pom_file.exists() {
        signals.manifests.insert("pom.xml".to_string());
        signals.languages.insert("Java".to_string());
        signals.package_managers.insert("Maven".to_string());
        if fs::read_to_string(&pom_file)
            .map(|content| content.contains("spring-boot"))
            .unwrap_or(false)
        {
            signals.frameworks.insert("Spring Boot".to_string());
        }
    }
    if project_root.join("build.gradle").exists() || project_root.join("build.gradle.kts").exists()
    {
        signals.manifests.insert("build.gradle".to_string());
        signals.languages.insert("Java".to_string());
        signals.package_managers.insert("Gradle".to_string());
    }
}

fn collect_python_signals(project_root: &Path, signals: &mut RepoSignalSummary) {
    let has_pyproject = project_root.join("pyproject.toml").exists();
    let has_requirements = project_root.join("requirements.txt").exists();
    if !has_pyproject && !has_requirements {
        return;
    }
    signals.languages.insert("Python".to_string());
    signals.package_managers.insert("pip".to_string());
    if has_pyproject {
        signals.manifests.insert("pyproject.toml".to_string());
    }
    if has_requirements {
        signals.manifests.insert("requirements.txt".to_string());
    }
    let combined = ["pyproject.toml", "requirements.txt"]
        .iter()
        .filter_map(|path| fs::read_to_string(project_root.join(path)).ok())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    for (needle, framework) in [
        ("fastapi", "FastAPI"),
        ("django", "Django"),
        ("flask", "Flask"),
    ] {
        if combined.contains(needle) {
            signals.frameworks.insert(framework.to_string());
        }
    }
}

fn collect_rust_signals(project_root: &Path, signals: &mut RepoSignalSummary) {
    if !project_root.join("Cargo.toml").exists() {
        return;
    }
    signals.manifests.insert("Cargo.toml".to_string());
    signals.languages.insert("Rust".to_string());
    signals.package_managers.insert("Cargo".to_string());
}

fn collect_go_signals(project_root: &Path, signals: &mut RepoSignalSummary) {
    if !project_root.join("go.mod").exists() {
        return;
    }
    signals.manifests.insert("go.mod".to_string());
    signals.languages.insert("Go".to_string());
    signals.package_managers.insert("go".to_string());
}

fn technical_baseline_selection_guidance(
    project_kind: ProjectKind,
    has_previous_baseline: bool,
) -> Option<Value> {
    if !matches!(project_kind, ProjectKind::NewProject) && !has_previous_baseline {
        return None;
    }
    Some(json!({
        "schemaVersion": "1.0",
        "purpose": if matches!(project_kind, ProjectKind::NewProject) {
            "Guide the agent-user technical baseline confirmation for a new project in an empty or uninitialized workspace before PGC."
        } else {
            "Guide the agent-user technical baseline confirmation when a previous baseline exists and the final candidate may add, replace, or conflict with stable baseline elements."
        },
        "runtimeBoundary": {
            "role": "The request provides materials, common examples, output contract, and confirmation rules only.",
            "doesNotDo": [
                "The request does not infer the concrete recommended stack for this requirement.",
                "The request does not parse the user's natural-language technology replies.",
                "The request does not participate in intermediate confirmation rounds."
            ],
            "requiredAgentLoop": [
                "Read the request refs and understand the confirmed requirement scope.",
                "Generate the concrete recommendation or baseline-change summary yourself.",
                "Talk with the user for as many rounds as needed.",
                "Write and submit the candidate only after the user explicitly confirms the final technology baseline."
            ]
        },
        "confirmationRules": confirmation_rules(has_previous_baseline),
        "trackModel": {
            "requiredFinalShape": "Use stack.tracks with web, app, backend, persistence, dataAccess, and externalServices keys. When web is selected, also include qualityAutomation. Each track should include status, selection, source, and rationale. A selected externalServices track must also include structured providers with confirmed capability roles.",
            "trackStatusValues": ["selected", "not_needed", "not_applicable", "user_custom"],
            "sourceValues": ["agent_recommended_user_confirmed", "user_adjusted", "user_specified", "previous_baseline", "not_applicable"],
            "coreTracks": ["web", "app", "backend", "persistence", "dataAccess", "externalServices"],
            "conditionalTracks": {
                "qualityAutomation": "Required when web.status is selected or user_custom; choose the browser automation stack in the same confirmed baseline."
            },
            "customTechnologyPolicy": "Common options are examples, not a whitelist. User-specified technologies outside these examples are allowed, but mark the relevant track source as user_specified or user_custom and include it in the final confirmation summary and reasoningSummary."
        },
        "recommendationBasis": {
            "authority": "Use the complete BrainstormContract as the product-scope authority for the first new-project TechnicalBaseline recommendation.",
            "mustRead": [
                "brainstormLens.summary",
                "brainstormLens.scopeIndex included/deferred/excluded labels and assumptions",
                "brainstormLens.domainModel capabilityNames and businessFlowNames when present",
                "brainstormLens.frontendTarget when present",
                "brainstormLens.roadmapSignal phase titles, phase goals, and next-phase preview"
            ],
            "currentPhaseLensRole": "currentPhaseLens identifies the first implementation slice only. Do not choose the initial technology baseline from the current phase scope alone when the full requirement or roadmap implies later product surfaces, persistence scale, app clients, services, integrations, or operational needs.",
            "recommendationRule": "Recommend a stable baseline for the full confirmed delivery/roadmap horizon; explain when the current phase can start small inside that baseline without hiding later known needs."
        },
        "userFacingConfirmationProtocol": {
            "mandatorySections": [
                "Recommendation basis: summarize the full requirement/roadmap signals used, not only the current phase.",
                "Recommended final baseline: list every core track with selection and short rationale, plus qualityAutomation when web is selected.",
                "Adjustable technology range: show common examples for every core track so the user knows how to modify the recommendation.",
                "Reply format: show canonical key=value examples using web, app, backend, persistence, dataAccess, externalServices, and qualityAutomation for web projects.",
                "When securityRequirement is protected, show the proposed mechanism, one selected algorithm, key source, transport, issuer/audience ownership, and the confirmation or adjustment needed before submitting the baseline.",
                "Final confirmation rule: if the user changes anything, summarize the final baseline and ask for explicit confirmation before submitting."
            ],
            "wordingRules": [
                "Do not present the recommendation as based only on the first phase or current small implementation slice.",
                "Do not omit the adjustable technology range.",
                "Do not present backend options as bare language-only labels when a mainstream framework choice is expected; show language + framework combinations in user-facing examples.",
                "Do not use db or orm as the primary reply keys; use persistence and dataAccess in the primary examples.",
                "When externalServices includes Redis, explain the business role in plain language, allow multiple roles, and ask only the persistence or startup questions needed by the selected roles.",
                "Do not ask users to choose Redis data structures, TTL values, Lua scripts, or Compose settings during TechnicalBaseline confirmation; those decisions belong to Architecture.",
                "Do not mention Loom internals, gates, submit permission, workflow blocking, or phrases like Loom allows, Loom requires, Loom is stuck, or Loom will not continue in user-facing text.",
                "It is fine to understand db as persistence and orm as dataAccess when the user writes those aliases, but normalize the final candidate to stack.tracks.persistence and stack.tracks.dataAccess."
            ]
        },
        "commonOptions": {
            "web": {
                "label": "Web client",
                "examples": ["Next.js", "React + Vite", "Vue + Vite", "SvelteKit", "Astro", "No Web client"]
            },
            "app": {
                "label": "App client",
                "examples": ["No App client", "React Native + Expo", "Flutter", "iOS Native (Swift / SwiftUI)", "Android Native (Kotlin / Jetpack Compose)", "Hybrid WebView (Capacitor / Ionic)", "PWA"]
            },
            "backend": {
                "label": "Backend / service",
                "examples": ["Next.js + Server Actions / Route Handlers / SSR", "Node.js + Fastify", "Node.js + Express", "Node.js + NestJS", "Python + FastAPI", "Python + Django", "Java + Spring Boot", "Go + net/http or Gin", ".NET + ASP.NET Core", "No independent backend"]
            },
            "persistence": {
                "label": "Database / persistence",
                "examples": ["SQLite", "PostgreSQL", "MySQL", "MongoDB", "File storage / local JSON", "No persistence yet"]
            },
            "dataAccess": {
                "label": "ORM / data access",
                "examples": ["Prisma", "Drizzle", "TypeORM", "SQLAlchemy", "Django ORM", "Spring Data JPA", "MyBatis Plus", "Entity Framework", "Raw SQL / lightweight wrapper", "No ORM"]
            },
            "externalServices": {
                "label": "External services",
                "examples": ["None", "Redis for cache, sessions, or background jobs", "User specified", "Only recommend services explicitly required by the confirmed requirement"],
                "capabilityRoles": {
                    "redis": [
                        "Login state and shared sessions",
                        "Background jobs and message processing",
                        "Query result acceleration",
                        "Atomic locks or rate limiting"
                    ]
                },
                "confirmationRule": "Show only capability roles supported by the confirmed requirement. Allow multiple roles. Do not choose a Redis role from a keyword alone; ask the user when the role is ambiguous."
            },
            "qualityAutomation": {
                "label": "Browser quality automation",
                "examples": ["Playwright", "User-specified existing browser test stack"]
            },
            "securityProfile": {
                "label": "Authentication profile when protected",
                "examples": ["Bearer JWT with the explicitly selected algorithm and identity-provider key source", "Existing session or gateway identity contract"],
                "rule": "Use the accepted requirement and repository identity evidence; do not ask the user to choose token claims or JWT data structures here."
            }
        },
        "shorthandNormalization": {
            "backend": [
                "If the user writes backend=Java without a framework, normalize it to Java + Spring Boot unless they explicitly name a different Java backend stack.",
                "If the user writes backend=Python without a framework, normalize it to Python + FastAPI for service/backend work unless the requirement or user explicitly points to Django-style site/admin/content capabilities.",
                "If the user writes backend=Node.js without a framework, ask for or summarize a concrete Node.js framework choice such as Fastify, Express, or NestJS before final confirmation.",
                "If the user writes backend=.NET without a framework, normalize it to .NET + ASP.NET Core unless they explicitly name another .NET backend stack."
            ],
            "dataAccessCompatibility": [
                "When backend is Java + Spring Boot and dataAccess is not specified, recommend Spring Data JPA or MyBatis Plus explicitly before final confirmation; do not leave it as generic Java persistence.",
                "When backend is Python + FastAPI and dataAccess is not specified, recommend SQLAlchemy or SQLModel explicitly before final confirmation.",
                "When backend is Python + Django and dataAccess is not specified, recommend Django ORM explicitly before final confirmation."
            ]
        },
        "recommendationPrinciples": [
            "Prefer mainstream, maintainable, community-mature technologies.",
            "Prefer technologies that match the confirmed product shape and implementation effort.",
            "For Web UI, TypeScript is preferred unless the user chooses otherwise.",
            "For small or medium local-first CRUD/admin systems, SQLite is a reasonable default unless the user needs a production multi-user database.",
            "Prefer integrated fullstack options when they reduce orchestration cost and still satisfy the product need.",
            "Respect explicit user technology choices even when they are outside common examples.",
            "Avoid niche stacks unless the user asks for them or the requirement clearly needs them."
        ],
        "replyProtocolForUser": {
            "acceptRecommendation": "确认推荐方案",
            "partialAdjustmentExample": "web=Vue+Vite, backend=Java+Spring Boot, persistence=PostgreSQL, dataAccess=Spring Data JPA, qualityAutomation=Playwright, app=不需要, externalServices=不需要",
            "fullCustomExample": "web=React+Vite, app=React Native+Expo, backend=Fastify, persistence=SQLite, dataAccess=Prisma, qualityAutomation=Playwright, externalServices=不需要",
            "redisCapabilityExample": "externalServices=Redis，用于登录会话和后台任务；登录会话重启后保留，后台任务失败可重试",
            "finalConfirmationPrompt": "When the user did not directly accept the recommendation, present a final technology baseline summary and ask them to reply 确认技术栈 or 修改: ..."
        }
    }))
}

fn confirmation_rules(has_previous_baseline: bool) -> Vec<&'static str> {
    let mut rules = vec![
        "User requirement confirmation is not technology baseline confirmation.",
        "If the user accepts the recommendation directly, that reply can be the final technology baseline confirmation.",
        "If the user adjusts part of the stack or specifies a custom stack, summarize the final baseline and ask for final confirmation before writing the candidate.",
        "Do not submit a confirmed candidate while any core track is ambiguous. Mark a track as not_applicable/not_needed only when the requirement or user confirmation supports that.",
        "Build commands, local run commands, and deployment preparation are derived later. For a selected Web client, include qualityAutomation in the same baseline confirmation; do not reopen confirmation only to update test commands or runtime details.",
    ];
    if has_previous_baseline {
        rules.extend([
            "When a previous baseline exists, unchanged baseline reuse is the default for normal bugfix, repair, optimization, or feature work inside the existing stack.",
            "Only a current confirmed scope that explicitly adds a new technology surface or replaces a previous baseline element needs explicit technology baseline confirmation.",
            "Current repository scripts, test commands, build commands, start commands, generated files, or framework implementation nuances are implementation facts; do not treat them as user-facing technology baseline changes by themselves.",
            "Preserve previous baseline tracks that the user did not confirm changing.",
        ]);
    }
    rules
}

pub fn accept_technical_baseline_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher + Clone,
{
    match accept_technical_baseline_file_inner(input, authorized, dispatcher) {
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

fn accept_technical_baseline_file_inner<D>(
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
    let request_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec!["securityRequirement".to_string()],
    })?;
    let security_requirement = request_fields
        .fields
        .get("securityRequirement")
        .and_then(|field| serde_json::from_value::<SecurityRequirement>(field.value.clone()).ok())
        .unwrap_or_default();
    let candidate_file = from_project_relative(project_root, &target.path)?;
    let raw = state::store::read_json_value(&candidate_file)?;
    let mut candidate: TechnicalBaselineCandidateAgentWritable =
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

    let now = state::store::now_string();
    normalize_user_confirmed_approval(&mut candidate, &now);
    let issues = validate_candidate(&candidate, &security_requirement);
    if !issues.is_empty() {
        return Ok(repairable(input, authorized, target.path.clone(), issues));
    }
    if matches!(candidate.project_kind, ProjectKind::Unknown) {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "Ask the user whether this phase continues an existing project or starts a new project, then rewrite the same candidate with the confirmed projectKind.".to_string(),
            "project_kind_confirmation".to_string(),
        ));
    }
    if matches!(candidate.project_kind, ProjectKind::NewProject)
        && candidate.approval.r#type != TechnicalBaselineApprovalType::UserConfirmed
    {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "The technology baseline for a new project must be explicitly confirmed by the user before planning continues. Present the recommended stack, capture corrections, then rewrite the same candidate with approval.type=user_confirmed.".to_string(),
            "new_project_baseline_confirmation".to_string(),
        ));
    }
    let previous_baseline_file = technical_baseline_file(project_root, &delivery_id);
    let protected_requirement = !matches!(
        security_requirement.applies,
        SecurityRequirementApplicability::NotApplicable
    );
    if protected_requirement
        && !previous_baseline_file.exists()
        && candidate.approval.r#type != TechnicalBaselineApprovalType::UserConfirmed
    {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "The protected security profile is a technology decision and must be explicitly confirmed before the first baseline is accepted. Present the selected mechanism, one algorithm, key source, and transport, then rewrite the same candidate with approval.type=user_confirmed.".to_string(),
            "security_profile_confirmation".to_string(),
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
            "The technology baseline still requires explicit user confirmation. Present the baseline change or recommendation, then rewrite the same candidate with the confirmed baseline.".to_string(),
            "technical_baseline_confirmation".to_string(),
        ));
    }
    if previous_baseline_file.exists() {
        let previous: TechnicalBaselineContract = state::store::read_json(&previous_baseline_file)?;
        let stack_or_baseline_changed = technical_baseline_conflicts(&previous, &candidate);
        let repo_signal_conflicts =
            repo_signals_conflict_with_previous(project_root, &input.request_ref, &previous)?;
        if (stack_or_baseline_changed || !repo_signal_conflicts.is_empty())
            && !matches!(
                candidate.approval.r#type,
                TechnicalBaselineApprovalType::UserConfirmed
                    | TechnicalBaselineApprovalType::ManualOverride
            )
        {
            return Ok(technical_baseline_user_gate(
                input,
                authorized,
                "The proposed technology baseline changes an existing baseline. Present the previous baseline and proposed change to the user, then rewrite the same candidate with approval.type=user_confirmed after explicit confirmation.".to_string(),
                "previous_baseline_change_confirmation".to_string(),
            ));
        }
    }

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
        security_profiles: candidate.security_profiles,
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
    state::lifecycle_store::finalize_agent_candidate(project_root, &target.path)?;
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

fn normalize_user_confirmed_approval(
    candidate: &mut TechnicalBaselineCandidateAgentWritable,
    confirmed_at: &str,
) {
    if candidate.approval.r#type != TechnicalBaselineApprovalType::UserConfirmed {
        return;
    }
    if candidate
        .approval
        .confirmed_at
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        candidate.approval.confirmed_at = Some(confirmed_at.to_string());
    }
}

fn validate_candidate(
    candidate: &TechnicalBaselineCandidateAgentWritable,
    security_requirement: &SecurityRequirement,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if !candidate.stack.is_object() {
        issues.push(issue(
            "TECHNICAL_BASELINE_STACK_INVALID",
            "stack",
            "stack must be a JSON object that describes the selected technology baseline.",
        ));
    }
    validate_external_services_track(&candidate.stack, &mut issues);
    validate_security_profiles(candidate, security_requirement, &mut issues);
    if candidate.approval.r#type == TechnicalBaselineApprovalType::None
        && matches!(candidate.status, TechnicalBaselineStatus::Confirmed)
    {
        issues.push(issue(
            "TECHNICAL_BASELINE_APPROVAL_INVALID",
            "approval.type",
            "A confirmed TechnicalBaseline cannot keep approval.type=none.",
        ));
    }
    if matches!(candidate.project_kind, ProjectKind::NewProject) {
        validate_new_project_candidate(candidate, &mut issues);
    }
    issues
}

fn validate_security_profiles(
    candidate: &TechnicalBaselineCandidateAgentWritable,
    requirement: &SecurityRequirement,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let protected = !matches!(
        requirement.applies,
        SecurityRequirementApplicability::NotApplicable
    );
    if !protected && !candidate.security_profiles.is_empty() {
        issues.push(issue(
            "TECHNICAL_BASELINE_SECURITY_PROFILE_UNEXPECTED",
            "securityProfiles",
            "securityProfiles must be empty when the accepted security requirement is not_applicable.",
        ));
        return;
    }
    if protected && candidate.security_profiles.is_empty() {
        issues.push(issue(
            "TECHNICAL_BASELINE_SECURITY_PROFILE_REQUIRED",
            "securityProfiles",
            "A protected requirement must select at least one canonical security profile before the TechnicalBaseline can be accepted.",
        ));
    }
    let mut profile_ids = BTreeSet::new();
    for (index, profile) in candidate.security_profiles.iter().enumerate() {
        let path = format!("securityProfiles[{index}]");
        if profile.profile_id.trim().is_empty() || !profile_ids.insert(profile.profile_id.clone()) {
            issues.push(issue(
                "TECHNICAL_BASELINE_SECURITY_PROFILE_ID_INVALID",
                &format!("{path}.profileId"),
                "Every security profile needs a unique non-empty profileId.",
            ));
        }
        if profile.name.trim().is_empty() || profile.rationale.trim().is_empty() {
            issues.push(issue(
                "TECHNICAL_BASELINE_SECURITY_PROFILE_DESCRIPTION_REQUIRED",
                &path,
                "Every security profile needs a non-empty name and rationale.",
            ));
        }
        match profile.mechanism {
            SecurityMechanism::None => {
                if profile.algorithm.is_some()
                    || !matches!(profile.key_source, SecurityKeySource::NotApplicable)
                    || !matches!(profile.transport, SecurityTransport::NotApplicable)
                {
                    issues.push(issue(
                        "TECHNICAL_BASELINE_SECURITY_PROFILE_NONE_INVALID",
                        &path,
                        "A none security profile must not declare an algorithm, key material, or a transport.",
                    ));
                }
            }
            SecurityMechanism::BearerJwt => {
                if profile.algorithm.is_none() {
                    issues.push(issue(
                        "TECHNICAL_BASELINE_SECURITY_ALGORITHM_REQUIRED",
                        &format!("{path}.algorithm"),
                        "A bearer JWT profile must explicitly select one signing algorithm.",
                    ));
                }
                if matches!(profile.key_source, SecurityKeySource::NotApplicable)
                    || matches!(profile.transport, SecurityTransport::NotApplicable)
                {
                    issues.push(issue(
                        "TECHNICAL_BASELINE_SECURITY_PROFILE_BOUNDARY_REQUIRED",
                        &path,
                        "A bearer JWT profile must declare a key source and transport.",
                    ));
                }
                if profile
                    .issuer
                    .as_deref()
                    .is_none_or(|issuer| issuer.trim().is_empty())
                    || profile.audiences.is_empty()
                    || !profile.claims.iter().any(|claim| claim.trim() == "sub")
                {
                    issues.push(issue(
                        "TECHNICAL_BASELINE_JWT_CLAIMS_INCOMPLETE",
                        &path,
                        "A bearer JWT profile must declare issuer, at least one audience, and the subject claim before it can be used by an API contract.",
                    ));
                }
            }
        }
    }
}

fn validate_external_services_track(stack: &Value, issues: &mut Vec<delivery_core::RepairIssue>) {
    let Some(track) = stack
        .pointer("/tracks/externalServices")
        .and_then(Value::as_object)
    else {
        return;
    };
    let status = track
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(status, "selected" | "user_custom")
        && !external_services_track_complete(track, status)
    {
        issues.push(issue(
            "TECHNICAL_BASELINE_EXTERNAL_SERVICES_UNSTRUCTURED",
            "stack.tracks.externalServices.providers",
            "A selected externalServices track must include at least one provider with one or more structured capability roles, each declaring purpose, durability, and startupRequirement.",
        ));
    }
}

const NEW_PROJECT_CORE_TRACKS: [&str; 6] = [
    "web",
    "app",
    "backend",
    "persistence",
    "dataAccess",
    "externalServices",
];
const NEW_PROJECT_TRACK_STATUSES: [&str; 4] =
    ["selected", "not_needed", "not_applicable", "user_custom"];

fn validate_new_project_candidate(
    candidate: &TechnicalBaselineCandidateAgentWritable,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if matches!(
        candidate.approval.r#type,
        TechnicalBaselineApprovalType::UserConfirmed
    ) && candidate
        .approval
        .confirmed_at
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(issue(
            "NEW_PROJECT_BASELINE_CONFIRMATION_REQUIRED",
            "approval.confirmedAt",
            "New-project TechnicalBaseline with approval.type=user_confirmed must include the actual user confirmation timestamp.",
        ));
    }
    if !matches!(
        candidate.status,
        TechnicalBaselineStatus::Confirmed | TechnicalBaselineStatus::NeedsUserConfirmation
    ) {
        issues.push(issue(
            "NEW_PROJECT_BASELINE_CONFIRMATION_REQUIRED",
            "status",
            "New-project TechnicalBaseline must be confirmed before planning, or use status=needs_user_confirmation when user confirmation is still pending.",
        ));
    }
    if !new_project_stack_tracks_complete(&candidate.stack) {
        issues.push(issue(
            "NEW_PROJECT_BASELINE_TRACKS_INCOMPLETE",
            "stack.tracks",
            "New-project TechnicalBaseline stack.tracks must include web, app, backend, persistence, dataAccess, and externalServices; each track needs a valid status and non-empty selection. A selected externalServices track must provide structured providers and capability roles.",
        ));
    }
    if new_project_web_selected(&candidate.stack)
        && !new_project_quality_automation_complete(&candidate.stack)
    {
        issues.push(issue(
            "NEW_PROJECT_QUALITY_AUTOMATION_INCOMPLETE",
            "stack.tracks.qualityAutomation",
            "New-project Web baselines must include a selected qualityAutomation track with a concrete browser automation stack.",
        ));
    }
}

fn new_project_stack_tracks_complete(stack: &Value) -> bool {
    let Some(tracks) = stack.get("tracks").and_then(Value::as_object) else {
        return false;
    };
    NEW_PROJECT_CORE_TRACKS.iter().all(|track| {
        let Some(value) = tracks.get(*track).and_then(Value::as_object) else {
            return false;
        };
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let selection = value
            .get("selection")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let track_valid = NEW_PROJECT_TRACK_STATUSES.contains(&status) && !selection.is_empty();
        track_valid
            && (*track != "externalServices" || external_services_track_complete(value, status))
    })
}

fn external_services_track_complete(track: &serde_json::Map<String, Value>, status: &str) -> bool {
    let Some(providers) = track.get("providers") else {
        return !matches!(status, "selected" | "user_custom");
    };
    let Some(providers) = providers.as_array() else {
        return false;
    };
    if !matches!(status, "selected" | "user_custom") {
        return providers.is_empty();
    }
    !providers.is_empty()
        && providers.iter().all(|provider| {
            let Some(provider) = provider.as_object() else {
                return false;
            };
            let provider_name = provider
                .get("provider")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let Some(capabilities) = provider.get("capabilities").and_then(Value::as_array) else {
                return false;
            };
            !provider_name.is_empty()
                && !capabilities.is_empty()
                && capabilities.iter().all(|capability| {
                    let Some(capability) = capability.as_object() else {
                        return false;
                    };
                    let purpose = capability
                        .get("purpose")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let durability = capability
                        .get("durability")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let startup_requirement = capability
                        .get("startupRequirement")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    matches!(
                        purpose,
                        "cache" | "session" | "queue" | "stream" | "lock_rate_limit"
                    ) && matches!(durability, "ephemeral" | "persistent")
                        && matches!(
                            startup_requirement,
                            "required" | "optional" | "not_applicable"
                        )
                })
        })
}

fn new_project_web_selected(stack: &Value) -> bool {
    stack
        .pointer("/tracks/web")
        .and_then(Value::as_object)
        .is_some_and(|track| {
            matches!(
                track.get("status").and_then(Value::as_str),
                Some("selected" | "user_custom")
            )
        })
}

fn new_project_quality_automation_complete(stack: &Value) -> bool {
    stack
        .pointer("/tracks/qualityAutomation")
        .and_then(Value::as_object)
        .is_some_and(|track| {
            matches!(
                track.get("status").and_then(Value::as_str),
                Some("selected" | "user_custom")
            ) && track
                .get("selection")
                .and_then(Value::as_str)
                .is_some_and(|selection| !selection.trim().is_empty())
        })
}

fn technical_baseline_decision_needs(
    project_kind: ProjectKind,
    baseline_exists: bool,
) -> Vec<String> {
    if matches!(project_kind, ProjectKind::NewProject) {
        return vec![
            "web client technology track when applicable".to_string(),
            "app client technology track when applicable".to_string(),
            "backend/service technology track".to_string(),
            "database or persistence technology track".to_string(),
            "ORM or data access technology track".to_string(),
            "external services only when required by the confirmed requirement".to_string(),
            "browser quality automation when a Web client is selected".to_string(),
        ];
    }
    if baseline_exists {
        return vec![
            "whether the current confirmed scope explicitly adds a new technology surface"
                .to_string(),
            "whether the current confirmed scope explicitly replaces a previous technology baseline element"
                .to_string(),
            "otherwise reuse the previous TechnicalBaseline unchanged for normal bugfix, repair, optimization, or feature work inside the existing stack"
                .to_string(),
        ];
    }
    if matches!(project_kind, ProjectKind::Unknown) {
        return vec!["confirm_project_kind".to_string()];
    }
    vec![
        "current repository runtime, language, framework, and package-manager evidence".to_string(),
        "current repository persistence or data-access evidence when present".to_string(),
        "confirmed requirement technology preferences, if the user explicitly provided any"
            .to_string(),
    ]
}

fn technical_baseline_conflicts(
    previous: &TechnicalBaselineContract,
    candidate: &TechnicalBaselineCandidateAgentWritable,
) -> bool {
    previous.project_kind != candidate.project_kind
        || previous.scope != candidate.scope
        || !stable_stack_equivalent(&previous.stack, &candidate.stack)
        || previous.constraints != candidate.constraints
        || previous.security_profiles != candidate.security_profiles
}

fn stable_stack_equivalent(left: &Value, right: &Value) -> bool {
    normalize_stack_for_comparison(left) == normalize_stack_for_comparison(right)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StableStack {
    runtimes: BTreeSet<String>,
    languages: BTreeSet<String>,
    frameworks: BTreeSet<String>,
    package_managers: BTreeSet<String>,
    databases: BTreeSet<String>,
    external_services: BTreeSet<String>,
    tracks: BTreeSet<String>,
}

fn normalize_stack_for_comparison(stack: &Value) -> StableStack {
    StableStack {
        runtimes: values_for_keys(stack, &["runtime", "runtimes", "runtimeKind"]),
        languages: values_for_keys(stack, &["language", "languages"]),
        frameworks: values_for_keys(
            stack,
            &[
                "framework",
                "frameworks",
                "frontendFramework",
                "backendFramework",
            ],
        ),
        package_managers: values_for_keys(stack, &["packageManager", "packageManagers"]),
        databases: values_for_keys(stack, &["database", "databases", "databaseProvider"]),
        external_services: external_service_values(stack),
        tracks: stack_track_selections_for_comparison(stack),
    }
}

fn external_service_values(stack: &Value) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let Some(track) = stack.pointer("/tracks/externalServices") else {
        return values;
    };
    if let Some(providers) = track.get("providers").and_then(Value::as_array) {
        for provider in providers {
            let provider_name = provider
                .get("provider")
                .and_then(Value::as_str)
                .map(normalize_stack_token)
                .unwrap_or_default();
            for capability in provider
                .get("capabilities")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let purpose = capability
                    .get("purpose")
                    .and_then(Value::as_str)
                    .map(normalize_stack_token)
                    .unwrap_or_default();
                let durability = capability
                    .get("durability")
                    .and_then(Value::as_str)
                    .map(normalize_stack_token)
                    .unwrap_or_default();
                let startup = capability
                    .get("startupRequirement")
                    .and_then(Value::as_str)
                    .map(normalize_stack_token)
                    .unwrap_or_default();
                values.insert(format!("{provider_name}:{purpose}:{durability}:{startup}"));
            }
        }
    } else if let Some(selection) = track.get("selection").and_then(Value::as_str) {
        let normalized = normalize_stack_token(selection);
        if !normalized.is_empty() {
            values.insert(normalized);
        }
    }
    values
}

fn values_for_keys(value: &Value, keys: &[&str]) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    for key in keys {
        collect_stack_value(value.get(*key).unwrap_or(&Value::Null), &mut values);
    }
    values
}

fn collect_stack_value(value: &Value, output: &mut BTreeSet<String>) {
    if let Some(text) = value.as_str() {
        let normalized = normalize_stack_token(text);
        if !normalized.is_empty() {
            output.insert(normalized);
        }
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            collect_stack_value(item, output);
        }
    }
}

fn stack_track_selections_for_comparison(stack: &Value) -> BTreeSet<String> {
    let Some(tracks) = stack.get("tracks").and_then(Value::as_object) else {
        return BTreeSet::new();
    };
    tracks
        .iter()
        .filter_map(|(track_name, track)| {
            let status = track
                .get("status")
                .and_then(Value::as_str)
                .map(normalize_stack_token)
                .unwrap_or_default();
            let selection = track
                .get("selection")
                .and_then(Value::as_str)
                .map(normalize_stack_token)
                .unwrap_or_default();
            if status.is_empty() && selection.is_empty() {
                None
            } else {
                Some(format!(
                    "{}:{}:{}",
                    normalize_stack_token(track_name),
                    status,
                    selection
                ))
            }
        })
        .collect()
}

fn repo_signals_conflict_with_previous(
    project_root: &Path,
    request_ref: &str,
    previous: &TechnicalBaselineContract,
) -> Result<Vec<String>, state::store::StateError> {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: project_root.to_string_lossy().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "repoEvidence.signals.packageManagers".to_string(),
            "repoEvidence.signals.frameworks".to_string(),
            "repoEvidence.signals.languages".to_string(),
        ],
    })?;
    let stack = normalize_stack_for_comparison(&previous.stack);
    let mut conflicts = Vec::new();
    push_signal_conflict(
        &mut conflicts,
        "packageManagers",
        &stack.package_managers,
        field_string_set(&fields.fields, "repoEvidence.signals.packageManagers"),
    );
    push_signal_conflict(
        &mut conflicts,
        "frameworks",
        &stack.frameworks,
        field_string_set(&fields.fields, "repoEvidence.signals.frameworks"),
    );
    push_signal_conflict(
        &mut conflicts,
        "languages",
        &stack.languages,
        field_string_set(&fields.fields, "repoEvidence.signals.languages"),
    );
    if stack.runtimes.contains("node") {
        let mut repo_tokens =
            field_string_set(&fields.fields, "repoEvidence.signals.packageManagers");
        repo_tokens.extend(field_string_set(
            &fields.fields,
            "repoEvidence.signals.languages",
        ));
        let has_node_signal = ["npm", "pnpm", "yarn", "bun", "typescript", "javascript"]
            .iter()
            .any(|token| repo_tokens.contains(*token));
        let has_other_runtime_signal = ["maven", "gradle", "java", "python", "go", "rust"]
            .iter()
            .any(|token| repo_tokens.contains(*token));
        if !has_node_signal && has_other_runtime_signal {
            conflicts.push("runtime".to_string());
        }
    }
    Ok(conflicts)
}

fn field_string_set(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    field: &str,
) -> BTreeSet<String> {
    fields
        .get(field)
        .and_then(|result| result.value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_stack_token)
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn push_signal_conflict(
    conflicts: &mut Vec<String>,
    label: &str,
    baseline_values: &BTreeSet<String>,
    signal_values: BTreeSet<String>,
) {
    if baseline_values.is_empty() || signal_values.is_empty() {
        return;
    }
    if !baseline_values
        .iter()
        .any(|value| signal_values.contains(value))
    {
        conflicts.push(label.to_string());
    }
}

fn normalize_stack_token(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .trim_end_matches(".js")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn infer_project_kind_for_baseline(
    project_root: &Path,
    delivery: &DeliveryIndex,
    phase_id: &str,
    has_previous_baseline: bool,
) -> ProjectKind {
    if has_previous_baseline || is_after_first_delivery_phase(delivery, phase_id) {
        return ProjectKind::ExistingProject;
    }
    infer_project_kind_from_repo(project_root)
}

fn is_after_first_delivery_phase(delivery: &DeliveryIndex, phase_id: &str) -> bool {
    delivery
        .phases
        .iter()
        .position(|phase| phase.phase_id == phase_id)
        .map(|index| index > 0)
        .unwrap_or(false)
}

fn infer_project_kind_from_repo(project_root: &Path) -> ProjectKind {
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
        ProjectKind::NewProject
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
    LoomMcpActionResult::UserGate(LoomMcpUserGateResult::new(
        input.project_root.clone(),
        prompt,
        vec!["reply_in_chat".to_string()],
        Some(input.request_ref.clone()),
        authorized.delivery_id.clone(),
        authorized.phase_id.clone(),
        Some(json!({
            "gateId": gate_id,
            "kind": "technical_baseline_confirmation"
        })),
    ))
}

fn repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
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
        resubmit_tool: "loom.technicalBaselineAcceptFile".to_string(),
        fix_scope: Some("technical_baseline_candidate_only".to_string()),
        read_groups: authorized.read_groups.clone(),
        agent_instruction: delivery_core::repairable_error_agent_instruction(
            "loom.technicalBaselineAcceptFile",
        ),
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
