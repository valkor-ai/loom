use std::path::{Path, PathBuf};

use state::paths::{delivery_dir, workspace_dir, DeliveryPhaseLocator};

pub fn technical_baseline_dir(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id).join("contracts")
}

pub fn technical_baseline_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    technical_baseline_dir(project_root, delivery_id).join("technical-baseline.json")
}

pub fn technical_baseline_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    technical_baseline_dir(project_root, &locator.delivery_id)
        .join("technical-baseline-requests")
        .join(format!("{request_id}.json"))
}

pub fn technical_baseline_candidate_file(
    project_root: &Path,
    _locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("technical-baseline-candidate.json")
}

pub fn repository_context_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    workspace_dir(project_root, locator)
        .join("repository-context-requests")
        .join(format!("{request_id}.json"))
}

pub fn repository_context_candidate_file(
    project_root: &Path,
    _locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("repository-context-candidate.json")
}

pub fn repository_context_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    workspace_dir(project_root, locator).join("repository-context.json")
}

pub fn planning_dir(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("contracts")
        .join("planning")
        .join(&locator.phase_id)
}

pub fn planning_contract_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    planning_dir(project_root, locator).join("pgc.json")
}

pub fn planning_latest_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    planning_dir(project_root, locator).join("latest.json")
}
