use std::collections::BTreeSet;

use serde_json::{json, Value};

pub fn build_api_quality_seed_from_foundation(
    foundation_content: &Value,
    project_api_contract: Option<&Value>,
) -> Value {
    let signals = collect_api_seed_signals(foundation_content, project_api_contract);
    if signals.http_interaction_ids.is_empty() {
        return Value::Null;
    }
    let mut api_groups = vec![
        "core".to_string(),
        "resource".to_string(),
        "errors".to_string(),
    ];
    if signals.security_required {
        api_groups.push("security".to_string());
    }
    if signals.pagination_required {
        api_groups.push("pagination".to_string());
    }
    if signals.contract_artifact_required {
        api_groups.push("contract".to_string());
    }
    if signals.compatibility_required {
        api_groups.push("evolution".to_string());
    }
    if signals.operations_required {
        api_groups.push("operations".to_string());
    }
    let reference_load_plan = api_reference_load_plan(&api_groups);
    json!({
        "required": true,
        "qualityLevel": "production_api_contract",
        "selectionReason": format!(
            "Accepted Foundation interactions declare current-phase HTTP boundaries: {}.",
            signals.http_interaction_ids.join(", ")
        ),
        "techReferenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": {
                "api": api_groups
            },
            "referenceLoadPlan": reference_load_plan
        },
        "interfaceContract": {
            "appliesTo": "Architecture content.interfaces entries with type=http_api or task-owned HTTP API bindings.",
            "requiredFields": [
                "interfaceId",
                "name",
                "type",
                "resource",
                "operationKind",
                "method",
                "path",
                "requestSchema",
                "responseSchema",
                "statusCodes",
                "errorSchema",
                "scopeRefs",
                "acceptanceRefs"
            ],
            "apiSurfaceFields": [
                "publicExposure.basePath",
                "publicExposure.preservePath",
                "browserBinding.mode",
                "browserBinding.baseUrl",
                "browserBinding.pathOwnership"
            ],
            "conditionalFields": [
                "paginationPolicy",
                "filterFields",
                "sortFields",
                "authPolicy",
                "contractFileRefs",
                "compatibilityPolicy",
                "idempotencyPolicy",
                "cachePolicy",
                "conditionalRequestPolicy",
                "rateLimitPolicy",
                "retryPolicy",
                "requestIdPolicy",
                "normalization"
            ]
        },
        "generationRules": [
            "Use apiQualitySeed only for HTTP interactions declared by the accepted Foundation; do not infer API work from prose or a backend-capable stack.",
            "Represent API contracts in Architecture interfaces and downstream apiContractRequirements; do not paste API reference prose into candidates.",
            "Declare publicExposure and browserBinding once at the accepted API contract level. RuntimeDelivery, TaskPlan, Execution, Review, and Deploy consume that contract; they must not invent a second API base prefix.",
            "Do not author runtime httpProbes.apiPaths or api.probePaths. Loom derives probe paths from accepted HTTP interface paths after architecture acceptance.",
            "Read only files listed in techReferenceProfile.referenceLoadPlan; selected API groups are semantic evidence labels, not path maps.",
            "Do not add versioned paths or deprecation policy unless techReferenceProfile.referenceLoadPlan selects tech/api/evolution.md.",
            "Do not require OpenAPI files unless techReferenceProfile.referenceLoadPlan selects tech/api/contract.md or the repository already owns one.",
            "Do not add authPolicy or authentication infrastructure unless techReferenceProfile.referenceLoadPlan selects tech/api/security.md or the accepted interface already has an auth policy.",
            "Do not add idempotency, cache, rate-limit, retry, or request-id infrastructure unless techReferenceProfile.referenceLoadPlan selects tech/api/operations.md or the repository already owns that convention."
        ]
    })
}

pub fn api_reference_load_plan(api_groups: &[String]) -> Vec<Value> {
    api_groups
        .iter()
        .map(|group| {
            json!({
                "refId": format!("tech.api.{group}"),
                "path": format!("tech/api/{group}.md"),
                "reason": format!("Selected API {group} quality reference for current-phase interface design.")
            })
        })
        .collect()
}

pub fn api_quality_seed_read_fields() -> [&'static str; 8] {
    [
        "apiQualitySeed.required",
        "apiQualitySeed.qualityLevel",
        "apiQualitySeed.selectionReason",
        "apiQualitySeed.techReferenceProfile.loadMode",
        "apiQualitySeed.techReferenceProfile.groups.api",
        "apiQualitySeed.techReferenceProfile.referenceLoadPlan",
        "apiQualitySeed.interfaceContract",
        "apiQualitySeed.generationRules",
    ]
}

pub fn api_quality_enum_refs() -> Value {
    json!({
        "knownReferenceGroups": {
            "api": ["core", "resource", "errors", "pagination", "contract", "security", "evolution", "operations"]
        },
        "interfaceType": ["http_api", "service_method", "external_adapter", "event", "job", "cli_command"],
        "httpMethod": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"],
        "operationKind": ["create", "read_list", "read_detail", "replace", "update", "delete", "state_transition", "domain_action", "search", "export"],
        "paginationStrategy": ["not_applicable", "page_size", "offset_limit", "cursor", "keyset"],
        "authRequirement": ["not_applicable", "required", "optional", "deferred_with_risk"],
        "statusCodeCategory": ["success", "validation", "business_conflict", "not_found", "auth", "rate_limit", "service_unavailable", "server_error"],
        "contractArtifact": ["aac_interface", "openapi", "schema_file", "source_code", "test"]
    })
}

#[derive(Default)]
struct ApiSeedSignals {
    http_interaction_ids: Vec<String>,
    security_required: bool,
    pagination_required: bool,
    contract_artifact_required: bool,
    compatibility_required: bool,
    operations_required: bool,
}

fn collect_api_seed_signals(
    foundation_content: &Value,
    project_api_contract: Option<&Value>,
) -> ApiSeedSignals {
    let mut signals = ApiSeedSignals::default();
    let interactions = foundation_content
        .pointer("/engineeringBoundary/applicationInteractions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut project_http_refs = BTreeSet::<String>::new();
    let mut project_secured_http_refs = BTreeSet::<String>::new();
    for interface in project_api_contract
        .and_then(|contract| contract.get("interfaces"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|interface| interface.get("type").and_then(Value::as_str) == Some("http_api"))
    {
        let Some(interface_id) = interface.get("interfaceId").and_then(Value::as_str) else {
            continue;
        };
        project_http_refs.insert(interface_id.to_string());
        if interface_auth_policy_applies(interface) {
            project_secured_http_refs.insert(interface_id.to_string());
        }
    }

    for (index, interaction) in interactions.iter().enumerate() {
        let direct_http =
            interaction.get("interactionType").and_then(Value::as_str) == Some("http_api");
        let interface_refs = interaction
            .get("interfaceRefs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        let references_http = interface_refs
            .iter()
            .any(|interface_ref| project_http_refs.contains(*interface_ref));
        if !direct_http && !references_http {
            continue;
        }
        signals.http_interaction_ids.push(
            interaction
                .get("interactionId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("interaction-{index}")),
        );
        let traits = interaction.get("qualityTraits").unwrap_or(&Value::Null);
        signals.security_required |= auth_requirement_selects_security(traits)
            || interface_refs
                .iter()
                .any(|interface_ref| project_secured_http_refs.contains(*interface_ref));
        signals.pagination_required |= bool_at(traits, "paginationRequired");
        signals.contract_artifact_required |= bool_at(traits, "contractArtifactRequired");
        signals.compatibility_required |= bool_at(traits, "compatibilityRequired");
        signals.operations_required |= traits
            .get("operationalPolicies")
            .and_then(Value::as_array)
            .is_some_and(|policies| !policies.is_empty());
    }
    signals.http_interaction_ids.sort();
    signals.http_interaction_ids.dedup();
    signals
}

fn bool_at(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn auth_requirement_selects_security(quality_traits: &Value) -> bool {
    matches!(
        quality_traits
            .get("authRequirement")
            .and_then(Value::as_str),
        Some("required" | "optional" | "deferred_with_risk")
    )
}

fn interface_auth_policy_applies(interface: &Value) -> bool {
    let Some(policy) = interface.get("authPolicy") else {
        return false;
    };
    match policy {
        Value::Null => false,
        Value::Bool(required) => *required,
        Value::String(requirement) => !matches!(
            requirement.as_str(),
            "" | "none" | "not_applicable" | "not_required"
        ),
        Value::Object(policy) => match policy.get("required") {
            Some(Value::Bool(required)) => *required,
            Some(Value::String(requirement)) => !matches!(
                requirement.as_str(),
                "" | "none" | "not_applicable" | "not_required"
            ),
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_http_interaction_selects_api_references_without_api_prose() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-reporting",
                    "interactionType": "http_api",
                    "qualityTraits": {
                        "authRequirement": "not_applicable",
                        "paginationRequired": true,
                        "contractArtifactRequired": false,
                        "compatibilityRequired": false,
                        "operationalPolicies": []
                    }
                }]
            }
        });
        let seed = build_api_quality_seed_from_foundation(&foundation, None);
        assert_eq!(seed["required"], json!(true));
        assert_eq!(
            seed["techReferenceProfile"]["groups"]["api"],
            json!(["core", "resource", "errors", "pagination"])
        );
    }

    #[test]
    fn structured_quality_traits_select_every_conditional_api_reference() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-public-orders",
                    "interactionType": "http_api",
                    "qualityTraits": {
                        "authRequirement": "required",
                        "paginationRequired": true,
                        "contractArtifactRequired": true,
                        "compatibilityRequired": true,
                        "operationalPolicies": ["idempotency", "rate_limit", "request_id"]
                    }
                }]
            }
        });
        let seed = build_api_quality_seed_from_foundation(&foundation, None);
        assert_eq!(
            seed["techReferenceProfile"]["groups"]["api"],
            json!([
                "core",
                "resource",
                "errors",
                "security",
                "pagination",
                "contract",
                "evolution",
                "operations"
            ])
        );
        let paths = seed["techReferenceProfile"]["referenceLoadPlan"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item.get("path").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), 8);
    }

    #[test]
    fn unauthenticated_http_interaction_does_not_select_security_reference() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-public-health",
                    "interactionType": "http_api",
                    "qualityTraits": {
                        "authRequirement": "not_applicable",
                        "paginationRequired": false,
                        "contractArtifactRequired": false,
                        "compatibilityRequired": false,
                        "operationalPolicies": []
                    }
                }]
            }
        });
        let seed = build_api_quality_seed_from_foundation(&foundation, None);
        assert!(!seed["techReferenceProfile"]["groups"]["api"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group == "security"));
    }

    #[test]
    fn non_http_interactions_do_not_select_api_references() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-domain-service",
                    "interactionType": "service_method"
                }]
            }
        });
        assert!(build_api_quality_seed_from_foundation(&foundation, None).is_null());
    }

    #[test]
    fn top_level_application_interactions_do_not_activate_api_references() {
        let foundation = json!({
            "applicationInteractions": [{
                "interactionId": "interaction-legacy-location",
                "interactionType": "http_api"
            }]
        });
        assert!(build_api_quality_seed_from_foundation(&foundation, None).is_null());
    }

    #[test]
    fn existing_http_contract_ref_selects_api_references() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-existing",
                    "interactionType": "service_method",
                    "interfaceRefs": ["interface-ticket-list"]
                }]
            }
        });
        let contract = json!({
            "interfaces": [{
                "interfaceId": "interface-ticket-list",
                "type": "http_api"
            }]
        });
        assert!(!build_api_quality_seed_from_foundation(&foundation, Some(&contract)).is_null());
    }

    #[test]
    fn existing_secured_http_contract_ref_selects_security_reference() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-existing-secured",
                    "interactionType": "service_method",
                    "interfaceRefs": ["interface-admin-list"]
                }]
            }
        });
        let contract = json!({
            "interfaces": [{
                "interfaceId": "interface-admin-list",
                "type": "http_api",
                "authPolicy": {
                    "required": "required",
                    "actorRefs": ["actor-admin"]
                }
            }]
        });
        let seed = build_api_quality_seed_from_foundation(&foundation, Some(&contract));
        assert!(seed["techReferenceProfile"]["groups"]["api"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group == "security"));
    }

    #[test]
    fn existing_boolean_auth_policy_selects_security_reference() {
        let foundation = json!({
            "engineeringBoundary": {
                "applicationInteractions": [{
                    "interactionId": "interaction-existing-secured",
                    "interactionType": "service_method",
                    "interfaceRefs": ["interface-admin-list"]
                }]
            }
        });
        let contract = json!({
            "interfaces": [{
                "interfaceId": "interface-admin-list",
                "type": "http_api",
                "authPolicy": {"required": true}
            }]
        });
        let seed = build_api_quality_seed_from_foundation(&foundation, Some(&contract));
        assert!(seed["techReferenceProfile"]["groups"]["api"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group == "security"));
    }
}
