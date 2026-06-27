mod accept;
mod artifacts;
mod clarification;
mod gate;
mod paths;
mod request;
mod requirements;
mod start;
mod validation;

use std::{collections::BTreeMap, path::Path};

use contracts::{BrainstormContract, NextPhasePreview};
use delivery_core::{
    apply_delivery_index, DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState,
    DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind, TransitionStore,
    ValidatedPlanInput,
};
use serde_json::json;
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, DeliveryPhaseLocator},
    store::{ensure_dir, StateError, StateResult},
};

pub use accept::accept_brainstorm_file;
pub use clarification::{confirm_block, BrainstormConfirmBlockInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextPhaseHandoff {
    pub phase_id: String,
    pub title: String,
    pub goal: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BrainstormDomainDispatcher;

impl DomainDispatcher for BrainstormDomainDispatcher {
    fn start_brainstorm(&self, input: &ValidatedPlanInput) -> LoomMcpActionResult {
        start::start_brainstorm(input)
    }

    fn dispatch_route_action(
        &self,
        project_root: &str,
        delivery_id: &str,
        phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult {
        match action.kind {
            RouteActionKind::BrainstormConfirmation => {
                clarification::materialize_confirmation_request(project_root, delivery_id, phase_id)
            }
            _ => delivery_core::UnimplementedDomainDispatcher.dispatch_route_action(
                project_root,
                delivery_id,
                phase_id,
                action,
            ),
        }
    }
}

pub fn module_name() -> &'static str {
    "brainstorm"
}

pub fn next_phase_handoff_from_preview(
    project_root: &str,
    delivery_id: &str,
    source_phase_id: &str,
    requested_phase_id: Option<&str>,
) -> StateResult<Option<NextPhaseHandoff>> {
    let root = Path::new(project_root);
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let contract = read_phase_brainstorm_contract(root, &delivery, source_phase_id)?;
    let NextPhasePreview::Candidate {
        suggested_phase_id,
        title,
        goal,
        reason,
        ..
    } = &contract.phase_plan.next_phase_preview
    else {
        return Ok(None);
    };
    let phase_id = requested_phase_id
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| normalize_next_phase_id(&delivery, suggested_phase_id));
    Ok(Some(NextPhaseHandoff {
        phase_id,
        title: title.clone(),
        goal: goal.clone(),
        reason: reason.clone(),
    }))
}

pub fn materialize_next_phase_from_preview(
    project_root: &str,
    delivery_id: &str,
    source_phase_id: &str,
    requested_phase_id: Option<&str>,
) -> StateResult<Option<NextPhaseHandoff>> {
    let Some(handoff) = next_phase_handoff_from_preview(
        project_root,
        delivery_id,
        source_phase_id,
        requested_phase_id,
    )?
    else {
        return Ok(None);
    };
    let root = Path::new(project_root);
    let store = FileTransitionStore;
    let mut status = store.load_status(project_root).map_err(to_state_error)?;
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    if delivery
        .phases
        .iter()
        .any(|phase| phase.phase_id == handoff.phase_id)
    {
        return Ok(Some(handoff));
    }
    let source_phase = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == source_phase_id)
        .ok_or_else(|| StateError::InvalidArgument(format!("phase {source_phase_id} not found")))?;
    let contract = read_phase_brainstorm_contract(root, &delivery, source_phase_id)?;
    let now = state::store::now_string();
    let brainstorm_run_id = format!("brainstorm-run-{}", state::store::now_millis());
    let request_id = format!("brainstorm-session-{}", state::store::now_millis());
    let contract_ref = latest_ref(source_phase, "brainstormContract")?;
    let requirement_context_ref = latest_ref(source_phase, "requirementContext")?;
    let context_refs = json!({
        "requirementContextRef": requirement_context_ref,
        "normalizedRequirementTextRef": source_phase.latest_refs.get("normalizedRequirementText"),
        "keywordHintsRef": source_phase.latest_refs.get("keywordHints"),
    });
    let request_root = request::build_brainstorm_request_root(
        root,
        &request_id,
        delivery_id,
        &handoff.phase_id,
        &brainstorm_run_id,
        &contract.delivery_context.user_facing_language,
        context_refs,
    );
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "brainstorm_clarification_block".to_string(),
            request_file: None,
            delivery_id: Some(delivery_id.to_string()),
            phase_id: Some(handoff.phase_id.clone()),
            root: request_root,
        },
    )?;
    let clarification_state =
        clarification::initial_state(delivery_id, &handoff.phase_id, &brainstorm_run_id);
    let clarification_state_ref = clarification::write_initial_state_file(
        root,
        delivery_id,
        &handoff.phase_id,
        &clarification_state,
    )?;
    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.to_string(),
        phase_id: handoff.phase_id.clone(),
    };
    ensure_dir(
        &state::paths::workspace_dir(root, &locator)
            .join("brainstorm-knowledge")
            .join(&request_id),
    )?;
    let mut latest_refs = BTreeMap::new();
    latest_refs.insert("brainstormRequestId".to_string(), request_id);
    latest_refs.insert(
        "brainstormRequestRef".to_string(),
        stored.request_ref.clone(),
    );
    latest_refs.insert("brainstormRunId".to_string(), brainstorm_run_id);
    latest_refs.insert("brainstormContract".to_string(), contract_ref.to_string());
    latest_refs.insert(
        "brainstormClarificationState".to_string(),
        clarification_state_ref,
    );
    latest_refs.insert(
        "requirementContext".to_string(),
        requirement_context_ref.to_string(),
    );
    if let Some(normalized) = source_phase.latest_refs.get("normalizedRequirementText") {
        latest_refs.insert("normalizedRequirementText".to_string(), normalized.clone());
    }
    if let Some(keyword_hints) = source_phase.latest_refs.get("keywordHints") {
        latest_refs.insert("keywordHints".to_string(), keyword_hints.clone());
    }
    delivery.phases.push(DeliveryPhaseState {
        phase_id: handoff.phase_id.clone(),
        latest_refs,
        next_action: Some(RouteAction {
            kind: RouteActionKind::BrainstormClarification,
            source: "phase_handoff".to_string(),
            reason: "next_phase_preview_candidate".to_string(),
            prompt: Some(
                "Read the current Brainstorm block request, query request-scoped knowledge for this block, and confirm the next active phase boundary in the user's language."
                    .to_string(),
            ),
            accepted_responses: vec!["reply_in_chat".to_string()],
            request_ref: Some(stored.request_ref),
            details: Some(json!({
                "fromPhaseId": source_phase_id,
                "phaseId": handoff.phase_id,
                "title": handoff.title,
                "goal": handoff.goal,
                "reason": handoff.reason
            })),
            target_phase_id: None,
        }),
    });
    delivery.status = DeliveryLifecycleStatus::Planning;
    delivery.updated_at = now;
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)?;
    Ok(Some(handoff))
}

fn read_phase_brainstorm_contract(
    project_root: &Path,
    delivery: &DeliveryIndex,
    phase_id: &str,
) -> StateResult<BrainstormContract> {
    let phase = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| StateError::InvalidArgument(format!("phase {phase_id} not found")))?;
    let contract_ref = latest_ref(phase, "brainstormContract")?;
    state::store::read_json(&from_project_relative(project_root, contract_ref)?)
}

fn latest_ref<'a>(phase: &'a DeliveryPhaseState, key: &str) -> StateResult<&'a str> {
    phase
        .latest_refs
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| {
            StateError::InvalidArgument(format!(
                "phase {} missing latestRefs.{key}",
                phase.phase_id
            ))
        })
}

fn normalize_next_phase_id(delivery: &DeliveryIndex, suggested_phase_id: &str) -> String {
    let suggested = suggested_phase_id.trim();
    if !suggested.is_empty() && suggested != "phase-next" {
        return suggested.to_string();
    }
    let next = delivery
        .phases
        .iter()
        .filter_map(|phase| phase.phase_id.strip_prefix("phase-"))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(delivery.phases.len() as u32)
        + 1;
    format!("phase-{next}")
}

fn to_state_error(error: delivery_core::LoomCoreError) -> StateError {
    StateError::StateCorrupted(error.to_string())
}
