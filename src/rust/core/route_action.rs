use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RouteActionKind {
    BrainstormStart,
    BrainstormClarification,
    BrainstormConfirmation,
    TechnicalBaselineRequest,
    RepositoryContextRequest,
    PlanningContractCreate,
    ArchitectureArtifactContract,
    TaskplanGeneration,
    ContinueExecution,
    Review,
    ExecutionRepair,
    TaskResultRepair,
    TaskplanRepair,
    ArchitectureArtifactRepair,
    NeedsUserDecision,
    ManualReview,
    VsefmOnboarding,
    VsefmVerification,
    VsefmResultGate,
    VsefmRepair,
    ContinueToNextPhase,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteAction {
    pub kind: RouteActionKind,
    pub source: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_responses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_phase_id: Option<String>,
}

impl RouteAction {
    pub fn done(reason: impl Into<String>) -> Self {
        Self {
            kind: RouteActionKind::Done,
            source: "transition_engine".to_string(),
            reason: reason.into(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        }
    }
}

impl RouteActionKind {
    pub fn target_batch(self) -> Option<u32> {
        match self {
            Self::BrainstormStart
            | Self::BrainstormClarification
            | Self::BrainstormConfirmation => Some(7),
            Self::TechnicalBaselineRequest
            | Self::RepositoryContextRequest
            | Self::PlanningContractCreate
            | Self::ArchitectureArtifactContract
            | Self::TaskplanGeneration
            | Self::ContinueExecution => Some(8),
            Self::Review | Self::ExecutionRepair | Self::NeedsUserDecision | Self::ManualReview => {
                Some(9)
            }
            Self::VsefmOnboarding
            | Self::VsefmVerification
            | Self::VsefmResultGate
            | Self::VsefmRepair => Some(10),
            Self::TaskResultRepair | Self::TaskplanRepair | Self::ArchitectureArtifactRepair => {
                Some(5)
            }
            Self::ContinueToNextPhase | Self::Done => None,
        }
    }

    pub fn domain(self) -> Option<&'static str> {
        match self {
            Self::BrainstormStart
            | Self::BrainstormClarification
            | Self::BrainstormConfirmation => Some("brainstorm"),
            Self::TechnicalBaselineRequest
            | Self::RepositoryContextRequest
            | Self::PlanningContractCreate
            | Self::ArchitectureArtifactContract
            | Self::TaskplanGeneration => Some("planning"),
            Self::ContinueExecution
            | Self::Review
            | Self::ExecutionRepair
            | Self::NeedsUserDecision
            | Self::ManualReview => Some("execution"),
            Self::VsefmOnboarding
            | Self::VsefmVerification
            | Self::VsefmResultGate
            | Self::VsefmRepair => Some("verification"),
            Self::TaskResultRepair | Self::TaskplanRepair | Self::ArchitectureArtifactRepair => {
                Some("repair")
            }
            Self::ContinueToNextPhase | Self::Done => None,
        }
    }

    pub fn is_user_gate(self) -> bool {
        matches!(
            self,
            Self::BrainstormStart
                | Self::BrainstormClarification
                | Self::NeedsUserDecision
                | Self::ManualReview
                | Self::VsefmOnboarding
                | Self::VsefmResultGate
        )
    }
}
