use delivery_core::{
    DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind, ValidatedPlanInput,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct WorkflowDomainDispatcher;

impl DomainDispatcher for WorkflowDomainDispatcher {
    fn start_brainstorm(&self, input: &ValidatedPlanInput) -> LoomMcpActionResult {
        brainstorm::BrainstormDomainDispatcher.start_brainstorm(input)
    }

    fn dispatch_route_action(
        &self,
        project_root: &str,
        delivery_id: &str,
        phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult {
        match action.kind {
            RouteActionKind::BrainstormConfirmation => brainstorm::BrainstormDomainDispatcher
                .dispatch_route_action(project_root, delivery_id, phase_id, action),
            RouteActionKind::TechnicalBaselineRequest => {
                planning::materialize_technical_baseline_request(
                    project_root,
                    delivery_id,
                    phase_id,
                )
            }
            RouteActionKind::RepositoryContextRequest => {
                planning::materialize_repository_context_request(
                    project_root,
                    delivery_id,
                    phase_id,
                )
            }
            RouteActionKind::PlanningContractCreate => {
                planning::create_contract_and_route(project_root, delivery_id, phase_id, *self)
            }
            RouteActionKind::ArchitectureArtifactContract => {
                architecture::ArchitectureDomainDispatcher.dispatch_route_action(
                    project_root,
                    delivery_id,
                    phase_id,
                    action,
                )
            }
            RouteActionKind::TaskplanGeneration
            | RouteActionKind::ContinueExecution
            | RouteActionKind::Review
            | RouteActionKind::ExecutionRepair
            | RouteActionKind::TaskResultRepair
            | RouteActionKind::TaskplanRepair
            | RouteActionKind::ArchitectureArtifactRepair => execution::ExecutionDomainDispatcher
                .dispatch_route_action(project_root, delivery_id, phase_id, action),
            _ => delivery_core::UnimplementedDomainDispatcher.dispatch_route_action(
                project_root,
                delivery_id,
                phase_id,
                action,
            ),
        }
    }
}
