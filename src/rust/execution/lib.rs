mod browser;
mod paths;
mod repair;
mod review;
mod task_execution;
mod task_plan;
mod task_result;
mod templates;

use delivery_core::{
    DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind, ValidatedPlanInput,
};
use serde_json::Value;

pub use browser::{browser_runtime_targets, BrowserRuntimeTarget};
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
            RouteActionKind::ExecutionRepair => {
                let context = execution_repair_context(action);
                repair::materialize_delivery_execution_repair(
                    project_root,
                    delivery_id,
                    phase_id,
                    context.origin,
                    action.request_ref.clone(),
                    context.finding_refs,
                    context.target_task_ids,
                )
            }
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

struct ExecutionRepairRouteContext {
    origin: &'static str,
    finding_refs: Vec<String>,
    target_task_ids: Vec<String>,
}

fn execution_repair_context(action: &RouteAction) -> ExecutionRepairRouteContext {
    let origin = execution_repair_origin(action);
    let finding_refs = action
        .details
        .as_ref()
        .and_then(|details| {
            details
                .pointer("/nextAction/findingRefs")
                .or_else(|| details.get("findingRefs"))
        })
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect();
    let target_task_ids = action
        .details
        .as_ref()
        .map(target_task_ids_from_action_details)
        .unwrap_or_default();
    ExecutionRepairRouteContext {
        origin,
        finding_refs,
        target_task_ids,
    }
}

fn target_task_ids_from_action_details(details: &Value) -> Vec<String> {
    let mut values = Vec::new();
    for pointer in [
        "/nextAction/targetTaskIds",
        "/targetTaskIds",
        "/changeRequest/details/targetTaskIds",
    ] {
        values.extend(
            details
                .pointer(pointer)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_string)),
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    values.retain(|value| !value.is_empty() && seen.insert(value.clone()));
    values
}

fn execution_repair_origin(action: &RouteAction) -> &'static str {
    let detail_origin = action
        .details
        .as_ref()
        .and_then(|details| details.get("origin"))
        .and_then(Value::as_str);
    match (action.source.as_str(), detail_origin) {
        ("review_result", _) | (_, Some("review_result")) => "review_result",
        ("manual_review_resolution", _) | (_, Some("manual_review_resolution")) => {
            "manual_review_resolution"
        }
        _ => "task_failure",
    }
}
