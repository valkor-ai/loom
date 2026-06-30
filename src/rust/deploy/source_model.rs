use contracts::{
    DeploymentRuntimeContract, DeploymentShape, DeploymentSourceModel, DeploymentSourceService,
    PackageManager, RuntimeKind, SourceModelSource, SourceServiceRole,
};

use crate::code_evidence::DeploymentCodeProbe;

pub fn source_model_from_runtime_contract(
    runtime: &DeploymentRuntimeContract,
    fallback_probe: &DeploymentCodeProbe,
    build_context_path: String,
) -> DeploymentSourceModel {
    let shape = runtime
        .deployment_shape
        .unwrap_or(DeploymentShape::SingleService);
    if runtime.source == "heuristic" {
        return source_model_from_probe(fallback_probe, build_context_path);
    }
    if shape == DeploymentShape::FrontendAndBackend {
        let frontend_root = service_root_from_refs(&[
            runtime
                .frontend
                .as_ref()
                .and_then(|item| item.source_root.as_deref()),
            runtime
                .frontend
                .as_ref()
                .and_then(|item| item.output_dir.as_deref()),
            runtime
                .frontend
                .as_ref()
                .and_then(|item| item.build_command.as_deref()),
            runtime.build_command.as_deref(),
        ]);
        let backend_root = service_root_from_refs(&[
            runtime.api.as_ref().and_then(|item| item.entry.as_deref()),
            runtime
                .api
                .as_ref()
                .and_then(|item| item.build_command.as_deref()),
            runtime.start_command.as_deref(),
            runtime.build_command.as_deref(),
        ]);
        let backend_kind = runtime_kind_from_signals(&[
            runtime.api.as_ref().and_then(|api| api.kind.as_deref()),
            runtime.start_command.as_deref(),
            runtime.runtime_kind.as_deref(),
        ]);
        let frontend_build = runtime
            .frontend
            .as_ref()
            .and_then(|item| item.build_command.clone())
            .or_else(|| runtime.build_command.clone());
        let backend_build = runtime
            .api
            .as_ref()
            .and_then(|item| item.build_command.clone())
            .or_else(|| runtime.build_command.clone());
        let frontend = DeploymentSourceService {
            service_id: "frontend".to_string(),
            role: SourceServiceRole::Frontend,
            root: frontend_root.clone(),
            working_directory: (frontend_root != ".").then_some(frontend_root.clone()),
            workspace_package_json_paths: fallback_probe.workspace_package_json_paths.clone(),
            runtime_kind: RuntimeKind::Node,
            package_manager: package_manager_from_command(frontend_build.as_deref())
                .or(Some(PackageManager::Npm)),
            has_lockfile: fallback_probe.has_lockfile,
            framework: runtime
                .frontend
                .as_ref()
                .and_then(|item| item.kind.clone())
                .or_else(|| Some("frontend".to_string())),
            runtime_version: None,
            runtime_version_source: None,
            build_command: command_for_root(frontend_build, &frontend_root),
            start_command: None,
            output_directory: runtime
                .frontend
                .as_ref()
                .and_then(|item| item.output_dir.clone())
                .or_else(|| runtime.frontend_output_dir.clone())
                .or_else(|| Some("dist".to_string())),
            port: 80,
            healthcheck_path: Some("/".to_string()),
        };
        let backend = DeploymentSourceService {
            service_id: "backend".to_string(),
            role: SourceServiceRole::Backend,
            root: backend_root.clone(),
            working_directory: (backend_root != ".").then_some(backend_root.clone()),
            workspace_package_json_paths: vec![],
            runtime_kind: backend_kind,
            package_manager: package_manager_from_command(
                runtime
                    .api
                    .as_ref()
                    .and_then(|api| api.build_command.as_deref())
                    .or(runtime.start_command.as_deref())
                    .or(runtime.build_command.as_deref()),
            )
            .or_else(|| default_package_manager(backend_kind)),
            has_lockfile: fallback_probe.has_lockfile,
            framework: runtime
                .api
                .as_ref()
                .and_then(|item| item.kind.clone())
                .or_else(|| runtime.runtime_kind.clone()),
            runtime_version: fallback_probe.runtime_version.clone(),
            runtime_version_source: fallback_probe.runtime_version_source.clone(),
            build_command: command_for_root(backend_build, &backend_root),
            start_command: command_for_root(runtime.start_command.clone(), &backend_root),
            output_directory: None,
            port: runtime.port.unwrap_or(8080),
            healthcheck_path: runtime
                .health_path
                .clone()
                .or_else(|| Some(runtime.preview_path.clone())),
        };
        return DeploymentSourceModel {
            schema_version: 1,
            source: SourceModelSource::RuntimeContract,
            shape,
            primary_service_id: backend.service_id.clone(),
            preview_service_id: frontend.service_id.clone(),
            build_context_path,
            services: vec![frontend, backend],
            dependencies: dependencies_from_runtime_or_probe(runtime, fallback_probe),
            notes: vec![
                "Deployment source model was derived from RuntimeDelivery frontend and api services.".to_string(),
            ],
        };
    }

    let contract_kind = runtime_kind_from_signals(&[
        runtime.api.as_ref().and_then(|api| api.kind.as_deref()),
        runtime.runtime_kind.as_deref(),
        runtime.start_command.as_deref(),
    ]);
    let service = DeploymentSourceService {
        service_id: "app".to_string(),
        role: SourceServiceRole::App,
        root: fallback_probe
            .working_directory
            .clone()
            .unwrap_or_else(|| ".".to_string()),
        working_directory: fallback_probe.working_directory.clone(),
        workspace_package_json_paths: fallback_probe.workspace_package_json_paths.clone(),
        runtime_kind: if contract_kind == RuntimeKind::Unknown {
            fallback_probe.kind
        } else {
            contract_kind
        },
        package_manager: runtime
            .build_command
            .as_deref()
            .or(runtime.start_command.as_deref())
            .filter(|command| command_is_usable(command))
            .and_then(|command| package_manager_from_command(Some(command)))
            .or(fallback_probe.package_manager)
            .or_else(|| default_package_manager(fallback_probe.kind)),
        has_lockfile: fallback_probe.has_lockfile,
        framework: runtime
            .runtime_kind
            .clone()
            .or_else(|| fallback_probe.framework.clone()),
        runtime_version: fallback_probe.runtime_version.clone(),
        runtime_version_source: fallback_probe.runtime_version_source.clone(),
        build_command: runtime
            .build_command
            .clone()
            .filter(|command| command_is_usable(command))
            .or_else(|| fallback_probe.build_command.clone()),
        start_command: runtime
            .start_command
            .clone()
            .filter(|command| command_is_usable(command))
            .or_else(|| fallback_probe.start_command.clone()),
        output_directory: runtime
            .frontend_output_dir
            .clone()
            .or_else(|| fallback_probe.output_directory.clone()),
        port: runtime.port.unwrap_or(fallback_probe.port),
        healthcheck_path: runtime
            .health_path
            .clone()
            .or_else(|| fallback_probe.healthcheck_path.clone())
            .or_else(|| Some(runtime.preview_path.clone())),
    };
    DeploymentSourceModel {
        schema_version: 1,
        source: SourceModelSource::RuntimeContract,
        shape,
        primary_service_id: service.service_id.clone(),
        preview_service_id: service.service_id.clone(),
        build_context_path,
        services: vec![service],
        dependencies: dependencies_from_runtime_or_probe(runtime, fallback_probe),
        notes: vec![
            "Deployment source model was derived from RuntimeDelivery single service and repository code evidence.".to_string(),
        ],
    }
}

fn source_model_from_probe(
    probe: &DeploymentCodeProbe,
    build_context_path: String,
) -> DeploymentSourceModel {
    let service = DeploymentSourceService {
        service_id: "app".to_string(),
        role: SourceServiceRole::App,
        root: probe
            .working_directory
            .clone()
            .unwrap_or_else(|| ".".to_string()),
        working_directory: probe.working_directory.clone(),
        workspace_package_json_paths: probe.workspace_package_json_paths.clone(),
        runtime_kind: probe.kind,
        package_manager: probe.package_manager,
        has_lockfile: probe.has_lockfile,
        framework: probe.framework.clone(),
        runtime_version: probe.runtime_version.clone(),
        runtime_version_source: probe.runtime_version_source.clone(),
        build_command: probe.build_command.clone(),
        start_command: probe.start_command.clone(),
        output_directory: probe.output_directory.clone(),
        port: probe.port,
        healthcheck_path: probe.healthcheck_path.clone(),
    };
    DeploymentSourceModel {
        schema_version: 1,
        source: SourceModelSource::CodeProbe,
        shape: DeploymentShape::SingleService,
        primary_service_id: service.service_id.clone(),
        preview_service_id: service.service_id.clone(),
        build_context_path,
        services: vec![service],
        dependencies: probe.services.clone(),
        notes: vec![
            "Deployment source model was derived from repository code evidence.".to_string(),
        ],
    }
}

fn dependencies_from_runtime_or_probe(
    runtime: &DeploymentRuntimeContract,
    probe: &DeploymentCodeProbe,
) -> Vec<contracts::DependencyService> {
    if runtime.dependency_services.is_empty() {
        probe.services.clone()
    } else {
        runtime.dependency_services.clone()
    }
}

fn command_is_usable(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    !(lower.contains("service:") || lower.contains("web:"))
}

fn runtime_kind_from_signals(signals: &[Option<&str>]) -> RuntimeKind {
    let text = signals
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    if text.contains("spring")
        || text.contains("java")
        || text.contains("mvn")
        || text.contains("gradle")
    {
        return RuntimeKind::Java;
    }
    if text.contains("vite")
        || text.contains("react")
        || text.contains("node")
        || text.contains("npm")
    {
        return RuntimeKind::Node;
    }
    if text.contains("fastapi")
        || text.contains("flask")
        || text.contains("django")
        || text.contains("python")
    {
        return RuntimeKind::Python;
    }
    if text.contains("dotnet") || text.contains("aspnet") {
        return RuntimeKind::Dotnet;
    }
    if text.contains("go") || text.contains("golang") {
        return RuntimeKind::Go;
    }
    RuntimeKind::Unknown
}

fn package_manager_from_command(command: Option<&str>) -> Option<PackageManager> {
    let value = command.unwrap_or_default().to_ascii_lowercase();
    if value.contains("pnpm") {
        Some(PackageManager::Pnpm)
    } else if value.contains("yarn") {
        Some(PackageManager::Yarn)
    } else if value.contains("bun") {
        Some(PackageManager::Bun)
    } else if value.contains("npm") {
        Some(PackageManager::Npm)
    } else if value.contains("mvn") {
        Some(PackageManager::Maven)
    } else if value.contains("gradle") {
        Some(PackageManager::Gradle)
    } else if value.contains("poetry") {
        Some(PackageManager::Poetry)
    } else if value.contains("uv ") || value == "uv" {
        Some(PackageManager::Uv)
    } else if value.contains("pip") {
        Some(PackageManager::Pip)
    } else {
        None
    }
}

fn default_package_manager(kind: RuntimeKind) -> Option<PackageManager> {
    match kind {
        RuntimeKind::Node => Some(PackageManager::Npm),
        RuntimeKind::Java => Some(PackageManager::Maven),
        RuntimeKind::Python => Some(PackageManager::Pip),
        RuntimeKind::Go => Some(PackageManager::Go),
        RuntimeKind::Dotnet => Some(PackageManager::Dotnet),
        RuntimeKind::Php => Some(PackageManager::Composer),
        RuntimeKind::Ruby => Some(PackageManager::Bundler),
        RuntimeKind::Static | RuntimeKind::Unknown => None,
    }
}

fn service_root_from_refs(values: &[Option<&str>]) -> String {
    for value in values.iter().flatten() {
        if let Some(root) = prefix_root(value) {
            return root;
        }
        for prefix in ["apps/", "services/", "packages/"] {
            if let Some(index) = value.find(prefix) {
                let tail = &value[index..];
                let root = tail
                    .split(|ch: char| ch.is_whitespace() || ch == ';' || ch == '&' || ch == '|')
                    .next()
                    .unwrap_or(".");
                let mut parts = root.split('/').take(2).collect::<Vec<_>>();
                if parts.len() == 2 {
                    return parts.drain(..).collect::<Vec<_>>().join("/");
                }
            }
        }
    }
    ".".to_string()
}

fn prefix_root(value: &str) -> Option<String> {
    let marker = "--prefix ";
    let index = value.find(marker)?;
    let after = &value[index + marker.len()..];
    after
        .split_whitespace()
        .next()
        .map(|item| item.trim_matches('"').trim_matches('\'').to_string())
}

fn command_for_root(command: Option<String>, root: &str) -> Option<String> {
    let command = command?;
    if root == "." {
        return Some(command);
    }
    Some(
        command
            .replace(&format!("{root}/"), "")
            .replace(&format!("--prefix {root}"), ""),
    )
}
