use std::{collections::BTreeMap, path::Path};

use contracts::{
    BrainstormContract, BrainstormHandoff, BrainstormHandoffNode, BrainstormStatus, Complexity,
    PhasePlan, PhasePlanCurrent, PhasePlanCurrentStatus, RequestSummary, Roadmap, RoadmapPhase,
    UserConfirmation,
};
use delivery_core::{
    DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState, LoomMcpActionResult,
    LoomMcpFailure, LoomMcpFailureResult, LoomMcpUserGateResult, RouteAction, RouteActionKind,
    TransitionStore, ValidatedPlanInput,
};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{self, to_project_relative, DeliveryPhaseLocator},
    store::{ensure_dir, write_json_atomic},
};

use crate::{
    clarification::{initial_state, write_initial_state_file, ClarificationProfile},
    gate::{gate_for_block, to_value},
    paths::brainstorm_contract_file,
    request::build_brainstorm_request_root,
    requirements::{build_requirement_artifacts, RequirementArtifacts},
};

pub fn start_brainstorm(input: &ValidatedPlanInput) -> LoomMcpActionResult {
    match start_brainstorm_inner(input) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "BRAINSTORM_START_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(7),
                domain: Some("brainstorm".to_string()),
                route_action: Some("brainstorm_start".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn start_brainstorm_inner(
    input: &ValidatedPlanInput,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let project_root = Path::new(&input.project_root);
    let transaction_id = format!("lifecycle-tx-{}", state::store::now_millis());
    let staged = state::prepare_staged_project(project_root, &transaction_id)?;
    let staged_root = staged.root.as_path();
    let store = FileTransitionStore;
    let status = store
        .load_status(&input.project_root)
        .map_err(to_state_error)?;
    if let Some(active_delivery_id) = status.active_delivery_id.as_deref() {
        let active_delivery = store
            .load_delivery_index(&input.project_root, active_delivery_id)
            .map_err(to_state_error)?;
        let active = !matches!(
            active_delivery.status,
            DeliveryLifecycleStatus::Completed
                | DeliveryLifecycleStatus::CompletedWithOverride
                | DeliveryLifecycleStatus::Superseded
        );
        if active && input.supersede_active_delivery_id.as_deref() != Some(active_delivery_id) {
            return Err(state::store::StateError::StateCorrupted(
                "an active Loom delivery already exists; resolve the plan conflict before starting another delivery".to_string(),
            ));
        }
    }
    let now = state::store::now_string();
    let delivery_id = format!("delivery-{}", state::store::now_millis());
    let phase_id = "phase-1".to_string();
    let brainstorm_run_id = format!("brainstorm-run-{}", state::store::now_millis());
    let request_id = format!("brainstorm-session-{}", state::store::now_millis());
    let contract_id = format!("brainstorm-contract-{}", state::store::now_millis());
    let requirement = build_requirement_artifacts(
        staged_root,
        &delivery_id,
        &input.request_text,
        &input.requirement_files,
    )?;
    let summary = summarize_request(&input.request_text, input.requirement_files.len());
    let context_refs = serde_json::json!({
        "requirementContextRef": requirement.context_ref,
        "normalizedRequirementTextRef": requirement.normalized_text_ref,
        "keywordHintsRef": requirement.keyword_hints_ref,
    });

    let contract = initial_contract(
        &delivery_id,
        &phase_id,
        &brainstorm_run_id,
        &contract_id,
        &summary,
        &requirement,
        &input.request_text,
        &input.requirement_files,
        &now,
    );
    let contract_file = brainstorm_contract_file(staged_root, &delivery_id);
    write_json_atomic(&contract_file, &contract)?;
    let contract_ref = to_project_relative(staged_root, &contract_file)?;
    let (clarification_profile, clarification_profile_reason) =
        classify_clarification_profile(project_root, &input.request_text);
    let clarification_state = initial_state(
        &delivery_id,
        &phase_id,
        &brainstorm_run_id,
        clarification_profile.clone(),
        clarification_profile_reason,
    );
    let clarification_state_ref =
        write_initial_state_file(staged_root, &delivery_id, &phase_id, &clarification_state)?;

    let request_root = build_brainstorm_request_root(
        staged_root,
        &request_id,
        &delivery_id,
        &phase_id,
        &brainstorm_run_id,
        &requirement.user_facing_language,
        context_refs,
        &clarification_profile,
    );
    let stored = state::write_native_request_staged(
        &staged_root.to_string_lossy(),
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "brainstorm_clarification_block".to_string(),
            request_file: None,
            delivery_id: Some(delivery_id.clone()),
            phase_id: Some(phase_id.clone()),
            root: request_root,
        },
    )?;

    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    ensure_dir(
        &paths::workspace_dir(staged_root, &locator)
            .join("brainstorm-knowledge")
            .join(&request_id),
    )?;

    let gate = gate_for_block(
        contracts::ClarificationBlockName::PhaseScope,
        vec![],
        vec![],
        &clarification_profile,
    );

    let mut latest_refs = BTreeMap::new();
    latest_refs.insert("brainstormRequestId".to_string(), request_id.clone());
    latest_refs.insert(
        "brainstormRequestRef".to_string(),
        stored.request_ref.clone(),
    );
    latest_refs.insert("brainstormRunId".to_string(), brainstorm_run_id.clone());
    latest_refs.insert("brainstormContract".to_string(), contract_ref);
    latest_refs.insert(
        "brainstormClarificationState".to_string(),
        clarification_state_ref,
    );
    latest_refs.insert(
        "requirementContext".to_string(),
        requirement.context_ref.clone(),
    );
    if let Some(normalized) = &requirement.normalized_text_ref {
        latest_refs.insert("normalizedRequirementText".to_string(), normalized.clone());
    }
    if let Some(keyword_hints) = &requirement.keyword_hints_ref {
        latest_refs.insert("keywordHints".to_string(), keyword_hints.clone());
    }

    let route_action = RouteAction {
        kind: RouteActionKind::BrainstormClarification,
        source: "brainstorm_start".to_string(),
        reason: "await_phase_scope_confirmation".to_string(),
        prompt: Some(
            "Read the current Brainstorm block request, query request-scoped knowledge for this block, and present only active phase boundary options in the user's language."
                .to_string(),
        ),
        accepted_responses: vec!["reply_in_chat".to_string()],
        request_ref: Some(stored.request_ref.clone()),
        details: Some(to_value(&gate)),
        target_phase_id: None,
    };

    let delivery = DeliveryIndex {
        schema_version: 1,
        delivery_id: delivery_id.clone(),
        active_phase_id: phase_id.clone(),
        status: DeliveryLifecycleStatus::Planning,
        phases: vec![DeliveryPhaseState {
            phase_id: phase_id.clone(),
            latest_refs,
            next_action: Some(route_action),
            pending_repair: None,
        }],
        updated_at: now.clone(),
        request_ref: Some(input.request_identity.request_ref.clone()),
        request_fingerprint: Some(input.request_identity.fingerprint.clone()),
    };
    state::persist_plan_request(input)?;
    let mut lifecycle = state::LifecycleCommit {
        expected_revision: input.expected_lifecycle_revision,
        expected_active_delivery_id: Some(input.supersede_active_delivery_id.clone()),
        deliveries: vec![delivery],
        pending_plan_conflict_id: Some(None),
        ..state::LifecycleCommit::default()
    };
    if input.plan_conflict_id.is_none() {
        if let Some(conflict_id) = status.pending_plan_conflict_id.as_deref() {
            let mut conflict = state::load_plan_conflict(&input.project_root, conflict_id)?;
            if conflict.status == delivery_core::PlanConflictStatus::Pending {
                conflict.status = delivery_core::PlanConflictStatus::Expired;
                conflict.updated_at = state::store::now_string();
                lifecycle.conflicts.push(conflict);
            }
        }
    }
    if let Some(conflict_id) = input.plan_conflict_id.as_deref() {
        let mut conflict = state::load_plan_conflict(&input.project_root, conflict_id)?;
        if conflict.status != delivery_core::PlanConflictStatus::Pending
            || conflict.active_delivery_id
                != input
                    .supersede_active_delivery_id
                    .as_deref()
                    .unwrap_or_default()
            || status.pending_plan_conflict_id.as_deref() != Some(conflict_id)
        {
            return Err(state::store::StateError::StateCorrupted(
                "STALE_PLAN_CONFLICT: the plan conflict changed before the new delivery was committed"
                    .to_string(),
            ));
        }
        conflict.status = delivery_core::PlanConflictStatus::ResolvedStartNew;
        conflict.updated_at = state::store::now_string();
        lifecycle.conflicts.push(conflict);
    }
    if let Some(old_delivery_id) = input.supersede_active_delivery_id.as_deref() {
        let mut old_delivery = store
            .load_delivery_index(&input.project_root, old_delivery_id)
            .map_err(to_state_error)?;
        old_delivery.status = DeliveryLifecycleStatus::Superseded;
        old_delivery.updated_at = state::store::now_string();
        lifecycle.deliveries.insert(0, old_delivery);
        if let Some(mut lease) = store
            .read_operation_lease(&input.project_root, old_delivery_id)
            .map_err(to_state_error)?
        {
            lease.close(state::store::now_string());
            lifecycle.leases.push(lease);
        }
    }
    let artifacts = state::collect_prepared_artifacts(staged_root, project_root)?;
    let commit_result = state::commit_lifecycle_with_artifacts(
        &input.project_root,
        lifecycle,
        artifacts,
        Some(staged.into_commit_cleanup()),
    );
    if let Err(error) = commit_result {
        return Err(error);
    }
    Ok(LoomMcpActionResult::UserGate(
        LoomMcpUserGateResult::new(
            input.project_root.clone(),
            "Read the current Brainstorm block request, query request-scoped knowledge for this block, present only active phase boundary options in the user's language, then call loom.brainstormConfirmBlock after the user confirms one boundary.",
            vec!["reply_in_chat".to_string()],
            Some(stored.request_ref),
            Some(delivery_id),
            Some(phase_id),
            Some(to_value(&gate)),
        )
        .with_brainstorm_knowledge("phase_scope"),
    ))
}

fn classify_clarification_profile(
    project_root: &Path,
    request_text: &str,
) -> (ClarificationProfile, Option<String>) {
    if !looks_like_existing_repository(project_root) {
        return (ClarificationProfile::Full, None);
    }

    let normalized = request_text.to_ascii_lowercase();
    let has_maintenance_intent = [
        "fix",
        "bug",
        "regression",
        "crash",
        "exception",
        "incorrect",
        "wrong",
        "fails",
        "failure",
        "should return",
        "should handle",
    ]
    .iter()
    .any(|signal| normalized.contains(signal));
    let has_concrete_target = request_text.contains('`')
        || request_text.contains(".rs")
        || request_text.contains(".py")
        || request_text.contains(".ts")
        || request_text.contains(".js")
        || request_text.contains(".go")
        || normalized.contains("function")
        || normalized.contains("method")
        || normalized.contains("test");
    let has_broad_or_sensitive_intent = [
        " ui ",
        "frontend",
        "page",
        "screen",
        "api",
        "endpoint",
        "schema",
        "migration",
        "database",
        "auth",
        "authorization",
        "security",
        "permission",
        "deploy",
        "deployment",
        "infrastructure",
        "runtime",
        "feature",
        "redesign",
        "refactor",
    ]
    .iter()
    .any(|signal| has_unexcluded_signal(&normalized, signal));
    let is_bounded = request_text.chars().count() <= 1_200;

    if has_maintenance_intent && has_concrete_target && is_bounded && !has_broad_or_sensitive_intent
    {
        return (
            ClarificationProfile::ExplicitMaintenance,
            Some("Existing repository with a bounded maintenance request and a concrete implementation target; broad or sensitive delivery signals were absent.".to_string()),
        );
    }
    (ClarificationProfile::Full, None)
}

fn has_unexcluded_signal(request_text: &str, signal: &str) -> bool {
    request_text.split(['.', ';', '\n']).any(|clause| {
        clause.contains(signal)
            && ![
                "do not ",
                "don't ",
                "no ",
                "without ",
                "not change",
                "not modify",
                "not touch",
                "preserve ",
                "keep existing",
                "keep the existing",
            ]
            .iter()
            .any(|exclusion| clause.contains(exclusion))
    })
}

fn looks_like_existing_repository(project_root: &Path) -> bool {
    project_root.join(".git").exists()
        || [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "setup.py",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "Gemfile",
        ]
        .iter()
        .any(|marker| project_root.join(marker).is_file())
}

fn initial_contract(
    delivery_id: &str,
    phase_id: &str,
    brainstorm_run_id: &str,
    contract_id: &str,
    summary: &RequestSummary,
    requirement: &RequirementArtifacts,
    request_text: &str,
    requirement_files: &[String],
    now: &str,
) -> BrainstormContract {
    BrainstormContract {
        schema_version: "1.0".to_string(),
        contract_id: contract_id.to_string(),
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
        brainstorm_run_id: brainstorm_run_id.to_string(),
        status: BrainstormStatus::NeedsClarification,
        sources: requirement.formal_sources.clone(),
        summary: summary.clone(),
        scope: contracts::BrainstormScope {
            included: vec![],
            excluded: vec![],
            deferred: vec![],
            assumptions: vec![],
        },
        acceptance: vec![],
        domain_model: None,
        user_confirmation: UserConfirmation {
            confirmed: false,
            confirmed_at: None,
            confirmation_summary: "Waiting for final_summary confirmation.".to_string(),
            confirmation_basis: None,
        },
        delivery_context: contracts::DeliveryContext {
            original_request: contracts::OriginalRequestContext {
                text: request_text.to_string(),
                input_refs: requirement_files.to_vec(),
            },
            user_facing_language: requirement.user_facing_language.clone(),
            workflow_profile: contracts::DeliveryWorkflowProfile::Full,
        },
        roadmap: Roadmap {
            required: false,
            current_phase_id: phase_id.to_string(),
            phases: vec![RoadmapPhase {
                phase_id: phase_id.to_string(),
                title: Some(summary.title.clone()),
                name: Some(summary.title.clone()),
                status: contracts::PhaseStatus::ScopeConfirmed,
                goal: summary
                    .business_goal
                    .clone()
                    .or_else(|| Some(summary.one_line.clone())),
                scope_refs: vec![],
                acceptance_refs: vec![],
                depends_on: vec![],
            }],
        },
        phase_plan: PhasePlan {
            current: PhasePlanCurrent {
                phase_id: phase_id.to_string(),
                title: summary.title.clone(),
                goal: summary.one_line.clone(),
                scope_refs: vec![],
                acceptance_refs: vec![],
                status: PhasePlanCurrentStatus::ScopeConfirmed,
            },
            next_phase_preview: contracts::NextPhasePreview::None {
                reason: "Next phase preview will be decided during Brainstorm clarification."
                    .to_string(),
            },
        },
        security_requirement: contracts::SecurityRequirement {
            applies: contracts::SecurityRequirementApplicability::NotApplicable,
            client_trust_models: vec![],
            source_refs: vec![],
            rationale: "Security applicability will be confirmed during requirement clarification."
                .to_string(),
        },
        concept_grounding: None,
        concept_confirmation: None,
        clarification_progress: None,
        concept_grounding_refs: None,
        frontend_experience: None,
        frontend_experience_refs: None,
        handoff: BrainstormHandoff {
            ready: false,
            next_node: BrainstormHandoffNode::BrainstormClarification,
            blocking_reasons: vec![],
        },
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn summarize_request(request_text: &str, file_count: usize) -> RequestSummary {
    let one_line = request_text
        .lines()
        .next()
        .unwrap_or(request_text)
        .trim()
        .chars()
        .take(120)
        .collect::<String>();
    let title = one_line
        .split(['。', '.', '!', '?', '\n'])
        .next()
        .unwrap_or("Brainstorm request")
        .trim()
        .chars()
        .take(60)
        .collect::<String>();
    let complexity = if request_text.len() > 1600 || file_count > 1 {
        Complexity::Large
    } else if request_text.len() > 500 || file_count == 1 {
        Complexity::Medium
    } else {
        Complexity::Small
    };
    RequestSummary {
        title: if title.is_empty() {
            "Brainstorm request".to_string()
        } else {
            title
        },
        one_line: if one_line.is_empty() {
            "Clarify the current phase scope.".to_string()
        } else {
            one_line.clone()
        },
        business_goal: Some(one_line),
        complexity,
    }
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::from_core_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_path(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "loom-brainstorm-profile-{name}-{}",
            state::store::now_millis()
        ));
        std::fs::create_dir_all(&path).expect("create test project");
        std::fs::write(path.join("pyproject.toml"), "[project]\nname = 'test'\n")
            .expect("write repository marker");
        path
    }

    #[test]
    fn selects_explicit_maintenance_only_for_bounded_fix() {
        let path = repo_path("maintenance");
        let (profile, reason) = classify_clarification_profile(
            &path,
            "Fix `django.utils.numberformat.format()` so None returns unchanged and add a regression test in tests/utils_tests/test_numberformat.py.",
        );
        assert_eq!(profile, ClarificationProfile::ExplicitMaintenance);
        assert!(reason.is_some());
        std::fs::remove_dir_all(path).expect("remove test project");
    }

    #[test]
    fn falls_back_to_full_for_api_change() {
        let path = repo_path("api");
        let (profile, reason) = classify_clarification_profile(
            &path,
            "Fix `format()` and add a new API endpoint for callers.",
        );
        assert_eq!(profile, ClarificationProfile::Full);
        assert!(reason.is_none());
        std::fs::remove_dir_all(path).expect("remove test project");
    }

    #[test]
    fn explicit_exclusions_do_not_disable_maintenance_profile() {
        let path = repo_path("excluded-signals");
        let (profile, _) = classify_clarification_profile(
            &path,
            "Fix `format()` and add a regression test. Do not change public APIs, database schema, migrations, authentication, security, deployment, runtime configuration, or UI.",
        );
        assert_eq!(profile, ClarificationProfile::ExplicitMaintenance);
        std::fs::remove_dir_all(path).expect("remove test project");
    }

    #[test]
    fn maintenance_delivery_language_does_not_disable_the_profile() {
        let path = repo_path("delivery-language");
        let (profile, _) = classify_clarification_profile(
            &path,
            "Fix `format()` and add a regression test. Complete implementation and focused verification.",
        );
        assert_eq!(profile, ClarificationProfile::ExplicitMaintenance);
        std::fs::remove_dir_all(path).expect("remove test project");
    }

    #[test]
    fn preserving_existing_security_boundary_does_not_disable_maintenance_profile() {
        let path = repo_path("preserve-security-boundary");
        let (profile, _) = classify_clarification_profile(
            &path,
            "Fix `django.utils.numberformat.format()` so None returns unchanged and add a regression test in tests/utils_tests/test_numberformat.py. Keep the existing repository stack and security boundary.",
        );
        assert_eq!(profile, ClarificationProfile::ExplicitMaintenance);
        std::fs::remove_dir_all(path).expect("remove test project");
    }
}
