mod active_operation;
mod bootstrap;
mod code_evidence;
mod down;
mod existing;
mod facts;
mod generate;
mod inspect;
mod logs;
mod paths;
mod port_plan;
mod prepare;
mod references;
mod repair;
mod run;
mod runtime_contract;
mod runtime_state;
mod source_model;
mod status;
mod strategy;
mod topology;
mod up;
mod validate;

use contracts::DeploymentProviderPolicy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use bootstrap::deploy_bootstrap;
pub use down::deploy_down;
pub use inspect::deploy_inspect;
pub use logs::deploy_logs;
pub use prepare::deploy_prepare;
pub use repair::{accept_deploy_execution_repair_file, deploy_repair};
pub use run::deploy_run;
pub use status::deploy_status;
pub use up::deploy_up;
pub use validate::deploy_validate;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployToolInput {
    pub project_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_policy: Option<DeploymentProviderPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployBootstrapInput {
    pub project_root: String,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

pub fn module_name() -> &'static str {
    "deploy"
}
