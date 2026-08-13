use std::path::{Path, PathBuf};

use delivery_core::{
    canonical_plan_request, CanonicalPlanRequest, PlanConflictRecord, PlanConflictStatus,
    ValidatedPlanInput,
};

use crate::{
    paths::{from_project_relative, plan_conflict_file, project_paths},
    store::{now_string, read_json, write_json_atomic, StateError, StateResult},
};

pub fn persist_plan_request(input: &ValidatedPlanInput) -> StateResult<String> {
    let paths = project_paths(&input.project_root)?;
    let request =
        canonical_plan_request(&paths.root, &input.request_text, &input.requirement_files)
            .map_err(StateError::InvalidArgument)?;
    let file = from_project_relative(&paths.root, &input.request_identity.request_ref)?;
    if file.exists() {
        let existing: CanonicalPlanRequest = read_json(&file)?;
        if existing != request {
            return Err(StateError::StateCorrupted(format!(
                "canonical plan request fingerprint collision: {}",
                input.request_identity.fingerprint
            )));
        }
    } else {
        write_json_atomic(&file, &request)?;
    }
    Ok(input.request_identity.request_ref.clone())
}

pub fn load_plan_request(
    project_root: &str,
    request_ref: &str,
) -> StateResult<CanonicalPlanRequest> {
    let paths = project_paths(project_root)?;
    read_json(&from_project_relative(&paths.root, request_ref)?)
}

pub fn create_or_load_plan_conflict(
    project_root: &str,
    active_delivery_id: &str,
    active_revision: u64,
    input: &ValidatedPlanInput,
) -> StateResult<PlanConflictRecord> {
    let paths = project_paths(project_root)?;
    persist_plan_request(input)?;
    let conflict_id = format!(
        "plan-conflict-{}-{}",
        active_delivery_id,
        input.request_identity.fingerprint.replace(':', "-")
    );
    let file = plan_conflict_file(&paths.root, &conflict_id);
    if file.exists() {
        let existing: PlanConflictRecord = read_json(&file)?;
        if existing.active_delivery_id == active_delivery_id
            && existing.incoming_request_fingerprint == input.request_identity.fingerprint
            && existing.incoming_request_ref == input.request_identity.request_ref
        {
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
        active_revision,
        incoming_request_ref: input.request_identity.request_ref.clone(),
        incoming_request_fingerprint: input.request_identity.fingerprint.clone(),
        status: PlanConflictStatus::Pending,
        created_at: now.clone(),
        updated_at: now,
    };
    Ok(record)
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

pub fn staged_project_root(project_root: &Path, transaction_id: &str) -> PathBuf {
    project_root
        .join(".loom")
        .join("tmp")
        .join(transaction_id)
        .join("project")
}

pub fn conflict_ref(record: &PlanConflictRecord) -> String {
    format!(".loom/plan-conflicts/{}.json", record.conflict_id)
}
