use std::{
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

use contracts::{DeploymentRoute, DeploymentSpec};
use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult, LoomMcpFailure, LoomMcpFailureResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use state::{
    paths::from_project_relative,
    store::{read_text, StateResult},
};

use crate::{prepare::read_spec, runtime_state::write_success_state, DeployToolInput};

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
    let preview = spec
        .topology
        .validation
        .preview_paths
        .iter()
        .map(|path| probe_http(spec.runtime.host_port, path, false))
        .collect::<Vec<_>>();
    let api_routes = spec
        .topology
        .validation
        .api_paths
        .iter()
        .map(|path| probe_http(spec.runtime.host_port, path, true))
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

pub fn validate_generated_assets(
    project_root: &Path,
    spec: &DeploymentSpec,
) -> StateResult<Vec<String>> {
    let mut issues = Vec::new();
    let compose = read_text(&from_project_relative(
        project_root,
        &spec.files.compose_path,
    )?)?;
    if !compose.contains("services:") {
        issues.push("compose file must include services.".to_string());
    }
    for service in &spec.source_model.services {
        if !compose.contains(&format!("  {}:", service.service_id)) {
            issues.push(format!(
                "compose is missing service {}.",
                service.service_id
            ));
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
