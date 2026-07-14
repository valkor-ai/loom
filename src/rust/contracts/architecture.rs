use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AcceptancePriority;

pub const COVERAGE_ARTIFACT_TYPES: [&str; 14] = [
    "module",
    "data_entity",
    "data_constraint",
    "relationship",
    "interface",
    "user_flow",
    "state_machine",
    "state_rule",
    "frontend_data_view",
    "frontend_action",
    "frontend_operation_path",
    "decision",
    "nfr",
    "risk",
];

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureSectionGroup {
    Foundation,
    DomainContract,
    Behavior,
    FrontendExperience,
    RuntimeDelivery,
    Coverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureSectionStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureBlockedReason {
    pub code: String,
    pub message: String,
    pub next_node: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureSectionCandidateAgentWritable {
    #[serde(default)]
    #[schemars(skip)]
    pub schema_version: String,
    #[serde(default)]
    #[schemars(skip)]
    pub request_id: String,
    #[serde(default)]
    #[schemars(skip)]
    pub delivery_id: String,
    #[serde(default)]
    #[schemars(skip)]
    pub phase_id: String,
    #[schemars(skip)]
    pub section: ArchitectureSectionGroup,
    pub status: ArchitectureSectionStatus,
    pub content: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<ArchitectureBlockedReason>,
    #[serde(default)]
    #[schemars(skip)]
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureArtifactStatus {
    Ready,
    Blocked,
    NeedsUserDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureArtifactSource {
    pub planning_generation_contract_id: String,
    pub technical_baseline_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brainstorm_contract_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_context_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Covered,
    Partial,
    NotApplicable,
    Deferred,
    Uncovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCoverageArtifact {
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationHint {
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceMatrixEntry {
    pub acceptance_id: String,
    pub priority: AcceptancePriority,
    pub statement: String,
    pub coverage_status: CoverageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage: Vec<AcceptanceCoverageArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_hints: Vec<VerificationHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetailCoverageArtifactRefs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_flows: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_machines: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontend_data_views: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontend_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontend_operation_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_matrix: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureDetailCoverageEntry {
    pub detail_id: String,
    pub coverage_status: CoverageStatus,
    pub artifact_refs: DetailCoverageArtifactRefs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureHandoff {
    pub ready_for_task_plan: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_reasons: Vec<String>,
    pub next_node: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureDecisionAlternative {
    pub name: String,
    pub tradeoff: String,
    pub rejected_because: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureDecisionConsequences {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positive: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub negative: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neutral: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureQualitySourceRefs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_detail_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureDecision {
    pub decision_id: String,
    pub category: String,
    pub title: String,
    pub status: String,
    pub context: String,
    pub decision: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives_considered: Vec<ArchitectureDecisionAlternative>,
    pub consequences: ArchitectureDecisionConsequences,
    pub source_refs: ArchitectureQualitySourceRefs,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureNfrRefs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureNfr {
    pub nfr_id: String,
    pub category: String,
    pub target: String,
    pub rationale: String,
    pub architecture_refs: ArchitectureNfrRefs,
    pub verification_strategy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRiskOwnerRefs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nfrs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRisk {
    pub risk_id: String,
    pub category: String,
    pub severity: String,
    pub likelihood: String,
    pub impact: String,
    pub mitigation: String,
    pub owner_artifact_refs: ArchitectureRiskOwnerRefs,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureQuality {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<ArchitectureDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nfrs: Vec<ArchitectureNfr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<ArchitectureRisk>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureArtifactContract {
    pub schema_version: String,
    pub architecture_artifact_contract_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub status: ArchitectureArtifactStatus,
    pub source: ArchitectureArtifactSource,
    pub engineering_boundary: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<Value>,
    pub data_model: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_contract: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_flows: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_machines: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_delivery: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_matrix: Vec<AcceptanceMatrixEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detail_coverage: Vec<ArchitectureDetailCoverageEntry>,
    #[serde(default)]
    pub architecture_quality: ArchitectureQuality,
    pub handoff: ArchitectureHandoff,
    pub created_at: String,
    pub updated_at: String,
}
