use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentShape {
    SingleService,
    FrontendAndBackend,
}

impl Default for DeploymentShape {
    fn default() -> Self {
        Self::SingleService
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Node,
    Python,
    Go,
    Java,
    Dotnet,
    Php,
    Ruby,
    Static,
    Unknown,
}

impl Default for RuntimeKind {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Pip,
    Poetry,
    Uv,
    Go,
    Maven,
    Gradle,
    Dotnet,
    Composer,
    Bundler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyServiceKind {
    Postgres,
    Redis,
    Mysql,
    Mongodb,
    Rabbitmq,
    Elasticsearch,
    Minio,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyService {
    pub kind: DependencyServiceKind,
    pub service_name: String,
    pub image: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub connection_env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_target: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContractEndpoint {
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_by_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContractApi {
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentContract {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRuntimeContract {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    pub status: String,
    pub dependency_service_policy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_shape: Option<DeploymentShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub preview_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_output_dir: Option<String>,
    pub probe_kind: String,
    pub environment: RuntimeEnvironmentContract,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend: Option<RuntimeContractEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<RuntimeContractApi>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_services: Vec<DependencyService>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceModelSource {
    CodeProbe,
    RuntimeContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceServiceRole {
    App,
    Frontend,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentSourceService {
    pub service_id: String,
    pub role: SourceServiceRole,
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_package_json_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifest_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lockfile_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<String>,
    pub runtime_kind: RuntimeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<PackageManager>,
    pub has_lockfile: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_version_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_directory: Option<String>,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentSourceModel {
    pub schema_version: u32,
    pub source: SourceModelSource,
    pub shape: DeploymentShape,
    pub primary_service_id: String,
    pub preview_service_id: String,
    pub build_context_path: String,
    pub services: Vec<DeploymentSourceService>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DependencyService>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum DeploymentRoute {
    StaticSpa {
        public_path: String,
        target_service_id: String,
    },
    HttpProxy {
        public_path: String,
        target_service_id: String,
        target_port: u16,
        preserve_path: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentTopologyValidation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preview_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentTopology {
    pub schema_version: u32,
    pub public_entry_service_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<DeploymentRoute>,
    pub validation: DeploymentTopologyValidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentTopologyClass {
    SingleServiceApp,
    ApiOnlySingleService,
    StaticSite,
    BackendServedFrontendApi,
    FrontendGatewayBackendApi,
    MultiService,
    ExistingCompose,
    ExistingDockerfileWrapper,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentLayoutKind {
    RootApp,
    SplitFrontendBackend,
    WorkspaceApp,
    SameRootFullstack,
    ExistingAssets,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentFacts {
    pub schema_version: u32,
    pub topology_class: DeploymentTopologyClass,
    pub layout_kind: DeploymentLayoutKind,
    pub public_entry_service_id: String,
    pub primary_service_id: String,
    pub service_count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack_kinds: Vec<RuntimeKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_kinds: Vec<DependencyServiceKind>,
    pub public_port_count: u32,
    pub internal_port_count: u32,
    pub generated_asset_policy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeployProvider {
    ComposeExisting,
    DockerfileExisting,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentProviderPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<DeployProvider>,
    pub reuse_existing: bool,
    pub force_generate: bool,
}

impl Default for DeploymentProviderPolicy {
    fn default() -> Self {
        Self {
            provider: None,
            reuse_existing: true,
            force_generate: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentProviderCandidateStatus {
    Selected,
    Available,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentProviderCandidate {
    pub provider: DeployProvider,
    pub status: DeploymentProviderCandidateStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentComposePort {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_port: Option<u16>,
    pub container_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentComposeService {
    pub name: String,
    pub score: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub build: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<DeploymentComposePort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expose: Vec<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    pub dependency_like: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentComposeInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_service: Option<String>,
    pub service_reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<DeploymentComposeService>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEnvVariable {
    pub name: String,
    pub required: bool,
    pub sensitive: bool,
    pub provided: bool,
    pub generated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEnvDiagnostics {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<DeploymentEnvVariable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referenced: Vec<DeploymentEnvVariable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provided: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub generated: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<DeploymentEnvVariable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentBootstrapTask {
    pub kind: String,
    pub command: String,
    pub automatic: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentBootstrapDiagnostics {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<DeploymentBootstrapTask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentGeneratedFiles {
    pub compose_path: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dockerfile_paths: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dockerignore_paths: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nginx_config_paths: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reused: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRuntimePort {
    pub service_id: String,
    pub purpose: String,
    pub container_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_host_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_port: Option<u16>,
    pub path: String,
    pub internal_only: bool,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRuntime {
    pub primary_service_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<DeploymentRuntimePort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentSpec {
    pub schema_version: u32,
    pub provider: DeployProvider,
    pub provider_reason: String,
    pub provider_policy: DeploymentProviderPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_candidates: Vec<DeploymentProviderCandidate>,
    pub service_name: String,
    pub image_name: String,
    pub project_root: String,
    pub generated_at: String,
    pub runtime_contract_ref: String,
    pub source_model_ref: String,
    pub topology_ref: String,
    pub code_evidence_ref: String,
    pub facts_ref: String,
    pub runtime_contract: DeploymentRuntimeContract,
    pub source_model: DeploymentSourceModel,
    pub topology: DeploymentTopology,
    pub facts: DeploymentFacts,
    pub environment: DeploymentEnvDiagnostics,
    pub bootstrap: DeploymentBootstrapDiagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose: Option<DeploymentComposeInfo>,
    pub files: DeploymentGeneratedFiles,
    pub runtime: DeploymentRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentFailureKind {
    DockerUnavailable,
    ComposeConfig,
    RegistryNetwork,
    ImageBuild,
    ContainerStart,
    Healthcheck,
    RuntimeContractMissing,
    RuntimeContractNotApplicable,
    RuntimeContractMismatch,
    BuildCommandFailed,
    StartCommandFailed,
    ApplicationStartupFailed,
    HttpProbeFailed,
    PreviewNotVerified,
    ApiRouteNotVerified,
    DeployAssetInvalid,
    Logs,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentFailureOwner {
    ApplicationCode,
    DeploymentAssets,
    Environment,
    ExternalSystem,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentRepairRoute {
    ExecutionRepair,
    DeployRepair,
    ManualReview,
    None,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    pub truncated: bool,
    pub total_line_count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentFailedContract {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRepairAction {
    pub schema_version: u32,
    pub repair_id: String,
    pub created_at: String,
    pub project_root: String,
    pub spec_ref: String,
    pub failure_kind: DeploymentFailureKind,
    pub failure_owner: DeploymentFailureOwner,
    pub repair_route: DeploymentRepairRoute,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_log_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_window: Option<DeploymentErrorWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DeploymentFailureDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub editable_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_files: Vec<String>,
    pub instruction: String,
    pub max_attempts: u32,
    pub attempts: u32,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentFailureReport {
    pub schema_version: String,
    pub failure_id: String,
    pub source: String,
    pub created_at: String,
    pub deployment_attempt_id: String,
    pub failure_kind: DeploymentFailureKind,
    pub failure_owner: DeploymentFailureOwner,
    pub repair_route: DeploymentRepairRoute,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_delivery_ref: Option<String>,
    pub deployment_spec_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_contract: Option<DeploymentFailedContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deploy_command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_log_ref: Option<String>,
    pub failed_contract_fields: Vec<String>,
    pub required_code_level_checks: Vec<String>,
    pub error_window: DeploymentErrorWindow,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_not_edit: Vec<String>,
    pub attempt: u32,
    pub max_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployExecutionRepairTaskResult {
    pub schema_version: String,
    pub repair_id: String,
    pub status: String,
    pub deployment_failure_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    pub runtime_delivery_evidence: serde_json::Value,
    pub self_repair_summary: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}
