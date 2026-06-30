use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeRegistry {
    #[serde(deserialize_with = "deserialize_schema_version")]
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

#[derive(Debug, Clone, Serialize, JsonSchema)]
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

impl<'de> Deserialize<'de> for KnowledgeSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireSource {
            source_id: String,
            name: String,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default)]
            status: Option<String>,
            #[serde(default)]
            document_paths: Option<Vec<String>>,
            #[serde(default)]
            roots: Vec<WireRoot>,
            #[serde(default)]
            current_build_id: Option<String>,
            #[serde(default)]
            index: Option<WireIndex>,
            created_at: String,
            updated_at: String,
            #[serde(default)]
            last_built_at: Option<String>,
        }

        #[derive(Deserialize)]
        struct WireRoot {
            path: String,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireIndex {
            #[serde(default)]
            current_build_id: Option<String>,
            #[serde(default)]
            last_built_at: Option<String>,
        }

        let wire = WireSource::deserialize(deserializer)?;
        let enabled = wire
            .enabled
            .unwrap_or_else(|| wire.status.as_deref() != Some("disabled"));
        let document_paths = wire.document_paths.unwrap_or_else(|| {
            wire.roots
                .iter()
                .map(|root| root.path.clone())
                .collect::<Vec<_>>()
        });
        let current_build_id = wire.current_build_id.or_else(|| {
            wire.index
                .as_ref()
                .and_then(|index| index.current_build_id.clone())
        });
        let last_built_at = wire.last_built_at.or_else(|| {
            wire.index
                .as_ref()
                .and_then(|index| index.last_built_at.clone())
        });
        Ok(Self {
            source_id: wire.source_id,
            name: wire.name,
            enabled,
            document_paths,
            current_build_id,
            created_at: wire.created_at,
            updated_at: wire.updated_at,
            last_built_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
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

impl<'de> Deserialize<'de> for PendingQueue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireQueue {
            #[serde(deserialize_with = "deserialize_schema_version")]
            schema_version: u32,
            #[serde(default)]
            source_id: Option<String>,
            #[serde(default)]
            source_name: Option<String>,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            operations: Vec<PendingOperation>,
        }

        let wire = WireQueue::deserialize(deserializer)?;
        Ok(Self {
            schema_version: wire.schema_version,
            source_id: wire.source_id.unwrap_or_default(),
            source_name: wire.source_name.or(wire.name).unwrap_or_default(),
            operations: wire.operations,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingOperationKind {
    AddPaths,
    RemovePaths,
    ReplacePaths,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingOperation {
    pub operation_id: String,
    pub kind: PendingOperationKind,
    pub paths: Vec<String>,
    pub created_at: String,
}

impl<'de> Deserialize<'de> for PendingOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireOperation {
            #[serde(default)]
            operation_id: Option<String>,
            #[serde(default)]
            kind: Option<PendingOperationKind>,
            #[serde(default, rename = "type")]
            operation_type: Option<PendingOperationKind>,
            #[serde(default)]
            paths: Vec<String>,
            #[serde(default)]
            created_at: Option<String>,
        }

        let wire = WireOperation::deserialize(deserializer)?;
        let kind = wire
            .kind
            .or(wire.operation_type)
            .ok_or_else(|| de::Error::custom("pending operation kind is required"))?;
        Ok(Self {
            operation_id: wire
                .operation_id
                .unwrap_or_else(|| "legacy_pending_operation".to_string()),
            kind,
            paths: wire.paths,
            created_at: wire.created_at.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBuildSnapshot {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u32,
    pub source_id: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
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
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u32,
    pub source_id: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub source_name: String,
    pub build_id: String,
    pub chunks: Vec<KnowledgeChunk>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
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

impl<'de> Deserialize<'de> for KnowledgeChunk {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireChunk {
            chunk_id: String,
            document_id: String,
            #[serde(default)]
            document_title: Option<String>,
            #[serde(default)]
            title: Option<String>,
            #[serde(default)]
            source_path: Option<String>,
            #[serde(default)]
            heading_path: Vec<String>,
            token_estimate: u32,
            #[serde(default)]
            context_prefix: String,
            #[serde(default)]
            neighbor_chunk_ids: Vec<String>,
            #[serde(default)]
            split_reason: String,
            #[serde(default)]
            body_ref: Option<String>,
            #[serde(default)]
            text_ref: Option<String>,
            #[serde(default)]
            summary: Option<String>,
            #[serde(default)]
            retrieval_fields: Option<WireRetrievalFields>,
            #[serde(default)]
            semantic_labels: Vec<SemanticLabel>,
            #[serde(default)]
            semantic_aliases: Vec<String>,
            #[serde(default)]
            block_affinity: Option<BlockAffinity>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireRetrievalFields {
            #[serde(default)]
            summary: Option<String>,
            #[serde(default)]
            semantic_aliases: Vec<String>,
            #[serde(default)]
            body_text_ref: Option<String>,
        }

        let wire = WireChunk::deserialize(deserializer)?;
        let retrieval = wire.retrieval_fields;
        let summary = wire
            .summary
            .or_else(|| retrieval.as_ref().and_then(|fields| fields.summary.clone()));
        let mut semantic_aliases = wire.semantic_aliases;
        if let Some(fields) = &retrieval {
            semantic_aliases.extend(fields.semantic_aliases.clone());
        }
        semantic_aliases.sort();
        semantic_aliases.dedup();
        Ok(Self {
            chunk_id: wire.chunk_id,
            document_id: wire.document_id,
            document_title: wire
                .document_title
                .or(wire.title)
                .unwrap_or_else(|| "knowledge document".to_string()),
            source_path: wire.source_path.unwrap_or_default(),
            heading_path: wire.heading_path,
            token_estimate: wire.token_estimate,
            context_prefix: wire.context_prefix,
            neighbor_chunk_ids: wire.neighbor_chunk_ids,
            split_reason: wire.split_reason,
            body_ref: wire
                .body_ref
                .or(wire.text_ref)
                .or_else(|| retrieval.and_then(|fields| fields.body_text_ref))
                .unwrap_or_default(),
            summary,
            semantic_labels: wire.semantic_labels,
            semantic_aliases,
            block_affinity: wire.block_affinity,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLabel {
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockAffinity {
    #[serde(default)]
    pub phase_scope: f64,
    #[serde(default)]
    pub concept_grounding: f64,
    #[serde(default)]
    pub frontend_experience: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LexicalIndex {
    #[serde(deserialize_with = "deserialize_schema_version")]
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
    #[serde(deserialize_with = "deserialize_schema_version")]
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
    #[serde(deserialize_with = "deserialize_schema_version")]
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

fn deserialize_schema_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|raw| u32::try_from(raw).ok())
            .ok_or_else(|| de::Error::custom("schemaVersion must be a positive integer")),
        Value::String(raw) => raw
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok())
            .ok_or_else(|| de::Error::custom("schemaVersion string must start with a number")),
        other => Err(de::Error::custom(format!(
            "unsupported schemaVersion value: {other}"
        ))),
    }
}

fn deserialize_string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}
