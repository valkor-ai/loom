use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::{BrowserAutomationFacts, BrowserVerificationProfile, CodeStackSignal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskPlanStatus {
    Draft,
    Ready,
    NeedsCandidateRepair,
    Blocked,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    FeatureIncrement,
    DataModelIncrement,
    InterfaceIncrement,
    UiFlowIncrement,
    FrontendExperience,
    RuntimeDelivery,
    RuntimeDeliveryClosure,
    BrowserQualityClosure,
    IntegrationIncrement,
    VerificationIncrement,
    RefactorSupport,
    ConfigurationSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationAction {
    CreateOrUpdateEntity,
    CreateOrUpdatePersistence,
    CreateOrUpdateInterface,
    CreateOrUpdateUiFlow,
    CreateOrUpdateFrontendNavigation,
    ImplementReactiveClientFlow,
    ImplementSharedClientState,
    OptimizeFrontendPerformance,
    ImplementServerRenderedComponent,
    ImplementServerMutation,
    ImplementFrontendFrameworkVersionFeature,
    CreateOrUpdateStateMachine,
    CreateOrUpdateBusinessRule,
    AddReferenceField,
    ValidateReferenceFormat,
    UseFixtureOrMockData,
    WireReferenceInApiOrUi,
    CreateEntityCrud,
    CreateEntityRepository,
    CreateEntityAdminPage,
    CreateEntityMigration,
    CreateOrUpdatePersistenceQuery,
    ImplementPersistenceTransaction,
    OptimizePersistenceQuery,
    ImplementAnalyticalQuery,
    ImplementEntityLifecycle,
    AddOrUpdateTests,
    AddOrUpdatePersistenceTests,
    AddOrUpdateConfig,
    ImplementAuthenticationOrAuthorization,
    ImplementAsyncProcessing,
    ImplementCachePolicy,
    ImplementExternalServiceIntegration,
    ImplementResiliencePolicy,
    ConfigureServiceRoutingOrDiscovery,
    ImplementObservability,
    MigrateFrameworkImplementation,
    ImplementFrontendExperienceContract,
    ImplementRuntimeDeliveryContract,
    RefactorSupportingCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationEvidence {
    AutomatedTest,
    BrowserAutomation,
    ManualCommandOutput,
    RuntimeApiCheck,
    StaticCheck,
    AgentReviewExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactRefs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_flows: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_machines: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub nfrs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationIntent {
    pub verification_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_detail_refs: Vec<String>,
    pub behavior: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_evidence: Vec<VerificationEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptable_evidence: Vec<VerificationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptResponsibility {
    pub concept_ref: String,
    pub responsibility: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptVerificationIntent {
    pub concept_ref: String,
    pub evidence_type: String,
    pub intent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskWriteBoundary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_paths: Vec<String>,
    pub artifact_refs: TaskArtifactRefs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCodeLevelCheck {
    #[schemars(skip)]
    #[serde(default)]
    pub check_id: String,
    #[schemars(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_field: Option<String>,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptable_evidence: Vec<VerificationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskRuntimeDeliveryRequirement {
    pub applies_to_this_task: bool,
    pub reason: String,
    #[schemars(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_delivery_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_contract_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_code_level_checks: Vec<RuntimeCodeLevelCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_expected_in_task_result: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_failure_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EngineeringQualityRequirement {
    pub requirement_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to_task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stack_signals: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alignment_targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_field_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_obligations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureQualityRequirement {
    pub requirement_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to_task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nfr_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_obligations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_obligations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiContractRequirement {
    pub requirement_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to_task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interface_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_obligations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_obligations: Vec<String>,
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
pub struct CodeQualityRequirement {
    pub requirement_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to_task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack_signals: Vec<CodeStackSignal>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reference_groups: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_load_plan: Vec<ReferenceLoadPlanItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_naming_policy: Option<CodePackageNamingPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_obligations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_obligations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodePackageNamingPolicy {
    pub applies_to: Vec<String>,
    pub priority_order: Vec<String>,
    pub forbidden_package_prefixes: Vec<String>,
    pub fallback_package_template: String,
    pub absolute_fallback_package: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskDefinition {
    pub task_id: String,
    pub group_id: String,
    pub title: String,
    pub task_kind: TaskKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_actions: Vec<ImplementationAction>,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_detail_refs: Vec<String>,
    pub write_boundary: TaskWriteBoundary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_intents: Vec<VerificationIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concept_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concept_responsibilities: Vec<ConceptResponsibility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concept_verification_intents: Vec<ConceptVerificationIntent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience_requirement: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_delivery_requirement: Option<TaskRuntimeDeliveryRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub engineering_quality_requirement_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub architecture_quality_requirement_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub api_contract_requirement_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub code_quality_requirement_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanGroup {
    pub group_id: String,
    pub title: String,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanBlockedReason {
    pub code: String,
    pub message: String,
    pub next_node: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanOutlineCandidateAgentWritable {
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
    pub status: String,
    #[serde(default)]
    #[schemars(skip)]
    pub task_plan_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<TaskPlanGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<TaskPlanBlockedReason>,
    #[serde(default)]
    #[schemars(skip)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanGroupCandidateAgentWritable {
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
    pub status: String,
    pub group: TaskPlanGroup,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<TaskPlanBlockedReason>,
    #[serde(default)]
    #[schemars(skip)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roadmap_id: Option<String>,
    pub phase_id: String,
    pub planning_generation_contract_id: String,
    pub architecture_artifact_contract_id: String,
    pub technical_baseline_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_contract_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanScopeSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub included_scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred_scope_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanPolicy {
    pub task_granularity: String,
    pub group_granularity: String,
    pub allow_task_split_during_repair: bool,
    pub allow_task_merge_during_repair: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanHandoff {
    pub ready_for_execution: bool,
    pub next_node: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlan {
    pub schema_version: String,
    pub task_plan_id: String,
    pub version: u32,
    pub status: TaskPlanStatus,
    pub source: TaskPlanSource,
    pub scope_snapshot: TaskPlanScopeSnapshot,
    pub planning_policy: TaskPlanPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<TaskPlanGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engineering_quality_requirements: Vec<EngineeringQualityRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architecture_quality_requirements: Vec<ArchitectureQualityRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_contract_requirements: Vec<ApiContractRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_quality_requirements: Vec<CodeQualityRequirement>,
    #[serde(default, skip_serializing_if = "BrowserAutomationFacts::is_empty")]
    pub browser_automation_facts: BrowserAutomationFacts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub browser_verification_profiles: Vec<BrowserVerificationProfile>,
    pub handoff: TaskPlanHandoff,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunStatus {
    Pending,
    Running,
    Completed,
    CompletedWithNotes,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskPlanRunStatus {
    NotStarted,
    Running,
    Completed,
    CompletedWithNotes,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanRunScheduler {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupRunState {
    pub group_id: String,
    pub status: TaskRunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskAttemptState {
    pub attempt: u32,
    pub result_id: String,
    pub status: TaskRunStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunState {
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    pub status: TaskRunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<TaskAttemptState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanRunSummary {
    pub total: u32,
    pub completed: u32,
    pub completed_with_notes: u32,
    pub blocked: u32,
    pub failed: u32,
    pub pending: u32,
    pub running: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanRunNextAction {
    pub r#type: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<String>,
    pub target_node: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanRun {
    pub schema_version: String,
    pub run_id: String,
    pub task_plan_id: String,
    pub status: TaskPlanRunStatus,
    pub scheduler: TaskPlanRunScheduler,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_states: Vec<TaskGroupRunState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_states: Vec<TaskRunState>,
    pub summary: TaskPlanRunSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<TaskPlanRunNextAction>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskResultNoChangeReason {
    pub code: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub verification_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_type: Option<VerificationEvidence>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub browser_checks: Vec<crate::BrowserCheckResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelfRepairSummary {
    pub attempted: bool,
    pub attempt_count: u32,
    pub stop_reason: String,
    pub progress_observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskFailure {
    pub code: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionContinuity {
    pub task_result_submitted_after_verification: bool,
    pub agent_owned_long_running_work: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDetailEvidence {
    pub detail_id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConceptEvidence {
    pub concept_ref: String,
    pub evidence_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureQualityEvidence {
    pub requirement_id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiContractEvidence {
    pub requirement_id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interface_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub success_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pagination_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contract_file_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_gaps: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodeQualityEvidence {
    pub requirement_id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reference_groups_checked: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_files_checked: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands_run: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_gaps: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendQualityStateCoverage {
    pub state: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendQualityBusinessRuleCheck {
    pub rule_id: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendQualityForbiddenContentCheck {
    pub checked: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendQualitySurfaceEvidence {
    pub surface_id: String,
    pub surface_role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub business_actions: Vec<String>,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSurfaceContractEvidence {
    pub id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendContentBoundaryEvidence {
    pub checked: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_content_examples: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_content_violations: Vec<String>,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendDesignTokenEvidence {
    pub strategy_used: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id_used: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_asset_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_consumer_files: Vec<String>,
    pub existing_token_system_reused: bool,
    pub parallel_token_system_created: bool,
    pub merge_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendQualitySelfCheck {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_decision_contract_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_region_evidence: Vec<FrontendSurfaceContractEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_action_evidence: Vec<FrontendSurfaceContractEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_state_evidence: Vec<FrontendSurfaceContractEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_quality_rule_evidence: Vec<FrontendSurfaceContractEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_boundary_evidence: Option<FrontendContentBoundaryEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_plan_files_checked: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_token_evidence: Option<FrontendDesignTokenEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_gaps: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockedReason {
    pub code: String,
    pub next_node: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskResultStatus {
    Completed,
    CompletedWithNotes,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    pub schema_version: String,
    pub task_result_id: String,
    pub task_id: String,
    pub task_plan_id: String,
    pub status: TaskResultStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_change_reason: Option<TaskResultNoChangeReason>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_results: Vec<VerificationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_repair_summary: Option<SelfRepairSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<TaskFailure>,
    pub execution_continuity: ExecutionContinuity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_experience_self_check: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_quality_self_check: Option<FrontendQualitySelfCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_delivery_evidence: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_detail_evidence: Vec<RequirementDetailEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concept_evidence: Vec<ConceptEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architecture_quality_evidence: Vec<ArchitectureQualityEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_contract_evidence: Vec<ApiContractEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_quality_evidence: Vec<CodeQualityEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<BlockedReason>,
    pub created_at: String,
    pub updated_at: String,
}
