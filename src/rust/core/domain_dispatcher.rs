use crate::{
    LoomMcpActionResult, LoomMcpFailure, LoomMcpFailureResult, RouteAction, RouteActionKind,
    ValidatedPlanInput,
};

pub trait DomainDispatcher {
    fn start_brainstorm(&self, input: &ValidatedPlanInput) -> LoomMcpActionResult;
    fn dispatch_route_action(
        &self,
        project_root: &str,
        delivery_id: &str,
        phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnimplementedDomainDispatcher;

impl DomainDispatcher for UnimplementedDomainDispatcher {
    fn start_brainstorm(&self, input: &ValidatedPlanInput) -> LoomMcpActionResult {
        LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "not_implemented_for_batch".to_string(),
                message: "loom.plan requires the Brainstorm domain handler.".to_string(),
                target_batch: Some(7),
                domain: Some("brainstorm".to_string()),
                route_action: None,
                recovery_tool: None,
            },
        })
    }

    fn dispatch_route_action(
        &self,
        project_root: &str,
        _delivery_id: &str,
        _phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult {
        match (
            action.kind.clone().target_batch(),
            action.kind.clone().domain(),
        ) {
            (Some(target_batch), Some(domain)) => {
                LoomMcpActionResult::Failed(LoomMcpFailureResult {
                    project_root: project_root.to_string(),
                    error: LoomMcpFailure {
                        code: "not_implemented_for_batch".to_string(),
                        message: format!(
                            "{} requires the {} domain handler.",
                            route_action_name(&action.kind),
                            domain
                        ),
                        target_batch: Some(target_batch),
                        domain: Some(domain.to_string()),
                        route_action: Some(route_action_name(&action.kind)),
                        recovery_tool: None,
                    },
                })
            }
            _ => LoomMcpActionResult::Done(crate::LoomMcpDoneResult {
                project_root: project_root.to_string(),
                summary: "No further action is required.".to_string(),
                details: None,
                warnings: vec![],
            }),
        }
    }
}

fn route_action_name(kind: &RouteActionKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "route_action".to_string())
}
