use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResultSource {
    pub request_id: String,
    pub phase_id: String,
    pub task_plan_id: String,
    pub task_plan_run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReadRef {
    pub r#type: String,
    pub r#ref: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvidenceRef {
    pub r#type: String,
    pub r#ref: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFinding {
    pub finding_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finding_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_ref: Option<String>,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    pub category: String,
    pub summary: String,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_refs: Vec<ReviewReadRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<ReviewEvidenceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub artifact_refs: Value,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub location: Value,
    pub task_relevance: String,
    pub scope_relation: String,
    pub introduced_by_current_task: String,
    pub recommended_next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAcceptanceAssessment {
    pub acceptance_ref: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_task_results: Vec<String>,
    pub evidence_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCoverageSummary {
    pub total_must: u32,
    pub satisfied: u32,
    pub insufficient_evidence: u32,
    pub not_satisfied: u32,
    pub not_reviewed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCoverageAssessment {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_acceptance: Vec<ReviewAcceptanceAssessment>,
    pub summary: ReviewCoverageSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLimitation {
    pub code: String,
    pub summary: String,
    pub impact: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPendingAction {
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_refs: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewNextAction {
    pub r#type: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_phase_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_visible_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub schema_version: String,
    pub review_id: String,
    pub source: ReviewResultSource,
    pub decision: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<ReviewFinding>,
    pub coverage_assessment: ReviewCoverageAssessment,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<ReviewLimitation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_actions: Vec<ReviewPendingAction>,
    pub next_action: ReviewNextAction,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewUserAnswer {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_short_reply: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewChangeRequest {
    pub summary: String,
    pub route: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExternalEvidence {
    pub check_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    pub observed_outcome: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrowserQualityManualResolution {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_evidence: Vec<BrowserExternalEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiver_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewResolution {
    pub schema_version: String,
    pub manual_review_resolution_id: String,
    pub manual_review_request_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub user_answer: ManualReviewUserAnswer,
    pub decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_request: Option<ManualReviewChangeRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_quality_resolution: Option<BrowserQualityManualResolution>,
    pub next_action: ReviewNextAction,
    pub created_at: String,
}
