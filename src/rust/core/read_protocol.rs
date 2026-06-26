use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ReadGroupRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectRequestInput {
    pub project_root: String,
    pub request_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectRequestResult {
    pub request_ref: String,
    pub request_id: String,
    pub project_id: String,
    pub request_kind: String,
    pub read_groups: Vec<ReadGroupRef>,
    pub write_targets: Vec<Value>,
    pub submit_tool: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFieldGroupInput {
    pub project_root: String,
    pub request_ref: String,
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFieldGroupResult {
    pub request_ref: String,
    pub request_id: String,
    pub group_id: String,
    pub required: bool,
    pub order: u32,
    pub fields: BTreeMap<String, FieldReadResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadRequestFieldsInput {
    pub project_root: String,
    pub request_ref: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadRequestFieldsResult {
    pub request_ref: String,
    pub request_id: String,
    pub fields: BTreeMap<String, FieldReadResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct FieldReadResult {
    pub value: Value,
}
