use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    paths::project_paths,
    store::{now_string, path_exists, read_json, write_json_atomic, StateError, StateResult},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestIndex {
    pub schema_version: u32,
    pub requests: Vec<RequestIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestIndexEntry {
    pub request_id: String,
    pub request_ref: String,
    pub request_file: String,
    pub delivery_id: Option<String>,
    pub phase_id: Option<String>,
    pub request_kind: String,
    pub source_protocol: RequestSourceProtocol,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestSourceProtocol {
    RustMcpNative,
}

impl RequestIndex {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            requests: vec![],
        }
    }
}

pub fn load_request_index(project_root: &str) -> StateResult<RequestIndex> {
    let paths = project_paths(project_root)?;
    if !path_exists(&paths.request_index_file) {
        return Ok(RequestIndex::empty());
    }
    read_json(&paths.request_index_file)
}

pub fn save_request_index(project_root: &str, index: &RequestIndex) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    write_json_atomic(&paths.request_index_file, index)
}

pub fn upsert_request_index_entry(
    project_root: &str,
    mut entry: RequestIndexEntry,
) -> StateResult<RequestIndexEntry> {
    validate_request_id(&entry.request_id)?;
    let mut index = load_request_index(project_root)?;
    let now = now_string();
    if let Some(existing) = index
        .requests
        .iter_mut()
        .find(|existing| existing.request_id == entry.request_id)
    {
        if existing.request_file != entry.request_file {
            return Err(StateError::InvalidArgument(format!(
                "REQUEST_ID_COLLISION: {} maps to both {} and {}",
                entry.request_id, existing.request_file, entry.request_file
            )));
        }
        entry.created_at = existing.created_at.clone();
        entry.updated_at = now;
        *existing = entry.clone();
    } else {
        if entry.created_at.is_empty() {
            entry.created_at = now.clone();
        }
        entry.updated_at = now;
        index.requests.push(entry.clone());
    }
    index
        .requests
        .sort_by(|left, right| left.request_id.cmp(&right.request_id));
    save_request_index(project_root, &index)?;
    Ok(entry)
}

pub fn get_request_index_entry(
    project_root: &str,
    request_id: &str,
) -> StateResult<RequestIndexEntry> {
    let index = load_request_index(project_root)?;
    index
        .requests
        .into_iter()
        .find(|entry| entry.request_id == request_id)
        .ok_or_else(|| StateError::InvalidArgument(format!("requestId not found: {request_id}")))
}

pub fn validate_request_id(request_id: &str) -> StateResult<()> {
    if !(3..=160).contains(&request_id.len())
        || !request_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':'))
    {
        return Err(StateError::InvalidArgument(format!(
            "invalid requestId: {request_id}"
        )));
    }
    Ok(())
}
