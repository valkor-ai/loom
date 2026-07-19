use contracts::{
    DeploymentRoute, DeploymentRuntimeContract, DeploymentShape, DeploymentSourceModel,
    DeploymentSourceService, DeploymentTopology, DeploymentTopologyValidation, SourceServiceRole,
};

pub fn build_topology(
    runtime: &DeploymentRuntimeContract,
    source_model: &DeploymentSourceModel,
) -> DeploymentTopology {
    let preview_service = preview_service(source_model);
    let public_entry = preview_service
        .map(|service| service.service_id.clone())
        .unwrap_or_else(|| source_model.preview_service_id.clone());
    let mut routes = Vec::new();
    if let Some(service) = preview_service {
        if service.role == SourceServiceRole::Frontend && service.start_command.is_none() {
            routes.push(DeploymentRoute::StaticSpa {
                public_path: "/".to_string(),
                target_service_id: service.service_id.clone(),
            });
        }
    }
    if source_model.shape == DeploymentShape::FrontendAndBackend {
        if let Some(backend) = backend_service(source_model) {
            routes.push(DeploymentRoute::HttpProxy {
                public_path: api_base_path(runtime),
                target_service_id: backend.service_id.clone(),
                target_port: backend.port,
                preserve_path: runtime
                    .api_contract
                    .as_ref()
                    .map(|contract| contract.preserve_path)
                    .unwrap_or(true),
            });
        }
    }
    let mut preview_paths = vec![runtime.preview_path.clone()];
    if preview_paths.is_empty() {
        preview_paths.push("/".to_string());
    }
    DeploymentTopology {
        schema_version: 1,
        public_entry_service_id: public_entry,
        routes,
        validation: DeploymentTopologyValidation {
            preview_paths: dedupe_paths(preview_paths),
            api_probes: runtime
                .safe_http_probes
                .iter()
                .filter(|probe| probe.source == "accepted_api_contract")
                .cloned()
                .collect(),
        },
    }
}

pub fn proxy_target_service_ids(topology: &DeploymentTopology) -> Vec<String> {
    let mut ids = topology
        .routes
        .iter()
        .filter_map(|route| match route {
            DeploymentRoute::HttpProxy {
                target_service_id, ..
            } => Some(target_service_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn preview_service(model: &DeploymentSourceModel) -> Option<&DeploymentSourceService> {
    model
        .services
        .iter()
        .find(|service| service.service_id == model.preview_service_id)
        .or_else(|| model.services.first())
}

fn backend_service(model: &DeploymentSourceModel) -> Option<&DeploymentSourceService> {
    model
        .services
        .iter()
        .find(|service| service.service_id == model.primary_service_id)
        .or_else(|| {
            model
                .services
                .iter()
                .find(|service| service.role == SourceServiceRole::Backend)
        })
        .or_else(|| {
            model
                .services
                .iter()
                .find(|service| service.service_id != model.preview_service_id)
        })
        .or_else(|| model.services.first())
}

fn api_base_path(runtime: &DeploymentRuntimeContract) -> String {
    if let Some(base) = runtime
        .api
        .as_ref()
        .and_then(|api| api.base_path.as_ref())
        .map(|path| normalize_path(path))
    {
        if base != "/" {
            return base;
        }
    }
    runtime
        .safe_http_probes
        .iter()
        .find(|probe| normalize_path(&probe.path) != "/")
        .and_then(|probe| {
            normalize_path(&probe.path)
                .split('/')
                .nth(1)
                .map(|segment| format!("/{segment}"))
        })
        .unwrap_or_else(|| "/api".to_string())
}

fn dedupe_paths(paths: Vec<String>) -> Vec<String> {
    let mut paths = paths
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
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
