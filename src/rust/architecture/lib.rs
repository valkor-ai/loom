mod paths;
mod request;
mod submit;

use contracts::ArchitectureSectionGroup;
use delivery_core::{
    ArtifactKind, DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind,
    ValidatedPlanInput,
};

pub use submit::{accept_architecture_repair_file, accept_architecture_section_file};

#[derive(Debug, Default, Clone, Copy)]
pub struct ArchitectureDomainDispatcher;

impl DomainDispatcher for ArchitectureDomainDispatcher {
    fn start_brainstorm(&self, input: &ValidatedPlanInput) -> LoomMcpActionResult {
        delivery_core::UnimplementedDomainDispatcher.start_brainstorm(input)
    }

    fn dispatch_route_action(
        &self,
        project_root: &str,
        delivery_id: &str,
        phase_id: &str,
        action: &RouteAction,
    ) -> LoomMcpActionResult {
        match action.kind {
            RouteActionKind::ArchitectureArtifactContract => match action.request_ref.as_deref() {
                Some(request_ref) => {
                    match state::inspect_request(delivery_core::InspectRequestInput {
                        project_root: project_root.to_string(),
                        request_ref: request_ref.to_string(),
                    }) {
                        Ok(request) if request.request_kind == "architecture_artifact_repair" => {
                            write_artifact_result(
                                project_root,
                                request_ref,
                                ArtifactKind::ArchitectureArtifactRepair,
                            )
                            .unwrap_or_else(|error| {
                                delivery_core::LoomMcpActionResult::Failed(
                                    delivery_core::LoomMcpFailureResult {
                                        project_root: project_root.to_string(),
                                        error: delivery_core::LoomMcpFailure {
                                            code: "ARCHITECTURE_REPAIR_RESUME_FAILED".to_string(),
                                            message: error.to_string(),
                                            target_batch: Some(9),
                                            domain: Some("architecture".to_string()),
                                            route_action: Some(
                                                "architecture_artifact_repair".to_string(),
                                            ),
                                            recovery_tool: Some("loom.continue".to_string()),
                                        },
                                    },
                                )
                            })
                        }
                        Ok(request)
                            if request.request_kind == "architecture_sections_generation" =>
                        {
                            write_artifact_result(
                                project_root,
                                request_ref,
                                ArtifactKind::ArchitectureSectionCandidate,
                            )
                            .unwrap_or_else(|error| {
                                delivery_core::LoomMcpActionResult::Failed(
                                    delivery_core::LoomMcpFailureResult {
                                        project_root: project_root.to_string(),
                                        error: delivery_core::LoomMcpFailure {
                                            code: "ARCHITECTURE_RESUME_FAILED".to_string(),
                                            message: error.to_string(),
                                            target_batch: Some(8),
                                            domain: Some("architecture".to_string()),
                                            route_action: Some(
                                                "architecture_artifact_contract".to_string(),
                                            ),
                                            recovery_tool: Some("loom.continue".to_string()),
                                        },
                                    },
                                )
                            })
                        }
                        _ => request::materialize_request(project_root, delivery_id, phase_id),
                    }
                }
                None => request::materialize_request(project_root, delivery_id, phase_id),
            },
            _ => delivery_core::UnimplementedDomainDispatcher.dispatch_route_action(
                project_root,
                delivery_id,
                phase_id,
                action,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct SectionOutput {
    section: ArchitectureSectionGroup,
    candidate_file: String,
    schema_ref: String,
    schema_shape: serde_json::Value,
    result_template: serde_json::Value,
    enum_refs: serde_json::Value,
    generation_rules: Vec<String>,
}

fn write_artifact_result(
    project_root: &str,
    request_ref: &str,
    artifact_kind: ArtifactKind,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })?;
    let submit_tool = inspected.submit_tool.ok_or_else(|| {
        state::store::StateError::InvalidArgument(format!(
            "request {} is missing outputContract.submitTool",
            inspected.request_id
        ))
    })?;
    let write_targets = inspected
        .write_targets
        .iter()
        .map(value_to_write_target)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LoomMcpActionResult::AutoRunnable(
        delivery_core::LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            delivery_core::LoomMcpNextAction::WriteArtifact(delivery_core::WriteArtifactNext {
                artifact_kind,
                request_ref: request_ref.to_string(),
                write_mode: delivery_core::WriteMode::ArchitectureSection,
                write_targets,
                read_groups: inspected.read_groups,
                submit_tool,
            }),
        ),
    ))
}

fn value_to_write_target(
    value: &serde_json::Value,
) -> Result<delivery_core::WriteTarget, state::store::StateError> {
    Ok(delivery_core::WriteTarget {
        target_id: value
            .get("targetId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                state::store::StateError::InvalidArgument(
                    "write target is missing targetId".to_string(),
                )
            })?
            .to_string(),
        path: value
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                state::store::StateError::InvalidArgument(
                    "write target is missing path".to_string(),
                )
            })?
            .to_string(),
        required: value
            .get("required")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        description: value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Write the requested artifact.")
            .to_string(),
    })
}

pub fn module_name() -> &'static str {
    "architecture"
}
