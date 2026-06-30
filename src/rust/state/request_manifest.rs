use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use delivery_core::ReadGroupRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    field_projection::{
        select_compact_keyword_hints, select_compact_requirement_semantic_rules,
        select_source_ref_registry, select_value, selector_parts,
    },
    paths::{
        from_project_relative, request_file_for_id, request_refs_dir,
        request_storage_manifest_file, to_project_relative,
    },
    project::initialize_project,
    read_audit::{now_for_audit, record_request_size_audit, ReadPlanSizeWarning, RequestSizeAudit},
    request_index::{
        upsert_request_index_entry, validate_request_id, RequestIndexEntry, RequestSourceProtocol,
    },
    store::{
        now_string, read_json, read_json_value, read_text, write_json_atomic, StateError,
        StateResult,
    },
};

const SPLITTABLE_REF_KEYS: &[&str] = &[
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
    "reviewPacket",
    "changeContext",
    "task",
    "taskConceptGrounding",
    "sectionOutputs",
    "outputContract",
    "blockedOutput",
];

const FORBIDDEN_ROOT_KEYS: &[&str] = &["agentAction", "requestManifest", "submitCommand"];

const FORBIDDEN_EXACT_READ_FIELDS: &[&str] = &[
    "allowedRefs",
    "blockConfirmationContract",
    "blockedOutput",
    "changeContext",
    "confirmedClarificationState",
    "contextProjection.phaseScope",
    "contextProjection.technicalBaseline",
    "currentSectionContract",
    "enumRefs",
    "executionRules",
    "frontendExperienceSource",
    "outputContract.allowedRefs",
    "outputContract.reviewSignals",
    "repairContext",
    "repoEvidence",
    "reviewPacket",
    "reviewScope",
    "reviewSignals",
    "rules",
    "scanPurpose",
    "sectionOutputs",
    "sourceContext",
    "sourceRefs",
    "task",
    "taskConceptGrounding",
    "outputContract.schemaShape",
    "clarificationConversationProtocol",
    "executionRules.rules",
    "contextProjection.requirementDetailTransfer",
];

const MAX_READ_FIELD_BYTES: usize = 32 * 1024;
const MAX_READ_GROUP_BYTES: usize = 96 * 1024;

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
pub(crate) struct RequestStorageManifest {
    schema_version: String,
    protocol_authority: String,
    request_id: String,
    refs: BTreeMap<String, RequestStorageManifestRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestStorageManifestRef {
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
    reject_forbidden_root_keys(root_object)?;
    reject_root_ref_aliases(root_object)?;
    normalize_output_contract_submit_metadata(root_object)?;

    let read_groups = canonicalize_read_plan(
        root_object,
        &request_ref,
        &config.project_id,
        &input.request_id,
    )?;
    canonicalize_context_refs(root_object, &read_groups);
    let used_ref_keys = used_storage_ref_keys(root_object, &read_groups);
    let manifest_refs = write_storage_refs(
        &project_paths.root,
        &request_file,
        root_object,
        &used_ref_keys,
    )?;
    let read_plan_warnings = validate_read_plan_contract(
        &project_paths.root,
        root_object,
        &manifest_refs,
        &read_groups,
    )?;
    let ref_count = manifest_refs.len();
    write_storage_manifest(
        &project_paths.root,
        &input.request_id,
        RequestStorageManifest {
            schema_version: "1.0".to_string(),
            protocol_authority: "rust_private_request_storage_manifest".to_string(),
            request_id: input.request_id.clone(),
            refs: manifest_refs,
        },
    )?;

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
            read_plan_warnings,
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

pub fn request_storage_ref(
    project_root: &Path,
    request_id: &str,
    key: &str,
) -> StateResult<Option<String>> {
    let manifest = read_request_storage_manifest(project_root, request_id)?;
    Ok(manifest.refs.get(key).map(|entry| entry.r#ref.clone()))
}

fn read_request_storage_manifest(
    project_root: &Path,
    request_id: &str,
) -> StateResult<RequestStorageManifest> {
    read_json(&request_storage_manifest_file(project_root, request_id))
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
    reject_read_group_sidecar_fields(&group_id, object)?;
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
        .map(validate_read_field_selector)
        .collect::<StateResult<Vec<_>>>()?
        .into_iter()
        .fold(Vec::<String>::new(), |mut acc, field| {
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

fn write_storage_refs(
    project_root: &Path,
    request_file: &Path,
    root_object: &mut Map<String, Value>,
    used_ref_keys: &BTreeSet<String>,
) -> StateResult<BTreeMap<String, RequestStorageManifestRef>> {
    let refs_dir = request_refs_dir(request_file);
    let mut refs = BTreeMap::new();
    for key in SPLITTABLE_REF_KEYS {
        let Some(value) = root_object.remove(*key) else {
            continue;
        };
        if used_ref_keys.contains(*key) {
            write_ref(project_root, &refs_dir, key, value, &mut refs)?;
        }
    }
    Ok(refs)
}

fn write_ref(
    project_root: &Path,
    refs_dir: &Path,
    key: &str,
    value: Value,
    refs: &mut BTreeMap<String, RequestStorageManifestRef>,
) -> StateResult<()> {
    let ref_file = refs_dir.join(format!("{}.json", kebab_case(key)));
    write_json_atomic(&ref_file, &value)?;
    let relative = to_project_relative(project_root, &ref_file)?;
    refs.insert(
        key.to_string(),
        RequestStorageManifestRef {
            ref_key: format!("{key}Ref"),
            r#ref: relative.clone(),
            purpose: format!("Private storage ref for {key}. Not agent-facing."),
        },
    );
    Ok(())
}

fn write_storage_manifest(
    project_root: &Path,
    request_id: &str,
    manifest: RequestStorageManifest,
) -> StateResult<()> {
    write_json_atomic(
        &request_storage_manifest_file(project_root, request_id),
        &manifest,
    )
}

fn reject_forbidden_root_keys(root_object: &Map<String, Value>) -> StateResult<()> {
    for key in FORBIDDEN_ROOT_KEYS {
        if root_object.contains_key(*key) {
            return Err(StateError::InvalidArgument(format!(
                "native MCP request root must not include {key}; use requestReadPlan.groups and submitTool/writeTargets only"
            )));
        }
    }
    Ok(())
}

fn reject_root_ref_aliases(root_object: &Map<String, Value>) -> StateResult<()> {
    let illegal = root_object
        .keys()
        .filter(|key| key.as_str() != "contextRefs" && key.ends_with("Ref"))
        .cloned()
        .collect::<Vec<_>>();
    if !illegal.is_empty() {
        return Err(StateError::InvalidArgument(format!(
            "native MCP request root must not include root ref aliases: {}",
            illegal.join(",")
        )));
    }
    Ok(())
}

fn reject_read_group_sidecar_fields(
    group_id: &str,
    object: &Map<String, Value>,
) -> StateResult<()> {
    for key in ["readCommand", "fallbackRule"] {
        if object.contains_key(key) {
            return Err(StateError::InvalidArgument(format!(
                "read group {group_id} must not include {key}; requestReadPlan.groups is the only read contract"
            )));
        }
    }
    Ok(())
}

fn normalize_output_contract_submit_metadata(
    root_object: &mut Map<String, Value>,
) -> StateResult<()> {
    let Some(contract) = root_object
        .get("outputContract")
        .and_then(Value::as_object)
        .cloned()
    else {
        return Ok(());
    };
    for key in ["artifactKind", "submitTool", "writeTargets", "writeMode"] {
        let Some(contract_value) = contract.get(key) else {
            continue;
        };
        let Some(root_value) = root_object.get(key) else {
            continue;
        };
        if root_value != contract_value {
            return Err(StateError::InvalidArgument(format!(
                "native MCP request root {key} conflicts with outputContract.{key}; keep one authoritative value in outputContract"
            )));
        }
        root_object.remove(key);
    }
    Ok(())
}

fn canonicalize_context_refs(root_object: &mut Map<String, Value>, read_groups: &[ReadGroupRef]) {
    let used = used_context_ref_keys(read_groups);
    let Some(context_refs) = root_object
        .get_mut("contextRefs")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let current_keys = context_refs.keys().cloned().collect::<Vec<_>>();
    for key in current_keys {
        if !used.contains(key.as_str()) {
            context_refs.remove(&key);
        }
    }
    if context_refs.is_empty() {
        root_object.remove("contextRefs");
    }
}

fn used_context_ref_keys(read_groups: &[ReadGroupRef]) -> BTreeSet<&'static str> {
    let mut used = BTreeSet::new();
    for field in read_groups.iter().flat_map(|group| group.fields.iter()) {
        if field == "requirementContext.normalizedText" {
            used.insert("normalizedRequirementTextRef");
            continue;
        }
        let Some(root_key) = field.split('.').next() else {
            continue;
        };
        match root_key {
            "requirementContext" => {
                used.insert("requirementContextRef");
            }
            "sourceRefRegistry" => {
                used.insert("requirementContextRef");
            }
            "originalRequirementContext" => {
                used.insert("originalRequirementContextRef");
            }
            "keywordHints" => {
                used.insert("keywordHintsRef");
            }
            "deliveryContext" => {
                used.insert("deliveryContextRef");
            }
            "latestRepositoryContext" => {
                used.insert("latestRepositoryContextRef");
            }
            "latestConfirmedRequirementDecision" => {
                used.insert("latestConfirmedRequirementDecisionRef");
            }
            "confirmedRequirementDecisionsIndex" => {
                used.insert("confirmedRequirementDecisionsIndexRef");
            }
            "deliveryConceptGlossary" => {
                used.insert("deliveryConceptGlossaryRef");
            }
            "phaseConceptGrounding" => {
                used.insert("phaseConceptGroundingRef");
            }
            "currentFrontendExperience" => {
                used.insert("currentFrontendExperienceRef");
            }
            "confirmedClarificationState" => {
                used.insert("clarificationStateRef");
            }
            _ => {}
        }
    }
    used
}

fn used_storage_ref_keys(
    root_object: &Map<String, Value>,
    read_groups: &[ReadGroupRef],
) -> BTreeSet<String> {
    let splittable = SPLITTABLE_REF_KEYS.iter().copied().collect::<BTreeSet<_>>();
    let mut used = read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
        .filter_map(|field| field.split('.').next())
        .filter(|root_key| splittable.contains(root_key))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if root_object.contains_key("outputContract") {
        used.insert("outputContract".to_string());
    }
    if root_object.contains_key("sectionOutputs") {
        used.insert("sectionOutputs".to_string());
    }
    used
}

fn validate_read_plan_contract(
    project_root: &Path,
    root_object: &Map<String, Value>,
    refs: &BTreeMap<String, RequestStorageManifestRef>,
    read_groups: &[ReadGroupRef],
) -> StateResult<Vec<ReadPlanSizeWarning>> {
    let mut warnings = Vec::new();
    for group in read_groups {
        let mut group_bytes = 0usize;
        for field in &group.fields {
            let value = match resolve_field_for_validation(project_root, root_object, refs, field) {
                Ok(value) => value,
                Err(error) if is_field_not_found(&error) => continue,
                Err(error) => {
                    return Err(StateError::InvalidArgument(format!(
                        "requestReadPlan field validation failed: request group {} field {}: {}",
                        group.group_id, field, error
                    )));
                }
            };
            let field_bytes = pretty_len(&value);
            if field_bytes > MAX_READ_FIELD_BYTES {
                warnings.push(ReadPlanSizeWarning {
                    level: "warn",
                    group_id: group.group_id.clone(),
                    field: Some(field.clone()),
                    bytes: field_bytes,
                    limit_bytes: MAX_READ_FIELD_BYTES,
                    message: format!(
                        "requestReadPlan field {} in group {} is too large: {} bytes > {} bytes",
                        field, group.group_id, field_bytes, MAX_READ_FIELD_BYTES
                    ),
                });
            }
            group_bytes += field_bytes;
        }
        if group_bytes > MAX_READ_GROUP_BYTES {
            warnings.push(ReadPlanSizeWarning {
                level: "warn",
                group_id: group.group_id.clone(),
                field: None,
                bytes: group_bytes,
                limit_bytes: MAX_READ_GROUP_BYTES,
                message: format!(
                    "requestReadPlan group {} is too large: {} bytes > {} bytes",
                    group.group_id, group_bytes, MAX_READ_GROUP_BYTES
                ),
            });
        }
    }
    Ok(warnings)
}

fn is_field_not_found(error: &StateError) -> bool {
    matches!(error, StateError::InvalidArgument(message) if message.starts_with("FIELD_NOT_FOUND:"))
}

fn resolve_field_for_validation(
    project_root: &Path,
    root_object: &Map<String, Value>,
    refs: &BTreeMap<String, RequestStorageManifestRef>,
    field: &str,
) -> StateResult<Value> {
    let parts = selector_parts(field)?;
    if let Some(value) =
        resolve_context_ref_for_validation(project_root, root_object, field, &parts)?
    {
        return Ok(value);
    }
    let root_key = parts.first().expect("selector has first part");
    if let Some(ref_entry) = refs.get(root_key) {
        let ref_file = from_project_relative(project_root, &ref_entry.r#ref)?;
        let ref_value = read_json_value(&ref_file)?;
        if root_key == "rules"
            && parts[1..].join(".") == "requirementSemanticGrounding.compactRules"
        {
            return Ok(select_compact_requirement_semantic_rules(&ref_value));
        }
        return if parts.len() == 1 {
            Ok(ref_value)
        } else {
            select_value(&ref_value, &parts[1..])
        };
    }
    let root = Value::Object(root_object.clone());
    if field == "rules.requirementSemanticGrounding.compactRules" {
        Ok(select_compact_requirement_semantic_rules(
            root.get("rules").unwrap_or(&Value::Null),
        ))
    } else {
        select_value(&root, &parts)
    }
}

fn resolve_context_ref_for_validation(
    project_root: &Path,
    root_object: &Map<String, Value>,
    field: &str,
    parts: &[String],
) -> StateResult<Option<Value>> {
    let Some(context_refs) = root_object.get("contextRefs").and_then(Value::as_object) else {
        return Ok(None);
    };
    if field == "requirementContext.normalizedText" {
        if let Some(relative) = context_refs
            .get("normalizedRequirementTextRef")
            .and_then(Value::as_str)
        {
            let text_file = from_project_relative(project_root, relative)?;
            return Ok(Some(Value::String(read_text(&text_file)?)));
        }
    }
    if parts[0] == "sourceRefRegistry" {
        let Some(relative) = context_refs
            .get("requirementContextRef")
            .and_then(Value::as_str)
        else {
            return Ok(None);
        };
        let ref_file = from_project_relative(project_root, relative)?;
        let ref_value = read_json_value(&ref_file)?;
        return Ok(Some(select_source_ref_registry(&ref_value, &parts[1..])?));
    }
    let aliases = [
        ("requirementContext", "requirementContextRef"),
        (
            "originalRequirementContext",
            "originalRequirementContextRef",
        ),
        ("keywordHints", "keywordHintsRef"),
        ("deliveryContext", "deliveryContextRef"),
        ("latestRepositoryContext", "latestRepositoryContextRef"),
        (
            "latestConfirmedRequirementDecision",
            "latestConfirmedRequirementDecisionRef",
        ),
        (
            "confirmedRequirementDecisionsIndex",
            "confirmedRequirementDecisionsIndexRef",
        ),
        ("deliveryConceptGlossary", "deliveryConceptGlossaryRef"),
        ("phaseConceptGrounding", "phaseConceptGroundingRef"),
        ("currentFrontendExperience", "currentFrontendExperienceRef"),
        ("confirmedClarificationState", "clarificationStateRef"),
    ];
    let Some((_alias, ref_field)) = aliases.iter().find(|(alias, _)| parts[0] == *alias) else {
        return Ok(None);
    };
    let Some(relative) = context_refs.get(*ref_field).and_then(Value::as_str) else {
        return Ok(None);
    };
    let ref_file = from_project_relative(project_root, relative)?;
    let ref_value = read_json_value(&ref_file)?;
    let value = if parts.len() == 1 {
        ref_value
    } else if parts[0] == "keywordHints" && parts.get(1).map(String::as_str) == Some("compact") {
        select_compact_keyword_hints(&ref_value, &parts[2..])?
    } else {
        select_value(&ref_value, &parts[1..])?
    };
    Ok(Some(value))
}

fn validate_read_field_selector(field: &str) -> StateResult<String> {
    let field = field.trim();
    if field.is_empty() {
        return Err(StateError::InvalidArgument(
            "read group field selector is required".to_string(),
        ));
    }
    if field.contains(".refs")
        || field.contains("readCommand")
        || field.contains("fallbackRule")
        || field.contains("inspect")
        || field.contains("jq")
        || field.contains(" ")
    {
        return Err(StateError::InvalidArgument(format!(
            "field is not allowed through requestReadPlan.groups: {field}"
        )));
    }
    if FORBIDDEN_EXACT_READ_FIELDS.contains(&field) {
        return Err(StateError::InvalidArgument(format!(
            "read field is too broad for requestReadPlan.groups: {field}"
        )));
    }
    let parts = selector_parts(field)?;
    if parts[0] == "requestManifest" || parts[0] == "agentAction" {
        return Err(StateError::InvalidArgument(format!(
            "field is not allowed through request read protocol: {field}"
        )));
    }
    if parts[0] == "sectionOutputs" {
        return Err(StateError::InvalidArgument(format!(
            "sectionOutputs is private workflow state and must not be exposed through requestReadPlan.groups: {field}"
        )));
    }
    Ok(field.to_string())
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
