use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use contracts::{
    DeployProvider, DeploymentEnvDiagnostics, DeploymentEnvVariable, DeploymentFacts,
    DeploymentGeneratedFiles, DeploymentProviderPolicy, DeploymentRuntimeContract,
    DeploymentSourceModel, DeploymentSpec, DeploymentTopology,
};
use delivery_core::{
    LoomMcpActionResult, LoomMcpBlockedResult, LoomMcpDoneResult, LoomMcpFailure,
    LoomMcpFailureResult,
};
use serde::de::DeserializeOwned;
use serde_json::json;
use state::{
    lifecycle_store::init_project_state,
    paths::{from_project_relative, to_project_relative},
    store::{
        ensure_dir, now_string, write_json_atomic, write_text_atomic, StateError, StateResult,
    },
};

use crate::{
    active_operation::{acquire_operation, active_operation_result},
    bootstrap::analyze_deployment_bootstrap,
    code_evidence::{build_deployment_code_probe, DeploymentCodeProbe},
    existing::{
        analyze_existing_compose, find_existing_deployment_files, selected_compose_port,
        ExistingDeploymentFiles,
    },
    facts::build_deployment_facts,
    generate::{generate_deployment_files, generated_file_refs},
    paths::{deployment_paths, DeploymentPaths},
    port_plan::{build_deployment_runtime, primary_url},
    references::reference_profile_value,
    runtime_contract::load_runtime_contract,
    source_model::{runtime_contract_declares_multi_root, source_model_from_runtime_contract},
    strategy::resolve_deployment_strategy,
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
    input: DeployToolInput,
) -> StateResult<LoomMcpActionResult> {
    init_project_state(&project_root.to_string_lossy())?;
    let paths = deployment_paths(project_root);
    ensure_dir(&paths.specs_dir)?;
    ensure_dir(&paths.generated_dir)?;
    ensure_dir(&paths.evidence_dir)?;
    ensure_dir(&paths.state_dir)?;
    ensure_dir(&paths.logs_dir)?;

    let mut runtime_contract = load_runtime_contract(project_root)?;
    let healthcheck_override = normalize_healthcheck_override(input.healthcheck.as_deref());
    if let Some(path) = &healthcheck_override {
        runtime_contract.health_path = Some(path.clone());
    }
    let requested_deployment_root = deployment_root_for(project_root, input.app_path.as_deref())?;
    let deployment_root = deployment_root_for_runtime_contract(
        project_root,
        requested_deployment_root,
        &runtime_contract,
    );
    let code_probe = build_deployment_code_probe(&deployment_root)?;
    let build_context_path = relative_context_from_generated_to_root(
        project_root,
        &paths.generated_dir,
        &deployment_root,
    );
    let source_model =
        source_model_from_runtime_contract(&runtime_contract, &code_probe, build_context_path);
    if source_model.shape == contracts::DeploymentShape::FrontendAndBackend
        && source_model.services.len() < 2
    {
        return Err(StateError::StateCorrupted(
            "frontend-and-backend SourceModel must contain frontend and backend services."
                .to_string(),
        ));
    }
    let existing = find_existing_deployment_files(&deployment_root);
    let strategy = resolve_deployment_strategy(
        &code_probe,
        &source_model,
        &existing,
        input.provider_policy.clone(),
    );
    validate_selected_provider(
        &strategy.policy,
        strategy.provider,
        &existing,
        &source_model,
    )?;
    let compose_info = if strategy.provider == DeployProvider::ComposeExisting {
        existing
            .compose_path
            .as_ref()
            .map(|path| analyze_existing_compose(path))
    } else {
        None
    };
    let compose_port = compose_info.as_ref().and_then(selected_compose_port);
    let mut source_model = if strategy.provider == DeployProvider::ComposeExisting {
        compose_port
            .as_ref()
            .map(|port| source_model_with_preview_port(source_model.clone(), port.container_port))
            .unwrap_or(source_model)
    } else {
        source_model
    };
    if let Some(path) = &healthcheck_override {
        apply_healthcheck_override(&mut source_model, path);
    }
    let topology = build_topology(&runtime_contract, &source_model);
    let runtime_contract_ref = to_project_relative(
        project_root,
        &paths.generated_dir.join("runtime-contract.json"),
    )?;
    let source_model_ref =
        to_project_relative(project_root, &paths.generated_dir.join("source-model.json"))?;
    let topology_ref =
        to_project_relative(project_root, &paths.generated_dir.join("topology.json"))?;
    let code_evidence_ref = to_project_relative(project_root, &paths.code_evidence_file)?;
    let facts_ref = to_project_relative(project_root, &paths.generated_dir.join("facts.json"))?;
    let environment = env_diagnostics(&runtime_contract, &code_probe);
    let bootstrap = analyze_deployment_bootstrap(project_root, &code_probe);
    let runtime = build_deployment_runtime(
        &runtime_contract,
        &source_model,
        &topology,
        compose_info.as_ref(),
    );
    let service_name = sanitize_name(
        deployment_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("loom-app"),
    );
    let files = deployment_files_for_provider(
        project_root,
        &paths,
        strategy.provider,
        &source_model,
        &topology,
        &existing,
    )?;
    let facts = build_deployment_facts(
        strategy.provider,
        &runtime_contract,
        &source_model,
        &topology,
        &runtime,
    );
    let spec = DeploymentSpec {
        schema_version: 1,
        provider: strategy.provider,
        provider_reason: strategy.reason,
        provider_policy: strategy.policy,
        provider_candidates: strategy.candidates,
        service_name: service_name.clone(),
        image_name: format!("{service_name}:loom-local"),
        project_root: project_root.to_string_lossy().into_owned(),
        generated_at: now_string(),
        runtime_contract_ref: runtime_contract_ref.clone(),
        source_model_ref: source_model_ref.clone(),
        topology_ref: topology_ref.clone(),
        code_evidence_ref: code_evidence_ref.clone(),
        facts_ref: facts_ref.clone(),
        runtime_contract,
        source_model,
        topology,
        facts,
        environment,
        bootstrap,
        compose: compose_info,
        files,
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
    write_json_atomic(&paths.generated_dir.join("facts.json"), &spec.facts)?;
    let mut code_evidence = code_probe.evidence.clone();
    if let Some(object) = code_evidence.as_object_mut() {
        object.insert(
            "runtimeContractRef".to_string(),
            serde_json::Value::String(runtime_contract_ref),
        );
        object.insert(
            "sourceModelRef".to_string(),
            serde_json::Value::String(source_model_ref),
        );
        object.insert(
            "topologyRef".to_string(),
            serde_json::Value::String(topology_ref),
        );
        object.insert("factsRef".to_string(), serde_json::Value::String(facts_ref));
    }
    write_json_atomic(&paths.code_evidence_file, &code_evidence)?;
    let generated = generate_deployment_files(&spec);
    match spec.provider {
        DeployProvider::ComposeExisting => {}
        DeployProvider::DockerfileExisting => {
            write_text_atomic(&paths.compose_file, &generated.compose)?;
        }
        DeployProvider::Generated => {
            for (service_id, content) in &generated.dockerfiles {
                write_text_atomic(
                    &crate::paths::dockerfile_path(project_root, service_id),
                    content,
                )?;
                write_text_atomic(
                    &crate::paths::dockerfile_ignore_path(project_root, service_id),
                    &generated.dockerignore,
                )?;
            }
            for (service_id, content) in &generated.nginx_configs {
                write_text_atomic(
                    &crate::paths::nginx_config_path(project_root, service_id),
                    content,
                )?;
            }
            write_text_atomic(&paths.compose_file, &generated.compose)?;
        }
    }
    write_json_atomic(&paths.spec_file, &spec)?;

    Ok(LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root.to_string_lossy().into_owned(),
        summary: format!(
            "Deployment prepared with {} provider.",
            provider_label(spec.provider)
        ),
        details: Some(deployment_prepare_details(project_root, &spec)?),
        warnings: vec![],
    }))
}

pub fn read_spec(project_root: &Path) -> StateResult<DeploymentSpec> {
    let paths = deployment_paths(project_root);
    let mut spec: DeploymentSpec = state::store::read_json(&paths.spec_file)?;
    overlay_generated_sidecars(&paths, &mut spec)?;
    Ok(spec)
}

fn overlay_generated_sidecars(
    paths: &DeploymentPaths,
    spec: &mut DeploymentSpec,
) -> StateResult<()> {
    if let Some(source_model) = read_generated_sidecar::<DeploymentSourceModel>(
        &paths.generated_dir.join("source-model.json"),
        "source-model.json",
    )? {
        spec.source_model = source_model;
    }
    if let Some(topology) = read_generated_sidecar::<DeploymentTopology>(
        &paths.generated_dir.join("topology.json"),
        "topology.json",
    )? {
        spec.topology = topology;
    }
    if let Some(facts) = read_generated_sidecar::<DeploymentFacts>(
        &paths.generated_dir.join("facts.json"),
        "facts.json",
    )? {
        spec.facts = facts;
    }
    Ok(())
}

fn read_generated_sidecar<T: DeserializeOwned>(path: &Path, label: &str) -> StateResult<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    state::store::read_json(path).map(Some).map_err(|error| {
        StateError::StateCorrupted(format!(
            "generated deployment sidecar {label} is invalid: {error}"
        ))
    })
}

fn apply_healthcheck_override(source_model: &mut DeploymentSourceModel, path: &str) {
    let target_service_id = source_model.primary_service_id.clone();
    let target_index = source_model
        .services
        .iter()
        .position(|service| service.service_id == target_service_id)
        .or_else(|| {
            source_model
                .services
                .iter()
                .position(|service| service.role != contracts::SourceServiceRole::Frontend)
        })
        .or_else(|| (!source_model.services.is_empty()).then_some(0));
    if let Some(service) = target_index.and_then(|index| source_model.services.get_mut(index)) {
        service.healthcheck_path = Some(path.to_string());
    }
    source_model.notes.push(format!(
        "Deployment healthcheck path was overridden by DeployToolInput.healthcheck: {path}."
    ));
}

fn normalize_healthcheck_override(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    let without_query = value.split(['?', '#']).next().unwrap_or(value).trim();
    let mut path = if let Some(index) = without_query.find("://") {
        let after_scheme = &without_query[index + 3..];
        after_scheme
            .find('/')
            .map(|path_start| &after_scheme[path_start..])
            .unwrap_or("/")
    } else if !without_query.starts_with('/') {
        without_query
            .find('/')
            .filter(|slash| {
                let prefix = &without_query[..*slash];
                prefix.contains(':') || prefix.contains('.')
            })
            .map(|slash| &without_query[slash..])
            .unwrap_or(without_query)
    } else {
        without_query
    }
    .trim()
    .to_string();
    if path.is_empty() {
        return None;
    }
    if !path.starts_with('/') {
        path = format!("/{path}");
    }
    if path.len() > 1 {
        path = path.trim_end_matches('/').to_string();
    }
    Some(path)
}

fn env_diagnostics(
    runtime: &contracts::DeploymentRuntimeContract,
    code_probe: &DeploymentCodeProbe,
) -> DeploymentEnvDiagnostics {
    let mut generated = BTreeMap::new();
    for dependency in &runtime.dependency_services {
        generated.extend(dependency.connection_env.clone());
    }
    for name in &runtime.environment.required {
        if std::env::var(name).is_ok() || generated.contains_key(name) {
            continue;
        }
        if let Some(default_value) = code_probe.env_defaults.get(name) {
            generated.insert(
                name.clone(),
                container_env_default(name, default_value).unwrap_or_else(|| default_value.clone()),
            );
        }
    }
    for (name, default_value) in &code_probe.env_defaults {
        if std::env::var(name).is_ok() || generated.contains_key(name) {
            continue;
        }
        if let Some(container_value) = container_env_default(name, default_value) {
            generated.insert(name.clone(), container_value);
        }
    }
    if should_disable_spring_ddl_validation(code_probe, &generated) {
        generated
            .entry("SPRING_JPA_HIBERNATE_DDL_AUTO".to_string())
            .or_insert_with(|| "none".to_string());
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

fn container_env_default(name: &str, value: &str) -> Option<String> {
    if !name.to_ascii_uppercase().contains("DATABASE") {
        return None;
    }
    containerize_file_database_url(value)
}

fn containerize_file_database_url(value: &str) -> Option<String> {
    for prefix in [
        "jdbc:sqlite:",
        "sqlite:",
        "jdbc:h2:file:",
        "jdbc:hsqldb:file:",
    ] {
        if let Some(path) = value.strip_prefix(prefix) {
            return containerize_database_path(prefix, path);
        }
    }
    if let Some(path) = value.strip_prefix("jdbc:derby:") {
        if !path.starts_with("//") {
            return containerize_database_path("jdbc:derby:", path);
        }
    }
    None
}

fn containerize_database_path(prefix: &str, path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty()
        || path.starts_with("/app/data/")
        || path.starts_with("http:")
        || path.starts_with("https:")
        || path.starts_with("tcp:")
        || path.starts_with("mem:")
        || path == ":memory:"
    {
        return None;
    }
    let (path_part, suffix) = split_database_url_suffix(path);
    let file_name = path_part
        .rsplit(['/', '\\'])
        .next()
        .filter(|item| !item.is_empty())
        .unwrap_or("app.db");
    Some(format!("{prefix}/app/data/{file_name}{suffix}"))
}

fn split_database_url_suffix(path: &str) -> (&str, &str) {
    let index = path.find(['?', ';']).unwrap_or(path.len());
    path.split_at(index)
}

fn should_disable_spring_ddl_validation(
    code_probe: &DeploymentCodeProbe,
    generated: &BTreeMap<String, String>,
) -> bool {
    code_probe.framework.as_deref() == Some("spring-boot")
        && code_probe.flyway_detected
        && code_probe.spring_ddl_auto_validate
        && generated.values().any(|value| {
            let lower = value.to_ascii_lowercase();
            containerize_file_database_url(&lower).is_some() || lower.contains("/app/data/")
        })
}

fn deployment_root_for(project_root: &Path, app_path: Option<&str>) -> StateResult<PathBuf> {
    let Some(app_path) = app_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(project_root.to_path_buf());
    };
    let root = from_project_relative(project_root, app_path)?;
    if !root.is_dir() {
        return Err(StateError::InvalidArgument(format!(
            "appPath must point to an existing directory: {app_path}"
        )));
    }
    Ok(root)
}

fn deployment_root_for_runtime_contract(
    project_root: &Path,
    requested_root: PathBuf,
    runtime: &DeploymentRuntimeContract,
) -> PathBuf {
    if requested_root != project_root && runtime_contract_declares_multi_root(runtime) {
        project_root.to_path_buf()
    } else {
        requested_root
    }
}

fn validate_selected_provider(
    policy: &DeploymentProviderPolicy,
    provider: DeployProvider,
    existing: &ExistingDeploymentFiles,
    source_model: &DeploymentSourceModel,
) -> StateResult<()> {
    match provider {
        DeployProvider::ComposeExisting if existing.compose_path.is_none() => {
            Err(StateError::InvalidArgument(
                "providerPolicy selected compose-existing, but no root-level Compose file was found."
                    .to_string(),
            ))
        }
        DeployProvider::DockerfileExisting if existing.dockerfile_path.is_none() => {
            Err(StateError::InvalidArgument(
                "providerPolicy selected dockerfile-existing, but no root-level Dockerfile was found."
                    .to_string(),
            ))
        }
        DeployProvider::DockerfileExisting if source_model.services.len() > 1 => {
            Err(StateError::InvalidArgument(
                "providerPolicy selected dockerfile-existing, but one root Dockerfile cannot represent multiple application services.".to_string(),
            ))
        }
        DeployProvider::Generated | DeployProvider::ComposeExisting | DeployProvider::DockerfileExisting => {
            if policy.force_generate && provider != DeployProvider::Generated {
                Err(StateError::InvalidArgument(
                    "forceGenerate can only select generated provider.".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    }
}

fn deployment_files_for_provider(
    project_root: &Path,
    paths: &DeploymentPaths,
    provider: DeployProvider,
    source_model: &DeploymentSourceModel,
    topology: &contracts::DeploymentTopology,
    existing: &ExistingDeploymentFiles,
) -> StateResult<DeploymentGeneratedFiles> {
    match provider {
        DeployProvider::Generated => generated_file_refs(project_root, source_model, topology),
        DeployProvider::DockerfileExisting => {
            let dockerfile_path = existing.dockerfile_path.as_ref().ok_or_else(|| {
                StateError::InvalidArgument(
                    "dockerfile-existing provider requires an existing Dockerfile.".to_string(),
                )
            })?;
            let dockerfile_ref = to_project_relative(project_root, dockerfile_path)?;
            let mut dockerfile_paths = BTreeMap::new();
            if let Some(service) = source_model.services.first() {
                dockerfile_paths.insert(service.service_id.clone(), dockerfile_ref.clone());
            }
            Ok(DeploymentGeneratedFiles {
                compose_path: to_project_relative(project_root, &paths.compose_file)?,
                dockerfile_paths,
                dockerignore_paths: BTreeMap::new(),
                nginx_config_paths: BTreeMap::new(),
                reused: vec![dockerfile_ref],
            })
        }
        DeployProvider::ComposeExisting => {
            let compose_path = existing.compose_path.as_ref().ok_or_else(|| {
                StateError::InvalidArgument(
                    "compose-existing provider requires an existing Compose file.".to_string(),
                )
            })?;
            let mut reused = vec![to_project_relative(project_root, compose_path)?];
            if let Some(dockerfile) = &existing.dockerfile_path {
                reused.push(to_project_relative(project_root, dockerfile)?);
            }
            reused.sort();
            reused.dedup();
            Ok(DeploymentGeneratedFiles {
                compose_path: to_project_relative(project_root, compose_path)?,
                dockerfile_paths: BTreeMap::new(),
                dockerignore_paths: BTreeMap::new(),
                nginx_config_paths: BTreeMap::new(),
                reused,
            })
        }
    }
}

fn source_model_with_preview_port(
    mut source_model: DeploymentSourceModel,
    container_port: u16,
) -> DeploymentSourceModel {
    for service in &mut source_model.services {
        if service.service_id == source_model.preview_service_id {
            service.port = container_port;
        }
    }
    source_model
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

fn relative_context_from_generated_to_root(
    project_root: &Path,
    generated_dir: &Path,
    deployment_root: &Path,
) -> String {
    let Ok(relative) = generated_dir.strip_prefix(project_root) else {
        return ".".to_string();
    };
    let depth = relative.components().count();
    let prefix = if depth == 0 {
        ".".to_string()
    } else {
        std::iter::repeat("..")
            .take(depth)
            .collect::<Vec<_>>()
            .join("/")
    };
    let Ok(deployment_relative) = deployment_root.strip_prefix(project_root) else {
        return prefix;
    };
    let deployment_relative = deployment_relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if deployment_relative.is_empty() {
        prefix
    } else if prefix == "." {
        deployment_relative
    } else {
        format!("{prefix}/{deployment_relative}")
    }
}

fn provider_label(provider: DeployProvider) -> &'static str {
    match provider {
        DeployProvider::ComposeExisting => "compose-existing",
        DeployProvider::DockerfileExisting => "dockerfile-existing",
        DeployProvider::Generated => "generated",
    }
}

pub(crate) fn deployment_prepare_details(
    project_root: &Path,
    spec: &DeploymentSpec,
) -> StateResult<serde_json::Value> {
    let paths = deployment_paths(project_root);
    Ok(json!({
        "specRef": to_project_relative(project_root, &paths.spec_file)?,
        "provider": spec.provider,
        "providerReason": spec.provider_reason,
        "providerCandidates": spec.provider_candidates.iter().map(|candidate| json!({
            "provider": candidate.provider,
            "status": candidate.status,
            "reason": candidate.reason.clone()
        })).collect::<Vec<_>>(),
        "runtimeContractRef": spec.runtime_contract_ref,
        "sourceModelRef": spec.source_model_ref,
        "topologyRef": spec.topology_ref,
        "codeEvidenceRef": spec.code_evidence_ref,
        "factsRef": spec.facts_ref,
        "deployFactsSummary": {
            "topologyClass": spec.facts.topology_class,
            "layoutKind": spec.facts.layout_kind,
            "stackKinds": spec.facts.stack_kinds,
            "serviceRoots": spec.facts.service_roots,
            "dependencyKinds": spec.facts.dependency_kinds,
            "publicPortCount": spec.facts.public_port_count,
            "internalPortCount": spec.facts.internal_port_count,
            "generatedAssetPolicy": spec.facts.generated_asset_policy
        },
        "sourceModelSummary": {
            "shape": spec.source_model.shape,
            "primaryServiceId": spec.source_model.primary_service_id,
            "previewServiceId": spec.source_model.preview_service_id,
            "serviceIds": spec.source_model.services.iter().map(|service| service.service_id.clone()).collect::<Vec<_>>()
        },
        "topologySummary": {
            "publicEntryServiceId": spec.topology.public_entry_service_id,
            "routeCount": spec.topology.routes.len(),
            "previewPaths": spec.topology.validation.preview_paths,
            "apiPaths": spec.topology.validation.api_paths
        },
        "composeSummary": spec.compose.as_ref().map(|compose| json!({
            "selectedService": compose.selected_service.clone(),
            "serviceReason": compose.service_reason.clone(),
            "serviceCount": compose.services.len(),
            "warnings": compose.warnings.clone()
        })),
        "generatedFileRefs": deployment_generated_file_refs(spec),
        "reusedFileRefs": spec.files.reused,
        "deployReferenceProfile": reference_profile_value(spec, None, false),
        "primaryUrl": primary_url(&spec.runtime),
        "ports": spec.runtime.ports.iter().map(|port| json!({
            "serviceId": port.service_id.clone(),
            "purpose": port.purpose.clone(),
            "containerPort": port.container_port,
            "preferredHostPort": port.preferred_host_port,
            "hostPort": port.host_port,
            "path": port.path.clone(),
            "internalOnly": port.internal_only,
            "protocol": port.protocol.clone(),
            "url": port.url.clone()
        })).collect::<Vec<_>>()
    }))
}

pub(crate) fn deployment_file_refs(spec: &DeploymentSpec) -> Vec<String> {
    let mut refs = vec![spec.files.compose_path.clone()];
    refs.extend(spec.files.dockerfile_paths.values().cloned());
    refs.extend(spec.files.dockerignore_paths.values().cloned());
    refs.extend(spec.files.nginx_config_paths.values().cloned());
    refs.sort();
    refs.dedup();
    refs
}

pub(crate) fn deployment_generated_file_refs(spec: &DeploymentSpec) -> Vec<String> {
    if spec.provider == DeployProvider::ComposeExisting {
        return vec![];
    }
    let reused = spec
        .files
        .reused
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let mut refs = deployment_file_refs(spec);
    refs.extend(deployment_generated_sidecar_refs(spec));
    refs.sort();
    refs.dedup();
    refs.into_iter()
        .filter(|item| !item.is_empty() && !reused.contains(item))
        .collect()
}

fn deployment_generated_sidecar_refs(spec: &DeploymentSpec) -> Vec<String> {
    [
        spec.source_model_ref.clone(),
        spec.topology_ref.clone(),
        spec.facts_ref.clone(),
    ]
    .into_iter()
    .filter(|item| !item.is_empty())
    .collect()
}

fn runtime_contract_blocked(project_root: &Path, error: StateError) -> LoomMcpActionResult {
    LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
        project_root: project_root.to_string_lossy().into_owned(),
        blockers: vec![error.to_string()],
        recommended_tool: Some("loom.deployInspect".to_string()),
        details: Some(json!({
            "failureKind": "runtime_contract_unavailable",
            "reason": "Deploy could not prepare a runtime contract from accepted AAC runtimeDelivery or repository code evidence."
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
