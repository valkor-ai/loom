use std::collections::{BTreeMap, BTreeSet};

use delivery_core::{
    FieldReadResult, FieldReadStatus, FieldSourceKind, InspectRequestInput, InspectRequestResult,
    ReadFieldGroupInput, ReadFieldGroupResult, ReadGroupRef, ReadRequestFieldsInput,
    ReadRequestFieldsResult,
};
use serde_json::Value;

use crate::{
    legacy_ts_reader::hydrate_legacy_request_value,
    paths::{from_project_relative, project_paths},
    project::read_project_config,
    read_audit::{now_for_audit, record_field_read_audit, FieldReadAudit},
    request_index::{get_request_index_entry, RequestSourceProtocol},
    request_manifest::{encode_component, read_group_refs_from_root},
    store::{read_json_value, read_text, StateError, StateResult},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRequestRef {
    project_id: String,
    request_id: String,
}

#[derive(Debug)]
struct LoadedRequest {
    request_ref: String,
    request_id: String,
    project_id: String,
    request_kind: String,
    root: Value,
    read_groups: Vec<ReadGroupRef>,
}

pub fn inspect_request(input: InspectRequestInput) -> StateResult<InspectRequestResult> {
    let request = load_request(&input.project_root, &input.request_ref)?;
    Ok(InspectRequestResult {
        request_ref: request.request_ref,
        request_id: request.request_id,
        project_id: request.project_id,
        request_kind: request.request_kind,
        read_groups: request.read_groups,
        write_targets: extract_write_targets(&request.root),
        submit_tool: extract_submit_tool(&request.root),
    })
}

pub fn read_field_group(input: ReadFieldGroupInput) -> StateResult<ReadFieldGroupResult> {
    let request = load_request(&input.project_root, &input.request_ref)?;
    let group = request
        .read_groups
        .iter()
        .find(|group| group.group_id == input.group_id)
        .ok_or_else(|| {
            StateError::InvalidArgument(format!(
                "UNKNOWN_FIELD_GROUP: {}. Available groups: {}",
                input.group_id,
                request
                    .read_groups
                    .iter()
                    .map(|group| group.group_id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        })?
        .clone();
    let fields = resolve_fields(&input.project_root, &request, &group.fields)?;
    record_field_read_audit(
        &input.project_root,
        FieldReadAudit {
            request_ref: &request.request_ref,
            request_id: &request.request_id,
            fields: &group.fields,
            source: "readFieldGroup",
            recorded_at: now_for_audit(),
        },
    );
    Ok(ReadFieldGroupResult {
        request_ref: request.request_ref,
        request_id: request.request_id,
        group_id: group.group_id,
        required: group.required,
        order: group.order,
        fields,
    })
}

pub fn read_request_fields(input: ReadRequestFieldsInput) -> StateResult<ReadRequestFieldsResult> {
    let request = load_request(&input.project_root, &input.request_ref)?;
    let fields = dedupe(input.fields);
    if fields.is_empty() {
        return Err(StateError::InvalidArgument(
            "fields must include at least one field".to_string(),
        ));
    }
    if fields.len() > 20 {
        return Err(StateError::InvalidArgument(
            "readRequestFields accepts at most 20 fields".to_string(),
        ));
    }
    let allowed = allowed_fields(&request.read_groups);
    let forbidden: Vec<String> = fields
        .iter()
        .filter(|field| !allowed.contains(*field))
        .cloned()
        .collect();
    if !forbidden.is_empty() {
        return Err(StateError::InvalidArgument(format!(
            "FIELD_NOT_ALLOWED: {}. Available groups: {}",
            forbidden.join(","),
            request
                .read_groups
                .iter()
                .map(|group| group.group_id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )));
    }
    let resolved = resolve_fields(&input.project_root, &request, &fields)?;
    record_field_read_audit(
        &input.project_root,
        FieldReadAudit {
            request_ref: &request.request_ref,
            request_id: &request.request_id,
            fields: &fields,
            source: "readRequestFields",
            recorded_at: now_for_audit(),
        },
    );
    Ok(ReadRequestFieldsResult {
        request_ref: request.request_ref,
        request_id: request.request_id,
        fields: resolved,
    })
}

pub fn read_field_group_by_resource_uri(uri: &str) -> StateResult<ReadFieldGroupResult> {
    let parsed = parse_field_group_resource(uri)?;
    let project_root = crate::project::project_root_for_project_id(&parsed.project_id)?;
    read_field_group(ReadFieldGroupInput {
        project_root: project_root.to_string_lossy().into_owned(),
        request_ref: crate::request_manifest::request_ref(&parsed.project_id, &parsed.request_id),
        group_id: parsed.group_id,
    })
}

pub fn read_field_by_resource_uri(uri: &str) -> StateResult<ReadRequestFieldsResult> {
    let parsed = parse_field_resource(uri)?;
    let project_root = crate::project::project_root_for_project_id(&parsed.project_id)?;
    read_request_fields(ReadRequestFieldsInput {
        project_root: project_root.to_string_lossy().into_owned(),
        request_ref: crate::request_manifest::request_ref(&parsed.project_id, &parsed.request_id),
        fields: vec![parsed.field_path],
    })
}

fn load_request(project_root: &str, request_ref: &str) -> StateResult<LoadedRequest> {
    let parsed = parse_request_ref(request_ref)?;
    let config = read_project_config(project_root)?;
    if config.project_id != parsed.project_id {
        return Err(StateError::InvalidArgument(format!(
            "requestRef projectId {} does not match project root projectId {}",
            parsed.project_id, config.project_id
        )));
    }
    let index_entry = get_request_index_entry(project_root, &parsed.request_id)?;
    let paths = project_paths(project_root)?;
    let request_file = from_project_relative(&paths.root, &index_entry.request_file)?;
    let mut root = read_json_value(&request_file)?;
    if index_entry.source_protocol == RequestSourceProtocol::LegacyTypeScript {
        root = hydrate_legacy_request_value(root, &parsed.project_id, &parsed.request_id)?;
    }
    let read_groups = read_group_refs_from_root(&root, &parsed.project_id, &parsed.request_id)?;
    Ok(LoadedRequest {
        request_ref: request_ref.to_string(),
        request_id: parsed.request_id,
        project_id: parsed.project_id,
        request_kind: index_entry.request_kind,
        root,
        read_groups,
    })
}

fn resolve_fields(
    project_root: &str,
    request: &LoadedRequest,
    fields: &[String],
) -> StateResult<BTreeMap<String, FieldReadResult>> {
    let mut resolved = BTreeMap::new();
    for field in fields {
        resolved.insert(field.clone(), resolve_field(project_root, request, field)?);
    }
    Ok(resolved)
}

fn resolve_field(
    project_root: &str,
    request: &LoadedRequest,
    field: &str,
) -> StateResult<FieldReadResult> {
    let parts = selector_parts(field)?;
    if let Some(context_result) = resolve_context_ref_field(project_root, request, field, &parts)? {
        return Ok(context_result);
    }
    let root_key = parts.first().expect("selector has first part");
    if let Some(ref_entry) = request_manifest_ref(&request.root, root_key) {
        let paths = project_paths(project_root)?;
        let ref_file = from_project_relative(&paths.root, &ref_entry)?;
        let ref_value = read_json_value(&ref_file)?;
        let value = if root_key == "rules"
            && parts[1..].join(".") == "requirementSemanticGrounding.compactRules"
        {
            select_compact_requirement_semantic_rules(&ref_value)
        } else if parts.len() == 1 {
            ref_value
        } else {
            select_value(&ref_value, &parts[1..])?
        };
        return Ok(field_result(
            request,
            field,
            value,
            FieldSourceKind::RequestManifestRef,
            format!(".{}", parts[1..].join(".")),
        ));
    }
    let value = if field == "rules.requirementSemanticGrounding.compactRules" {
        select_compact_requirement_semantic_rules(request.root.get("rules").unwrap_or(&Value::Null))
    } else {
        select_value(&request.root, &parts)?
    };
    Ok(field_result(
        request,
        field,
        value,
        FieldSourceKind::RequestRoot,
        format!(".{}", parts.join(".")),
    ))
}

fn resolve_context_ref_field(
    project_root: &str,
    request: &LoadedRequest,
    field: &str,
    parts: &[String],
) -> StateResult<Option<FieldReadResult>> {
    let Some(context_refs) = request.root.get("contextRefs").and_then(Value::as_object) else {
        return Ok(None);
    };
    if field == "requirementContext.normalizedText" {
        if let Some(relative) = context_refs
            .get("normalizedRequirementTextRef")
            .and_then(Value::as_str)
        {
            let paths = project_paths(project_root)?;
            let text_file = from_project_relative(&paths.root, relative)?;
            return Ok(Some(field_result(
                request,
                field,
                Value::String(read_text(&text_file)?),
                FieldSourceKind::TextRef,
                "$".to_string(),
            )));
        }
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
    ];
    let Some((_alias, ref_field)) = aliases.iter().find(|(alias, _)| parts[0] == *alias) else {
        return Ok(None);
    };
    let Some(relative) = context_refs.get(*ref_field).and_then(Value::as_str) else {
        return Ok(None);
    };
    let paths = project_paths(project_root)?;
    let ref_file = from_project_relative(&paths.root, relative)?;
    let ref_value = read_json_value(&ref_file)?;
    let value = if parts.len() == 1 {
        ref_value
    } else if parts[0] == "keywordHints" && parts.get(1).map(String::as_str) == Some("compact") {
        select_compact_keyword_hints(&ref_value, &parts[2..])?
    } else {
        select_value(&ref_value, &parts[1..])?
    };
    Ok(Some(field_result(
        request,
        field,
        value,
        FieldSourceKind::ContextRef,
        format!(".{}", parts[1..].join(".")),
    )))
}

fn field_result(
    request: &LoadedRequest,
    field: &str,
    value: Value,
    source_kind: FieldSourceKind,
    selector: String,
) -> FieldReadResult {
    FieldReadResult {
        status: FieldReadStatus::Resolved,
        value,
        source_ref: format!(
            "loom://projects/{}/requests/{}/fields/{}",
            request.project_id,
            request.request_id,
            encode_component(field)
        ),
        source_kind,
        selector,
    }
}

fn request_manifest_ref(root: &Value, key: &str) -> Option<String> {
    root.get("requestManifest")
        .and_then(|manifest| manifest.get("refs"))
        .and_then(|refs| refs.get(key))
        .and_then(|entry| entry.get("ref"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn select_compact_keyword_hints(root: &Value, selector_parts: &[String]) -> StateResult<Value> {
    let compact = compact_keyword_hints(root);
    if selector_parts.is_empty() {
        return Ok(compact);
    }
    select_value(&compact, selector_parts)
}

fn compact_keyword_hints(root: &Value) -> Value {
    let Some(object) = root.as_object() else {
        return serde_json::json!({
            "usage": "advisory_only",
            "status": "empty",
            "languageHints": [],
            "topKeywords": [],
            "sectionKeywords": [],
            "rules": keyword_hint_compact_rules(),
        });
    };
    if let Some(compact) = object.get("compact").and_then(Value::as_object) {
        return Value::Object(compact.clone());
    }

    let language_hints = object
        .get("languageHints")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .take(5)
                .map(|item| Value::String(item.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let top_keywords = object
        .get("globalKeywords")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_object)
                .take(16)
                .filter_map(|hint| {
                    let keyword = hint.get("keyword").and_then(Value::as_str)?;
                    if keyword.is_empty() {
                        return None;
                    }
                    let occurrences = hint.get("occurrences").and_then(Value::as_u64).unwrap_or(0);
                    let source_item_ids = hint
                        .get("sourceItemIds")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .take(3)
                                .map(|item| Value::String(item.to_string()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    Some(serde_json::json!({
                        "keyword": keyword,
                        "occurrences": occurrences,
                        "sourceItemIds": source_item_ids,
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let section_keywords = object
        .get("sectionKeywords")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_object)
                .take(6)
                .filter_map(|section| {
                    let section_id = section
                        .get("sectionId")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let title = section.get("title").and_then(Value::as_str);
                    let source_item_id = section
                        .get("sourceItemId")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let keywords = section
                        .get("keywords")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_object)
                                .take(6)
                                .filter_map(|hint| hint.get("keyword").and_then(Value::as_str))
                                .filter(|keyword| !keyword.is_empty())
                                .map(|keyword| Value::String(keyword.to_string()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if section_id.is_empty() && keywords.is_empty() {
                        return None;
                    }
                    let mut value = serde_json::Map::new();
                    value.insert(
                        "sectionId".to_string(),
                        Value::String(section_id.to_string()),
                    );
                    value.insert(
                        "sourceItemId".to_string(),
                        Value::String(source_item_id.to_string()),
                    );
                    if let Some(title) = title {
                        value.insert("title".to_string(), Value::String(title.to_string()));
                    }
                    value.insert("keywords".to_string(), Value::Array(keywords));
                    Some(Value::Object(value))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    serde_json::json!({
        "usage": "advisory_only",
        "status": if object.get("status").and_then(Value::as_str) == Some("completed") {
            "completed"
        } else {
            "empty"
        },
        "languageHints": language_hints,
        "topKeywords": top_keywords,
        "sectionKeywords": section_keywords,
        "rules": keyword_hint_compact_rules(),
    })
}

fn keyword_hint_compact_rules() -> Value {
    serde_json::json!({
        "advisoryOnly": true,
        "mustNotTreatAsScope": true,
        "mustNotTreatAsAcceptance": true,
        "ignoreWhenIrrelevant": true,
    })
}

fn select_compact_requirement_semantic_rules(root: &Value) -> Value {
    let Some(object) = root.as_object() else {
        return Value::Array(vec![]);
    };
    let Some(semantic) = object
        .get("requirementSemanticGrounding")
        .and_then(Value::as_object)
    else {
        return Value::Array(vec![]);
    };
    if let Some(compact_rules) = semantic.get("compactRules").and_then(Value::as_array) {
        return Value::Array(
            compact_rules
                .iter()
                .filter_map(Value::as_str)
                .map(|item| Value::String(item.to_string()))
                .collect(),
        );
    }
    Value::Array(
        semantic
            .get("rules")
            .and_then(Value::as_array)
            .map(|rules| {
                rules
                    .iter()
                    .filter_map(Value::as_str)
                    .take(7)
                    .map(|item| Value::String(item.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )
}

fn select_value(root: &Value, parts: &[String]) -> StateResult<Value> {
    let mut current = root;
    for part in parts {
        match current {
            Value::Object(object) => {
                current = object.get(part).ok_or_else(|| {
                    StateError::InvalidArgument(format!("FIELD_NOT_FOUND: {}", parts.join(".")))
                })?;
            }
            Value::Array(array) => {
                let index = part.parse::<usize>().map_err(|_| {
                    StateError::InvalidArgument(format!("invalid array index in selector: {part}"))
                })?;
                current = array.get(index).ok_or_else(|| {
                    StateError::InvalidArgument(format!("array index out of bounds: {part}"))
                })?;
            }
            _ => {
                return Err(StateError::InvalidArgument(format!(
                    "selector cannot traverse non-container value at {part}"
                )));
            }
        }
    }
    Ok(current.clone())
}

fn selector_parts(field: &str) -> StateResult<Vec<String>> {
    let parts = field
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(StateError::InvalidArgument(
            "field selector is required".to_string(),
        ));
    }
    if parts[0] == "requestManifest" || parts[0] == "agentAction" {
        return Err(StateError::InvalidArgument(format!(
            "field is not allowed through request read protocol: {field}"
        )));
    }
    Ok(parts)
}

fn parse_request_ref(request_ref: &str) -> StateResult<ParsedRequestRef> {
    let prefix = "loom://projects/";
    let rest = request_ref.strip_prefix(prefix).ok_or_else(|| {
        StateError::InvalidArgument("requestRef must start with loom://projects/".to_string())
    })?;
    let (project_id, rest) = rest.split_once("/requests/").ok_or_else(|| {
        StateError::InvalidArgument("requestRef must include /requests/".to_string())
    })?;
    if project_id.is_empty() || rest.is_empty() || rest.contains('/') {
        return Err(StateError::InvalidArgument(format!(
            "invalid requestRef: {request_ref}"
        )));
    }
    Ok(ParsedRequestRef {
        project_id: project_id.to_string(),
        request_id: rest.to_string(),
    })
}

struct ParsedGroupResource {
    project_id: String,
    request_id: String,
    group_id: String,
}

struct ParsedFieldResource {
    project_id: String,
    request_id: String,
    field_path: String,
}

fn parse_field_group_resource(uri: &str) -> StateResult<ParsedGroupResource> {
    let prefix = "loom://projects/";
    let rest = uri.strip_prefix(prefix).ok_or_else(|| {
        StateError::InvalidArgument("resource URI must start with loom://projects/".to_string())
    })?;
    let (project_id, rest) = rest.split_once("/requests/").ok_or_else(|| {
        StateError::InvalidArgument("resource URI must include /requests/".to_string())
    })?;
    let (request_id, group_id) = rest.split_once("/field-groups/").ok_or_else(|| {
        StateError::InvalidArgument("resource URI must include /field-groups/".to_string())
    })?;
    Ok(ParsedGroupResource {
        project_id: project_id.to_string(),
        request_id: request_id.to_string(),
        group_id: decode_component(group_id)?,
    })
}

fn parse_field_resource(uri: &str) -> StateResult<ParsedFieldResource> {
    let prefix = "loom://projects/";
    let rest = uri.strip_prefix(prefix).ok_or_else(|| {
        StateError::InvalidArgument("resource URI must start with loom://projects/".to_string())
    })?;
    let (project_id, rest) = rest.split_once("/requests/").ok_or_else(|| {
        StateError::InvalidArgument("resource URI must include /requests/".to_string())
    })?;
    let (request_id, field_path) = rest.split_once("/fields/").ok_or_else(|| {
        StateError::InvalidArgument("resource URI must include /fields/".to_string())
    })?;
    Ok(ParsedFieldResource {
        project_id: project_id.to_string(),
        request_id: request_id.to_string(),
        field_path: decode_component(field_path)?,
    })
}

fn decode_component(value: &str) -> StateResult<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(StateError::InvalidArgument(
                    "invalid percent escape".to_string(),
                ));
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| StateError::InvalidArgument("invalid percent escape".to_string()))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| StateError::InvalidArgument("invalid percent escape".to_string()))?;
            output.push(byte);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| {
        StateError::InvalidArgument("resource URI component is not valid UTF-8".to_string())
    })
}

fn allowed_fields(groups: &[ReadGroupRef]) -> BTreeSet<String> {
    groups
        .iter()
        .flat_map(|group| group.fields.iter().cloned())
        .collect()
}

fn dedupe(fields: Vec<String>) -> Vec<String> {
    fields
        .into_iter()
        .map(|field| field.trim().to_string())
        .filter(|field| !field.is_empty())
        .fold(Vec::new(), |mut acc, field| {
            if !acc.contains(&field) {
                acc.push(field);
            }
            acc
        })
}

fn extract_write_targets(root: &Value) -> Vec<Value> {
    root.get("writeTargets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn extract_submit_tool(root: &Value) -> Option<String> {
    root.get("submitTool")
        .and_then(Value::as_str)
        .map(str::to_string)
}
