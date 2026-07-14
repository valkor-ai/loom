use contracts::{
    DeployProvider, DeploymentFacts, DeploymentLayoutKind, DeploymentRoute, DeploymentRuntime,
    DeploymentRuntimeContract, DeploymentSourceModel, DeploymentTopology, DeploymentTopologyClass,
    RuntimeKind, SourceServiceRole,
};

pub fn build_deployment_facts(
    provider: DeployProvider,
    runtime: &DeploymentRuntimeContract,
    source_model: &DeploymentSourceModel,
    topology: &DeploymentTopology,
    deployment_runtime: &DeploymentRuntime,
) -> DeploymentFacts {
    let topology_class = topology_class(provider, runtime, source_model, topology);
    let layout_kind = layout_kind(provider, source_model);
    let mut stack_kinds = source_model
        .services
        .iter()
        .map(|service| service.runtime_kind)
        .filter(|kind| *kind != RuntimeKind::Unknown)
        .collect::<Vec<_>>();
    stack_kinds.sort_by_key(|kind| format!("{kind:?}"));
    stack_kinds.dedup();

    let mut service_roots = source_model
        .services
        .iter()
        .map(|service| service.root.clone())
        .collect::<Vec<_>>();
    service_roots.sort();
    service_roots.dedup();

    let mut dependency_kinds = source_model
        .dependencies
        .iter()
        .map(|service| service.kind)
        .collect::<Vec<_>>();
    dependency_kinds.sort_by_key(|kind| format!("{kind:?}"));
    dependency_kinds.dedup();

    let public_port_count = deployment_runtime
        .ports
        .iter()
        .filter(|port| !port.internal_only)
        .count() as u32;
    let internal_port_count = deployment_runtime
        .ports
        .iter()
        .filter(|port| port.internal_only)
        .count() as u32;

    DeploymentFacts {
        schema_version: 1,
        topology_class,
        layout_kind,
        public_entry_service_id: topology.public_entry_service_id.clone(),
        primary_service_id: source_model.primary_service_id.clone(),
        service_count: source_model.services.len() as u32,
        stack_kinds,
        service_roots,
        dependency_kinds,
        public_port_count,
        internal_port_count,
        generated_asset_policy: generated_asset_policy(provider).to_string(),
        notes: fact_notes(
            provider,
            topology_class,
            layout_kind,
            runtime,
            source_model,
            topology,
        ),
    }
}

fn topology_class(
    provider: DeployProvider,
    runtime: &DeploymentRuntimeContract,
    source_model: &DeploymentSourceModel,
    topology: &DeploymentTopology,
) -> DeploymentTopologyClass {
    match provider {
        DeployProvider::ComposeExisting => return DeploymentTopologyClass::ExistingCompose,
        DeployProvider::DockerfileExisting => {
            return DeploymentTopologyClass::ExistingDockerfileWrapper;
        }
        DeployProvider::Generated => {}
    }

    let has_proxy = topology
        .routes
        .iter()
        .any(|route| matches!(route, DeploymentRoute::HttpProxy { .. }));
    if source_model.services.len() > 1 {
        if has_proxy
            && public_entry_role(source_model, topology) == Some(SourceServiceRole::Frontend)
        {
            return DeploymentTopologyClass::FrontendGatewayBackendApi;
        }
        return DeploymentTopologyClass::MultiService;
    }

    let Some(service) = source_model.services.first() else {
        return DeploymentTopologyClass::Unknown;
    };
    if service.runtime_kind == RuntimeKind::Static
        || (service.role == SourceServiceRole::Frontend && service.start_command.is_none())
    {
        return DeploymentTopologyClass::StaticSite;
    }
    if backend_serves_frontend(runtime) {
        return DeploymentTopologyClass::BackendServedFrontendApi;
    }
    if !topology.validation.api_probes.is_empty() {
        return DeploymentTopologyClass::ApiOnlySingleService;
    }
    DeploymentTopologyClass::SingleServiceApp
}

fn public_entry_role(
    source_model: &DeploymentSourceModel,
    topology: &DeploymentTopology,
) -> Option<SourceServiceRole> {
    source_model
        .services
        .iter()
        .find(|service| service.service_id == topology.public_entry_service_id)
        .map(|service| service.role)
}

fn backend_serves_frontend(runtime: &DeploymentRuntimeContract) -> bool {
    let Some(frontend) = runtime.frontend.as_ref() else {
        return false;
    };
    [
        frontend.served_by.as_deref(),
        frontend.served_by_ref.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        let normalized = value.to_ascii_lowercase().replace(['_', '-'], "");
        [
            "springbootstatic",
            "backendstatic",
            "serverstatic",
            "servicestatic",
            "appstatic",
            "sameprocess",
            "sameapp",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
    })
}

fn layout_kind(
    provider: DeployProvider,
    source_model: &DeploymentSourceModel,
) -> DeploymentLayoutKind {
    if provider != DeployProvider::Generated {
        return DeploymentLayoutKind::ExistingAssets;
    }
    if source_model.services.iter().any(|service| {
        service.root.starts_with("apps/")
            || service.root.starts_with("services/")
            || service.root.starts_with("packages/")
    }) {
        return DeploymentLayoutKind::WorkspaceApp;
    }
    let has_frontend_root = source_model
        .services
        .iter()
        .any(|service| matches!(service.root.as_str(), "web" | "frontend" | "client" | "ui"));
    let has_backend_root = source_model.services.iter().any(|service| {
        matches!(
            service.root.as_str(),
            "service" | "backend" | "api" | "server"
        )
    });
    if has_frontend_root && has_backend_root {
        return DeploymentLayoutKind::SplitFrontendBackend;
    }
    if source_model.services.len() == 1
        && source_model.services[0].root == "."
        && source_model.services[0].output_directory.is_some()
        && source_model.services[0].runtime_kind != RuntimeKind::Node
    {
        return DeploymentLayoutKind::SameRootFullstack;
    }
    if source_model
        .services
        .iter()
        .all(|service| service.root == ".")
    {
        return DeploymentLayoutKind::RootApp;
    }
    DeploymentLayoutKind::Unknown
}

fn generated_asset_policy(provider: DeployProvider) -> &'static str {
    match provider {
        DeployProvider::ComposeExisting => "reuse_user_compose_read_only",
        DeployProvider::DockerfileExisting => "generate_compose_wrapper_only",
        DeployProvider::Generated => "generate_loom_owned_assets",
    }
}

fn fact_notes(
    provider: DeployProvider,
    topology_class: DeploymentTopologyClass,
    layout_kind: DeploymentLayoutKind,
    _runtime: &DeploymentRuntimeContract,
    source_model: &DeploymentSourceModel,
    _topology: &DeploymentTopology,
) -> Vec<String> {
    let mut notes = vec![format!(
        "Deploy facts were derived by Loom from RuntimeDelivery, repository probes, source model, and topology; agent-submitted topology fields are not authoritative."
    )];
    if provider != DeployProvider::Generated {
        notes.push("User-owned deployment assets remain protected; Loom only wraps or observes them according to provider policy.".to_string());
    }
    if topology_class == DeploymentTopologyClass::FrontendGatewayBackendApi {
        notes.push("Public frontend gateway must own preview traffic and proxy API paths to the internal backend service before SPA fallback.".to_string());
    }
    if topology_class == DeploymentTopologyClass::BackendServedFrontendApi {
        notes.push("A single backend runtime serves frontend assets and API routes; no generated HTTP proxy route is required.".to_string());
    }
    if layout_kind == DeploymentLayoutKind::WorkspaceApp {
        notes.push("Build context and Dockerfile COPY paths must preserve workspace root manifests and the selected app working directory.".to_string());
    }
    if source_model.services.is_empty() {
        notes.push("No deployable source services were modeled.".to_string());
    }
    notes
}
