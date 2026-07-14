use std::{collections::BTreeSet, path::Path};

use contracts::ArchitectureArtifactContract;
use serde_json::{json, Value};
use state::{paths::from_project_relative, store::StateResult};

pub(crate) fn load_project_api_contract(
    project_root: &Path,
    architecture: &ArchitectureArtifactContract,
) -> StateResult<Option<Value>> {
    let Some(contract_ref) = architecture.api_contract_ref.as_deref() else {
        return Ok(None);
    };
    let path = from_project_relative(project_root, contract_ref)?;
    state::store::read_json_value(&path).map(Some)
}

pub(crate) fn interfaces_for_refs(contract: Option<&Value>, refs: &[String]) -> Vec<Value> {
    if refs.is_empty() {
        return vec![];
    }
    let refs = refs.iter().map(String::as_str).collect::<BTreeSet<_>>();
    contract
        .and_then(|value| value.get("interfaces"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|interface| {
            interface
                .get("interfaceId")
                .and_then(Value::as_str)
                .is_some_and(|interface_id| refs.contains(interface_id))
        })
        .cloned()
        .collect()
}

pub(crate) fn exposure_projection(contract_ref: Option<&str>, contract: Option<&Value>) -> Value {
    let Some(contract) = contract else {
        return Value::Null;
    };
    json!({
        "apiContractRef": contract_ref,
        "publicExposure": contract.get("publicExposure").cloned().unwrap_or(Value::Null),
        "browserBinding": contract.get("browserBinding").cloned().unwrap_or(Value::Null)
    })
}
