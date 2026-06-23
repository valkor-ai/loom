use serde::Serialize;

use crate::{
    paths::project_paths,
    store::{ensure_dir, now_string, StateResult},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSizeAudit<'a> {
    pub request_ref: &'a str,
    pub request_file: &'a str,
    pub full_bytes: usize,
    pub compact_bytes: usize,
    pub ref_count: usize,
    pub recorded_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldReadAudit<'a> {
    pub request_ref: &'a str,
    pub request_id: &'a str,
    pub fields: &'a [String],
    pub source: &'a str,
    pub recorded_at: String,
}

pub fn record_request_size_audit(project_root: &str, audit: RequestSizeAudit<'_>) {
    let _ = append_json_line(project_root, AuditFile::RequestSize, &audit);
}

pub fn record_field_read_audit(project_root: &str, audit: FieldReadAudit<'_>) {
    let _ = append_json_line(project_root, AuditFile::FieldRead, &audit);
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
