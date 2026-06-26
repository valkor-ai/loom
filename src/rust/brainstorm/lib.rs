mod accept;
mod artifacts;
mod clarification;
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
pub use clarification::{confirm_block, BrainstormConfirmBlockInput};

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
