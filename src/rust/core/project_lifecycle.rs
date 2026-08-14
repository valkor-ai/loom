use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::normalize_project_root;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanToolInput {
    pub project_root: String,
    pub request_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequirementFileIdentity {
    pub path: String,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequestIdentity {
    pub schema_version: u32,
    pub fingerprint: String,
    pub request_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalPlanRequest {
    pub schema_version: u32,
    pub request_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_files: Vec<PlanRequirementFileIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_file_refs: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlanInput {
    pub project_root: String,
    pub request_text: String,
    pub requirement_files: Vec<String>,
    pub request_identity: PlanRequestIdentity,
    pub supersede_active_delivery_id: Option<String>,
    pub expected_lifecycle_revision: Option<u64>,
    pub plan_conflict_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanConflictChoice {
    #[serde(alias = "1")]
    ContinueCurrent,
    #[serde(alias = "2")]
    StartNew,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanConflictResolveInput {
    pub project_root: String,
    pub conflict_ref: String,
    pub choice: PlanConflictChoice,
}

pub fn validate_plan_input(input: PlanToolInput) -> Result<ValidatedPlanInput, String> {
    let normalized = normalize_project_root(&input.project_root)?;
    let request_text = normalize_request_text(&input.request_text);
    if request_text.is_empty() {
        return Err("requestText is required.".to_string());
    }
    let requirement_files = input
        .requirement_files
        .into_iter()
        .map(|file| normalize_requirement_file(&normalized.path, &file))
        .collect::<Result<Vec<_>, _>>()?;
    let request_identity =
        build_plan_request_identity(&normalized.path, &request_text, &requirement_files)?;
    Ok(ValidatedPlanInput {
        project_root: normalized.display,
        request_text,
        requirement_files,
        request_identity,
        supersede_active_delivery_id: None,
        expected_lifecycle_revision: None,
        plan_conflict_id: None,
    })
}

pub fn build_plan_request_identity(
    project_root: &Path,
    request_text: &str,
    requirement_files: &[String],
) -> Result<PlanRequestIdentity, String> {
    let mut files = requirement_files
        .iter()
        .map(|file| {
            let path = Path::new(file);
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                project_root.join(path)
            };
            let canonical = absolute.canonicalize().map_err(|error| {
                format!("requirementFile cannot be canonicalized: {file}: {error}")
            })?;
            let display_path = canonical
                .strip_prefix(project_root)
                .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| canonical.to_string_lossy().replace('\\', "/"));
            let content = std::fs::read(&canonical)
                .map_err(|error| format!("requirementFile cannot be read: {file}: {error}"))?;
            let digest = Sha256::digest(content);
            Ok(PlanRequirementFileIdentity {
                path: display_path,
                content_digest: format!("sha256:{digest:x}"),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    // Requirement file order is an invocation detail. Treat the canonical
    // path and content digest as the identity so adapters cannot create a new
    // delivery merely by reordering the same requirement files.
    files.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.content_digest.cmp(&right.content_digest))
    });
    files.dedup();
    let request_text = normalize_request_text(request_text);
    let fingerprint_source = json!({
        "schemaVersion": 1,
        "requestText": &request_text,
        "requirementFiles": &files,
    });
    let canonical = serde_json::to_string(&fingerprint_source)
        .map_err(|error| format!("request identity serialization failed: {error}"))?;
    let digest = Sha256::digest(canonical.as_bytes());
    let fingerprint = format!("sha256:{digest:x}");
    Ok(PlanRequestIdentity {
        schema_version: 1,
        request_ref: format!(".loom/plan-requests/{}.json", fingerprint.replace(':', "-")),
        fingerprint,
    })
}

pub fn canonical_plan_request(
    project_root: &Path,
    request_text: &str,
    requirement_files: &[String],
) -> Result<CanonicalPlanRequest, String> {
    let identity = build_plan_request_identity(project_root, request_text, requirement_files)?;
    let mut files = requirement_files
        .iter()
        .map(|file| {
            let canonical = Path::new(file).canonicalize().map_err(|error| {
                format!("requirementFile cannot be canonicalized: {file}: {error}")
            })?;
            let display_path = canonical
                .strip_prefix(project_root)
                .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| canonical.to_string_lossy().replace('\\', "/"));
            let content = std::fs::read(&canonical)
                .map_err(|error| format!("requirementFile cannot be read: {file}: {error}"))?;
            let digest = Sha256::digest(content);
            Ok(PlanRequirementFileIdentity {
                path: display_path,
                content_digest: format!("sha256:{digest:x}"),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    files.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.content_digest.cmp(&right.content_digest))
    });
    files.dedup();
    let requirement_file_refs = files.iter().map(|file| file.path.clone()).collect();
    Ok(CanonicalPlanRequest {
        schema_version: 1,
        request_text: normalize_request_text(request_text),
        requirement_files: files,
        requirement_file_refs,
        fingerprint: identity.fingerprint,
    })
}

pub fn normalize_request_text(value: &str) -> String {
    value.trim().replace("\r\n", "\n").replace('\r', "\n")
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
