use std::path::{Path, PathBuf};

use crate::store::{StateError, StateResult};

pub fn ensure_project_contained(project_root: &Path, candidate: &Path) -> StateResult<PathBuf> {
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        project_root.join(candidate)
    };
    let normalized = normalize_existing_or_parent(&absolute)?;
    if !normalized.starts_with(project_root) {
        return Err(StateError::InvalidArgument(format!(
            "path escapes project root: {}",
            candidate.display()
        )));
    }
    Ok(absolute)
}

fn normalize_existing_or_parent(path: &Path) -> StateResult<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let parent = path.parent().ok_or_else(|| {
        StateError::InvalidArgument(format!("path has no parent: {}", path.display()))
    })?;
    if parent.exists() {
        return Ok(parent.canonicalize()?);
    }
    normalize_existing_or_parent(parent)
}
