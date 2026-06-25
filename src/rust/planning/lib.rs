mod paths;
mod pgc;
mod repository_context;
mod technical_baseline;

use delivery_core::{
    ArtifactKind, DomainDispatcher, LoomMcpActionResult, RouteAction, RouteActionKind,
    ValidatedPlanInput,
};

pub use repository_context::accept_repository_context_file;
pub use technical_baseline::accept_technical_baseline_file;

#[derive(Debug, Default, Clone, Copy)]
pub struct PlanningDomainDispatcher;

impl DomainDispatcher for PlanningDomainDispatcher {
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
            RouteActionKind::TechnicalBaselineRequest => {
                technical_baseline::materialize_request(project_root, delivery_id, phase_id)
            }
            RouteActionKind::RepositoryContextRequest => {
                repository_context::materialize_request(project_root, delivery_id, phase_id)
            }
            RouteActionKind::PlanningContractCreate => {
                pgc::create_contract_and_route(project_root, delivery_id, phase_id)
            }
            RouteActionKind::ArchitectureArtifactContract => {
                architecture::ArchitectureDomainDispatcher.dispatch_route_action(
                    project_root,
                    delivery_id,
                    phase_id,
                    action,
                )
            }
            RouteActionKind::TaskplanGeneration | RouteActionKind::ContinueExecution => {
                execution::ExecutionDomainDispatcher.dispatch_route_action(
                    project_root,
                    delivery_id,
                    phase_id,
                    action,
                )
            }
            _ => delivery_core::UnimplementedDomainDispatcher.dispatch_route_action(
                project_root,
                delivery_id,
                phase_id,
                action,
            ),
        }
    }
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
    let write_mode = artifact_write_mode(artifact_kind);
    Ok(LoomMcpActionResult::AutoRunnable(
        delivery_core::LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            delivery_core::LoomMcpNextAction::WriteArtifact(delivery_core::WriteArtifactNext {
                artifact_kind,
                request_ref: request_ref.to_string(),
                write_mode,
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

fn artifact_write_mode(kind: ArtifactKind) -> delivery_core::WriteMode {
    match kind {
        ArtifactKind::TaskPlanCandidate => delivery_core::WriteMode::TaskplanGrouped,
        ArtifactKind::ArchitectureSectionCandidate => delivery_core::WriteMode::ArchitectureSection,
        ArtifactKind::TaskResult
        | ArtifactKind::BrainstormCandidate
        | ArtifactKind::KnowledgeSemanticPackResult
        | ArtifactKind::TechnicalBaselineCandidate
        | ArtifactKind::RepositoryContextCandidate
        | ArtifactKind::ReviewResult
        | ArtifactKind::ManualReviewResolution => delivery_core::WriteMode::SingleJson,
        ArtifactKind::TaskResultRepair
        | ArtifactKind::TaskplanRepair
        | ArtifactKind::ArchitectureArtifactRepair
        | ArtifactKind::DeployExecutionRepairResult => delivery_core::WriteMode::RepairJson,
    }
}

pub fn module_name() -> &'static str {
    "planning"
}
