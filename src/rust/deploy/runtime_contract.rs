use std::{collections::BTreeMap, path::Path};

use contracts::{
    DependencyService, DependencyServiceKind, DeploymentRuntimeContract, DeploymentShape,
    RuntimeContractApi, RuntimeContractEndpoint, RuntimeEnvironmentContract,
};
use delivery_core::{DeliveryIndex, DeliveryPhaseState, TransitionStore};
use serde_json::{json, Value};
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
        Err(_) => Ok(heuristic_runtime_contract()),
    }
}

pub fn runtime_contract_from_value(
    runtime: &Value,
    runtime_ref: Option<String>,
) -> StateResult<DeploymentRuntimeContract> {
    let status = string_at(runtime, &["status"]).unwrap_or_else(|| "modified".to_string());
    if status == "not_applicable" {
        return Err(StateError::InvalidArgument(
            "AAC runtimeDelivery is not_applicable; deploy cannot guess runtime stack.".to_string(),
        ));
    }
    let frontend = runtime.get("frontend").and_then(endpoint_from_value);
    let api = runtime.get("api").and_then(api_from_value);
    let shape = deployment_shape(runtime, frontend.as_ref(), api.as_ref());
    let http_probes = runtime.get("httpProbes").unwrap_or(&Value::Null);
    let preview_path = string_at(http_probes, &["previewPath"]).unwrap_or_else(|| "/".to_string());
    let api_paths = string_array_at(http_probes, &["apiPaths"])
        .or_else(|| api.as_ref().map(|api| api.probe_paths.clone()))
        .unwrap_or_default();
    let runtime_kind = string_at(runtime, &["runtimeKind"]);
    let build_command =
        string_at(runtime, &["build", "command"]).or_else(|| string_at(runtime, &["buildCommand"]));
    let start_command =
        string_at(runtime, &["start", "command"]).or_else(|| string_at(runtime, &["startCommand"]));
    let port = u16_at(runtime, &["start", "port"]).or_else(|| u16_at(runtime, &["port"]));
    let health_path = string_at(http_probes, &["healthPath"]);
    let environment = RuntimeEnvironmentContract {
        required: string_array_at(runtime, &["environment", "required"]).unwrap_or_default(),
        optional: string_array_at(runtime, &["environment", "optional"]).unwrap_or_default(),
    };
    let dependency_services = dependency_services_from_runtime(runtime);

    Ok(DeploymentRuntimeContract {
        source: "accepted_aac".to_string(),
        r#ref: runtime_ref,
        status,
        dependency_service_policy: "contract_only".to_string(),
        deployment_shape: Some(shape),
        runtime_kind,
        build_command,
        start_command,
        port,
        preview_path,
        health_path,
        api_paths,
        frontend_output_dir: frontend
            .as_ref()
            .and_then(|frontend| frontend.output_dir.clone())
            .or_else(|| string_at(runtime, &["frontendOutputDir"])),
        probe_kind: "http".to_string(),
        environment,
        frontend,
        api,
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
        build_command: string_at(value, &["buildCommand"]),
        source_root: string_at(value, &["sourceRoot"]),
        output_dir: string_at(value, &["outputDir"]),
        served_by: string_at(value, &["servedBy"]),
        served_by_ref: string_at(value, &["servedByRef"]),
    })
}

fn api_from_value(value: &Value) -> Option<RuntimeContractApi> {
    Some(RuntimeContractApi {
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        kind: string_at(value, &["kind"]),
        build_command: string_at(value, &["buildCommand"]),
        entry: string_at(value, &["entry"]),
        base_path: string_at(value, &["basePath"]),
        probe_paths: string_array_at(value, &["probePaths"]).unwrap_or_default(),
    })
}

fn deployment_shape(
    runtime: &Value,
    frontend: Option<&RuntimeContractEndpoint>,
    api: Option<&RuntimeContractApi>,
) -> DeploymentShape {
    if let Some(value) = string_at(runtime, &["deploymentShape"]) {
        let normalized = value.replace(['_', ' '], "-");
        if matches!(
            normalized.as_str(),
            "frontend-and-backend" | "frontend-backend" | "dual-service" | "multi-service"
        ) {
            return DeploymentShape::FrontendAndBackend;
        }
        if matches!(normalized.as_str(), "single-service" | "single") {
            return DeploymentShape::SingleService;
        }
    }
    if frontend.map(|item| item.required).unwrap_or(false)
        && api.map(|item| item.required).unwrap_or(false)
    {
        return DeploymentShape::FrontendAndBackend;
    }
    DeploymentShape::SingleService
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
        let Ok(contract) = runtime_contract_from_architecture_ref(project_root, architecture_ref)
        else {
            continue;
        };
        return Ok(contract);
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
    runtime_contract_from_value(
        runtime,
        Some(format!(
            "{}#/runtimeDelivery",
            to_project_relative(project_root, &aac_file)?
        )),
    )
}

fn heuristic_runtime_contract() -> DeploymentRuntimeContract {
    DeploymentRuntimeContract {
        source: "heuristic".to_string(),
        r#ref: None,
        status: "modified".to_string(),
        dependency_service_policy: "code_probe".to_string(),
        deployment_shape: None,
        runtime_kind: None,
        build_command: None,
        start_command: None,
        port: None,
        preview_path: "/".to_string(),
        health_path: None,
        api_paths: vec![],
        frontend_output_dir: None,
        probe_kind: "http".to_string(),
        environment: RuntimeEnvironmentContract {
            required: vec![],
            optional: vec![],
        },
        frontend: None,
        api: None,
        dependency_services: vec![],
    }
}

fn dependency_services_from_runtime(runtime: &Value) -> Vec<DependencyService> {
    let signals = serde_json::to_string(&json!({
        "runtimeKind": runtime.get("runtimeKind"),
        "environment": runtime.get("environment"),
        "deliveryMechanics": runtime.get("deliveryMechanics"),
        "api": runtime.get("api"),
        "httpProbes": runtime.get("httpProbes"),
    }))
    .unwrap_or_default()
    .to_ascii_lowercase();
    let mut services = Vec::new();
    if signals.contains("postgres")
        || signals.contains("postgresql")
        || signals.contains("jdbc:postgresql")
    {
        services.push(service_definition(DependencyServiceKind::Postgres));
    }
    if signals.contains("redis") {
        services.push(service_definition(DependencyServiceKind::Redis));
    }
    if signals.contains("mysql") || signals.contains("mariadb") {
        services.push(service_definition(DependencyServiceKind::Mysql));
    }
    services
}

fn service_definition(kind: DependencyServiceKind) -> DependencyService {
    match kind {
        DependencyServiceKind::Postgres => DependencyService {
            kind,
            service_name: "postgres".to_string(),
            image: "postgres:16-alpine".to_string(),
            port: 5432,
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
            reason: "Declared by RuntimeDeliveryContract runtime/environment signals.".to_string(),
        },
        DependencyServiceKind::Redis => DependencyService {
            kind,
            service_name: "redis".to_string(),
            image: "redis:7-alpine".to_string(),
            port: 6379,
            env: BTreeMap::new(),
            connection_env: BTreeMap::from([(
                "REDIS_URL".to_string(),
                "redis://redis:6379".to_string(),
            )]),
            volume_name: None,
            volume_target: None,
            reason: "Declared by RuntimeDeliveryContract runtime/environment signals.".to_string(),
        },
        DependencyServiceKind::Mysql => DependencyService {
            kind,
            service_name: "mysql".to_string(),
            image: "mysql:8".to_string(),
            port: 3306,
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
            reason: "Declared by RuntimeDeliveryContract runtime/environment signals.".to_string(),
        },
        _ => DependencyService {
            kind,
            service_name: "dependency".to_string(),
            image: "alpine:3.20".to_string(),
            port: 1,
            env: BTreeMap::new(),
            connection_env: BTreeMap::new(),
            volume_name: None,
            volume_target: None,
            reason: "Declared by RuntimeDeliveryContract runtime/environment signals.".to_string(),
        },
    }
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
