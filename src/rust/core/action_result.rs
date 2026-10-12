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
    pub fn with_warnings(self, warnings: Vec<String>) -> Self {
        if warnings.is_empty() {
            return self;
        }
        match self {
            Self::AutoRunnable(result) => Self::AutoRunnable(result.with_warnings(warnings)),
            Self::UserGate(result) => Self::UserGate(result.with_warnings(warnings)),
            Self::Done(mut result) => {
                result.warnings.extend(warnings);
                Self::Done(result)
            }
            other => other,
        }
    }

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
    pub agent_instruction: String,
    pub next: LoomMcpNextAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl LoomMcpAutoRunnableResult {
    pub fn new(project_root: impl Into<String>, next: LoomMcpNextAction) -> Self {
        Self {
            project_root: project_root.into(),
            stop_allowed: false,
            agent_instruction: auto_runnable_agent_instruction(&next).to_string(),
            next,
            warnings: vec![],
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }
}

fn auto_runnable_agent_instruction(next: &LoomMcpNextAction) -> &'static str {
    match next {
        LoomMcpNextAction::WriteArtifact(_) => {
            concat!(
                "This is a required continuation checkpoint. Continue immediately: inspect requestRef, read required groups, write the returned artifact targets, and submit with submitTool. On local read/edit/command failure, retry the smallest corrective step. If an outer exec wrapper reports success or failure, parse the nested Loom result. Do not stop at a progress recap. Do not mark the workflow complete, send a final answer, or ask the user whether to continue; follow the nested Loom state through user_gate, done, blocked, or failed."
            )
        }
        LoomMcpNextAction::ExecuteTask(_) => {
            concat!(
                "This is a required continuation checkpoint. Continue immediately: inspect next.requestRef, read required groups, execute only this task, write resultFile, and submit with submitTool; TaskResult submit succeeds only after validation. On local read/edit/test/command failure, retry the smallest corrective step. If an outer exec wrapper reports success or failure, parse the nested Loom result. Do not stop at a progress recap. Do not mark the workflow complete, send a final answer, or ask the user whether to continue; follow the nested Loom state through user_gate, done, blocked, or failed."
            )
        }
        LoomMcpNextAction::RunVsefmVerification(_) => concat!(
            "Required V-SEFM verification: inspect requestRef, read every verification group and declared reference, run bounded read-only checks, write the result, and submit with loom.vsefmVerificationAcceptFile. Do not modify product or Loom canonical files. Record unknown when evidence is insufficient; continue until submit succeeds or Loom is terminal."
        ),
        LoomMcpNextAction::RunVsefmRepair(_) => concat!(
            "Required V-SEFM repair: inspect requestRef, read every repair group, modify only ordinary project files needed for the findings, run bounded verification, write the result, and submit with loom.vsefmRepairAcceptFile. Do not modify protected Loom or V-SEFM files; continue until submit succeeds or Loom is terminal."
        ),
        LoomMcpNextAction::RunLoomTool(_) => {
            concat!(
                "This is a required continuation checkpoint. Continue immediately: inspect requestRef, read required groups, call the returned Loom tool, then retry retryTool. On local read/command failure, retry the smallest corrective step. If an outer exec wrapper reports success or failure, parse the nested Loom result. Do not stop at a progress recap. Do not mark the workflow complete, send a final answer, or ask the user whether to continue; follow the nested Loom state through user_gate, done, blocked, or failed."
            )
        }
        LoomMcpNextAction::GenerateKnowledgeSemantics(_) => {
            concat!(
                "Required checkpoint: read the semantic request, fill and submit the result template, and follow semantic next actions until published, blocked, failed, or user-gated. ",
                "On failure, make the smallest corrective edit and retry. Do not stop while a semantic next action remains auto-runnable."
            )
        }
        LoomMcpNextAction::DeployRepairAssets(_) => {
            concat!(
                "Required checkpoint. Continue immediately: edit only the returned deployment asset targets and retry the returned deploy tool. On local read/edit/command failure, retry the smallest corrective step. If an outer exec wrapper reports success or failure, parse the nested Loom result. Do not stop at a progress recap. Do not mark the workflow complete, send a final answer, or ask the user whether to continue; follow the nested Loom state through user_gate, done, blocked, or failed."
            )
        }
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
    pub agent_instruction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_response_contract: Option<LoomMcpUserGatePreResponseContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl LoomMcpUserGateResult {
    pub fn new(
        project_root: impl Into<String>,
        prompt: impl Into<String>,
        accepted_responses: Vec<String>,
        request_ref: Option<String>,
        delivery_id: Option<String>,
        phase_id: Option<String>,
        gate: Option<Value>,
    ) -> Self {
        let pre_response_contract = request_ref
            .as_deref()
            .map(LoomMcpUserGatePreResponseContract::request_scoped);
        let agent_instruction = if pre_response_contract.is_some() {
            "Before replying, execute preResponseContract in order. For requestRef, inspect the request and read only groups applicable before the visible response; read post-confirmation groups before submitting. Complete required Brainstorm knowledge steps, then present the prompt in the user's language and wait. Do not answer from prompt alone, call loom.continue, or expose internal ids.".to_string()
        } else {
            "This is a user gate. Present the returned prompt in the user's language and wait for the user's response. Do not invent a continuation action or report the workflow as complete.".to_string()
        };
        Self {
            project_root: project_root.into(),
            prompt: prompt.into(),
            accepted_responses,
            request_ref,
            delivery_id,
            phase_id,
            gate,
            agent_instruction,
            pre_response_contract,
            warnings: vec![],
        }
    }

    pub fn with_agent_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.agent_instruction = instruction.into();
        self
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn with_brainstorm_knowledge(mut self, block: impl Into<String>) -> Self {
        let block = block.into();
        if block != "final_summary" {
            if let Some(contract) = self.pre_response_contract.as_mut() {
                contract.steps.insert(
                    contract.steps.len().saturating_sub(1),
                    LoomMcpUserGatePreResponseStep::RunKnowledgeContextPlan {
                        tool_name: "loom.knowledgeBrainstormContext".to_string(),
                        request_ref: self.request_ref.clone().unwrap_or_default(),
                        group_id: "knowledge_context_plan".to_string(),
                        block,
                    },
                );
            }
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpUserGatePreResponseContract {
    pub required: bool,
    pub steps: Vec<LoomMcpUserGatePreResponseStep>,
    pub completion_rule: String,
}

impl LoomMcpUserGatePreResponseContract {
    fn request_scoped(request_ref: &str) -> Self {
        Self {
            required: true,
            steps: vec![
                LoomMcpUserGatePreResponseStep::InspectRequest {
                    tool_name: "loom.inspectRequest".to_string(),
                    request_ref: request_ref.to_string(),
                },
                LoomMcpUserGatePreResponseStep::ReadRequiredRequestGroups {
                    tool_name: "loom.readFieldGroup".to_string(),
                    request_ref: request_ref.to_string(),
                    source: "requestReadPlan.groups".to_string(),
                    timing: "before_visible_response".to_string(),
                },
                LoomMcpUserGatePreResponseStep::PresentGate,
            ],
            completion_rule: "Do not present the visible gate response until every preceding step has completed successfully. Read only required groups whose whenToRead applies before the visible response; read post-confirmation groups immediately before submitting the confirmation.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoomMcpUserGatePreResponseStep {
    InspectRequest {
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "requestRef")]
        request_ref: String,
    },
    ReadRequiredRequestGroups {
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "requestRef")]
        request_ref: String,
        source: String,
        timing: String,
    },
    RunKnowledgeContextPlan {
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "requestRef")]
        request_ref: String,
        #[serde(rename = "groupId")]
        group_id: String,
        block: String,
    },
    PresentGate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomMcpActiveOperationResult {
    pub project_root: String,
    pub operation: ActiveOperationRef,
    pub allowed_observation_tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_policy: Option<ActiveOperationObservationPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_summary: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveOperationObservationPolicy {
    pub quiet_mode: bool,
    pub initial_quiet_window_ms: u32,
    pub min_next_observation_interval_ms: u32,
    pub logs_policy: String,
    pub user_visible_update_policy: String,
    pub final_response_policy: String,
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
    pub stop_allowed: bool,
    pub target_file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_ids: Vec<String>,
    pub issues: Vec<RepairIssue>,
    pub resubmit_tool: String,
    pub fix_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_groups: Vec<crate::ReadGroupRef>,
    pub agent_instruction: String,
}

pub fn repairable_error_agent_instruction(resubmit_tool: &str) -> String {
    format!(
        "Required repair: inspect the returned requestRef, read every required group, repair only the returned targets, and call {resubmit_tool}. Use the current contract fingerprint; preserve valid business content, references, evidence, and array entries. Remove fields only when explicitly not_applicable. On failure, retry the smallest corrective step; do not produce a progress summary or final answer. Continue until the repair submit succeeds or Loom is terminal."
    )
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
