use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult, LoomMcpUserGateResult};
use serde_json::json;

use crate::DeployBootstrapInput;

pub fn deploy_bootstrap(input: DeployBootstrapInput) -> LoomMcpActionResult {
    if !input.confirm {
        return LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
            project_root: input.project_root,
            prompt: "Deployment bootstrap may run database migrations or seed commands. Confirm before execution.".to_string(),
            accepted_responses: vec!["confirm".to_string()],
            request_ref: None,
            delivery_id: None,
            phase_id: None,
            gate: Some(json!({
                "kind": input.kind,
                "tool": "loom.deployBootstrap",
                "confirmRequired": true
            })),
        });
    }
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: input.project_root,
        summary: "Deployment bootstrap has no automatic tasks to run.".to_string(),
        details: Some(
            json!({ "executed": false, "reason": "No bootstrap task was declared in this Rust deploy batch." }),
        ),
        warnings: vec![],
    })
}
