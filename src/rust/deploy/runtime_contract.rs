use std::{collections::BTreeMap, path::Path};

use contracts::{
    DependencyService, DependencyServiceKind, DeploymentApiContract, DeploymentApiInterface,
    DeploymentCommandContract, DeploymentCommandPhase, DeploymentContractAuthority,
    DeploymentHttpProbe, DeploymentRuntimeContract, DeploymentShape, RuntimeContractApi,
    RuntimeContractEndpoint, RuntimeEnvironmentContract,
};
use delivery_core::{DeliveryIndex, DeliveryPhaseState, TransitionStore};
use serde_json::Value;
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative},
    store::{read_json_value, StateError, StateResult},
};

pub fn load_runtime_contract(project_root: &Path) -> StateResult<DeploymentRuntimeContract> {
    let project_root_string = project_root.to_string_lossy();
    let store = FileTransitionStore;
    let status = store
        .load_status(&project_root_string)
        .map_err(|error| StateError::InvalidArgument(error.message().to_string()))?;
    let Some(delivery_id) = status
        .active_delivery_id
        .or(status.last_completed_delivery_id)
    else {
        return Ok(heuristic_runtime_contract());
    };
    let Ok(delivery) = store
        .load_delivery_index(&project_root_string, &delivery_id)
        .map_err(|error| StateError::InvalidArgument(error.message().to_string()))
    else {
        return Ok(heuristic_runtime_contract());
    };
    match load_runtime_contract_from_delivery(project_root, &delivery) {
        Ok(contract) => Ok(contract),
        Err(error)
            if error
                .to_string()
                .contains("No accepted AAC runtimeDelivery") =>
        {
            Ok(heuristic_runtime_contract())
        }
        Err(error) => Err(error),
    }
}

fn runtime_contract_from_value_with_api_contract(
    runtime: &Value,
    runtime_ref: Option<String>,
    api_contract: Option<DeploymentApiContract>,
) -> StateResult<DeploymentRuntimeContract> {
    let status = string_at(runtime, &["status"]).unwrap_or_else(|| "modified".to_string());
    if status == "not_applicable" {
        return Err(StateError::InvalidArgument(
            "AAC runtimeDelivery is not_applicable; deploy cannot guess runtime stack.".to_string(),
        ));
    }
    let frontend = runtime.get("frontend").and_then(endpoint_from_value);
    let api = runtime.get("api").and_then(api_from_value);
    let shape = deployment_shape(
        runtime,
        frontend.as_ref(),
        api.as_ref(),
        api_contract.as_ref(),
    );
    let http_probes = runtime.get("httpProbes").unwrap_or(&Value::Null);
    let preview_path = string_at(http_probes, &["previewPath"]).unwrap_or_else(|| "/".to_string());
    let runtime_kind = string_at(runtime, &["runtimeKind"]);
    let commands = command_contract_from_runtime(runtime);
    let port = ["development", "verification", "deployment"]
        .into_iter()
        .find_map(|phase| u16_at(runtime, &["commands", phase, "start", "port"]));
    let health_path = string_at(http_probes, &["healthPath"]);
    let safe_http_probes =
        safe_http_probes(&preview_path, health_path.as_deref(), api_contract.as_ref());
    let environment = RuntimeEnvironmentContract {
        required: string_array_at(runtime, &["environment", "required"]).unwrap_or_default(),
        optional: string_array_at(runtime, &["environment", "optional"]).unwrap_or_default(),
    };
    let dependency_services = dependency_services_from_runtime(runtime);

    let mut api = api;
    if api.is_none() {
        if let Some(contract) = &api_contract {
            api = Some(RuntimeContractApi {
                required: true,
                kind: None,
                commands: DeploymentCommandPhase::default(),
                entry: None,
                base_path: contract.public_base_path.clone(),
            });
        }
    }
    if let Some(contract) = &api_contract {
        if let Some(api) = api.as_mut() {
            api.base_path = contract.public_base_path.clone();
        }
    }
    Ok(DeploymentRuntimeContract {
        authority: DeploymentContractAuthority::AcceptedContract,
        r#ref: runtime_ref,
        status,
        dependency_service_policy: "contract_only".to_string(),
        deployment_shape: Some(shape),
        runtime_kind,
        commands,
        port,
        preview_path,
        health_path,
        safe_http_probes,
        frontend_output_dir: frontend
            .as_ref()
            .and_then(|frontend| frontend.output_dir.clone())
            .or_else(|| string_at(runtime, &["frontendOutputDir"])),
        probe_kind: "http".to_string(),
        environment,
        frontend,
        api,
        api_contract,
        dependency_services,
    })
}

fn endpoint_from_value(value: &Value) -> Option<RuntimeContractEndpoint> {
    Some(RuntimeContractEndpoint {
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        kind: string_at(value, &["kind"]),
        commands: endpoint_commands_from_value(value),
        source_root: string_at(value, &["sourceRoot"]),
        output_dir: string_at(value, &["outputDir"]),
        served_by: string_at(value, &["servedBy"]),
        served_by_ref: string_at(value, &["servedByRef"]),
    })
}

fn command_contract_from_runtime(runtime: &Value) -> DeploymentCommandContract {
    let commands = runtime.get("commands");
    DeploymentCommandContract {
        development: command_phase_from_value(commands.and_then(|value| value.get("development"))),
        verification: command_phase_from_value(
            commands.and_then(|value| value.get("verification")),
        ),
        deployment: command_phase_from_value(commands.and_then(|value| value.get("deployment"))),
    }
}

fn command_phase_from_value(value: Option<&Value>) -> DeploymentCommandPhase {
    let Some(value) = value else {
        return DeploymentCommandPhase::default();
    };
    DeploymentCommandPhase {
        build: string_at(value, &["build", "command"]),
        start: string_at(value, &["start", "command"]),
        working_directory: string_at(value, &["start", "workingDirectory"])
            .or_else(|| string_at(value, &["build", "workingDirectory"])),
    }
}

fn api_from_value(value: &Value) -> Option<RuntimeContractApi> {
    Some(RuntimeContractApi {
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        kind: string_at(value, &["kind"]),
        commands: endpoint_commands_from_value(value),
        entry: string_at(value, &["entry"]),
        base_path: string_at(value, &["basePath"]),
    })
}

fn endpoint_commands_from_value(value: &Value) -> DeploymentCommandPhase {
    command_phase_from_value(value.get("commands"))
}

fn safe_http_probes(
    preview_path: &str,
    health_path: Option<&str>,
    contract: Option<&DeploymentApiContract>,
) -> Vec<DeploymentHttpProbe> {
    let mut probes = Vec::new();
    probes.push(DeploymentHttpProbe {
        method: "GET".to_string(),
        path: normalize_api_path(preview_path),
        interface_id: None,
        source: "runtime_preview".to_string(),
    });
    if let Some(path) = health_path {
        probes.push(DeploymentHttpProbe {
            method: "GET".to_string(),
            path: normalize_api_path(path),
            interface_id: None,
            source: "runtime_healthcheck".to_string(),
        });
    }
    if let Some(contract) = contract {
        for interface in &contract.interfaces {
            if matches!(interface.method.as_str(), "GET" | "HEAD")
                && is_concrete_probe_path(&interface.path)
            {
                probes.push(DeploymentHttpProbe {
                    method: interface.method.clone(),
                    path: interface.path.clone(),
                    interface_id: Some(interface.interface_id.clone()),
                    source: "accepted_api_contract".to_string(),
                });
            }
        }
    }
    probes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.method.cmp(&right.method))
    });
    probes.dedup_by(|left, right| left.method == right.method && left.path == right.path);
    probes
}

fn is_concrete_probe_path(path: &str) -> bool {
    !path.contains(['{', '}', '*'])
}

fn api_contract_from_project_value(
    project_contract: &Value,
    runtime: &Value,
    contract_ref: &str,
) -> Option<DeploymentApiContract> {
    let interfaces = project_contract
        .get("interfaces")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|interface| interface.get("type").and_then(Value::as_str) == Some("http_api"))
        .filter_map(|interface| {
            let interface_id = interface.get("interfaceId")?.as_str()?.trim();
            let method = interface.get("method")?.as_str()?.trim();
            let path = normalize_api_path(interface.get("path")?.as_str()?);
            if interface_id.is_empty() || method.is_empty() || path.is_empty() {
                return None;
            }
            Some(DeploymentApiInterface {
                interface_id: interface_id.to_string(),
                method: method.to_ascii_uppercase(),
                path,
            })
        })
        .collect::<Vec<_>>();
    if interfaces.is_empty() {
        return None;
    }

    let public_base_path = project_contract
        .pointer("/publicExposure/basePath")
        .and_then(Value::as_str)
        .or_else(|| runtime.pointer("/api/basePath").and_then(Value::as_str))
        .map(normalize_api_path)
        .filter(|path| !path.is_empty() && path != "/")
        .or_else(|| common_api_base_path(&interfaces));
    let preserve_path = project_contract
        .pointer("/publicExposure/preservePath")
        .and_then(Value::as_bool)
        .or_else(|| {
            runtime
                .pointer("/api/preservePath")
                .and_then(Value::as_bool)
        })
        .unwrap_or(true);
    let browser_mode = project_contract
        .pointer("/browserBinding/mode")
        .and_then(Value::as_str)
        .or_else(|| {
            runtime
                .pointer("/api/browserBinding/mode")
                .and_then(Value::as_str)
        })
        .unwrap_or("same_origin")
        .to_string();
    let browser_base_url = project_contract
        .pointer("/browserBinding/baseUrl")
        .and_then(Value::as_str)
        .or_else(|| {
            runtime
                .pointer("/api/browserBinding/baseUrl")
                .and_then(Value::as_str)
        })
        .map(str::to_string)
        .filter(|value| !value.is_empty());

    Some(DeploymentApiContract {
        source_ref: format!(
            "{}#/interfaces",
            contract_ref.trim_end_matches("#/interfaces")
        ),
        status: if public_base_path.as_deref().is_none_or(|base| {
            interfaces.iter().all(|interface| {
                interface.path == base || interface.path.starts_with(&format!("{base}/"))
            })
        }) {
            "resolved".to_string()
        } else {
            "invalid".to_string()
        },
        interfaces,
        public_base_path,
        preserve_path,
        browser_mode,
        browser_base_url,
    })
}

fn normalize_api_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }
    let path = path.split(['?', '#']).next().unwrap_or(path).trim();
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if path.len() == 1 {
        path
    } else {
        path.trim_end_matches('/').to_string()
    }
}

fn common_api_base_path(interfaces: &[DeploymentApiInterface]) -> Option<String> {
    let first = interfaces
        .first()?
        .path
        .split('/')
        .find(|segment| !segment.is_empty() && !segment.starts_with('{'))?;
    if first.is_empty() {
        return None;
    }
    Some(format!("/{first}"))
}

fn deployment_shape(
    runtime: &Value,
    frontend: Option<&RuntimeContractEndpoint>,
    api: Option<&RuntimeContractApi>,
    api_contract: Option<&DeploymentApiContract>,
) -> DeploymentShape {
    if runtime_requires_public_frontend_api_topology(runtime, frontend, api, api_contract) {
        return DeploymentShape::FrontendAndBackend;
    }
    if !frontend_served_by_integrated_app(frontend)
        && frontend.map(|item| item.required).unwrap_or(false)
        && api.map(|item| item.required).unwrap_or(false)
    {
        return DeploymentShape::FrontendAndBackend;
    }
    DeploymentShape::SingleService
}

fn runtime_requires_public_frontend_api_topology(
    runtime: &Value,
    frontend: Option<&RuntimeContractEndpoint>,
    api: Option<&RuntimeContractApi>,
    api_contract: Option<&DeploymentApiContract>,
) -> bool {
    if frontend_served_by_integrated_app(frontend) {
        return false;
    }
    let frontend_required = frontend.map(|item| item.required).unwrap_or(false);
    let api_required = api.map(|item| item.required).unwrap_or(false)
        || api_contract
            .map(|contract| !contract.interfaces.is_empty())
            .unwrap_or(false);
    let has_frontend_surface =
        runtime_has_surface_kind(runtime, &["frontend", "web", "ui"]) || frontend_required;
    let has_api_surface = runtime_has_surface_kind(runtime, &["api", "backend"]) || api_required;

    if has_frontend_surface && api_required {
        return true;
    }
    if frontend_required && has_api_surface {
        return true;
    }
    if runtime_has_surface_kind(runtime, &["frontend", "web", "ui"])
        && runtime_has_surface_kind(runtime, &["api", "backend"])
    {
        return true;
    }
    if has_frontend_surface && labeled_commands_declare_frontend_and_backend(runtime) {
        return true;
    }
    false
}

fn frontend_served_by_integrated_app(frontend: Option<&RuntimeContractEndpoint>) -> bool {
    [
        frontend.and_then(|endpoint| endpoint.served_by.as_deref()),
        frontend.and_then(|endpoint| endpoint.served_by_ref.as_deref()),
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

fn runtime_has_surface_kind(runtime: &Value, accepted: &[&str]) -> bool {
    runtime
        .get("runtimeSurfaces")
        .and_then(Value::as_array)
        .map(|surfaces| {
            surfaces.iter().any(|surface| {
                string_at(surface, &["kind"])
                    .map(|kind| {
                        let normalized = kind.to_ascii_lowercase().replace(['_', '-'], "");
                        accepted
                            .iter()
                            .any(|item| normalized.contains(&item.replace(['_', '-'], "")))
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn labeled_commands_declare_frontend_and_backend(runtime: &Value) -> bool {
    let mut labels = Vec::new();
    for phase_name in ["development", "verification", "deployment"] {
        let phase = runtime
            .get("commands")
            .and_then(|commands| commands.get(phase_name))
            .unwrap_or(&Value::Null);
        for command in [
            string_at(phase, &["build", "command"]),
            string_at(phase, &["start", "command"]),
        ]
        .into_iter()
        .flatten()
        {
            labels.extend(labeled_command_segments(&command));
        }
    }
    for endpoint in ["frontend", "api"] {
        let commands = runtime
            .get(endpoint)
            .and_then(|endpoint| endpoint.get("commands"))
            .unwrap_or(&Value::Null);
        labels.extend(
            [
                string_at(commands, &["build", "command"]),
                string_at(commands, &["start", "command"]),
            ]
            .into_iter()
            .flatten()
            .flat_map(|command| labeled_command_segments(&command)),
        );
    }
    let has_frontend = labels
        .iter()
        .any(|label| matches!(label.as_str(), "frontend" | "web" | "client" | "ui"));
    let has_backend = labels
        .iter()
        .any(|label| matches!(label.as_str(), "backend" | "api" | "service" | "server"));
    has_frontend && has_backend
}

fn labeled_command_segments(command: &str) -> Vec<String> {
    command
        .split(';')
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .filter_map(|part| {
            let trimmed = part.trim();
            let (label, rest) = trimmed.split_once(':')?;
            let label = label.trim();
            if rest.trim().is_empty()
                || label.is_empty()
                || label.contains(char::is_whitespace)
                || !label
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            {
                return None;
            }
            Some(label.to_ascii_lowercase())
        })
        .collect()
}

fn load_runtime_contract_from_delivery(
    project_root: &Path,
    delivery: &DeliveryIndex,
) -> StateResult<DeploymentRuntimeContract> {
    for phase in deploy_runtime_phase_order(delivery) {
        let Some(architecture_ref) = phase
            .latest_refs
            .get("architectureArtifact")
            .or_else(|| phase.latest_refs.get("architectureArtifactContract"))
        else {
            continue;
        };
        return runtime_contract_from_architecture_ref(project_root, architecture_ref);
    }
    Err(StateError::InvalidArgument(
        "No accepted AAC runtimeDelivery contract was found.".to_string(),
    ))
}

fn deploy_runtime_phase_order(delivery: &DeliveryIndex) -> Vec<&DeliveryPhaseState> {
    let mut phases = Vec::new();
    if let Some(active) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == delivery.active_phase_id)
    {
        phases.push(active);
    }
    for phase in delivery.phases.iter().rev() {
        if phases
            .iter()
            .any(|existing| existing.phase_id == phase.phase_id)
        {
            continue;
        }
        phases.push(phase);
    }
    phases
}

fn runtime_contract_from_architecture_ref(
    project_root: &Path,
    architecture_ref: &str,
) -> StateResult<DeploymentRuntimeContract> {
    let aac_file = from_project_relative(project_root, architecture_ref)?;
    let aac = read_json_value(&aac_file)?;
    let runtime = aac.get("runtimeDelivery").ok_or_else(|| {
        StateError::InvalidArgument("AAC runtimeDelivery is missing.".to_string())
    })?;
    let runtime_ref = format!(
        "{}#/runtimeDelivery",
        to_project_relative(project_root, &aac_file)?
    );
    let api_contract_ref = aac.get("apiContractRef").and_then(Value::as_str);
    let api_contract = api_contract_ref
        .and_then(|contract_ref| from_project_relative(project_root, contract_ref).ok())
        .and_then(|contract_path| read_json_value(&contract_path).ok())
        .and_then(|contract| {
            api_contract_from_project_value(
                &contract,
                runtime,
                api_contract_ref.unwrap_or_default(),
            )
        });
    if aac
        .get("currentPhaseInterfaceRefs")
        .and_then(Value::as_array)
        .is_some_and(|refs| !refs.is_empty())
        && api_contract.is_none()
    {
        return Err(StateError::InvalidArgument(
            "AAC declares HTTP interfaces but apiContractRef cannot be loaded.".to_string(),
        ));
    }
    runtime_contract_from_value_with_api_contract(runtime, Some(runtime_ref), api_contract)
}

fn heuristic_runtime_contract() -> DeploymentRuntimeContract {
    DeploymentRuntimeContract {
        authority: DeploymentContractAuthority::RepositoryHeuristic,
        r#ref: None,
        status: "modified".to_string(),
        dependency_service_policy: "code_probe".to_string(),
        deployment_shape: None,
        runtime_kind: None,
        commands: DeploymentCommandContract::default(),
        port: None,
        preview_path: "/".to_string(),
        health_path: None,
        safe_http_probes: vec![],
        frontend_output_dir: None,
        probe_kind: "http".to_string(),
        environment: RuntimeEnvironmentContract {
            required: vec![],
            optional: vec![],
        },
        frontend: None,
        api: None,
        api_contract: None,
        dependency_services: vec![],
    }
}

fn dependency_services_from_runtime(runtime: &Value) -> Vec<DependencyService> {
    runtime
        .get("runtimeDependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|dependency| {
            dependency.get("startupRequirement").and_then(Value::as_str) != Some("not_applicable")
        })
        .filter_map(|dependency| {
            let kind = dependency_service_kind(dependency)?;
            Some(service_definition_for_dependency(kind, dependency))
        })
        .fold(Vec::new(), |mut services, service| {
            if !services
                .iter()
                .any(|existing: &DependencyService| existing.service_name == service.service_name)
            {
                services.push(service);
            }
            services
        })
}

fn dependency_service_kind(dependency: &Value) -> Option<DependencyServiceKind> {
    let provider = dependency
        .get("provider")
        .or_else(|| dependency.get("serviceKind"))
        .or_else(|| dependency.get("technologyId"))
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "");
    match provider.as_str() {
        "postgres" | "postgresql" => Some(DependencyServiceKind::Postgres),
        "mysql" | "mariadb" => Some(DependencyServiceKind::Mysql),
        "redis" => Some(DependencyServiceKind::Redis),
        "mongodb" | "mongo" => Some(DependencyServiceKind::Mongodb),
        "rabbitmq" => Some(DependencyServiceKind::Rabbitmq),
        "elasticsearch" | "opensearch" => Some(DependencyServiceKind::Elasticsearch),
        "minio" | "s3" => Some(DependencyServiceKind::Minio),
        _ => None,
    }
}

pub(crate) fn service_definition(kind: DependencyServiceKind) -> DependencyService {
    match kind {
        DependencyServiceKind::Postgres => DependencyService {
            kind,
            service_name: "postgres".to_string(),
            image: "postgres:16-alpine".to_string(),
            port: 5432,
            startup_requirement: "required".to_string(),
            env: BTreeMap::from([
                ("POSTGRES_DB".to_string(), "loom_app".to_string()),
                ("POSTGRES_USER".to_string(), "loom".to_string()),
                ("POSTGRES_PASSWORD".to_string(), "loom".to_string()),
            ]),
            connection_env: BTreeMap::from([
                (
                    "SPRING_DATASOURCE_URL".to_string(),
                    "jdbc:postgresql://postgres:5432/loom_app".to_string(),
                ),
                ("SPRING_DATASOURCE_USERNAME".to_string(), "loom".to_string()),
                ("SPRING_DATASOURCE_PASSWORD".to_string(), "loom".to_string()),
            ]),
            volume_name: Some("loom_postgres_data".to_string()),
            volume_target: Some("/var/lib/postgresql/data".to_string()),
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
        DependencyServiceKind::Redis => DependencyService {
            kind,
            service_name: "redis".to_string(),
            image: "redis:7-alpine".to_string(),
            port: 6379,
            startup_requirement: "required".to_string(),
            env: BTreeMap::new(),
            connection_env: BTreeMap::from([(
                "REDIS_URL".to_string(),
                "redis://redis:6379".to_string(),
            )]),
            volume_name: None,
            volume_target: None,
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
        DependencyServiceKind::Mysql => DependencyService {
            kind,
            service_name: "mysql".to_string(),
            image: "mysql:8".to_string(),
            port: 3306,
            startup_requirement: "required".to_string(),
            env: BTreeMap::from([
                ("MYSQL_DATABASE".to_string(), "loom_app".to_string()),
                ("MYSQL_USER".to_string(), "loom".to_string()),
                ("MYSQL_PASSWORD".to_string(), "loom".to_string()),
                ("MYSQL_ROOT_PASSWORD".to_string(), "loom".to_string()),
            ]),
            connection_env: BTreeMap::from([
                (
                    "SPRING_DATASOURCE_URL".to_string(),
                    "jdbc:mysql://mysql:3306/loom_app".to_string(),
                ),
                ("SPRING_DATASOURCE_USERNAME".to_string(), "loom".to_string()),
                ("SPRING_DATASOURCE_PASSWORD".to_string(), "loom".to_string()),
            ]),
            volume_name: Some("loom_mysql_data".to_string()),
            volume_target: Some("/var/lib/mysql".to_string()),
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
        DependencyServiceKind::Mongodb => DependencyService {
            kind,
            service_name: "mongodb".to_string(),
            image: "mongo:8".to_string(),
            port: 27017,
            startup_requirement: "required".to_string(),
            env: BTreeMap::from([("MONGO_INITDB_DATABASE".to_string(), "loom_app".to_string())]),
            connection_env: BTreeMap::from([(
                "MONGODB_URL".to_string(),
                "mongodb://mongodb:27017/loom_app".to_string(),
            )]),
            volume_name: Some("loom_mongodb_data".to_string()),
            volume_target: Some("/data/db".to_string()),
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
        DependencyServiceKind::Rabbitmq => DependencyService {
            kind,
            service_name: "rabbitmq".to_string(),
            image: "rabbitmq:3-management-alpine".to_string(),
            port: 5672,
            startup_requirement: "required".to_string(),
            env: BTreeMap::from([
                ("RABBITMQ_DEFAULT_USER".to_string(), "loom".to_string()),
                ("RABBITMQ_DEFAULT_PASS".to_string(), "loom".to_string()),
            ]),
            connection_env: BTreeMap::from([(
                "AMQP_URL".to_string(),
                "amqp://loom:loom@rabbitmq:5672/%2f".to_string(),
            )]),
            volume_name: Some("loom_rabbitmq_data".to_string()),
            volume_target: Some("/var/lib/rabbitmq".to_string()),
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
        DependencyServiceKind::Elasticsearch => DependencyService {
            kind,
            service_name: "elasticsearch".to_string(),
            image: "docker.elastic.co/elasticsearch/elasticsearch:8.15.0".to_string(),
            port: 9200,
            startup_requirement: "required".to_string(),
            env: BTreeMap::from([
                ("discovery.type".to_string(), "single-node".to_string()),
                ("xpack.security.enabled".to_string(), "false".to_string()),
            ]),
            connection_env: BTreeMap::from([(
                "ELASTICSEARCH_URL".to_string(),
                "http://elasticsearch:9200".to_string(),
            )]),
            volume_name: Some("loom_elasticsearch_data".to_string()),
            volume_target: Some("/usr/share/elasticsearch/data".to_string()),
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
        DependencyServiceKind::Minio => DependencyService {
            kind,
            service_name: "minio".to_string(),
            image: "minio/minio:latest".to_string(),
            port: 9000,
            startup_requirement: "required".to_string(),
            env: BTreeMap::from([
                ("MINIO_ROOT_USER".to_string(), "loom".to_string()),
                (
                    "MINIO_ROOT_PASSWORD".to_string(),
                    "loom-password".to_string(),
                ),
            ]),
            connection_env: BTreeMap::from([
                ("S3_ENDPOINT".to_string(), "http://minio:9000".to_string()),
                ("S3_ACCESS_KEY".to_string(), "loom".to_string()),
                ("S3_SECRET_KEY".to_string(), "loom-password".to_string()),
            ]),
            volume_name: Some("loom_minio_data".to_string()),
            volume_target: Some("/data".to_string()),
            reason: "Declared by structured RuntimeDeliveryContract dependency facts.".to_string(),
        },
    }
}

fn service_definition_for_dependency(
    kind: DependencyServiceKind,
    dependency: &Value,
) -> DependencyService {
    let mut service = service_definition(kind);
    let service_name = dependency_service_name(dependency, kind);
    service.service_name = service_name.clone();
    service.startup_requirement = dependency
        .get("startupRequirement")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "required" | "optional" | "not_applicable"))
        .unwrap_or("required")
        .to_string();
    if kind == DependencyServiceKind::Redis && service_name != "redis" {
        service.connection_env.insert(
            "REDIS_URL".to_string(),
            format!("redis://{service_name}:{}", service.port),
        );
    }
    if kind == DependencyServiceKind::Redis && dependency_has_persistent_capability(dependency) {
        service.volume_name = Some(format!("loom_{service_name}_data"));
        service.volume_target = Some("/data".to_string());
    }
    service
}

fn dependency_service_name(dependency: &Value, kind: DependencyServiceKind) -> String {
    let default_name = match kind {
        DependencyServiceKind::Redis => "redis",
        DependencyServiceKind::Postgres => "postgres",
        DependencyServiceKind::Mysql => "mysql",
        DependencyServiceKind::Mongodb => "mongodb",
        DependencyServiceKind::Rabbitmq => "rabbitmq",
        DependencyServiceKind::Elasticsearch => "elasticsearch",
        DependencyServiceKind::Minio => "minio",
    };
    let Some(dependency_id) = dependency.get("dependencyId").and_then(Value::as_str) else {
        return default_name.to_string();
    };
    let normalized_id = dependency_id
        .strip_prefix("runtime_")
        .unwrap_or(dependency_id);
    if dependency_service_kind(dependency) == Some(kind)
        && (normalized_id == "persistence" || normalized_id == default_name)
    {
        return default_name.to_string();
    }
    let suffix = dependency_id
        .strip_prefix("runtime_")
        .unwrap_or(dependency_id)
        .split('_')
        .skip_while(|part| *part == default_name)
        .collect::<Vec<_>>()
        .join("_");
    if suffix.is_empty() {
        default_name.to_string()
    } else {
        format!("{default_name}_{suffix}")
    }
}

fn dependency_has_persistent_capability(dependency: &Value) -> bool {
    dependency
        .get("capabilities")
        .and_then(Value::as_array)
        .is_some_and(|capabilities| {
            capabilities.iter().any(|capability| {
                capability.get("durability").and_then(Value::as_str) == Some("persistent")
            })
        })
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(str::to_string)
}

fn u16_at(value: &Value, path: &[&str]) -> Option<u16> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_u64().and_then(|value| u16::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepted_api_contract_source_ref_points_to_project_contract() {
        let contract = json!({
            "interfaces": [{
                "interfaceId": "interface-ticket-list",
                "type": "http_api",
                "method": "GET",
                "path": "/api/tickets"
            }],
            "publicExposure": {"basePath": "/api"},
            "browserBinding": {"mode": "same_origin"}
        });
        let runtime = json!({});
        let parsed = api_contract_from_project_value(
            &contract,
            &runtime,
            ".loom/deliveries/delivery_1/contracts/api/current.json",
        )
        .expect("API contract");

        assert_eq!(
            parsed.source_ref,
            ".loom/deliveries/delivery_1/contracts/api/current.json#/interfaces"
        );
    }

    #[test]
    fn safe_probes_skip_parameterized_read_interfaces() {
        let contract = DeploymentApiContract {
            source_ref: "contract#/interfaces".to_string(),
            status: "resolved".to_string(),
            interfaces: vec![
                DeploymentApiInterface {
                    interface_id: "tickets.list".to_string(),
                    method: "GET".to_string(),
                    path: "/api/tickets".to_string(),
                },
                DeploymentApiInterface {
                    interface_id: "tickets.detail".to_string(),
                    method: "GET".to_string(),
                    path: "/api/tickets/{ticketId}".to_string(),
                },
            ],
            public_base_path: Some("/api".to_string()),
            preserve_path: true,
            browser_mode: "same_origin".to_string(),
            browser_base_url: None,
        };

        let probes = safe_http_probes("/", None, Some(&contract));
        assert!(probes.iter().any(|probe| probe.path == "/api/tickets"));
        assert!(!probes.iter().any(|probe| probe.path.contains("{ticketId}")));
    }

    #[test]
    fn runtime_dependencies_use_structured_provider_only() {
        let runtime = json!({
            "status": "modified",
            "runtimeKind": "java",
            "runtimeDependencies": [
                {
                    "dependencyId": "runtime_persistence",
                    "kind": "storage",
                    "provider": "postgresql",
                    "startupRequirement": "required",
                    "failureBehavior": "postgres is unavailable",
                    "recoveryStrategy": "restore storage",
                    "requiredFor": ["writes"],
                    "observability": ["readiness"]
                },
                {
                    "dependencyId": "runtime_external",
                    "kind": "external_runtime",
                    "startupRequirement": "optional",
                    "failureBehavior": "a redis mention in prose must not create a service",
                    "recoveryStrategy": "retry",
                    "requiredFor": ["notifications"],
                    "observability": ["connection errors"]
                }
            ],
            "commands": {
                "development": {"start": {"port": 8080}},
                "verification": {},
                "deployment": {}
            },
            "httpProbes": {"previewPath": "/", "expectedStatus": "2xx_or_3xx"},
            "environment": {"required": [], "optional": []}
        });
        let contract = runtime_contract_from_value_with_api_contract(&runtime, None, None)
            .expect("runtime contract");

        assert_eq!(
            contract
                .dependency_services
                .iter()
                .map(|service| service.service_name.as_str())
                .collect::<Vec<_>>(),
            vec!["postgres"]
        );
    }

    #[test]
    fn labeled_runtime_commands_are_read_from_each_command_phase() {
        let runtime = json!({
            "runtimeSurfaces": [
                {"surfaceId": "web", "kind": "frontend"},
                {"surfaceId": "api", "kind": "api"}
            ],
            "commands": {
                "development": {
                    "build": {"command": "frontend: npm run build; backend: mvn package"},
                    "start": {"command": "backend: java -jar app.jar"}
                }
            }
        });
        assert!(runtime_requires_public_frontend_api_topology(
            &runtime, None, None, None
        ));
    }

    #[test]
    fn redis_capability_durability_controls_volume_projection() {
        let base = json!({
            "status": "modified",
            "runtimeKind": "python",
            "commands": {"development": {"start": {"port": 8000}}},
            "httpProbes": {"previewPath": "/", "expectedStatus": "2xx_or_3xx"},
            "environment": {"required": [], "optional": []}
        });
        let mut ephemeral = base.clone();
        ephemeral["runtimeDependencies"] = json!([{
            "dependencyId": "runtime_redis",
            "kind": "external_runtime",
            "provider": "redis",
            "startupRequirement": "optional",
            "capabilities": [{
                "purpose": "cache",
                "durability": "ephemeral",
                "startupRequirement": "optional"
            }]
        }]);
        let ephemeral_contract =
            runtime_contract_from_value_with_api_contract(&ephemeral, None, None)
                .expect("ephemeral Redis runtime contract");
        let ephemeral_redis = ephemeral_contract
            .dependency_services
            .iter()
            .find(|service| service.kind == DependencyServiceKind::Redis)
            .expect("ephemeral Redis service");
        assert_eq!(ephemeral_redis.startup_requirement, "optional");
        assert!(ephemeral_redis.volume_name.is_none());

        let mut persistent = base;
        persistent["runtimeDependencies"] = json!([{
            "dependencyId": "runtime_redis",
            "kind": "external_runtime",
            "provider": "redis",
            "startupRequirement": "required",
            "capabilities": [{
                "purpose": "session",
                "durability": "persistent",
                "startupRequirement": "required"
            }]
        }]);
        let persistent_contract =
            runtime_contract_from_value_with_api_contract(&persistent, None, None)
                .expect("persistent Redis runtime contract");
        let persistent_redis = persistent_contract
            .dependency_services
            .iter()
            .find(|service| service.kind == DependencyServiceKind::Redis)
            .expect("persistent Redis service");
        assert_eq!(persistent_redis.startup_requirement, "required");
        assert_eq!(
            persistent_redis.volume_name.as_deref(),
            Some("loom_redis_data")
        );
    }
}

fn string_array_at(value: &Value, path: &[&str]) -> Option<Vec<String>> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_array().map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    })
}
