use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserFacingLanguageConstraint {
    pub default_locale: UserFacingLocale,
    pub source: UserFacingLanguageSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub does_not_apply_to: Vec<String>,
    pub rule: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SecurityRequirementApplicability {
    NotApplicable,
    Required,
    Optional,
    DeferredWithRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClientTrustModel {
    SameOriginBrowser,
    ExternalApi,
    ServiceToService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityRequirement {
    pub applies: SecurityRequirementApplicability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub client_trust_models: Vec<ClientTrustModel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    pub rationale: String,
}

impl Default for SecurityRequirement {
    fn default() -> Self {
        Self {
            applies: SecurityRequirementApplicability::NotApplicable,
            client_trust_models: Vec::new(),
            source_refs: Vec::new(),
            rationale: "Security applicability was not selected for this delivery.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UserFacingLocale {
    ZhCn,
    En,
    Und,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserFacingLanguageSource {
    ProjectDefault,
    RequirementPrimaryLanguage,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequirementSourceType {
    UserText,
    Pdf,
    Word,
    Markdown,
    Text,
    Code,
    Spreadsheet,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementSource {
    pub source_id: String,
    pub r#type: RequirementSourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementSourceItem {
    pub item_id: String,
    pub kind: RequirementSourceItemKind,
    pub origin: RequirementSourceOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_text_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_count: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequirementSourceItemKind {
    Text,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequirementSourceOrigin {
    UserMessage,
    CliOption,
    Stdin,
    RequestFile,
    ContextFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementContext {
    pub schema_version: String,
    pub delivery_id: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_items: Vec<RequirementSourceItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_text_ref: Option<String>,
    pub normalized_text_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_text_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword_hints_ref: Option<String>,
    pub keyword_hints_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword_hints_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Small,
    Medium,
    Large,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestSummary {
    pub title: String,
    pub one_line: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_goal: Option<String>,
    pub complexity: Complexity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScopeSource {
    SourceExplicit,
    UserConfirmed,
    UserOverridden,
    ModelRecommended,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeItem {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub source: ScopeSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeAssumption {
    pub id: String,
    pub text: String,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormScope {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub included: Vec<ScopeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<ScopeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred: Vec<ScopeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<ScopeAssumption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityGroup {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BusinessFlow {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_refs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainModel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<Actor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_groups: Vec<CapabilityGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub business_flows: Vec<BusinessFlow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AcceptancePriority {
    Must,
    Should,
    Could,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCandidate {
    pub id: String,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    pub priority: AcceptancePriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    ScopeConfirmed,
    Proposed,
    Delivered,
    Paused,
    Skipped,
    Revised,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapPhase {
    pub phase_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub status: PhaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Roadmap {
    pub required: bool,
    #[serde(default)]
    #[schemars(skip)]
    pub current_phase_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<RoadmapPhase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PhasePlanCurrent {
    #[serde(default)]
    #[schemars(skip)]
    pub phase_id: String,
    pub title: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    pub status: PhasePlanCurrentStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhasePlanCurrentStatus {
    ScopeConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NextPhasePreview {
    Candidate {
        #[serde(rename = "suggestedPhaseId", alias = "suggested_phase_id")]
        suggested_phase_id: String,
        title: String,
        goal: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[serde(rename = "scopePreview", alias = "scope_preview")]
        scope_preview: Vec<String>,
        reason: String,
    },
    None {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PhasePlan {
    pub current: PhasePlanCurrent,
    pub next_phase_preview: NextPhasePreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConceptGroundingMode {
    ConceptsPresent,
    NoneRequired,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConceptPhaseRelevance {
    Current,
    CurrentAdjacent,
    Future,
    Deferred,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConceptPriority {
    MustUnderstand,
    ShouldUnderstand,
    NiceToUnderstand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConceptRiskFactor {
    BusinessInvariant,
    StateTransition,
    ResourceConsistency,
    PermissionBoundary,
    ExternalContract,
    ScopeConfusionRisk,
    UserVisibleFlow,
    RuntimeOrDeliverySemantics,
    FrontendExperienceSemantics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementConcept {
    pub concept_id: String,
    pub term: String,
    pub normalized_name: String,
    pub explanation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_not_misinterpret_as: Vec<String>,
    pub phase_relevance: ConceptPhaseRelevance,
    pub priority: ConceptPriority,
    pub attention_rank: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_factors: Vec<ConceptRiskFactor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    pub human_readable_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptSet {
    pub mode: ConceptGroundingMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<RequirementConcept>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GlossaryUpdate {
    #[serde(default)]
    #[schemars(skip)]
    pub update_id: String,
    pub operation: GlossaryUpdateOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept: Option<RequirementConcept>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryUpdateOperation {
    Add,
    Replace,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptGrounding {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_concept_glossary: Option<ConceptSet>,
    pub phase_concept_grounding: ConceptSet,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glossary_updates: Vec<GlossaryUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptConfirmation {
    pub shown_to_user: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub confirmed_concept_refs: Vec<String>,
    pub confirmation_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClarificationBlockName {
    PhaseScope,
    ConceptGrounding,
    FrontendExperience,
    FinalSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationBlock {
    pub block: ClarificationBlockName,
    pub summary: String,
    pub confirmed_by_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkippedClarificationBlock {
    pub block: ClarificationBlockName,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationProgress {
    pub mode: ClarificationMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub confirmed_blocks: Vec<ClarificationBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_blocks: Vec<SkippedClarificationBlock>,
    pub final_summary_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClarificationMode {
    ProgressiveBlocks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontendExperienceLevel {
    None,
    TechnicalDemo,
    UsableInternalProduct,
    PolishedProduct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontendTargetSelectionMode {
    QueryAndSelect,
    DirectIdLookup,
    PreselectedContext,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontendActionEntryPoint {
    ResultRowAction,
    DetailButton,
    FormSubmit,
    BulkAction,
    InlineAction,
    NavigationEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontendResultObservationMode {
    ListRefresh,
    DetailRefresh,
    InlineStatusUpdate,
    ResponseMessage,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontendInteractionState {
    Loading,
    Success,
    Error,
    Empty,
    BusinessBlocking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSearchCriterion {
    pub criterion_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_ref: Option<String>,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendDataView {
    pub view_id: String,
    pub name: String,
    pub purpose: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_object: Option<String>,
    pub selection_mode: FrontendTargetSelectionMode,
    pub pagination_required: bool,
    pub default_loads_first_page: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_criteria: Vec<FrontendSearchCriterion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criteria_unclear_note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendActionPath {
    pub action_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_object: Option<String>,
    pub entry_point: FrontendActionEntryPoint,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result_observation: Vec<FrontendResultObservationMode>,
    pub refresh_policy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub success_feedback: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_or_error_feedback: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendOperationPath {
    pub path_id: String,
    pub name: String,
    pub user_goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_object: Option<String>,
    pub selection_mode: FrontendTargetSelectionMode,
    pub selection_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_view_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_states: Vec<FrontendInteractionState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAudience {
    pub audience_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primary_jobs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSurface {
    pub surface_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audience_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primary_jobs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendExperience {
    pub required: bool,
    pub kind: String,
    pub experience_level: FrontendExperienceLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audiences: Vec<FrontendAudience>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<FrontendSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_views: Vec<FrontendDataView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<FrontendActionPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operation_paths: Vec<FrontendOperationPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_not: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationBasis {
    pub initial_request_only: bool,
    pub summary_presented_to_user: bool,
    pub confirmed_after_summary: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presented_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserConfirmation {
    pub confirmed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<String>,
    pub confirmation_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_basis: Option<ConfirmationBasis>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrainstormCandidateAgentWritable {
    pub request_summary: RequestSummary,
    pub scope: BrainstormScope,
    pub roadmap: Roadmap,
    pub phase_plan: PhasePlan,
    #[serde(default)]
    pub security_requirement: SecurityRequirement,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance: Vec<AcceptanceCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_model: Option<DomainModel>,
    #[schemars(skip)]
    pub user_confirmation: UserConfirmation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_grounding: Option<ConceptGrounding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_confirmation: Option<ConceptConfirmation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub clarification_progress: Option<ClarificationProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience: Option<FrontendExperience>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptGroundingRefs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_concept_glossary_ref: Option<String>,
    pub phase_concept_grounding_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendExperienceRefs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_frontend_experience_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_frontend_experience_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormStatus {
    NeedsClarification,
    Confirmed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormHandoffNode {
    TechnicalBaselineGeneration,
    PlanningGenerationContract,
    BrainstormClarification,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormHandoff {
    pub ready: bool,
    pub next_node: BrainstormHandoffNode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryContext {
    pub original_request: OriginalRequestContext,
    pub user_facing_language: UserFacingLanguageConstraint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OriginalRequestContext {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormContract {
    pub schema_version: String,
    pub contract_id: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub brainstorm_run_id: String,
    pub status: BrainstormStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<RequirementSource>,
    pub summary: RequestSummary,
    pub scope: BrainstormScope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance: Vec<AcceptanceCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_model: Option<DomainModel>,
    pub user_confirmation: UserConfirmation,
    pub delivery_context: DeliveryContext,
    pub roadmap: Roadmap,
    pub phase_plan: PhasePlan,
    #[serde(default)]
    pub security_requirement: SecurityRequirement,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_grounding: Option<ConceptGrounding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_confirmation: Option<ConceptConfirmation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_progress: Option<ClarificationProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_grounding_refs: Option<ConceptGroundingRefs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience: Option<FrontendExperience>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience_refs: Option<FrontendExperienceRefs>,
    pub handoff: BrainstormHandoff,
    pub created_at: String,
    pub updated_at: String,
}
