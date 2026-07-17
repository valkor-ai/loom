use std::collections::{BTreeMap, BTreeSet};

use delivery_core::{
    FieldReadResult, InspectRequestInput, InspectRequestResult, ReadFieldGroupFields,
    ReadFieldGroupInput, ReadFieldGroupResult, ReadGroupRef, ReadRequestFieldsInput,
    ReadRequestFieldsResult,
};
use serde_json::Value;

use crate::{
    field_projection::{
        select_compact_keyword_hints, select_compact_requirement_semantic_rules,
        select_source_ref_registry, select_value, selector_parts as projection_selector_parts,
    },
    paths::{from_project_relative, project_paths},
    project::read_project_config,
    read_audit::{
        now_for_audit, record_field_read_audit, record_request_inspect_audit, FieldReadAudit,
    },
    request_index::get_request_index_entry,
    request_manifest::{read_group_refs_from_root, request_storage_ref},
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
    let result = inspect_request_inner(&input)?;
    record_request_inspect_audit(&input.project_root, &result.request_ref, &result.request_id);
    Ok(result)
}

pub fn inspect_request_unrecorded(input: InspectRequestInput) -> StateResult<InspectRequestResult> {
    inspect_request_inner(&input)
}

fn inspect_request_inner(input: &InspectRequestInput) -> StateResult<InspectRequestResult> {
    let request = load_request(&input.project_root, &input.request_ref)?;
    let output_contract = read_output_contract(&input.project_root, &request)?;
    Ok(InspectRequestResult {
        request_ref: request.request_ref,
        request_id: request.request_id,
        project_id: request.project_id,
        request_kind: request.request_kind,
        read_groups: request.read_groups,
        write_targets: extract_write_targets(&request.root, output_contract.as_ref()),
        submit_tool: extract_submit_tool(&request.root, output_contract.as_ref()),
    })
}

pub fn read_field_group(input: ReadFieldGroupInput) -> StateResult<ReadFieldGroupResult> {
    let flat = read_field_group_flat(input)?;
    Ok(ReadFieldGroupResult {
        fields: ReadFieldGroupFields::from_flat(flat.fields),
    })
}

pub fn read_field_group_flat(input: ReadFieldGroupInput) -> StateResult<ReadRequestFieldsResult> {
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
    let expanded_fields = group.expanded_fields();
    let fields = resolve_fields(&input.project_root, &request, &expanded_fields)?;
    let contract_fingerprint = fields
        .get("outputContract.contractFingerprint")
        .and_then(|result| result.value.as_str());
    record_field_read_audit(
        &input.project_root,
        FieldReadAudit {
            request_ref: &request.request_ref,
            request_id: &request.request_id,
            fields: &expanded_fields,
            source: "readFieldGroup",
            group_id: Some(&input.group_id),
            contract_fingerprint,
            recorded_at: now_for_audit(),
        },
    );
    Ok(ReadRequestFieldsResult { fields })
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
    let contract_fingerprint = resolved
        .get("outputContract.contractFingerprint")
        .and_then(|result| result.value.as_str());
    record_field_read_audit(
        &input.project_root,
        FieldReadAudit {
            request_ref: &request.request_ref,
            request_id: &request.request_id,
            fields: &fields,
            source: "internalReadRequestFields",
            group_id: None,
            contract_fingerprint,
            recorded_at: now_for_audit(),
        },
    );
    Ok(ReadRequestFieldsResult { fields: resolved })
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
    let root = read_json_value(&request_file)?;
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
        match resolve_field(project_root, request, field) {
            Ok(value) => {
                resolved.insert(field.clone(), value);
            }
            Err(error) if is_field_not_found(&error) => {}
            Err(error) => return Err(error),
        }
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
    if let Some(ref_entry) = request_storage_manifest_ref(project_root, request, root_key)? {
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
        return Ok(field_result(value));
    }
    let value = if field == "rules.requirementSemanticGrounding.compactRules" {
        select_compact_requirement_semantic_rules(request.root.get("rules").unwrap_or(&Value::Null))
    } else {
        select_value(&request.root, &parts)?
    };
    Ok(field_result(value))
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
            return Ok(Some(field_result(Value::String(read_text(&text_file)?))));
        }
    }
    if parts[0] == "sourceRefRegistry" {
        let Some(relative) = context_refs
            .get("requirementContextRef")
            .and_then(Value::as_str)
        else {
            return Ok(None);
        };
        let paths = project_paths(project_root)?;
        let ref_file = from_project_relative(&paths.root, relative)?;
        let ref_value = read_json_value(&ref_file)?;
        return Ok(Some(field_result(select_source_ref_registry(
            &ref_value,
            &parts[1..],
        )?)));
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
    Ok(Some(field_result(value)))
}

fn field_result(value: Value) -> FieldReadResult {
    FieldReadResult { value }
}

fn is_field_not_found(error: &StateError) -> bool {
    matches!(error, StateError::InvalidArgument(message) if message.starts_with("FIELD_NOT_FOUND:"))
}

fn request_storage_manifest_ref(
    project_root: &str,
    request: &LoadedRequest,
    key: &str,
) -> StateResult<Option<String>> {
    let paths = project_paths(project_root)?;
    request_storage_ref(&paths.root, &request.request_id, key)
}

fn selector_parts(field: &str) -> StateResult<Vec<String>> {
    let parts = projection_selector_parts(field)?;
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
        .flat_map(ReadGroupRef::expanded_fields)
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

fn extract_write_targets(root: &Value, output_contract: Option<&Value>) -> Vec<Value> {
    root.get("writeTargets")
        .or_else(|| root.pointer("/outputContract/writeTargets"))
        .or_else(|| output_contract.and_then(|contract| contract.get("writeTargets")))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn extract_submit_tool(root: &Value, output_contract: Option<&Value>) -> Option<String> {
    root.get("submitTool")
        .or_else(|| root.pointer("/outputContract/submitTool"))
        .or_else(|| output_contract.and_then(|contract| contract.get("submitTool")))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn read_output_contract(project_root: &str, request: &LoadedRequest) -> StateResult<Option<Value>> {
    if let Some(value) = request.root.get("outputContract") {
        return Ok(Some(value.clone()));
    }
    let Some(relative) = request_storage_manifest_ref(project_root, request, "outputContract")?
    else {
        return Ok(None);
    };
    let paths = project_paths(project_root)?;
    let ref_file = from_project_relative(&paths.root, &relative)?;
    read_json_value(&ref_file).map(Some)
}
