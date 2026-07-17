use std::{cell::RefCell, collections::BTreeMap};

use delivery_core::{
    apply_delivery_index, DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState,
    DomainDispatcher, LoomCoreError, LoomMcpActionResult, LoomMcpDoneResult, LoomResult,
    OperationContext, OperationLease, OperationLeaseStatus, OperationType, ProjectStatus,
    RouteAction, RouteActionKind, TransitionDiagnostic, TransitionEngine, TransitionStore,
    ValidatedPlanInput,
};
use serde_json::json;

#[test]
fn continue_reports_state_not_initialized_through_failed_result() {
    let engine = TransitionEngine {
        store: MemoryStore::missing(),
        dispatcher: TestDispatcher,
    };

    let result = engine
        .continue_current(OperationContext {
            project_root: "/tmp/project".to_string(),
        })
        .expect("continue result");

    assert_eq!(state_name(&result), "failed");
    let LoomMcpActionResult::Failed(result) = result else {
        panic!("expected failed result");
    };
    assert_eq!(result.error.code, "STATE_NOT_INITIALIZED");
    assert_eq!(
        result.error.recovery_tool.as_deref(),
        Some("loom.initProject")
    );
}

#[test]
fn continue_blocks_when_no_active_delivery_exists() {
    let engine = TransitionEngine {
        store: MemoryStore::new(ProjectStatus::empty("1")),
        dispatcher: TestDispatcher,
    };

    let result = engine
        .continue_current(OperationContext {
            project_root: "/tmp/project".to_string(),
        })
        .expect("continue result");

    assert_eq!(state_name(&result), "blocked");
}

#[test]
fn continue_returns_active_operation_for_fresh_lease() {
    let mut status = ProjectStatus::empty("1");
    let delivery = sample_delivery(RouteAction {
        kind: RouteActionKind::TechnicalBaselineRequest,
        source: "test".to_string(),
        reason: "need baseline".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: None,
        details: None,
        target_phase_id: None,
    });
    apply_delivery_index(&mut status, &delivery);
    let store = MemoryStore::new(status)
        .with_delivery(delivery)
        .with_lease(OperationLease {
            schema_version: "1.0".to_string(),
            operation_id: "op_1".to_string(),
            delivery_id: "delivery_1".to_string(),
            phase_id: "phase_1".to_string(),
            operation_type: OperationType::TaskExecution,
            status: OperationLeaseStatus::Active,
            started_at: "10".to_string(),
            heartbeat_at: "10".to_string(),
            expires_at: "20".to_string(),
            refs: json!({ "requestRef": "loom://projects/p/requests/r" }),
        });
    let engine = TransitionEngine {
        store,
        dispatcher: TestDispatcher,
    };

    let result = engine
        .continue_current(OperationContext {
            project_root: "/tmp/project".to_string(),
        })
        .expect("continue result");

    assert_eq!(state_name(&result), "active_operation");
}

#[test]
fn continue_activates_next_phase_before_dispatching() {
    let mut status = ProjectStatus::empty("1");
    let delivery = DeliveryIndex {
        schema_version: 1,
        delivery_id: "delivery_1".to_string(),
        active_phase_id: "phase_1".to_string(),
        status: DeliveryLifecycleStatus::Planning,
        phases: vec![
            DeliveryPhaseState {
                phase_id: "phase_1".to_string(),
                latest_refs: Default::default(),
                next_action: Some(RouteAction {
                    kind: RouteActionKind::ContinueToNextPhase,
                    source: "review".to_string(),
                    reason: "approved".to_string(),
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: None,
                    details: None,
                    target_phase_id: Some("phase_2".to_string()),
                }),
            },
            DeliveryPhaseState {
                phase_id: "phase_2".to_string(),
                latest_refs: Default::default(),
                next_action: Some(RouteAction {
                    kind: RouteActionKind::TechnicalBaselineRequest,
                    source: "phase_2".to_string(),
                    reason: "baseline".to_string(),
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: None,
                    details: None,
                    target_phase_id: None,
                }),
            },
        ],
        updated_at: "1".to_string(),
    };
    apply_delivery_index(&mut status, &delivery);
    let store = MemoryStore::new(status).with_delivery(delivery);
    let engine = TransitionEngine {
        store,
        dispatcher: TestDispatcher,
    };

    let result = engine
        .continue_current(OperationContext {
            project_root: "/tmp/project".to_string(),
        })
        .expect("continue result");

    let LoomMcpActionResult::Done(LoomMcpDoneResult { summary, .. }) = result else {
        panic!("expected dispatcher done result");
    };
    assert!(summary.contains("technical_baseline_request"));
}

#[test]
fn continue_rejects_invalid_next_phase_target_before_mutating_delivery() {
    let mut status = ProjectStatus::empty("1");
    let delivery = sample_delivery(RouteAction {
        kind: RouteActionKind::ContinueToNextPhase,
        source: "review".to_string(),
        reason: "approved".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: None,
        details: None,
        target_phase_id: Some("phase-missing".to_string()),
    });
    apply_delivery_index(&mut status, &delivery);
    let store = MemoryStore::new(status).with_delivery(delivery);
    let engine = TransitionEngine {
        store,
        dispatcher: TestDispatcher,
    };

    let error = engine
        .continue_current(OperationContext {
            project_root: "/tmp/project".to_string(),
        })
        .expect_err("invalid phase target must fail");
    assert_eq!(error.code(), "PHASE_NOT_FOUND");
    assert_eq!(
        engine
            .store
            .deliveries
            .borrow()
            .get("delivery_1")
            .expect("delivery")
            .active_phase_id,
        "phase_1"
    );
}

#[test]
fn continue_refreshes_brainstorm_gate_consumption_contract() {
    let mut status = ProjectStatus::empty("1");
    let request_ref = "loom://projects/project_1/requests/brainstorm_1".to_string();
    let delivery = sample_delivery(RouteAction {
        kind: RouteActionKind::BrainstormClarification,
        source: "repository_context_accept".to_string(),
        reason: "phase_scope_required".to_string(),
        prompt: Some("Confirm the active phase boundary.".to_string()),
        accepted_responses: vec!["reply_in_chat".to_string()],
        request_ref: Some(request_ref),
        details: Some(serde_json::json!({
            "kind": "phase_brainstorm_continuation",
            "currentBlock": "phase_scope"
        })),
        target_phase_id: None,
    });
    apply_delivery_index(&mut status, &delivery);
    let store = MemoryStore::new(status).with_delivery(delivery);
    let engine = TransitionEngine {
        store,
        dispatcher: TestDispatcher,
    };

    let result = engine
        .continue_current(OperationContext {
            project_root: "/tmp/project".to_string(),
        })
        .expect("continue result");
    let LoomMcpActionResult::UserGate(gate) = result else {
        panic!("expected user gate");
    };
    let contract = gate
        .pre_response_contract
        .expect("brainstorm pre-response contract");
    assert!(contract.steps.iter().any(|step| matches!(
        step,
        delivery_core::LoomMcpUserGatePreResponseStep::RunKnowledgeContextPlan { block, .. }
            if block == "phase_scope"
    )));
}

fn sample_delivery(next_action: RouteAction) -> DeliveryIndex {
    DeliveryIndex {
        schema_version: 1,
        delivery_id: "delivery_1".to_string(),
        active_phase_id: "phase_1".to_string(),
        status: DeliveryLifecycleStatus::Planning,
        phases: vec![DeliveryPhaseState {
            phase_id: "phase_1".to_string(),
            latest_refs: Default::default(),
            next_action: Some(next_action),
        }],
        updated_at: "1".to_string(),
    }
}

fn state_name(result: &LoomMcpActionResult) -> &'static str {
    match result {
        LoomMcpActionResult::AutoRunnable(_) => "auto_runnable",
        LoomMcpActionResult::UserGate(_) => "user_gate",
        LoomMcpActionResult::ActiveOperation(_) => "active_operation",
        LoomMcpActionResult::Done(_) => "done",
        LoomMcpActionResult::Blocked(_) => "blocked",
        LoomMcpActionResult::RepairableError(_) => "repairable_error",
        LoomMcpActionResult::Failed(_) => "failed",
    }
}

struct TestDispatcher;

impl DomainDispatcher for TestDispatcher {
    fn start_brainstorm(&self, _input: &ValidatedPlanInput) -> LoomMcpActionResult {
        LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: "/tmp/project".to_string(),
            summary: "brainstorm".to_string(),
            details: None,
            warnings: vec![],
        })
    }

    fn dispatch_route_action(
        &self,
        project_root: &str,
        _delivery_id: &str,
        _phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult {
        LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: project_root.to_string(),
            summary: serde_json::to_value(&action.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| "route_action".to_string()),
            details: None,
            warnings: vec![],
        })
    }
}

struct MemoryStore {
    missing: bool,
    status: RefCell<ProjectStatus>,
    deliveries: RefCell<BTreeMap<String, DeliveryIndex>>,
    leases: RefCell<BTreeMap<String, OperationLease>>,
    diagnostics: RefCell<Vec<TransitionDiagnostic>>,
}

impl MemoryStore {
    fn missing() -> Self {
        Self {
            missing: true,
            status: RefCell::new(ProjectStatus::empty("0")),
            deliveries: RefCell::new(BTreeMap::new()),
            leases: RefCell::new(BTreeMap::new()),
            diagnostics: RefCell::new(vec![]),
        }
    }

    fn new(status: ProjectStatus) -> Self {
        Self {
            missing: false,
            status: RefCell::new(status),
            deliveries: RefCell::new(BTreeMap::new()),
            leases: RefCell::new(BTreeMap::new()),
            diagnostics: RefCell::new(vec![]),
        }
    }

    fn with_delivery(self, delivery: DeliveryIndex) -> Self {
        self.deliveries
            .borrow_mut()
            .insert(delivery.delivery_id.clone(), delivery);
        self
    }

    fn with_lease(self, lease: OperationLease) -> Self {
        self.leases
            .borrow_mut()
            .insert(lease.delivery_id.clone(), lease);
        self
    }
}

impl TransitionStore for MemoryStore {
    fn load_status(&self, _project_root: &str) -> LoomResult<ProjectStatus> {
        if self.missing {
            return Err(LoomCoreError::failure(
                "STATE_NOT_INITIALIZED",
                "missing status",
            ));
        }
        Ok(self.status.borrow().clone())
    }

    fn save_status(&self, _project_root: &str, status: &ProjectStatus) -> LoomResult<()> {
        *self.status.borrow_mut() = status.clone();
        Ok(())
    }

    fn load_delivery_index(
        &self,
        _project_root: &str,
        delivery_id: &str,
    ) -> LoomResult<DeliveryIndex> {
        self.deliveries
            .borrow()
            .get(delivery_id)
            .cloned()
            .ok_or_else(|| LoomCoreError::failure("DELIVERY_INDEX_CORRUPTED", "missing delivery"))
    }

    fn save_delivery_index(&self, _project_root: &str, delivery: &DeliveryIndex) -> LoomResult<()> {
        self.deliveries
            .borrow_mut()
            .insert(delivery.delivery_id.clone(), delivery.clone());
        Ok(())
    }

    fn read_operation_lease(
        &self,
        _project_root: &str,
        delivery_id: &str,
    ) -> LoomResult<Option<OperationLease>> {
        Ok(self.leases.borrow().get(delivery_id).cloned())
    }

    fn write_operation_lease(
        &self,
        _project_root: &str,
        delivery_id: &str,
        lease: &OperationLease,
    ) -> LoomResult<()> {
        self.leases
            .borrow_mut()
            .insert(delivery_id.to_string(), lease.clone());
        Ok(())
    }

    fn write_transition_diagnostic(
        &self,
        _project_root: &str,
        diagnostic: &TransitionDiagnostic,
    ) -> LoomResult<()> {
        self.diagnostics.borrow_mut().push(diagnostic.clone());
        Ok(())
    }

    fn now_millis(&self) -> u128 {
        10
    }

    fn now_string(&self) -> String {
        "10".to_string()
    }
}
