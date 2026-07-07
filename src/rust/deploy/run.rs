use std::path::{Path, PathBuf};

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;

use crate::{
    active_operation::{acquire_operation, active_operation_result, update_operation_phase},
    paths::deployment_paths,
    prepare::deploy_prepare_inner,
    up::deploy_up_inner,
    DeployToolInput,
};

pub fn deploy_run(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root_buf = PathBuf::from(&input.project_root);
    let project_root = project_root_buf.as_path();
    let guard = match acquire_operation(project_root, "deploy.run", "preparing") {
        Ok(Ok(guard)) => guard,
        Ok(Err(operation)) => return active_operation_result(project_root, operation),
        Err(error) => {
            return LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: input.project_root,
                summary: "Deployment run could not acquire operation.".to_string(),
                details: Some(json!({ "error": error.to_string() })),
                warnings: vec![error.to_string()],
            })
        }
    };
    if !deployment_paths(project_root).spec_file.exists() {
        let _ = update_operation_phase(project_root, "preparing", "running");
        let prepare = deploy_prepare_inner(project_root, input.clone());
        match prepare {
            Ok(LoomMcpActionResult::Done(_)) => {}
            Ok(result) => {
                drop(guard);
                return result;
            }
            Err(error) => {
                drop(guard);
                return LoomMcpActionResult::Blocked(delivery_core::LoomMcpBlockedResult {
                    project_root: project_root.to_string_lossy().into_owned(),
                    blockers: vec![error.to_string()],
                    recommended_tool: Some("loom.deployInspect".to_string()),
                    details: Some(json!({ "failureKind": "deploy_prepare_failed" })),
                });
            }
        }
    }
    let _ = update_operation_phase(project_root, "building", "running");
    let result = deploy_up_inner(project_root, input);
    drop(guard);
    result
}

pub fn deploy_retry_after_repair(project_root: &Path) -> LoomMcpActionResult {
    deploy_up_inner(
        project_root,
        DeployToolInput {
            project_root: project_root.to_string_lossy().into_owned(),
            app_path: None,
            healthcheck: None,
            provider_policy: None,
        },
    )
}
