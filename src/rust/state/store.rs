use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

pub type StateResult<T> = Result<T, StateError>;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("state corrupted: {0}")]
    StateCorrupted(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn ensure_dir(path: &Path) -> StateResult<()> {
    if path.exists() {
        if !path.is_dir() {
            return Err(StateError::StateCorrupted(format!(
                "path exists but is not a directory: {}",
                path.display()
            )));
        }
        return Ok(());
    }
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn path_exists(path: &Path) -> bool {
    path.exists()
}

pub fn remove_file_if_exists(path: &Path) -> StateResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StateError::Io(error)),
    }
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> StateResult<T> {
    let raw = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StateError::InvalidArgument(format!("JSON file does not exist: {}", path.display()))
        } else {
            StateError::Io(error)
        }
    })?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn read_json_value(path: &Path) -> StateResult<serde_json::Value> {
    read_json(path)
}

/// Read a project-relative JSON reference with an optional JSON Pointer
/// fragment. A fragment keeps phase projections addressable without creating a
/// second JSON file that copies the same payload.
pub fn read_json_reference(project_root: &Path, relative: &str) -> StateResult<serde_json::Value> {
    read_json_reference_inner(project_root, relative, 0)
}

fn read_json_reference_inner(
    project_root: &Path,
    relative: &str,
    depth: u8,
) -> StateResult<serde_json::Value> {
    if depth > 8 {
        return Err(StateError::StateCorrupted(
            "JSON reference indirection exceeds the supported depth".to_string(),
        ));
    }
    let (path_ref, fragment) = relative.split_once('#').unwrap_or((relative, ""));
    let path = crate::paths::from_project_relative(project_root, path_ref)?;
    let mut value = read_json_value(&path)?;
    if let Some(canonical_ref) = value
        .get("canonicalRef")
        .and_then(serde_json::Value::as_str)
    {
        value = read_json_reference_inner(project_root, canonical_ref, depth + 1)?;
    }
    if fragment.is_empty() {
        return Ok(value);
    }
    let pointer = if fragment.starts_with('/') {
        fragment
    } else {
        return Err(StateError::InvalidArgument(format!(
            "JSON reference fragment must be a JSON Pointer: {relative}"
        )));
    };
    value.pointer(pointer).cloned().ok_or_else(|| {
        StateError::StateCorrupted(format!(
            "JSON reference fragment does not exist: {relative}"
        ))
    })
}

pub fn read_text(path: &Path) -> StateResult<String> {
    Ok(fs::read_to_string(path)?)
}

pub fn write_json_atomic(path: &Path, value: &impl Serialize) -> StateResult<()> {
    let text = format!("{}\n", serde_json::to_string_pretty(value)?);
    write_text_atomic(path, &text)
}

pub fn write_text_atomic(path: &Path, text: &str) -> StateResult<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let tmp = tmp_path(path);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    path.with_file_name(format!(
        "{file_name}.tmp-{}-{}",
        std::process::id(),
        now_millis()
    ))
}

pub fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn now_string() -> String {
    now_millis().to_string()
}
