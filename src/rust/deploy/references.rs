use contracts::{
    DependencyServiceKind, DeployProvider, DeploymentFailureKind, DeploymentSpec,
    DeploymentTopologyClass, RuntimeKind,
};
use delivery_core::{DeployReferenceProfile, ReferenceLoadPlanItem};
use serde_json::json;

pub(crate) fn reference_profile_value(
    spec: &DeploymentSpec,
    failure_kind: Option<DeploymentFailureKind>,
    repair: bool,
) -> serde_json::Value {
    let profile = reference_profile(spec, failure_kind, repair);
    json!({
        "loadMode": profile.load_mode,
        "referenceLoadPlan": profile.reference_load_plan,
    })
}

pub(crate) fn reference_profile(
    spec: &DeploymentSpec,
    failure_kind: Option<DeploymentFailureKind>,
    repair: bool,
) -> DeployReferenceProfile {
    DeployReferenceProfile {
        load_mode: "mcp_reference_load_plan".to_string(),
        reference_load_plan: reference_load_plan(spec, failure_kind, repair),
    }
}

pub(crate) fn repair_only_reference_profile() -> DeployReferenceProfile {
    DeployReferenceProfile {
        load_mode: "mcp_reference_load_plan".to_string(),
        reference_load_plan: vec![reference_load_plan_item("deploy.repair")],
    }
}

fn reference_load_plan(
    spec: &DeploymentSpec,
    failure_kind: Option<DeploymentFailureKind>,
    repair: bool,
) -> Vec<ReferenceLoadPlanItem> {
    let mut ids = Vec::new();
    push(&mut ids, "deploy.providers");
    push(&mut ids, "deploy.matrix");
    push(&mut ids, "deploy.source-model");
    push(&mut ids, "deploy.compose");
    if topology_reference_needed(spec) {
        push(&mut ids, "deploy.topology");
    }

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
    if spec
        .source_model
        .dependencies
        .iter()
        .any(|dependency| dependency.kind == DependencyServiceKind::Redis)
    {
        push(&mut ids, "deploy.dependencies.redis");
    }

    if let Some(kind) = failure_kind {
        add_failure_references(&mut ids, kind);
    }

    ids.into_iter()
        .map(|reference_id| reference_load_plan_item(&reference_id))
        .collect()
}

fn reference_load_plan_item(reference_id: &str) -> ReferenceLoadPlanItem {
    let (path, reason) = reference_metadata(reference_id);
    ReferenceLoadPlanItem {
        ref_id: reference_id.to_string(),
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

fn reference_metadata(reference_id: &str) -> (&'static str, &'static str) {
    match reference_id {
        "deploy.providers" => (
            "providers.md",
            "Provider selection and generated/existing asset policy.",
        ),
        "deploy.matrix" => (
            "matrix.md",
            "Deployment topology, runtime, layout, port, and dependency matrix.",
        ),
        "deploy.source-model" => (
            "source-model.md",
            "Repository evidence to deployable service model guidance.",
        ),
        "deploy.topology" => (
            "topology.md",
            "Public entry, proxy route, and validation topology guidance.",
        ),
        "deploy.compose" => (
            "compose.md",
            "Compose service wiring, ports, dependencies, and health guidance.",
        ),
        "deploy.dockerfile" => (
            "dockerfile.md",
            "Dockerfile context, workdir, copy, build, and runtime guidance.",
        ),
        "deploy.environment" => (
            "environment.md",
            "Environment, local defaults, dependency URL, and state guidance.",
        ),
        "deploy.workspaces" => (
            "workspaces.md",
            "Workspace app path, source root, and build context guidance.",
        ),
        "deploy.bootstrap" => (
            "bootstrap.md",
            "Migration/bootstrap diagnostics and approval boundary guidance.",
        ),
        "deploy.repair" => (
            "repair.md",
            "Deploy repair decision tree and editable asset boundary.",
        ),
        "deploy.dependencies.redis" => (
            "redis.md",
            "Redis dependency capabilities, persistence, health, and generated asset boundary.",
        ),
        "deploy.stacks.node" => (
            "node.md",
            "Node-family scanner, generated asset, and repair guidance.",
        ),
        "deploy.stacks.python" => (
            "python.md",
            "Python scanner, generated asset, and repair guidance.",
        ),
        "deploy.stacks.go" => ("go.md", "Go scanner, generated asset, and repair guidance."),
        "deploy.stacks.java" => (
            "java.md",
            "Java scanner, generated asset, and repair guidance.",
        ),
        "deploy.stacks.dotnet" => (
            "dotnet.md",
            ".NET scanner, generated asset, and repair guidance.",
        ),
        "deploy.stacks.php" => (
            "php.md",
            "PHP scanner, generated asset, and repair guidance.",
        ),
        "deploy.stacks.ruby" => (
            "ruby.md",
            "Ruby scanner, generated asset, and repair guidance.",
        ),
        "deploy.stacks.static" => (
            "static.md",
            "Static site scanner, generated asset, and repair guidance.",
        ),
        _ => ("providers.md", "Deploy reference selected by MCP."),
    }
}

fn topology_reference_needed(spec: &DeploymentSpec) -> bool {
    matches!(
        spec.facts.topology_class,
        DeploymentTopologyClass::FrontendGatewayBackendApi
            | DeploymentTopologyClass::BackendServedFrontendApi
            | DeploymentTopologyClass::MultiService
            | DeploymentTopologyClass::ExistingCompose
            | DeploymentTopologyClass::ExistingDockerfileWrapper
    ) || !spec.topology.routes.is_empty()
        || !spec.topology.validation.api_probes.is_empty()
        || spec.source_model.services.len() > 1
}

fn add_failure_references(ids: &mut Vec<String>, kind: DeploymentFailureKind) {
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
