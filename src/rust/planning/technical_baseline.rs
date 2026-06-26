use std::path::Path;

use contracts::{
    BrainstormContract, ProjectKind, TechnicalBaselineApprovalType,
    TechnicalBaselineCandidateAgentWritable, TechnicalBaselineContract, TechnicalBaselineStatus,
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
    let previous_baseline_file = technical_baseline_file(root, delivery_id);
    let previous_baseline = if previous_baseline_file.exists() {
        let previous_baseline_ref = to_project_relative(root, &previous_baseline_file)?;
        let previous: TechnicalBaselineContract = state::store::read_json(&previous_baseline_file)?;
        Some((previous_baseline_ref, previous))
    } else {
        None
    };
    let request_root = build_request_root(
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
            "updatedAt": previous.updated_at
        })
    });
    let selection_guidance = technical_baseline_selection_guidance(project_kind, baseline_exists);
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
            "domainModel": brainstorm.domain_model,
            "acceptanceIndex": brainstorm.acceptance.iter().map(|acceptance| json!({
                "id": acceptance.id,
                "priority": acceptance.priority,
                "capabilityRefs": acceptance.capability_refs,
                "sourceRefs": acceptance.source_refs
            })).collect::<Vec<_>>(),
            "frontendExperience": brainstorm.frontend_experience,
            "userFacingLanguage": brainstorm.delivery_context.user_facing_language,
            "roadmap": brainstorm.roadmap,
            "phasePlan": {
                "current": brainstorm.phase_plan.current,
                "nextPhasePreview": brainstorm.phase_plan.next_phase_preview
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
                    "purpose": "Read the confirmed Brainstorm scope, acceptance ids, frontend target, current phase lens, and baseline decision needs before drafting the baseline.",
                    "whenToRead": "Read before producing any TechnicalBaseline recommendation.",
                    "fields": [
                        "brainstormLens.summary",
                        "brainstormLens.scope",
                        "brainstormLens.domainModel",
                        "brainstormLens.acceptanceIndex",
                        "brainstormLens.frontendExperience",
                        "brainstormLens.userFacingLanguage",
                        "brainstormLens.roadmap",
                        "brainstormLens.phasePlan",
                        "currentPhaseLens",
                        "decisionNeeds",
                        "previousBaselineContext",
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
                    "required": selection_guidance.is_some(),
                    "purpose": "Read the greenfield confirmation discipline before asking the user to confirm the baseline.",
                    "whenToRead": "Read only when the projectKind is greenfield or the baseline still needs explicit user confirmation.",
                    "fields": [
                        "selectionGuidance"
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

fn technical_baseline_selection_guidance(
    project_kind: ProjectKind,
    has_previous_baseline: bool,
) -> Option<Value> {
    if !matches!(project_kind, ProjectKind::Greenfield) && !has_previous_baseline {
        return None;
    }
    Some(json!({
        "schemaVersion": "1.0",
        "purpose": if matches!(project_kind, ProjectKind::Greenfield) {
            "Guide the agent-user technical baseline confirmation for a greenfield empty project before PGC."
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
            "requiredFinalShape": "Use stack.tracks with web, app, backend, persistence, dataAccess, and externalServices keys. Each track should include status, selection, source, and rationale.",
            "trackStatusValues": ["selected", "not_needed", "not_applicable", "user_custom"],
            "sourceValues": ["agent_recommended_user_confirmed", "user_adjusted", "user_specified", "previous_baseline", "not_applicable"],
            "coreTracks": ["web", "app", "backend", "persistence", "dataAccess", "externalServices"],
            "customTechnologyPolicy": "Common options are examples, not a whitelist. User-specified technologies outside these examples are allowed, but mark the relevant track source as user_specified or user_custom and include it in the final confirmation summary and reasoningSummary."
        },
        "recommendationBasis": {
            "authority": "Use the complete BrainstormContract as the product-scope authority for the first greenfield TechnicalBaseline recommendation.",
            "mustRead": [
                "Brainstorm summary and original requirement context",
                "scope.included, scope.deferred, scope.excluded, and assumptions",
                "domainModel capability groups and business flows",
                "frontendExperience when present",
                "roadmap phases, deferred scope, and known next-phase previews"
            ],
            "currentPhaseLensRole": "currentPhaseLens identifies the first implementation slice only. Do not choose the initial technology baseline from the current phase scope alone when the full requirement or roadmap implies later product surfaces, persistence scale, app clients, services, integrations, or operational needs.",
            "recommendationRule": "Recommend a stable baseline for the full confirmed delivery/roadmap horizon; explain when the current phase can start small inside that baseline without hiding later known needs."
        },
        "userFacingConfirmationProtocol": {
            "mandatorySections": [
                "Recommendation basis: summarize the full requirement/roadmap signals used, not only the current phase.",
                "Recommended final baseline: list every core track with selection and short rationale.",
                "Adjustable technology range: show common examples for every core track so the user knows how to modify the recommendation.",
                "Reply format: show canonical key=value examples using web, app, backend, persistence, dataAccess, and externalServices.",
                "Final confirmation rule: if the user changes anything, summarize the final baseline and ask for explicit confirmation before submitting."
            ],
            "wordingRules": [
                "Do not present the recommendation as based only on the first phase or current small implementation slice.",
                "Do not omit the adjustable technology range.",
                "Do not present backend options as bare language-only labels when a mainstream framework choice is expected; show language + framework combinations in user-facing examples.",
                "Do not use db or orm as the primary reply keys; use persistence and dataAccess in the primary examples.",
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
                "examples": ["None", "User specified", "Only recommend services explicitly required by the confirmed requirement"]
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
            "partialAdjustmentExample": "web=Vue+Vite, backend=Java+Spring Boot, persistence=PostgreSQL, dataAccess=Spring Data JPA, app=不需要, externalServices=不需要",
            "fullCustomExample": "web=React+Vite, app=React Native+Expo, backend=Fastify, persistence=SQLite, dataAccess=Prisma, externalServices=不需要",
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
        "Testing, build, local run, and deployment preparation are derived later. Do not require first-screen user choices for them and do not reopen technology baseline confirmation only to update those commands.",
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
            "Ask the user whether this phase continues an existing project or starts a new project, then rewrite the same candidate with the confirmed projectKind.".to_string(),
            "project_kind_confirmation".to_string(),
        ));
    }
    if matches!(candidate.project_kind, ProjectKind::Greenfield)
        && candidate.approval.r#type != TechnicalBaselineApprovalType::UserConfirmed
    {
        return Ok(technical_baseline_user_gate(
            input,
            authorized,
            "The technology baseline for a new project must be explicitly confirmed by the user before planning continues. Present the recommended stack, capture corrections, then rewrite the same candidate with approval.type=user_confirmed.".to_string(),
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
            "The technology baseline still requires explicit user confirmation. Present the baseline change or recommendation, then rewrite the same candidate with the confirmed baseline.".to_string(),
            "technical_baseline_confirmation".to_string(),
        ));
    }
    let previous_baseline_file = technical_baseline_file(project_root, &delivery_id);
    if previous_baseline_file.exists() {
        let previous: TechnicalBaselineContract = state::store::read_json(&previous_baseline_file)?;
        if technical_baseline_conflicts(&previous, &candidate)
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

fn technical_baseline_conflicts(
    previous: &TechnicalBaselineContract,
    candidate: &TechnicalBaselineCandidateAgentWritable,
) -> bool {
    previous.project_kind != candidate.project_kind
        || previous.scope != candidate.scope
        || previous.stack != candidate.stack
        || previous.constraints != candidate.constraints
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
