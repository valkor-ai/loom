use delivery_core::{
    apply_machine_owned_fields, compact_agent_field_contract, derive_agent_field_policies,
    finalize_output_contract, strip_machine_owned_fields, validate_typed, AgentFieldApplicability,
    ContractProjection, RepairIssue, SubmitValidationContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn contract_projection_uses_agent_writable_type_schema() {
    let schema = ExampleContract::schema_shape();
    let schema_text = schema.to_string();
    assert!(schema_text.contains("summary"));
    assert!(schema_text.contains("confirmed"));
    assert!(!schema_text.contains("updatedAt"));
}

#[test]
fn machine_owned_fields_are_removed_from_agent_input_and_reapplied_by_core() {
    let mut value = json!({
        "summary": "ok",
        "updatedAt": "agent_time",
        "requestRef": "agent_ref"
    });
    strip_machine_owned_fields(&mut value);
    assert_eq!(value, json!({ "summary": "ok" }));

    apply_machine_owned_fields(
        &mut value,
        &SubmitValidationContext {
            request_ref: "loom://projects/p/requests/r".to_string(),
            request_id: "r".to_string(),
            delivery_id: Some("d".to_string()),
            phase_id: Some("p1".to_string()),
            now: "100".to_string(),
        },
    );
    assert_eq!(value["requestId"], "r");
    assert_eq!(value["requestRef"], "loom://projects/p/requests/r");
    assert_eq!(value["updatedAt"], "100");
}

#[test]
fn typed_validation_returns_repair_issue() {
    let issues = validate_typed::<ExampleAgentWritable>(json!({ "confirmed": true }))
        .expect_err("summary is required");
    assert_eq!(issues[0].code, "INVALID_SCHEMA");
}

#[test]
fn shared_field_contract_exposes_nested_shape_without_flat_paths() {
    let schema =
        serde_json::to_value(schemars::schema_for!(NestedAgentWritable)).expect("nested schema");
    let contract = compact_agent_field_contract(&schema, &BTreeMap::new());
    assert_eq!(contract["properties"]["approval"]["type"], "object");
    assert_eq!(
        contract["properties"]["approval"]["properties"]["type"]["type"],
        "string"
    );
    assert_eq!(contract["defaults"]["preserveOnRepair"], true);
    assert!(contract.to_string().contains("properties"));
    assert!(!contract.to_string().contains("approval.type"));
}

#[test]
fn shared_field_contract_handles_architecture_pseudo_schema_shape() {
    let schema = json!({
        "status": "ready | blocked",
        "content": {
            "dataModel": {
                "entities": [{"entityId": "string", "fields": ["string"]}]
            }
        }
    });
    let contract = compact_agent_field_contract(&schema, &BTreeMap::new());
    assert_eq!(contract["properties"]["status"]["type"], "string");
    assert_eq!(
        contract["properties"]["status"]["enum"],
        json!(["ready", "blocked"])
    );
    assert_eq!(
        contract["properties"]["content"]["properties"]["dataModel"]["properties"]["entities"]
            ["type"],
        "array"
    );
    assert_eq!(
        contract["properties"]["content"]["properties"]["dataModel"]["properties"]["entities"]
            ["items"]["type"],
        "object"
    );
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NestedAgentWritable {
    summary: String,
    approval: NestedApproval,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NestedApproval {
    r#type: String,
}

#[test]
fn contract_fingerprint_ignores_target_paths_but_changes_contract_shape() {
    let mut first = json!({
        "schemaShape": {"type": "object", "properties": {"summary": {"type": "string"}}},
        "schemaProjection": {"requiredTopLevelFields": ["summary"]},
        "writeTargets": [{"path": ".loom/agent-writable/one.json"}]
    });
    let mut second = first.clone();
    second["writeTargets"][0]["path"] = json!(".loom/agent-writable/two.json");
    finalize_output_contract(&mut first, &BTreeMap::new());
    finalize_output_contract(&mut second, &BTreeMap::new());
    assert_eq!(first["contractFingerprint"], second["contractFingerprint"]);
    first["schemaProjection"]["requiredTopLevelFields"] = json!(["summary", "confirmed"]);
    finalize_output_contract(&mut first, &BTreeMap::new());
    assert_ne!(first["contractFingerprint"], second["contractFingerprint"]);
}

#[test]
fn finalize_output_contract_adds_version_fingerprint_and_compact_projection() {
    let mut output = json!({
        "schemaShape": {
            "type": "object",
            "properties": {"summary": {"type": "string"}},
            "required": ["summary"]
        },
        "schemaProjection": {"requiredTopLevelFields": ["summary"]},
        "writeTargets": [{"path": ".loom/agent-writable/candidate.json"}]
    });
    finalize_output_contract(&mut output, &BTreeMap::new());
    assert_eq!(output["contractVersion"], "1.0");
    assert!(output["contractFingerprint"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        output["schemaProjection"]["fieldContract"]["properties"]["summary"]["type"],
        "string"
    );
}

#[test]
fn applicability_uses_structured_contract_facts_without_api_keyword_matching() {
    let policies = derive_agent_field_policies(&json!({
        "apiQualitySeed": null,
        "contextProjection": {
            "technicalBaseline": {
                "stack": {
                    "tracks": {
                        "persistence": {"status": "not_needed", "selection": "none"}
                    }
                }
            }
        },
        "task": {
            "frontendExperienceRequirement": {
                "executionGuidance": {
                    "uiProductionBrief": {
                        "surfaceDecisionContract": {"contractRef": "surface-contract"}
                    }
                }
            },
            "runtimeDeliveryRequirement": {"appliesToThisTask": false},
            "apiContractRequirements": [],
            "architectureQualityRequirements": [],
            "codeQualityRequirements": []
        }
    }));
    assert_eq!(
        policies["content.apiContract"].applicability,
        AgentFieldApplicability::NotApplicable
    );
    assert_eq!(
        policies["content.interfaces"].applicability,
        AgentFieldApplicability::CurrentRequest
    );
    assert_eq!(
        policies["content.dataModel.dataArchitecture.ownership"].applicability,
        AgentFieldApplicability::NotApplicable
    );
    assert_eq!(
        policies["frontendQualitySelfCheck"].applicability,
        AgentFieldApplicability::CurrentRequest
    );
    assert_eq!(
        policies["runtimeDeliveryEvidence"].applicability,
        AgentFieldApplicability::NotApplicable
    );
}

#[derive(Debug, Serialize)]
struct ExamplePersisted {
    summary: String,
    confirmed: bool,
    request_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ExampleAgentWritable {
    summary: String,
    confirmed: bool,
}

struct ExampleContract;

impl ContractProjection for ExampleContract {
    type AgentWritable = ExampleAgentWritable;
    type Persisted = ExamplePersisted;

    fn canonicalize(
        input: Self::AgentWritable,
        ctx: &SubmitValidationContext,
    ) -> Result<Self::Persisted, Vec<RepairIssue>> {
        Ok(ExamplePersisted {
            summary: input.summary,
            confirmed: input.confirmed,
            request_id: ctx.request_id.clone(),
        })
    }
}
