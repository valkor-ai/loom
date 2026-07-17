use serde_json::json;

use crate::{
    apply_delivery_index, current_phase, status_details, ActiveOperationRef, DeliveryIndex,
    DomainDispatcher, LoomCoreError, LoomMcpActionResult, LoomMcpActiveOperationResult,
    LoomMcpBlockedResult, LoomMcpDoneResult, LoomMcpFailure, LoomMcpFailureResult, LoomResult,
    OperationLease, ProjectStatus, RouteAction, RouteActionKind, TransitionDiagnostic,
};

pub trait TransitionStore {
    fn load_status(&self, project_root: &str) -> LoomResult<ProjectStatus>;
    fn save_status(&self, project_root: &str, status: &ProjectStatus) -> LoomResult<()>;
    fn load_delivery_index(
        &self,
        project_root: &str,
        delivery_id: &str,
    ) -> LoomResult<DeliveryIndex>;
    fn save_delivery_index(&self, project_root: &str, delivery: &DeliveryIndex) -> LoomResult<()>;
    fn read_operation_lease(
        &self,
        project_root: &str,
        delivery_id: &str,
    ) -> LoomResult<Option<OperationLease>>;
    fn write_operation_lease(
        &self,
        project_root: &str,
        delivery_id: &str,
        lease: &OperationLease,
    ) -> LoomResult<()>;
    fn write_transition_diagnostic(
        &self,
        project_root: &str,
        diagnostic: &TransitionDiagnostic,
    ) -> LoomResult<()>;
    fn now_millis(&self) -> u128;
    fn now_string(&self) -> String;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationContext {
    pub project_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitAcceptedEvent {
    pub delivery_id: String,
    pub phase_id: String,
    pub source_tool: String,
    pub accepted_artifact_ref: String,
    pub next_action: Option<RouteAction>,
}

pub struct TransitionEngine<S, D> {
    pub store: S,
    pub dispatcher: D,
}

impl<S, D> TransitionEngine<S, D>
where
    S: TransitionStore,
    D: DomainDispatcher,
{
    pub fn continue_current(&self, ctx: OperationContext) -> LoomResult<LoomMcpActionResult> {
        let mut status = match self.store.load_status(&ctx.project_root) {
            Ok(status) => status,
            Err(error) if error.code() == "STATE_NOT_INITIALIZED" => {
                return Ok(LoomMcpActionResult::Failed(LoomMcpFailureResult {
                    project_root: ctx.project_root,
                    error: LoomMcpFailure {
                        code: "STATE_NOT_INITIALIZED".to_string(),
                        message: error.message().to_string(),
                        target_batch: None,
                        domain: Some("project_lifecycle".to_string()),
                        route_action: None,
                        recovery_tool: Some("loom.initProject".to_string()),
                    },
                }));
            }
            Err(error) => return Err(error),
        };

        let Some(active_delivery_id) = status.active_delivery_id.clone() else {
            return Ok(if status.last_completed_delivery_id.is_some() {
                LoomMcpActionResult::Done(LoomMcpDoneResult {
                    project_root: ctx.project_root,
                    summary: "No active delivery. The last Loom delivery is already complete."
                        .to_string(),
                    details: Some(status_details(&status, None, None, &[])),
                    warnings: vec![],
                })
            } else {
                LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
                    project_root: ctx.project_root,
                    blockers: vec![
                        "No active Loom delivery is available. Start a new one with loom.plan."
                            .to_string(),
                    ],
                    recommended_tool: Some("loom.plan".to_string()),
                    details: Some(status_details(&status, None, None, &[])),
                })
            });
        };

        let mut delivery = self
            .store
            .load_delivery_index(&ctx.project_root, &active_delivery_id)?;
        let mut lease = self
            .store
            .read_operation_lease(&ctx.project_root, &active_delivery_id)?;
        let mut active_phase = current_phase(&delivery).cloned().ok_or_else(|| {
            LoomCoreError::failure(
                "PHASE_NOT_FOUND",
                format!(
                    "Active phase {} was not found for delivery {}.",
                    delivery.active_phase_id, delivery.delivery_id
                ),
            )
        })?;

        if let Some(existing) = lease.as_mut() {
            if existing.is_fresh_at(self.store.now_millis()) {
                if active_phase
                    .next_action
                    .as_ref()
                    .map(|action| action.kind.clone().is_user_gate())
                    .unwrap_or(false)
                {
                    return Ok(user_gate_result(
                        ctx.project_root,
                        &delivery.delivery_id,
                        &delivery.active_phase_id,
                        active_phase
                            .next_action
                            .as_ref()
                            .expect("user gate action exists"),
                    ));
                }
                return Ok(LoomMcpActionResult::ActiveOperation(
                    LoomMcpActiveOperationResult {
                        project_root: ctx.project_root,
                        operation: ActiveOperationRef {
                            operation_id: existing.operation_id.clone(),
                            operation_type: serde_json::to_value(&existing.operation_type)
                                .ok()
                                .and_then(|value| value.as_str().map(str::to_string))
                                .unwrap_or_else(|| "operation".to_string()),
                            delivery_id: Some(existing.delivery_id.clone()),
                            phase_id: Some(existing.phase_id.clone()),
                            started_at: existing.started_at.clone(),
                            expires_at: existing.expires_at.clone(),
                        },
                        allowed_observation_tools: vec![
                            "loom.status".to_string(),
                            "loom.inspectRequest".to_string(),
                            "loom.readFieldGroup".to_string(),
                        ],
                        observation_policy: None,
                        forbidden_actions: vec![],
                        progress_summary: Some(json!({
                            "status": existing.status,
                            "refs": existing.refs,
                        })),
                    },
                ));
            }
            existing.mark_stale_recovered(self.store.now_string());
            self.store
                .write_operation_lease(&ctx.project_root, &active_delivery_id, existing)?;
        }

        if active_phase
            .next_action
            .as_ref()
            .map(|action| action.kind == RouteActionKind::ContinueToNextPhase)
            .unwrap_or(false)
        {
            let target_phase_id = active_phase
                .next_action
                .as_ref()
                .and_then(|action| action.target_phase_id.clone())
                .ok_or_else(|| {
                    LoomCoreError::failure(
                        "PHASE_NOT_FOUND",
                        "continue_to_next_phase is missing targetPhaseId.",
                    )
                })?;
            let current_index = delivery
                .phases
                .iter()
                .position(|phase| phase.phase_id == delivery.active_phase_id)
                .ok_or_else(|| {
                    LoomCoreError::failure(
                        "PHASE_NOT_FOUND",
                        format!(
                            "Active phase {} was not found in delivery {}.",
                            delivery.active_phase_id, delivery.delivery_id
                        ),
                    )
                })?;
            let target_index = delivery
                .phases
                .iter()
                .position(|phase| phase.phase_id == target_phase_id)
                .ok_or_else(|| {
                    LoomCoreError::failure(
                        "PHASE_NOT_FOUND",
                        format!("Target phase {} was not found.", target_phase_id),
                    )
                })?;
            if target_index <= current_index {
                return Err(LoomCoreError::failure(
                    "INVALID_NEXT_PHASE_TARGET",
                    format!(
                        "continue_to_next_phase must target a later phase than {}.",
                        delivery.active_phase_id
                    ),
                ));
            }
            delivery.active_phase_id = target_phase_id;
            self.store
                .save_delivery_index(&ctx.project_root, &delivery)?;
            apply_delivery_index(&mut status, &delivery);
            self.store.save_status(&ctx.project_root, &status)?;
            active_phase = current_phase(&delivery).cloned().ok_or_else(|| {
                LoomCoreError::failure(
                    "PHASE_NOT_FOUND",
                    format!(
                        "Activated phase {} was not found for delivery {}.",
                        delivery.active_phase_id, delivery.delivery_id
                    ),
                )
            })?;
        }

        let result = match active_phase.next_action.as_ref() {
            Some(action) if action.kind.clone().is_user_gate() => user_gate_result(
                ctx.project_root.clone(),
                &delivery.delivery_id,
                &delivery.active_phase_id,
                action,
            ),
            Some(action) if action.kind == RouteActionKind::Done => {
                LoomMcpActionResult::Done(LoomMcpDoneResult {
                    project_root: ctx.project_root.clone(),
                    summary: "The current Loom delivery is complete.".to_string(),
                    details: Some(status_details(&status, Some(&delivery), None, &[])),
                    warnings: vec![],
                })
            }
            Some(action) => self.dispatcher.dispatch_route_action(
                &ctx.project_root,
                &delivery.delivery_id,
                &delivery.active_phase_id,
                action,
            ),
            None => LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: ctx.project_root.clone(),
                summary: "No next action is recorded for the active Loom delivery.".to_string(),
                details: Some(status_details(&status, Some(&delivery), None, &[])),
                warnings: vec![],
            }),
        };

        self.store.write_transition_diagnostic(
            &ctx.project_root,
            &TransitionDiagnostic {
                decision_id: format!("decision_{}", self.store.now_string()),
                delivery_id: delivery.delivery_id.clone(),
                phase_id: delivery.active_phase_id.clone(),
                source: "continue".to_string(),
                route_action: active_phase
                    .next_action
                    .as_ref()
                    .map(|action| {
                        serde_json::to_value(&action.kind)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_else(|| "route_action".to_string())
                    })
                    .unwrap_or_else(|| "none".to_string()),
                result_state: result_state_name(&result),
                target_batch: target_batch_from_result(&result),
                created_at: self.store.now_string(),
            },
        )?;

        Ok(result)
    }

    pub fn advance_after_submit(
        &self,
        ctx: OperationContext,
        event: SubmitAcceptedEvent,
    ) -> LoomResult<LoomMcpActionResult> {
        let mut status = self.store.load_status(&ctx.project_root)?;
        let mut delivery = self
            .store
            .load_delivery_index(&ctx.project_root, &event.delivery_id)?;
        if delivery.active_phase_id != event.phase_id {
            return Err(LoomCoreError::failure(
                "STALE_SUBMIT_PHASE",
                format!(
                    "Accepted submit event belongs to {}, but the active delivery phase is {}.",
                    event.phase_id, delivery.active_phase_id
                ),
            ));
        }
        let phase = delivery
            .phases
            .iter_mut()
            .find(|phase| phase.phase_id == event.phase_id)
            .ok_or_else(|| {
                LoomCoreError::failure(
                    "PHASE_NOT_FOUND",
                    format!(
                        "Submit event phase {} was not found for delivery {}.",
                        event.phase_id, event.delivery_id
                    ),
                )
            })?;
        phase.next_action = event
            .next_action
            .clone()
            .or_else(|| Some(RouteAction::done("submit_accepted")));
        delivery.updated_at = self.store.now_string();
        self.store
            .save_delivery_index(&ctx.project_root, &delivery)?;
        apply_delivery_index(&mut status, &delivery);
        self.store.save_status(&ctx.project_root, &status)?;
        self.continue_current(ctx)
    }
}

fn user_gate_result(
    project_root: String,
    delivery_id: &str,
    phase_id: &str,
    action: &RouteAction,
) -> LoomMcpActionResult {
    let gate = crate::LoomMcpUserGateResult::new(
        project_root,
        action.prompt.clone().unwrap_or_else(|| {
            "User confirmation is required before Loom can continue.".to_string()
        }),
        if action.accepted_responses.is_empty() {
            vec!["confirm".to_string()]
        } else {
            action.accepted_responses.clone()
        },
        action.request_ref.clone(),
        Some(delivery_id.to_string()),
        Some(phase_id.to_string()),
        action.details.clone(),
    );
    if action.kind == RouteActionKind::BrainstormClarification
        || action
            .details
            .as_ref()
            .and_then(|details| details.get("kind"))
            .and_then(serde_json::Value::as_str)
            == Some("phase_brainstorm_continuation")
    {
        let block = action
            .details
            .as_ref()
            .and_then(|details| details.get("currentBlock"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("phase_scope");
        return LoomMcpActionResult::UserGate(gate.with_brainstorm_knowledge(block));
    }
    LoomMcpActionResult::UserGate(gate)
}

fn result_state_name(result: &LoomMcpActionResult) -> String {
    match result {
        LoomMcpActionResult::AutoRunnable(_) => "auto_runnable",
        LoomMcpActionResult::UserGate(_) => "user_gate",
        LoomMcpActionResult::ActiveOperation(_) => "active_operation",
        LoomMcpActionResult::Done(_) => "done",
        LoomMcpActionResult::Blocked(_) => "blocked",
        LoomMcpActionResult::RepairableError(_) => "repairable_error",
        LoomMcpActionResult::Failed(_) => "failed",
    }
    .to_string()
}

fn target_batch_from_result(result: &LoomMcpActionResult) -> Option<u32> {
    match result {
        LoomMcpActionResult::Failed(result) => result.error.target_batch,
        _ => None,
    }
}
