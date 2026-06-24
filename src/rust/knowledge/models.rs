use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeRegistry {
    pub schema_version: u32,
    pub sources: Vec<KnowledgeSource>,
}

impl KnowledgeRegistry {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            sources: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSource {
    pub source_id: String,
    pub name: String,
    pub enabled: bool,
    pub document_paths: Vec<String>,
    pub current_build_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_built_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingQueue {
    pub schema_version: u32,
    pub source_id: String,
    pub source_name: String,
    pub operations: Vec<PendingOperation>,
}

impl PendingQueue {
    pub fn empty(source_id: &str, source_name: &str) -> Self {
        Self {
            schema_version: 1,
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            operations: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingOperationKind {
    AddPaths,
    RemovePaths,
    ReplacePaths,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingOperation {
    pub operation_id: String,
    pub kind: PendingOperationKind,
    pub paths: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBuildSnapshot {
    pub schema_version: u32,
    pub source_id: String,
    pub source_name: String,
    pub build_id: String,
    pub documents: Vec<KnowledgeDocument>,
    pub skipped_files: Vec<SkippedFile>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDocument {
    pub document_id: String,
    pub path: String,
    pub title: String,
    pub content_type: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunksFile {
    pub schema_version: u32,
    pub source_id: String,
    pub source_name: String,
    pub build_id: String,
    pub chunks: Vec<KnowledgeChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunk {
    pub chunk_id: String,
    pub document_id: String,
    pub document_title: String,
    pub source_path: String,
    pub heading_path: Vec<String>,
    pub token_estimate: u32,
    pub context_prefix: String,
    pub neighbor_chunk_ids: Vec<String>,
    pub split_reason: String,
    pub body_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub semantic_labels: Vec<SemanticLabel>,
    #[serde(default)]
    pub semantic_aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_affinity: Option<BlockAffinity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLabel {
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockAffinity {
    pub phase_scope: f64,
    pub concept_grounding: f64,
    pub frontend_experience: f64,
    pub business_rules: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LexicalIndex {
    pub schema_version: u32,
    pub source_id: String,
    pub build_id: String,
    pub documents: Vec<LexicalDocument>,
    pub keywords: Vec<LexicalKeyword>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LexicalDocument {
    pub id: String,
    pub text: String,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LexicalKeyword {
    pub term: String,
    pub score: f64,
    pub document_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndex {
    pub schema_version: u32,
    pub source_id: String,
    pub build_id: String,
    pub chunk_features: Vec<SemanticChunkFeature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticChunkFeature {
    pub chunk_id: String,
    pub summary: String,
    pub labels: Vec<SemanticLabel>,
    pub aliases: Vec<String>,
    pub block_affinity: BlockAffinity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SemanticBuildStatus {
    MechanicalReady,
    SemanticPending,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticState {
    pub schema_version: u32,
    pub source_id: String,
    pub source_name: String,
    pub build_id: String,
    pub status: SemanticBuildStatus,
    pub pack_count: u32,
    pub packs: Vec<SemanticPackState>,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SemanticPackStatus {
    Pending,
    Accepted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPackState {
    pub pack_id: String,
    pub pack_index: u32,
    pub chunk_ids: Vec<String>,
    pub status: SemanticPackStatus,
    pub request_ref: String,
    pub result_file: String,
    pub accepted_at: Option<String>,
}
