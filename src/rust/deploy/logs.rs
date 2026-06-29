use std::path::PathBuf;

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{
    paths::to_project_relative,
    store::{path_exists, read_text},
};

use crate::{paths::deployment_paths, DeployToolInput};

pub fn deploy_logs(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root_buf = PathBuf::from(&input.project_root);
    let project_root = project_root_buf.as_path();
    let project_root_display = input.project_root.clone();
    let paths = deployment_paths(project_root);
    let lines = if path_exists(&paths.log_file) {
        read_text(&paths.log_file)
            .unwrap_or_default()
            .lines()
            .rev()
            .take(120)
            .map(str::to_string)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root_display,
        summary: "Deployment logs loaded.".to_string(),
        details: Some(json!({
            "tail": lines,
            "errorWindow": lines.iter().rev().take(40).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
            "fullLogRef": to_project_relative(project_root, &paths.log_file).ok(),
        })),
        warnings: vec![],
    })
}
