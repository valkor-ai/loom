use contracts::{DeployProvider, DeploymentFailureKind, DeploymentSpec, RuntimeKind};
use delivery_core::DeployReferenceProfile;
use serde_json::json;

pub(crate) fn reference_profile_value(
    spec: &DeploymentSpec,
    failure_kind: Option<DeploymentFailureKind>,
    repair: bool,
) -> serde_json::Value {
    let profile = reference_profile(spec, failure_kind, repair);
    json!({
        "referenceIds": profile.reference_ids,
        "loadMode": profile.load_mode,
    })
}

pub(crate) fn reference_profile(
    spec: &DeploymentSpec,
    failure_kind: Option<DeploymentFailureKind>,
    repair: bool,
) -> DeployReferenceProfile {
    DeployReferenceProfile {
        reference_ids: reference_ids(spec, failure_kind, repair),
        load_mode: "skill_reference_by_id".to_string(),
    }
}

fn reference_ids(
    spec: &DeploymentSpec,
    failure_kind: Option<DeploymentFailureKind>,
    repair: bool,
) -> Vec<String> {
    let mut ids = Vec::new();
    push(&mut ids, "deploy.providers");
    push(&mut ids, "deploy.compose");

    if repair {
        push(&mut ids, "deploy.repair");
    }

    if matches!(
        spec.provider,
        DeployProvider::Generated | DeployProvider::DockerfileExisting
    ) {
        push(&mut ids, "deploy.dockerfile");
    }

    if !spec.environment.required.is_empty()
        || !spec.environment.generated.is_empty()
        || !spec.environment.missing.is_empty()
    {
        push(&mut ids, "deploy.environment");
    }

    if !spec.bootstrap.tasks.is_empty() {
        push(&mut ids, "deploy.bootstrap");
    }

    if is_workspace_or_multi_root(spec) {
        push(&mut ids, "deploy.workspaces");
    }

    for service in &spec.source_model.services {
        if let Some(reference_id) = stack_reference_id(service.runtime_kind) {
            push(&mut ids, reference_id);
        }
    }

    if let Some(kind) = failure_kind {
        add_failure_reference_ids(&mut ids, kind);
    }

    ids
}

fn add_failure_reference_ids(ids: &mut Vec<String>, kind: DeploymentFailureKind) {
    match kind {
        DeploymentFailureKind::ComposeConfig
        | DeploymentFailureKind::Healthcheck
        | DeploymentFailureKind::PreviewNotVerified
        | DeploymentFailureKind::ApiRouteNotVerified
        | DeploymentFailureKind::DeployAssetInvalid => push(ids, "deploy.compose"),
        DeploymentFailureKind::ImageBuild | DeploymentFailureKind::ContainerStart => {
            push(ids, "deploy.dockerfile");
            push(ids, "deploy.compose");
        }
        DeploymentFailureKind::RuntimeContractMissing
        | DeploymentFailureKind::RuntimeContractNotApplicable
        | DeploymentFailureKind::RuntimeContractMismatch => push(ids, "deploy.workspaces"),
        DeploymentFailureKind::BuildCommandFailed
        | DeploymentFailureKind::StartCommandFailed
        | DeploymentFailureKind::ApplicationStartupFailed
        | DeploymentFailureKind::HttpProbeFailed => {
            push(ids, "deploy.environment");
            push(ids, "deploy.bootstrap");
        }
        DeploymentFailureKind::DockerUnavailable
        | DeploymentFailureKind::RegistryNetwork
        | DeploymentFailureKind::Logs
        | DeploymentFailureKind::Unknown => {}
    }
}

fn is_workspace_or_multi_root(spec: &DeploymentSpec) -> bool {
    spec.source_model.build_context_path != "."
        || spec.source_model.services.len() > 1
        || spec.source_model.services.iter().any(|service| {
            service.root != "."
                || service.working_directory.is_some()
                || !service.workspace_package_json_paths.is_empty()
        })
}

fn stack_reference_id(kind: RuntimeKind) -> Option<&'static str> {
    match kind {
        RuntimeKind::Node => Some("deploy.stacks.node"),
        RuntimeKind::Python => Some("deploy.stacks.python"),
        RuntimeKind::Go => Some("deploy.stacks.go"),
        RuntimeKind::Java => Some("deploy.stacks.java"),
        RuntimeKind::Dotnet => Some("deploy.stacks.dotnet"),
        RuntimeKind::Php => Some("deploy.stacks.php"),
        RuntimeKind::Ruby => Some("deploy.stacks.ruby"),
        RuntimeKind::Static => Some("deploy.stacks.static"),
        RuntimeKind::Unknown => None,
    }
}

fn push(ids: &mut Vec<String>, reference_id: &str) {
    if !ids.iter().any(|id| id == reference_id) {
        ids.push(reference_id.to_string());
    }
}
