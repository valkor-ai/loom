use std::path::Path;

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::store::{path_exists, read_json_value};

use crate::{
    active_operation::{active_operation_result, live_operation},
    paths::deployment_paths,
    DeployToolInput,
};

pub fn deploy_status(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root = Path::new(&input.project_root);
    match live_operation(project_root) {
        Ok(Some(operation)) => return active_operation_result(project_root, operation),
        Ok(None) => {}
        Err(error) => {
            return LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: input.project_root,
                summary: "Deployment status could not read active operation.".to_string(),
                details: Some(json!({ "error": error.to_string() })),
                warnings: vec![error.to_string()],
            })
        }
    }
    let paths = deployment_paths(project_root);
    let state = if path_exists(&paths.state_file) {
        read_json_value(&paths.state_file).ok()
    } else {
        None
    };
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: input.project_root,
        summary: "Deployment status loaded.".to_string(),
        details: Some(json!({
            "prepared": path_exists(&paths.spec_file),
            "state": state,
        })),
        warnings: vec![],
    })
}
