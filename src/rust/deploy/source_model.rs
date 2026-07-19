use std::collections::BTreeSet;

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
    let declared_shape = runtime
        .deployment_shape
        .unwrap_or(DeploymentShape::SingleService);
    if matches!(
        runtime.authority,
        contracts::DeploymentContractAuthority::RepositoryHeuristic
    ) {
        return source_model_from_probe(fallback_probe, build_context_path);
    }
    let shape = inferred_shape(declared_shape, runtime, fallback_probe);
    if shape == DeploymentShape::FrontendAndBackend {
        let frontend_root = service_root_from_refs(
            &[
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
                    .and_then(|item| item.commands.build.as_deref()),
                runtime.commands.development.build.as_deref(),
                runtime.commands.development.start.as_deref(),
                runtime.commands.deployment.build.as_deref(),
                runtime.commands.deployment.start.as_deref(),
            ],
            &["frontend", "web", "client", "ui"],
        );
        let frontend_root = resolve_frontend_root(frontend_root, fallback_probe);
        let backend_root = service_root_from_refs(
            &[
                runtime.api.as_ref().and_then(|item| item.entry.as_deref()),
                runtime
                    .api
                    .as_ref()
                    .and_then(|item| item.commands.build.as_deref()),
                runtime.commands.development.start.as_deref(),
                runtime.commands.development.build.as_deref(),
                runtime.commands.deployment.start.as_deref(),
                runtime.commands.deployment.build.as_deref(),
            ],
            &["backend", "api", "service", "server"],
        );
        let backend_root = resolve_backend_root(backend_root, fallback_probe);
        let frontend_build = command_for_service(
            runtime
                .frontend
                .as_ref()
                .and_then(|item| item.commands.build.clone())
                .or_else(|| runtime.commands.deployment.build.clone()),
            &frontend_root,
            &["frontend", "web", "client", "ui"],
        );
        let backend_build = command_for_service(
            runtime
                .api
                .as_ref()
                .and_then(|item| item.commands.build.clone())
                .or_else(|| runtime.commands.deployment.build.clone()),
            &backend_root,
            &["backend", "api", "service", "server"],
        );
        let backend_start_candidate = command_for_service(
            runtime.commands.deployment.start.clone(),
            &backend_root,
            &["backend", "api", "service", "server"],
        );
        let declared_backend_kind = runtime_kind_from_signals(&[
            runtime.api.as_ref().and_then(|api| api.kind.as_deref()),
            runtime
                .api
                .as_ref()
                .and_then(|api| api.commands.build.as_deref()),
            backend_build.as_deref(),
            backend_start_candidate.as_deref(),
        ]);
        let backend_kind = if declared_backend_kind == RuntimeKind::Unknown {
            fallback_backend_kind(fallback_probe)
        } else {
            declared_backend_kind
        };
        let backend_start = backend_start_candidate
            .filter(|command| start_command_is_runtime_safe(backend_kind, command));
        let frontend_package_manager =
            package_manager_from_command(frontend_build.as_deref()).or(Some(PackageManager::Npm));
        let frontend_lockfile_refs =
            node_lockfile_refs(fallback_probe, &frontend_root, frontend_package_manager);
        let backend_lockfile_refs = node_lockfile_refs(
            fallback_probe,
            &backend_root,
            package_manager_from_command(backend_build.as_deref())
                .or_else(|| package_manager_from_command(backend_start.as_deref())),
        );
        let frontend = DeploymentSourceService {
            service_id: "frontend".to_string(),
            role: SourceServiceRole::Frontend,
            root: frontend_root.clone(),
            working_directory: (frontend_root != ".").then_some(frontend_root.clone()),
            workspace_package_json_paths: fallback_probe.workspace_package_json_paths.clone(),
            manifest_refs: node_manifest_refs(fallback_probe, &frontend_root),
            lockfile_refs: frontend_lockfile_refs.clone(),
            artifact_refs: vec![runtime
                .frontend
                .as_ref()
                .and_then(|item| item.output_dir.clone())
                .or_else(|| runtime.frontend_output_dir.clone())
                .unwrap_or_else(|| default_frontend_output_dir(&frontend_root))],
            runtime_kind: RuntimeKind::Node,
            package_manager: frontend_package_manager,
            has_lockfile: !frontend_lockfile_refs.is_empty(),
            framework: runtime
                .frontend
                .as_ref()
                .and_then(|item| item.kind.clone())
                .or_else(|| {
                    frontend_framework_from_signals(&[
                        frontend_build.as_deref(),
                        runtime.runtime_kind.as_deref(),
                    ])
                })
                .or_else(|| Some("frontend".to_string())),
            runtime_version: None,
            runtime_version_source: None,
            build_command: frontend_build,
            start_command: None,
            output_directory: runtime
                .frontend
                .as_ref()
                .and_then(|item| item.output_dir.clone())
                .or_else(|| runtime.frontend_output_dir.clone())
                .or_else(|| Some(default_frontend_output_dir(&frontend_root))),
            port: 80,
            healthcheck_path: None,
        };
        let backend = DeploymentSourceService {
            service_id: "backend".to_string(),
            role: SourceServiceRole::Backend,
            root: backend_root.clone(),
            working_directory: (backend_root != ".").then_some(backend_root.clone()),
            workspace_package_json_paths: vec![],
            manifest_refs: backend_manifest_refs(
                fallback_probe,
                &backend_root,
                backend_kind,
                backend_build.as_deref(),
            ),
            lockfile_refs: if backend_kind == RuntimeKind::Node {
                backend_lockfile_refs.clone()
            } else {
                vec![]
            },
            artifact_refs: backend_artifact_refs(&backend_root, backend_kind),
            runtime_kind: backend_kind,
            package_manager: package_manager_from_command(backend_build.as_deref())
                .or_else(|| package_manager_from_command(backend_start.as_deref()))
                .or_else(|| fallback_package_manager_for_kind(fallback_probe, backend_kind))
                .or_else(|| default_package_manager(backend_kind)),
            has_lockfile: backend_kind == RuntimeKind::Node && !backend_lockfile_refs.is_empty(),
            framework: runtime
                .api
                .as_ref()
                .and_then(|item| item.kind.clone())
                .or_else(|| {
                    normalized_framework_from_signals(&[
                        backend_build.as_deref(),
                        backend_start.as_deref(),
                        runtime.api.as_ref().and_then(|api| api.kind.as_deref()),
                    ])
                })
                .or_else(|| fallback_framework_for_kind(fallback_probe, backend_kind)),
            runtime_version: fallback_probe.runtime_version.clone(),
            runtime_version_source: fallback_probe.runtime_version_source.clone(),
            build_command: backend_build,
            start_command: backend_start,
            output_directory: None,
            port: runtime.port.unwrap_or(8080),
            healthcheck_path: backend_healthcheck_path(runtime, fallback_probe),
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

    let contract_kind = if runtime_contract_declares_multi_root(runtime)
        && fallback_probe.kind != RuntimeKind::Unknown
    {
        RuntimeKind::Unknown
    } else {
        runtime_kind_from_signals(&[
            runtime.api.as_ref().and_then(|api| api.kind.as_deref()),
            runtime.runtime_kind.as_deref(),
            runtime.commands.deployment.start.as_deref(),
        ])
    };
    let service_runtime_kind = if contract_kind == RuntimeKind::Unknown {
        fallback_probe.kind
    } else {
        contract_kind
    };
    let service = DeploymentSourceService {
        service_id: "app".to_string(),
        role: if service_runtime_kind == RuntimeKind::Static {
            SourceServiceRole::Frontend
        } else {
            SourceServiceRole::App
        },
        root: fallback_probe
            .working_directory
            .clone()
            .unwrap_or_else(|| ".".to_string()),
        working_directory: fallback_probe.working_directory.clone(),
        workspace_package_json_paths: fallback_probe.workspace_package_json_paths.clone(),
        manifest_refs: probe_manifest_refs(fallback_probe),
        lockfile_refs: probe_lockfile_refs(fallback_probe),
        artifact_refs: probe_artifact_refs(fallback_probe),
        runtime_kind: service_runtime_kind,
        package_manager: runtime
            .commands
            .deployment
            .build
            .as_deref()
            .or(runtime.commands.deployment.start.as_deref())
            .filter(|command| command_is_usable(command))
            .and_then(|command| package_manager_from_command(Some(command)))
            .or(fallback_probe.package_manager)
            .or_else(|| default_package_manager(fallback_probe.kind)),
        has_lockfile: fallback_probe.has_lockfile,
        framework: fallback_probe.framework.clone().or_else(|| {
            runtime
                .runtime_kind
                .as_deref()
                .and_then(normalized_framework_label)
        }),
        runtime_version: fallback_probe.runtime_version.clone(),
        runtime_version_source: fallback_probe.runtime_version_source.clone(),
        build_command: runtime
            .commands
            .deployment
            .build
            .clone()
            .filter(|command| command_is_usable(command))
            .or_else(|| fallback_probe.build_command.clone()),
        start_command: runtime
            .commands
            .deployment
            .start
            .clone()
            .filter(|command| command_is_usable(command))
            .or_else(|| fallback_probe.start_command.clone()),
        output_directory: runtime
            .frontend_output_dir
            .clone()
            .or_else(|| fallback_probe.output_directory.clone()),
        port: runtime.port.unwrap_or(fallback_probe.port),
        healthcheck_path: runtime.health_path.clone(),
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

fn inferred_shape(
    declared: DeploymentShape,
    runtime: &DeploymentRuntimeContract,
    probe: &DeploymentCodeProbe,
) -> DeploymentShape {
    if declared == DeploymentShape::FrontendAndBackend {
        return declared;
    }
    let has_frontend_code =
        !probe.workspace_package_json_paths.is_empty() || probe.output_directory.is_some();
    let has_api_contract = runtime
        .api_contract
        .as_ref()
        .is_some_and(|contract| !contract.interfaces.is_empty());
    let integrated_frontend = runtime
        .frontend
        .as_ref()
        .and_then(|frontend| {
            frontend
                .served_by
                .as_deref()
                .or(frontend.served_by_ref.as_deref())
        })
        .is_some_and(|value| {
            let value = value.to_ascii_lowercase().replace(['-', '_'], "");
            value.contains("sameprocess")
                || value.contains("sameapp")
                || value.contains("backendstatic")
                || value.contains("springbootstatic")
        });
    if has_frontend_code && has_api_contract && !integrated_frontend {
        DeploymentShape::FrontendAndBackend
    } else {
        declared
    }
}

fn source_model_from_probe(
    probe: &DeploymentCodeProbe,
    build_context_path: String,
) -> DeploymentSourceModel {
    let service = DeploymentSourceService {
        service_id: "app".to_string(),
        role: if probe.kind == RuntimeKind::Static {
            SourceServiceRole::Frontend
        } else {
            SourceServiceRole::App
        },
        root: probe
            .working_directory
            .clone()
            .unwrap_or_else(|| ".".to_string()),
        working_directory: probe.working_directory.clone(),
        workspace_package_json_paths: probe.workspace_package_json_paths.clone(),
        manifest_refs: probe_manifest_refs(probe),
        lockfile_refs: probe_lockfile_refs(probe),
        artifact_refs: probe_artifact_refs(probe),
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

fn frontend_root_from_probe(probe: &DeploymentCodeProbe) -> Option<String> {
    if probe
        .workspace_package_json_paths
        .iter()
        .any(|path| path == "package.json")
    {
        return Some(".".to_string());
    }
    probe
        .workspace_package_json_paths
        .iter()
        .filter_map(|path| path.rsplit_once('/').map(|(root, _)| root.to_string()))
        .find(|root| !root.is_empty())
        .or_else(|| {
            probe.output_directory.as_ref().and_then(|output| {
                if !output.contains('/') && !output.is_empty() {
                    return Some(".".to_string());
                }
                output
                    .split_once('/')
                    .map(|(root, _)| root.to_string())
                    .filter(|root| !root.is_empty())
            })
        })
}

fn backend_root_from_probe(probe: &DeploymentCodeProbe) -> Option<String> {
    probe
        .build_command
        .as_deref()
        .and_then(|command| {
            service_root_from_cd_segments(command, &["backend", "api", "service", "server"])
        })
        .or_else(|| probe.working_directory.clone().filter(|root| root != "."))
        .or_else(|| {
            matches!(
                probe.kind,
                RuntimeKind::Java
                    | RuntimeKind::Python
                    | RuntimeKind::Go
                    | RuntimeKind::Dotnet
                    | RuntimeKind::Php
                    | RuntimeKind::Ruby
            )
            .then(|| ".".to_string())
        })
}

fn resolve_frontend_root(candidate: String, probe: &DeploymentCodeProbe) -> String {
    if frontend_root_matches_probe(&candidate, probe) {
        return candidate;
    }
    frontend_root_from_probe(probe).unwrap_or(candidate)
}

fn resolve_backend_root(candidate: String, probe: &DeploymentCodeProbe) -> String {
    if candidate == "." {
        return backend_root_from_probe(probe).unwrap_or(candidate);
    }
    if backend_root_from_probe(probe).as_deref() == Some(candidate.as_str()) {
        return candidate;
    }
    backend_root_from_probe(probe).unwrap_or(candidate)
}

fn frontend_root_matches_probe(root: &str, probe: &DeploymentCodeProbe) -> bool {
    let manifest = join_root(root, "package.json");
    probe
        .workspace_package_json_paths
        .iter()
        .any(|path| path == &manifest)
}

fn node_manifest_refs(probe: &DeploymentCodeProbe, root: &str) -> Vec<String> {
    let path = join_root(root, "package.json");
    evidence_has_file_path(probe, &path)
        .then_some(path)
        .into_iter()
        .collect()
}

fn node_lockfile_refs(
    probe: &DeploymentCodeProbe,
    root: &str,
    _package_manager: Option<PackageManager>,
) -> Vec<String> {
    node_workspace_file_refs(
        probe,
        root,
        &[
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lockb",
        ],
    )
}

fn backend_manifest_refs(
    probe: &DeploymentCodeProbe,
    root: &str,
    kind: RuntimeKind,
    build_command: Option<&str>,
) -> Vec<String> {
    let candidates: Vec<String> = match kind {
        RuntimeKind::Java => {
            if package_manager_from_command(build_command) == Some(PackageManager::Gradle) {
                vec!["build.gradle".to_string(), "build.gradle.kts".to_string()]
            } else {
                vec!["pom.xml".to_string()]
            }
        }
        RuntimeKind::Dotnet => dotnet_manifest_candidates(probe, root),
        RuntimeKind::Python => vec![
            "pyproject.toml".to_string(),
            "requirements.txt".to_string(),
            "Pipfile".to_string(),
        ],
        RuntimeKind::Go => vec!["go.mod".to_string()],
        RuntimeKind::Node => return node_manifest_refs(probe, root),
        RuntimeKind::Php => vec!["composer.json".to_string()],
        RuntimeKind::Ruby => vec!["Gemfile".to_string()],
        _ => vec![],
    };
    candidates
        .into_iter()
        .map(|relative| join_root(root, &relative))
        .filter(|path| evidence_has_file_path(probe, path))
        .collect()
}

fn dotnet_manifest_candidates(probe: &DeploymentCodeProbe, root: &str) -> Vec<String> {
    let prefix = if root == "." {
        String::new()
    } else {
        format!("{}/", root.trim_matches('/'))
    };
    probe
        .evidence
        .get("files")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|file| file.get("path").and_then(serde_json::Value::as_str))
        .filter(|path| path.starts_with(&prefix) && path.ends_with(".csproj"))
        .filter_map(|path| path.strip_prefix(&prefix).map(str::to_string))
        .collect()
}

fn backend_artifact_refs(root: &str, kind: RuntimeKind) -> Vec<String> {
    match kind {
        RuntimeKind::Java => vec![
            join_root(root, "target/*.jar"),
            join_root(root, "build/libs/*.jar"),
        ],
        RuntimeKind::Dotnet => vec![join_root(root, "bin/Release/*/publish/*.dll")],
        RuntimeKind::Go => vec![join_root(root, "bin/*")],
        RuntimeKind::Node | RuntimeKind::Python | RuntimeKind::Php | RuntimeKind::Ruby => vec![],
        RuntimeKind::Static | RuntimeKind::Unknown => vec![],
    }
}

fn probe_manifest_refs(probe: &DeploymentCodeProbe) -> Vec<String> {
    let root = probe.working_directory.as_deref().unwrap_or(".");
    match probe.kind {
        RuntimeKind::Node => node_manifest_refs(probe, root),
        RuntimeKind::Java => backend_manifest_refs(
            probe,
            root,
            RuntimeKind::Java,
            probe.build_command.as_deref(),
        ),
        RuntimeKind::Python => backend_manifest_refs(
            probe,
            root,
            RuntimeKind::Python,
            probe.build_command.as_deref(),
        ),
        RuntimeKind::Go => {
            backend_manifest_refs(probe, root, RuntimeKind::Go, probe.build_command.as_deref())
        }
        RuntimeKind::Dotnet => backend_manifest_refs(
            probe,
            root,
            RuntimeKind::Dotnet,
            probe.build_command.as_deref(),
        ),
        RuntimeKind::Php | RuntimeKind::Ruby => {
            backend_manifest_refs(probe, root, probe.kind, probe.build_command.as_deref())
        }
        RuntimeKind::Static | RuntimeKind::Unknown => vec![],
    }
}

fn probe_lockfile_refs(probe: &DeploymentCodeProbe) -> Vec<String> {
    let root = probe.working_directory.as_deref().unwrap_or(".");
    match probe.kind {
        RuntimeKind::Node => node_lockfile_refs(probe, root, probe.package_manager),
        RuntimeKind::Python
            if probe.has_lockfile && probe.package_manager == Some(PackageManager::Poetry) =>
        {
            vec![join_root(root, "poetry.lock")]
        }
        RuntimeKind::Php
            if probe.has_lockfile && probe.package_manager == Some(PackageManager::Composer) =>
        {
            vec![join_root(root, "composer.lock")]
        }
        RuntimeKind::Ruby
            if probe.has_lockfile && probe.package_manager == Some(PackageManager::Bundler) =>
        {
            vec![join_root(root, "Gemfile.lock")]
        }
        _ => vec![],
    }
}

fn probe_artifact_refs(probe: &DeploymentCodeProbe) -> Vec<String> {
    let root = probe.working_directory.as_deref().unwrap_or(".");
    let mut refs = backend_artifact_refs(root, probe.kind);
    if let Some(output) = &probe.output_directory {
        refs.push(output.clone());
    }
    refs.sort();
    refs.dedup();
    refs
}

fn join_root(root: &str, path: &str) -> String {
    if root == "." || root.is_empty() {
        path.to_string()
    } else {
        format!("{}/{}", root.trim_matches('/'), path)
    }
}

fn fallback_package_manager_for_kind(
    probe: &DeploymentCodeProbe,
    kind: RuntimeKind,
) -> Option<PackageManager> {
    (probe.kind == kind)
        .then_some(probe.package_manager)
        .flatten()
}

fn fallback_framework_for_kind(probe: &DeploymentCodeProbe, kind: RuntimeKind) -> Option<String> {
    (probe.kind == kind)
        .then(|| probe.framework.clone())
        .flatten()
}

fn fallback_backend_kind(probe: &DeploymentCodeProbe) -> RuntimeKind {
    match probe.kind {
        RuntimeKind::Node | RuntimeKind::Static | RuntimeKind::Unknown => RuntimeKind::Unknown,
        kind => kind,
    }
}

fn normalized_framework_from_signals(signals: &[Option<&str>]) -> Option<String> {
    signals
        .iter()
        .flatten()
        .find_map(|signal| normalized_framework_label(signal))
}

fn frontend_framework_from_signals(signals: &[Option<&str>]) -> Option<String> {
    let text = signals
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    if text.contains("next") {
        Some("nextjs".to_string())
    } else if text.contains("vite") {
        Some("vite".to_string())
    } else if text.contains("react") {
        Some("react".to_string())
    } else if text.contains("vue") {
        Some("vue".to_string())
    } else if text.contains("svelte") {
        Some("svelte".to_string())
    } else {
        None
    }
}

fn node_workspace_file_refs(
    probe: &DeploymentCodeProbe,
    root: &str,
    file_names: &[&str],
) -> Vec<String> {
    let root_prefix = if root == "." || root.is_empty() {
        String::new()
    } else {
        format!("{}/", root.trim_matches('/'))
    };
    let package_roots = probe
        .workspace_package_json_paths
        .iter()
        .filter_map(|path| {
            if path == "package.json" {
                Some("")
            } else {
                path.strip_suffix("/package.json")
            }
        })
        .filter(|package_root| {
            root == "."
                || *package_root == root.trim_matches('/')
                || package_root.starts_with(&root_prefix)
        })
        .map(|package_root| {
            if package_root.is_empty() {
                ".".to_string()
            } else {
                package_root.to_string()
            }
        })
        .collect::<BTreeSet<_>>();
    let mut refs = package_roots
        .into_iter()
        .flat_map(|package_root| {
            file_names
                .iter()
                .map(move |name| join_root(&package_root, name))
        })
        .filter(|path| evidence_has_file_path(probe, path))
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn evidence_has_file_path(probe: &DeploymentCodeProbe, relative: &str) -> bool {
    probe
        .evidence
        .get("files")
        .and_then(serde_json::Value::as_array)
        .map(|files| {
            files.iter().any(|file| {
                file.get("path")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|path| path == relative)
            })
        })
        .unwrap_or(false)
}

fn backend_healthcheck_path(
    runtime: &DeploymentRuntimeContract,
    _fallback_probe: &DeploymentCodeProbe,
) -> Option<String> {
    runtime
        .health_path
        .clone()
        .or_else(|| preferred_api_probe_path(runtime))
}

fn preferred_api_probe_path(runtime: &DeploymentRuntimeContract) -> Option<String> {
    runtime
        .safe_http_probes
        .iter()
        .find(|probe| probe.path.to_ascii_lowercase().contains("health"))
        .map(|probe| probe.path.clone())
}

fn command_is_usable(command: &str) -> bool {
    labeled_command_segments(command).is_empty()
}

pub fn runtime_contract_declares_multi_root(runtime: &DeploymentRuntimeContract) -> bool {
    if runtime.deployment_shape == Some(DeploymentShape::FrontendAndBackend) {
        return true;
    }
    let mut labels = Vec::new();
    for command in [
        runtime.commands.development.build.as_deref(),
        runtime.commands.development.start.as_deref(),
        runtime.commands.verification.build.as_deref(),
        runtime.commands.verification.start.as_deref(),
        runtime.commands.deployment.build.as_deref(),
        runtime.commands.deployment.start.as_deref(),
        runtime
            .frontend
            .as_ref()
            .and_then(|endpoint| endpoint.commands.build.as_deref()),
        runtime
            .api
            .as_ref()
            .and_then(|api| api.commands.build.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        labels.extend(labeled_command_segments(command));
    }
    labels.sort();
    labels.dedup();
    labels.len() >= 2
}

fn labeled_command_segments(command: &str) -> Vec<String> {
    let mut labels = command
        .split(';')
        .filter_map(command_label)
        .collect::<Vec<_>>();
    labels.extend(
        command
            .split(';')
            .flat_map(|part| part.split("&&"))
            .flat_map(|part| part.split("||"))
            .filter_map(command_label),
    );
    labels.sort();
    labels.dedup();
    labels
}

fn command_label(part: &str) -> Option<String> {
    let trimmed = part.trim();
    let (label, rest) = trimmed.split_once(':')?;
    let label = label.trim();
    if rest.trim().is_empty() || !is_command_segment_label(label) {
        return None;
    }
    Some(label.to_ascii_lowercase())
}

fn is_command_segment_label(value: &str) -> bool {
    if value.is_empty() || value.contains(char::is_whitespace) {
        return false;
    }
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
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
    if text.contains("fastapi")
        || text.contains("flask")
        || text.contains("django")
        || text.contains("python")
        || text.contains("uvicorn")
        || text.contains("gunicorn")
        || text.contains("manage.py")
    {
        return RuntimeKind::Python;
    }
    if text.contains("vite")
        || text.contains("react")
        || text.contains("node")
        || text.contains("npm")
    {
        return RuntimeKind::Node;
    }
    if text.contains("dotnet") || text.contains("aspnet") {
        return RuntimeKind::Dotnet;
    }
    if text.contains("go") || text.contains("golang") {
        return RuntimeKind::Go;
    }
    if text.contains("php") || text.contains("laravel") || text.contains("symfony") {
        return RuntimeKind::Php;
    }
    if text.contains("ruby") || text.contains("rails") || text.contains("rack") {
        return RuntimeKind::Ruby;
    }
    if text.contains("static") || text.contains("nginx") {
        return RuntimeKind::Static;
    }
    RuntimeKind::Unknown
}

fn normalized_framework_label(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("spring") {
        Some("spring-boot".to_string())
    } else if lower.contains("fastapi") {
        Some("fastapi".to_string())
    } else if lower.contains("django") {
        Some("django".to_string())
    } else if lower.contains("flask") {
        Some("flask".to_string())
    } else if lower.contains("aspnet") || lower.contains("asp.net") {
        Some("aspnet".to_string())
    } else if lower.contains("laravel") {
        Some("laravel".to_string())
    } else if lower.contains("symfony") {
        Some("symfony".to_string())
    } else if lower.contains("rails") {
        Some("rails".to_string())
    } else if lower.contains("sinatra") {
        Some("sinatra".to_string())
    } else if lower.contains("express") {
        Some("express".to_string())
    } else if lower.contains("next") {
        Some("nextjs".to_string())
    } else if lower.contains("vite") {
        Some("vite".to_string())
    } else {
        None
    }
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
    } else if value.contains("composer") {
        Some(PackageManager::Composer)
    } else if value.contains("bundle") {
        Some(PackageManager::Bundler)
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

fn service_root_from_refs(values: &[Option<&str>], preferred_labels: &[&str]) -> String {
    for value in values.iter().flatten() {
        if let Some(root) = preferred_labeled_root(value, preferred_labels) {
            return root;
        }
        if let Some(root) = service_root_from_cd_segments(value, preferred_labels) {
            return root;
        }
        if let Some(root) = prefix_root(value) {
            return root;
        }
        if let Some(root) = service_root_from_path_ref(value, preferred_labels) {
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

fn service_root_from_cd_segments(command: &str, preferred_labels: &[&str]) -> Option<String> {
    command_cd_segments(command)
        .into_iter()
        .map(|segment| segment.root)
        .find(|root| {
            let aliases = root_aliases(root);
            preferred_labels
                .iter()
                .any(|label| aliases.iter().any(|alias| alias == label))
        })
}

fn preferred_labeled_root(value: &str, preferred_labels: &[&str]) -> Option<String> {
    let labels = labeled_command_segments(value);
    preferred_labels
        .iter()
        .find(|label| labels.iter().any(|candidate| candidate == **label))
        .map(|label| label.to_string())
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

fn service_root_from_path_ref(value: &str, preferred_labels: &[&str]) -> Option<String> {
    for token in value
        .split(|ch: char| ch.is_whitespace() || ch == ';' || ch == '&' || ch == '|')
        .map(|item| {
            item.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim_start_matches("./")
        })
        .filter(|item| {
            !item.is_empty()
                && !item.starts_with('-')
                && !item.starts_with('/')
                && !item.contains("://")
                && item.contains('/')
        })
    {
        let parts = token.split('/').collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        if matches!(parts[0], "apps" | "services" | "packages") && parts.len() >= 3 {
            let aliases = root_aliases(parts[1]);
            if preferred_labels
                .iter()
                .any(|label| aliases.iter().any(|alias| alias == label))
            {
                return Some(format!("{}/{}", parts[0], parts[1]));
            }
        }
        let aliases = root_aliases(parts[0]);
        if preferred_labels
            .iter()
            .any(|label| aliases.iter().any(|alias| alias == label))
        {
            return Some(parts[0].to_string());
        }
    }
    None
}

fn command_for_service(
    command: Option<String>,
    root: &str,
    role_labels: &[&str],
) -> Option<String> {
    let command = command?;
    if let Some(segment) = labeled_command_for_labels(&command, role_labels) {
        return Some(normalize_command_for_root(&segment, root));
    }
    if let Some(segment) = labeled_command_for_root(&command, root) {
        return Some(normalize_command_for_root(&segment, root));
    }
    if let Some(segment) = cd_command_for_root(&command, root) {
        return Some(normalize_command_for_root(&segment, root));
    }
    if root == "." && labeled_command_segments(&command).is_empty() {
        return Some(command);
    }
    if root == "." {
        return None;
    }
    Some(normalize_command_for_root(&command, root))
}

fn normalize_command_for_root(command: &str, root: &str) -> String {
    if root == "." {
        return command.trim().to_string();
    }
    let mut output = command
        .trim()
        .replace(&format!("{root}/"), "")
        .replace(&format!("--prefix {root}"), "")
        .replace(&format!("--prefix={root}"), "")
        .to_string();
    for prefix in [
        format!("cd {root} &&"),
        format!("cd ./{root} &&"),
        format!("cd {root};"),
        format!("cd ./{root};"),
    ] {
        if let Some(rest) = output.strip_prefix(&prefix) {
            output = rest.trim().to_string();
            break;
        }
    }
    while output.contains("  ") {
        output = output.replace("  ", " ");
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CdCommandSegment {
    root: String,
    command: String,
}

fn cd_command_for_root(command: &str, root: &str) -> Option<String> {
    let aliases = root_aliases(root);
    let commands = command_cd_segments(command)
        .into_iter()
        .filter(|segment| {
            let segment_aliases = root_aliases(&segment.root);
            aliases
                .iter()
                .any(|alias| segment_aliases.iter().any(|candidate| candidate == alias))
        })
        .map(|segment| segment.command)
        .filter(|segment| !segment.trim().is_empty())
        .collect::<Vec<_>>();
    (!commands.is_empty()).then(|| commands.join(" && "))
}

fn command_cd_segments(command: &str) -> Vec<CdCommandSegment> {
    let mut current_root = ".".to_string();
    let mut output = Vec::new();
    for raw in split_shell_sequence(command) {
        let segment = raw.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(target) = segment.strip_prefix("cd ").and_then(first_shell_word) {
            current_root = normalize_cd_target(&current_root, &target);
            continue;
        }
        output.push(CdCommandSegment {
            root: current_root.clone(),
            command: segment.to_string(),
        });
    }
    output
}

fn split_shell_sequence(command: &str) -> Vec<&str> {
    command
        .split(';')
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .collect()
}

fn first_shell_word(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .next()
        .map(|word| word.trim_matches('"').trim_matches('\'').to_string())
        .filter(|word| !word.is_empty())
}

fn normalize_cd_target(current_root: &str, target: &str) -> String {
    let mut parts = if target.starts_with('/') || current_root == "." {
        Vec::new()
    } else {
        current_root
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>()
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                let _ = parts.pop();
            }
            item => parts.push(item),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn labeled_command_for_root(command: &str, root: &str) -> Option<String> {
    let aliases = root_aliases(root);
    labeled_command_for_labels(
        command,
        &aliases.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn labeled_command_for_labels(command: &str, labels: &[&str]) -> Option<String> {
    labeled_command_from_parts(command.split(';'), labels).or_else(|| {
        labeled_command_from_parts(
            command
                .split(';')
                .flat_map(|part| part.split("&&"))
                .flat_map(|part| part.split("||")),
            labels,
        )
    })
}

fn labeled_command_from_parts<'a>(
    parts: impl Iterator<Item = &'a str>,
    labels: &[&str],
) -> Option<String> {
    parts
        .filter_map(|part| {
            let trimmed = part.trim();
            let (label, rest) = trimmed.split_once(':')?;
            let label = label.trim().to_ascii_lowercase();
            if !labels.iter().any(|alias| *alias == label) {
                return None;
            }
            let rest = rest.trim();
            (!rest.is_empty()).then(|| rest.to_string())
        })
        .next()
}

fn root_aliases(root: &str) -> Vec<String> {
    let root = root.trim_matches('/').to_ascii_lowercase();
    let mut aliases = vec![root.clone()];
    match root.as_str() {
        "frontend" | "web" | "client" | "ui" => {
            aliases.extend(["frontend", "web", "client", "ui"].map(str::to_string));
        }
        "backend" | "api" | "service" | "server" => {
            aliases.extend(["backend", "api", "service", "server"].map(str::to_string));
        }
        _ => {}
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn default_frontend_output_dir(frontend_root: &str) -> String {
    if frontend_root == "." {
        "dist".to_string()
    } else {
        format!("{frontend_root}/dist")
    }
}

fn start_command_is_runtime_safe(kind: RuntimeKind, command: &str) -> bool {
    if kind == RuntimeKind::Java {
        let lower = command.to_ascii_lowercase();
        return !(lower.contains("spring-boot:run")
            || lower.contains("bootrun")
            || lower.contains("./mvnw")
            || lower.contains(" mvn")
            || lower.starts_with("mvn")
            || lower.contains("./gradlew")
            || lower.contains(" gradle")
            || lower.starts_with("gradle"));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn node_source_facts_include_lockfiles_for_each_workspace_package_root() {
        let probe = DeploymentCodeProbe {
            kind: RuntimeKind::Node,
            workspace_package_json_paths: vec![
                "package.json".to_string(),
                "apps/admin/package.json".to_string(),
                "packages/shared/package.json".to_string(),
            ],
            evidence: json!({
                "files": [
                    {"path": "package.json"},
                    {"path": "package-lock.json"},
                    {"path": "apps/admin/package.json"},
                    {"path": "apps/admin/pnpm-lock.yaml"},
                    {"path": "packages/shared/package.json"},
                    {"path": "packages/shared/yarn.lock"}
                ]
            }),
            ..DeploymentCodeProbe::unknown()
        };

        assert_eq!(
            node_lockfile_refs(&probe, ".", Some(PackageManager::Npm)),
            vec![
                "apps/admin/pnpm-lock.yaml",
                "package-lock.json",
                "packages/shared/yarn.lock"
            ]
        );
    }
}
