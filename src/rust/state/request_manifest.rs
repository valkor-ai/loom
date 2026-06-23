use std::collections::BTreeMap;

use delivery_core::ReadGroupRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    paths::{from_project_relative, request_file_for_id, request_refs_dir, to_project_relative},
    project::initialize_project,
    read_audit::{now_for_audit, record_request_size_audit, RequestSizeAudit},
    request_index::{
        upsert_request_index_entry, validate_request_id, RequestIndexEntry, RequestSourceProtocol,
    },
    store::{now_string, write_json_atomic, StateError, StateResult},
};

const DEFAULT_REF_KEYS: &[&str] = &[
    "agentAction",
    "referencedArtifactReadGuide",
    "generationProtocol",
    "generationRules",
    "fieldAccessHints",
    "requestOptimization",
    "validatorRulesSummary",
    "validatorPolicy",
    "executionRules",
    "reviewRules",
    "enumRefs",
    "allowedRefs",
    "rules",
    "sourceRefs",
    "contextProjection",
    "sourceContracts",
    "sourceContext",
    "executionArtifacts",
    "changeSet",
    "reviewScope",
    "task",
    "taskConceptGrounding",
    "outputContract",
    "blockedOutput",
];

#[derive(Debug, Clone)]
pub struct NativeRequestInput {
    pub request_id: String,
    pub request_kind: String,
    pub request_file: Option<String>,
    pub delivery_id: Option<String>,
    pub phase_id: Option<String>,
    pub root: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoredRequest {
    pub request_ref: String,
    pub request_id: String,
    pub project_id: String,
    pub request_file: String,
    pub read_groups: Vec<ReadGroupRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestManifest {
    schema_version: String,
    ref_first: bool,
    protocol_authority: String,
    refs: BTreeMap<String, RequestManifestRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestManifestRef {
    ref_key: String,
    r#ref: String,
    purpose: String,
}

pub fn write_native_request(
    project_root: &str,
    input: NativeRequestInput,
) -> StateResult<StoredRequest> {
    validate_request_id(&input.request_id)?;
    let config = initialize_project(project_root)?;
    let project_paths = crate::paths::project_paths(project_root)?;
    let request_ref = request_ref(&config.project_id, &input.request_id);
    let request_file = match &input.request_file {
        Some(relative) => from_project_relative(&project_paths.root, relative)?,
        None => request_file_for_id(&project_paths.root, &input.request_id),
    };
    let request_file_relative = to_project_relative(&project_paths.root, &request_file)?;

    let full_bytes = pretty_len(&input.root);
    let mut root = input.root;
    let root_object = root.as_object_mut().ok_or_else(|| {
        StateError::InvalidArgument("native request root must be a JSON object".to_string())
    })?;
    root_object.insert(
        "requestId".to_string(),
        Value::String(input.request_id.clone()),
    );
    root_object.insert(
        "requestKind".to_string(),
        Value::String(input.request_kind.clone()),
    );
    remove_agent_action_read(root_object);

    let read_groups = canonicalize_read_plan(
        root_object,
        &request_ref,
        &config.project_id,
        &input.request_id,
    )?;
    let manifest_refs = write_manifest_refs(&project_paths.root, &request_file, root_object)?;
    let ref_count = manifest_refs.len();
    root_object.insert(
        "requestManifest".to_string(),
        serde_json::to_value(RequestManifest {
            schema_version: "1.0".to_string(),
            ref_first: true,
            protocol_authority: "request_manifest_refs".to_string(),
            refs: manifest_refs,
        })?,
    );

    write_json_atomic(&request_file, &root)?;
    let compact_bytes = pretty_len(&root);
    upsert_request_index_entry(
        project_root,
        RequestIndexEntry {
            request_id: input.request_id.clone(),
            request_ref: request_ref.clone(),
            request_file: request_file_relative.clone(),
            delivery_id: input.delivery_id,
            phase_id: input.phase_id,
            request_kind: input.request_kind,
            source_protocol: RequestSourceProtocol::RustMcpNative,
            created_at: now_string(),
            updated_at: now_string(),
        },
    )?;
    record_request_size_audit(
        project_root,
        RequestSizeAudit {
            request_ref: &request_ref,
            request_file: &request_file_relative,
            full_bytes,
            compact_bytes,
            ref_count,
            recorded_at: now_for_audit(),
        },
    );

    Ok(StoredRequest {
        request_ref,
        request_id: input.request_id,
        project_id: config.project_id,
        request_file: request_file_relative,
        read_groups,
    })
}

pub fn request_ref(project_id: &str, request_id: &str) -> String {
    format!("loom://projects/{project_id}/requests/{request_id}")
}

pub fn read_group_refs_from_root(
    root: &Value,
    project_id: &str,
    request_id: &str,
) -> StateResult<Vec<ReadGroupRef>> {
    let groups = root
        .get("requestReadPlan")
        .and_then(|plan| plan.get("groups"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            StateError::InvalidArgument("requestReadPlan.groups is required".to_string())
        })?;
    groups
        .iter()
        .enumerate()
        .map(|(index, value)| read_group_ref_from_value(value, index + 1, project_id, request_id))
        .collect()
}

fn canonicalize_read_plan(
    root_object: &mut Map<String, Value>,
    request_ref: &str,
    project_id: &str,
    request_id: &str,
) -> StateResult<Vec<ReadGroupRef>> {
    let mut plan = root_object
        .remove("requestReadPlan")
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| {
            StateError::InvalidArgument("requestReadPlan.groups is required".to_string())
        })?;
    let groups = plan
        .remove("groups")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| {
            StateError::InvalidArgument("requestReadPlan.groups is required".to_string())
        })?;
    let mut canonical_groups = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let group_ref = read_group_ref_from_value(group, index + 1, project_id, request_id)?;
        canonical_groups.push(group_ref);
    }
    let group_values = canonical_groups
        .iter()
        .map(|group| {
            serde_json::json!({
                "groupId": group.group_id,
                "required": group.required,
                "purpose": group.purpose,
                "whenToRead": group.when_to_read,
                "fields": group.fields,
            })
        })
        .collect::<Vec<_>>();
    root_object.insert(
        "requestReadPlan".to_string(),
        serde_json::json!({
            "schemaVersion": "1.0",
            "authority": "requestReadPlan.groups",
            "requestRef": request_ref,
            "groups": group_values,
        }),
    );
    Ok(canonical_groups)
}

fn read_group_ref_from_value(
    value: &Value,
    order: usize,
    project_id: &str,
    request_id: &str,
) -> StateResult<ReadGroupRef> {
    let object = value
        .as_object()
        .ok_or_else(|| StateError::InvalidArgument("read group must be an object".to_string()))?;
    let group_id = string_field(object, "groupId")?;
    let fields = object
        .get("fields")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            StateError::InvalidArgument(format!("read group {group_id} fields are required"))
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
    if fields.is_empty() {
        return Err(StateError::InvalidArgument(format!(
            "read group {group_id} must include at least one field"
        )));
    }
    Ok(ReadGroupRef {
        group_id: group_id.clone(),
        required: object
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        order: order as u32,
        purpose: object
            .get("purpose")
            .and_then(Value::as_str)
            .unwrap_or("Read request fields for the current action.")
            .to_string(),
        when_to_read: object
            .get("whenToRead")
            .and_then(Value::as_str)
            .unwrap_or("Before acting on this request.")
            .to_string(),
        fields,
        read_tool: "loom.readFieldGroup".to_string(),
        resource_uri: format!(
            "loom://projects/{project_id}/requests/{request_id}/field-groups/{}",
            encode_component(&group_id)
        ),
    })
}

fn write_manifest_refs(
    project_root: &std::path::Path,
    request_file: &std::path::Path,
    root_object: &mut Map<String, Value>,
) -> StateResult<BTreeMap<String, RequestManifestRef>> {
    let refs_dir = request_refs_dir(request_file);
    let mut refs = BTreeMap::new();
    for key in DEFAULT_REF_KEYS {
        let Some(value) = root_object.remove(*key) else {
            continue;
        };
        if key == &"agentAction" {
            if let Some(mut object) = value.as_object().cloned() {
                remove_agent_action_read(&mut object);
                write_ref(
                    project_root,
                    &refs_dir,
                    key,
                    Value::Object(object),
                    &mut refs,
                )?;
                continue;
            }
        }
        write_ref(project_root, &refs_dir, key, value, &mut refs)?;
    }
    Ok(refs)
}

fn write_ref(
    project_root: &std::path::Path,
    refs_dir: &std::path::Path,
    key: &str,
    value: Value,
    refs: &mut BTreeMap<String, RequestManifestRef>,
) -> StateResult<()> {
    let ref_file = refs_dir.join(format!("{}.json", kebab_case(key)));
    write_json_atomic(&ref_file, &value)?;
    let relative = to_project_relative(project_root, &ref_file)?;
    refs.insert(
        key.to_string(),
        RequestManifestRef {
            ref_key: format!("{key}Ref"),
            r#ref: relative.clone(),
            purpose: format!(
                "Internal storage ref for {key}. Use requestReadPlan.groups for normal reads."
            ),
        },
    );
    Ok(())
}

fn remove_agent_action_read(root_object: &mut Map<String, Value>) {
    if let Some(agent_action) = root_object
        .get_mut("agentAction")
        .and_then(Value::as_object_mut)
    {
        agent_action.remove("read");
    }
}

fn string_field(object: &Map<String, Value>, key: &str) -> StateResult<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| StateError::InvalidArgument(format!("{key} is required")))
}

fn kebab_case(value: &str) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                output.push('-');
            }
            output.push(ch.to_ascii_lowercase());
        } else if ch == '_' || ch == ' ' {
            output.push('-');
        } else {
            output.push(ch);
        }
    }
    output
}

pub fn encode_component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':') {
                vec![byte as char]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect()
}

fn pretty_len(value: &Value) -> usize {
    serde_json::to_vec_pretty(value)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}
