use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitionDiagnostic {
    pub decision_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub source: String,
    pub route_action: String,
    pub result_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_batch: Option<u32>,
    pub created_at: String,
}
