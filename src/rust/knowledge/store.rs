use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local, TimeZone, Utc};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

use crate::{
    models::{KnowledgeRegistry, PendingQueue},
    paths,
};

#[derive(Debug, Clone)]
pub struct PendingQueueRecord {
    pub file: PathBuf,
    pub queue: PendingQueue,
}

pub type KnowledgeResult<T> = Result<T, KnowledgeError>;

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("{0}")]
    Invalid(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

impl KnowledgeError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

pub fn ensure_dir(path: &Path) -> KnowledgeResult<()> {
    if path.exists() {
        if !path.is_dir() {
            return Err(KnowledgeError::invalid(format!(
                "path exists but is not a directory: {}",
                path.display()
            )));
        }
        return Ok(());
    }
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> KnowledgeResult<T> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn read_json_or<T: DeserializeOwned>(path: &Path, fallback: T) -> KnowledgeResult<T> {
    if !path.exists() {
        return Ok(fallback);
    }
    read_json(path)
}

pub fn write_json(path: &Path, value: &impl Serialize) -> KnowledgeResult<()> {
    let text = format!("{}\n", serde_json::to_string_pretty(value)?);
    write_text(path, &text)
}

pub fn write_text(path: &Path, text: &str) -> KnowledgeResult<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let tmp = tmp_path(path);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn remove_file_if_exists(path: &Path) -> KnowledgeResult<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn remove_dir_if_exists(path: &Path) -> KnowledgeResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub fn load_registry() -> KnowledgeResult<KnowledgeRegistry> {
    read_json_or(&paths::registry_file()?, KnowledgeRegistry::empty())
}

pub fn save_registry(registry: &KnowledgeRegistry) -> KnowledgeResult<()> {
    write_json(&paths::registry_file()?, registry)
}

pub fn load_pending(source_id: &str, source_name: &str) -> KnowledgeResult<PendingQueue> {
    read_json_or(
        &paths::pending_file(source_id)?,
        PendingQueue::empty(source_id, source_name),
    )
}

pub fn save_pending(queue: &PendingQueue) -> KnowledgeResult<()> {
    write_json(&paths::pending_file(&queue.source_id)?, queue)
}

pub fn list_pending_records() -> KnowledgeResult<Vec<PendingQueueRecord>> {
    let pending_dir = paths::pending_dir()?;
    if !pending_dir.exists() {
        return Ok(vec![]);
    }
    let mut records = Vec::new();
    for entry in fs::read_dir(pending_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        records.push(PendingQueueRecord {
            file: entry.path(),
            queue: read_json(&entry.path())?,
        });
    }
    records.sort_by(|left, right| {
        left.queue
            .source_name
            .cmp(&right.queue.source_name)
            .then_with(|| left.queue.source_id.cmp(&right.queue.source_id))
    });
    Ok(records)
}

pub fn load_pending_by_name(name: &str) -> KnowledgeResult<Option<PendingQueue>> {
    Ok(list_pending_records()?
        .into_iter()
        .find(|record| record.queue.source_name == name)
        .map(|record| record.queue))
}

pub fn remove_pending_by_name(name: &str) -> KnowledgeResult<bool> {
    let mut removed = false;
    for record in list_pending_records()? {
        if record.queue.source_name == name {
            remove_file_if_exists(&record.file)?;
            removed = true;
        }
    }
    Ok(removed)
}

pub fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn now_string() -> String {
    Utc::now().to_rfc3339()
}

pub fn local_time(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|time| {
            time.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S %Z")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

pub fn local_time_optional(value: &Option<String>) -> Option<String> {
    value.as_deref().map(local_time)
}

pub fn local_time_zone() -> String {
    let now = Local
        .timestamp_opt(Utc::now().timestamp(), 0)
        .single()
        .unwrap_or_else(Local::now);
    now.format("%Z").to_string()
}

pub fn canonical_path(path: &Path) -> KnowledgeResult<PathBuf> {
    path.canonicalize().map_err(|error| {
        KnowledgeError::invalid(format!("invalid path {}: {error}", path.display()))
    })
}

fn tmp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("knowledge");
    path.with_file_name(format!("{file_name}.tmp-{}", now_millis()))
}
