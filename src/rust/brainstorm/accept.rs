use std::{collections::BTreeSet, path::Path};

use contracts::{
    BrainstormCandidateAgentWritable, RequirementContext, RequirementSourceItem,
    UserFacingLanguageConstraint,
};
use delivery_core::{
    DomainDispatcher, FileSubmitInput, LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpRepairableErrorResult, LoomMcpUserGateResult, OperationContext, ReadFieldGroupInput,
    SubmitAcceptedEvent, TransitionEngine, TransitionStore,
};
use state::{
    lifecycle_store::FileTransitionStore, paths::from_project_relative,
    write_targets::AuthorizedWriteSet,
};

use crate::{
    artifacts::write_accepted_artifacts,
    gate::to_value,
    requirements::formal_sources_from_items,
    validation::{gate_check, validate_candidate},
};

pub fn accept_brainstorm_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher + Clone,
{
    match accept_brainstorm_file_inner(input, authorized, dispatcher) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "BRAINSTORM_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(7),
                domain: Some("brainstorm".to_string()),
                route_action: Some("brainstorm_accept".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

fn accept_brainstorm_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher + Clone,
{
    let Some(target) = authorized.targets.first() else {
        return Ok(LoomMcpActionResult::RepairableError(
            LoomMcpRepairableErrorResult {
                project_root: input.project_root.clone(),
                stop_allowed: false,
                target_file: String::new(),
                target_ids: vec![],
                issues: vec![delivery_core::RepairIssue {
                    code: "TARGET_MISSING".to_string(),
                    message: "No authorized Brainstorm candidate target was written.".to_string(),
                    target_id: Some("candidate".to_string()),
                    field_path: None,
                }],
                resubmit_tool: "loom.brainstormAcceptFile".to_string(),
                fix_scope: Some("brainstorm_candidate_only".to_string()),
                read_groups: authorized.read_groups.clone(),
                agent_instruction: delivery_core::repairable_error_agent_instruction(
                    "loom.brainstormAcceptFile",
                ),
            },
        ));
    };
    let project_root = Path::new(&input.project_root);
    let candidate_file = from_project_relative(project_root, &target.path)?;
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("authorized deliveryId is missing".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("authorized phaseId is missing".to_string())
    })?;
    if let Some(result) = ensure_latest_brainstorm_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
    )? {
        return Ok(result);
    }
    let mut raw = state::store::read_json_value(&candidate_file)?;
    normalize_machine_owned_candidate_fields(
        &input.project_root,
        &input.request_ref,
        &phase_id,
        &mut raw,
    )?;
    let gate_check = gate_check(&raw);
    if !gate_check.repair_issues.is_empty() {
        return Ok(LoomMcpActionResult::RepairableError(
            LoomMcpRepairableErrorResult {
                project_root: input.project_root.clone(),
                stop_allowed: false,
                target_file: target.path.clone(),
                target_ids: vec![target.target_id.clone()],
                issues: gate_check.repair_issues,
                resubmit_tool: "loom.brainstormAcceptFile".to_string(),
                fix_scope: Some("brainstorm_candidate_only".to_string()),
                read_groups: authorized.read_groups.clone(),
                agent_instruction: delivery_core::repairable_error_agent_instruction(
                    "loom.brainstormAcceptFile",
                ),
            },
        ));
    }
    if let Some(gate) = gate_check.gate {
        return Ok(LoomMcpActionResult::UserGate(LoomMcpUserGateResult::new(
            input.project_root.clone(),
            gate.user_message.clone(),
            vec!["reply_in_chat".to_string()],
            Some(input.request_ref.clone()),
            authorized.delivery_id.clone(),
            authorized.phase_id.clone(),
            Some(to_value(&gate)),
        )));
    }

    let candidate: BrainstormCandidateAgentWritable = match serde_json::from_value(raw) {
        Ok(candidate) => candidate,
        Err(error) => {
            return Ok(LoomMcpActionResult::RepairableError(
                LoomMcpRepairableErrorResult {
                    project_root: input.project_root.clone(),
                    stop_allowed: false,
                    target_file: target.path.clone(),
                    target_ids: vec![target.target_id.clone()],
                    issues: vec![delivery_core::RepairIssue {
                        code: "BRAINSTORM_CANDIDATE_SCHEMA_INVALID".to_string(),
                        message: format!(
                            "Brainstorm candidate JSON has an invalid schema: {error}"
                        ),
                        target_id: Some(target.target_id.clone()),
                        field_path: Some("candidate".to_string()),
                    }],
                    resubmit_tool: "loom.brainstormAcceptFile".to_string(),
                    fix_scope: Some("brainstorm_candidate_only".to_string()),
                    read_groups: authorized.read_groups.clone(),
                    agent_instruction: delivery_core::repairable_error_agent_instruction(
                        "loom.brainstormAcceptFile",
                    ),
                },
            ));
        }
    };

    let request_context = load_accept_request_context(&input.project_root, &input.request_ref)?;
    let source_items = request_context.source_items;
    let source_ids = source_items
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<BTreeSet<_>>();
    let validation_phase_id = authorized.phase_id.as_deref().unwrap_or("phase-1");
    let issues = validate_candidate(&candidate, validation_phase_id, &source_ids);
    if !issues.is_empty() {
        return Ok(LoomMcpActionResult::RepairableError(
            LoomMcpRepairableErrorResult {
                project_root: input.project_root.clone(),
                stop_allowed: false,
                target_file: target.path.clone(),
                target_ids: vec![target.target_id.clone()],
                issues,
                resubmit_tool: "loom.brainstormAcceptFile".to_string(),
                fix_scope: Some("brainstorm_candidate_only".to_string()),
                read_groups: authorized.read_groups.clone(),
                agent_instruction: delivery_core::repairable_error_agent_instruction(
                    "loom.brainstormAcceptFile",
                ),
            },
        ));
    }

    let request_id = request_context.request_id;
    let project_paths = state::paths::project_paths(&input.project_root)?;
    let request_index =
        state::request_index::get_request_index_entry(&input.project_root, &request_id)?;
    let request_file = from_project_relative(&project_paths.root, &request_index.request_file)?;
    let request_root = state::store::read_json_value(&request_file)?;
    let brainstorm_run_id = request_root
        .get("brainstormRunId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("brainstorm-run")
        .to_string();
    let user_facing_language = request_context.user_facing_language;
    let formal_sources = formal_sources_from_items(&source_items);
    let original_request_text = build_original_request_text(project_root, &source_items)
        .unwrap_or(request_context.normalized_text);
    let persisted = write_accepted_artifacts(
        project_root,
        &delivery_id,
        &phase_id,
        &brainstorm_run_id,
        &format!("contract-{brainstorm_run_id}"),
        &candidate,
        &original_request_text,
        &source_ids.iter().cloned().collect::<Vec<_>>(),
        &formal_sources,
        user_facing_language,
        authorized
            .next_action
            .as_ref()
            .map(|action| action.kind.clone()),
        &state::store::now_string(),
    )?;

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
            "brainstormContract".to_string(),
            persisted.contract_ref.clone(),
        );
        phase.latest_refs.insert(
            "brainstormDecisionSnapshot".to_string(),
            persisted.decision_snapshot_ref.clone(),
        );
        if let Some(refs) = &persisted.concept_grounding_refs {
            if let Some(delivery_glossary) = &refs.delivery_concept_glossary_ref {
                phase.latest_refs.insert(
                    "deliveryConceptGlossary".to_string(),
                    delivery_glossary.clone(),
                );
            }
            phase.latest_refs.insert(
                "phaseConceptGrounding".to_string(),
                refs.phase_concept_grounding_ref.clone(),
            );
        }
        if let Some(refs) = &persisted.frontend_experience_refs {
            if let Some(confirmed) = &refs.confirmed_frontend_experience_ref {
                phase
                    .latest_refs
                    .insert("confirmedFrontendExperience".to_string(), confirmed.clone());
            }
            if let Some(current) = &refs.current_frontend_experience_ref {
                phase
                    .latest_refs
                    .insert("currentFrontendExperience".to_string(), current.clone());
            }
        }
    }
    store
        .save_delivery_index(&input.project_root, &delivery)
        .map_err(to_state_error)?;

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
                source_tool: "loom.brainstormAcceptFile".to_string(),
                accepted_artifact_ref: format!(
                    "{}/targets/{}",
                    input.request_ref, target.target_id
                ),
                next_action: authorized.next_action.clone(),
            },
        )
        .map_err(to_state_error)
}

fn normalize_machine_owned_candidate_fields(
    project_root: &str,
    request_ref: &str,
    phase_id: &str,
    candidate: &mut serde_json::Value,
) -> Result<(), state::store::StateError> {
    let confirmed = state::read_field_group_flat(ReadFieldGroupInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
        group_id: "confirmed_clarification_state".to_string(),
    })?;
    let blocks = confirmed
        .fields
        .get("confirmedClarificationState.blocks")
        .and_then(|field| field.value.as_array())
        .cloned()
        .unwrap_or_default();
    let final_summary_confirmed = confirmed
        .fields
        .get("confirmedClarificationState.finalSummaryConfirmed")
        .and_then(|field| field.value.as_bool())
        .unwrap_or(false);
    let mut confirmed_blocks = Vec::new();
    let mut skipped_blocks = Vec::new();
    let mut presented_items = Vec::new();
    let mut final_summary = None;
    let mut final_confirmed_at = None;
    for block in blocks {
        let Some(block_name) = block.get("block").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let summary = block
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        if block_name == "final_summary" {
            final_summary = Some(summary);
            final_confirmed_at = block
                .get("confirmedAt")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            continue;
        }
        presented_items.push(block_name.to_string());
        if block
            .get("skipped")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            skipped_blocks.push(serde_json::json!({
                "block": block_name,
                "reason": block
                    .get("skipReason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&summary)
            }));
        } else if block
            .get("confirmedByUser")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            confirmed_blocks.push(serde_json::json!({
                "block": block_name,
                "summary": summary,
                "confirmedByUser": true
            }));
        }
    }
    if final_summary_confirmed {
        presented_items.push("final_summary".to_string());
    }

    let Some(object) = candidate.as_object_mut() else {
        return Ok(());
    };
    object.insert(
        "userConfirmation".to_string(),
        serde_json::json!({
            "confirmed": final_summary_confirmed,
            "confirmedAt": final_confirmed_at,
            "confirmationSummary": final_summary.unwrap_or_default(),
            "confirmationBasis": {
                "initialRequestOnly": false,
                "summaryPresentedToUser": final_summary_confirmed,
                "confirmedAfterSummary": final_summary_confirmed,
                "presentedItems": presented_items
            }
        }),
    );
    object.insert(
        "clarificationProgress".to_string(),
        serde_json::json!({
            "mode": "progressive_blocks",
            "confirmedBlocks": confirmed_blocks,
            "skippedBlocks": skipped_blocks,
            "finalSummaryConfirmed": final_summary_confirmed
        }),
    );
    if let Some(roadmap) = object
        .get_mut("roadmap")
        .and_then(serde_json::Value::as_object_mut)
    {
        roadmap.insert(
            "currentPhaseId".to_string(),
            serde_json::Value::String(phase_id.to_string()),
        );
    }
    if let Some(current) = object
        .get_mut("phasePlan")
        .and_then(|phase_plan| phase_plan.get_mut("current"))
        .and_then(serde_json::Value::as_object_mut)
    {
        current.insert(
            "phaseId".to_string(),
            serde_json::Value::String(phase_id.to_string()),
        );
    }

    let Some(updates) = candidate
        .pointer_mut("/conceptGrounding/glossaryUpdates")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };

    for (index, update) in updates.iter_mut().enumerate() {
        let Some(update) = update.as_object_mut() else {
            continue;
        };
        update.insert(
            "updateId".to_string(),
            serde_json::Value::String(format!("glossary_update_{}", index + 1)),
        );
    }
    Ok(())
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}

struct AcceptRequestContext {
    request_id: String,
    source_items: Vec<RequirementSourceItem>,
    normalized_text: String,
    user_facing_language: UserFacingLanguageConstraint,
}

fn load_accept_request_context(
    project_root: &str,
    request_ref: &str,
) -> Result<AcceptRequestContext, state::store::StateError> {
    let request_id = parse_request_id(request_ref)?;
    let request_index = state::request_index::get_request_index_entry(project_root, &request_id)?;
    let project_paths = state::paths::project_paths(project_root)?;
    let request_file = from_project_relative(&project_paths.root, &request_index.request_file)?;
    let request_root = state::store::read_json_value(&request_file)?;
    let user_facing_language = serde_json::from_value::<UserFacingLanguageConstraint>(
        request_root
            .get("userFacingLanguage")
            .cloned()
            .ok_or_else(|| {
                state::store::StateError::StateCorrupted(
                    "Brainstorm request userFacingLanguage is missing".to_string(),
                )
            })?,
    )
    .map_err(|error| {
        state::store::StateError::StateCorrupted(format!("invalid userFacingLanguage: {error}"))
    })?;
    let context_refs = request_root
        .get("contextRefs")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "Brainstorm request contextRefs is missing".to_string(),
            )
        })?;
    let requirement_context_ref = context_refs
        .get("requirementContextRef")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "Brainstorm request contextRefs.requirementContextRef is missing".to_string(),
            )
        })?;
    let requirement_context_file =
        from_project_relative(&project_paths.root, requirement_context_ref)?;
    let requirement_context: RequirementContext =
        state::store::read_json(&requirement_context_file)?;
    let normalized_text = context_refs
        .get("normalizedRequirementTextRef")
        .and_then(serde_json::Value::as_str)
        .map(|relative| {
            from_project_relative(&project_paths.root, relative)
                .and_then(|path| state::store::read_text(&path))
        })
        .transpose()?
        .unwrap_or_default();
    Ok(AcceptRequestContext {
        request_id,
        source_items: requirement_context.source_items,
        normalized_text,
        user_facing_language,
    })
}

fn parse_request_id(request_ref: &str) -> Result<String, state::store::StateError> {
    let prefix = "loom://projects/";
    let rest = request_ref.strip_prefix(prefix).ok_or_else(|| {
        state::store::StateError::InvalidArgument(
            "requestRef must start with loom://projects/.".to_string(),
        )
    })?;
    let (_project_id, request_id) = rest.split_once("/requests/").ok_or_else(|| {
        state::store::StateError::InvalidArgument("requestRef must include /requests/.".to_string())
    })?;
    if request_id.is_empty() || request_id.contains('/') {
        return Err(state::store::StateError::InvalidArgument(format!(
            "invalid requestRef: {request_ref}"
        )));
    }
    Ok(request_id.to_string())
}

fn ensure_latest_brainstorm_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    if delivery.active_phase_id != phase_id {
        return Ok(Some(stale_request_failure(
            project_root,
            format!(
                "Brainstorm submit is bound to phase {phase_id}, but the active phase is {}.",
                delivery.active_phase_id
            ),
        )));
    }
    let Some(phase) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return Ok(Some(stale_request_failure(
            project_root,
            format!("Active delivery {delivery_id} is missing phase {phase_id}."),
        )));
    };
    let latest_request = phase.latest_refs.get("brainstormRequestRef");
    if latest_request.map(String::as_str) != Some(request_ref) {
        return Ok(Some(stale_request_failure(
            project_root,
            "Brainstorm submit must use the active phase latest Brainstorm requestRef. Read the latest request and resubmit from that request only.".to_string(),
        )));
    }
    Ok(None)
}

fn stale_request_failure(project_root: &str, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: "STALE_BRAINSTORM_REQUEST".to_string(),
            message,
            target_batch: Some(7),
            domain: Some("brainstorm".to_string()),
            route_action: Some("brainstorm_accept".to_string()),
            recovery_tool: Some("loom.continue".to_string()),
        },
    })
}

fn build_original_request_text(
    project_root: &Path,
    source_items: &[RequirementSourceItem],
) -> Option<String> {
    let mut parts = Vec::new();
    for item in source_items {
        let relative = item
            .text_ref
            .as_deref()
            .or(item.extracted_text_ref.as_deref())?;
        let absolute = from_project_relative(project_root, relative).ok()?;
        let text = std::fs::read_to_string(absolute).ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        parts.push(trimmed.to_string());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}
