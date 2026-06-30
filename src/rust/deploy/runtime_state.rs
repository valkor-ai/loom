use std::{fs, path::Path};

use contracts::DeploymentSpec;
use serde_json::json;
use state::{
    paths::to_project_relative,
    store::{now_string, path_exists, write_json_atomic, StateResult},
};

use crate::{
    paths::deployment_paths, port_plan::primary_url, validate::DeploymentValidationResult,
};

pub fn write_success_state(
    project_root: &Path,
    spec: &DeploymentSpec,
    validation: &DeploymentValidationResult,
) -> StateResult<String> {
    let paths = deployment_paths(project_root);
    write_json_atomic(
        &paths.state_file,
        &json!({
            "schemaVersion": 1,
            "provider": spec.provider,
            "serviceName": spec.service_name,
            "projectRoot": spec.project_root,
            "specRef": to_project_relative(project_root, &paths.spec_file).ok(),
            "composePath": spec.files.compose_path,
            "running": true,
            "primaryUrl": primary_url(&spec.runtime),
            "ports": spec.runtime.ports.clone(),
            "preview": validation.preview,
            "apiRoutes": validation.api_routes,
            "updatedAt": now_string()
        }),
    )?;
    clear_failure_artifacts(project_root)?;
    to_project_relative(project_root, &paths.state_file)
}

pub fn clear_failure_artifacts(project_root: &Path) -> StateResult<()> {
    let paths = deployment_paths(project_root);
    remove_file_if_exists(&paths.failure_file)?;
    remove_file_if_exists(&paths.repair_action_file)?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> StateResult<()> {
    if path_exists(path) {
        fs::remove_file(path)?;
    }
    Ok(())
}
