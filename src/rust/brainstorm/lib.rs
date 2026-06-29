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
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, DeliveryPhaseLocator},
    store::{ensure_dir, read_json_value, StateError, StateResult},
};

pub use accept::accept_brainstorm_file;
pub use clarification::{confirm_block, BrainstormConfirmBlockInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextPhaseHandoff {
    pub phase_id: String,
    pub title: String,
    pub goal: String,
    pub scope_preview: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseBrainstormRequest {
    pub phase_id: String,
    pub request_id: String,
    pub request_ref: String,
    pub brainstorm_run_id: String,
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
        scope_preview,
        reason,
        ..
    } = &contract.phase_plan.next_phase_preview
    else {
        return Ok(None);
    };
    let phase_id =
        if let Some(requested) = requested_phase_id.filter(|value| !value.trim().is_empty()) {
            requested.to_string()
        } else if delivery
            .phases
            .iter()
            .any(|phase| phase.phase_id == *suggested_phase_id)
        {
            suggested_phase_id.clone()
        } else {
            normalize_next_phase_id(&delivery, source_phase_id, suggested_phase_id)
        };
    Ok(Some(NextPhaseHandoff {
        phase_id,
        title: title.clone(),
        goal: goal.clone(),
        scope_preview: scope_preview.clone(),
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
    let source_phase = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == source_phase_id)
        .cloned()
        .ok_or_else(|| StateError::InvalidArgument(format!("phase {source_phase_id} not found")))?;
    let now = state::store::now_string();
    let latest_refs = next_phase_latest_refs(root, delivery_id, &source_phase)?;
    let next_action = next_phase_repository_context_action(source_phase_id, &handoff);
    if let Some(existing_phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == handoff.phase_id)
    {
        for (key, value) in latest_refs {
            existing_phase.latest_refs.entry(key).or_insert(value);
        }
        if should_attach_repository_context_start(existing_phase) {
            existing_phase.next_action = Some(next_action);
        }
    } else {
        delivery.phases.push(DeliveryPhaseState {
            phase_id: handoff.phase_id.clone(),
            latest_refs,
            next_action: Some(next_action),
        });
    }
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

fn next_phase_latest_refs(
    root: &Path,
    delivery_id: &str,
    source_phase: &DeliveryPhaseState,
) -> StateResult<BTreeMap<String, String>> {
    let contract_ref = latest_ref(source_phase, "brainstormContract")?;
    let requirement_context_ref = latest_ref(source_phase, "requirementContext")?;
    let mut latest_refs = BTreeMap::new();
    latest_refs.insert("brainstormContract".to_string(), contract_ref.to_string());
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
    if let Some(technical_baseline) = source_phase.latest_refs.get("technicalBaseline") {
        latest_refs.insert("technicalBaseline".to_string(), technical_baseline.clone());
    } else {
        let baseline_file = root
            .join(".loom")
            .join("deliveries")
            .join(delivery_id)
            .join("contracts")
            .join("technical-baseline.json");
        if baseline_file.exists() {
            latest_refs.insert(
                "technicalBaseline".to_string(),
                state::paths::to_project_relative(root, &baseline_file)?,
            );
        }
    }
    if let Some(decision) = source_phase.latest_refs.get("brainstormDecisionSnapshot") {
        latest_refs.insert(
            "latestConfirmedRequirementDecision".to_string(),
            decision.clone(),
        );
    }
    let decisions_index = paths::brainstorm_decisions_index_file(root, delivery_id);
    if decisions_index.exists() {
        latest_refs.insert(
            "confirmedRequirementDecisionsIndex".to_string(),
            state::paths::to_project_relative(root, &decisions_index)?,
        );
    }
    Ok(latest_refs)
}

fn next_phase_repository_context_action(
    source_phase_id: &str,
    handoff: &NextPhaseHandoff,
) -> RouteAction {
    RouteAction {
        kind: RouteActionKind::RepositoryContextRequest,
        source: "phase_handoff".to_string(),
        reason: "next_phase_preview_candidate".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: None,
        details: Some(json!({
            "fromPhaseId": source_phase_id,
            "phaseId": handoff.phase_id.clone(),
            "title": handoff.title.clone(),
            "goal": handoff.goal.clone(),
            "scopePreview": handoff.scope_preview.clone(),
            "reason": handoff.reason.clone()
        })),
        target_phase_id: None,
    }
}

fn should_attach_repository_context_start(phase: &DeliveryPhaseState) -> bool {
    let has_started_refs = [
        "repositoryContext",
        "brainstormRequestRef",
        "planningGenerationContract",
        "architectureArtifact",
        "taskPlan",
        "reviewResult",
    ]
    .iter()
    .any(|key| phase.latest_refs.contains_key(*key));
    if has_started_refs {
        return false;
    }
    phase
        .next_action
        .as_ref()
        .map(|action| {
            matches!(
                action.kind,
                RouteActionKind::RepositoryContextRequest | RouteActionKind::Done
            )
        })
        .unwrap_or(true)
}

pub fn materialize_phase_brainstorm_from_preview(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    repository_context_ref: Option<&str>,
) -> StateResult<Option<PhaseBrainstormRequest>> {
    let root = Path::new(project_root);
    let store = FileTransitionStore;
    let mut status = store.load_status(project_root).map_err(to_state_error)?;
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let phase = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| StateError::InvalidArgument(format!("phase {phase_id} not found")))?;
    if let Some(existing_request_ref) = phase.latest_refs.get("brainstormRequestRef").cloned() {
        if state::inspect_request(delivery_core::InspectRequestInput {
            project_root: project_root.to_string(),
            request_ref: existing_request_ref.clone(),
        })
        .map(|request| request.request_kind == "brainstorm_clarification_block")
        .unwrap_or(false)
        {
            let request_id = phase
                .latest_refs
                .get("brainstormRequestId")
                .cloned()
                .unwrap_or_else(|| "brainstorm-session".to_string());
            let brainstorm_run_id = phase
                .latest_refs
                .get("brainstormRunId")
                .cloned()
                .unwrap_or_else(|| "brainstorm-run".to_string());
            return Ok(Some(PhaseBrainstormRequest {
                phase_id: phase_id.to_string(),
                request_id,
                request_ref: existing_request_ref,
                brainstorm_run_id,
            }));
        }
    }

    let contract_ref = latest_ref(phase, "brainstormContract")?;
    let contract: BrainstormContract =
        state::store::read_json(&from_project_relative(root, contract_ref)?)?;
    let source_phase_id = contract.phase_id.clone();
    let Some(handoff) = next_phase_handoff_from_preview(
        project_root,
        delivery_id,
        &source_phase_id,
        Some(phase_id),
    )?
    else {
        return Ok(None);
    };
    let brainstorm_run_id = format!("brainstorm-run-{}", state::store::now_millis());
    let request_id = format!("brainstorm-session-{}", state::store::now_millis());
    let mut context_refs = json!({
        "deliveryContextRef": contract_ref,
        "requirementContextRef": phase.latest_refs.get("requirementContext"),
        "normalizedRequirementTextRef": phase.latest_refs.get("normalizedRequirementText"),
        "keywordHintsRef": phase.latest_refs.get("keywordHints"),
        "latestConfirmedRequirementDecisionRef": phase.latest_refs.get("latestConfirmedRequirementDecision"),
        "confirmedRequirementDecisionsIndexRef": phase.latest_refs.get("confirmedRequirementDecisionsIndex"),
    });
    if let Some(repository_context_ref) = repository_context_ref {
        context_refs["latestRepositoryContextRef"] = json!(repository_context_ref);
    }
    let mut request_root = request::build_brainstorm_request_root(
        root,
        &request_id,
        delivery_id,
        &handoff.phase_id,
        &brainstorm_run_id,
        &contract.delivery_context.user_facing_language,
        context_refs,
    );
    attach_next_phase_seed(&mut request_root, &source_phase_id, &handoff);
    let repository_projection = match repository_context_ref {
        Some(repository_context_ref) => Some(repository_continuation_projection(
            root,
            repository_context_ref,
        )?),
        None => None,
    };
    attach_phase_continuation_context(
        &mut request_root,
        &source_phase_id,
        &handoff,
        repository_projection,
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
    if let Some(active_phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == handoff.phase_id)
    {
        active_phase
            .latest_refs
            .insert("brainstormRequestId".to_string(), request_id.clone());
        active_phase.latest_refs.insert(
            "brainstormRequestRef".to_string(),
            stored.request_ref.clone(),
        );
        active_phase
            .latest_refs
            .insert("brainstormRunId".to_string(), brainstorm_run_id.clone());
        active_phase.latest_refs.insert(
            "brainstormClarificationState".to_string(),
            clarification_state_ref,
        );
        if let Some(repository_context_ref) = repository_context_ref {
            active_phase.latest_refs.insert(
                "latestRepositoryContext".to_string(),
                repository_context_ref.to_string(),
            );
        }
        active_phase.next_action = Some(RouteAction {
            kind: RouteActionKind::BrainstormClarification,
            source: "repository_context_accept".to_string(),
            reason: "repository_context_ready_for_phase_brainstorm".to_string(),
            prompt: Some(
                "Read the current Brainstorm block request, query request-scoped knowledge for this block, and confirm the next active phase boundary in the user's language."
                    .to_string(),
            ),
            accepted_responses: vec!["reply_in_chat".to_string()],
            request_ref: Some(stored.request_ref.clone()),
            details: Some(json!({
                "fromPhaseId": source_phase_id,
                "phaseId": handoff.phase_id,
                "title": handoff.title,
                "goal": handoff.goal,
                "scopePreview": handoff.scope_preview,
                "reason": handoff.reason
            })),
            target_phase_id: None,
        });
    }
    delivery.status = DeliveryLifecycleStatus::Planning;
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(to_state_error)?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(to_state_error)?;
    Ok(Some(PhaseBrainstormRequest {
        phase_id: phase_id.to_string(),
        request_id,
        request_ref: stored.request_ref,
        brainstorm_run_id,
    }))
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

fn attach_next_phase_seed(root: &mut Value, source_phase_id: &str, handoff: &NextPhaseHandoff) {
    root["nextPhaseSeed"] = json!({
        "source": "phasePlan.nextPhasePreview",
        "fromPhaseId": source_phase_id,
        "phaseId": handoff.phase_id,
        "title": handoff.title,
        "goal": handoff.goal,
        "scopePreview": handoff.scope_preview,
        "reason": handoff.reason,
        "usageRule": "Use this as the non-binding seed for the active phase boundary. Query request-scoped knowledge before presenting options, and do not turn this seed into a full-project roadmap."
    });
    if let Some(groups) = root
        .pointer_mut("/requestReadPlan/groups")
        .and_then(Value::as_array_mut)
    {
        if !groups
            .iter()
            .any(|group| group.get("groupId").and_then(Value::as_str) == Some("next_phase_seed"))
        {
            let insert_at = groups.len().min(2);
            groups.insert(
                insert_at,
                json!({
                    "groupId": "next_phase_seed",
                    "required": true,
                    "purpose": "Read the next phase seed before composing phase continuation options.",
                    "whenToRead": "Read after the conversation protocol and compact requirement context, before querying knowledge or presenting phase_scope options.",
                    "fields": [
                        "nextPhaseSeed.fromPhaseId",
                        "nextPhaseSeed.phaseId",
                        "nextPhaseSeed.title",
                        "nextPhaseSeed.goal",
                        "nextPhaseSeed.scopePreview",
                        "nextPhaseSeed.reason",
                        "nextPhaseSeed.usageRule"
                    ]
                }),
            );
        }
    }
}

fn repository_continuation_projection(
    root: &Path,
    repository_context_ref: &str,
) -> StateResult<Value> {
    let repository_context =
        read_json_value(&from_project_relative(root, repository_context_ref)?)?;
    let capability_summaries = repository_context
        .get("existingCapabilities")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name").and_then(Value::as_str)?;
                    let status = item
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let summary = item.get("summary").and_then(Value::as_str).unwrap_or("");
                    Some(format!("{name} [{status}]: {summary}"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surface_summaries = repository_context
        .get("relevantSurfaces")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let path = item.get("path").and_then(Value::as_str)?;
                    let kind = item
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("surface");
                    let summary = item.get("summary").and_then(Value::as_str).unwrap_or("");
                    Some(format!("{path} [{kind}]: {summary}"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({
        "repoSummary": repository_context
            .pointer("/repoOverview/summary")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "capabilitySummaries": capability_summaries,
        "surfaceSummaries": surface_summaries
    }))
}

fn attach_phase_continuation_context(
    root: &mut Value,
    source_phase_id: &str,
    handoff: &NextPhaseHandoff,
    repository_projection: Option<Value>,
) {
    root["phaseContinuationContext"] = json!({
        "activePhase": {
            "phaseId": handoff.phase_id,
            "title": handoff.title,
            "goal": handoff.goal
        },
        "previousPhaseId": source_phase_id,
        "rules": [
            "Use nextPhaseSeed as the non-binding starting point for this phase clarification.",
            "Use phaseContinuationContext.repository as current code facts so implemented prior-phase capabilities are not re-asked as new scope.",
            "Use the field-level deliveryContext and latestConfirmedRequirementDecision reads as accepted prior Brainstorm facts.",
            "Confirm only the current phase; do not regenerate a complete future roadmap."
        ]
    });
    if let Some(repository_projection) = repository_projection {
        root["phaseContinuationContext"]["repository"] = repository_projection;
    }
    let has_repository_projection = root
        .pointer("/phaseContinuationContext/repository")
        .is_some();
    if let Some(groups) = root
        .pointer_mut("/requestReadPlan/groups")
        .and_then(Value::as_array_mut)
    {
        if !groups.iter().any(|group| {
            group.get("groupId").and_then(Value::as_str) == Some("phase_continuation_context")
        }) {
            let mut fields = vec![
                "phaseContinuationContext.activePhase.phaseId",
                "phaseContinuationContext.activePhase.title",
                "phaseContinuationContext.activePhase.goal",
                "phaseContinuationContext.previousPhaseId",
                "phaseContinuationContext.rules",
                "latestConfirmedRequirementDecision.summary.title",
                "latestConfirmedRequirementDecision.summary.oneLine",
                "latestConfirmedRequirementDecision.summary.businessGoal",
            ];
            if has_repository_projection {
                fields.extend([
                    "phaseContinuationContext.repository.repoSummary",
                    "phaseContinuationContext.repository.capabilitySummaries",
                    "phaseContinuationContext.repository.surfaceSummaries",
                ]);
            }
            let insert_at = groups.len().min(3);
            groups.insert(
                insert_at,
                json!({
                    "groupId": "phase_continuation_context",
                    "required": true,
                    "purpose": "Read phase continuation context before presenting the current phase scope.",
                    "whenToRead": "Read before knowledge queries and before presenting phase_scope options.",
                    "fields": fields
                }),
            );
        }
    }
}

fn normalize_next_phase_id(
    delivery: &DeliveryIndex,
    source_phase_id: &str,
    suggested_phase_id: &str,
) -> String {
    let suggested = suggested_phase_id.trim();
    if !suggested.is_empty() && suggested != "phase-next" {
        return suggested.to_string();
    }
    if let Some(next) = source_phase_id
        .strip_prefix("phase-")
        .and_then(|suffix| suffix.parse::<u32>().ok())
        .map(|phase_number| format!("phase-{}", phase_number + 1))
    {
        return next;
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
