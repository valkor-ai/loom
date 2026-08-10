use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectToolInput {
    pub project_root: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    Codex,
    ClaudeCode,
    Opencode,
}

impl HostKind {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value
            .unwrap_or("codex")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "claude_code" | "claude-code" | "claude" => Self::ClaudeCode,
            "opencode" | "open_code" | "open-code" => Self::Opencode,
            _ => Self::Codex,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoomMcpRuntimeContext {
    pub host: HostKind,
    pub locale: Option<String>,
    pub time_zone: Option<String>,
}

impl LoomMcpRuntimeContext {
    pub fn from_env() -> Self {
        let host = std::env::var("LOOM_HOST").ok();
        Self {
            host: HostKind::from_env_value(host.as_deref()),
            locale: std::env::var("LANG").ok(),
            time_zone: std::env::var("TZ").ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedProjectRoot {
    pub path: PathBuf,
    pub display: String,
}

pub fn normalize_project_root(raw: &str) -> Result<NormalizedProjectRoot, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("projectRoot is required.".to_string());
    }

    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err("projectRoot must be an absolute path.".to_string());
    }
    if !path.exists() {
        return Err("projectRoot must exist.".to_string());
    }
    if !path.is_dir() {
        return Err("projectRoot must be a directory.".to_string());
    }

    let canonical = path
        .canonicalize()
        .map_err(|error| format!("projectRoot cannot be canonicalized: {error}"))?;
    Ok(NormalizedProjectRoot {
        display: canonical.to_string_lossy().into_owned(),
        path: canonical,
    })
}

pub fn loom_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("LOOM_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Ok(path);
    }
    let user_home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "HOME, USERPROFILE, or LOOM_HOME is required.".to_string())?;
    Ok(user_home.join(".loom"))
}

pub fn loom_runtime_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("LOOM_RUNTIME_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Ok(path);
    }
    Ok(loom_home()?.join("runtime/current"))
}
