use std::path::PathBuf;

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{
    paths::to_project_relative,
    store::{path_exists, read_json_value},
};

use crate::{
    active_operation::{active_operation_result, live_operation},
    paths::deployment_paths,
    prepare::read_spec,
    DeployToolInput,
};

pub fn deploy_inspect(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root_buf = PathBuf::from(&input.project_root);
    let project_root = project_root_buf.as_path();
    let project_root_display = input.project_root.clone();
    if let Ok(Some(operation)) = live_operation(project_root) {
        return active_operation_result(project_root, operation);
    }
    let paths = deployment_paths(project_root);
    let spec = read_spec(project_root).ok();
    let repair = if path_exists(&paths.repair_file) {
        read_json_value(&paths.repair_file).ok()
    } else {
        None
    };
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root_display,
        summary: "Deployment inspect loaded.".to_string(),
        details: Some(json!({
            "specRef": spec.as_ref().and_then(|_| to_project_relative(project_root, &paths.spec_file).ok()),
            "sourceModel": spec.as_ref().map(|spec| &spec.source_model),
            "topology": spec.as_ref().map(|spec| &spec.topology),
            "files": spec.as_ref().map(|spec| &spec.files),
            "codeEvidenceRef": spec.as_ref().map(|spec| &spec.code_evidence_ref),
            "stateRef": path_exists(&paths.state_file).then(|| to_project_relative(project_root, &paths.state_file).ok()).flatten(),
            "repair": repair,
        })),
        warnings: vec![],
    })
}
