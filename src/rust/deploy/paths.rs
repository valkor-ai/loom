use std::path::{Path, PathBuf};

use state::paths::LOOM_DIR;

#[derive(Debug, Clone)]
pub struct DeploymentPaths {
    pub specs_dir: PathBuf,
    pub state_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub repairs_dir: PathBuf,
    pub evidence_dir: PathBuf,
    pub spec_file: PathBuf,
    pub state_file: PathBuf,
    pub active_operation_file: PathBuf,
    pub stale_operation_file: PathBuf,
    pub log_file: PathBuf,
    pub repair_action_file: PathBuf,
    pub failure_file: PathBuf,
    pub code_evidence_file: PathBuf,
    pub generated_dir: PathBuf,
    pub compose_file: PathBuf,
    pub dockerignore_file: PathBuf,
}

pub fn deployment_paths(project_root: &Path) -> DeploymentPaths {
    let deployment_dir = project_root.join(LOOM_DIR).join("deployment");
    let specs_dir = deployment_dir.join("specs");
    let state_dir = deployment_dir.join("state");
    let logs_dir = deployment_dir.join("logs");
    let repairs_dir = deployment_dir.join("repairs");
    let evidence_dir = deployment_dir.join("evidence");
    let generated_dir = specs_dir.join("generated");
    DeploymentPaths {
        spec_file: specs_dir.join("local.json"),
        state_file: state_dir.join("local.json"),
        active_operation_file: state_dir.join("active-operation.json"),
        stale_operation_file: state_dir.join("last-stale-operation.json"),
        log_file: logs_dir.join("local.log"),
        repair_action_file: state_dir.join("repair-action.json"),
        failure_file: state_dir.join("latest-failure.json"),
        code_evidence_file: evidence_dir.join("latest-code-evidence.json"),
        compose_file: generated_dir.join("compose.yaml"),
        dockerignore_file: generated_dir.join("Dockerfile.dockerignore"),
        specs_dir,
        state_dir,
        logs_dir,
        repairs_dir,
        evidence_dir,
        generated_dir,
    }
}

pub fn dockerfile_path(project_root: &Path, service_id: &str) -> PathBuf {
    deployment_paths(project_root)
        .generated_dir
        .join(format!("Dockerfile.{service_id}"))
}

pub fn dockerfile_ignore_path(project_root: &Path, service_id: &str) -> PathBuf {
    deployment_paths(project_root)
        .generated_dir
        .join(format!("Dockerfile.{service_id}.dockerignore"))
}

pub fn nginx_config_path(project_root: &Path, service_id: &str) -> PathBuf {
    deployment_paths(project_root)
        .generated_dir
        .join(format!("nginx.{service_id}.conf"))
}

pub fn deploy_execution_repair_result_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(LOOM_DIR)
        .join("agent-writable")
        .join(request_id)
        .join("deploy-execution-repair-result.json")
}

pub fn deploy_execution_repair_action_file(project_root: &Path, request_id: &str) -> PathBuf {
    deployment_paths(project_root)
        .repairs_dir
        .join(request_id)
        .join("request.json")
}
