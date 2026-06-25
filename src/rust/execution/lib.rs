mod paths;
mod repair;
mod review;
mod task_execution;
mod task_plan;
mod task_result;

use delivery_core::{
    DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind, ValidatedPlanInput,
};

pub use repair::accept_repair_file;
pub use review::{accept_manual_review_resolution_file, accept_review_result_file};
pub use task_plan::accept_task_plan_file;
pub use task_result::accept_task_result_file;

#[derive(Debug, Default, Clone, Copy)]
pub struct ExecutionDomainDispatcher;

impl DomainDispatcher for ExecutionDomainDispatcher {
    fn start_brainstorm(&self, input: &ValidatedPlanInput) -> LoomMcpActionResult {
        delivery_core::UnimplementedDomainDispatcher.start_brainstorm(input)
    }

    fn dispatch_route_action(
        &self,
        project_root: &str,
        delivery_id: &str,
        phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult {
        match action.kind {
            RouteActionKind::TaskplanGeneration => {
                task_plan::materialize_request(project_root, delivery_id, phase_id)
            }
            RouteActionKind::ContinueExecution => {
                task_execution::continue_execution(project_root, delivery_id, phase_id)
            }
            RouteActionKind::Review => {
                review::materialize_review_request(project_root, delivery_id, phase_id)
            }
            RouteActionKind::ExecutionRepair => repair::materialize_delivery_execution_repair(
                project_root,
                delivery_id,
                phase_id,
                "task_failure",
                action.request_ref.clone(),
                vec![],
            ),
            RouteActionKind::TaskResultRepair
            | RouteActionKind::TaskplanRepair
            | RouteActionKind::ArchitectureArtifactRepair => {
                repair::dispatch_repair_route(project_root, delivery_id, phase_id, action)
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
    "execution"
}
