use std::path::PathBuf;

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{paths::to_project_relative, store::path_exists};

use crate::{
    active_operation::{active_operation_result, live_operation},
    paths::deployment_paths,
    prepare::{deployment_file_refs, read_spec},
    DeployToolInput,
};

pub fn deploy_inspect(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root_buf = PathBuf::from(&input.project_root);
    let project_root = project_root_buf.as_path();
    let project_root_display = input.project_root.clone();
    if let Ok(Some(operation)) = live_operation(project_root) {
        return active_operation_result(project_root, operation);
    }
    let paths = deployment_paths(project_root);
    let spec = read_spec(project_root).ok();
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root_display,
        summary: "Deployment inspect loaded.".to_string(),
        details: Some(json!({
            "prepared": spec.is_some(),
            "specRef": spec.as_ref().and_then(|_| to_project_relative(project_root, &paths.spec_file).ok()),
            "runtimeContractRef": spec.as_ref().map(|spec| &spec.runtime_contract_ref),
            "sourceModelRef": spec.as_ref().map(|spec| &spec.source_model_ref),
            "topologyRef": spec.as_ref().map(|spec| &spec.topology_ref),
            "codeEvidenceRef": spec.as_ref().map(|spec| &spec.code_evidence_ref),
            "sourceModelSummary": spec.as_ref().map(|spec| json!({
                "shape": spec.source_model.shape,
                "primaryServiceId": spec.source_model.primary_service_id,
                "previewServiceId": spec.source_model.preview_service_id,
                "serviceIds": spec.source_model.services.iter().map(|service| service.service_id.clone()).collect::<Vec<_>>()
            })),
            "topologySummary": spec.as_ref().map(|spec| json!({
                "publicEntryServiceId": spec.topology.public_entry_service_id,
                "routeCount": spec.topology.routes.len(),
                "previewPaths": spec.topology.validation.preview_paths,
                "apiPaths": spec.topology.validation.api_paths
            })),
            "generatedFileRefs": spec.as_ref().map(deployment_file_refs).unwrap_or_default(),
            "reusedFileRefs": spec.as_ref().map(|spec| spec.files.reused.clone()).unwrap_or_default(),
            "stateRef": path_exists(&paths.state_file).then(|| to_project_relative(project_root, &paths.state_file).ok()).flatten(),
            "repairRef": path_exists(&paths.repair_action_file).then(|| to_project_relative(project_root, &paths.repair_action_file).ok()).flatten(),
        })),
        warnings: vec![],
    })
}
