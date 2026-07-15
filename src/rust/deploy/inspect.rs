use std::path::PathBuf;

use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{paths::to_project_relative, store::path_exists};

use crate::{
    active_operation::{active_operation_result, live_operation},
    paths::deployment_paths,
    prepare::{deployment_generated_file_refs, read_spec},
    references::reference_profile_value,
    repair::latest_repair_summary,
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
            "provider": spec.as_ref().map(|spec| spec.provider),
            "providerReason": spec.as_ref().map(|spec| spec.provider_reason.clone()),
            "providerCandidates": spec.as_ref().map(|spec| spec.provider_candidates.iter().map(|candidate| json!({
                "provider": candidate.provider,
                "status": candidate.status,
                "reason": candidate.reason.clone()
            })).collect::<Vec<_>>()).unwrap_or_default(),
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
                "apiProbes": spec.topology.validation.api_probes
            })),
            "frontendApiBinding": spec.as_ref().map(|spec| spec.frontend_api_binding.clone()),
            "composeSummary": spec.as_ref().and_then(|spec| spec.compose.as_ref().map(|compose| json!({
                "selectedService": compose.selected_service.clone(),
                "serviceReason": compose.service_reason.clone(),
                "serviceCount": compose.services.len(),
                "warnings": compose.warnings.clone()
            }))),
            "generatedFileRefs": spec.as_ref().map(deployment_generated_file_refs).unwrap_or_default(),
            "reusedFileRefs": spec.as_ref().map(|spec| spec.files.reused.clone()).unwrap_or_default(),
            "deployReferenceProfile": spec.as_ref().map(|spec| reference_profile_value(spec, None, false)),
            "stateRef": path_exists(&paths.state_file).then(|| to_project_relative(project_root, &paths.state_file).ok()).flatten(),
            "repairRef": path_exists(&paths.repair_action_file).then(|| to_project_relative(project_root, &paths.repair_action_file).ok()).flatten(),
            "repairSummary": latest_repair_summary(project_root),
        })),
        warnings: vec![],
    })
}
