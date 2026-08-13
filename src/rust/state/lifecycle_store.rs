use delivery_core::{
    DeliveryIndex, DeliveryLifecycleStatus, LoomCoreError, LoomResult, OperationLease,
    ProjectStatus, TransitionDiagnostic, TransitionStore,
};

use crate::{
    paths::{
        from_project_relative, operation_lease_file, project_paths, to_project_relative,
        transition_decision_file, transition_decision_latest_file,
    },
    project::initialize_project,
    store::{
        ensure_dir, now_millis, now_string, path_exists, read_json, write_json_atomic,
        write_text_atomic, StateError, StateResult,
    },
};

/// Finalize an accepted agent artifact by removing its writable candidate.
///
/// Candidates are inputs to acceptance, never another source of truth. Keeping
/// one shared finalizer prevents individual submit paths from drifting in how
/// they clean up accepted or repaired artifacts.
pub fn finalize_agent_candidate(
    project_root: &std::path::Path,
    relative_path: &str,
) -> StateResult<()> {
    let delivery_candidate = relative_path.starts_with(".loom/deliveries/")
        && relative_path.contains("/agent-writable/");
    if !relative_path.starts_with(".loom/agent-writable/") && !delivery_candidate {
        return Err(crate::store::StateError::InvalidArgument(
            "accepted candidate must be under .loom/agent-writable/".to_string(),
        ));
    }
    let candidate = from_project_relative(project_root, relative_path)?;
    crate::store::remove_file_if_exists(&candidate)?;
    if let Some(parent) = candidate.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitProjectStateResult {
    pub created: Vec<String>,
    pub already_existed: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileTransitionStore;

pub fn init_project_state(project_root: &str) -> StateResult<InitProjectStateResult> {
    let paths = project_paths(project_root)?;
    let already_existed = path_exists(&paths.loom_dir);
    let candidates = [
        &paths.loom_dir,
        &paths.deliveries_dir,
        &paths.tmp_dir,
        &paths.requests_dir,
        &paths.metrics_dir,
        &crate::paths::lifecycle_dir(&paths.root),
        &crate::paths::plan_requests_dir(&paths.root),
        &paths
            .root
            .join(crate::paths::LOOM_DIR)
            .join("plan-conflicts"),
        &paths.config_file,
        &paths.status_file,
        &paths.gitignore_file,
    ];
    let missing_status = !path_exists(&paths.status_file);
    let missing_gitignore = !path_exists(&paths.gitignore_file);
    let mut missing_before = candidates
        .iter()
        .filter(|path| !path_exists(path))
        .map(|path| path.to_path_buf())
        .collect::<Vec<_>>();

    initialize_project(project_root)?;

    if !path_exists(&paths.status_file) {
        write_json_atomic(&paths.status_file, &ProjectStatus::empty(now_string()))?;
    }
    if !path_exists(&paths.gitignore_file) {
        write_text_atomic(&paths.gitignore_file, "*\n!.gitignore\n")?;
    }

    let mut created = Vec::new();
    missing_before.sort();
    missing_before.dedup();
    for path in missing_before {
        if path_exists(&path) {
            created.push(to_project_relative(&paths.root, &path)?);
        }
    }
    if missing_status && !created.iter().any(|path| path == ".loom/status.json") {
        created.push(".loom/status.json".to_string());
    }
    if missing_gitignore && !created.iter().any(|path| path == ".loom/.gitignore") {
        created.push(".loom/.gitignore".to_string());
    }
    created.sort();
    created.dedup();

    Ok(InitProjectStateResult {
        created,
        already_existed,
    })
}

pub fn ensure_active_delivery(project_root: &str, delivery_id: &str) -> StateResult<()> {
    let store = FileTransitionStore;
    let status = store
        .load_status(project_root)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if status.active_delivery_id.as_deref() != Some(delivery_id) {
        return Err(StateError::StateCorrupted(format!(
            "STALE_DELIVERY_REQUEST: delivery {delivery_id} is not the active Loom delivery"
        )));
    }
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if matches!(
        delivery.status,
        DeliveryLifecycleStatus::Completed
            | DeliveryLifecycleStatus::CompletedWithOverride
            | DeliveryLifecycleStatus::Superseded
    ) {
        return Err(StateError::StateCorrupted(format!(
            "STALE_DELIVERY_REQUEST: delivery {delivery_id} is no longer active"
        )));
    }
    Ok(())
}

impl TransitionStore for FileTransitionStore {
    fn load_status(&self, project_root: &str) -> LoomResult<ProjectStatus> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        if !path_exists(&paths.config_file) || !path_exists(&paths.status_file) {
            return Err(LoomCoreError::failure(
                "STATE_NOT_INITIALIZED",
                format!("Loom is not initialized for {}.", paths.root.display()),
            ));
        }
        read_json(&paths.status_file)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))
    }

    fn save_status(&self, project_root: &str, status: &ProjectStatus) -> LoomResult<()> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        write_json_atomic(&paths.status_file, status)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))
    }

    fn load_delivery_index(
        &self,
        project_root: &str,
        delivery_id: &str,
    ) -> LoomResult<DeliveryIndex> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        let file = crate::paths::delivery_index_file(&paths.root, delivery_id);
        if !path_exists(&file) {
            return Err(LoomCoreError::failure(
                "DELIVERY_INDEX_CORRUPTED",
                format!("Delivery index does not exist for {delivery_id}."),
            ));
        }
        read_json(&file)
            .map_err(|error| LoomCoreError::failure("DELIVERY_INDEX_CORRUPTED", error.to_string()))
    }

    fn save_delivery_index(&self, project_root: &str, delivery: &DeliveryIndex) -> LoomResult<()> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        let file = crate::paths::delivery_index_file(&paths.root, &delivery.delivery_id);
        write_json_atomic(&file, delivery)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))
    }

    fn read_operation_lease(
        &self,
        project_root: &str,
        delivery_id: &str,
    ) -> LoomResult<Option<OperationLease>> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        let file = operation_lease_file(&paths.root, delivery_id);
        if !path_exists(&file) {
            return Ok(None);
        }
        read_json(&file)
            .map(Some)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))
    }

    fn write_operation_lease(
        &self,
        project_root: &str,
        delivery_id: &str,
        lease: &OperationLease,
    ) -> LoomResult<()> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        let file = operation_lease_file(&paths.root, delivery_id);
        write_json_atomic(&file, lease)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))
    }

    fn write_transition_diagnostic(
        &self,
        project_root: &str,
        diagnostic: &TransitionDiagnostic,
    ) -> LoomResult<()> {
        let paths = project_paths(project_root)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        let latest = transition_decision_latest_file(&paths.root, &diagnostic.delivery_id);
        if path_exists(&latest) {
            if let Ok(summary) = read_json::<serde_json::Value>(&latest) {
                if let Some(decision_ref) = summary.get("decisionRef").and_then(|v| v.as_str()) {
                    if let Ok(previous_file) = from_project_relative(&paths.root, decision_ref) {
                        if let Ok(previous) = read_json::<TransitionDiagnostic>(&previous_file) {
                            let same_decision = previous.delivery_id == diagnostic.delivery_id
                                && previous.phase_id == diagnostic.phase_id
                                && previous.source == diagnostic.source
                                && previous.route_action == diagnostic.route_action
                                && previous.result_state == diagnostic.result_state
                                && previous.target_batch == diagnostic.target_batch;
                            if same_decision {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        let file = transition_decision_file(
            &paths.root,
            &diagnostic.delivery_id,
            &diagnostic.decision_id,
        );
        if let Some(parent) = file.parent() {
            ensure_dir(parent)
                .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        }
        write_json_atomic(&file, diagnostic)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        let decision_ref = crate::paths::to_project_relative(&paths.root, &file)
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        write_json_atomic(
            &latest,
            &serde_json::json!({
                "schemaVersion": "1.0",
                "decisionId": diagnostic.decision_id,
                "decisionRef": decision_ref,
                "updatedAt": diagnostic.created_at
            }),
        )
        .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))
    }

    fn commit_delivery_state(
        &self,
        project_root: &str,
        delivery: &DeliveryIndex,
        status: &mut ProjectStatus,
    ) -> LoomResult<()> {
        let result = crate::commit_delivery(project_root, delivery.clone(), Some(status.revision))
            .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        *status = result.status;
        Ok(())
    }

    fn commit_operation_lease(
        &self,
        project_root: &str,
        delivery_id: &str,
        lease: &OperationLease,
        expected_revision: u64,
    ) -> LoomResult<u64> {
        let result = crate::commit_lifecycle(
            project_root,
            crate::LifecycleCommit {
                expected_revision: Some(expected_revision),
                expected_active_delivery_id: Some(Some(delivery_id.to_string())),
                leases: vec![lease.clone()],
                ..crate::LifecycleCommit::default()
            },
        )
        .map_err(|error| LoomCoreError::failure("STATE_ERROR", error.to_string()))?;
        Ok(result.revision)
    }

    fn now_millis(&self) -> u128 {
        now_millis()
    }

    fn now_string(&self) -> String {
        now_string()
    }
}
