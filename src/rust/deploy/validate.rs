use std::{
    io::{Read, Write},
    net::TcpStream,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use contracts::{DeployProvider, DeploymentRoute, DeploymentSpec};
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
        .api_paths
        .iter()
        .map(|path| match public_port {
            Some(port) => probe_http(port, path, true),
            None => missing_public_port_probe(path),
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
    for build in compose_build_specs(&compose) {
        let context_dir = normalize_path(compose_dir.join(&build.context));
        let dockerfile_path = normalize_path(context_dir.join(&build.dockerfile));
        if !dockerfile_path.exists() {
            issues.push(format!(
                "compose dockerfile path {} does not resolve from build context {}.",
                build.dockerfile, build.context
            ));
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
                ..
            } = route
            {
                let proxy_index = nginx.find(&format!(
                    "proxy_pass http://{target_service_id}:{target_port};"
                ));
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
    if !spec.topology.validation.api_paths.is_empty()
        && !spec
            .topology
            .routes
            .iter()
            .any(|route| matches!(route, DeploymentRoute::HttpProxy { .. }))
    {
        issues.push("topology has apiPaths but no http-proxy route.".to_string());
    }
    Ok(issues)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeBuildSpec {
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
        .values()
        .filter_map(|service| {
            let service = service.as_mapping()?;
            let build = yaml_get(service, "build")?;
            if let Some(context) = build.as_str() {
                return Some(ComposeBuildSpec {
                    context: context.to_string(),
                    dockerfile: "Dockerfile".to_string(),
                });
            }
            let build = build.as_mapping()?;
            Some(ComposeBuildSpec {
                context: yaml_string_field(build, "context").unwrap_or_else(|| ".".to_string()),
                dockerfile: yaml_string_field(build, "dockerfile")
                    .unwrap_or_else(|| "Dockerfile".to_string()),
            })
        })
        .collect()
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
