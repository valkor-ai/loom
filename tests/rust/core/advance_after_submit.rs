use std::{cell::RefCell, collections::BTreeMap};

use delivery_core::{
    apply_delivery_index, DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState,
    DomainDispatcher, LoomCoreError, LoomMcpActionResult, LoomMcpDoneResult, LoomResult,
    OperationContext, OperationLease, ProjectStatus, RouteAction, RouteActionKind,
    SubmitAcceptedEvent, TransitionDiagnostic, TransitionEngine, TransitionStore,
    ValidatedPlanInput,
};

#[test]
fn advance_after_submit_updates_phase_and_returns_next_result() {
    let mut status = ProjectStatus::empty("1");
    let delivery = DeliveryIndex {
        schema_version: 1,
        delivery_id: "delivery_1".to_string(),
        active_phase_id: "phase_1".to_string(),
        status: DeliveryLifecycleStatus::Planning,
        phases: vec![DeliveryPhaseState {
            phase_id: "phase_1".to_string(),
            latest_refs: Default::default(),
            next_action: None,
        }],
        updated_at: "1".to_string(),
    };
    apply_delivery_index(&mut status, &delivery);
    let store = MemoryStore::new(status).with_delivery(delivery);
    let engine = TransitionEngine {
        store,
        dispatcher: TestDispatcher,
    };

    let result = engine
        .advance_after_submit(
            OperationContext {
                project_root: "/tmp/project".to_string(),
            },
            SubmitAcceptedEvent {
                delivery_id: "delivery_1".to_string(),
                phase_id: "phase_1".to_string(),
                source_tool: "loom.technicalBaselineAcceptFile".to_string(),
                accepted_artifact_ref: "loom://artifact/baseline".to_string(),
                next_action: Some(RouteAction {
                    kind: RouteActionKind::RepositoryContextRequest,
                    source: "submit".to_string(),
                    reason: "baseline_accepted".to_string(),
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: None,
                    details: None,
                    target_phase_id: None,
                }),
            },
        )
        .expect("advance result");

    let LoomMcpActionResult::Done(LoomMcpDoneResult { summary, .. }) = result else {
        panic!("expected done result");
    };
    assert!(summary.contains("repository_context_request"));
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
    status: RefCell<ProjectStatus>,
    deliveries: RefCell<BTreeMap<String, DeliveryIndex>>,
}

impl MemoryStore {
    fn new(status: ProjectStatus) -> Self {
        Self {
            status: RefCell::new(status),
            deliveries: RefCell::new(BTreeMap::new()),
        }
    }

    fn with_delivery(self, delivery: DeliveryIndex) -> Self {
        self.deliveries
            .borrow_mut()
            .insert(delivery.delivery_id.clone(), delivery);
        self
    }
}

impl TransitionStore for MemoryStore {
    fn load_status(&self, _project_root: &str) -> LoomResult<ProjectStatus> {
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
        _delivery_id: &str,
    ) -> LoomResult<Option<OperationLease>> {
        Ok(None)
    }

    fn write_operation_lease(
        &self,
        _project_root: &str,
        _delivery_id: &str,
        _lease: &OperationLease,
    ) -> LoomResult<()> {
        Ok(())
    }

    fn write_transition_diagnostic(
        &self,
        _project_root: &str,
        _diagnostic: &TransitionDiagnostic,
    ) -> LoomResult<()> {
        Ok(())
    }

    fn now_millis(&self) -> u128 {
        10
    }

    fn now_string(&self) -> String {
        "10".to_string()
    }
}
