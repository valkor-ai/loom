use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use delivery_core::{
    apply_delivery_index, DeliveryIndex, DeliveryLifecycleStatus, OperationLease,
    PlanConflictRecord, PlanConflictStatus, ProjectStatus, TransitionStore,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::{
    lifecycle_store::FileTransitionStore,
    paths::{
        delivery_index_file, lifecycle_dir, lifecycle_lock_file, lifecycle_transaction_file,
        operation_lease_file, plan_conflict_file, project_paths,
    },
    request_index::RequestIndex,
    store::{
        ensure_dir, now_millis, now_string, path_exists, read_json, write_json_atomic, StateError,
        StateResult,
    },
};

const LOCK_WAIT: Duration = Duration::from_secs(2);
const LOCK_RETRY: Duration = Duration::from_millis(20);

static PROCESS_PROJECT_LOCKS: OnceLock<(Mutex<HashMap<PathBuf, bool>>, Condvar)> = OnceLock::new();

struct ProcessProjectGuard {
    key: PathBuf,
}

impl Drop for ProcessProjectGuard {
    fn drop(&mut self) {
        let (locks, changed) =
            PROCESS_PROJECT_LOCKS.get_or_init(|| (Mutex::new(HashMap::new()), Condvar::new()));
        if let Ok(mut locks) = locks.lock() {
            locks.remove(&self.key);
            changed.notify_all();
        }
    }
}

struct LifecycleCommitGuard {
    _process_guard: ProcessProjectGuard,
    file: File,
}

impl Drop for LifecycleCommitGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleTransaction {
    schema_version: u32,
    transaction_id: String,
    expected_revision: u64,
    target_revision: u64,
    status: LifecycleTransactionStatus,
    project_status: ProjectStatus,
    deliveries: Vec<DeliveryIndex>,
    conflicts: Vec<PlanConflictRecord>,
    leases: Vec<OperationLease>,
    artifacts: Vec<PreparedArtifact>,
    cleanup_root: Option<PathBuf>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LifecycleTransactionStatus {
    Prepared,
    Applied,
}

#[derive(Debug, Clone, Default)]
pub struct LifecycleCommit {
    pub expected_revision: Option<u64>,
    pub expected_active_delivery_id: Option<Option<String>>,
    pub expected_active_phase_id: Option<String>,
    pub deliveries: Vec<DeliveryIndex>,
    pub conflicts: Vec<PlanConflictRecord>,
    pub leases: Vec<OperationLease>,
    pub pending_plan_conflict_id: Option<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct LifecycleCommitResult {
    pub status: ProjectStatus,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedArtifact {
    pub staged: PathBuf,
    pub target: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StagedProject {
    pub root: PathBuf,
    pub cleanup_root: PathBuf,
    committed: bool,
}

impl Drop for StagedProject {
    fn drop(&mut self) {
        if !self.committed && self.cleanup_root.exists() {
            let _ = fs::remove_dir_all(&self.cleanup_root);
        }
    }
}

impl StagedProject {
    pub fn into_commit_cleanup(mut self) -> PathBuf {
        let cleanup_root = self.cleanup_root.clone();
        self.committed = true;
        cleanup_root
    }
}

pub fn prepare_staged_project(
    project_root: &Path,
    transaction_id: &str,
) -> StateResult<StagedProject> {
    let cleanup_root = project_root.join(".loom").join("tmp").join(transaction_id);
    let staged_root = cleanup_root.join("project");
    ensure_dir(&staged_root)?;
    let source_config = project_paths(&project_root.to_string_lossy())?.config_file;
    let staged_config = staged_root.join(".loom").join("config.json");
    ensure_dir(staged_config.parent().expect("config has parent"))?;
    fs::copy(&source_config, &staged_config)?;
    crate::initialize_staged_project(&staged_root.to_string_lossy())?;
    Ok(StagedProject {
        root: staged_root,
        cleanup_root,
        committed: false,
    })
}

pub fn collect_prepared_artifacts(
    staged_root: &Path,
    project_root: &Path,
) -> StateResult<Vec<PreparedArtifact>> {
    let mut artifacts = Vec::new();
    collect_prepared_artifacts_inner(staged_root, staged_root, project_root, &mut artifacts)?;
    artifacts.sort_by(|left, right| left.target.cmp(&right.target));
    Ok(artifacts)
}

fn collect_prepared_artifacts_inner(
    root: &Path,
    current: &Path,
    project_root: &Path,
    artifacts: &mut Vec<PreparedArtifact>,
) -> StateResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_prepared_artifacts_inner(root, &path, project_root, artifacts)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| {
                StateError::StateCorrupted(format!(
                    "staged artifact escaped root: {}",
                    path.display()
                ))
            })?
            .to_path_buf();
        if relative == Path::new(".loom/config.json") || relative == Path::new(".loom/status.json")
        {
            continue;
        }
        artifacts.push(PreparedArtifact {
            staged: path,
            target: project_root.join(&relative),
        });
    }
    Ok(())
}

pub fn commit_lifecycle(
    project_root: &str,
    commit: LifecycleCommit,
) -> StateResult<LifecycleCommitResult> {
    commit_lifecycle_with_artifacts(project_root, commit, Vec::new(), None)
}

pub fn commit_lifecycle_with_artifacts(
    project_root: &str,
    commit: LifecycleCommit,
    artifacts: Vec<PreparedArtifact>,
    cleanup_root: Option<PathBuf>,
) -> StateResult<LifecycleCommitResult> {
    let root = project_paths(project_root)?.root;
    let _guard = acquire_lifecycle_lock(&root)?;
    recover_lifecycle_transaction_locked(&root)?;
    let store = FileTransitionStore;
    let mut status = store
        .load_status(project_root)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;

    if let Some(expected) = commit.expected_revision {
        if status.revision != expected {
            return Err(StateError::StateCorrupted(format!(
                "STALE_LIFECYCLE_REVISION: expected {expected}, current {}",
                status.revision
            )));
        }
    }
    if let Some(expected) = commit.expected_active_delivery_id.as_ref() {
        if &status.active_delivery_id != expected {
            return Err(StateError::StateCorrupted(format!(
                "STALE_DELIVERY_REQUEST: expected active delivery {expected:?}, current {:?}",
                status.active_delivery_id
            )));
        }
    }
    if let Some(expected_phase) = commit.expected_active_phase_id.as_deref() {
        let active_id = status.active_delivery_id.as_deref().ok_or_else(|| {
            StateError::StateCorrupted(
                "STALE_DELIVERY_REQUEST: no active delivery is available".to_string(),
            )
        })?;
        let active = store
            .load_delivery_index(project_root, active_id)
            .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
        if active.active_phase_id != expected_phase {
            return Err(StateError::StateCorrupted(format!(
                "STALE_SUBMIT_PHASE: expected {expected_phase}, current {}",
                active.active_phase_id
            )));
        }
    }

    if let Some(pending) = commit.pending_plan_conflict_id {
        status.pending_plan_conflict_id = pending;
    }
    persist_locked(
        &root,
        status,
        commit.deliveries,
        commit.conflicts,
        commit.leases,
        artifacts,
        cleanup_root,
    )
}

pub fn recover_lifecycle_transaction(project_root: &str) -> StateResult<()> {
    let root = project_paths(project_root)?.root;
    let _guard = acquire_lifecycle_lock(&root)?;
    recover_lifecycle_transaction_locked(&root)
}

pub fn commit_delivery(
    project_root: &str,
    delivery: DeliveryIndex,
    expected_revision: Option<u64>,
) -> StateResult<LifecycleCommitResult> {
    let delivery_id = delivery.delivery_id.clone();
    commit_lifecycle(
        project_root,
        LifecycleCommit {
            expected_revision,
            expected_active_delivery_id: Some(Some(delivery_id)),
            expected_active_phase_id: Some(delivery.active_phase_id.clone()),
            deliveries: vec![delivery],
            ..LifecycleCommit::default()
        },
    )
}

pub fn mutate_active_delivery<T>(
    project_root: &str,
    delivery_id: &str,
    phase_id: Option<&str>,
    mutate: impl FnOnce(&mut DeliveryIndex, &mut ProjectStatus) -> StateResult<T>,
) -> StateResult<T> {
    let root = project_paths(project_root)?.root;
    let _guard = acquire_lifecycle_lock(&root)?;
    recover_lifecycle_transaction_locked(&root)?;
    let store = FileTransitionStore;
    let mut status = store
        .load_status(project_root)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if status.active_delivery_id.as_deref() != Some(delivery_id) {
        return Err(StateError::StateCorrupted(format!(
            "STALE_DELIVERY_REQUEST: delivery {delivery_id} is not the active Loom delivery"
        )));
    }
    let mut delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if let Some(expected_phase) = phase_id {
        if delivery.active_phase_id != expected_phase {
            return Err(StateError::StateCorrupted(format!(
                "STALE_SUBMIT_PHASE: expected {expected_phase}, current {}",
                delivery.active_phase_id
            )));
        }
    }
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
    let value = mutate(&mut delivery, &mut status)?;
    persist_locked(
        &root,
        status,
        vec![delivery],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )?;
    Ok(value)
}

pub fn mutate_lifecycle<T>(
    project_root: &str,
    mutate: impl FnOnce(
        &mut ProjectStatus,
        &FileTransitionStore,
    ) -> StateResult<(
        T,
        Vec<DeliveryIndex>,
        Vec<PlanConflictRecord>,
        Vec<OperationLease>,
    )>,
) -> StateResult<T> {
    let root = project_paths(project_root)?.root;
    let _guard = acquire_lifecycle_lock(&root)?;
    recover_lifecycle_transaction_locked(&root)?;
    let store = FileTransitionStore;
    let mut status = store
        .load_status(project_root)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    let (value, deliveries, conflicts, leases) = mutate(&mut status, &store)?;
    persist_locked(
        &root,
        status,
        deliveries,
        conflicts,
        leases,
        Vec::new(),
        None,
    )?;
    Ok(value)
}

fn acquire_lifecycle_lock(root: &Path) -> StateResult<LifecycleCommitGuard> {
    ensure_dir(&lifecycle_dir(root))?;
    let key = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let (locks, changed) =
        PROCESS_PROJECT_LOCKS.get_or_init(|| (Mutex::new(HashMap::new()), Condvar::new()));
    let deadline = Instant::now() + LOCK_WAIT;
    let mut active = locks.lock().map_err(|_| {
        StateError::StateCorrupted("lifecycle mutex registry is poisoned".to_string())
    })?;
    while active.contains_key(&key) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(StateError::StateCorrupted(
                "LIFECYCLE_COMMIT_BUSY: another in-process lifecycle commit is in progress"
                    .to_string(),
            ));
        }
        let (next, timeout) = changed
            .wait_timeout(active, remaining.min(LOCK_RETRY))
            .map_err(|_| {
                StateError::StateCorrupted("lifecycle mutex registry is poisoned".to_string())
            })?;
        active = next;
        if timeout.timed_out() && Instant::now() >= deadline {
            return Err(StateError::StateCorrupted(
                "LIFECYCLE_COMMIT_BUSY: another in-process lifecycle commit is in progress"
                    .to_string(),
            ));
        }
    }
    active.insert(key.clone(), true);
    drop(active);
    let process_guard = ProcessProjectGuard { key };
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lifecycle_lock_file(root))?;
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(StateError::StateCorrupted(
                        "LIFECYCLE_COMMIT_BUSY: another lifecycle commit is in progress"
                            .to_string(),
                    ));
                }
                thread::sleep(LOCK_RETRY);
            }
            Err(error) => return Err(StateError::Io(error)),
        }
    }
    Ok(LifecycleCommitGuard {
        _process_guard: process_guard,
        file,
    })
}

fn recover_lifecycle_transaction_locked(root: &Path) -> StateResult<()> {
    let file = lifecycle_transaction_file(root);
    if !path_exists(&file) {
        return Ok(());
    }
    let transaction: LifecycleTransaction = read_json(&file)?;
    apply_transaction(root, &transaction)?;
    crate::store::remove_file_if_exists(&file)
}

fn persist_locked(
    root: &Path,
    mut status: ProjectStatus,
    deliveries: Vec<DeliveryIndex>,
    mut conflicts: Vec<PlanConflictRecord>,
    leases: Vec<OperationLease>,
    artifacts: Vec<PreparedArtifact>,
    cleanup_root: Option<PathBuf>,
) -> StateResult<LifecycleCommitResult> {
    let expected_revision = status.revision;
    let target_revision = status.revision.saturating_add(1);
    let mut ordered = deliveries.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|delivery| match delivery.status {
        DeliveryLifecycleStatus::Superseded
        | DeliveryLifecycleStatus::Completed
        | DeliveryLifecycleStatus::CompletedWithOverride => 0,
        _ => 1,
    });
    for delivery in ordered {
        apply_delivery_index(&mut status, delivery);
    }
    expire_terminal_delivery_conflict(root, &mut status, &mut conflicts)?;
    status.revision = target_revision;
    status.updated_at = now_string();
    validate_lifecycle_invariants(&status, &deliveries)?;
    let transaction = LifecycleTransaction {
        schema_version: 1,
        transaction_id: format!("lifecycle-tx-{}", now_millis()),
        expected_revision,
        target_revision,
        status: LifecycleTransactionStatus::Prepared,
        project_status: status.clone(),
        deliveries,
        conflicts,
        leases,
        artifacts,
        cleanup_root,
        created_at: now_string(),
    };
    write_json_atomic(&lifecycle_transaction_file(root), &transaction)?;
    apply_transaction(root, &transaction)?;
    crate::store::remove_file_if_exists(&lifecycle_transaction_file(root))?;
    Ok(LifecycleCommitResult {
        revision: target_revision,
        status,
    })
}

fn apply_transaction(root: &Path, transaction: &LifecycleTransaction) -> StateResult<()> {
    let request_index_target = project_paths(&root.to_string_lossy())?.request_index_file;
    for artifact in &transaction.artifacts {
        if let Some(parent) = artifact.target.parent() {
            ensure_dir(parent)?;
        }
        if artifact.target == request_index_target {
            merge_request_index_artifact(&artifact.staged, &artifact.target)?;
        } else if artifact
            .target
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            merge_jsonl_artifact(&artifact.staged, &artifact.target)?;
        } else if !artifact.staged.exists() && artifact.target.exists() {
            continue;
        } else if artifact.target.exists()
            && artifact.staged.is_file()
            && fs::read(&artifact.target).ok() == fs::read(&artifact.staged).ok()
        {
            fs::remove_file(&artifact.staged)?;
        } else {
            if artifact.target.exists() {
                if artifact.target.is_dir() {
                    fs::remove_dir_all(&artifact.target)?;
                } else {
                    fs::remove_file(&artifact.target)?;
                }
            }
            fs::rename(&artifact.staged, &artifact.target)?;
        }
    }
    for delivery in &transaction.deliveries {
        write_json_atomic(&delivery_index_file(root, &delivery.delivery_id), delivery)?;
    }
    for conflict in &transaction.conflicts {
        write_json_atomic(&plan_conflict_file(root, &conflict.conflict_id), conflict)?;
    }
    for lease in &transaction.leases {
        write_json_atomic(&operation_lease_file(root, &lease.delivery_id), lease)?;
    }
    write_json_atomic(&root.join(".loom/status.json"), &transaction.project_status)?;
    if let Some(cleanup_root) = transaction.cleanup_root.as_ref() {
        if cleanup_root.exists() {
            fs::remove_dir_all(cleanup_root)?;
        }
    }
    Ok(())
}

fn merge_request_index_artifact(staged: &Path, target: &Path) -> StateResult<()> {
    if !staged.exists() {
        return if target.exists() {
            Ok(())
        } else {
            Err(StateError::StateCorrupted(format!(
                "staged request index is missing: {}",
                staged.display()
            )))
        };
    }
    let staged_index: RequestIndex = read_json(staged)?;
    let mut merged = if path_exists(target) {
        read_json::<RequestIndex>(target)?
    } else {
        RequestIndex::empty()
    };
    for entry in staged_index.requests {
        if let Some(existing) = merged
            .requests
            .iter_mut()
            .find(|existing| existing.request_id == entry.request_id)
        {
            if existing.request_file != entry.request_file {
                return Err(StateError::StateCorrupted(format!(
                    "REQUEST_ID_COLLISION: {} maps to both {} and {}",
                    entry.request_id, existing.request_file, entry.request_file
                )));
            }
            *existing = entry;
        } else {
            merged.requests.push(entry);
        }
    }
    merged
        .requests
        .sort_by(|left, right| left.request_id.cmp(&right.request_id));
    write_json_atomic(target, &merged)?;
    fs::remove_file(staged)?;
    Ok(())
}

fn merge_jsonl_artifact(staged: &Path, target: &Path) -> StateResult<()> {
    if !staged.exists() {
        return Ok(());
    }
    let existing = fs::read_to_string(target).unwrap_or_default();
    let mut lines = existing.lines().map(str::to_string).collect::<Vec<_>>();
    let mut known = lines
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for line in fs::read_to_string(staged)?.lines() {
        if known.insert(line.to_string()) {
            lines.push(line.to_string());
        }
    }
    crate::store::write_text_atomic(target, &format!("{}\n", lines.join("\n")))?;
    fs::remove_file(staged)?;
    Ok(())
}

fn expire_terminal_delivery_conflict(
    root: &Path,
    status: &mut ProjectStatus,
    conflicts: &mut Vec<PlanConflictRecord>,
) -> StateResult<()> {
    let active_terminal = status.active_delivery_id.is_none();
    if !active_terminal {
        return Ok(());
    }
    let Some(conflict_id) = status.pending_plan_conflict_id.take() else {
        return Ok(());
    };
    let mut conflict = conflicts
        .iter()
        .find(|item| item.conflict_id == conflict_id)
        .cloned()
        .or_else(|| read_json(&plan_conflict_file(root, &conflict_id)).ok());
    if let Some(conflict) = conflict.as_mut() {
        if conflict.status == PlanConflictStatus::Pending {
            conflict.status = PlanConflictStatus::Expired;
            conflict.updated_at = now_string();
            conflicts.retain(|item| item.conflict_id != conflict.conflict_id);
            conflicts.push(conflict.clone());
        }
    }
    Ok(())
}

fn validate_lifecycle_invariants(
    status: &ProjectStatus,
    deliveries: &[DeliveryIndex],
) -> StateResult<()> {
    let Some(active_id) = status.active_delivery_id.as_deref() else {
        return Ok(());
    };
    let entry = status
        .deliveries
        .iter()
        .find(|entry| entry.delivery_id == active_id)
        .ok_or_else(|| {
            StateError::StateCorrupted(format!(
                "active delivery {active_id} is absent from project status"
            ))
        })?;
    if matches!(
        entry.status,
        DeliveryLifecycleStatus::Completed
            | DeliveryLifecycleStatus::CompletedWithOverride
            | DeliveryLifecycleStatus::Superseded
    ) {
        return Err(StateError::StateCorrupted(format!(
            "terminal delivery {active_id} cannot remain active"
        )));
    }
    if let Some(delivery) = deliveries.iter().find(|item| item.delivery_id == active_id) {
        if delivery.active_phase_id.is_empty() {
            return Err(StateError::StateCorrupted(
                "active delivery must identify an active phase".to_string(),
            ));
        }
    }
    Ok(())
}
