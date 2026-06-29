use std::{path::Path, process};

use delivery_core::{ActiveOperationRef, LoomMcpActionResult, LoomMcpActiveOperationResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use state::{
    paths::to_project_relative,
    store::{now_millis, now_string, path_exists, read_json, write_json_atomic, StateResult},
};

use crate::paths::deployment_paths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentActiveOperation {
    pub schema_version: u32,
    pub operation_id: String,
    pub command: String,
    pub phase: String,
    pub pid: u32,
    pub project_root: String,
    pub started_at: String,
    pub updated_at: String,
    pub log_ref: String,
    pub spec_ref: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct DeploymentOperationGuard {
    project_root: String,
    operation_id: String,
    owns_operation: bool,
}

impl Drop for DeploymentOperationGuard {
    fn drop(&mut self) {
        if !self.owns_operation {
            return;
        }
        let root = Path::new(&self.project_root);
        let paths = deployment_paths(root);
        let existing = read_json::<DeploymentActiveOperation>(&paths.active_operation_file).ok();
        if existing
            .as_ref()
            .map(|operation| operation.operation_id.as_str())
            == Some(self.operation_id.as_str())
        {
            let _ = std::fs::remove_file(paths.active_operation_file);
        }
    }
}

pub fn active_operation_result(
    project_root: &Path,
    operation: DeploymentActiveOperation,
) -> LoomMcpActionResult {
    LoomMcpActionResult::ActiveOperation(LoomMcpActiveOperationResult {
        project_root: project_root.to_string_lossy().into_owned(),
        operation: ActiveOperationRef {
            operation_id: operation.operation_id,
            operation_type: operation.command,
            delivery_id: None,
            phase_id: None,
            started_at: operation.started_at,
            expires_at: (now_millis() + 30 * 60 * 1000).to_string(),
        },
        allowed_observation_tools: vec![
            "loom.deployStatus".to_string(),
            "loom.deployInspect".to_string(),
            "loom.deployLogs".to_string(),
        ],
        progress_summary: Some(json!({
            "phase": operation.phase,
            "logRef": operation.log_ref,
            "specRef": operation.spec_ref,
            "status": operation.status,
        })),
    })
}

pub fn acquire_operation(
    project_root: &Path,
    command: &str,
    phase: &str,
) -> StateResult<Result<DeploymentOperationGuard, DeploymentActiveOperation>> {
    let paths = deployment_paths(project_root);
    state::store::ensure_dir(&paths.state_dir)?;
    state::store::ensure_dir(&paths.logs_dir)?;
    if let Some(existing) = live_operation(project_root)? {
        return Ok(Err(existing));
    }
    let operation_id = format!("deploy_op_{}", now_millis());
    let operation = DeploymentActiveOperation {
        schema_version: 1,
        operation_id: operation_id.clone(),
        command: command.to_string(),
        phase: phase.to_string(),
        pid: process::id(),
        project_root: project_root.to_string_lossy().into_owned(),
        started_at: now_string(),
        updated_at: now_string(),
        log_ref: to_project_relative(project_root, &paths.log_file)?,
        spec_ref: if path_exists(&paths.spec_file) {
            Some(to_project_relative(project_root, &paths.spec_file)?)
        } else {
            None
        },
        status: "running".to_string(),
    };
    write_json_atomic(&paths.active_operation_file, &operation)?;
    Ok(Ok(DeploymentOperationGuard {
        project_root: project_root.to_string_lossy().into_owned(),
        operation_id,
        owns_operation: true,
    }))
}

pub fn live_operation(project_root: &Path) -> StateResult<Option<DeploymentActiveOperation>> {
    let paths = deployment_paths(project_root);
    if !path_exists(&paths.active_operation_file) {
        return Ok(None);
    }
    let mut operation: DeploymentActiveOperation = read_json(&paths.active_operation_file)?;
    let updated = operation.updated_at.parse::<u128>().unwrap_or(0);
    let stale = updated > 0 && now_millis().saturating_sub(updated) > 30 * 60 * 1000;
    if stale {
        operation.status = "stale".to_string();
        operation.updated_at = now_string();
        write_json_atomic(&paths.stale_operation_file, &operation)?;
        let _ = std::fs::remove_file(paths.active_operation_file);
        return Ok(None);
    }
    Ok(Some(operation))
}
