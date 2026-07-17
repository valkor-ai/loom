use serde::Serialize;

use crate::{
    paths::project_paths,
    store::{ensure_dir, now_string, StateResult},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadPlanSizeWarning {
    pub level: &'static str,
    pub group_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub bytes: usize,
    pub limit_bytes: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSizeAudit<'a> {
    pub request_ref: &'a str,
    pub request_file: &'a str,
    pub full_bytes: usize,
    pub compact_bytes: usize,
    pub ref_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub read_plan_warnings: Vec<ReadPlanSizeWarning>,
    pub recorded_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldReadAudit<'a> {
    pub request_ref: &'a str,
    pub request_id: &'a str,
    pub fields: &'a [String],
    pub source: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<&'a str>,
    pub recorded_at: String,
}

pub fn record_request_size_audit(project_root: &str, audit: RequestSizeAudit<'_>) {
    let _ = append_json_line(project_root, AuditFile::RequestSize, &audit);
}

pub fn record_field_read_audit(project_root: &str, audit: FieldReadAudit<'_>) {
    let _ = append_json_line(project_root, AuditFile::FieldRead, &audit);
}

pub fn record_request_inspect_audit(project_root: &str, request_ref: &str, request_id: &str) {
    let _ = append_json_line(
        project_root,
        AuditFile::FieldRead,
        &FieldReadAudit {
            request_ref,
            request_id,
            fields: &[],
            source: "inspectRequest",
            group_id: None,
            recorded_at: now_for_audit(),
        },
    );
}

pub fn request_was_inspected(project_root: &str, request_ref: &str) -> bool {
    read_audit_entries(project_root).iter().any(|entry| {
        entry.get("requestRef").and_then(serde_json::Value::as_str) == Some(request_ref)
            && entry.get("source").and_then(serde_json::Value::as_str) == Some("inspectRequest")
    })
}

pub fn required_groups_not_read(
    project_root: &str,
    request_ref: &str,
    required_group_ids: &[String],
) -> Vec<String> {
    let read_groups = read_audit_entries(project_root)
        .iter()
        .filter(|entry| {
            entry.get("requestRef").and_then(serde_json::Value::as_str) == Some(request_ref)
                && entry.get("source").and_then(serde_json::Value::as_str) == Some("readFieldGroup")
        })
        .filter_map(|entry| {
            entry
                .get("groupId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<std::collections::BTreeSet<_>>();
    required_group_ids
        .iter()
        .filter(|group_id| !read_groups.contains(*group_id))
        .cloned()
        .collect()
}

fn read_audit_entries(project_root: &str) -> Vec<serde_json::Value> {
    let Ok(paths) = project_paths(project_root) else {
        return vec![];
    };
    let Ok(content) = std::fs::read_to_string(paths.field_read_audit_file) else {
        return vec![];
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}

enum AuditFile {
    RequestSize,
    FieldRead,
}

fn append_json_line(
    project_root: &str,
    kind: AuditFile,
    value: &impl Serialize,
) -> StateResult<()> {
    let paths = project_paths(project_root)?;
    ensure_dir(&paths.metrics_dir)?;
    let file = match kind {
        AuditFile::RequestSize => paths.request_size_audit_file,
        AuditFile::FieldRead => paths.field_read_audit_file,
    };
    let line = format!("{}\n", serde_json::to_string(value)?);
    use std::io::Write;
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    handle.write_all(line.as_bytes())?;
    Ok(())
}

pub fn now_for_audit() -> String {
    now_string()
}
