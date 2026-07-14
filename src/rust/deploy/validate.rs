use std::{
    io::{Read, Write},
    net::TcpStream,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use contracts::{
    DeployProvider, DeploymentRoute, DeploymentSourceService, DeploymentSpec,
    DeploymentTopologyClass, RuntimeKind, SourceServiceRole,
};
use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult, LoomMcpFailure, LoomMcpFailureResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use state::{
    paths::from_project_relative,
    store::{read_text, StateResult},
};

use crate::{
    port_plan::primary_public_port, prepare::read_spec, runtime_state::write_success_state,
    DeployToolInput,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentValidationResult {
    pub valid: bool,
    pub compose_valid: bool,
    pub preview: Vec<HttpProbeResult>,
    pub api_routes: Vec<HttpProbeResult>,
    pub asset_issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpProbeResult {
    pub path: String,
    pub status: String,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    pub html_fallback: bool,
}

pub fn deploy_validate(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root = Path::new(&input.project_root);
    match deploy_validate_inner(project_root) {
        Ok(result) => {
            let mut details = json!(result);
            if result.valid {
                match read_spec(project_root)
                    .and_then(|spec| write_success_state(project_root, &spec, &result))
                {
                    Ok(state_ref) => {
                        if let Some(object) = details.as_object_mut() {
                            object.insert("stateRef".to_string(), json!(state_ref));
                        }
                    }
                    Err(error) => {
                        return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                            project_root: input.project_root,
                            error: LoomMcpFailure {
                                code: "DEPLOY_VALIDATE_STATE_WRITE_FAILED".to_string(),
                                message: error.to_string(),
                                target_batch: Some(10),
                                domain: Some("deploy".to_string()),
                                route_action: None,
                                recovery_tool: Some("loom.deployInspect".to_string()),
                            },
                        })
                    }
                }
            }
            LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: input.project_root,
                summary: if result.valid {
                    "Deployment validation passed.".to_string()
                } else {
                    "Deployment validation found issues.".to_string()
                },
                details: Some(details),
                warnings: vec![],
            })
        }
        Err(error) => LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: input.project_root,
            summary: "Deployment validation could not run because deploy is not prepared."
                .to_string(),
            details: Some(json!({ "valid": false, "error": error.to_string() })),
            warnings: vec![error.to_string()],
        }),
    }
}

pub fn deploy_validate_inner(project_root: &Path) -> StateResult<DeploymentValidationResult> {
    let spec = read_spec(project_root)?;
    let asset_issues = validate_generated_assets(project_root, &spec)?;
    let public_port = primary_public_port(&spec.runtime);
    let preview = spec
        .topology
        .validation
        .preview_paths
        .iter()
        .map(|path| match public_port {
            Some(port) => probe_http(port, path, false),
            None => missing_public_port_probe(path),
        })
        .collect::<Vec<_>>();
    let api_routes = spec
        .topology
        .validation
        .api_probes
        .iter()
        .map(|probe| match public_port {
            Some(port) => probe_http(port, &probe.path, true),
            None => missing_public_port_probe(&probe.path),
        })
        .collect::<Vec<_>>();
    let compose_valid = asset_issues.iter().all(|issue| !issue.contains("compose"));
    let http_valid = preview.iter().all(|probe| probe.status == "ok")
        && api_routes
            .iter()
            .all(|probe| probe.status == "ok" && !probe.html_fallback);
    Ok(DeploymentValidationResult {
        valid: asset_issues.is_empty() && http_valid,
        compose_valid,
        preview,
        api_routes,
        asset_issues,
    })
}

fn missing_public_port_probe(path: &str) -> HttpProbeResult {
    HttpProbeResult {
        path: if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        },
        status: "invalid".to_string(),
        status_code: None,
        error: Some("Deployment runtime has no public host port.".to_string()),
        html_fallback: false,
    }
}

pub fn validate_generated_assets(
    project_root: &Path,
    spec: &DeploymentSpec,
) -> StateResult<Vec<String>> {
    let mut issues = Vec::new();
    let compose_file = from_project_relative(project_root, &spec.files.compose_path)?;
    let compose = read_text(&compose_file)?;
    if !compose.contains("services:") {
        issues.push("compose file must include services.".to_string());
    }
    let compose_dir = compose_file.parent().unwrap_or(project_root);
    let build_specs = compose_build_specs(&compose);
    for build in &build_specs {
        let context_dir = normalize_path(compose_dir.join(&build.context));
        let dockerfile_path = normalize_path(context_dir.join(&build.dockerfile));
        if !dockerfile_path.exists() {
            issues.push(format!(
                "compose dockerfile path {} for service {} does not resolve from build context {}.",
                build.dockerfile, build.service_id, build.context
            ));
            continue;
        }
        let dockerfile = read_text(&dockerfile_path)?;
        for source in dockerfile_copy_sources(&dockerfile) {
            if source_is_dynamic_or_external(&source) {
                continue;
            }
            let source_path = normalize_path(context_dir.join(&source));
            if !source_path.exists() {
                issues.push(format!(
                    "dockerfile for service {} copies missing source {} from build context {}.",
                    build.service_id, source, build.context
                ));
            }
        }
    }
    if spec.provider != DeployProvider::ComposeExisting {
        for service in &spec.source_model.services {
            if !compose.contains(&format!("  {}:", service.service_id)) {
                issues.push(format!(
                    "compose is missing service {}.",
                    service.service_id
                ));
            }
            match build_specs
                .iter()
                .find(|build| build.service_id == service.service_id)
            {
                Some(build) => {
                    let context_dir = normalize_path(compose_dir.join(&build.context));
                    let dockerfile_path = normalize_path(context_dir.join(&build.dockerfile));
                    if dockerfile_path.exists() {
                        let dockerfile = read_text(&dockerfile_path)?;
                        validate_service_asset_graph(
                            &mut issues,
                            service,
                            &context_dir,
                            &dockerfile,
                            spec,
                        );
                        if let Some(ignore_ref) =
                            spec.files.dockerignore_paths.get(&service.service_id)
                        {
                            let ignore_path = from_project_relative(project_root, ignore_ref)?;
                            if ignore_path.exists() {
                                let ignore = read_text(&ignore_path)?;
                                validate_dockerignore_inputs(&mut issues, service, &ignore);
                            }
                        }
                    }
                }
                None => issues.push(format!(
                    "compose is missing build config for service {}.",
                    service.service_id
                )),
            }
        }
    }
    for (service_id, nginx_ref) in &spec.files.nginx_config_paths {
        let nginx = read_text(&from_project_relative(project_root, nginx_ref)?)?;
        let spa_index = nginx.find("try_files $uri $uri/ /index.html");
        for route in &spec.topology.routes {
            if let DeploymentRoute::HttpProxy {
                public_path,
                target_service_id,
                target_port,
                preserve_path,
            } = route
            {
                let proxy_pass = if *preserve_path {
                    format!("http://{target_service_id}:{target_port};")
                } else {
                    format!("http://{target_service_id}:{target_port}/;")
                };
                let proxy_index = nginx.find(&format!("proxy_pass {proxy_pass}"));
                if proxy_index.is_none() {
                    issues.push(format!("nginx config for {service_id} is missing proxy to {target_service_id}:{target_port}."));
                }
                if proxy_index
                    .zip(spa_index)
                    .map(|(proxy, spa)| proxy > spa)
                    .unwrap_or(false)
                {
                    issues.push(format!(
                        "nginx config for {service_id} places API proxy after SPA fallback."
                    ));
                }
                if !nginx.contains(&format!("location {}", normalize_nginx_path(public_path))) {
                    issues.push(format!(
                        "nginx config for {service_id} is missing public API route {public_path}."
                    ));
                }
            }
        }
    }
    if !spec.topology.validation.api_probes.is_empty()
        && !spec
            .topology
            .routes
            .iter()
            .any(|route| matches!(route, DeploymentRoute::HttpProxy { .. }))
        && topology_requires_api_proxy(spec)
    {
        issues.push("topology has API probes but no http-proxy route.".to_string());
    }
    validate_deployment_facts(&mut issues, spec);
    Ok(issues)
}

fn validate_deployment_facts(issues: &mut Vec<String>, spec: &DeploymentSpec) {
    if !spec
        .source_model
        .services
        .iter()
        .any(|service| service.service_id == spec.topology.public_entry_service_id)
    {
        issues.push(format!(
            "topology publicEntryServiceId {} is not present in sourceModel services.",
            spec.topology.public_entry_service_id
        ));
    }
    for route in &spec.topology.routes {
        if let DeploymentRoute::HttpProxy {
            target_service_id, ..
        } = route
        {
            if !spec
                .source_model
                .services
                .iter()
                .any(|service| &service.service_id == target_service_id)
            {
                issues.push(format!(
                    "topology http-proxy target service {target_service_id} is not present in sourceModel services."
                ));
            }
        }
    }
    match spec.facts.topology_class {
        DeploymentTopologyClass::FrontendGatewayBackendApi => {
            if !spec
                .topology
                .routes
                .iter()
                .any(|route| matches!(route, DeploymentRoute::HttpProxy { .. }))
            {
                issues.push(
                    "deploy facts classify this as frontend_gateway_backend_api but topology has no http-proxy route."
                        .to_string(),
                );
            }
        }
        DeploymentTopologyClass::SingleServiceApp
        | DeploymentTopologyClass::BackendServedFrontendApi
        | DeploymentTopologyClass::ApiOnlySingleService => {
            if spec.source_model.services.len() != 1 {
                issues.push(format!(
                    "deploy facts classify this as {:?} but sourceModel has {} services.",
                    spec.facts.topology_class,
                    spec.source_model.services.len()
                ));
            }
        }
        DeploymentTopologyClass::StaticSite => {
            if !spec.topology.validation.api_probes.is_empty() {
                issues.push(
                    "deploy facts classify this as static_site but topology still validates API paths."
                        .to_string(),
                );
            }
        }
        DeploymentTopologyClass::ExistingCompose
            if spec.provider != DeployProvider::ComposeExisting =>
        {
            issues.push("deploy facts classify this as existing_compose but provider is not compose-existing.".to_string());
        }
        DeploymentTopologyClass::ExistingDockerfileWrapper
            if spec.provider != DeployProvider::DockerfileExisting =>
        {
            issues.push("deploy facts classify this as existing_dockerfile_wrapper but provider is not dockerfile-existing.".to_string());
        }
        DeploymentTopologyClass::MultiService
        | DeploymentTopologyClass::ExistingCompose
        | DeploymentTopologyClass::ExistingDockerfileWrapper
        | DeploymentTopologyClass::Unknown => {}
    }
    let public_ports = spec
        .runtime
        .ports
        .iter()
        .filter(|port| !port.internal_only)
        .count() as u32;
    if public_ports != spec.facts.public_port_count {
        issues.push(format!(
            "deploy facts publicPortCount {} does not match runtime public ports {}.",
            spec.facts.public_port_count, public_ports
        ));
    }
}

fn topology_requires_api_proxy(spec: &DeploymentSpec) -> bool {
    let public_entry = &spec.topology.public_entry_service_id;
    let public_service = spec
        .source_model
        .services
        .iter()
        .find(|service| &service.service_id == public_entry);
    let has_backend_service = spec
        .source_model
        .services
        .iter()
        .any(|service| service.role == SourceServiceRole::Backend);
    public_service
        .map(|service| service.role == SourceServiceRole::Frontend && has_backend_service)
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeBuildSpec {
    service_id: String,
    context: String,
    dockerfile: String,
}

fn compose_build_specs(compose: &str) -> Vec<ComposeBuildSpec> {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(compose) else {
        return Vec::new();
    };
    let Some(root) = value.as_mapping() else {
        return Vec::new();
    };
    let Some(services) = yaml_get(root, "services").and_then(serde_yaml::Value::as_mapping) else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|(service_id, service)| {
            let service_id = service_id.as_str()?.to_string();
            let service = service.as_mapping()?;
            let build = yaml_get(service, "build")?;
            if let Some(context) = build.as_str() {
                return Some(ComposeBuildSpec {
                    service_id,
                    context: context.to_string(),
                    dockerfile: "Dockerfile".to_string(),
                });
            }
            let build = build.as_mapping()?;
            Some(ComposeBuildSpec {
                service_id,
                context: yaml_string_field(build, "context").unwrap_or_else(|| ".".to_string()),
                dockerfile: yaml_string_field(build, "dockerfile")
                    .unwrap_or_else(|| "Dockerfile".to_string()),
            })
        })
        .collect()
}

fn validate_service_asset_graph(
    issues: &mut Vec<String>,
    service: &DeploymentSourceService,
    context_dir: &Path,
    dockerfile: &str,
    spec: &DeploymentSpec,
) {
    if service.root != "." && !normalize_path(context_dir.join(&service.root)).exists() {
        issues.push(format!(
            "sourceModel service {} root {} does not exist inside build context.",
            service.service_id, service.root
        ));
    }
    for manifest in &service.manifest_refs {
        if manifest.contains('*') {
            continue;
        }
        if !normalize_path(context_dir.join(manifest)).exists() {
            if manifest_ref_satisfied_by_alternative(service, context_dir, manifest) {
                continue;
            }
            issues.push(format!(
                "sourceModel service {} manifestRef {} does not exist inside build context.",
                service.service_id, manifest
            ));
        }
    }
    for lockfile in &service.lockfile_refs {
        if lockfile.contains('*') {
            continue;
        }
        if !normalize_path(context_dir.join(lockfile)).exists() {
            issues.push(format!(
                "sourceModel service {} lockfileRef {} does not exist inside build context.",
                service.service_id, lockfile
            ));
        }
    }
    if service.root != "." {
        let workspace_workdir = format!("WORKDIR /workspace/{}", service.root);
        let app_workdir = format!("WORKDIR /app/{}", service.root);
        let src_workdir = format!("WORKDIR /src/{}", service.root);
        if !dockerfile.contains(&workspace_workdir)
            && !dockerfile.contains(&app_workdir)
            && !dockerfile.contains(&src_workdir)
            && spec.provider == DeployProvider::Generated
        {
            issues.push(format!(
                "dockerfile for service {} does not set WORKDIR to service root {}.",
                service.service_id, service.root
            ));
        }
    }
    validate_install_lockfile_command(issues, service, context_dir, dockerfile);
    validate_artifact_closure(issues, service, dockerfile);
    validate_port_closure(issues, service, dockerfile);
}

fn manifest_ref_satisfied_by_alternative(
    service: &DeploymentSourceService,
    context_dir: &Path,
    manifest: &str,
) -> bool {
    if service.runtime_kind != RuntimeKind::Python {
        return false;
    }
    let root = manifest
        .rsplit_once('/')
        .map(|(root, _)| root)
        .unwrap_or("");
    ["requirements.txt", "pyproject.toml", "Pipfile"]
        .iter()
        .any(|name| {
            let candidate = if root.is_empty() {
                (*name).to_string()
            } else {
                format!("{root}/{name}")
            };
            normalize_path(context_dir.join(candidate)).exists()
        })
}

fn validate_install_lockfile_command(
    issues: &mut Vec<String>,
    service: &DeploymentSourceService,
    context_dir: &Path,
    dockerfile: &str,
) {
    for (command, expected) in [
        ("npm ci", "package-lock.json"),
        ("pnpm install --frozen-lockfile", "pnpm-lock.yaml"),
        ("yarn install --frozen-lockfile", "yarn.lock"),
        ("bun install --frozen-lockfile", "bun.lockb"),
    ] {
        if !dockerfile.contains(command) {
            continue;
        }
        let expected_path = if service.root == "." {
            expected.to_string()
        } else {
            format!("{}/{}", service.root, expected)
        };
        if !normalize_path(context_dir.join(&expected_path)).exists() {
            issues.push(format!(
                "dockerfile for service {} uses `{command}` but {} is missing.",
                service.service_id, expected_path
            ));
        }
    }
}

fn validate_artifact_closure(
    issues: &mut Vec<String>,
    service: &DeploymentSourceService,
    dockerfile: &str,
) {
    if service.role == SourceServiceRole::Frontend
        && service.start_command.is_none()
        && service.runtime_kind != RuntimeKind::Static
    {
        let expected_output = service
            .output_directory
            .as_deref()
            .unwrap_or(if service.root == "." { "dist" } else { "" });
        if !expected_output.is_empty()
            && !dockerfile.contains(&format!("/workspace/{expected_output}"))
        {
            issues.push(format!(
                "dockerfile for service {} does not copy declared frontend outputDirectory {}.",
                service.service_id, expected_output
            ));
        }
    }
    if service.runtime_kind == RuntimeKind::Java
        && !dockerfile.contains("find target")
        && !dockerfile.contains("find build/libs")
        && !dockerfile.contains("find . -type f -name '*.jar'")
    {
        issues.push(format!(
            "dockerfile for Java service {} does not declare a runnable jar discovery step.",
            service.service_id
        ));
    }
}

fn validate_port_closure(
    issues: &mut Vec<String>,
    service: &DeploymentSourceService,
    dockerfile: &str,
) {
    let expose = format!("EXPOSE {}", service.port);
    if service.role != SourceServiceRole::Frontend && !dockerfile.contains(&expose) {
        issues.push(format!(
            "dockerfile for service {} does not expose sourceModel port {}.",
            service.service_id, service.port
        ));
    }
}

fn validate_dockerignore_inputs(
    issues: &mut Vec<String>,
    service: &DeploymentSourceService,
    dockerignore: &str,
) {
    let required = service
        .manifest_refs
        .iter()
        .chain(&service.lockfile_refs)
        .collect::<Vec<_>>();
    for path in required {
        if dockerignore_excludes_path(dockerignore, path) {
            issues.push(format!(
                "dockerignore for service {} excludes required build input {}.",
                service.service_id, path
            ));
        }
    }
}

fn dockerignore_excludes_path(dockerignore: &str, path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path);
    dockerignore
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
        .any(|line| line == path || line == basename)
}

fn dockerfile_copy_sources(dockerfile: &str) -> Vec<String> {
    dockerfile
        .lines()
        .map(str::trim)
        .filter(|line| dockerfile_copy_instruction(line).is_some())
        .filter(|line| !line.contains("--from="))
        .flat_map(copy_sources_from_line)
        .collect()
}

fn copy_sources_from_line(line: &str) -> Vec<String> {
    let Some(rest) = dockerfile_copy_instruction(line) else {
        return vec![];
    };
    let rest = strip_copy_options(rest);
    if rest.starts_with('[') {
        let json_text = rest.split_once('#').map(|(value, _)| value).unwrap_or(rest);
        if let Ok(items) = serde_json::from_str::<Vec<String>>(json_text.trim()) {
            let source_count = items.len().saturating_sub(1);
            return items.into_iter().take(source_count).collect::<Vec<_>>();
        }
    }
    let parts = rest.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return vec![];
    }
    parts
        .iter()
        .take(parts.len() - 1)
        .map(|part| part.trim_matches('"').trim_matches('\'').to_string())
        .collect()
}

fn dockerfile_copy_instruction(line: &str) -> Option<&str> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let instruction = parts.next()?.to_ascii_uppercase();
    if instruction != "COPY" && instruction != "ADD" {
        return None;
    }
    parts.next().map(str::trim).filter(|rest| !rest.is_empty())
}

fn strip_copy_options(mut rest: &str) -> &str {
    loop {
        let Some(stripped) = rest.strip_prefix("--") else {
            return rest.trim();
        };
        let Some(index) = stripped.find(char::is_whitespace) else {
            return "";
        };
        rest = stripped[index..].trim_start();
    }
}

fn source_is_dynamic_or_external(source: &str) -> bool {
    source == "."
        || source.contains('*')
        || source.starts_with('$')
        || source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("--")
}

fn yaml_get<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Value> {
    mapping.get(&serde_yaml::Value::String(key.to_string()))
}

fn yaml_string_field(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    yaml_get(mapping, key)
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn probe_http(port: u16, path: &str, reject_html_fallback: bool) -> HttpProbeResult {
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let mut stream = match TcpStream::connect(("127.0.0.1", port)) {
        Ok(stream) => stream,
        Err(error) => {
            return HttpProbeResult {
                path: normalized_path,
                status: "unreachable".to_string(),
                status_code: None,
                error: Some(error.to_string()),
                html_fallback: false,
            }
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let request =
        format!("GET {normalized_path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return HttpProbeResult {
            path: normalized_path,
            status: "unreachable".to_string(),
            status_code: None,
            error: Some(error.to_string()),
            html_fallback: false,
        };
    }
    let mut response = String::new();
    if let Err(error) = stream.read_to_string(&mut response) {
        return HttpProbeResult {
            path: normalized_path,
            status: "unreachable".to_string(),
            status_code: None,
            error: Some(error.to_string()),
            html_fallback: false,
        };
    }
    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok());
    let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
    let body_lower = body.to_ascii_lowercase();
    let html_fallback = reject_html_fallback
        && (body_lower.contains("<!doctype html") || body_lower.contains("<html"));
    let ok = status_code
        .map(|code| (200..=399).contains(&code))
        .unwrap_or(false)
        && !html_fallback;
    HttpProbeResult {
        path: normalized_path,
        status: if ok { "ok" } else { "invalid" }.to_string(),
        status_code,
        error: None,
        html_fallback,
    }
}

fn normalize_nginx_path(path: &str) -> String {
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        format!("{path}/")
    }
}
