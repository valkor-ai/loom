use std::path::{Path, PathBuf};

use state::paths::{delivery_dir, DeliveryPhaseLocator};

pub fn task_plan_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tasks")
        .join(&locator.phase_id)
        .join("requests")
        .join(format!("{request_id}.json"))
}

pub fn task_plan_outline_candidate_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("taskplan-outline.json")
}

pub fn task_plan_group_candidate_file(
    project_root: &Path,
    request_id: &str,
    group_id: &str,
) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("groups")
        .join(format!("{group_id}.json"))
}

pub fn task_plan_group_pattern(project_root: &Path, request_id: &str) -> PathBuf {
    task_plan_group_candidate_file(project_root, request_id, "{groupId}")
}

pub fn task_plan_dir(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tasks")
        .join(&locator.phase_id)
        .join("taskplans")
}

pub fn task_plan_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    task_plan_id: &str,
) -> PathBuf {
    task_plan_dir(project_root, locator).join(format!("{task_plan_id}.json"))
}

pub fn task_plan_latest_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    task_plan_dir(project_root, locator).join("latest.json")
}

pub fn task_plan_run_dir(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tasks")
        .join(&locator.phase_id)
        .join("runs")
}

pub fn task_plan_run_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    run_id: &str,
) -> PathBuf {
    task_plan_run_dir(project_root, locator).join(format!("{run_id}.json"))
}

pub fn task_plan_run_latest_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    task_plan_run_dir(project_root, locator).join("latest.json")
}

pub fn task_execution_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tasks")
        .join(&locator.phase_id)
        .join("execution-requests")
        .join(format!("{request_id}.json"))
}

pub fn task_execution_result_candidate_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("task-result.json")
}

pub fn task_result_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    run_id: &str,
    task_id: &str,
    result_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("tasks")
        .join(&locator.phase_id)
        .join("results")
        .join(run_id)
        .join(task_id)
        .join(format!("{result_id}.json"))
}

pub fn review_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("reviews")
        .join(&locator.phase_id)
        .join("requests")
        .join(format!("{request_id}.json"))
}

pub fn review_result_candidate_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("review-result.json")
}

pub fn review_result_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    review_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("reviews")
        .join(&locator.phase_id)
        .join("results")
        .join(format!("{review_id}.json"))
}

pub fn review_latest_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("reviews")
        .join(&locator.phase_id)
        .join("latest.json")
}

pub fn manual_review_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("reviews")
        .join(&locator.phase_id)
        .join("manual")
        .join(format!("{request_id}.json"))
}

pub fn manual_review_resolution_candidate_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("manual-review-resolution.json")
}

pub fn manual_review_resolution_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    resolution_id: &str,
) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("reviews")
        .join(&locator.phase_id)
        .join("manual-resolutions")
        .join(format!("{resolution_id}.json"))
}
