use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::normalize_project_root;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanToolInput {
    pub project_root: String,
    pub request_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlanInput {
    pub project_root: String,
    pub request_text: String,
    pub requirement_files: Vec<String>,
}

pub fn validate_plan_input(input: PlanToolInput) -> Result<ValidatedPlanInput, String> {
    let normalized = normalize_project_root(&input.project_root)?;
    let request_text = input.request_text.trim().to_string();
    if request_text.is_empty() {
        return Err("requestText is required.".to_string());
    }
    let requirement_files = input
        .requirement_files
        .into_iter()
        .map(|file| normalize_requirement_file(&normalized.path, &file))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValidatedPlanInput {
        project_root: normalized.display,
        request_text,
        requirement_files,
    })
}

fn normalize_requirement_file(project_root: &Path, raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("requirementFiles cannot include an empty path.".to_string());
    }
    let path = Path::new(trimmed);
    let absolute = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        project_root.join(path)
    };
    if !absolute.exists() {
        return Err(format!(
            "requirementFile does not exist: {}",
            absolute.display()
        ));
    }
    if !absolute.is_file() {
        return Err(format!(
            "requirementFile must be a file: {}",
            absolute.display()
        ));
    }
    absolute
        .canonicalize()
        .map_err(|error| format!("requirementFile cannot be canonicalized: {error}"))
        .map(|path| path.to_string_lossy().into_owned())
}
