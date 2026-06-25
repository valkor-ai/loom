use std::{path::Path, process::Command};

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{paths::from_project_relative, store::write_json_atomic};

use crate::{
    active_operation::{acquire_operation, active_operation_result},
    prepare::read_spec,
    DeployToolInput,
};

pub fn deploy_down(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root = Path::new(&input.project_root);
    let guard = match acquire_operation(project_root, "deploy.down", "stopping") {
        Ok(Ok(guard)) => guard,
        Ok(Err(operation)) => return active_operation_result(project_root, operation),
        Err(error) => {
            return LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: input.project_root,
                summary: "Deployment down could not acquire operation.".to_string(),
                details: Some(json!({ "error": error.to_string() })),
                warnings: vec![error.to_string()],
            })
        }
    };
    let spec = match read_spec(project_root) {
        Ok(spec) => spec,
        Err(error) => {
            drop(guard);
            return LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: input.project_root,
                summary: "Deployment is not prepared; nothing was stopped.".to_string(),
                details: Some(json!({ "error": error.to_string() })),
                warnings: vec![],
            });
        }
    };
    let compose_file = from_project_relative(project_root, &spec.files.compose_path).ok();
    let output = compose_file.as_ref().and_then(|file| {
        Command::new("docker")
            .args(["compose", "-f"])
            .arg(file)
            .arg("down")
            .output()
            .ok()
    });
    let state_file = crate::paths::deployment_paths(project_root).state_file;
    let _ = write_json_atomic(
        &state_file,
        &json!({
            "schemaVersion": 1,
            "running": false,
            "updatedAt": state::store::now_string(),
            "lastDownExitCode": output.as_ref().and_then(|output| output.status.code())
        }),
    );
    drop(guard);
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: input.project_root,
        summary: "Deployment stop requested.".to_string(),
        details: Some(json!({
            "exitCode": output.as_ref().and_then(|output| output.status.code()),
            "stdoutTail": output.as_ref().map(|output| String::from_utf8_lossy(&output.stdout).lines().rev().take(40).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default(),
            "stderrTail": output.as_ref().map(|output| String::from_utf8_lossy(&output.stderr).lines().rev().take(40).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default(),
        })),
        warnings: vec![],
    })
}
