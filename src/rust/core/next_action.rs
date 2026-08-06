use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoomMcpNextAction {
    WriteArtifact(WriteArtifactNext),
    ExecuteTask(ExecuteTaskNext),
    RunVsefmVerification(VsefmVerificationNext),
    RunVsefmRepair(VsefmRepairNext),
    RunLoomTool(RunLoomToolNext),
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
    VsefmVerificationResult,
    VsefmRepairResult,
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
pub struct RunLoomToolNext {
    pub tool_name: String,
    pub request_ref: String,
    pub read_groups: Vec<ReadGroupRef>,
    pub retry_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadGroupRef {
    pub group_id: String,
    pub required: bool,
    pub order: u32,
    pub purpose: String,
    pub when_to_read: String,
    #[serde(default = "default_projection_mode")]
    pub projection_mode: String,
    pub selectors: Vec<ReadSelector>,
    pub read_tool: String,
    pub resource_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadSelector {
    pub base: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<ReadSelectorField>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_base: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ReadSelectorField {
    Field(String),
    Nested(ReadSelector),
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
            projection_mode: default_projection_mode(),
            selectors: read_selectors_from_paths(fields),
            read_tool: "loom.readFieldGroup".to_string(),
            resource_uri: resource_uri.into(),
        }
    }

    pub fn expanded_fields(&self) -> Vec<String> {
        expand_read_selectors(&self.selectors)
    }
}

fn default_projection_mode() -> String {
    "targeted".to_string()
}

pub fn read_selectors_from_paths<I, S>(paths: I) -> Vec<ReadSelector>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut root = BTreeMap::<String, SelectorNode>::new();
    let mut seen = BTreeSet::new();
    for path in paths {
        let path = path.as_ref().trim();
        if path.is_empty() || !seen.insert(path.to_string()) {
            continue;
        }
        let parts = path
            .split('.')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            continue;
        }
        insert_selector_path(&mut root, &parts);
    }
    root.into_iter()
        .map(|(base, node)| selector_from_node(base, node))
        .collect()
}

pub fn read_selectors_value_from_paths<I, S>(paths: I) -> serde_json::Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    serde_json::to_value(read_selectors_from_paths(paths))
        .unwrap_or_else(|_| serde_json::Value::Array(vec![]))
}

pub fn expand_read_selectors(selectors: &[ReadSelector]) -> Vec<String> {
    let mut fields = Vec::new();
    for selector in selectors {
        expand_selector("", selector, &mut fields);
    }
    let mut seen = BTreeSet::new();
    fields
        .into_iter()
        .filter(|field| seen.insert(field.clone()))
        .collect()
}

fn expand_selector(prefix: &str, selector: &ReadSelector, fields: &mut Vec<String>) {
    let base = selector.base.trim();
    if base.is_empty() {
        return;
    }
    let full_base = join_selector_path(prefix, base);
    if selector.include_base {
        fields.push(full_base.clone());
    }
    for field in &selector.fields {
        match field {
            ReadSelectorField::Field(name) => {
                let name = name.trim();
                if !name.is_empty() {
                    fields.push(join_selector_path(&full_base, name));
                }
            }
            ReadSelectorField::Nested(nested) => expand_selector(&full_base, nested, fields),
        }
    }
}

fn join_selector_path(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        suffix.to_string()
    } else {
        format!("{prefix}.{suffix}")
    }
}

#[derive(Default)]
struct SelectorNode {
    include_self: bool,
    children: BTreeMap<String, SelectorNode>,
}

fn insert_selector_path(root: &mut BTreeMap<String, SelectorNode>, parts: &[&str]) {
    let mut node = root.entry(parts[0].to_string()).or_default();
    for part in &parts[1..] {
        node = node.children.entry((*part).to_string()).or_default();
    }
    node.include_self = true;
}

fn selector_from_node(base: String, node: SelectorNode) -> ReadSelector {
    let fields = node
        .children
        .into_iter()
        .map(|(name, child)| {
            if child.include_self && child.children.is_empty() {
                ReadSelectorField::Field(name)
            } else {
                ReadSelectorField::Nested(selector_from_node(name, child))
            }
        })
        .collect();
    ReadSelector {
        base,
        fields,
        include_base: node.include_self,
    }
}

fn is_false(value: &bool) -> bool {
    !*value
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
#[serde(rename_all = "camelCase")]
pub struct VsefmVerificationNext {
    pub verification_id: String,
    pub request_ref: String,
    pub result_file: String,
    pub prompt_ref: String,
    pub subject_ref: String,
    pub scope: String,
    pub read_groups: Vec<ReadGroupRef>,
    pub submit_tool: String,
    pub allowed_paths: Vec<String>,
    pub protected_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VsefmRepairNext {
    pub repair_id: String,
    pub verification_id: String,
    pub request_ref: String,
    pub result_file: String,
    pub read_groups: Vec<ReadGroupRef>,
    pub submit_tool: String,
    pub scope_hints: Vec<String>,
    pub protected_paths: Vec<String>,
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
    pub primary_reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DeploymentFailureDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_actions: Vec<String>,
    pub editable_files: Vec<String>,
    pub protected_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_repair_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated_file_refs: Vec<String>,
    pub diagnostics_ref: Option<String>,
    pub error_window: Option<DeploymentErrorWindow>,
    pub read_policy: DeploymentRepairReadPolicy,
    pub deploy_reference_profile: DeployReferenceProfile,
    pub retry_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployReferenceProfile {
    pub load_mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_load_plan: Vec<ReferenceLoadPlanItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceLoadPlanItem {
    pub ref_id: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentFailureDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    pub suggested_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentErrorWindow {
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub lines: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_line_count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRepairReadPolicy {
    pub first_read: String,
    pub diagnostics_ref: String,
    pub full_log_ref: String,
}
