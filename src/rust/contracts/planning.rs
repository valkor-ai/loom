use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AcceptanceCandidate, FrontendExperience, ScopeItem, UserFacingLanguageConstraint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    #[serde(rename = "new_project", alias = "greenfield")]
    NewProject,
    ExistingProject,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TechnicalBaselineStatus {
    Draft,
    NeedsUserConfirmation,
    AutoAccepted,
    Confirmed,
    Blocked,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TechnicalBaselineSource {
    UserSpecified,
    UserConfirmed,
    DetectedFromRepo,
    AgentInferredFromRepoSignals,
    #[serde(
        rename = "agent_recommended_for_new_project",
        alias = "agent_recommended_for_greenfield"
    )]
    AgentRecommendedForNewProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TechnicalBaselineScope {
    Project,
    Roadmap,
    PhaseOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TechnicalBaselineApprovalType {
    UserConfirmed,
    PolicyAutoAccept,
    ManualOverride,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalBaselineApproval {
    pub r#type: TechnicalBaselineApprovalType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalBaselineEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalBaselineAlternative {
    pub name: String,
    pub tradeoff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalBaselineCandidateAgentWritable {
    pub status: TechnicalBaselineStatus,
    pub source: TechnicalBaselineSource,
    pub project_kind: ProjectKind,
    pub scope: TechnicalBaselineScope,
    pub stack: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<TechnicalBaselineEvidence>,
    pub approval: TechnicalBaselineApproval,
    pub confidence: ConfidenceLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_user_confirmation: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_summary: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<TechnicalBaselineAlternative>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalBaselineContract {
    pub schema_version: String,
    pub technical_baseline_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub status: TechnicalBaselineStatus,
    pub source: TechnicalBaselineSource,
    pub project_kind: ProjectKind,
    pub scope: TechnicalBaselineScope,
    pub stack: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<TechnicalBaselineEvidence>,
    pub approval: TechnicalBaselineApproval,
    pub confidence: ConfidenceLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_user_confirmation: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_summary: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<TechnicalBaselineAlternative>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryMode {
    EmptyProject,
    ExistingProject,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhaseDevelopmentMode {
    InitialDelivery,
    IncrementalDelivery,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelevantSurfaceKind {
    Entrypoint,
    Module,
    Service,
    Controller,
    DataAccess,
    Ui,
    Config,
    Test,
    Script,
    Documentation,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceRelevance {
    ImplementedCapability,
    ArchitectureBoundary,
    ExtensionPoint,
    ValidationSurface,
    DeliveryContext,
    Unrelated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedUse {
    InspectOnly,
    InspectOrExtend,
    ReuseExistingPattern,
    AvoidModifying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Implemented,
    Partial,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedReadReason {
    ImplementedCapability,
    DependencyContext,
    IntegrationBoundary,
    TestOrValidation,
    RiskReview,
    ExtensionPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadPriority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryShape {
    SinglePackage,
    Monorepo,
    MultiApplication,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextCoverage {
    Focused,
    Partial,
    Broad,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryContextSource {
    pub request_ref: String,
    pub brainstorm_contract_ref: String,
    pub technical_baseline_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryContextRequestLens {
    pub project_kind: ProjectKind,
    pub baseline_project_kind: ProjectKind,
    pub repository_mode: RepositoryMode,
    pub phase_development_mode: PhaseDevelopmentMode,
    pub scan_purpose: String,
    pub primary_consumer: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub later_consumers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryApplication {
    pub application_id: String,
    pub name: String,
    pub kind: String,
    pub root_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryOverview {
    pub summary: String,
    pub repository_shape: RepositoryShape,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primary_applications: Vec<PrimaryApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnologySignals {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primary_languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_managers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructureRootPath {
    pub path: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryEntryPoint {
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructureSignals {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_paths: Vec<StructureRootPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entry_points: Vec<RepositoryEntryPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configuration_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExistingCapability {
    pub capability_id: String,
    pub name: String,
    pub status: CapabilityStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ConfidenceLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_relevance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelevantSurface {
    pub surface_id: String,
    pub kind: RelevantSurfaceKind,
    pub path: String,
    pub summary: String,
    pub relevance: SurfaceRelevance,
    pub suggested_use: SuggestedUse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedReadRef {
    pub path: String,
    pub reason: RecommendedReadReason,
    pub priority: ReadPriority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextQuality {
    pub coverage: ContextCoverage,
    pub confidence: ConfidenceLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ContextWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryContextCandidateAgentWritable {
    pub status: String,
    pub source: RepositoryContextSource,
    pub request_lens: RepositoryContextRequestLens,
    pub repo_overview: RepositoryOverview,
    pub technology_signals: TechnologySignals,
    pub structure_signals: StructureSignals,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub existing_capabilities: Vec<ExistingCapability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relevant_surfaces: Vec<RelevantSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommended_read_refs: Vec<RecommendedReadRef>,
    pub context_quality: ContextQuality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ContextWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryContextContract {
    pub schema_version: String,
    pub repository_context_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub status: String,
    pub source: RepositoryContextSource,
    pub request_lens: RepositoryContextRequestLens,
    pub repo_overview: RepositoryOverview,
    pub technology_signals: TechnologySignals,
    pub structure_signals: StructureSignals,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub existing_capabilities: Vec<ExistingCapability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relevant_surfaces: Vec<RelevantSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommended_read_refs: Vec<RecommendedReadRef>,
    pub context_quality: ContextQuality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ContextWarning>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanningContractStatus {
    Draft,
    Ready,
    Blocked,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDetailItem {
    pub detail_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub required_for_current_phase: bool,
    pub priority: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_field_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concept_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontend_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impact_tags: Vec<String>,
    pub lifecycle_stage: String,
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDetailsIndex {
    pub schema_version: String,
    pub authority: String,
    pub source_brainstorm_contract_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<RequirementDetailItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extraction_warnings: Vec<ContextWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningContractSource {
    pub brainstorm_run_id: String,
    pub brainstorm_contract_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roadmap_id: Option<String>,
    pub phase_id: String,
    pub technical_baseline_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningContractPhaseScope {
    pub phase_name: String,
    pub phase_goal: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub included: Vec<ScopeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred: Vec<ScopeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<ScopeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_candidates: Vec<AcceptanceCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningContractContextRefs {
    pub brainstorm_contract_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_context_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_concept_glossary_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_concept_grounding_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_frontend_experience_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_frontend_experience_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningContractTechnicalBaseline {
    pub technical_baseline_id: String,
    pub status: TechnicalBaselineStatus,
    pub scope: TechnicalBaselineScope,
    pub summary: Value,
    pub must_follow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningInputs {
    pub business_goal: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_groups: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub business_flows: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience: Option<FrontendExperience>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_facing_language: Option<UserFacingLanguageConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeIsolationRules {
    pub only_plan_current_phase: bool,
    pub forbid_deferred_scope_implementation: bool,
    pub forbid_future_phase_implementation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputRequirements {
    pub must_create_architecture_artifact_contract: bool,
    pub must_create_task_plan: bool,
    pub task_plan_must_reference_acceptance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningDeploymentRules {
    pub default_enabled: bool,
    pub requires_explicit_user_request: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningRules {
    pub scope_isolation: ScopeIsolationRules,
    pub output_requirements: OutputRequirements,
    pub deployment: PlanningDeploymentRules,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualityGates {
    pub requires_architecture_before_task_plan: bool,
    pub requires_acceptance_coverage: bool,
    pub requires_verification_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningHandoff {
    pub ready_for_architecture: bool,
    pub ready_for_task_plan: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_reasons: Vec<String>,
    pub next_node: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningGenerationContract {
    pub schema_version: String,
    pub planning_contract_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub status: PlanningContractStatus,
    pub source: PlanningContractSource,
    pub phase_scope: PlanningContractPhaseScope,
    pub context_refs: PlanningContractContextRefs,
    pub technical_baseline: PlanningContractTechnicalBaseline,
    pub planning_inputs: PlanningInputs,
    pub requirement_details: RequirementDetailsIndex,
    pub planning_rules: PlanningRules,
    pub quality_gates: QualityGates,
    pub handoff: PlanningHandoff,
    pub created_at: String,
    pub updated_at: String,
}
