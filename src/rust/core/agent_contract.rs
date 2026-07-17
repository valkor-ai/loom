use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// The external request protocol stays `outputContract` plus
/// `requestReadPlan.groups`. This descriptor only centralizes how that
/// existing contract is materialized and fingerprinted.
pub const AGENT_WRITE_CONTRACT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentFieldOwner {
    Agent,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentFieldApplicability {
    CurrentRequest,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentFieldPolicy {
    pub owner: AgentFieldOwner,
    pub applicability: AgentFieldApplicability,
    pub empty_policy: String,
    pub preserve_on_repair: bool,
}

impl Default for AgentFieldPolicy {
    fn default() -> Self {
        Self {
            owner: AgentFieldOwner::Agent,
            applicability: AgentFieldApplicability::Unknown,
            empty_policy: "follow_schema_and_applicability".to_string(),
            preserve_on_repair: true,
        }
    }
}

/// Build the compact, tree-shaped field contract from the canonical schema.
/// The full JSON schema remains private; the tree avoids repeating long leaf
/// prefixes in the Agent-facing read group.
pub fn compact_agent_field_contract(
    schema_shape: &Value,
    policies: &BTreeMap<String, AgentFieldPolicy>,
) -> Value {
    compact_agent_field_contract_with_required(schema_shape, &[], policies, None)
}

fn compact_agent_field_contract_with_required(
    schema_shape: &Value,
    required_override: &[String],
    policies: &BTreeMap<String, AgentFieldPolicy>,
    shape_rules: Option<&Value>,
) -> Value {
    let mut contract = json!({
        "version": AGENT_WRITE_CONTRACT_VERSION,
        "authority": "outputContract.schemaShape",
        "defaults": {
            "owner": "agent",
            "applicability": "unknown",
            "emptyPolicy": "follow_schema_and_applicability",
            "preserveOnRepair": true
        },
        "properties": {}
    });
    let required = schema_shape
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .chain(required_override.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(object) = contract.as_object_mut() {
        object.insert(
            "required".to_string(),
            Value::Array(required.iter().map(|field| json!(field)).collect()),
        );
    }
    let Some(output) = contract
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    else {
        return contract;
    };
    let sparse_paths = shape_rules.map(|shape_rules| {
        policies
            .keys()
            .cloned()
            .chain(required.iter().cloned())
            .chain(
                shape_rules
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(path, _)| path.clone()),
            )
            .collect::<BTreeSet<_>>()
    });
    if let Some(properties) = schema_shape.get("properties").and_then(Value::as_object) {
        for (name, schema) in properties {
            let path = name.clone();
            if sparse_paths
                .as_ref()
                .is_some_and(|paths| !path_is_interesting(&path, paths))
            {
                continue;
            }
            output.insert(
                name.clone(),
                compact_schema_node(
                    schema,
                    schema_shape,
                    &path,
                    required.contains(name.as_str()),
                    policies,
                    0,
                    sparse_paths.as_ref(),
                ),
            );
        }
    } else if let Some(properties) = schema_shape.as_object() {
        for (name, shape) in properties {
            if matches!(name.as_str(), "$defs" | "$schema" | "title" | "type") {
                continue;
            }
            let path = name.clone();
            if sparse_paths
                .as_ref()
                .is_some_and(|paths| !path_is_interesting(&path, paths))
            {
                continue;
            }
            output.insert(
                name.clone(),
                compact_manual_node(
                    shape,
                    &path,
                    required.contains(name.as_str()),
                    policies,
                    0,
                    sparse_paths.as_ref(),
                ),
            );
        }
    }
    contract
}

/// Attach the shared contract metadata to the existing outputContract object.
/// Callers may provide field policies for deterministic applicability facts;
/// absent policies default to current-request Agent-owned fields.
pub fn finalize_output_contract(
    output_contract: &mut Value,
    field_policies: &BTreeMap<String, AgentFieldPolicy>,
) {
    let Some(contract) = output_contract.as_object_mut() else {
        return;
    };
    contract.insert(
        "contractVersion".to_string(),
        json!(AGENT_WRITE_CONTRACT_VERSION),
    );
    let mut projection = contract
        .remove("schemaProjection")
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if let Some(projection_object) = projection.as_object_mut() {
        projection_object.remove("fieldContract");
        projection_object.remove("fieldContractByTarget");
    }
    let legacy_shape_rules = projection
        .get("objectShapeRules")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let required_override = projection
        .get("requiredTopLevelFields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let schema_entries = contract
        .iter()
        .filter(|(key, value)| {
            (key.as_str() == "schemaShape" || key.ends_with("SchemaShape")) && value.is_object()
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    let mut field_contracts = Map::new();
    for (key, schema_shape) in schema_entries {
        let mut field_contract = compact_agent_field_contract_with_required(
            &schema_shape,
            if key == "schemaShape" {
                &required_override
            } else {
                &[]
            },
            field_policies,
            Some(&legacy_shape_rules),
        );
        if key == "schemaShape" {
            apply_shape_rule_constraints(&mut field_contract, &legacy_shape_rules);
        }
        field_contracts.insert(schema_target_name(&key), field_contract);
    }
    if let Some(result_contract) = field_contracts.remove("result") {
        projection
            .as_object_mut()
            .expect("schemaProjection is an object")
            .insert("fieldContract".to_string(), result_contract.clone());
        if !field_contracts.is_empty() {
            field_contracts.insert("result".to_string(), result_contract);
        }
    }
    if !field_contracts.is_empty() {
        projection
            .as_object_mut()
            .expect("schemaProjection is an object")
            .insert(
                "fieldContractByTarget".to_string(),
                Value::Object(field_contracts),
            );
    }
    projection
        .as_object_mut()
        .expect("schemaProjection is an object")
        .remove("objectShapeRules");
    contract.insert("schemaProjection".to_string(), projection);
    let mut fingerprint_source = Value::Object(contract.clone());
    remove_volatile_contract_fields(&mut fingerprint_source);
    contract.insert(
        "contractFingerprint".to_string(),
        json!(contract_fingerprint(&fingerprint_source)),
    );
}

fn schema_target_name(key: &str) -> String {
    if key == "schemaShape" {
        return "result".to_string();
    }
    key.strip_suffix("SchemaShape")
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or("result")
        .to_string()
}

/// Derive only deterministic applicability facts from structured request
/// inputs. This intentionally does not inspect requirement prose or search
/// for technology/API keywords.
pub fn derive_agent_field_policies(root: &Value) -> BTreeMap<String, AgentFieldPolicy> {
    let mut policies = BTreeMap::new();
    let api_applicability = if root
        .get("apiQualitySeed")
        .and_then(|value| value.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            root.get("apiQualitySeed")
                .is_some_and(|value| !value.is_null())
        }) {
        AgentFieldApplicability::CurrentRequest
    } else {
        AgentFieldApplicability::NotApplicable
    };
    // `interfaces` remains the architecture boundary even when no HTTP API is
    // selected. Only the API-specific contract is conditional.
    policies.insert(
        "content.interfaces".to_string(),
        policy_for(AgentFieldApplicability::CurrentRequest),
    );
    policies.insert(
        "content.apiContract".to_string(),
        policy_for(api_applicability),
    );
    derive_architecture_field_policies(root, &mut policies);

    let task = root.get("task").unwrap_or(&Value::Null);
    let frontend_experience = task
        .get("frontendExperienceRequirement")
        .filter(|value| !value.is_null());
    let runtime_applies = task
        .pointer("/runtimeDeliveryRequirement/appliesToThisTask")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    policies.insert(
        "frontendExperienceSelfCheck".to_string(),
        policy_for(if frontend_experience.is_some() && !runtime_applies {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    let frontend_quality_applies = frontend_experience.is_some()
        && !runtime_applies
        && frontend_experience
            .and_then(|value| {
                value.pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
            })
            .is_some_and(|value| !value.is_null());
    policies.insert(
        "frontendQualitySelfCheck".to_string(),
        policy_for(if frontend_quality_applies {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies.insert(
        "runtimeDeliveryEvidence".to_string(),
        policy_for(if runtime_applies {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policy_for_task_array_field(
        &mut policies,
        task,
        "apiContractRequirements",
        "apiContractEvidence",
    );
    policy_for_task_array_field(
        &mut policies,
        task,
        "architectureQualityRequirements",
        "architectureQualityEvidence",
    );
    policy_for_task_array_field(
        &mut policies,
        task,
        "codeQualityRequirements",
        "codeQualityEvidence",
    );
    policies
}

fn derive_architecture_field_policies(
    root: &Value,
    policies: &mut BTreeMap<String, AgentFieldPolicy>,
) {
    let persistence = structured_track_applicability(root, "persistence");
    policies.insert(
        "content.dataModel.dataArchitecture.persistenceMode".to_string(),
        policy_for(AgentFieldApplicability::CurrentRequest),
    );
    policies.insert(
        "content.dataModel.dataArchitecture.sourceOfTruth".to_string(),
        policy_for(persistence),
    );
    for field in [
        "ownership",
        "invariants",
        "transactionBoundaries",
        "consistencyRules",
        "migrationImpacts",
        "readModels",
        "lifecyclePolicies",
        "derivedData",
    ] {
        policies.insert(
            format!("content.dataModel.dataArchitecture.{field}"),
            policy_for(persistence),
        );
    }
    let architecture_quality = root
        .pointer("/architectureQualitySeed/required")
        .and_then(Value::as_bool)
        .map(|required| {
            if required {
                AgentFieldApplicability::CurrentRequest
            } else {
                AgentFieldApplicability::NotApplicable
            }
        })
        .unwrap_or(AgentFieldApplicability::Unknown);
    policies.insert(
        "content.architectureQuality".to_string(),
        policy_for(architecture_quality),
    );
}

fn structured_track_applicability(root: &Value, track_name: &str) -> AgentFieldApplicability {
    let track = root
        .pointer(&format!(
            "/contextProjection/technicalBaseline/stack/tracks/{track_name}"
        ))
        .or_else(|| root.pointer(&format!("/technicalBaseline/stack/tracks/{track_name}")));
    match track
        .and_then(Value::as_object)
        .and_then(|track| track.get("status"))
        .and_then(Value::as_str)
    {
        Some("selected" | "user_custom") => AgentFieldApplicability::CurrentRequest,
        Some("not_needed" | "not_applicable") => AgentFieldApplicability::NotApplicable,
        Some(_) | None => AgentFieldApplicability::Unknown,
    }
}

pub fn contract_fingerprint(value: &Value) -> String {
    let canonical = canonical_json(value);
    let digest = Sha256::digest(canonical.as_bytes());
    format!("sha256:{digest:x}")
}

fn compact_schema_node(
    schema: &Value,
    root_schema: &Value,
    path: &str,
    required: bool,
    policies: &BTreeMap<String, AgentFieldPolicy>,
    depth: usize,
    sparse_paths: Option<&BTreeSet<String>>,
) -> Value {
    let schema = if depth < 32 {
        resolved_schema(schema, root_schema, 0)
    } else {
        schema
    };
    let mut node = Map::new();
    let schema_kind = schema_type(schema);
    node.insert("type".to_string(), json!(schema_kind));
    if required {
        node.insert("required".to_string(), json!(true));
    }
    let policy = policies
        .get(path)
        .or_else(|| path.strip_suffix("[]").and_then(|base| policies.get(base)))
        .cloned()
        .unwrap_or_else(|| {
            let mut policy = AgentFieldPolicy::default();
            if required {
                policy.applicability = AgentFieldApplicability::CurrentRequest;
            }
            policy
        });
    add_compact_policy_fields(&mut node, &policy);
    if let Some(enum_values) = schema.get("enum") {
        node.insert("enum".to_string(), enum_values.clone());
    }
    if schema.get("type").and_then(Value::as_str) == Some("object") {
        if let Some(required_fields) = schema.get("required") {
            node.insert("requiredFields".to_string(), required_fields.clone());
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            let nested = properties
                .iter()
                .filter(|(name, _)| {
                    sparse_paths
                        .is_none_or(|paths| path_is_interesting(&format!("{path}.{name}"), paths))
                })
                .map(|(name, child)| {
                    let child_path = format!("{path}.{name}");
                    (
                        name.clone(),
                        compact_schema_node(
                            child,
                            root_schema,
                            &child_path,
                            schema
                                .get("required")
                                .and_then(Value::as_array)
                                .is_some_and(|items| {
                                    items.iter().any(|item| item.as_str() == Some(name))
                                }),
                            policies,
                            depth + 1,
                            sparse_paths,
                        ),
                    )
                })
                .collect::<Map<_, _>>();
            node.insert("properties".to_string(), Value::Object(nested));
        }
    }
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get("items") {
            node.insert(
                "items".to_string(),
                compact_schema_node(
                    items,
                    root_schema,
                    &format!("{path}[]"),
                    true,
                    policies,
                    depth + 1,
                    sparse_paths,
                ),
            );
        }
    }
    Value::Object(node)
}

fn resolved_schema<'a>(schema: &'a Value, root_schema: &'a Value, depth: usize) -> &'a Value {
    if depth >= 32 {
        return schema;
    }
    if let Some(reference) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/"))
        .and_then(|reference| root_schema.pointer(&format!("/{reference}")))
    {
        return resolved_schema(reference, root_schema, depth + 1);
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(key).and_then(Value::as_array) {
            if let Some(variant) = variants
                .iter()
                .find(|variant| variant.get("type").and_then(Value::as_str) != Some("null"))
            {
                return resolved_schema(variant, root_schema, depth + 1);
            }
        }
    }
    schema
}

fn compact_manual_node(
    shape: &Value,
    path: &str,
    required: bool,
    policies: &BTreeMap<String, AgentFieldPolicy>,
    depth: usize,
    sparse_paths: Option<&BTreeSet<String>>,
) -> Value {
    if depth > 32 {
        return json!({"type": "unknown", "required": required});
    }
    let mut node = Map::new();
    let (kind, nested_shape) = match shape {
        Value::Object(object) => ("object", Some(Value::Object(object.clone()))),
        Value::Array(items) => (
            "array",
            items.first().cloned().map(|item| Value::Array(vec![item])),
        ),
        Value::Bool(_) => ("boolean", None),
        Value::Number(_) => ("number", None),
        Value::String(description) => {
            if matches!(
                description.trim(),
                "object" | "string" | "boolean" | "number" | "array"
            ) {
                return add_policy(
                    json!({"type": description.trim(), "required": required}),
                    path,
                    required,
                    policies,
                );
            }
            let mut value = json!({"type": "string", "required": required});
            if description.contains(" | ") {
                value["enum"] = Value::Array(
                    description
                        .split(" | ")
                        .map(|item| json!(item.trim()))
                        .collect(),
                );
            } else if !description.is_empty() {
                value["description"] = json!(description);
            }
            return add_policy(value, path, required, policies);
        }
        Value::Null => ("null", None),
    };
    node.insert("type".to_string(), json!(kind));
    if required {
        node.insert("required".to_string(), json!(true));
    }
    let policy = policies.get(path).cloned().unwrap_or_else(|| {
        if required {
            AgentFieldPolicy {
                applicability: AgentFieldApplicability::CurrentRequest,
                ..AgentFieldPolicy::default()
            }
        } else {
            AgentFieldPolicy::default()
        }
    });
    add_compact_policy_fields(&mut node, &policy);
    match kind {
        "object" => {
            if let Some(Value::Object(properties)) = nested_shape {
                let nested = properties
                    .iter()
                    .filter(|(name, _)| {
                        sparse_paths.is_none_or(|paths| {
                            path_is_interesting(&format!("{path}.{name}"), paths)
                        })
                    })
                    .map(|(name, child)| {
                        (
                            name.clone(),
                            compact_manual_node(
                                child,
                                &format!("{path}.{name}"),
                                false,
                                policies,
                                depth + 1,
                                sparse_paths,
                            ),
                        )
                    })
                    .collect::<Map<_, _>>();
                node.insert("properties".to_string(), Value::Object(nested));
            }
        }
        "array" => {
            if let Some(Value::Array(items)) = nested_shape {
                if let Some(item) = items.first() {
                    node.insert(
                        "items".to_string(),
                        compact_manual_node(
                            item,
                            &format!("{path}[]"),
                            true,
                            policies,
                            depth + 1,
                            sparse_paths,
                        ),
                    );
                }
            }
        }
        _ => {}
    }
    Value::Object(node)
}

fn path_is_interesting(path: &str, paths: &BTreeSet<String>) -> bool {
    paths.iter().any(|candidate| {
        candidate == path
            || candidate.starts_with(&format!("{path}."))
            || candidate.starts_with(&format!("{path}[]"))
    })
}

fn add_policy(
    mut value: Value,
    path: &str,
    required: bool,
    policies: &BTreeMap<String, AgentFieldPolicy>,
) -> Value {
    let policy = policies.get(path).cloned().unwrap_or_else(|| {
        if required {
            AgentFieldPolicy {
                applicability: AgentFieldApplicability::CurrentRequest,
                ..AgentFieldPolicy::default()
            }
        } else {
            AgentFieldPolicy::default()
        }
    });
    if let Some(object) = value.as_object_mut() {
        add_compact_policy_fields(object, &policy);
    }
    value
}

fn add_compact_policy_fields(node: &mut Map<String, Value>, policy: &AgentFieldPolicy) {
    if policy.owner != AgentFieldOwner::Agent {
        node.insert("owner".to_string(), json!(policy.owner));
    }
    if policy.applicability != AgentFieldApplicability::Unknown {
        node.insert("applicability".to_string(), json!(policy.applicability));
    }
    if policy.empty_policy != "follow_schema_and_applicability" {
        node.insert("emptyPolicy".to_string(), json!(policy.empty_policy));
    }
    if !policy.preserve_on_repair {
        node.insert("preserveOnRepair".to_string(), json!(false));
    }
}

fn apply_shape_rule_constraints(field_contract: &mut Value, shape_rules: &Value) {
    let Some(rules) = shape_rules.as_object() else {
        return;
    };
    for (path, rule) in rules {
        let Some(rule) = rule.as_str() else {
            continue;
        };
        let Some(node) = field_contract_node_mut(field_contract, path) else {
            continue;
        };
        node.as_object_mut()
            .expect("field contract node is an object")
            .entry("constraints".to_string())
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .expect("field contract constraints are an array")
            .push(json!(rule));
    }
}

fn field_contract_node_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut current = root.get_mut("properties").and_then(Value::as_object_mut)?;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        let is_array = part.ends_with("[]");
        let name = part.strip_suffix("[]").unwrap_or(part);
        let node = current.get_mut(name)?;
        if parts.peek().is_none() {
            return Some(node);
        }
        if is_array {
            current = node
                .get_mut("items")
                .and_then(Value::as_object_mut)
                .and_then(|items| items.get_mut("properties"))
                .and_then(Value::as_object_mut)?;
        } else {
            current = node.get_mut("properties").and_then(Value::as_object_mut)?;
        }
    }
    None
}

fn policy_for(applicability: AgentFieldApplicability) -> AgentFieldPolicy {
    AgentFieldPolicy {
        applicability,
        ..AgentFieldPolicy::default()
    }
}

fn policy_for_task_array_field(
    policies: &mut BTreeMap<String, AgentFieldPolicy>,
    task: &Value,
    task_field: &str,
    result_field: &str,
) {
    let applicability = if task
        .get(task_field)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        AgentFieldApplicability::CurrentRequest
    } else {
        AgentFieldApplicability::NotApplicable
    };
    policies.insert(result_field.to_string(), policy_for(applicability));
}

fn schema_type(schema: &Value) -> String {
    schema
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            schema
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.rsplit('/').next())
                .map(|name| format!("object:{name}"))
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn remove_volatile_contract_fields(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for field in [
        "contractFingerprint",
        "writeTargets",
        "resultFile",
        "candidateFile",
        "outlineFile",
        "groupFilePattern",
        "path",
    ] {
        object.remove(field);
    }
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}
