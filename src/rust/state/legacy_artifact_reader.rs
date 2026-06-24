use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    paths::{from_project_relative, project_paths},
    store::{read_json_value, StateError, StateResult},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LegacyArtifactValue {
    pub artifact_file: String,
    pub value: Value,
}

pub fn read_legacy_ts_artifact(
    project_root: &str,
    artifact_file: &str,
) -> StateResult<LegacyArtifactValue> {
    if artifact_file.trim().is_empty() {
        return Err(StateError::InvalidArgument(
            "legacy artifact file is required".to_string(),
        ));
    }
    let paths = project_paths(project_root)?;
    let absolute = from_project_relative(&paths.root, artifact_file)?;
    let value = read_json_value(&absolute)?;
    Ok(LegacyArtifactValue {
        artifact_file: artifact_file.to_string(),
        value,
    })
}
