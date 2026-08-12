use std::{
    fs::{self, OpenOptions},
    thread,
    time::Duration,
};

use delivery_core::{
    mark_delivery_superseded, DeliveryLifecycleStatus, PlanConflictRecord, PlanConflictStatus,
    PlanRequestIdentity, PlanSwitchJournal, PlanSwitchStatus, TransitionStore, ValidatedPlanInput,
};

use crate::{
    paths::{plan_conflict_file, plan_conflicts_dir, plan_switch_journal_file, project_paths},
    store::{ensure_dir, now_string, read_json, write_json_atomic, StateError, StateResult},
};

pub fn with_plan_transition_lock<T>(
    project_root: &str,
    operation: impl FnOnce() -> StateResult<T>,
) -> StateResult<T> {
    let paths = project_paths(project_root)?;
    ensure_dir(&paths.loom_dir)?;
    let lock_file = paths.root.join(".loom").join("plan-transition.lock");
    for _ in 0..500 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_file)
        {
            Ok(file) => {
                let _ = file.sync_all();
                let _guard = PlanTransitionLockGuard {
                    path: lock_file.clone(),
                };
                return operation();
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(&lock_file)
                    .and_then(|metadata| metadata.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(300));
                if stale {
                    let _ = fs::remove_file(&lock_file);
                    continue;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(StateError::Io(error)),
        }
    }
    Err(StateError::StateCorrupted(
        "plan transition lock is held by another Loom process".to_string(),
    ))
}

struct PlanTransitionLockGuard {
    path: std::path::PathBuf,
}

impl Drop for PlanTransitionLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn create_or_load_plan_conflict(
    project_root: &str,
    active_delivery_id: &str,
    input: &ValidatedPlanInput,
) -> StateResult<PlanConflictRecord> {
    let paths = project_paths(project_root)?;
    ensure_dir(&plan_conflicts_dir(&paths.root))?;
    let conflict_id = format!(
        "plan-conflict-{}-{}",
        active_delivery_id,
        input.request_identity.fingerprint.replace(':', "-")
    );
    let file = plan_conflict_file(&paths.root, &conflict_id);
    if file.exists() {
        let mut existing: PlanConflictRecord = read_json(&file)?;
        if existing.active_delivery_id == active_delivery_id
            && existing.incoming_request == input.request_identity
        {
            // A previous choice of "continue current" does not consume the
            // request forever. The same pending request may be submitted
            // again while the original delivery is still active; reopen the
            // canonical conflict record instead of creating an error or a
            // second conflict file.
            if existing.status != PlanConflictStatus::Pending {
                existing.status = PlanConflictStatus::Pending;
                existing.updated_at = now_string();
                write_json_atomic(&file, &existing)?;
            }
            return Ok(existing);
        }
        return Err(StateError::StateCorrupted(format!(
            "plan conflict identity collision: {conflict_id}"
        )));
    }
    let now = now_string();
    let record = PlanConflictRecord {
        schema_version: 1,
        conflict_id,
        active_delivery_id: active_delivery_id.to_string(),
        incoming_request: input.request_identity.clone(),
        request_text: input.request_text.clone(),
        requirement_files: input.requirement_files.clone(),
        status: PlanConflictStatus::Pending,
        created_at: now.clone(),
        updated_at: now,
    };
    write_json_atomic(&file, &record)?;
    Ok(record)
}

pub fn begin_plan_switch(
    project_root: &str,
    conflict_id: &str,
    old_delivery_id: &str,
    incoming_request: &PlanRequestIdentity,
) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    write_json_atomic(
        &plan_switch_journal_file(&paths.root),
        &PlanSwitchJournal {
            schema_version: 1,
            conflict_id: conflict_id.to_string(),
            old_delivery_id: old_delivery_id.to_string(),
            incoming_request: incoming_request.clone(),
            new_delivery_id: None,
            status: PlanSwitchStatus::Preparing,
            updated_at: now_string(),
        },
    )
}

pub fn mark_plan_switch_new_delivery(project_root: &str, new_delivery_id: &str) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    let file = plan_switch_journal_file(&paths.root);
    let mut journal: PlanSwitchJournal = read_json(&file)?;
    journal.new_delivery_id = Some(new_delivery_id.to_string());
    journal.status = PlanSwitchStatus::NewDeliveryCreated;
    journal.updated_at = now_string();
    write_json_atomic(&file, &journal)
}

pub fn complete_plan_switch(project_root: &str) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    crate::store::remove_file_if_exists(&plan_switch_journal_file(&paths.root))
}

pub fn recover_plan_switch(project_root: &str) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    let file = plan_switch_journal_file(&paths.root);
    if !file.exists() {
        return Ok(());
    }
    let journal: PlanSwitchJournal = read_json(&file)?;
    let store = crate::lifecycle_store::FileTransitionStore;
    let mut status = store
        .load_status(project_root)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    let new_delivery_id = journal
        .new_delivery_id
        .clone()
        .or_else(|| {
            status
                .active_delivery_id
                .clone()
                .filter(|delivery_id| delivery_id != &journal.old_delivery_id)
        })
        .or_else(|| {
            let candidates = fs::read_dir(&paths.deliveries_dir)
                .ok()?
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let delivery_id = entry.file_name().to_string_lossy().into_owned();
                    if delivery_id == journal.old_delivery_id {
                        return None;
                    }
                    let index_file = entry.path().join("index.json");
                    let index: delivery_core::DeliveryIndex = read_json(&index_file).ok()?;
                    (index.request_identity.as_ref() == Some(&journal.incoming_request))
                        .then_some(delivery_id)
                })
                .collect::<Vec<_>>();
            (candidates.len() == 1).then_some(candidates[0].clone())
        });
    let Some(new_delivery_id) = new_delivery_id else {
        // The journal is written before the new delivery is created. A
        // restart at this point is safe to retry from the still-active old
        // delivery, so remove only the incomplete journal.
        if status.active_delivery_id.as_deref() == Some(journal.old_delivery_id.as_str()) {
            complete_plan_switch(project_root)?;
            return Ok(());
        }
        return Err(StateError::StateCorrupted(
            "plan switch journal is incomplete and no unique new delivery can be identified"
                .to_string(),
        ));
    };
    let new_delivery = store
        .load_delivery_index(project_root, &new_delivery_id)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if new_delivery.request_identity.as_ref() != Some(&journal.incoming_request) {
        return Err(StateError::StateCorrupted(
            "plan switch journal new delivery does not match the pending request".to_string(),
        ));
    }
    if !matches!(
        new_delivery.status,
        DeliveryLifecycleStatus::Planning
            | DeliveryLifecycleStatus::Executing
            | DeliveryLifecycleStatus::Reviewing
            | DeliveryLifecycleStatus::Repairing
            | DeliveryLifecycleStatus::Blocked
    ) {
        return Err(StateError::StateCorrupted(
            "plan switch journal points to a non-active new delivery".to_string(),
        ));
    }
    let mut old_delivery = store
        .load_delivery_index(project_root, &journal.old_delivery_id)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    old_delivery.status = DeliveryLifecycleStatus::Superseded;
    old_delivery.updated_at = now_string();
    store
        .save_delivery_index(project_root, &old_delivery)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if let Ok(Some(mut lease)) = store.read_operation_lease(project_root, &journal.old_delivery_id)
    {
        lease.close(now_string());
        store
            .write_operation_lease(project_root, &journal.old_delivery_id, &lease)
            .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    }
    mark_delivery_superseded(&mut status, &journal.old_delivery_id, now_string());
    status.active_delivery_id = Some(new_delivery_id);
    status.pending_plan_conflict_id = None;
    store
        .save_status(project_root, &status)
        .map_err(|error| StateError::StateCorrupted(error.to_string()))?;
    if let Ok(mut conflict) = load_plan_conflict(project_root, &journal.conflict_id) {
        conflict.status = PlanConflictStatus::ResolvedStartNew;
        conflict.updated_at = now_string();
        save_plan_conflict(project_root, &conflict)?;
    }
    complete_plan_switch(project_root)
}

pub fn load_plan_conflict(
    project_root: &str,
    conflict_id: &str,
) -> StateResult<PlanConflictRecord> {
    let paths = project_paths(project_root)?;
    read_json(&plan_conflict_file(&paths.root, conflict_id))
}

pub fn save_plan_conflict(project_root: &str, record: &PlanConflictRecord) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    write_json_atomic(
        &plan_conflict_file(&paths.root, &record.conflict_id),
        record,
    )
}

pub fn expire_plan_conflict(project_root: &str, conflict_id: &str) -> StateResult<()> {
    let mut conflict = load_plan_conflict(project_root, conflict_id)?;
    if conflict.status == PlanConflictStatus::Pending {
        conflict.status = PlanConflictStatus::Expired;
        conflict.updated_at = now_string();
        save_plan_conflict(project_root, &conflict)?;
    }
    Ok(())
}

pub fn conflict_id_from_ref(value: &str) -> StateResult<String> {
    let trimmed = value.trim();
    let id = trimmed
        .strip_prefix(".loom/plan-conflicts/")
        .and_then(|value| value.strip_suffix(".json"))
        .or_else(|| trimmed.strip_suffix(".json"))
        .unwrap_or(trimmed);
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(StateError::InvalidArgument(
            "conflictRef must identify a Loom plan conflict".to_string(),
        ));
    }
    Ok(id.to_string())
}
