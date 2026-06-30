use std::{collections::BTreeSet, net::TcpListener};

use contracts::{
    DependencyService, DeploymentComposeInfo, DeploymentRoute, DeploymentRuntime,
    DeploymentRuntimeContract, DeploymentRuntimePort, DeploymentSourceModel,
    DeploymentSourceService, DeploymentTopology, SourceServiceRole,
};

pub fn build_deployment_runtime(
    runtime_contract: &DeploymentRuntimeContract,
    source_model: &DeploymentSourceModel,
    topology: &DeploymentTopology,
    compose_info: Option<&DeploymentComposeInfo>,
) -> DeploymentRuntime {
    let compose_ports = selected_compose_ports(compose_info);
    let mut allocated = BTreeSet::new();
    let mut ports = Vec::new();

    for service in &source_model.services {
        let purpose = service_purpose(source_model, topology, service);
        let internal_only = service.service_id != topology.public_entry_service_id
            && service.service_id != source_model.preview_service_id;
        let compose_port = compose_ports
            .iter()
            .find(|port| port.container_port == service.port)
            .or_else(|| {
                (service.service_id == source_model.preview_service_id)
                    .then(|| compose_ports.first())
                    .flatten()
            });
        let fixed_compose_host_port = compose_port.and_then(|port| port.host_port);
        let preferred = compose_port
            .and_then(|port| port.host_port)
            .or_else(|| (!internal_only).then(|| preferred_host_port(service)));
        let host_port = if internal_only {
            None
        } else if let Some(port) = fixed_compose_host_port {
            Some(port)
        } else {
            Some(resolve_host_port(
                preferred.unwrap_or_else(|| preferred_host_port(service)),
                &mut allocated,
            ))
        };
        let path = service_path(runtime_contract, topology, service);
        ports.push(DeploymentRuntimePort {
            service_id: service.service_id.clone(),
            purpose,
            container_port: compose_port
                .map(|port| port.container_port)
                .unwrap_or(service.port),
            preferred_host_port: preferred,
            host_port,
            path: path.clone(),
            internal_only,
            protocol: "http".to_string(),
            url: host_port.map(|port| url_for(port, &path)),
        });
    }

    for dependency in &source_model.dependencies {
        ports.push(dependency_port(dependency));
    }

    DeploymentRuntime {
        primary_service_id: source_model.preview_service_id.clone(),
        ports,
    }
}

pub fn primary_url(runtime: &DeploymentRuntime) -> String {
    runtime
        .ports
        .iter()
        .find(|port| port.service_id == runtime.primary_service_id && !port.internal_only)
        .and_then(|port| port.url.clone())
        .or_else(|| {
            runtime
                .ports
                .iter()
                .find(|port| !port.internal_only)
                .and_then(|port| port.url.clone())
        })
        .unwrap_or_else(|| "http://localhost".to_string())
}

pub fn primary_public_port(runtime: &DeploymentRuntime) -> Option<u16> {
    runtime
        .ports
        .iter()
        .find(|port| port.service_id == runtime.primary_service_id && !port.internal_only)
        .and_then(|port| port.host_port)
        .or_else(|| {
            runtime
                .ports
                .iter()
                .find(|port| !port.internal_only)
                .and_then(|port| port.host_port)
        })
}

pub fn host_port_for_service(runtime: &DeploymentRuntime, service_id: &str) -> Option<u16> {
    runtime
        .ports
        .iter()
        .find(|port| port.service_id == service_id && !port.internal_only)
        .and_then(|port| port.host_port)
}

fn selected_compose_ports(
    compose_info: Option<&DeploymentComposeInfo>,
) -> Vec<contracts::DeploymentComposePort> {
    let Some(info) = compose_info else {
        return Vec::new();
    };
    let Some(selected) = info.selected_service.as_ref() else {
        return Vec::new();
    };
    info.services
        .iter()
        .find(|service| &service.name == selected)
        .map(|service| service.ports.clone())
        .unwrap_or_default()
}

fn service_purpose(
    source_model: &DeploymentSourceModel,
    topology: &DeploymentTopology,
    service: &DeploymentSourceService,
) -> String {
    if service.service_id == source_model.preview_service_id {
        return "preview".to_string();
    }
    if topology.routes.iter().any(|route| {
        matches!(
            route,
            DeploymentRoute::HttpProxy {
                target_service_id,
                ..
            } if target_service_id == &service.service_id
        )
    }) || service.role == SourceServiceRole::Backend
    {
        return "api".to_string();
    }
    "service".to_string()
}

fn service_path(
    runtime_contract: &DeploymentRuntimeContract,
    topology: &DeploymentTopology,
    service: &DeploymentSourceService,
) -> String {
    if service.service_id == topology.public_entry_service_id {
        return normalize_path(&runtime_contract.preview_path);
    }
    topology
        .routes
        .iter()
        .find_map(|route| match route {
            DeploymentRoute::HttpProxy {
                public_path,
                target_service_id,
                ..
            } if target_service_id == &service.service_id => Some(normalize_path(public_path)),
            _ => None,
        })
        .or_else(|| service.healthcheck_path.as_deref().map(normalize_path))
        .unwrap_or_else(|| "/".to_string())
}

fn dependency_port(dependency: &DependencyService) -> DeploymentRuntimePort {
    DeploymentRuntimePort {
        service_id: dependency.service_name.clone(),
        purpose: "dependency".to_string(),
        container_port: dependency.port,
        preferred_host_port: None,
        host_port: None,
        path: "/".to_string(),
        internal_only: true,
        protocol: "tcp".to_string(),
        url: None,
    }
}

fn preferred_host_port(service: &DeploymentSourceService) -> u16 {
    if service.role == SourceServiceRole::Frontend && service.port == 80 {
        4173
    } else {
        service.port
    }
}

fn resolve_host_port(preferred: u16, allocated: &mut BTreeSet<u16>) -> u16 {
    for range in [preferred..=u16::MAX, 1024..=preferred] {
        for port in range {
            if allocated.contains(&port) {
                continue;
            }
            if TcpListener::bind(("127.0.0.1", port)).is_ok() {
                allocated.insert(port);
                return port;
            }
        }
    }
    preferred
}

fn url_for(host_port: u16, path: &str) -> String {
    let path = normalize_path(path);
    if path == "/" {
        format!("http://localhost:{host_port}")
    } else {
        format!("http://localhost:{host_port}{path}")
    }
}

fn normalize_path(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let path = trimmed.split(['?', '#']).next().unwrap_or("/");
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if path.len() > 1 {
        path.trim_end_matches('/').to_string()
    } else {
        path
    }
}
