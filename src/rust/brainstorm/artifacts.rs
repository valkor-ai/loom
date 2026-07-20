use std::{collections::BTreeMap, path::Path};

use contracts::{
    BrainstormCandidateAgentWritable, BrainstormContract, BrainstormHandoff, BrainstormHandoffNode,
    BrainstormStatus, ConceptGroundingRefs, DeliveryContext, FrontendExperienceRefs,
    OriginalRequestContext, RequirementSource,
};
use delivery_core::RouteActionKind;
use state::{
    paths::to_project_relative,
    store::{write_json_atomic, StateResult},
};

use crate::paths::{
    brainstorm_contract_file, brainstorm_decision_snapshot_file, brainstorm_decisions_index_file,
    brainstorm_delivery_glossary_file, brainstorm_latest_file, brainstorm_phase_concept_file,
};

#[derive(Debug, Clone)]
pub struct PersistedArtifacts {
    pub contract_ref: String,
    pub decision_snapshot_ref: String,
    pub concept_grounding_refs: Option<ConceptGroundingRefs>,
    pub frontend_experience_refs: Option<FrontendExperienceRefs>,
}

pub fn write_accepted_artifacts(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
    brainstorm_run_id: &str,
    contract_id: &str,
    candidate: &BrainstormCandidateAgentWritable,
    original_request_text: &str,
    requirement_input_refs: &[String],
    formal_sources: &[RequirementSource],
    user_facing_language: contracts::UserFacingLanguageConstraint,
    next_action_kind: Option<RouteActionKind>,
    now: &str,
) -> StateResult<PersistedArtifacts> {
    let glossary_ref = if let Some(concept) = &candidate.concept_grounding {
        if let Some(glossary) = &concept.delivery_concept_glossary {
            let file = brainstorm_delivery_glossary_file(project_root, delivery_id);
            write_json_atomic(
                &file,
                &serde_json::json!({
                    "schemaVersion": "1.0",
                    "deliveryId": delivery_id,
                    "updatedAt": now,
                    "conceptSet": glossary
                }),
            )?;
            Some(to_project_relative(project_root, &file)?)
        } else {
            None
        }
    } else {
        None
    };

    let phase_concept_ref = if let Some(concept) = &candidate.concept_grounding {
        let file = brainstorm_phase_concept_file(project_root, delivery_id, phase_id);
        write_json_atomic(
            &file,
            &serde_json::json!({
                "schemaVersion": "1.0",
                "deliveryId": delivery_id,
                "phaseId": phase_id,
                "updatedAt": now,
                "conceptSet": concept.phase_concept_grounding
            }),
        )?;
        Some(to_project_relative(project_root, &file)?)
    } else {
        None
    };

    let contract_file = brainstorm_contract_file(project_root, delivery_id);
    let contract_ref = to_project_relative(project_root, &contract_file)?;
    let frontend_ref = candidate
        .frontend_experience
        .as_ref()
        .map(|_| format!("{contract_ref}#/frontendExperience"));
    let frontend_refs = frontend_ref
        .clone()
        .map(|reference| FrontendExperienceRefs {
            confirmed_frontend_experience_ref: Some(reference.clone()),
            current_frontend_experience_ref: Some(reference),
        });

    let contract = BrainstormContract {
        schema_version: "1.0".to_string(),
        contract_id: contract_id.to_string(),
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
        brainstorm_run_id: brainstorm_run_id.to_string(),
        status: BrainstormStatus::Confirmed,
        sources: formal_sources.to_vec(),
        summary: candidate.request_summary.clone(),
        scope: candidate.scope.clone(),
        acceptance: candidate.acceptance.clone(),
        domain_model: candidate.domain_model.clone(),
        user_confirmation: candidate.user_confirmation.clone(),
        delivery_context: DeliveryContext {
            original_request: OriginalRequestContext {
                text: original_request_text.to_string(),
                input_refs: requirement_input_refs.to_vec(),
            },
            user_facing_language,
        },
        roadmap: candidate.roadmap.clone(),
        phase_plan: candidate.phase_plan.clone(),
        concept_grounding: candidate.concept_grounding.clone(),
        concept_confirmation: candidate.concept_confirmation.clone(),
        clarification_progress: candidate.clarification_progress.clone(),
        concept_grounding_refs: phase_concept_ref
            .clone()
            .map(|phase_ref| ConceptGroundingRefs {
                delivery_concept_glossary_ref: glossary_ref.clone(),
                phase_concept_grounding_ref: phase_ref,
            }),
        frontend_experience: candidate.frontend_experience.clone(),
        frontend_experience_refs: frontend_refs.clone(),
        handoff: BrainstormHandoff {
            ready: true,
            next_node: brainstorm_handoff_node(next_action_kind),
            blocking_reasons: vec![],
        },
        created_at: now.to_string(),
        updated_at: now.to_string(),
    };
    write_json_atomic(&contract_file, &contract)?;

    let decision_snapshot_file =
        brainstorm_decision_snapshot_file(project_root, delivery_id, phase_id);
    write_json_atomic(
        &decision_snapshot_file,
        &serde_json::json!({
            "schemaVersion": "1.0",
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "brainstormRunId": brainstorm_run_id,
            "acceptedAt": now,
            "contractRef": contract_ref,
            "summary": contract.summary,
            "scope": contract.scope,
            "acceptance": contract.acceptance,
            "domainModel": contract.domain_model,
            "conceptGrounding": contract.concept_grounding,
            "conceptConfirmation": contract.concept_confirmation,
            "clarificationProgress": contract.clarification_progress,
            "phasePlan": contract.phase_plan,
            "userConfirmation": contract.user_confirmation
        }),
    )?;
    let decision_snapshot_ref = to_project_relative(project_root, &decision_snapshot_file)?;

    let decisions_index_file = brainstorm_decisions_index_file(project_root, delivery_id);
    let mut decisions = vec![serde_json::json!({
        "phaseId": phase_id,
        "decisionRef": decision_snapshot_ref,
        "brainstormRunId": brainstorm_run_id,
        "acceptedAt": now,
        "title": candidate.phase_plan.current.title,
        "goal": candidate.phase_plan.current.goal,
        "scopeLabels": candidate.scope.included.iter().map(|item| item.label.clone()).collect::<Vec<_>>(),
        "acceptanceStatements": candidate.acceptance.iter().map(|item| item.statement.clone()).collect::<Vec<_>>(),
        "nextPhasePreview": candidate.phase_plan.next_phase_preview
    })];
    if decisions_index_file.exists() {
        if let Ok(existing) = std::fs::read_to_string(&decisions_index_file) {
            if let Ok(existing_json) = serde_json::from_str::<serde_json::Value>(&existing) {
                if let Some(items) = existing_json
                    .get("decisions")
                    .and_then(serde_json::Value::as_array)
                {
                    decisions.extend(
                        items
                            .iter()
                            .filter(|item| {
                                item.get("phaseId").and_then(serde_json::Value::as_str)
                                    != Some(phase_id)
                            })
                            .cloned(),
                    );
                }
            }
        }
    }
    write_json_atomic(
        &decisions_index_file,
        &serde_json::json!({
            "schemaVersion": "1.0",
            "deliveryId": delivery_id,
            "latestConfirmedPhaseId": phase_id,
            "updatedAt": now,
            "decisions": decisions
        }),
    )?;

    let latest_file = brainstorm_latest_file(project_root, delivery_id);
    let latest_refs = BTreeMap::from([
        ("contractRef".to_string(), contract_ref.clone()),
        (
            "decisionSnapshotRef".to_string(),
            decision_snapshot_ref.clone(),
        ),
    ]);
    write_json_atomic(
        &latest_file,
        &serde_json::json!({
            "schemaVersion": "1.0",
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "updatedAt": now,
            "refs": latest_refs
        }),
    )?;

    Ok(PersistedArtifacts {
        contract_ref,
        decision_snapshot_ref,
        concept_grounding_refs: phase_concept_ref.map(|phase_ref| ConceptGroundingRefs {
            delivery_concept_glossary_ref: glossary_ref,
            phase_concept_grounding_ref: phase_ref,
        }),
        frontend_experience_refs: frontend_refs,
    })
}

fn brainstorm_handoff_node(next_action_kind: Option<RouteActionKind>) -> BrainstormHandoffNode {
    match next_action_kind {
        Some(RouteActionKind::PlanningContractCreate) => {
            BrainstormHandoffNode::PlanningGenerationContract
        }
        Some(RouteActionKind::BrainstormClarification) => {
            BrainstormHandoffNode::BrainstormClarification
        }
        _ => BrainstormHandoffNode::TechnicalBaselineGeneration,
    }
}
