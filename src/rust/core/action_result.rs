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
    pub continuation_policy: LoomMcpContinuationPolicy,
    pub agent_instruction: String,
    pub next: LoomMcpNextAction,
}

impl LoomMcpAutoRunnableResult {
    pub fn new(project_root: impl Into<String>, next: LoomMcpNextAction) -> Self {
        Self {
            project_root: project_root.into(),
            stop_allowed: false,
            continuation_policy: auto_runnable_continuation_policy(&next),
            agent_instruction: auto_runnable_agent_instruction(&next).to_string(),
            next,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpContinuationPolicy {
    pub must_continue: bool,
    pub progress_report_allowed: bool,
    pub completion_barrier: String,
    pub stop_conditions: Vec<String>,
}

fn auto_runnable_agent_instruction(next: &LoomMcpNextAction) -> &'static str {
    match next {
        LoomMcpNextAction::WriteArtifact(_) => {
            "Do not report progress now. Continue immediately: inspect requestRef, read required groups, write the returned artifact target(s), and submit with submitTool before reporting progress."
        }
        LoomMcpNextAction::ExecuteTask(_) => {
            "Do not report progress now. Continue immediately: inspect requestRef, read required groups, execute only this task, write resultFile, and submit with submitTool before reporting progress."
        }
        LoomMcpNextAction::GenerateKnowledgeSemantics(_) => {
            "Do not report progress now. Continue immediately: read the semantic request, fill the returned result template, submit it, and keep following semantic next actions until published, blocked, failed, or user-gated."
        }
        LoomMcpNextAction::DeployRepairAssets(_) => {
            "Do not report progress now. Continue immediately: edit only the returned deployment asset targets and retry through the returned deploy tool before reporting progress."
        }
    }
}

fn auto_runnable_continuation_policy(next: &LoomMcpNextAction) -> LoomMcpContinuationPolicy {
    let completion_barrier = match next {
        LoomMcpNextAction::WriteArtifact(_) => {
            "The artifact step is not complete until the returned writeTargets are written, submitTool succeeds, and the returned state is followed."
        }
        LoomMcpNextAction::ExecuteTask(_) => {
            "The execution step is not complete until source work is done, resultFile is written, submitTool succeeds, and the returned state is followed."
        }
        LoomMcpNextAction::GenerateKnowledgeSemantics(_) => {
            "The semantic build step is not complete until the returned semantic result is submitted and the semantic chain reaches published, blocked, failed, or user_gate."
        }
        LoomMcpNextAction::DeployRepairAssets(_) => {
            "The deploy repair step is not complete until the returned deployment assets are edited and the returned deploy tool is retried."
        }
    };
    LoomMcpContinuationPolicy {
        must_continue: true,
        progress_report_allowed: false,
        completion_barrier: completion_barrier.to_string(),
        stop_conditions: vec![
            "user_gate".to_string(),
            "done".to_string(),
            "blocked".to_string(),
            "failed".to_string(),
            "repairable_error".to_string(),
            "active_operation".to_string(),
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpUserGateResult {
    pub project_root: String,
    pub prompt: String,
    pub accepted_responses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<Value>,
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
