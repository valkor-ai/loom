use std::path::{Path, PathBuf};

use delivery_core::normalize_project_root;

use crate::store::{StateError, StateResult};

pub const LOOM_DIR: &str = ".loom";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub loom_dir: PathBuf,
    pub config_file: PathBuf,
    pub status_file: PathBuf,
    pub gitignore_file: PathBuf,
    pub deliveries_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub requests_dir: PathBuf,
    pub request_index_file: PathBuf,
    pub metrics_dir: PathBuf,
    pub request_size_audit_file: PathBuf,
    pub field_read_audit_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryPhaseLocator {
    pub delivery_id: String,
    pub phase_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryPhaseRunLocator {
    pub delivery_id: String,
    pub phase_id: String,
    pub run_id: String,
}

pub fn project_paths(project_root: &str) -> StateResult<ProjectPaths> {
    let normalized = normalize_project_root(project_root).map_err(StateError::InvalidArgument)?;
    let loom_dir = normalized.path.join(LOOM_DIR);
    let deliveries_dir = loom_dir.join("deliveries");
    let tmp_dir = loom_dir.join("tmp");
    let requests_dir = loom_dir.join("requests");
    let metrics_dir = loom_dir.join("metrics");
    Ok(ProjectPaths {
        root: normalized.path,
        status_file: loom_dir.join("status.json"),
        gitignore_file: loom_dir.join(".gitignore"),
        deliveries_dir,
        tmp_dir,
        config_file: loom_dir.join("config.json"),
        request_index_file: requests_dir.join("index.json"),
        request_size_audit_file: metrics_dir.join("request-size-audit.jsonl"),
        field_read_audit_file: metrics_dir.join("field-read-audit.jsonl"),
        loom_dir,
        requests_dir,
        metrics_dir,
    })
}

pub fn to_project_relative(project_root: &Path, absolute_path: &Path) -> StateResult<String> {
    let relative = absolute_path.strip_prefix(project_root).map_err(|_| {
        StateError::InvalidArgument(format!(
            "path is outside project root: {}",
            absolute_path.display()
        ))
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

pub fn from_project_relative(project_root: &Path, relative_path: &str) -> StateResult<PathBuf> {
    if relative_path.trim().is_empty() {
        return Err(StateError::InvalidArgument(
            "project-relative path is required".to_string(),
        ));
    }
    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return Err(StateError::InvalidArgument(
            "project-relative path must not be absolute".to_string(),
        ));
    }
    if relative
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(StateError::InvalidArgument(
            "project-relative path must not contain '..'".to_string(),
        ));
    }
    Ok(project_root.join(relative))
}

pub fn request_file_for_id(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(LOOM_DIR)
        .join("requests")
        .join(format!("{request_id}.json"))
}

pub fn shared_request_refs_dir(project_root: &Path) -> PathBuf {
    project_root.join(LOOM_DIR).join("refs").join("requests")
}

pub fn request_storage_manifest_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(LOOM_DIR)
        .join("requests")
        .join(format!("{request_id}.manifest.json"))
}

pub fn delivery_dir(project_root: &Path, delivery_id: &str) -> PathBuf {
    project_root
        .join(LOOM_DIR)
        .join("deliveries")
        .join(delivery_id)
}

pub fn delivery_index_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id).join("index.json")
}

pub fn phase_tmp_dir(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tmp")
        .join(&locator.phase_id)
}

pub fn workspace_dir(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("workspace")
        .join(&locator.phase_id)
}

pub fn task_run_file(project_root: &Path, locator: &DeliveryPhaseRunLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tasks")
        .join(&locator.phase_id)
        .join("runs")
        .join(format!("{}.json", locator.run_id))
}

pub fn operation_lease_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id)
        .join("operations")
        .join("active-lease.json")
}

pub fn transition_decisions_dir(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id).join("transition-decisions")
}

pub fn transition_decision_file(
    project_root: &Path,
    delivery_id: &str,
    decision_id: &str,
) -> PathBuf {
    transition_decisions_dir(project_root, delivery_id).join(format!("{decision_id}.json"))
}

pub fn transition_decision_latest_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    transition_decisions_dir(project_root, delivery_id).join("latest.json")
}
