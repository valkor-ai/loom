use schemars::{schema_for, JsonSchema};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::{RepairIssue, SubmitValidationContext};

pub trait ContractProjection {
    type AgentWritable: DeserializeOwned + JsonSchema;
    type Persisted: Serialize;

    fn schema_shape() -> Value {
        serde_json::to_value(schema_for!(Self::AgentWritable))
            .unwrap_or_else(|_| serde_json::json!({ "type": "object" }))
    }

    fn canonicalize(
        input: Self::AgentWritable,
        ctx: &SubmitValidationContext,
    ) -> Result<Self::Persisted, Vec<RepairIssue>>;
}

pub fn validate_typed<T>(value: Value) -> Result<T, Vec<RepairIssue>>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).map_err(|error| {
        vec![RepairIssue {
            code: "INVALID_SCHEMA".to_string(),
            message: error.to_string(),
            target_id: None,
            field_path: None,
        }]
    })
}
