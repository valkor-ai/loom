use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    paths::project_paths,
    store::{
        ensure_dir, now_millis, now_string, path_exists, read_json, write_json_atomic, StateError,
        StateResult,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub schema_version: u32,
    pub project_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserProjectRegistry {
    schema_version: u32,
    projects: Vec<UserProjectEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserProjectEntry {
    project_id: String,
    project_root: String,
    updated_at: String,
}

pub fn initialize_project(project_root: &str) -> StateResult<ProjectConfig> {
    let paths = project_paths(project_root)?;
    ensure_dir(&paths.loom_dir)?;
    ensure_dir(&paths.deliveries_dir)?;
    ensure_dir(&paths.tmp_dir)?;
    ensure_dir(&paths.requests_dir)?;
    ensure_dir(&paths.metrics_dir)?;
    let config = if path_exists(&paths.config_file) {
        read_project_config(project_root)?
    } else {
        let config = ProjectConfig {
            schema_version: 1,
            project_id: create_project_id(paths.root.to_string_lossy().as_ref()),
            created_at: now_string(),
        };
        write_json_atomic(&paths.config_file, &config)?;
        config
    };
    validate_project_id(&config.project_id)?;
    register_user_project(&config.project_id, paths.root.to_string_lossy().as_ref())?;
    Ok(config)
}

pub fn read_project_config(project_root: &str) -> StateResult<ProjectConfig> {
    let paths = project_paths(project_root)?;
    let config: ProjectConfig = read_json(&paths.config_file)?;
    validate_project_id(&config.project_id)?;
    Ok(config)
}

pub fn project_root_for_project_id(project_id: &str) -> StateResult<PathBuf> {
    validate_project_id(project_id)?;
    let registry = read_user_registry()?;
    let entry = registry
        .projects
        .iter()
        .find(|entry| entry.project_id == project_id)
        .ok_or_else(|| {
            StateError::InvalidArgument(format!("projectId is not registered: {project_id}"))
        })?;
    Ok(PathBuf::from(&entry.project_root))
}

fn create_project_id(seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(now_millis().to_string().as_bytes());
    hasher.update(std::process::id().to_string().as_bytes());
    let digest = hasher.finalize();
    format!("proj_{}", hex_prefix(&digest, 16))
}

fn validate_project_id(project_id: &str) -> StateResult<()> {
    if !(8..=80).contains(&project_id.len())
        || !project_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(StateError::StateCorrupted(format!(
            "invalid projectId: {project_id}"
        )));
    }
    Ok(())
}

fn register_user_project(project_id: &str, project_root: &str) -> StateResult<()> {
    // The registry is process-wide; protect its read-modify-write cycle when
    // multiple delivery operations initialize projects concurrently.
    let _guard = project_registry_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let mut registry = read_user_registry_unlocked().unwrap_or(UserProjectRegistry {
        schema_version: 1,
        projects: vec![],
    });
    let entry = UserProjectEntry {
        project_id: project_id.to_string(),
        project_root: project_root.to_string(),
        updated_at: now_string(),
    };
    if let Some(existing) = registry
        .projects
        .iter_mut()
        .find(|existing| existing.project_id == project_id)
    {
        *existing = entry;
    } else {
        registry.projects.push(entry);
    }
    let file = user_registry_file()?;
    if let Some(parent) = file.parent() {
        ensure_dir(parent)?;
    }
    write_json_atomic(&file, &registry)
}

fn read_user_registry() -> StateResult<UserProjectRegistry> {
    let _guard = project_registry_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    read_user_registry_unlocked()
}

fn read_user_registry_unlocked() -> StateResult<UserProjectRegistry> {
    let file = user_registry_file()?;
    if !path_exists(&file) {
        return Ok(UserProjectRegistry {
            schema_version: 1,
            projects: vec![],
        });
    }
    read_json(&file)
}

fn project_registry_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn user_registry_file() -> StateResult<PathBuf> {
    if let Ok(loom_home) = std::env::var("LOOM_HOME") {
        return Ok(PathBuf::from(loom_home).join("projects").join("index.json"));
    }
    let home = std::env::var("HOME").map_err(|_| {
        StateError::InvalidArgument("HOME is required for Loom project registry".to_string())
    })?;
    Ok(PathBuf::from(home)
        .join(".loom")
        .join("projects")
        .join("index.json"))
}

fn hex_prefix(bytes: &[u8], count: usize) -> String {
    bytes
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .take(count)
        .map(|nibble| char::from_digit(nibble as u32, 16).expect("hex nibble"))
        .collect()
}
