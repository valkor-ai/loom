use delivery_core::{
    apply_machine_owned_fields, strip_machine_owned_fields, validate_typed, ContractProjection,
    RepairIssue, SubmitValidationContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

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
