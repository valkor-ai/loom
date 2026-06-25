use std::{collections::BTreeSet, path::Path};

use delivery_core::{
    submit_tool_accepts_artifact, ArtifactKind, FileSubmitInput, ReadGroupRef, RepairIssue,
    RouteAction, SubmitPreflightSummary, WriteMode, WriteTarget,
};
use serde_json::Value;

use crate::{
    boundary::ensure_project_contained,
    paths::{from_project_relative, project_paths},
    project::read_project_config,
    request_index::{get_request_index_entry, RequestSourceProtocol},
    request_manifest::{read_group_refs_from_root, request_storage_ref},
    store::{path_exists, read_json_value, StateError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedWriteSet {
    pub request_ref: String,
    pub request_id: String,
    pub delivery_id: Option<String>,
    pub phase_id: Option<String>,
    pub artifact_kind: ArtifactKind,
    pub write_mode: WriteMode,
    pub submit_tool: String,
    pub targets: Vec<WriteTarget>,
    pub read_groups: Vec<ReadGroupRef>,
    pub next_action: Option<RouteAction>,
}

#[derive(Debug)]
pub enum WriteTargetAuthorizationError {
    Fatal {
        code: &'static str,
        message: String,
    },
    Repairable {
        target_file: String,
        target_ids: Vec<String>,
        issues: Vec<RepairIssue>,
        read_groups: Vec<ReadGroupRef>,
        resubmit_tool: String,
    },
}

pub fn authorize_write_targets(
    input: &FileSubmitInput,
    submit_tool: &str,
) -> Result<AuthorizedWriteSet, WriteTargetAuthorizationError> {
    let parsed = parse_request_ref(&input.request_ref)?;
    let config = read_project_config(&input.project_root).map_err(fatal_state)?;
    if config.project_id != parsed.project_id {
        return Err(fatal(
            "REQUEST_PROJECT_MISMATCH",
            format!(
                "requestRef projectId {} does not match project root projectId {}.",
                parsed.project_id, config.project_id
            ),
        ));
    }

    let index_entry =
        get_request_index_entry(&input.project_root, &parsed.request_id).map_err(fatal_state)?;
    if index_entry.source_protocol != RequestSourceProtocol::RustMcpNative {
        return Err(fatal(
            "LEGACY_REQUEST_NOT_ALLOWED",
            "MCP native submit only accepts Rust MCP native requestRef. Legacy TS requests are migration inputs, not submit targets.",
        ));
    }

    let paths = project_paths(&input.project_root).map_err(fatal_state)?;
    let request_file =
        from_project_relative(&paths.root, &index_entry.request_file).map_err(fatal_state)?;
    let mut root = read_json_value(&request_file).map_err(fatal_state)?;
    hydrate_submit_refs(&mut root, &paths.root, &parsed.request_id)?;
    let read_groups = read_group_refs_from_root(&root, &parsed.project_id, &parsed.request_id)
        .map_err(fatal_state)?;

    let artifact_kind = extract_artifact_kind(&root)?;
    if !submit_tool_accepts_artifact(submit_tool, artifact_kind) {
        return Err(fatal(
            "SUBMIT_TOOL_ARTIFACT_MISMATCH",
            format!("{submit_tool} cannot submit artifact kind {artifact_kind:?}."),
        ));
    }
    let declared_submit_tool = extract_submit_tool(&root)?;
    if declared_submit_tool != submit_tool {
        return Err(fatal(
            "SUBMIT_TOOL_MISMATCH",
            format!(
                "request declares submitTool {declared_submit_tool}, but {submit_tool} was called."
            ),
        ));
    }

    let write_mode = extract_write_mode(&root)?;
    let all_targets = extract_write_targets(&root)?;
    if all_targets.is_empty() {
        return Err(fatal(
            "WRITE_TARGETS_REQUIRED",
            "request outputContract.writeTargets must declare at least one target.",
        ));
    }
    validate_target_paths(&paths.root, &all_targets)?;

    let selected_targets = select_targets(&all_targets, input.written_target_ids.as_deref())?;
    let repair_issues = validate_target_files(&paths.root, &selected_targets, write_mode);
    if !repair_issues.is_empty() {
        let target_file = selected_targets
            .first()
            .map(|target| target.path.clone())
            .unwrap_or_default();
        let target_ids = selected_targets
            .iter()
            .map(|target| target.target_id.clone())
            .collect();
        return Err(WriteTargetAuthorizationError::Repairable {
            target_file,
            target_ids,
            issues: repair_issues,
            read_groups,
            resubmit_tool: submit_tool.to_string(),
        });
    }

    Ok(AuthorizedWriteSet {
        request_ref: input.request_ref.clone(),
        request_id: parsed.request_id,
        delivery_id: index_entry.delivery_id,
        phase_id: index_entry.phase_id,
        artifact_kind,
        write_mode,
        submit_tool: submit_tool.to_string(),
        targets: selected_targets,
        read_groups,
        next_action: extract_next_action(&root)?,
    })
}

impl AuthorizedWriteSet {
    pub fn summary(&self) -> SubmitPreflightSummary {
        SubmitPreflightSummary {
            request_ref: self.request_ref.clone(),
            request_id: self.request_id.clone(),
            artifact_kind: self.artifact_kind,
            submit_tool: self.submit_tool.clone(),
            target_ids: self
                .targets
                .iter()
                .map(|target| target.target_id.clone())
                .collect(),
            next_action: self.next_action.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRequestRef {
    project_id: String,
    request_id: String,
}

fn parse_request_ref(request_ref: &str) -> Result<ParsedRequestRef, WriteTargetAuthorizationError> {
    let prefix = "loom://projects/";
    let rest = request_ref.strip_prefix(prefix).ok_or_else(|| {
        fatal(
            "INVALID_REQUEST_REF",
            "requestRef must start with loom://projects/.",
        )
    })?;
    let (project_id, rest) = rest
        .split_once("/requests/")
        .ok_or_else(|| fatal("INVALID_REQUEST_REF", "requestRef must include /requests/."))?;
    if project_id.is_empty() || rest.is_empty() || rest.contains('/') {
        return Err(fatal(
            "INVALID_REQUEST_REF",
            format!("invalid requestRef: {request_ref}"),
        ));
    }
    Ok(ParsedRequestRef {
        project_id: project_id.to_string(),
        request_id: rest.to_string(),
    })
}

fn extract_artifact_kind(root: &Value) -> Result<ArtifactKind, WriteTargetAuthorizationError> {
    let value = root
        .get("artifactKind")
        .or_else(|| root.pointer("/outputContract/artifactKind"))
        .cloned()
        .ok_or_else(|| {
            fatal(
                "ARTIFACT_KIND_REQUIRED",
                "request must declare artifactKind.",
            )
        })?;
    serde_json::from_value(value).map_err(|error| {
        fatal(
            "ARTIFACT_KIND_INVALID",
            format!("invalid artifactKind: {error}"),
        )
    })
}

fn extract_write_mode(root: &Value) -> Result<WriteMode, WriteTargetAuthorizationError> {
    let value = root
        .get("writeMode")
        .or_else(|| root.pointer("/outputContract/writeMode"))
        .cloned()
        .unwrap_or_else(|| Value::String("single_json".to_string()));
    serde_json::from_value(value)
        .map_err(|error| fatal("WRITE_MODE_INVALID", format!("invalid writeMode: {error}")))
}

fn extract_submit_tool(root: &Value) -> Result<String, WriteTargetAuthorizationError> {
    root.get("submitTool")
        .or_else(|| root.pointer("/outputContract/submitTool"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| fatal("SUBMIT_TOOL_REQUIRED", "request must declare submitTool."))
}

fn extract_write_targets(root: &Value) -> Result<Vec<WriteTarget>, WriteTargetAuthorizationError> {
    let value = root
        .get("writeTargets")
        .or_else(|| root.pointer("/outputContract/writeTargets"))
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    serde_json::from_value(value).map_err(|error| {
        fatal(
            "WRITE_TARGETS_INVALID",
            format!("invalid writeTargets: {error}"),
        )
    })
}

fn extract_next_action(root: &Value) -> Result<Option<RouteAction>, WriteTargetAuthorizationError> {
    let Some(value) = root.pointer("/postSubmit/nextAction").cloned() else {
        return Ok(None);
    };
    serde_json::from_value(value).map(Some).map_err(|error| {
        fatal(
            "NEXT_ACTION_INVALID",
            format!("invalid postSubmit.nextAction: {error}"),
        )
    })
}

fn hydrate_submit_refs(
    root: &mut Value,
    project_root: &Path,
    request_id: &str,
) -> Result<(), WriteTargetAuthorizationError> {
    if root.get("outputContract").is_some() {
        return Ok(());
    }
    let Some(relative) =
        request_storage_ref(project_root, request_id, "outputContract").map_err(fatal_state)?
    else {
        return Ok(());
    };
    let ref_file = from_project_relative(project_root, &relative).map_err(fatal_state)?;
    let value = read_json_value(&ref_file).map_err(fatal_state)?;
    let Some(object) = root.as_object_mut() else {
        return Err(fatal(
            "REQUEST_ROOT_INVALID",
            "request root must be a JSON object.",
        ));
    };
    object.insert("outputContract".to_string(), value);
    Ok(())
}

fn validate_target_paths(
    project_root: &std::path::Path,
    targets: &[WriteTarget],
) -> Result<(), WriteTargetAuthorizationError> {
    for target in targets {
        if target.target_id.trim().is_empty() {
            return Err(fatal(
                "TARGET_ID_REQUIRED",
                "write target targetId is required.",
            ));
        }
        let absolute = from_project_relative(project_root, &target.path).map_err(fatal_state)?;
        ensure_project_contained(project_root, &absolute).map_err(fatal_state)?;
        if !is_agent_writable_path(&target.path) {
            return Err(fatal(
                "TARGET_PATH_NOT_AGENT_WRITABLE",
                format!(
                    "write target {} path must be under an agent-writable .loom directory.",
                    target.target_id
                ),
            ));
        }
        if is_protected_state_path(&target.path) {
            return Err(fatal(
                "TARGET_PATH_PROTECTED",
                format!(
                    "write target {} points at protected Loom state: {}",
                    target.target_id, target.path
                ),
            ));
        }
    }
    Ok(())
}

fn select_targets(
    targets: &[WriteTarget],
    written_target_ids: Option<&[String]>,
) -> Result<Vec<WriteTarget>, WriteTargetAuthorizationError> {
    let ids = written_target_ids
        .unwrap_or(&[])
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<BTreeSet<_>>();
    let known = targets
        .iter()
        .map(|target| target.target_id.clone())
        .collect::<BTreeSet<_>>();
    for id in &ids {
        if !known.contains(id) {
            return Err(fatal(
                "TARGET_NOT_ALLOWED",
                format!("writtenTargetIds contains unknown targetId: {id}"),
            ));
        }
    }

    let mut selected = Vec::new();
    for target in targets {
        if target.required || ids.is_empty() || ids.contains(&target.target_id) {
            selected.push(target.clone());
        }
    }
    Ok(selected)
}

fn validate_target_files(
    project_root: &std::path::Path,
    targets: &[WriteTarget],
    write_mode: WriteMode,
) -> Vec<RepairIssue> {
    let mut issues = Vec::new();
    for target in targets {
        if write_mode == WriteMode::TaskplanGrouped
            && (target.path.contains('{') || target.path.contains('}'))
        {
            continue;
        }
        let Ok(absolute) = from_project_relative(project_root, &target.path) else {
            issues.push(issue(
                "TARGET_PATH_INVALID",
                &target.target_id,
                format!("target path is invalid: {}", target.path),
            ));
            continue;
        };
        if !path_exists(&absolute) {
            issues.push(issue(
                "TARGET_MISSING",
                &target.target_id,
                format!("target file does not exist: {}", target.path),
            ));
            continue;
        }
        match read_json_value(&absolute) {
            Ok(Value::Object(_)) => {}
            Ok(_) => issues.push(issue(
                "TARGET_NOT_JSON_OBJECT",
                &target.target_id,
                format!("target file must contain a JSON object: {}", target.path),
            )),
            Err(error) => issues.push(issue(
                "INVALID_JSON",
                &target.target_id,
                format!("target file is not valid JSON: {error}"),
            )),
        }
    }
    issues
}

fn issue(code: &'static str, target_id: &str, message: String) -> RepairIssue {
    RepairIssue {
        code: code.to_string(),
        message,
        target_id: Some(target_id.to_string()),
        field_path: None,
    }
}

fn is_agent_writable_path(path: &str) -> bool {
    path.starts_with(".loom/agent-writable/")
        || path.starts_with(".loom/deliveries/") && path.contains("/agent-writable/")
}

fn is_protected_state_path(path: &str) -> bool {
    path == ".loom/status.json"
        || path == ".loom/config.json"
        || path == ".loom/.gitignore"
        || path.starts_with(".loom/requests/")
        || path.starts_with(".loom/metrics/")
        || path.ends_with("/index.json")
}

fn fatal(code: &'static str, message: impl Into<String>) -> WriteTargetAuthorizationError {
    WriteTargetAuthorizationError::Fatal {
        code,
        message: message.into(),
    }
}

fn fatal_state(error: StateError) -> WriteTargetAuthorizationError {
    WriteTargetAuthorizationError::Fatal {
        code: "STATE_ERROR",
        message: error.to_string(),
    }
}
