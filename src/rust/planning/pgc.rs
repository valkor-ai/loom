use std::{collections::BTreeMap, path::Path};

use contracts::{
    BrainstormContract, ContextWarning, PlanningContractContextRefs, PlanningContractPhaseScope,
    PlanningContractSource, PlanningContractStatus, PlanningContractTechnicalBaseline,
    PlanningDeploymentRules, PlanningGenerationContract, PlanningHandoff, PlanningInputs,
    PlanningRules, QualityGates, RequirementDetailItem, RequirementDetailsIndex,
    ScopeIsolationRules, ScopeItem, TechnicalBaselineContract,
};
use delivery_core::{
    apply_delivery_index, DeliveryLifecycleStatus, DomainDispatcher, LoomMcpActionResult,
    LoomMcpFailure, LoomMcpFailureResult, RouteAction, RouteActionKind, TransitionStore,
};
use state::{lifecycle_store::FileTransitionStore, paths::DeliveryPhaseLocator};

use crate::{
    paths::{planning_contract_file, planning_latest_file},
    PlanningDomainDispatcher,
};

pub fn create_contract_and_route(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match create_contract_and_route_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: project_root.to_string(),
            error: LoomMcpFailure {
                code: "PLANNING_CONTRACT_CREATE_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("planning".to_string()),
                route_action: Some("planning_contract_create".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn create_contract_and_route_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let root = Path::new(project_root);
    let store = FileTransitionStore;
    let mut status = store.load_status(project_root).map_err(to_state_error)?;
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let phase = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(format!(
                "phase {} does not exist in delivery {}",
                phase_id, delivery_id
            ))
        })?;

    let brainstorm_contract_ref = phase
        .latest_refs
        .get("brainstormContract")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest brainstormContract ref is missing".to_string(),
            )
        })?;
    let brainstorm = read_brainstorm_contract(root, &brainstorm_contract_ref)?;
    let baseline_ref = phase
        .latest_refs
        .get("technicalBaseline")
        .cloned()
        .ok_or_else(|| {
            state::store::StateError::InvalidArgument(
                "latest technicalBaseline ref is missing".to_string(),
            )
        })?;
    let baseline = read_technical_baseline(root, &baseline_ref)?;

    let repository_context_ref = phase.latest_refs.get("latestRepositoryContext").cloned();
    let current_scope_ids = brainstorm
        .phase_plan
        .current
        .scope_refs
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let current_acceptance_ids = brainstorm
        .phase_plan
        .current
        .acceptance_refs
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let acceptance_by_id = brainstorm
        .acceptance
        .iter()
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    let phase_scope = PlanningContractPhaseScope {
        phase_name: brainstorm.phase_plan.current.title.clone(),
        phase_goal: brainstorm.phase_plan.current.goal.clone(),
        included: filter_scope_items(&brainstorm.scope.included, &current_scope_ids),
        deferred: brainstorm.scope.deferred.clone(),
        excluded: brainstorm.scope.excluded.clone(),
        acceptance_candidates: current_acceptance_ids
            .iter()
            .filter_map(|id| acceptance_by_id.get(id).cloned().cloned())
            .collect(),
    };

    let requirement_details = build_requirement_details_index(
        &brainstorm,
        &brainstorm_contract_ref,
        &phase_scope,
        &current_acceptance_ids,
    );
    let context_refs = PlanningContractContextRefs {
        brainstorm_contract_ref: brainstorm_contract_ref.clone(),
        repository_context_ref: repository_context_ref.clone(),
        delivery_concept_glossary_ref: phase.latest_refs.get("deliveryConceptGlossary").cloned(),
        phase_concept_grounding_ref: phase.latest_refs.get("phaseConceptGrounding").cloned(),
        confirmed_frontend_experience_ref: phase
            .latest_refs
            .get("confirmedFrontendExperience")
            .cloned(),
        current_frontend_experience_ref: phase
            .latest_refs
            .get("currentFrontendExperience")
            .cloned(),
    };

    let business_goal = brainstorm
        .summary
        .business_goal
        .clone()
        .unwrap_or_else(|| brainstorm.summary.one_line.clone());
    let planning_inputs = PlanningInputs {
        business_goal,
        business_flows: brainstorm
            .domain_model
            .as_ref()
            .map(|model| {
                model
                    .business_flows
                    .iter()
                    .map(|flow| serde_json::to_value(flow).unwrap_or(serde_json::Value::Null))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        frontend_experience: brainstorm.frontend_experience.clone(),
        user_facing_language: Some(brainstorm.delivery_context.user_facing_language.clone()),
        source_refs: brainstorm
            .sources
            .iter()
            .map(|source| source.source_id.clone())
            .collect(),
        context_notes: build_context_notes(&repository_context_ref),
    };

    let planning_contract = PlanningGenerationContract {
        schema_version: "1.0".to_string(),
        planning_contract_id: format!("pgc-{}", phase_id),
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
        status: PlanningContractStatus::Ready,
        source: PlanningContractSource {
            brainstorm_run_id: brainstorm.brainstorm_run_id.clone(),
            brainstorm_contract_id: brainstorm.contract_id.clone(),
            roadmap_id: None,
            phase_id: phase_id.to_string(),
            technical_baseline_id: baseline.technical_baseline_id.clone(),
        },
        phase_scope,
        context_refs,
        technical_baseline: PlanningContractTechnicalBaseline {
            technical_baseline_id: baseline.technical_baseline_id.clone(),
            status: baseline.status,
            scope: baseline.scope,
            summary: baseline.stack.clone(),
            must_follow: true,
        },
        planning_inputs,
        requirement_details,
        planning_rules: PlanningRules {
            scope_isolation: ScopeIsolationRules {
                only_plan_current_phase: true,
                forbid_deferred_scope_implementation: true,
                forbid_future_phase_implementation: true,
            },
            output_requirements: contracts::OutputRequirements {
                must_create_architecture_artifact_contract: true,
                must_create_task_plan: true,
                task_plan_must_reference_acceptance: true,
            },
            deployment: PlanningDeploymentRules {
                default_enabled: false,
                requires_explicit_user_request: true,
            },
        },
        quality_gates: QualityGates {
            requires_architecture_before_task_plan: true,
            requires_acceptance_coverage: true,
            requires_verification_evidence: true,
        },
        handoff: PlanningHandoff {
            ready_for_architecture: true,
            ready_for_task_plan: false,
            blocking_reasons: vec![],
            next_node: "architecture_artifact_contract".to_string(),
        },
        created_at: state::store::now_string(),
        updated_at: state::store::now_string(),
    };

    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
    };
    let contract_file = planning_contract_file(root, &locator);
    state::store::write_json_atomic(&contract_file, &planning_contract)?;
    let contract_ref = state::paths::to_project_relative(root, &contract_file)?;
    let latest_file = planning_latest_file(root, &locator);
    state::store::write_json_atomic(
        &latest_file,
        &serde_json::json!({
            "schemaVersion": "1.0",
            "planningContractId": planning_contract.planning_contract_id,
            "contractRef": contract_ref,
            "updatedAt": planning_contract.updated_at
        }),
    )?;

    phase
        .latest_refs
        .insert("planningContract".to_string(), contract_ref.clone());
    let next_action = RouteAction {
        kind: RouteActionKind::ArchitectureArtifactContract,
        source: "planning_contract".to_string(),
        reason: "planning_contract_ready".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(contract_ref),
        details: None,
        target_phase_id: None,
    };
    phase.next_action = Some(next_action.clone());
    delivery.status = DeliveryLifecycleStatus::Planning;
    delivery.updated_at = state::store::now_string();
    let action = next_action;
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)?;
    Ok(
        PlanningDomainDispatcher.dispatch_route_action(
            project_root,
            delivery_id,
            phase_id,
            &action,
        ),
    )
}

fn filter_scope_items(items: &[ScopeItem], current_scope_ids: &[String]) -> Vec<ScopeItem> {
    items
        .iter()
        .filter(|item| current_scope_ids.iter().any(|id| id == &item.id))
        .cloned()
        .collect()
}

fn build_requirement_details_index(
    brainstorm: &BrainstormContract,
    brainstorm_contract_ref: &str,
    phase_scope: &PlanningContractPhaseScope,
    current_acceptance_ids: &[String],
) -> RequirementDetailsIndex {
    let mut items = Vec::new();

    for scope in &phase_scope.included {
        items.push(RequirementDetailItem {
            detail_id: format!("detail.scope.{}", scope.id),
            kind: "scope_item".to_string(),
            title: scope.label.clone(),
            summary: scope.reason.clone().unwrap_or_else(|| scope.label.clone()),
            required_for_current_phase: true,
            priority: "must".to_string(),
            source_field_refs: vec!["scope.included".to_string()],
            source_refs: vec![brainstorm_contract_ref.to_string()],
            scope_refs: vec![scope.id.clone()],
            acceptance_refs: vec![],
            concept_refs: vec![],
            frontend_refs: vec![],
            impact_tags: vec!["scope".to_string()],
            lifecycle_stage: "current_phase".to_string(),
            quality: "confirmed".to_string(),
            unresolved_note: None,
        });
    }

    for acceptance in brainstorm
        .acceptance
        .iter()
        .filter(|item| current_acceptance_ids.iter().any(|id| id == &item.id))
    {
        items.push(RequirementDetailItem {
            detail_id: format!("detail.acceptance.{}", acceptance.id),
            kind: "acceptance_candidate".to_string(),
            title: acceptance.id.clone(),
            summary: acceptance.statement.clone(),
            required_for_current_phase: true,
            priority: format!("{:?}", acceptance.priority).to_lowercase(),
            source_field_refs: vec!["acceptance".to_string()],
            source_refs: if acceptance.source_refs.is_empty() {
                vec![brainstorm_contract_ref.to_string()]
            } else {
                acceptance.source_refs.clone()
            },
            scope_refs: phase_scope
                .included
                .iter()
                .map(|scope| scope.id.clone())
                .collect(),
            acceptance_refs: vec![acceptance.id.clone()],
            concept_refs: vec![],
            frontend_refs: vec![],
            impact_tags: vec!["acceptance".to_string()],
            lifecycle_stage: "acceptance".to_string(),
            quality: "confirmed".to_string(),
            unresolved_note: None,
        });
    }

    if let Some(frontend) = &brainstorm.frontend_experience {
        for path in &frontend.operation_paths {
            items.push(RequirementDetailItem {
                detail_id: format!("detail.frontend.{}", path.path_id),
                kind: "frontend_operation_path".to_string(),
                title: path.name.clone(),
                summary: path.user_goal.clone(),
                required_for_current_phase: true,
                priority: "should".to_string(),
                source_field_refs: vec!["frontendExperience.operationPaths".to_string()],
                source_refs: if path.source_refs.is_empty() {
                    vec![brainstorm_contract_ref.to_string()]
                } else {
                    path.source_refs.clone()
                },
                scope_refs: phase_scope
                    .included
                    .iter()
                    .map(|scope| scope.id.clone())
                    .collect(),
                acceptance_refs: vec![],
                concept_refs: vec![],
                frontend_refs: vec![path.path_id.clone()],
                impact_tags: vec!["frontend".to_string()],
                lifecycle_stage: "frontend".to_string(),
                quality: "confirmed".to_string(),
                unresolved_note: None,
            });
        }
    }

    RequirementDetailsIndex {
        schema_version: "1.0".to_string(),
        authority: "brainstorm_confirmed_scope".to_string(),
        source_brainstorm_contract_ref: brainstorm_contract_ref.to_string(),
        items,
        extraction_warnings: vec![ContextWarning {
            code: "MECHANICAL_PGC_INDEX".to_string(),
            message: "Requirement details are mechanically projected from the confirmed Brainstorm contract.".to_string(),
        }],
    }
}

fn build_context_notes(repository_context_ref: &Option<String>) -> Vec<String> {
    let mut notes = vec![
        "Brainstorm contract is the authority for current phase scope and acceptance.".to_string(),
        "Technical baseline must be preserved as the implementation boundary.".to_string(),
    ];
    if repository_context_ref.is_some() {
        notes.push(
            "RepositoryContext provides repository facts only and must not override current phase scope."
                .to_string(),
        );
    }
    notes
}

fn read_brainstorm_contract(
    project_root: &Path,
    relative_ref: &str,
) -> Result<BrainstormContract, state::store::StateError> {
    let absolute = state::paths::from_project_relative(project_root, relative_ref)?;
    state::store::read_json(&absolute)
}

fn read_technical_baseline(
    project_root: &Path,
    relative_ref: &str,
) -> Result<TechnicalBaselineContract, state::store::StateError> {
    let absolute = state::paths::from_project_relative(project_root, relative_ref)?;
    state::store::read_json(&absolute)
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}
