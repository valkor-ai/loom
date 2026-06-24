use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitValidationContext {
    pub request_ref: String,
    pub request_id: String,
    pub delivery_id: Option<String>,
    pub phase_id: Option<String>,
    pub now: String,
}

pub const MACHINE_OWNED_FIELDS: &[&str] = &[
    "requestId",
    "deliveryId",
    "phaseId",
    "createdAt",
    "updatedAt",
    "status",
    "handoffStatus",
    "latestRef",
    "latestArtifactRef",
    "operationLease",
    "requestFile",
    "requestRef",
];

pub fn strip_machine_owned_fields(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for field in MACHINE_OWNED_FIELDS {
        object.remove(*field);
    }
}

pub fn apply_machine_owned_fields(value: &mut Value, ctx: &SubmitValidationContext) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert(
        "requestRef".to_string(),
        Value::String(ctx.request_ref.clone()),
    );
    object.insert(
        "requestId".to_string(),
        Value::String(ctx.request_id.clone()),
    );
    insert_optional(object, "deliveryId", ctx.delivery_id.as_ref());
    insert_optional(object, "phaseId", ctx.phase_id.as_ref());
    object.insert("updatedAt".to_string(), Value::String(ctx.now.clone()));
    if !object.contains_key("createdAt") {
        object.insert("createdAt".to_string(), Value::String(ctx.now.clone()));
    }
}

fn insert_optional(object: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        object.insert(key.to_string(), Value::String(value.clone()));
    }
}
