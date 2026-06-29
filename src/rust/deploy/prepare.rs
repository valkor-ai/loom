use std::{
    collections::BTreeMap,
    net::TcpListener,
    path::{Path, PathBuf},
};

use contracts::{
    DeployProvider, DeploymentBootstrapDiagnostics, DeploymentEnvDiagnostics,
    DeploymentEnvVariable, DeploymentSpec,
};
use delivery_core::{
    LoomMcpActionResult, LoomMcpBlockedResult, LoomMcpDoneResult, LoomMcpFailure,
    LoomMcpFailureResult,
};
use serde_json::json;
use state::{
    lifecycle_store::init_project_state,
    paths::to_project_relative,
    store::{
        ensure_dir, now_string, write_json_atomic, write_text_atomic, StateError, StateResult,
    },
};

use crate::{
    active_operation::{acquire_operation, active_operation_result},
    generate::{deployment_runtime, generate_deployment_files, generated_file_refs},
    paths::deployment_paths,
    runtime_contract::load_runtime_contract,
    source_model::source_model_from_runtime_contract,
    topology::build_topology,
    DeployToolInput,
};

pub fn deploy_prepare(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root_buf = PathBuf::from(&input.project_root);
    let project_root = project_root_buf.as_path();
    let guard = match acquire_operation(project_root, "deploy.prepare", "preparing") {
        Ok(Ok(guard)) => guard,
        Ok(Err(operation)) => return active_operation_result(project_root, operation),
        Err(error) => return state_failed(&input.project_root, error),
    };
    let result = deploy_prepare_inner(project_root, input);
    drop(guard);
    match result {
        Ok(result) => result,
        Err(error) => runtime_contract_blocked(project_root, error),
    }
}

pub fn deploy_prepare_inner(
    project_root: &Path,
    _input: DeployToolInput,
) -> StateResult<LoomMcpActionResult> {
    init_project_state(&project_root.to_string_lossy())?;
    let paths = deployment_paths(project_root);
    ensure_dir(&paths.specs_dir)?;
    ensure_dir(&paths.generated_dir)?;
    ensure_dir(&paths.evidence_dir)?;
    ensure_dir(&paths.state_dir)?;
    ensure_dir(&paths.logs_dir)?;

    let runtime_contract = load_runtime_contract(project_root)?;
    let source_model = source_model_from_runtime_contract(&runtime_contract);
    if source_model.shape == contracts::DeploymentShape::FrontendAndBackend
        && source_model.services.len() < 2
    {
        return Err(StateError::StateCorrupted(
            "frontend-and-backend SourceModel must contain frontend and backend services."
                .to_string(),
        ));
    }
    let topology = build_topology(&runtime_contract, &source_model);
    let generated_refs = generated_file_refs(project_root, &source_model, &topology)?;
    let runtime_contract_ref = to_project_relative(
        project_root,
        &paths.generated_dir.join("runtime-contract.json"),
    )?;
    let source_model_ref =
        to_project_relative(project_root, &paths.generated_dir.join("source-model.json"))?;
    let topology_ref =
        to_project_relative(project_root, &paths.generated_dir.join("topology.json"))?;
    let code_evidence_ref = to_project_relative(project_root, &paths.code_evidence_file)?;
    let environment = env_diagnostics(&runtime_contract);
    let bootstrap = DeploymentBootstrapDiagnostics {
        tasks: vec![],
        warnings: vec![],
    };
    let host_port = find_host_port();
    let runtime = deployment_runtime(&runtime_contract, &source_model, host_port);
    let service_name = sanitize_name(
        project_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("loom-app"),
    );
    let spec = DeploymentSpec {
        schema_version: 1,
        provider: DeployProvider::Generated,
        provider_reason:
            "Generated from RuntimeDeliveryContract, DeploymentSourceModel, and DeploymentTopology."
                .to_string(),
        service_name: service_name.clone(),
        image_name: format!("{service_name}:loom-local"),
        project_root: project_root.to_string_lossy().into_owned(),
        generated_at: now_string(),
        runtime_contract_ref: runtime_contract_ref.clone(),
        source_model_ref: source_model_ref.clone(),
        topology_ref: topology_ref.clone(),
        code_evidence_ref: code_evidence_ref.clone(),
        runtime_contract,
        source_model,
        topology,
        environment,
        bootstrap,
        files: generated_refs,
        runtime,
    };
    write_json_atomic(
        &paths.generated_dir.join("runtime-contract.json"),
        &spec.runtime_contract,
    )?;
    write_json_atomic(
        &paths.generated_dir.join("source-model.json"),
        &spec.source_model,
    )?;
    write_json_atomic(&paths.generated_dir.join("topology.json"), &spec.topology)?;
    write_json_atomic(
        &paths.code_evidence_file,
        &json!({
            "schemaVersion": 1,
            "source": "runtime_delivery_contract",
            "generatedAt": now_string(),
            "runtimeContractRef": runtime_contract_ref,
            "sourceModelRef": source_model_ref,
            "topologyRef": topology_ref
        }),
    )?;
    let generated = generate_deployment_files(&spec);
    for (service_id, content) in &generated.dockerfiles {
        write_text_atomic(
            &crate::paths::dockerfile_path(project_root, service_id),
            content,
        )?;
    }
    for (service_id, content) in &generated.nginx_configs {
        write_text_atomic(
            &crate::paths::nginx_config_path(project_root, service_id),
            content,
        )?;
    }
    write_text_atomic(&paths.compose_file, &generated.compose)?;
    write_text_atomic(&paths.dockerignore_file, &generated.dockerignore)?;
    write_json_atomic(&paths.spec_file, &spec)?;

    Ok(LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root.to_string_lossy().into_owned(),
        summary: "Deployment assets prepared from RuntimeDeliveryContract.".to_string(),
        details: Some(json!({
            "specRef": to_project_relative(project_root, &paths.spec_file)?,
            "runtimeContractRef": spec.runtime_contract_ref,
            "sourceModelRef": spec.source_model_ref,
            "topologyRef": spec.topology_ref,
            "codeEvidenceRef": spec.code_evidence_ref,
            "sourceModel": {
                "shape": spec.source_model.shape,
                "primaryServiceId": spec.source_model.primary_service_id,
                "previewServiceId": spec.source_model.preview_service_id,
                "serviceIds": spec.source_model.services.iter().map(|service| service.service_id.clone()).collect::<Vec<_>>()
            },
            "topology": {
                "publicEntryServiceId": spec.topology.public_entry_service_id,
                "routes": spec.topology.routes,
                "previewPaths": spec.topology.validation.preview_paths,
                "apiPaths": spec.topology.validation.api_paths
            },
            "generatedFiles": spec.files,
            "url": spec.runtime.url
        })),
        warnings: vec![],
    }))
}

pub fn read_spec(project_root: &Path) -> StateResult<DeploymentSpec> {
    state::store::read_json(&deployment_paths(project_root).spec_file)
}

fn env_diagnostics(runtime: &contracts::DeploymentRuntimeContract) -> DeploymentEnvDiagnostics {
    let mut generated = BTreeMap::new();
    for dependency in &runtime.dependency_services {
        generated.extend(dependency.connection_env.clone());
    }
    let required = runtime
        .environment
        .required
        .iter()
        .map(|name| DeploymentEnvVariable {
            name: name.clone(),
            required: true,
            sensitive: name.to_ascii_uppercase().contains("PASSWORD")
                || name.to_ascii_uppercase().contains("SECRET")
                || name.to_ascii_uppercase().contains("TOKEN"),
            provided: std::env::var(name).is_ok() || generated.contains_key(name),
            generated: generated.contains_key(name),
            sources: vec!["runtime-contract".to_string()],
            reason: "Declared by RuntimeDeliveryContract.".to_string(),
        })
        .collect::<Vec<_>>();
    let missing = required
        .iter()
        .filter(|variable| !variable.provided)
        .cloned()
        .collect();
    DeploymentEnvDiagnostics {
        required,
        referenced: vec![],
        provided: vec![],
        generated,
        missing,
        warnings: vec![],
    }
}

fn find_host_port() -> u16 {
    for port in 4173..4300 {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    4173
}

fn sanitize_name(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while output.contains("--") {
        output = output.replace("--", "-");
    }
    output.trim_matches('-').to_string()
}

fn runtime_contract_blocked(project_root: &Path, error: StateError) -> LoomMcpActionResult {
    LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
        project_root: project_root.to_string_lossy().into_owned(),
        blockers: vec![error.to_string()],
        recommended_tool: Some("loom.continue".to_string()),
        details: Some(json!({
            "failureKind": "runtime_contract_missing",
            "reason": "Deploy is MCP-only and must use accepted AAC runtime_delivery instead of guessing a detected stack."
        })),
    })
}

fn state_failed(project_root: &str, error: StateError) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: "DEPLOY_PREPARE_FAILED".to_string(),
            message: error.to_string(),
            target_batch: Some(10),
            domain: Some("deploy".to_string()),
            route_action: None,
            recovery_tool: Some("loom.deployInspect".to_string()),
        },
    })
}
