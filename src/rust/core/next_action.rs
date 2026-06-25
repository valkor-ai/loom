use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoomMcpNextAction {
    WriteArtifact(WriteArtifactNext),
    ExecuteTask(ExecuteTaskNext),
    GenerateKnowledgeSemantics(GenerateKnowledgeSemanticsNext),
    DeployRepairAssets(DeployRepairAssetsNext),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteArtifactNext {
    pub artifact_kind: ArtifactKind,
    pub request_ref: String,
    pub write_mode: WriteMode,
    pub write_targets: Vec<WriteTarget>,
    pub read_groups: Vec<ReadGroupRef>,
    pub submit_tool: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    BrainstormCandidate,
    KnowledgeSemanticPackResult,
    TechnicalBaselineCandidate,
    RepositoryContextCandidate,
    ArchitectureSectionCandidate,
    TaskPlanCandidate,
    TaskResult,
    ReviewResult,
    ManualReviewResolution,
    TaskResultRepair,
    TaskplanRepair,
    ArchitectureArtifactRepair,
    DeployExecutionRepairResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    CreateOrReplace,
    PatchExisting,
    SingleJson,
    ArchitectureSection,
    TaskplanGrouped,
    RepairJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteTarget {
    pub target_id: String,
    pub path: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadGroupRef {
    pub group_id: String,
    pub required: bool,
    pub order: u32,
    pub purpose: String,
    pub when_to_read: String,
    pub fields: Vec<String>,
    pub read_tool: String,
    pub resource_uri: String,
}

impl ReadGroupRef {
    pub fn new(
        group_id: impl Into<String>,
        order: u32,
        fields: Vec<String>,
        resource_uri: impl Into<String>,
    ) -> Self {
        Self {
            group_id: group_id.into(),
            required: true,
            order,
            purpose: "Read the request fields needed for the current action.".to_string(),
            when_to_read: "Before writing the requested artifact.".to_string(),
            fields,
            read_tool: "loom.readFieldGroup".to_string(),
            resource_uri: resource_uri.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteTaskNext {
    pub execution_kind: ExecutionKind,
    pub repair_origin: Option<RepairOrigin>,
    pub request_ref: String,
    pub result_file: String,
    pub task_id: String,
    pub group_id: Option<String>,
    pub read_groups: Vec<ReadGroupRef>,
    pub submit_tool: String,
    pub edit_boundary: ExecuteEditBoundary,
    pub verification_policy: ExecuteVerificationPolicy,
    pub repair_context: Option<RepairContext>,
    pub post_submit: PostSubmitAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    PlannedTask,
    DeliveryExecutionRepair,
    DeployExecutionRepair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepairOrigin {
    TaskFailure,
    ReviewResult,
    ManualReviewResolution,
    DeployFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteEditBoundary {
    pub allowed_paths: Vec<String>,
    pub protected_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteVerificationPolicy {
    pub required_commands: Vec<String>,
    pub evidence_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepairContext {
    pub repair_origin: RepairOrigin,
    pub repair_request_ref: String,
    pub source_task_id: String,
    pub issues: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_result_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_review_resolution_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_change_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_task_result_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_failure_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_contract_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_code_level_checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostSubmitAction {
    ContinueDelivery,
    RetryDeploy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateKnowledgeSemanticsNext {
    pub source_name: String,
    pub source_id: String,
    pub build_id: String,
    pub pack_id: String,
    pub pack_index: u32,
    pub pack_count: u32,
    pub request_ref: String,
    pub result_file: String,
    pub output_contract: serde_json::Value,
    pub generation_rules: serde_json::Value,
    pub read_mode: KnowledgeReadMode,
    pub chunk_read_plan: Vec<KnowledgeChunkReadRef>,
    pub submit_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeReadMode {
    ChunkInspect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunkReadRef {
    pub source_name: String,
    pub source_id: String,
    pub build_id: String,
    pub chunk_id: String,
    pub document_title: String,
    pub heading_path: Vec<String>,
    pub token_estimate: u32,
    pub summary_language: String,
    pub read_tool: String,
    pub resource_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployRepairAssetsNext {
    pub repair_id: String,
    pub failure_kind: String,
    pub failure_owner: String,
    pub repair_route: String,
    pub editable_files: Vec<String>,
    pub protected_files: Vec<String>,
    pub diagnostics_ref: Option<String>,
    pub error_window: Option<DeploymentErrorWindow>,
    pub retry_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentErrorWindow {
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub lines: Vec<String>,
}
