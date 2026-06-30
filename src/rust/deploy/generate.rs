use std::{collections::BTreeMap, path::Path};

use contracts::{
    DependencyService, DeploymentGeneratedFiles, DeploymentRoute, DeploymentSourceModel,
    DeploymentSourceService, DeploymentSpec, DeploymentTopology, PackageManager, RuntimeKind,
    SourceServiceRole,
};
use state::paths::to_project_relative;

use crate::{paths, port_plan::host_port_for_service, topology::proxy_target_service_ids};

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
    if java_service_has_frontend_overlay(service) {
        return generate_java_with_frontend_dockerfile(service);
    }
    let build_command = service
        .build_command
        .clone()
        .unwrap_or_else(|| default_java_build_command(service));
    let start_command = service
        .start_command
        .clone()
        .unwrap_or_else(|| "java -jar /app/app.jar".to_string());
    let builder_image = java_builder_image(service);
    [
        format!("FROM {builder_image} AS builder"),
        "WORKDIR /workspace".to_string(),
        "COPY . .".to_string(),
        service_root_workdir(service),
        format!("RUN {build_command}"),
        "RUN JAR=\"$(find . -type f -name '*.jar' ! -name '*-plain.jar' | sort | head -n 1)\" && test -n \"$JAR\" && cp \"$JAR\" /workspace/app.jar".to_string(),
        "RUN mkdir -p /tmp/data && if [ -d data ]; then cp -R data/. /tmp/data/; elif [ -d service/data ]; then cp -R service/data/. /tmp/data/; fi".to_string(),
        "".to_string(),
        "FROM eclipse-temurin:21-jre AS runner".to_string(),
        "WORKDIR /app".to_string(),
        format!("ENV PORT={}", service.port),
        format!("ENV SERVER_PORT={}", service.port),
        "COPY --from=builder /workspace/app.jar /app/app.jar".to_string(),
        "COPY --from=builder /tmp/data /app/data".to_string(),
        format!("EXPOSE {}", service.port),
        format!("CMD {}", json_shell_cmd(&start_command)),
        "".to_string(),
    ]
    .into_iter()
    .filter(|line| !line.is_empty() || true)
    .collect::<Vec<_>>()
    .join("\n")
}

fn generate_java_with_frontend_dockerfile(service: &DeploymentSourceService) -> String {
    let frontend_root =
        frontend_root_from_package_refs(service).unwrap_or_else(|| "web".to_string());
    let frontend_output = service
        .output_directory
        .clone()
        .unwrap_or_else(|| format!("{frontend_root}/dist"));
    let build_command = service
        .build_command
        .clone()
        .unwrap_or_else(|| default_java_build_command(service));
    let builder_image = java_builder_image(service);
    [
        "FROM node:22-bookworm-slim AS web-builder".to_string(),
        format!("WORKDIR /workspace/{frontend_root}"),
        format!("COPY {frontend_root}/ ./"),
        format!("RUN {}", install_command(PackageManager::Npm, service.has_lockfile)),
        format!("RUN {}", package_manager_run(PackageManager::Npm, "build")),
        "".to_string(),
        format!("FROM {builder_image} AS service-builder"),
        "WORKDIR /workspace".to_string(),
        "COPY . .".to_string(),
        format!("RUN {build_command}"),
        "RUN mkdir -p /tmp/static-overlay/BOOT-INF/classes/static".to_string(),
        format!(
            "COPY --from=web-builder /workspace/{frontend_output}/ /tmp/static-overlay/BOOT-INF/classes/static/"
        ),
        "RUN JAR_PATH=\"$(find . -type f -name '*.jar' ! -name '*-plain.jar' | sort | head -n 1)\" && test -n \"$JAR_PATH\" && cp \"$JAR_PATH\" /tmp/app.jar && jar --update --file /tmp/app.jar -C /tmp/static-overlay BOOT-INF/classes/static".to_string(),
        "RUN mkdir -p /tmp/data && if [ -d service/data ]; then cp -R service/data/. /tmp/data/; elif [ -d data ]; then cp -R data/. /tmp/data/; fi".to_string(),
        "".to_string(),
        "FROM eclipse-temurin:21-jre AS runner".to_string(),
        "WORKDIR /app".to_string(),
        format!("ENV PORT={}", service.port),
        format!("ENV SERVER_PORT={}", service.port),
        "COPY --from=service-builder /tmp/app.jar /app/app.jar".to_string(),
        "COPY --from=service-builder /tmp/data /app/data".to_string(),
        format!("EXPOSE {}", service.port),
        "ENTRYPOINT [\"java\",\"-jar\",\"/app/app.jar\"]".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

fn java_service_has_frontend_overlay(service: &DeploymentSourceService) -> bool {
    service.runtime_kind == RuntimeKind::Java
        && !service.workspace_package_json_paths.is_empty()
        && service.output_directory.is_some()
}

fn frontend_root_from_package_refs(service: &DeploymentSourceService) -> Option<String> {
    service
        .workspace_package_json_paths
        .first()
        .and_then(|path| path.rsplit_once('/').map(|(root, _)| root.to_string()))
        .filter(|root| !root.is_empty())
}

fn default_java_build_command(service: &DeploymentSourceService) -> String {
    match service.package_manager {
        Some(PackageManager::Gradle) => {
            "chmod +x ./gradlew && ./gradlew bootJar --no-daemon".to_string()
        }
        _ => "mvn -DskipTests package".to_string(),
    }
}

fn java_builder_image(service: &DeploymentSourceService) -> &'static str {
    match service.package_manager {
        Some(PackageManager::Maven) => "maven:3-eclipse-temurin-21",
        _ => "eclipse-temurin:21-jdk",
    }
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
    let dockerfile_project_path = spec
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
    let compose_dir = Path::new(&spec.files.compose_path)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or(".");
    let build_context_dir = project_path_join(compose_dir, &spec.source_model.build_context_path);
    let dockerfile =
        project_path_relative_to_directory(&build_context_dir, &dockerfile_project_path);
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
    if let Some(host_port) = host_port_for_service(&spec.runtime, &service.service_id) {
        lines.push("    ports:".to_string());
        lines.push(format!("      - \"{}:{}\"", host_port, service.port));
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

fn project_path_relative_to_directory(
    from_project_relative_directory: &str,
    to_project_relative_path: &str,
) -> String {
    let from_parts = normalized_relative_parts(from_project_relative_directory);
    let to_parts = normalized_relative_parts(to_project_relative_path);
    let common_len = from_parts
        .iter()
        .zip(&to_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = Vec::new();
    relative.extend(std::iter::repeat("..").take(from_parts.len().saturating_sub(common_len)));
    relative.extend(to_parts.iter().skip(common_len).copied());
    if relative.is_empty() {
        ".".to_string()
    } else {
        relative.join("/")
    }
}

fn project_path_join(base_project_relative_directory: &str, relative_path: &str) -> String {
    let value = match (base_project_relative_directory, relative_path) {
        ("", path) | (".", path) => path.to_string(),
        (base, "") | (base, ".") => base.to_string(),
        (base, path) => format!("{base}/{path}"),
    };
    let parts = normalized_relative_parts(&value);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn normalized_relative_parts(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            let _ = parts.pop();
        } else {
            parts.push(part);
        }
    }
    parts
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
