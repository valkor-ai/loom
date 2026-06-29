use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ArtifactKind, RouteAction};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileSubmitInput {
    pub project_root: String,
    pub request_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_target_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmitToolSpec {
    pub name: &'static str,
    pub target_batch: u32,
    pub allowed_artifact_kinds: &'static [ArtifactKind],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitPreflightSummary {
    pub request_ref: String,
    pub request_id: String,
    pub artifact_kind: ArtifactKind,
    pub submit_tool: String,
    pub target_ids: Vec<String>,
    pub next_action: Option<RouteAction>,
}

const BRAINSTORM_KINDS: &[ArtifactKind] = &[ArtifactKind::BrainstormCandidate];
const KNOWLEDGE_SEMANTIC_KINDS: &[ArtifactKind] = &[ArtifactKind::KnowledgeSemanticPackResult];
const TECHNICAL_BASELINE_KINDS: &[ArtifactKind] = &[ArtifactKind::TechnicalBaselineCandidate];
const REPOSITORY_CONTEXT_KINDS: &[ArtifactKind] = &[ArtifactKind::RepositoryContextCandidate];
const ARCHITECTURE_SECTION_KINDS: &[ArtifactKind] = &[ArtifactKind::ArchitectureSectionCandidate];
const TASK_PLAN_KINDS: &[ArtifactKind] = &[ArtifactKind::TaskPlanCandidate];
const TASK_RESULT_KINDS: &[ArtifactKind] = &[ArtifactKind::TaskResult];
const REVIEW_RESULT_KINDS: &[ArtifactKind] = &[ArtifactKind::ReviewResult];
const MANUAL_REVIEW_KINDS: &[ArtifactKind] = &[ArtifactKind::ManualReviewResolution];
const REPAIR_KINDS: &[ArtifactKind] = &[
    ArtifactKind::TaskResultRepair,
    ArtifactKind::TaskplanRepair,
    ArtifactKind::ArchitectureArtifactRepair,
    ArtifactKind::DeployExecutionRepairResult,
];

pub const SUBMIT_TOOL_SPECS: &[SubmitToolSpec] = &[
    SubmitToolSpec {
        name: "brainstormAcceptFile",
        target_batch: 7,
        allowed_artifact_kinds: BRAINSTORM_KINDS,
    },
    SubmitToolSpec {
        name: "knowledgeSemanticSubmitFile",
        target_batch: 6,
        allowed_artifact_kinds: KNOWLEDGE_SEMANTIC_KINDS,
    },
    SubmitToolSpec {
        name: "technicalBaselineAcceptFile",
        target_batch: 8,
        allowed_artifact_kinds: TECHNICAL_BASELINE_KINDS,
    },
    SubmitToolSpec {
        name: "repositoryContextAcceptFile",
        target_batch: 8,
        allowed_artifact_kinds: REPOSITORY_CONTEXT_KINDS,
    },
    SubmitToolSpec {
        name: "architectureSectionSubmitFile",
        target_batch: 8,
        allowed_artifact_kinds: ARCHITECTURE_SECTION_KINDS,
    },
    SubmitToolSpec {
        name: "taskPlanAcceptFile",
        target_batch: 8,
        allowed_artifact_kinds: TASK_PLAN_KINDS,
    },
    SubmitToolSpec {
        name: "recordTaskResultFile",
        target_batch: 8,
        allowed_artifact_kinds: TASK_RESULT_KINDS,
    },
    SubmitToolSpec {
        name: "reviewAcceptFile",
        target_batch: 9,
        allowed_artifact_kinds: REVIEW_RESULT_KINDS,
    },
    SubmitToolSpec {
        name: "reviewResolveFile",
        target_batch: 9,
        allowed_artifact_kinds: MANUAL_REVIEW_KINDS,
    },
    SubmitToolSpec {
        name: "repairSubmitFile",
        target_batch: 5,
        allowed_artifact_kinds: REPAIR_KINDS,
    },
];

pub fn submit_tool_spec(name: &str) -> Option<SubmitToolSpec> {
    let name = canonical_tool_name(name);
    SUBMIT_TOOL_SPECS
        .iter()
        .copied()
        .find(|spec| spec.name == name)
}

pub fn is_submit_tool(name: &str) -> bool {
    submit_tool_spec(name).is_some()
}

pub fn submit_tool_accepts_artifact(tool_name: &str, artifact_kind: ArtifactKind) -> bool {
    submit_tool_spec(tool_name)
        .map(|spec| spec.allowed_artifact_kinds.contains(&artifact_kind))
        .unwrap_or(false)
}

pub fn canonical_tool_name(name: &str) -> &str {
    name.strip_prefix("loom.").unwrap_or(name)
}
