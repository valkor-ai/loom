use std::{collections::BTreeMap, path::Path};

use contracts::{
    DependencyService, DeploymentGeneratedFiles, DeploymentRoute, DeploymentRuntime,
    DeploymentRuntimeContract, DeploymentSourceModel, DeploymentSourceService, DeploymentSpec,
    DeploymentTopology, PackageManager, RuntimeKind, SourceServiceRole,
};
use state::paths::to_project_relative;

use crate::{paths, topology::proxy_target_service_ids};

#[derive(Debug, Clone)]
pub struct GeneratedDeploymentText {
    pub dockerfiles: BTreeMap<String, String>,
    pub nginx_configs: BTreeMap<String, String>,
    pub compose: String,
    pub dockerignore: String,
}

pub fn generated_file_refs(
    project_root: &Path,
    source_model: &DeploymentSourceModel,
    topology: &DeploymentTopology,
) -> state::store::StateResult<DeploymentGeneratedFiles> {
    let mut dockerfile_paths = BTreeMap::new();
    let mut nginx_config_paths = BTreeMap::new();
    for service in &source_model.services {
        dockerfile_paths.insert(
            service.service_id.clone(),
            to_project_relative(
                project_root,
                &paths::dockerfile_path(project_root, &service.service_id),
            )?,
        );
        if service.service_id == topology.public_entry_service_id
            && topology
                .routes
                .iter()
                .any(|route| matches!(route, DeploymentRoute::HttpProxy { .. }))
            && service.role == SourceServiceRole::Frontend
        {
            nginx_config_paths.insert(
                service.service_id.clone(),
                to_project_relative(
                    project_root,
                    &paths::nginx_config_path(project_root, &service.service_id),
                )?,
            );
        }
    }
    let deployment_paths = paths::deployment_paths(project_root);
    Ok(DeploymentGeneratedFiles {
        compose_path: to_project_relative(project_root, &deployment_paths.compose_file)?,
        dockerignore_path: to_project_relative(project_root, &deployment_paths.dockerignore_file)?,
        dockerfile_paths,
        nginx_config_paths,
        reused: vec![],
    })
}

pub fn deployment_runtime(
    runtime_contract: &DeploymentRuntimeContract,
    source_model: &DeploymentSourceModel,
    host_port: u16,
) -> DeploymentRuntime {
    let preview = source_model
        .services
        .iter()
        .find(|service| service.service_id == source_model.preview_service_id)
        .or_else(|| source_model.services.first());
    let container_port = preview.map(|service| service.port).unwrap_or(8080);
    DeploymentRuntime {
        host_port,
        container_port,
        url: format!("http://localhost:{host_port}"),
        preview_path: runtime_contract.preview_path.clone(),
        api_paths: runtime_contract.api_paths.clone(),
    }
}

pub fn generate_deployment_files(spec: &DeploymentSpec) -> GeneratedDeploymentText {
    GeneratedDeploymentText {
        dockerfiles: spec
            .source_model
            .services
            .iter()
            .map(|service| {
                (
                    service.service_id.clone(),
                    generate_dockerfile(service, spec),
                )
            })
            .collect(),
        nginx_configs: spec
            .files
            .nginx_config_paths
            .keys()
            .map(|service_id| (service_id.clone(), generate_nginx_config(spec)))
            .collect(),
        compose: generate_compose(spec),
        dockerignore: generate_dockerignore(),
    }
}

fn generate_dockerfile(service: &DeploymentSourceService, spec: &DeploymentSpec) -> String {
    if service.role == SourceServiceRole::Frontend && service.start_command.is_none() {
        return generate_static_frontend_dockerfile(service, spec);
    }
    match service.runtime_kind {
        RuntimeKind::Node => generate_node_dockerfile(service),
        RuntimeKind::Java => generate_java_dockerfile(service),
        RuntimeKind::Python => generate_python_dockerfile(service),
        RuntimeKind::Go => generate_go_dockerfile(service),
        RuntimeKind::Dotnet => generate_dotnet_dockerfile(service),
        _ => [
            "FROM alpine:3.20",
            "WORKDIR /app",
            "COPY . .",
            "CMD [\"sh\", \"-c\", \"echo 'Loom cannot determine a runnable stack from RuntimeDeliveryContract.' && exit 64\"]",
            "",
        ]
        .join("\n"),
    }
}

fn generate_static_frontend_dockerfile(
    service: &DeploymentSourceService,
    spec: &DeploymentSpec,
) -> String {
    let package_manager = service.package_manager.unwrap_or(PackageManager::Npm);
    let build_command = service
        .build_command
        .clone()
        .unwrap_or_else(|| package_manager_run(package_manager, "build"));
    let output_dir = service.output_directory.as_deref().unwrap_or("dist");
    let nginx_copy = spec
        .files
        .nginx_config_paths
        .get(&service.service_id)
        .map(|path| {
            format!(
                "COPY {} /etc/nginx/conf.d/default.conf",
                relative_from_context(path)
            )
        })
        .into_iter()
        .collect::<Vec<_>>();
    [
        "FROM node:22-alpine AS builder".to_string(),
        "WORKDIR /workspace".to_string(),
        "COPY . .".to_string(),
        service_root_workdir(service),
        format!(
            "RUN {}",
            install_command(package_manager, service.has_lockfile)
        ),
        format!("RUN {build_command}"),
        "".to_string(),
        "FROM nginx:1.27-alpine AS runner".to_string(),
        nginx_copy.join("\n"),
        format!("COPY --from=builder /workspace/{output_dir} /usr/share/nginx/html"),
        "EXPOSE 80".to_string(),
        "".to_string(),
    ]
    .into_iter()
    .filter(|line| !line.is_empty() || true)
    .collect::<Vec<_>>()
    .join("\n")
}

fn generate_node_dockerfile(service: &DeploymentSourceService) -> String {
    let package_manager = service.package_manager.unwrap_or(PackageManager::Npm);
    let start_command = service.start_command.clone().unwrap_or_else(|| {
        "echo 'No Node start command declared in RuntimeDeliveryContract.' && exit 64".to_string()
    });
    [
        "FROM node:22-alpine AS runner".to_string(),
        "WORKDIR /app".to_string(),
        "ENV NODE_ENV=production".to_string(),
        format!("ENV PORT={}", service.port),
        "COPY . .".to_string(),
        service_root_workdir(service),
        format!(
            "RUN {}",
            install_command(package_manager, service.has_lockfile)
        ),
        service
            .build_command
            .as_ref()
            .map(|command| format!("RUN {command}"))
            .unwrap_or_default(),
        format!("EXPOSE {}", service.port),
        format!("CMD {}", json_shell_cmd(&start_command)),
        "".to_string(),
    ]
    .into_iter()
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn generate_java_dockerfile(service: &DeploymentSourceService) -> String {
    let build_command = service
        .build_command
        .clone()
        .unwrap_or_else(|| "mvn -DskipTests package".to_string());
    let start_command = service
        .start_command
        .clone()
        .unwrap_or_else(|| "java -jar /app/app.jar".to_string());
    [
        "FROM maven:3-eclipse-temurin-21 AS builder".to_string(),
        "WORKDIR /workspace".to_string(),
        "COPY . .".to_string(),
        service_root_workdir(service),
        format!("RUN {build_command}"),
        "RUN JAR=\"$(find . -type f -name '*.jar' ! -name '*-plain.jar' | sort | head -n 1)\" && test -n \"$JAR\" && cp \"$JAR\" /workspace/app.jar".to_string(),
        "".to_string(),
        "FROM eclipse-temurin:21-jre AS runner".to_string(),
        "WORKDIR /app".to_string(),
        format!("ENV PORT={}", service.port),
        format!("ENV SERVER_PORT={}", service.port),
        "COPY --from=builder /workspace/app.jar /app/app.jar".to_string(),
        format!("EXPOSE {}", service.port),
        format!("CMD {}", json_shell_cmd(&start_command)),
        "".to_string(),
    ]
    .into_iter()
    .filter(|line| !line.is_empty() || true)
    .collect::<Vec<_>>()
    .join("\n")
}

fn generate_python_dockerfile(service: &DeploymentSourceService) -> String {
    let start_command = service.start_command.clone().unwrap_or_else(|| {
        "echo 'No Python start command declared in RuntimeDeliveryContract.' && exit 64".to_string()
    });
    [
        "FROM python:3.13-slim AS runner".to_string(),
        "WORKDIR /app".to_string(),
        "ENV PYTHONDONTWRITEBYTECODE=1".to_string(),
        "ENV PYTHONUNBUFFERED=1".to_string(),
        format!("ENV PORT={}", service.port),
        "COPY . .".to_string(),
        service_root_workdir(service),
        "RUN if [ -f requirements.txt ]; then pip install --no-cache-dir -r requirements.txt; fi"
            .to_string(),
        format!("EXPOSE {}", service.port),
        format!("CMD {}", json_shell_cmd(&start_command)),
        "".to_string(),
    ]
    .join("\n")
}

fn generate_go_dockerfile(service: &DeploymentSourceService) -> String {
    let start_command = service
        .start_command
        .clone()
        .unwrap_or_else(|| "/app/server".to_string());
    [
        "FROM golang:1.23-alpine AS builder",
        "WORKDIR /src",
        "COPY . .",
        "RUN go build -o /out/server .",
        "",
        "FROM alpine:3.20 AS runner",
        "WORKDIR /app",
        "COPY --from=builder /out/server /app/server",
        &format!("EXPOSE {}", service.port),
        &format!("CMD {}", json_shell_cmd(&start_command)),
        "",
    ]
    .join("\n")
}

fn generate_dotnet_dockerfile(service: &DeploymentSourceService) -> String {
    let start_command = service
        .start_command
        .clone()
        .unwrap_or_else(|| "dotnet /app/app.dll".to_string());
    [
        "FROM mcr.microsoft.com/dotnet/sdk:9.0 AS build".to_string(),
        "WORKDIR /src".to_string(),
        "COPY . .".to_string(),
        "RUN dotnet restore".to_string(),
        "RUN dotnet publish -c Release -o /app/publish --no-restore".to_string(),
        "FROM mcr.microsoft.com/dotnet/aspnet:9.0 AS runner".to_string(),
        "WORKDIR /app".to_string(),
        format!("ENV ASPNETCORE_URLS=http://0.0.0.0:{}", service.port),
        "COPY --from=build /app/publish .".to_string(),
        format!("EXPOSE {}", service.port),
        format!("CMD {}", json_shell_cmd(&start_command)),
        "".to_string(),
    ]
    .join("\n")
}

fn generate_compose(spec: &DeploymentSpec) -> String {
    let mut lines = vec!["services:".to_string()];
    for service in &spec.source_model.services {
        lines.extend(generate_app_service(spec, service));
    }
    for dependency in &spec.source_model.dependencies {
        lines.extend(generate_dependency_service(dependency));
    }
    let volumes = spec
        .source_model
        .dependencies
        .iter()
        .filter_map(|dependency| dependency.volume_name.as_ref())
        .collect::<Vec<_>>();
    if !volumes.is_empty() {
        lines.push("volumes:".to_string());
        for volume in volumes {
            lines.push(format!("  {volume}:"));
        }
    }
    lines.join("\n")
}

fn generate_app_service(spec: &DeploymentSpec, service: &DeploymentSourceService) -> Vec<String> {
    let dockerfile = spec
        .files
        .dockerfile_paths
        .get(&service.service_id)
        .cloned()
        .unwrap_or_else(|| {
            format!(
                ".loom/deployment/specs/generated/Dockerfile.{}",
                service.service_id
            )
        });
    let mut env = runtime_env(service);
    if service.role != SourceServiceRole::Frontend {
        for dependency in &spec.source_model.dependencies {
            env.extend(dependency.connection_env.clone());
        }
        env.extend(spec.environment.generated.clone());
    }
    let depends_on = compose_depends_on(spec, service);
    let mut lines = vec![
        format!("  {}:", service.service_id),
        "    build:".to_string(),
        format!(
            "      context: {}",
            yaml_string(&spec.source_model.build_context_path)
        ),
        format!("      dockerfile: {}", yaml_string(&dockerfile)),
        format!("    image: {}-{}", spec.image_name, service.service_id),
    ];
    if service.service_id == spec.source_model.preview_service_id {
        lines.push("    ports:".to_string());
        lines.push(format!(
            "      - \"{}:{}\"",
            spec.runtime.host_port, service.port
        ));
    }
    lines.extend(yaml_environment(&env, 4));
    if service.start_command.is_some() {
        lines.push("    healthcheck:".to_string());
        lines.push(format!(
            "      test: [\"CMD-SHELL\", \"wget -qO- http://127.0.0.1:{}{} >/dev/null 2>&1 || exit 1\"]",
            service.port,
            service.healthcheck_path.as_deref().unwrap_or("/")
        ));
        lines.push("      interval: 10s".to_string());
        lines.push("      timeout: 3s".to_string());
        lines.push("      retries: 6".to_string());
        lines.push("      start_period: 10s".to_string());
    }
    if !depends_on.is_empty() {
        lines.push("    depends_on:".to_string());
        for dependency in depends_on {
            lines.push(format!("      - {dependency}"));
        }
    }
    lines.push("    restart: unless-stopped".to_string());
    lines.push(String::new());
    lines
}

fn compose_depends_on(spec: &DeploymentSpec, service: &DeploymentSourceService) -> Vec<String> {
    let mut dependencies = Vec::new();
    if service.service_id == spec.topology.public_entry_service_id {
        dependencies.extend(proxy_target_service_ids(&spec.topology));
    }
    if service.role != SourceServiceRole::Frontend {
        dependencies.extend(
            spec.source_model
                .dependencies
                .iter()
                .map(|dependency| dependency.service_name.clone()),
        );
    }
    dependencies.sort();
    dependencies.dedup();
    dependencies
        .into_iter()
        .filter(|dependency| dependency != &service.service_id)
        .collect()
}

fn generate_dependency_service(service: &DependencyService) -> Vec<String> {
    let mut lines = vec![
        format!("  {}:", service.service_name),
        format!("    image: {}", service.image),
    ];
    lines.extend(yaml_environment(&service.env, 4));
    lines.push("    expose:".to_string());
    lines.push(format!("      - \"{}\"", service.port));
    if let Some(volume) = &service.volume_name {
        lines.push("    volumes:".to_string());
        lines.push(format!(
            "      - {}:{}",
            volume,
            service.volume_target.as_deref().unwrap_or("/data")
        ));
    }
    lines.push(String::new());
    lines
}

fn generate_nginx_config(spec: &DeploymentSpec) -> String {
    let mut lines = vec![
        "server {".to_string(),
        "  listen 80;".to_string(),
        "  server_name localhost;".to_string(),
        "  root /usr/share/nginx/html;".to_string(),
        "  index index.html;".to_string(),
        "".to_string(),
    ];
    for route in &spec.topology.routes {
        if let DeploymentRoute::HttpProxy {
            public_path,
            target_service_id,
            target_port,
            ..
        } = route
        {
            lines.extend(nginx_proxy_location_lines(
                public_path,
                target_service_id,
                *target_port,
            ));
        }
    }
    lines.extend([
        "  location / {".to_string(),
        "    try_files $uri $uri/ /index.html;".to_string(),
        "  }".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    lines.join("\n")
}

fn nginx_proxy_location_lines(
    public_path: &str,
    target_service_id: &str,
    target_port: u16,
) -> Vec<String> {
    let prefix = normalize_nginx_public_path(public_path);
    let slash_path = if prefix == "/" {
        "/".to_string()
    } else {
        format!("{prefix}/")
    };
    let mut lines = Vec::new();
    if prefix != "/" {
        lines.push(format!("  location = {prefix} {{"));
        lines.extend(nginx_proxy_pass_lines(target_service_id, target_port));
        lines.push("  }".to_string());
        lines.push(String::new());
    }
    lines.push(format!("  location {slash_path} {{"));
    lines.extend(nginx_proxy_pass_lines(target_service_id, target_port));
    lines.push("  }".to_string());
    lines.push(String::new());
    lines
}

fn nginx_proxy_pass_lines(target_service_id: &str, target_port: u16) -> Vec<String> {
    vec![
        format!("    proxy_pass http://{target_service_id}:{target_port};"),
        "    proxy_http_version 1.1;".to_string(),
        "    proxy_set_header Host $host;".to_string(),
        "    proxy_set_header X-Real-IP $remote_addr;".to_string(),
        "    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;".to_string(),
        "    proxy_set_header X-Forwarded-Proto $scheme;".to_string(),
    ]
}

fn generate_dockerignore() -> String {
    [
        ".git",
        ".loom/deployment/state",
        ".loom/deployment/logs",
        ".loom/tmp",
        "node_modules",
        "dist",
        "build",
        "target",
        ".venv",
        "__pycache__",
        "*.log",
        ".env",
        ".env.*",
        "!.env.example",
        "",
    ]
    .join("\n")
}

fn runtime_env(service: &DeploymentSourceService) -> BTreeMap<String, String> {
    match service.runtime_kind {
        RuntimeKind::Node => BTreeMap::from([
            ("NODE_ENV".to_string(), "production".to_string()),
            ("PORT".to_string(), service.port.to_string()),
        ]),
        RuntimeKind::Java => BTreeMap::from([
            ("PORT".to_string(), service.port.to_string()),
            ("SERVER_PORT".to_string(), service.port.to_string()),
        ]),
        RuntimeKind::Python
        | RuntimeKind::Go
        | RuntimeKind::Dotnet
        | RuntimeKind::Php
        | RuntimeKind::Ruby => BTreeMap::from([("PORT".to_string(), service.port.to_string())]),
        RuntimeKind::Static | RuntimeKind::Unknown => BTreeMap::new(),
    }
}

fn yaml_environment(values: &BTreeMap<String, String>, indent: usize) -> Vec<String> {
    let prefix = " ".repeat(indent);
    if values.is_empty() {
        return vec![format!("{prefix}environment: {{}}")];
    }
    let mut lines = vec![format!("{prefix}environment:")];
    for (key, value) in values {
        lines.push(format!("{prefix}  {key}: {}", yaml_string(value)));
    }
    lines
}

fn yaml_string(value: &str) -> String {
    serde_yaml::to_string(value)
        .unwrap_or_else(|_| format!("{value:?}"))
        .trim()
        .trim_start_matches("---")
        .trim()
        .to_string()
}

fn json_shell_cmd(command: &str) -> String {
    serde_json::to_string(&["sh", "-c", command])
        .unwrap_or_else(|_| "[\"sh\",\"-c\",\"exit 64\"]".to_string())
}

fn install_command(package_manager: PackageManager, has_lockfile: bool) -> String {
    match package_manager {
        PackageManager::Npm => if has_lockfile {
            "npm ci"
        } else {
            "npm install"
        }
        .to_string(),
        PackageManager::Pnpm => if has_lockfile {
            "corepack enable && pnpm install --frozen-lockfile"
        } else {
            "corepack enable && pnpm install"
        }
        .to_string(),
        PackageManager::Yarn => if has_lockfile {
            "corepack enable && yarn install --frozen-lockfile"
        } else {
            "corepack enable && yarn install"
        }
        .to_string(),
        PackageManager::Bun => if has_lockfile {
            "bun install --frozen-lockfile"
        } else {
            "bun install"
        }
        .to_string(),
        PackageManager::Maven
        | PackageManager::Gradle
        | PackageManager::Pip
        | PackageManager::Poetry
        | PackageManager::Uv
        | PackageManager::Go
        | PackageManager::Dotnet
        | PackageManager::Composer
        | PackageManager::Bundler => "true".to_string(),
    }
}

fn package_manager_run(package_manager: PackageManager, script: &str) -> String {
    match package_manager {
        PackageManager::Pnpm => format!("pnpm {script}"),
        PackageManager::Yarn => format!("yarn {script}"),
        PackageManager::Bun => format!("bun run {script}"),
        _ => format!("npm run {script}"),
    }
}

fn service_root_workdir(service: &DeploymentSourceService) -> String {
    if service.root == "." {
        String::new()
    } else {
        format!("WORKDIR /workspace/{}", service.root)
    }
}

fn relative_from_context(path: &str) -> String {
    path.to_string()
}

fn normalize_nginx_public_path(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let path = trimmed.split(['?', '#']).next().unwrap_or("/");
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    path.trim_end_matches('/').to_string()
}
