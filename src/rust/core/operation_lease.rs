use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    TechnicalBaselineGeneration,
    RepositoryContextGeneration,
    ArchitectureGeneration,
    TaskplanGeneration,
    TaskExecution,
    ExecutionRepair,
    TaskResultRepair,
    TaskplanRepair,
    ArchitectureArtifactRepair,
    ReviewGeneration,
    DeployOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationLeaseStatus {
    Active,
    Closed,
    StaleRecovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationLease {
    pub schema_version: String,
    pub operation_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub operation_type: OperationType,
    pub status: OperationLeaseStatus,
    pub started_at: String,
    pub heartbeat_at: String,
    pub expires_at: String,
    pub refs: Value,
}

impl OperationType {
    pub fn ttl_seconds(self) -> u64 {
        match self {
            Self::TechnicalBaselineGeneration => 900,
            Self::RepositoryContextGeneration => 900,
            Self::ArchitectureGeneration => 1200,
            Self::TaskplanGeneration => 900,
            Self::TaskExecution => 1800,
            Self::ExecutionRepair => 1800,
            Self::TaskResultRepair => 600,
            Self::TaskplanRepair => 900,
            Self::ArchitectureArtifactRepair => 1200,
            Self::ReviewGeneration => 900,
            Self::DeployOperation => 1800,
        }
    }
}

impl OperationLease {
    pub fn is_fresh_at(&self, now_millis: u128) -> bool {
        self.status == OperationLeaseStatus::Active
            && self
                .expires_at
                .parse::<u128>()
                .map(|expires_at| expires_at > now_millis)
                .unwrap_or(false)
    }

    pub fn mark_stale_recovered(&mut self, now: impl Into<String>) {
        let now = now.into();
        self.status = OperationLeaseStatus::StaleRecovered;
        self.heartbeat_at = now;
    }

    pub fn close(&mut self, now: impl Into<String>) {
        let now = now.into();
        self.status = OperationLeaseStatus::Closed;
        self.heartbeat_at = now;
        self.expires_at = "0".to_string();
    }
}
