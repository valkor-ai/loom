use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    paths::{from_project_relative, project_paths, to_project_relative},
    project::initialize_project,
    request_index::{upsert_request_index_entry, RequestIndexEntry, RequestSourceProtocol},
    request_manifest::request_ref,
    store::{now_string, read_json_value, StateError, StateResult},
};

pub fn register_legacy_ts_request(project_root: &str, request_file: &str) -> StateResult<String> {
    let config = initialize_project(project_root)?;
    let paths = project_paths(project_root)?;
    let absolute = from_project_relative(&paths.root, request_file)?;
    let mut root = read_json_value(&absolute)?;
    let request_id = root
        .get("requestId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| legacy_request_id(request_file));
    if root.get("requestReadPlan").is_none() {
        let canonical_plan = legacy_read_plan(&root, &config.project_id, &request_id)?;
        if let Some(object) = root.as_object_mut() {
            object.insert("requestReadPlan".to_string(), canonical_plan);
        }
    }
    let request_ref = request_ref(&config.project_id, &request_id);
    upsert_request_index_entry(
        project_root,
        RequestIndexEntry {
            request_id,
            request_ref: request_ref.clone(),
            request_file: to_project_relative(&paths.root, &absolute)?,
            delivery_id: None,
            phase_id: None,
            request_kind: root
                .get("requestKind")
                .and_then(Value::as_str)
                .unwrap_or("legacy_typescript_request")
                .to_string(),
            source_protocol: RequestSourceProtocol::LegacyTypeScript,
            created_at: now_string(),
            updated_at: now_string(),
        },
    )?;
    Ok(request_ref)
}

pub fn hydrate_legacy_request_value(
    mut root: Value,
    project_id: &str,
    request_id: &str,
) -> StateResult<Value> {
    if root.get("requestReadPlan").is_none() {
        let canonical_plan = legacy_read_plan(&root, project_id, request_id)?;
        if let Some(object) = root.as_object_mut() {
            object.insert("requestReadPlan".to_string(), canonical_plan);
        }
    }
    Ok(root)
}

fn legacy_read_plan(root: &Value, project_id: &str, request_id: &str) -> StateResult<Value> {
    let groups = root
        .get("agentAction")
        .and_then(|value| value.get("read"))
        .and_then(|value| value.get("fieldGroups"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            StateError::InvalidArgument(
                "legacy request has no requestReadPlan or agentAction.read.fieldGroups".to_string(),
            )
        })?;
    let canonical_groups = groups
        .iter()
        .filter_map(Value::as_object)
        .enumerate()
        .map(|(index, group)| legacy_group(group, project_id, request_id, index + 1))
        .collect::<StateResult<Vec<_>>>()?;
    Ok(serde_json::json!({
        "schemaVersion": "1.0",
        "authority": "requestReadPlan.groups",
        "requestRef": crate::request_manifest::request_ref(project_id, request_id),
        "groups": canonical_groups,
    }))
}

fn legacy_group(
    group: &Map<String, Value>,
    project_id: &str,
    request_id: &str,
    order: usize,
) -> StateResult<Value> {
    let group_id = group
        .get("groupId")
        .and_then(Value::as_str)
        .unwrap_or("request_fields");
    let fields = group
        .get("fields")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            StateError::InvalidArgument("legacy read group fields are required".to_string())
        })?
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .fold(Vec::<String>::new(), |mut acc, field| {
            let field = field.to_string();
            if !acc.contains(&field) {
                acc.push(field);
            }
            acc
        });
    Ok(serde_json::json!({
        "groupId": group_id,
        "required": group.get("required").and_then(Value::as_bool).unwrap_or(true),
        "order": order,
        "purpose": group.get("purpose").and_then(Value::as_str).unwrap_or("Read legacy request fields."),
        "whenToRead": group.get("whenToRead").and_then(Value::as_str).unwrap_or("Before acting on this request."),
        "fields": fields,
        "readTool": "loom.readFieldGroup",
        "resourceUri": format!(
            "loom://projects/{project_id}/requests/{request_id}/field-groups/{}",
            crate::request_manifest::encode_component(group_id)
        ),
    }))
}

fn legacy_request_id(request_file: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request_file.as_bytes());
    let digest = hasher.finalize();
    format!(
        "legacy_req_{}",
        digest
            .iter()
            .flat_map(|byte| [byte >> 4, byte & 0x0f])
            .take(16)
            .map(|nibble| char::from_digit(nibble as u32, 16).expect("hex nibble"))
            .collect::<String>()
    )
}
