use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::models::{KnowledgeSource, PendingQueue, SkippedFile};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeAddInput {
    pub project_root: String,
    pub name: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeUpdateInput {
    pub project_root: String,
    pub name: String,
    #[serde(default)]
    pub add_paths: Vec<String>,
    #[serde(default)]
    pub remove_paths: Vec<String>,
    #[serde(default)]
    pub replace_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeNameInput {
    pub project_root: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeProjectInput {
    pub project_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgePendingInput {
    pub project_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeInspectChunkInput {
    pub project_root: String,
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub build_id: String,
    pub chunk_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSearchInput {
    pub project_root: String,
    pub natural_language_query: String,
    #[serde(default)]
    pub semantic_focus: Vec<String>,
    #[serde(default)]
    pub source_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBrainstormContextInput {
    pub project_root: String,
    pub request_ref: String,
    pub block: String,
    pub step_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atomic_scope_reason: Option<String>,
    pub query_subject: String,
    pub natural_language_query: String,
    #[serde(default)]
    pub semantic_focus: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSemanticSubmitInput {
    pub project_root: String,
    pub request_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSummary {
    pub source: KnowledgeSource,
    pub pending: Option<PendingQueue>,
    pub time_zone: String,
    pub created_at_local: String,
    pub updated_at_local: String,
    pub last_built_at_local: Option<String>,
    pub warnings: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeList {
    pub sources: Vec<KnowledgeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDiscardSummary {
    pub name: String,
    pub discarded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeRemoveSummary {
    pub name: String,
    pub removed_source: bool,
    pub removed_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeStatusSummary {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<KnowledgeSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingQueue>,
    pub time_zone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_local: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_local: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_built_at_local: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunkCard {
    #[serde(default, skip_serializing)]
    pub source_id: String,
    pub source_name: String,
    pub build_id: String,
    pub chunk_id: String,
    pub document_title: String,
    pub heading_path: Vec<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub matched_labels: Vec<KnowledgeMatchedLabel>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeMatchedLabel {
    pub kind: String,
    pub text: String,
    pub match_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSearchResult {
    pub status: String,
    pub cards: Vec<KnowledgeChunkCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeInspectChunkResult {
    pub document_title: String,
    pub heading_path: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeMatchedSource {
    pub source_id: String,
    pub source_name: String,
    pub build_id: String,
    pub score: f64,
    pub best_chunk_score: f64,
    pub average_top3_chunk_score: f64,
    pub matched_focus_coverage: f64,
    pub top_chunks: Vec<KnowledgeChunkCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeContextMatchedSource {
    pub source_name: String,
    pub score: f64,
    pub matched_focus_coverage: f64,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeReadPlanChunk {
    pub source_name: String,
    pub build_id: String,
    pub chunk_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeReadPlan {
    pub mode: String,
    pub chunks: Vec<KnowledgeReadPlanChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBrainstormContextResult {
    pub status: String,
    pub matched_sources: Vec<KnowledgeContextMatchedSource>,
    pub read_plan: KnowledgeReadPlan,
}
