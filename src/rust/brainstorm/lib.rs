mod accept;
mod artifacts;
mod gate;
mod paths;
mod request;
mod requirements;
mod start;
mod validation;

use delivery_core::{
    DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind, ValidatedPlanInput,
};

pub use accept::accept_brainstorm_file;

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
            RouteActionKind::TechnicalBaselineRequest
            | RouteActionKind::RepositoryContextRequest
            | RouteActionKind::PlanningContractCreate
            | RouteActionKind::ArchitectureArtifactContract
            | RouteActionKind::TaskplanGeneration
            | RouteActionKind::ContinueExecution => planning::PlanningDomainDispatcher
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

pub fn module_name() -> &'static str {
    "brainstorm"
}
