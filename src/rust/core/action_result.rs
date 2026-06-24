use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{LoomError, LoomMcpNextAction};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LoomMcpActionResult {
    AutoRunnable(LoomMcpAutoRunnableResult),
    UserGate(LoomMcpUserGateResult),
    ActiveOperation(LoomMcpActiveOperationResult),
    Done(LoomMcpDoneResult),
    Blocked(LoomMcpBlockedResult),
    RepairableError(LoomMcpRepairableErrorResult),
    Failed(LoomMcpFailureResult),
}

impl LoomMcpActionResult {
    pub fn not_implemented_for_batch(
        project_root: impl Into<String>,
        tool_name: impl Into<String>,
        target_batch: u32,
    ) -> Self {
        let tool_name = tool_name.into();
        Self::Failed(LoomMcpFailureResult {
            project_root: project_root.into(),
            error: LoomMcpFailure {
                code: "not_implemented_for_batch".to_string(),
                message: format!("{tool_name} is registered but not implemented in this batch."),
                target_batch: Some(target_batch),
                domain: None,
                route_action: None,
                recovery_tool: None,
            },
        })
    }

    pub fn invalid_project_root(message: impl Into<String>) -> Self {
        Self::Failed(LoomMcpFailureResult {
            project_root: String::new(),
            error: LoomMcpFailure {
                code: "invalid_project_root".to_string(),
                message: message.into(),
                target_batch: None,
                domain: None,
                route_action: None,
                recovery_tool: None,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpAutoRunnableResult {
    pub project_root: String,
    pub stop_allowed: bool,
    pub next: LoomMcpNextAction,
}

impl LoomMcpAutoRunnableResult {
    pub fn new(project_root: impl Into<String>, next: LoomMcpNextAction) -> Self {
        Self {
            project_root: project_root.into(),
            stop_allowed: false,
            next,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpUserGateResult {
    pub project_root: String,
    pub prompt: String,
    pub accepted_responses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpActiveOperationResult {
    pub project_root: String,
    pub operation: ActiveOperationRef,
    pub allowed_observation_tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_summary: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveOperationRef {
    pub operation_id: String,
    pub operation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_id: Option<String>,
    pub started_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpDoneResult {
    pub project_root: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpBlockedResult {
    pub project_root: String,
    pub blockers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpRepairableErrorResult {
    pub project_root: String,
    pub target_file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_ids: Vec<String>,
    pub issues: Vec<RepairIssue>,
    pub resubmit_tool: String,
    pub fix_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_groups: Vec<crate::ReadGroupRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepairIssue {
    pub code: String,
    pub message: String,
    pub target_id: Option<String>,
    pub field_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpFailureResult {
    pub project_root: String,
    pub error: LoomMcpFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpFailure {
    pub code: String,
    pub message: String,
    pub target_batch: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_tool: Option<String>,
}

impl From<LoomError> for LoomMcpFailure {
    fn from(error: LoomError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            target_batch: None,
            domain: None,
            route_action: None,
            recovery_tool: None,
        }
    }
}
