use std::{fs, path::Path};

use contracts::{DeploymentComposeInfo, DeploymentComposePort, DeploymentComposeService};
use serde_yaml::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingDeploymentFiles {
    pub dockerfile_path: Option<std::path::PathBuf>,
    pub compose_path: Option<std::path::PathBuf>,
}

const COMPOSE_FILE_NAMES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

const DOCKERFILE_NAMES: &[&str] = &["Dockerfile", "dockerfile"];

pub fn find_existing_deployment_files(root: &Path) -> ExistingDeploymentFiles {
    ExistingDeploymentFiles {
        dockerfile_path: find_first_existing(root, DOCKERFILE_NAMES),
        compose_path: find_first_existing(root, COMPOSE_FILE_NAMES),
    }
}

pub fn analyze_existing_compose(compose_path: &Path) -> DeploymentComposeInfo {
    let raw = match fs::read_to_string(compose_path) {
        Ok(raw) => raw,
        Err(error) => return empty_compose_info(format!("Could not read Compose file: {error}")),
    };
    let document = match serde_yaml::from_str::<Value>(&raw) {
        Ok(document) => document,
        Err(error) => return empty_compose_info(format!("Could not parse Compose file: {error}")),
    };
    let Some(services) = document.get("services").and_then(Value::as_mapping) else {
        return empty_compose_info("Compose file has no services block.".to_string());
    };
    let mut analyzed = Vec::new();
    for (name, service) in services {
        let Some(name) = name.as_str() else {
            continue;
        };
        analyzed.push(analyze_compose_service(name, service));
    }
    if analyzed.is_empty() {
        return empty_compose_info("Compose file has no named services.".to_string());
    }
    let selected = select_compose_service(&analyzed);
    DeploymentComposeInfo {
        selected_service: selected.map(|service| service.name.clone()),
        service_reason: selected
            .map(|service| service.reason.clone())
            .unwrap_or_else(|| {
                "No application service could be selected from Compose.".to_string()
            }),
        warnings: selected
            .filter(|service| service.dependency_like)
            .map(|service| {
                vec![format!(
                    "Selected service {} looks dependency-like; generated fallback may be safer.",
                    service.name
                )]
            })
            .unwrap_or_default(),
        services: analyzed,
    }
}

pub fn selected_compose_port(info: &DeploymentComposeInfo) -> Option<DeploymentComposePort> {
    let selected = info.selected_service.as_ref()?;
    let service = info
        .services
        .iter()
        .find(|service| &service.name == selected)?;
    service
        .ports
        .iter()
        .find(|port| port.host_port.is_some())
        .cloned()
        .or_else(|| service.ports.first().cloned())
}

fn find_first_existing(root: &Path, names: &[&str]) -> Option<std::path::PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.exists())
}

fn empty_compose_info(reason: String) -> DeploymentComposeInfo {
    DeploymentComposeInfo {
        selected_service: None,
        service_reason: reason,
        services: vec![],
        warnings: vec![],
    }
}

fn analyze_compose_service(name: &str, service: &Value) -> DeploymentComposeService {
    let image = service
        .get("image")
        .and_then(Value::as_str)
        .map(str::to_string);
    let build = service.get("build").is_some();
    let ports = parse_ports(service.get("ports"));
    let expose = parse_expose(service.get("expose"));
    let depends_on = parse_depends_on(service.get("depends_on"));
    let profiles = parse_string_list(service.get("profiles"));
    let dependency_like = is_dependency_like_service(name, image.as_deref(), &ports);
    let mut signals = Vec::new();
    let mut score = 0;

    if is_app_service_name(name) {
        score += 70;
        signals.push(format!(
            "service name {name} looks like an application service"
        ));
    }
    if build {
        score += 45;
        signals.push("has build configuration".to_string());
    }
    if ports.iter().any(|port| port.host_port.is_some()) {
        score += 35;
        signals.push("publishes a host port".to_string());
    } else if !ports.is_empty() {
        score += 20;
        signals.push("declares service ports".to_string());
    }
    if !expose.is_empty() {
        score += 10;
        signals.push("exposes internal ports".to_string());
    }
    if !depends_on.is_empty() {
        score += 5;
        signals.push("depends on other services".to_string());
    }
    if dependency_like {
        score -= 90;
        signals.push("looks like an infrastructure dependency".to_string());
    }
    if profiles
        .iter()
        .any(|profile| contains_any(profile, &["test", "ci", "debug"]))
    {
        score -= 20;
        signals.push("is behind a test/debug profile".to_string());
    }
    if signals.is_empty() {
        signals.push("no strong service signals".to_string());
    }

    DeploymentComposeService {
        name: name.to_string(),
        score,
        image,
        build,
        ports,
        expose,
        depends_on,
        profiles,
        dependency_like,
        reason: signals.join("; "),
    }
}

fn select_compose_service(
    services: &[DeploymentComposeService],
) -> Option<&DeploymentComposeService> {
    let mut sorted = services.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.name.cmp(&right.name))
    });
    sorted
        .iter()
        .copied()
        .find(|service| !service.dependency_like)
        .or_else(|| sorted.first().copied())
}

fn parse_ports(value: Option<&Value>) -> Vec<DeploymentComposePort> {
    let Some(Value::Sequence(items)) = value else {
        return vec![];
    };
    items.iter().filter_map(parse_port).collect()
}

fn parse_port(value: &Value) -> Option<DeploymentComposePort> {
    match value {
        Value::String(raw) => parse_port_string(raw),
        Value::Number(number) => parse_port_string(&number.to_string()),
        Value::Mapping(mapping) => {
            let target = mapping
                .get(Value::String("target".to_string()))
                .and_then(numeric_port)?;
            let published = mapping
                .get(Value::String("published".to_string()))
                .and_then(numeric_port);
            let protocol = mapping
                .get(Value::String("protocol".to_string()))
                .and_then(Value::as_str)
                .map(str::to_string);
            Some(DeploymentComposePort {
                host_port: published,
                container_port: target,
                protocol,
                raw: serde_yaml::to_string(value).unwrap_or_else(|_| "port".to_string()),
            })
        }
        _ => None,
    }
}

fn parse_port_string(raw: &str) -> Option<DeploymentComposePort> {
    let trimmed = raw.trim();
    let (without_protocol, protocol) = trimmed
        .rsplit_once('/')
        .map(|(port, protocol)| (port, Some(protocol.to_string())))
        .unwrap_or((trimmed, None));
    let numeric_parts = without_protocol
        .split(':')
        .filter_map(|part| part.parse::<u16>().ok())
        .collect::<Vec<_>>();
    let container_port = *numeric_parts.last()?;
    let host_port = (numeric_parts.len() >= 2).then(|| numeric_parts[numeric_parts.len() - 2]);
    Some(DeploymentComposePort {
        host_port,
        container_port,
        protocol,
        raw: trimmed.to_string(),
    })
}

fn parse_expose(value: Option<&Value>) -> Vec<u16> {
    parse_string_list(value)
        .iter()
        .filter_map(|item| item.parse::<u16>().ok())
        .collect()
}

fn parse_depends_on(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Sequence(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::Mapping(mapping)) => mapping
            .keys()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => vec![],
    }
}

fn parse_string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Sequence(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .or_else(|| item.as_i64().map(|value| value.to_string()))
            })
            .collect(),
        Some(Value::String(item)) => vec![item.to_string()],
        Some(Value::Number(number)) => vec![number.to_string()],
        _ => vec![],
    }
}

fn numeric_port(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| value.as_str()?.parse::<u16>().ok())
}

fn is_app_service_name(name: &str) -> bool {
    contains_any(
        name,
        &[
            "app", "web", "frontend", "backend", "api", "server", "service",
        ],
    )
}

fn is_dependency_like_service(
    name: &str,
    image: Option<&str>,
    ports: &[DeploymentComposePort],
) -> bool {
    let combined = format!("{} {}", name, image.unwrap_or_default()).to_ascii_lowercase();
    contains_any(
        &combined,
        &[
            "postgres",
            "mysql",
            "redis",
            "mongo",
            "rabbit",
            "elasticsearch",
            "minio",
            "kafka",
        ],
    ) && !ports.iter().any(|port| port.host_port.is_some())
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}
