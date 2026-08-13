use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    BrainstormContract, ConceptPhaseRelevance, ConceptPriority, PlanningContractContextRefs,
    PlanningContractPhaseScope, PlanningContractSource, PlanningContractStatus,
    PlanningContractTechnicalBaseline, PlanningDeploymentRules, PlanningGenerationContract,
    PlanningHandoff, PlanningInputs, PlanningRules, ProjectKind, QualityGates,
    RequirementDetailItem, RequirementDetailsIndex, ScopeIsolationRules, ScopeItem,
    TechnicalBaselineContract, TechnicalBaselineStatus,
};
use delivery_core::{
    DeliveryLifecycleStatus, DomainDispatcher, LoomMcpActionResult, LoomMcpBlockedResult,
    LoomMcpFailure, LoomMcpFailureResult, RouteAction, RouteActionKind, TransitionStore,
};
use serde_json::json;
use state::{lifecycle_store::FileTransitionStore, paths::DeliveryPhaseLocator};

use crate::paths::{planning_contract_file, planning_latest_file};

pub fn create_contract_and_route<D>(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher + Clone,
{
    match create_contract_and_route_inner(project_root, delivery_id, phase_id, dispatcher) {
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

fn create_contract_and_route_inner<D>(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher + Clone,
{
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
    if !matches!(
        baseline.status,
        TechnicalBaselineStatus::AutoAccepted | TechnicalBaselineStatus::Confirmed
    ) {
        return Ok(LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: project_root.to_string(),
            blockers: vec![
                "TechnicalBaseline is not confirmed, so PlanningGenerationContract cannot be created.".to_string(),
            ],
            recommended_tool: Some("loom.continue".to_string()),
            details: Some(json!({
                "deliveryId": delivery_id,
                "phaseId": phase_id,
                "technicalBaselineRef": baseline_ref,
                "technicalBaselineStatus": baseline.status
            })),
        }));
    }
    if !matches!(
        brainstorm.security_requirement.applies,
        contracts::SecurityRequirementApplicability::NotApplicable
    ) && baseline.security_profiles.is_empty()
    {
        return Ok(LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: project_root.to_string(),
            blockers: vec![
                "The accepted security requirement is protected, but TechnicalBaseline has no complete security profile.".to_string(),
            ],
            recommended_tool: Some("loom.continue".to_string()),
            details: Some(json!({
                "deliveryId": delivery_id,
                "phaseId": phase_id,
                "securityRequirement": brainstorm.security_requirement,
                "technicalBaselineRef": baseline_ref
            })),
        }));
    }
    if matches!(baseline.project_kind, ProjectKind::ExistingProject)
        && repository_context_ref.is_none()
    {
        let action = RouteAction {
            kind: RouteActionKind::RepositoryContextRequest,
            source: "planning_contract".to_string(),
            reason: "repository_context_required_before_pgc".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        };
        phase.next_action = Some(action.clone());
        delivery.status = DeliveryLifecycleStatus::Planning;
        delivery.updated_at = state::store::now_string();
        store
            .commit_delivery_state(project_root, &delivery, &mut status)
            .map_err(to_state_error)?;
        return Ok(dispatcher.dispatch_route_action(project_root, delivery_id, phase_id, &action));
    }
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
        actors: brainstorm
            .domain_model
            .as_ref()
            .map(|model| values_from_iter(&model.actors))
            .unwrap_or_default(),
        capability_groups: brainstorm
            .domain_model
            .as_ref()
            .map(|model| values_from_iter(&model.capability_groups))
            .unwrap_or_default(),
        business_flows: brainstorm
            .domain_model
            .as_ref()
            .map(|model| values_from_iter(&model.business_flows))
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
            summary: json!({
                "stack": baseline.stack,
                "securityProfiles": baseline.security_profiles,
                "securityRequirement": brainstorm.security_requirement
            }),
            security_profiles: baseline.security_profiles.clone(),
            security_requirement: brainstorm.security_requirement.clone(),
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
        .commit_delivery_state(project_root, &delivery, &mut status)
        .map_err(to_state_error)?;
    Ok(dispatcher.dispatch_route_action(project_root, delivery_id, phase_id, &action))
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
    let mut extraction_warnings = Vec::new();
    let fallback_source_refs = fallback_source_refs(brainstorm, brainstorm_contract_ref);
    let current_scope_ids = phase_scope
        .included
        .iter()
        .map(|scope| scope.id.clone())
        .collect::<Vec<_>>();
    let deferred_scope_ids = phase_scope
        .deferred
        .iter()
        .map(|scope| scope.id.clone())
        .collect::<Vec<_>>();
    let excluded_scope_ids = phase_scope
        .excluded
        .iter()
        .map(|scope| scope.id.clone())
        .collect::<Vec<_>>();

    add_scope_details(
        &mut items,
        &mut extraction_warnings,
        &brainstorm.scope.included,
        &current_scope_ids,
        "included",
        true,
        "scope_boundary",
        &fallback_source_refs,
    );
    add_scope_details(
        &mut items,
        &mut extraction_warnings,
        &brainstorm.scope.deferred,
        &deferred_scope_ids,
        "deferred",
        false,
        "deferred_or_excluded_boundary",
        &fallback_source_refs,
    );
    add_scope_details(
        &mut items,
        &mut extraction_warnings,
        &brainstorm.scope.excluded,
        &excluded_scope_ids,
        "excluded",
        false,
        "deferred_or_excluded_boundary",
        &fallback_source_refs,
    );

    for (index, acceptance) in brainstorm
        .acceptance
        .iter()
        .enumerate()
        .filter(|(_, acceptance)| current_acceptance_ids.iter().any(|id| id == &acceptance.id))
    {
        push_detail(
            &mut items,
            RequirementDetailItem {
                detail_id: format!("detail.acceptance.{}", acceptance.id),
                kind: infer_requirement_detail_kind(&acceptance.statement, "acceptance_outcome"),
                title: acceptance.id.clone(),
                summary: acceptance.statement.clone(),
                required_for_current_phase: true,
                priority: format!("{:?}", acceptance.priority).to_lowercase(),
                source_field_refs: vec![format!("brainstorm.acceptance[{index}].statement")],
                source_refs: if acceptance.source_refs.is_empty() {
                    fallback_source_refs.clone()
                } else {
                    acceptance.source_refs.clone()
                },
                scope_refs: current_scope_ids.clone(),
                acceptance_refs: vec![acceptance.id.clone()],
                concept_refs: vec![],
                frontend_refs: vec![],
                impact_tags: infer_impact_tags(&acceptance.statement),
                lifecycle_stage: infer_lifecycle_stage(&acceptance.statement),
                quality: "confirmed".to_string(),
                unresolved_note: None,
            },
        );
    }

    if let Some(model) = &brainstorm.domain_model {
        for (index, flow) in model.business_flows.iter().enumerate() {
            let acceptance_refs = brainstorm
                .acceptance
                .iter()
                .filter(|acceptance| {
                    current_acceptance_ids.iter().any(|id| id == &acceptance.id)
                        && acceptance
                            .capability_refs
                            .iter()
                            .any(|capability_ref| flow.capability_refs.contains(capability_ref))
                })
                .map(|acceptance| acceptance.id.clone())
                .collect::<Vec<_>>();
            push_detail(
                &mut items,
                RequirementDetailItem {
                    detail_id: format!("detail.businessFlow.{}", flow.id),
                    kind: "business_flow".to_string(),
                    title: flow.name.clone(),
                    summary: flow.summary.clone(),
                    required_for_current_phase: true,
                    priority: "must".to_string(),
                    source_field_refs: vec![format!(
                        "brainstorm.domainModel.businessFlows[{index}].summary"
                    )],
                    source_refs: fallback_source_refs.clone(),
                    scope_refs: current_scope_ids.clone(),
                    acceptance_refs,
                    concept_refs: vec![],
                    frontend_refs: vec![],
                    impact_tags: infer_impact_tags(&flow.summary),
                    lifecycle_stage: infer_lifecycle_stage(&flow.summary),
                    quality: "confirmed".to_string(),
                    unresolved_note: None,
                },
            );
        }
    }

    if let Some(concept_grounding) = &brainstorm.concept_grounding {
        let concept_sets = concept_grounding
            .delivery_concept_glossary
            .iter()
            .map(|set| ("deliveryConceptGlossary", set))
            .chain(std::iter::once((
                "phaseConceptGrounding",
                &concept_grounding.phase_concept_grounding,
            )));
        for (set_path, concept_set) in concept_sets {
            for (index, concept) in concept_set.concepts.iter().enumerate() {
                let concept_scope_refs = concept
                    .scope_refs
                    .iter()
                    .filter(|scope_ref| current_scope_ids.iter().any(|id| id == *scope_ref))
                    .cloned()
                    .collect::<Vec<_>>();
                let concept_acceptance_refs = concept
                    .acceptance_refs
                    .iter()
                    .filter(|acceptance_ref| {
                        current_acceptance_ids
                            .iter()
                            .any(|id| id == *acceptance_ref)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let current_phase_relevant = matches!(
                    concept.phase_relevance,
                    ConceptPhaseRelevance::Current | ConceptPhaseRelevance::CurrentAdjacent
                ) || !concept_scope_refs.is_empty()
                    || !concept_acceptance_refs.is_empty();
                if !current_phase_relevant {
                    continue;
                }
                push_detail(
                    &mut items,
                    RequirementDetailItem {
                        detail_id: format!("detail.concept.{}", concept.concept_id),
                        kind: infer_requirement_detail_kind(
                            &concept.explanation,
                            "business_scenario",
                        ),
                        title: concept.term.clone(),
                        summary: concept.explanation.clone(),
                        required_for_current_phase: matches!(
                            concept.phase_relevance,
                            ConceptPhaseRelevance::Current
                        ),
                        priority: match concept.priority {
                            ConceptPriority::MustUnderstand => "must",
                            ConceptPriority::ShouldUnderstand => "should",
                            ConceptPriority::NiceToUnderstand => "could",
                        }
                        .to_string(),
                        source_field_refs: vec![format!(
                            "brainstorm.conceptGrounding.{set_path}.concepts[{index}].explanation"
                        )],
                        source_refs: fallback_source_refs.clone(),
                        scope_refs: concept_scope_refs,
                        acceptance_refs: concept_acceptance_refs,
                        concept_refs: vec![concept.concept_id.clone()],
                        frontend_refs: vec![],
                        impact_tags: infer_impact_tags(&concept.explanation),
                        lifecycle_stage: infer_lifecycle_stage(&concept.explanation),
                        quality: "confirmed".to_string(),
                        unresolved_note: None,
                    },
                );
            }
        }
    }

    if let Some(frontend) = &brainstorm.frontend_experience {
        for (index, view) in frontend.data_views.iter().enumerate() {
            let summary = format!(
                "{} Selection: {:?}. Pagination required: {}.",
                view.purpose, view.selection_mode, view.pagination_required
            );
            push_detail(
                &mut items,
                RequirementDetailItem {
                    detail_id: format!("detail.frontend.view.{}", view.view_id),
                    kind: "frontend_operation_path".to_string(),
                    title: view.name.clone(),
                    summary: summary.clone(),
                    required_for_current_phase: frontend.required,
                    priority: if frontend.required { "must" } else { "could" }.to_string(),
                    source_field_refs: vec![format!(
                        "brainstorm.frontendExperience.dataViews[{index}]"
                    )],
                    source_refs: if view.source_refs.is_empty() {
                        fallback_source_refs.clone()
                    } else {
                        view.source_refs.clone()
                    },
                    scope_refs: current_scope_ids.clone(),
                    acceptance_refs: vec![],
                    concept_refs: vec![],
                    frontend_refs: vec![view.view_id.clone()],
                    impact_tags: infer_impact_tags(&summary),
                    lifecycle_stage: "frontend".to_string(),
                    quality: "confirmed".to_string(),
                    unresolved_note: None,
                },
            );
        }
        for (index, action) in frontend.actions.iter().enumerate() {
            let summary = format!(
                "{}. Entry: {:?}. Refresh: {}. Success feedback: {}. Blocking or error feedback: {}.",
                action.label,
                action.entry_point,
                action.refresh_policy,
                action.success_feedback.join("; "),
                action.blocking_or_error_feedback.join("; ")
            );
            push_detail(
                &mut items,
                RequirementDetailItem {
                    detail_id: format!("detail.frontend.action.{}", action.action_id),
                    kind: "frontend_operation_path".to_string(),
                    title: action.label.clone(),
                    summary: summary.clone(),
                    required_for_current_phase: frontend.required,
                    priority: if frontend.required { "must" } else { "could" }.to_string(),
                    source_field_refs: vec![format!(
                        "brainstorm.frontendExperience.actions[{index}]"
                    )],
                    source_refs: if action.source_refs.is_empty() {
                        fallback_source_refs.clone()
                    } else {
                        action.source_refs.clone()
                    },
                    scope_refs: current_scope_ids.clone(),
                    acceptance_refs: vec![],
                    concept_refs: vec![],
                    frontend_refs: vec![action.action_id.clone()],
                    impact_tags: infer_impact_tags(&summary),
                    lifecycle_stage: "frontend".to_string(),
                    quality: "confirmed".to_string(),
                    unresolved_note: None,
                },
            );
        }
        for (index, path) in frontend.operation_paths.iter().enumerate() {
            push_detail(
                &mut items,
                RequirementDetailItem {
                    detail_id: format!("detail.frontend.{}", path.path_id),
                    kind: "frontend_operation_path".to_string(),
                    title: path.name.clone(),
                    summary: path.selection_summary.clone(),
                    required_for_current_phase: frontend.required,
                    priority: if frontend.required { "must" } else { "could" }.to_string(),
                    source_field_refs: vec![format!(
                        "brainstorm.frontendExperience.operationPaths[{index}]"
                    )],
                    source_refs: if path.source_refs.is_empty() {
                        fallback_source_refs.clone()
                    } else {
                        path.source_refs.clone()
                    },
                    scope_refs: current_scope_ids.clone(),
                    acceptance_refs: vec![],
                    concept_refs: vec![],
                    frontend_refs: std::iter::once(path.path_id.clone())
                        .chain(path.data_view_refs.iter().cloned())
                        .chain(path.action_refs.iter().cloned())
                        .collect(),
                    impact_tags: infer_impact_tags(&path.selection_summary),
                    lifecycle_stage: "frontend".to_string(),
                    quality: "confirmed".to_string(),
                    unresolved_note: None,
                },
            );
        }
    }

    for (index, assumption) in brainstorm.scope.assumptions.iter().enumerate() {
        push_detail(
            &mut items,
            RequirementDetailItem {
                detail_id: format!("detail.assumption.{}", assumption.id),
                kind: "assumption".to_string(),
                title: assumption.id.clone(),
                summary: assumption.text.clone(),
                required_for_current_phase: !assumption.requires_confirmation,
                priority: if assumption.requires_confirmation {
                    "should"
                } else {
                    "could"
                }
                .to_string(),
                source_field_refs: vec![format!("brainstorm.scope.assumptions[{index}].text")],
                source_refs: fallback_source_refs.clone(),
                scope_refs: vec![],
                acceptance_refs: vec![],
                concept_refs: vec![],
                frontend_refs: vec![],
                impact_tags: infer_impact_tags(&assumption.text),
                lifecycle_stage: infer_lifecycle_stage(&assumption.text),
                quality: "confirmed".to_string(),
                unresolved_note: if assumption.requires_confirmation {
                    Some("Assumption still requires confirmation.".to_string())
                } else {
                    None
                },
            },
        );
    }
    for item in &items {
        if item.quality == "thin" {
            extraction_warnings.push(contracts::ContextWarning {
                code: "REQUIREMENT_DETAIL_THIN".to_string(),
                message: format!(
                    "Requirement detail {} is thin; downstream stages must not invent missing business rules.",
                    item.detail_id
                ),
            });
        }
    }

    RequirementDetailsIndex {
        schema_version: "1.0".to_string(),
        authority: "brainstorm_contract".to_string(),
        source_brainstorm_contract_ref: brainstorm_contract_ref.to_string(),
        items,
        extraction_warnings,
    }
}

fn add_scope_details(
    items: &mut Vec<RequirementDetailItem>,
    extraction_warnings: &mut Vec<contracts::ContextWarning>,
    source_items: &[ScopeItem],
    active_scope_ids: &[String],
    bucket: &str,
    required_for_current_phase: bool,
    kind: &str,
    fallback_source_refs: &[String],
) {
    for (scope_index, scope) in source_items.iter().enumerate() {
        if !active_scope_ids.iter().any(|id| id == &scope.id) {
            continue;
        }
        let values = if scope.items.is_empty() {
            vec![scope.label.clone()]
        } else {
            scope.items.clone()
        };
        for (item_index, detail) in values.iter().enumerate() {
            let detail_bucket = if bucket == "included" {
                "scope"
            } else {
                bucket
            };
            let source_field_ref = if scope.items.is_empty() {
                format!("brainstorm.scope.{bucket}[{scope_index}].label")
            } else {
                format!("brainstorm.scope.{bucket}[{scope_index}].items[{item_index}]")
            };
            push_detail(
                items,
                RequirementDetailItem {
                    detail_id: format!("detail.{detail_bucket}.{}.{}", scope.id, item_index + 1),
                    kind: kind.to_string(),
                    title: truncate_detail_title(&format!("{}: {}", scope.label, detail)),
                    summary: detail.clone(),
                    required_for_current_phase,
                    priority: if required_for_current_phase {
                        "must"
                    } else {
                        "could"
                    }
                    .to_string(),
                    source_field_refs: vec![source_field_ref],
                    source_refs: fallback_source_refs.to_vec(),
                    scope_refs: vec![scope.id.clone()],
                    acceptance_refs: vec![],
                    concept_refs: vec![],
                    frontend_refs: vec![],
                    impact_tags: vec![],
                    lifecycle_stage: String::new(),
                    quality: String::new(),
                    unresolved_note: if scope.items.is_empty() {
                        Some("Scope item has no detailed items array; label was used as the detail source.".to_string())
                    } else {
                        None
                    },
                },
            );
        }
        if scope.items.is_empty() {
            extraction_warnings.push(contracts::ContextWarning {
                code: "SCOPE_ITEMS_EMPTY".to_string(),
                message: format!(
                    "Scope {} has no items array, so PGC could only index the scope label.",
                    scope.id
                ),
            });
        }
    }
}

fn values_from_iter<T: serde::Serialize>(items: &[T]) -> Vec<serde_json::Value> {
    items
        .iter()
        .filter_map(|item| serde_json::to_value(item).ok())
        .collect()
}

fn fallback_source_refs(
    brainstorm: &BrainstormContract,
    brainstorm_contract_ref: &str,
) -> Vec<String> {
    let source_refs = brainstorm
        .sources
        .iter()
        .map(|source| source.source_id.clone())
        .filter(|source_id| !source_id.trim().is_empty())
        .collect::<Vec<_>>();
    if source_refs.is_empty() {
        vec![brainstorm_contract_ref.to_string()]
    } else {
        source_refs
    }
}

fn push_detail(items: &mut Vec<RequirementDetailItem>, mut detail: RequirementDetailItem) {
    if detail.summary.trim().is_empty()
        || items.iter().any(|item| item.detail_id == detail.detail_id)
    {
        return;
    }
    detail.title = truncate_detail_title(&detail.title);
    detail.source_field_refs = unique_strings(detail.source_field_refs);
    detail.source_refs = unique_strings(detail.source_refs);
    detail.scope_refs = unique_strings(detail.scope_refs);
    detail.acceptance_refs = unique_strings(detail.acceptance_refs);
    detail.concept_refs = unique_strings(detail.concept_refs);
    detail.frontend_refs = unique_strings(detail.frontend_refs);
    detail.impact_tags = infer_requirement_detail_impact_tags(&detail.summary, &detail.kind);
    detail.lifecycle_stage = infer_lifecycle_stage(&detail.summary);
    detail.quality = infer_requirement_detail_quality(&detail.summary);
    if detail.unresolved_note.is_none() {
        detail.unresolved_note = infer_requirement_detail_unresolved_note(&detail.summary);
    }
    items.push(detail);
}

fn infer_requirement_detail_kind(value: &str, fallback: &str) -> String {
    let text = value.to_lowercase();
    if matches_any(
        &text,
        &[
            "frontend", "ui", "page", "screen", "list", "query", "select", "refresh", "页面",
            "列表", "查询", "选择", "刷新", "反馈",
        ],
    ) {
        return "frontend_operation_path".to_string();
    }
    if matches_any(
        &text,
        &[
            "block", "blocking", "deny", "reject", "error", "invalid", "阻断", "拒绝", "失败",
            "无效", "原因",
        ],
    ) {
        return "blocking_rule".to_string();
    }
    if matches_any(
        &text,
        &[
            "validate",
            "validation",
            "required",
            "format",
            "校验",
            "必填",
            "格式",
            "规则",
        ],
    ) {
        return "validation_rule".to_string();
    }
    if matches_any(
        &text,
        &["state", "status", "transition", "状态", "变更", "流转"],
    ) {
        return "state_transition".to_string();
    }
    if matches_any(
        &text,
        &[
            "field",
            "input",
            "display",
            "relationship",
            "字段",
            "录入",
            "展示",
            "关系",
        ],
    ) {
        return "object_field_set".to_string();
    }
    if matches_any(
        &text,
        &[
            "create",
            "update",
            "approve",
            "cancel",
            "close",
            "submit",
            "operation",
            "action",
            "创建",
            "修改",
            "审批",
            "提交",
            "销户",
            "操作",
            "办理",
        ],
    ) {
        return "object_operation".to_string();
    }
    if matches_any(&text, &["flow", "workflow", "process", "流程", "步骤"]) {
        return "business_flow".to_string();
    }
    fallback.to_string()
}

fn infer_impact_tags(value: &str) -> Vec<String> {
    let kind = infer_requirement_detail_kind(value, "business_scenario");
    infer_requirement_detail_impact_tags(value, &kind)
}

fn infer_requirement_detail_impact_tags(value: &str, kind: &str) -> Vec<String> {
    let text = value.to_lowercase();
    let mut tags = BTreeSet::new();
    if kind == "scope_boundary"
        || kind == "deferred_or_excluded_boundary"
        || matches_any(&text, &["scope", "phase", "范围", "阶段", "边界"])
    {
        tags.insert("scope".to_string());
    }
    if kind == "object_field_set"
        || matches_any(
            &text,
            &[
                "field", "data", "entity", "model", "字段", "数据", "模型", "关系",
            ],
        )
    {
        tags.insert("data_model".to_string());
    }
    if matches_any(
        &text,
        &[
            "flow",
            "workflow",
            "process",
            "operation",
            "state",
            "status",
            "transition",
            "流程",
            "操作",
            "状态",
        ],
    ) || matches!(
        kind,
        "business_flow" | "object_operation" | "state_transition"
    ) {
        tags.insert("business_flow".to_string());
    }
    if kind == "frontend_operation_path"
        || matches_any(
            &text,
            &[
                "frontend", "ui", "page", "list", "query", "页面", "列表", "查询", "反馈",
            ],
        )
    {
        tags.insert("frontend".to_string());
    }
    if matches_any(
        &text,
        &[
            "api",
            "interface",
            "request",
            "response",
            "http",
            "接口",
            "请求",
            "响应",
        ],
    ) {
        tags.insert("interface".to_string());
    }
    if matches_any(
        &text,
        &["acceptance", "verify", "success", "验收", "验证", "成功"],
    ) || kind == "acceptance_outcome"
    {
        tags.insert("acceptance".to_string());
    }
    if matches_any(
        &text,
        &[
            "runtime", "build", "start", "deploy", "运行", "构建", "启动", "部署",
        ],
    ) {
        tags.insert("runtime".to_string());
    }
    if tags.is_empty() {
        tags.insert("scope".to_string());
    }
    tags.into_iter().collect()
}

fn infer_lifecycle_stage(value: &str) -> String {
    let text = value.to_lowercase();
    if matches_any(
        &text,
        &[
            "create", "open", "apply", "submit", "创建", "开户", "申请", "提交", "新增",
        ],
    ) {
        "create".to_string()
    } else if matches_any(
        &text,
        &[
            "query", "search", "select", "list", "lookup", "查询", "搜索", "选择", "列表",
        ],
    ) {
        "query_select".to_string()
    } else if matches_any(
        &text,
        &["view", "detail", "display", "查看", "详情", "展示"],
    ) {
        "view".to_string()
    } else if matches_any(&text, &["update", "edit", "change", "修改", "更新", "变更"]) {
        "update".to_string()
    } else if matches_any(
        &text,
        &["approve", "review", "process", "审批", "审核", "处理"],
    ) {
        "approve_or_process".to_string()
    } else if matches_any(&text, &["state", "status", "transition", "状态", "流转"]) {
        "state_change".to_string()
    } else if matches_any(
        &text,
        &[
            "terminate",
            "cancel",
            "close",
            "delete",
            "撤销",
            "取消",
            "销户",
            "删除",
            "终止",
        ],
    ) {
        "terminate_or_cancel".to_string()
    } else if matches_any(
        &text,
        &[
            "block", "reject", "invalid", "error", "阻断", "拒绝", "失败", "异常",
        ],
    ) {
        "blocking_or_exception".to_string()
    } else {
        "not_applicable".to_string()
    }
}

fn matches_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn infer_requirement_detail_quality(value: &str) -> String {
    let text = value.trim();
    if text.chars().count() < 36 {
        return "thin".to_string();
    }
    let lower = text.to_lowercase();
    let detail_markers = [
        "field",
        "input",
        "precondition",
        "validation",
        "blocking",
        "reason",
        "state",
        "feedback",
        "refresh",
        "字段",
        "录入",
        "前置",
        "校验",
        "阻断",
        "原因",
        "状态",
        "反馈",
        "刷新",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count();
    if text.chars().count() >= 160 || detail_markers >= 3 {
        "rich".to_string()
    } else {
        "usable".to_string()
    }
}

fn infer_requirement_detail_unresolved_note(value: &str) -> Option<String> {
    let text = value.to_lowercase();
    matches_any(
        &text,
        &[
            "unclear",
            "unknown",
            "tbd",
            "to be confirmed",
            "未确认",
            "不明确",
            "待确认",
        ],
    )
    .then(|| "Detail source contains unresolved wording.".to_string())
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for value in values {
        if value.trim().is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        unique.push(value);
    }
    unique
}

fn truncate_detail_title(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= 96 {
        return trimmed.to_string();
    }
    let mut output = trimmed.chars().take(93).collect::<String>();
    output.push_str("...");
    output
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
