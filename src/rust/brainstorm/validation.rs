use std::collections::{BTreeMap, BTreeSet};

use contracts::{
    BrainstormCandidateAgentWritable, ClarificationBlockName, ClarificationProgress,
    FrontendExperience, GlossaryUpdateOperation, NextPhasePreview,
};
use delivery_core::RepairIssue;
use serde_json::Value;

use crate::gate::{block_message, BrainstormGate, BrainstormResponseRule, SkippedBlockSummary};

pub struct GateCheck {
    pub gate: Option<BrainstormGate>,
    pub repair_issues: Vec<RepairIssue>,
}

pub fn gate_check(raw: &Value) -> GateCheck {
    let Some(object) = raw.as_object() else {
        return GateCheck {
            gate: None,
            repair_issues: vec![issue(
                "BRAINSTORM_CANDIDATE_ROOT_INVALID",
                "candidate",
                "Brainstorm candidate JSON root must be an object.",
            )],
        };
    };
    let Some(progress_value) = object.get("clarificationProgress").cloned() else {
        return GateCheck {
            gate: Some(missing_block_gate(
                ClarificationBlockName::PhaseScope,
                vec!["clarificationProgress is missing. Continue the Brainstorm conversation before submit.".to_string()],
            )),
            repair_issues: vec![],
        };
    };
    if let Some(progress_object) = progress_value.as_object() {
        let forbidden_fields = ["completedBlocks", "currentBlock"];
        let present_forbidden = forbidden_fields
            .iter()
            .filter(|field| progress_object.contains_key(**field))
            .copied()
            .collect::<Vec<_>>();
        if !present_forbidden.is_empty() {
            return GateCheck {
                gate: None,
                repair_issues: vec![issue(
                    "CLARIFICATION_PROGRESS_LEGACY_FIELDS",
                    "clarificationProgress",
                    &format!(
                        "clarificationProgress must use confirmedBlocks/skippedBlocks/finalSummaryConfirmed. Remove unsupported fields: {}.",
                        present_forbidden.join(", ")
                    ),
                )],
            };
        }
        let final_summary_confirmed = progress_object
            .get("finalSummaryConfirmed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if final_summary_confirmed && !progress_object.contains_key("confirmedBlocks") {
            return GateCheck {
                gate: None,
                repair_issues: vec![issue(
                    "CLARIFICATION_PROGRESS_CONFIRMED_BLOCKS_REQUIRED",
                    "clarificationProgress.confirmedBlocks",
                    "clarificationProgress.confirmedBlocks must list each user-confirmed block when finalSummaryConfirmed is true.",
                )],
            };
        }
    }
    let progress = match serde_json::from_value::<ClarificationProgress>(progress_value) {
        Ok(progress) => progress,
        Err(error) => {
            return GateCheck {
                gate: None,
                repair_issues: vec![issue(
                    "CLARIFICATION_PROGRESS_INVALID",
                    "clarificationProgress",
                    &format!("clarificationProgress has an invalid shape: {error}"),
                )],
            };
        }
    };

    let confirmed = progress
        .confirmed_blocks
        .iter()
        .filter(|block| block.confirmed_by_user)
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();
    let skipped = progress
        .skipped_blocks
        .iter()
        .map(|block| SkippedBlockSummary {
            block: block.block.clone(),
            reason: block.reason.clone(),
        })
        .collect::<Vec<_>>();
    let skipped_map = progress
        .skipped_blocks
        .iter()
        .map(|block| (block.block.clone(), block.reason.clone()))
        .collect::<BTreeMap<_, _>>();
    let confirmed_set = confirmed.iter().cloned().collect::<BTreeSet<_>>();

    for block in [
        ClarificationBlockName::PhaseScope,
        ClarificationBlockName::ConceptGrounding,
    ] {
        if skipped_map.contains_key(&block) {
            return GateCheck {
                gate: Some(BrainstormGate {
                    gate_id: format!("gate_{block:?}").to_ascii_lowercase(),
                    current_block: block.clone(),
                    required_blocks: crate::gate::required_blocks(),
                    already_confirmed_blocks: confirmed.clone(),
                    skipped_blocks: skipped.clone(),
                    user_message: block_message(&block),
                    response_rule: BrainstormResponseRule {
                        mode: "progressive_brainstorm".to_string(),
                        final_summary_required_before_write: true,
                        user_visible_confirmation_required: true,
                    },
                    issues: vec![format!(
                        "{block:?} cannot be skipped. It must be user-confirmed before submit."
                    )],
                }),
                repair_issues: vec![],
            };
        }
    }

    for block in [
        ClarificationBlockName::PhaseScope,
        ClarificationBlockName::ConceptGrounding,
        ClarificationBlockName::FrontendExperience,
    ] {
        if confirmed_set.contains(&block) || skipped_map.contains_key(&block) {
            continue;
        }
        return GateCheck {
            gate: Some(BrainstormGate {
                gate_id: format!("gate_{block:?}").to_ascii_lowercase(),
                current_block: block.clone(),
                required_blocks: crate::gate::required_blocks(),
                already_confirmed_blocks: confirmed.clone(),
                skipped_blocks: skipped.clone(),
                user_message: block_message(&block),
                response_rule: BrainstormResponseRule {
                    mode: "progressive_brainstorm".to_string(),
                    final_summary_required_before_write: true,
                    user_visible_confirmation_required: true,
                },
                issues: vec![format!("{block:?} is not confirmed yet.")],
            }),
            repair_issues: vec![],
        };
    }

    if !progress.final_summary_confirmed {
        return GateCheck {
            gate: Some(BrainstormGate {
                gate_id: "gate_final_summary".to_string(),
                current_block: ClarificationBlockName::FinalSummary,
                required_blocks: crate::gate::required_blocks(),
                already_confirmed_blocks: confirmed,
                skipped_blocks: skipped,
                user_message: block_message(&ClarificationBlockName::FinalSummary),
                response_rule: BrainstormResponseRule {
                    mode: "progressive_brainstorm".to_string(),
                    final_summary_required_before_write: true,
                    user_visible_confirmation_required: true,
                },
                issues: vec!["final_summary is not confirmed yet.".to_string()],
            }),
            repair_issues: vec![],
        };
    }

    let basis = raw
        .pointer("/userConfirmation/confirmationBasis")
        .and_then(Value::as_object);
    let summary_presented = basis
        .and_then(|value| value.get("summaryPresentedToUser"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let confirmed_after_summary = basis
        .and_then(|value| value.get("confirmedAfterSummary"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let initial_request_only = basis
        .and_then(|value| value.get("initialRequestOnly"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let user_confirmed = raw
        .pointer("/userConfirmation/confirmed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !summary_presented || !confirmed_after_summary || initial_request_only || !user_confirmed {
        return GateCheck {
            gate: Some(BrainstormGate {
                gate_id: "gate_final_summary".to_string(),
                current_block: ClarificationBlockName::FinalSummary,
                required_blocks: crate::gate::required_blocks(),
                already_confirmed_blocks: confirmed,
                skipped_blocks: skipped,
                user_message: block_message(&ClarificationBlockName::FinalSummary),
                response_rule: BrainstormResponseRule {
                    mode: "progressive_brainstorm".to_string(),
                    final_summary_required_before_write: true,
                    user_visible_confirmation_required: true,
                },
                issues: vec![
                    "userConfirmation.confirmationBasis must prove the summary was shown and confirmed after presentation.".to_string(),
                ],
            }),
            repair_issues: vec![],
        };
    }

    GateCheck {
        gate: None,
        repair_issues: vec![],
    }
}

pub fn validate_candidate(
    candidate: &BrainstormCandidateAgentWritable,
    phase_id: &str,
    request_source_ids: &BTreeSet<String>,
) -> Vec<RepairIssue> {
    let mut issues = Vec::new();

    let scope_ids = candidate
        .scope
        .included
        .iter()
        .map(|item| item.id.clone())
        .collect::<BTreeSet<_>>();
    for scope_ref in &candidate.phase_plan.current.scope_refs {
        if !scope_ids.contains(scope_ref) {
            issues.push(issue(
                "PHASE_PLAN_SCOPE_REF_INVALID",
                "phasePlan.current.scopeRefs",
                "phasePlan.current.scopeRefs must reference scope.included ids.",
            ));
            break;
        }
    }
    let acceptance_ids = candidate
        .acceptance
        .iter()
        .map(|item| item.id.clone())
        .collect::<BTreeSet<_>>();
    for acceptance_ref in &candidate.phase_plan.current.acceptance_refs {
        if !acceptance_ids.contains(acceptance_ref) {
            issues.push(issue(
                "PHASE_PLAN_ACCEPTANCE_REF_INVALID",
                "phasePlan.current.acceptanceRefs",
                "phasePlan.current.acceptanceRefs must reference acceptance ids.",
            ));
            break;
        }
    }
    let current_phase = candidate
        .roadmap
        .phases
        .iter()
        .find(|phase| phase.phase_id == candidate.roadmap.current_phase_id);
    if current_phase.is_none() {
        issues.push(issue(
            "CURRENT_PHASE_MISSING",
            "roadmap.phases",
            "roadmap.phases must contain roadmap.currentPhaseId.",
        ));
    }
    if !candidate
        .roadmap
        .phases
        .iter()
        .any(|phase| phase.phase_id == phase_id)
    {
        issues.push(issue(
            "ACTIVE_PHASE_MISSING",
            "roadmap.phases",
            "roadmap.phases must include the active phase.",
        ));
    }
    if !candidate.scope.deferred.is_empty() {
        if matches!(
            candidate.phase_plan.next_phase_preview,
            NextPhasePreview::None { .. }
        ) {
            issues.push(issue(
                "NEXT_PHASE_PREVIEW_REQUIRED",
                "phasePlan.nextPhasePreview",
                "Deferred scope requires nextPhasePreview.kind=candidate.",
            ));
        }
    }
    if let Some(progress) = &candidate.clarification_progress {
        let frontend_confirmed = progress
            .confirmed_blocks
            .iter()
            .any(|block| block.block == ClarificationBlockName::FrontendExperience);
        let frontend_skipped = progress
            .skipped_blocks
            .iter()
            .any(|block| block.block == ClarificationBlockName::FrontendExperience);
        if frontend_confirmed && candidate.frontend_experience.is_none() {
            issues.push(issue(
                "FRONTEND_TARGET_MISSING",
                "frontendExperience",
                "A confirmed frontend_experience block requires frontendExperience.",
            ));
        }
        if !frontend_confirmed && !frontend_skipped {
            issues.push(issue(
                "FRONTEND_BLOCK_UNRESOLVED",
                "clarificationProgress",
                "frontend_experience must be confirmed or explicitly skipped.",
            ));
        }
    }
    if candidate.concept_grounding.is_none() {
        issues.push(issue(
            "CONCEPT_GROUNDING_MISSING",
            "conceptGrounding",
            "conceptGrounding is required after concept_grounding confirmation.",
        ));
    }
    if candidate.concept_confirmation.is_none() {
        issues.push(issue(
            "CONCEPT_CONFIRMATION_MISSING",
            "conceptConfirmation",
            "conceptConfirmation is required after concept_grounding confirmation.",
        ));
    }
    validate_glossary_updates(candidate, &mut issues);
    if let Some(frontend) = &candidate.frontend_experience {
        validate_frontend_source_refs(frontend, request_source_ids, &mut issues);
    }
    validate_nested_source_refs(candidate, request_source_ids, &mut issues);
    issues
}

fn validate_glossary_updates(
    candidate: &BrainstormCandidateAgentWritable,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(grounding) = &candidate.concept_grounding else {
        return;
    };

    for (index, update) in grounding.glossary_updates.iter().enumerate() {
        let path = format!("conceptGrounding.glossaryUpdates[{index}]");
        if update.reason.trim().is_empty() {
            issues.push(issue(
                "GLOSSARY_UPDATE_REASON_REQUIRED",
                &format!("{path}.reason"),
                "A glossary update must explain why the delivery glossary changes.",
            ));
        }
        match update.operation {
            GlossaryUpdateOperation::Add if update.concept.is_none() => issues.push(issue(
                "GLOSSARY_UPDATE_CONCEPT_REQUIRED",
                &format!("{path}.concept"),
                "An add glossary update requires a concept object using the phase concept shape.",
            )),
            GlossaryUpdateOperation::Replace
                if update
                    .concept_ref
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                    || update.concept.is_none() =>
            {
                issues.push(issue(
                    "GLOSSARY_UPDATE_REPLACE_PAYLOAD_REQUIRED",
                    &path,
                    "A replace glossary update requires a non-empty conceptRef and a replacement concept object.",
                ));
            }
            GlossaryUpdateOperation::Remove
                if update
                    .concept_ref
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty()) =>
            {
                issues.push(issue(
                    "GLOSSARY_UPDATE_CONCEPT_REF_REQUIRED",
                    &format!("{path}.conceptRef"),
                    "A remove glossary update requires a non-empty conceptRef.",
                ));
            }
            GlossaryUpdateOperation::Remove if update.concept.is_some() => issues.push(issue(
                "GLOSSARY_UPDATE_REMOVE_CONCEPT_FORBIDDEN",
                &format!("{path}.concept"),
                "A remove glossary update must not include a replacement concept.",
            )),
            _ => {}
        }
    }
}

fn validate_frontend_source_refs(
    frontend: &FrontendExperience,
    source_ids: &BTreeSet<String>,
    issues: &mut Vec<RepairIssue>,
) {
    for view in &frontend.data_views {
        for source_ref in &view.source_refs {
            validate_source_ref(
                source_ref,
                "frontendExperience.dataViews.sourceRefs",
                source_ids,
                issues,
            );
        }
    }
    for action in &frontend.actions {
        for source_ref in &action.source_refs {
            validate_source_ref(
                source_ref,
                "frontendExperience.actions.sourceRefs",
                source_ids,
                issues,
            );
        }
    }
    for path in &frontend.operation_paths {
        for source_ref in &path.source_refs {
            validate_source_ref(
                source_ref,
                "frontendExperience.operationPaths.sourceRefs",
                source_ids,
                issues,
            );
        }
    }
}

fn validate_nested_source_refs(
    candidate: &BrainstormCandidateAgentWritable,
    source_ids: &BTreeSet<String>,
    issues: &mut Vec<RepairIssue>,
) {
    for acceptance in &candidate.acceptance {
        for source_ref in &acceptance.source_refs {
            validate_source_ref(source_ref, "acceptance.sourceRefs", source_ids, issues);
        }
    }
    if let Some(concepts) = &candidate.concept_grounding {
        if let Some(glossary) = &concepts.delivery_concept_glossary {
            for concept in &glossary.concepts {
                for acceptance_ref in &concept.acceptance_refs {
                    if acceptance_ref.trim().is_empty() {
                        issues.push(issue(
                            "CONCEPT_ACCEPTANCE_REF_INVALID",
                            "conceptGrounding.deliveryConceptGlossary",
                            "acceptanceRefs must not be empty.",
                        ));
                        break;
                    }
                }
            }
        }
        for concept in &concepts.phase_concept_grounding.concepts {
            for acceptance_ref in &concept.acceptance_refs {
                if acceptance_ref.trim().is_empty() {
                    issues.push(issue(
                        "CONCEPT_ACCEPTANCE_REF_INVALID",
                        "conceptGrounding.phaseConceptGrounding",
                        "acceptanceRefs must not be empty.",
                    ));
                    break;
                }
            }
        }
    }
    let value = serde_json::to_value(candidate).unwrap_or_else(|_| Value::Null);
    collect_bad_source_refs(&value, source_ids, issues, "");
}

fn collect_bad_source_refs(
    value: &Value,
    source_ids: &BTreeSet<String>,
    issues: &mut Vec<RepairIssue>,
    path: &str,
) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let child = if path.is_empty() {
                    index.to_string()
                } else {
                    format!("{path}.{index}")
                };
                collect_bad_source_refs(item, source_ids, issues, &child);
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                if key == "sourceRefs" {
                    if let Some(items) = child.as_array() {
                        for item in items.iter().filter_map(Value::as_str) {
                            validate_source_ref(item, &child_path, source_ids, issues);
                        }
                    }
                    continue;
                }
                collect_bad_source_refs(child, source_ids, issues, &child_path);
            }
        }
        _ => {}
    }
}

fn validate_source_ref(
    source_ref: &str,
    field_path: &str,
    source_ids: &BTreeSet<String>,
    issues: &mut Vec<RepairIssue>,
) {
    if is_knowledge_ref(source_ref) {
        issues.push(issue(
            "KNOWLEDGE_REF_NOT_ALLOWED",
            field_path,
            "Knowledge source ids, chunk ids, inspect output, and knowledge paths must not enter Brainstorm formal sources.",
        ));
        return;
    }
    if !source_ref.trim().is_empty() && !source_ids.contains(source_ref) {
        issues.push(issue(
            "SOURCE_REF_INVALID",
            field_path,
            "sourceRefs must reference requirement source item ids from requirementContext.sourceItems.",
        ));
    }
}

fn is_knowledge_ref(value: &str) -> bool {
    value.starts_with("ksrc_")
        || value.starts_with("kchunk_")
        || value.contains("/knowledge/")
        || value.contains("knowledgeInspectChunk")
        || value.contains("brainstorm-context")
}

fn issue(code: &str, field_path: &str, message: &str) -> RepairIssue {
    RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("candidate".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

fn missing_block_gate(block: ClarificationBlockName, issues: Vec<String>) -> BrainstormGate {
    BrainstormGate {
        gate_id: format!("gate_{block:?}").to_ascii_lowercase(),
        current_block: block.clone(),
        required_blocks: crate::gate::required_blocks(),
        already_confirmed_blocks: vec![],
        skipped_blocks: vec![],
        user_message: block_message(&block),
        response_rule: BrainstormResponseRule {
            mode: "progressive_brainstorm".to_string(),
            final_summary_required_before_write: true,
            user_visible_confirmation_required: true,
        },
        issues,
    }
}
