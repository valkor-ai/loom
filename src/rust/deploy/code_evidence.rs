use std::{collections::BTreeMap, fs, path::Path};

use contracts::{DependencyService, PackageManager, RuntimeKind};
use serde_json::{json, Value};
use state::{
    paths::to_project_relative,
    store::{now_string, StateResult},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentCodeProbe {
    pub kind: RuntimeKind,
    pub package_manager: Option<PackageManager>,
    pub has_lockfile: bool,
    pub framework: Option<String>,
    pub runtime_version: Option<String>,
    pub runtime_version_source: Option<String>,
    pub build_command: Option<String>,
    pub start_command: Option<String>,
    pub output_directory: Option<String>,
    pub port: u16,
    pub healthcheck_path: Option<String>,
    pub working_directory: Option<String>,
    pub workspace_package_json_paths: Vec<String>,
    pub services: Vec<DependencyService>,
    pub env_defaults: BTreeMap<String, String>,
    pub spring_ddl_auto_validate: bool,
    pub flyway_detected: bool,
    pub evidence: Value,
}

impl DeploymentCodeProbe {
    pub fn unknown() -> Self {
        Self {
            kind: RuntimeKind::Unknown,
            package_manager: None,
            has_lockfile: false,
            framework: None,
            runtime_version: None,
            runtime_version_source: None,
            build_command: None,
            start_command: None,
            output_directory: None,
            port: 8080,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: vec![],
            services: vec![],
            env_defaults: BTreeMap::new(),
            spring_ddl_auto_validate: false,
            flyway_detected: false,
            evidence: json!({
                "schemaVersion": 1,
                "source": "code_probe",
                "runtimeFacts": {},
                "warnings": ["No deployable runtime files were detected."]
            }),
        }
    }
}

pub fn build_deployment_code_probe(project_root: &Path) -> StateResult<DeploymentCodeProbe> {
    let root_package = project_root.join("package.json");
    let root_pom = project_root.join("pom.xml");
    let root_gradle = project_root.join("build.gradle");

    let backend = find_java_backend_root(project_root);
    let frontend_root = find_frontend_root(project_root);
    let has_root_gradle = root_gradle.exists() || project_root.join("gradlew").exists();
    let has_root_maven = root_pom.exists() || project_root.join("mvnw").exists();
    let has_root_node = root_package.exists();
    let env_defaults = collect_env_defaults(project_root);
    let spring_ddl_auto_validate = spring_ddl_auto_validate(project_root);
    let flyway_detected = flyway_detected(project_root);

    if let Some(backend) = backend {
        let package_manager = Some(backend.package_manager);
        let frontend_package_refs = frontend_root
            .as_ref()
            .map(|root| vec![format!("{root}/package.json")])
            .unwrap_or_default();
        let has_lockfile = frontend_root
            .as_ref()
            .map(|root| has_node_lockfile(project_root.join(root).as_path()))
            .unwrap_or(false);
        let port = read_server_port(
            &project_root
                .join(&backend.root)
                .join("src/main/resources/application.properties"),
        )
        .unwrap_or(8080);
        let framework = Some("spring-boot".to_string());
        let build_command = Some(backend_build_command(&backend));
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Java,
            package_manager,
            has_lockfile,
            framework,
            runtime_version: None,
            runtime_version_source: None,
            build_command,
            start_command: None,
            output_directory: frontend_root.as_ref().map(|root| format!("{root}/dist")),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: frontend_package_refs.clone(),
            services: vec![],
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                "multi_application",
                RuntimeKind::Java,
                package_manager,
                port,
                vec![
                    file_fact(
                        project_root,
                        &format!("{}/build.gradle", backend.root),
                        "backend_build_file",
                    ),
                    file_fact(
                        project_root,
                        &format!("{}/gradlew", backend.root),
                        "backend_build_wrapper",
                    ),
                    file_fact(
                        project_root,
                        &format!("{}/pom.xml", backend.root),
                        "backend_build_file",
                    ),
                    file_fact(
                        project_root,
                        &format!("{}/mvnw", backend.root),
                        "backend_build_wrapper",
                    ),
                    frontend_root
                        .as_ref()
                        .map(|root| {
                            file_fact(
                                project_root,
                                &format!("{root}/package.json"),
                                "frontend_package_file",
                            )
                        })
                        .unwrap_or(Value::Null),
                    frontend_root
                        .as_ref()
                        .map(|root| {
                            file_fact(
                                project_root,
                                &format!("{root}/package-lock.json"),
                                "frontend_lockfile",
                            )
                        })
                        .unwrap_or(Value::Null),
                    frontend_root
                        .as_ref()
                        .map(|root| {
                            file_fact(
                                project_root,
                                &format!("{root}/vite.config.ts"),
                                "frontend_config",
                            )
                        })
                        .unwrap_or(Value::Null),
                    frontend_root
                        .as_ref()
                        .map(|root| {
                            file_fact(
                                project_root,
                                &format!("{root}/vite.config.js"),
                                "frontend_config",
                            )
                        })
                        .unwrap_or(Value::Null),
                ],
                frontend_package_refs,
            )?,
        });
    }

    if has_root_gradle || has_root_maven {
        let package_manager = if has_root_gradle {
            Some(PackageManager::Gradle)
        } else {
            Some(PackageManager::Maven)
        };
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Java,
            package_manager,
            has_lockfile: false,
            framework: Some("java".to_string()),
            runtime_version: None,
            runtime_version_source: None,
            build_command: Some(if project_root.join("gradlew").exists() {
                "chmod +x ./gradlew && ./gradlew bootJar --no-daemon".to_string()
            } else if has_root_gradle {
                "gradle bootJar --no-daemon".to_string()
            } else if project_root.join("mvnw").exists() {
                "./mvnw -DskipTests package".to_string()
            } else {
                "mvn -DskipTests package".to_string()
            }),
            start_command: None,
            output_directory: None,
            port: 8080,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: vec![],
            services: vec![],
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                "single_service",
                RuntimeKind::Java,
                package_manager,
                8080,
                vec![
                    file_fact(project_root, "build.gradle", "backend_build_file"),
                    file_fact(project_root, "gradlew", "backend_build_wrapper"),
                    file_fact(project_root, "pom.xml", "backend_build_file"),
                    file_fact(project_root, "mvnw", "backend_build_wrapper"),
                ],
                vec![],
            )?,
        });
    }

    if has_root_node || frontend_root.is_some() {
        let root = if has_root_node {
            ".".to_string()
        } else {
            frontend_root.unwrap_or_else(|| "frontend".to_string())
        };
        let package_root = project_root.join(&root);
        let package_manager = node_package_manager(&package_root);
        let has_lockfile = has_node_lockfile(&package_root);
        let package_path = if root == "." {
            "package.json".to_string()
        } else {
            format!("{root}/package.json")
        };
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Node,
            package_manager,
            has_lockfile,
            framework: Some("node".to_string()),
            runtime_version: None,
            runtime_version_source: None,
            build_command: Some(package_manager_run(
                package_manager.unwrap_or(PackageManager::Npm),
                "build",
            )),
            start_command: Some(package_manager_run(
                package_manager.unwrap_or(PackageManager::Npm),
                "preview",
            )),
            output_directory: Some(if root == "." {
                "dist".to_string()
            } else {
                format!("{root}/dist")
            }),
            port: 5173,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.to_string()),
            workspace_package_json_paths: vec![package_path.clone()],
            services: vec![],
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                "node_application",
                RuntimeKind::Node,
                package_manager,
                5173,
                vec![
                    file_fact(project_root, &package_path, "package_file"),
                    file_fact(
                        project_root,
                        &format!("{root}/package-lock.json").replace("./", ""),
                        "lockfile",
                    ),
                    file_fact(
                        project_root,
                        &format!("{root}/vite.config.ts").replace("./", ""),
                        "frontend_config",
                    ),
                    file_fact(
                        project_root,
                        &format!("{root}/vite.config.js").replace("./", ""),
                        "frontend_config",
                    ),
                ],
                vec![package_path],
            )?,
        });
    }

    Ok(DeploymentCodeProbe::unknown())
}

#[derive(Debug, Clone)]
struct JavaBackendRoot {
    root: String,
    package_manager: PackageManager,
    has_wrapper: bool,
}

fn find_java_backend_root(project_root: &Path) -> Option<JavaBackendRoot> {
    for root in ["service", "backend", "api", "server"] {
        let path = project_root.join(root);
        if path.join("build.gradle").exists() || path.join("gradlew").exists() {
            return Some(JavaBackendRoot {
                root: root.to_string(),
                package_manager: PackageManager::Gradle,
                has_wrapper: path.join("gradlew").exists(),
            });
        }
        if path.join("pom.xml").exists() || path.join("mvnw").exists() {
            return Some(JavaBackendRoot {
                root: root.to_string(),
                package_manager: PackageManager::Maven,
                has_wrapper: path.join("mvnw").exists(),
            });
        }
    }
    None
}

fn find_frontend_root(project_root: &Path) -> Option<String> {
    ["web", "frontend", "client", "app"]
        .into_iter()
        .find(|root| project_root.join(root).join("package.json").exists())
        .map(str::to_string)
}

fn backend_build_command(backend: &JavaBackendRoot) -> String {
    match (backend.package_manager, backend.has_wrapper) {
        (PackageManager::Gradle, true) => {
            format!(
                "cd {} && chmod +x ./gradlew && ./gradlew bootJar --no-daemon",
                backend.root
            )
        }
        (PackageManager::Gradle, false) => {
            format!("cd {} && gradle bootJar --no-daemon", backend.root)
        }
        (PackageManager::Maven, true) => {
            format!("cd {} && ./mvnw -DskipTests package", backend.root)
        }
        (PackageManager::Maven, false) => {
            format!("cd {} && mvn -DskipTests package", backend.root)
        }
        _ => format!("cd {} && true", backend.root),
    }
}

fn evidence_value(
    project_root: &Path,
    repository_shape: &str,
    runtime_kind: RuntimeKind,
    package_manager: Option<PackageManager>,
    port: u16,
    files: Vec<Value>,
    workspace_package_json_paths: Vec<String>,
) -> StateResult<Value> {
    Ok(json!({
        "schemaVersion": 1,
        "source": "code_probe",
        "generatedAt": now_string(),
        "repositoryShape": repository_shape,
        "runtimeFacts": {
            "kind": runtime_kind,
            "packageManager": package_manager,
            "port": port,
            "workspacePackageJsonPaths": workspace_package_json_paths
        },
        "files": files.into_iter().filter(|value| !value.is_null()).collect::<Vec<_>>(),
        "warnings": [],
        "projectRoot": project_root.to_string_lossy()
    }))
}

fn file_fact(project_root: &Path, relative: &str, kind: &str) -> Value {
    let path = project_root.join(relative);
    if !path.exists() {
        return Value::Null;
    }
    json!({
        "path": to_project_relative(project_root, &path).unwrap_or_else(|_| relative.to_string()),
        "kind": kind
    })
}

fn has_node_lockfile(root: &Path) -> bool {
    [
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lockb",
    ]
    .iter()
    .any(|name| root.join(name).exists())
}

fn collect_env_defaults(project_root: &Path) -> BTreeMap<String, String> {
    let mut defaults = BTreeMap::new();
    for relative in config_file_candidates(project_root) {
        let Ok(text) = fs::read_to_string(project_root.join(&relative)) else {
            continue;
        };
        collect_env_placeholders(&text, &mut defaults);
    }
    defaults
}

fn config_file_candidates(project_root: &Path) -> Vec<String> {
    let mut candidates = vec![
        "application.yml".to_string(),
        "application.yaml".to_string(),
        "application.properties".to_string(),
        ".env.example".to_string(),
    ];
    for root in ["backend", "service", "api", "server"] {
        for name in [
            "application.yml",
            "application.yaml",
            "application.properties",
        ] {
            candidates.push(format!("{root}/src/main/resources/{name}"));
        }
        candidates.push(format!("{root}/.env.example"));
    }
    candidates
        .into_iter()
        .filter(|relative| project_root.join(relative).is_file())
        .collect()
}

fn collect_env_placeholders(text: &str, defaults: &mut BTreeMap<String, String>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(start) = text[index..].find("${") {
        let start = index + start + 2;
        let Some(end_offset) = text[start..].find('}') else {
            break;
        };
        let end = start + end_offset;
        let expression = &text[start..end];
        if let Some((name, default_value)) = expression.split_once(':') {
            let name = name.trim();
            let default_value = default_value.trim();
            if is_env_name(name) && !default_value.is_empty() {
                defaults
                    .entry(name.to_string())
                    .or_insert_with(|| default_value.to_string());
            }
        }
        index = end + 1;
        if index >= bytes.len() {
            break;
        }
    }
}

fn is_env_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn spring_ddl_auto_validate(project_root: &Path) -> bool {
    config_file_candidates(project_root)
        .into_iter()
        .any(|relative| {
            fs::read_to_string(project_root.join(relative))
                .map(|text| {
                    let normalized = text.to_ascii_lowercase().replace([' ', '\t', '\r'], "");
                    normalized.contains("ddl-auto:validate")
                        || normalized.contains("ddl-auto=validate")
                        || normalized.contains("hibernate.ddl-auto=validate")
                })
                .unwrap_or(false)
        })
}

fn flyway_detected(project_root: &Path) -> bool {
    config_file_candidates(project_root)
        .into_iter()
        .any(|relative| {
            fs::read_to_string(project_root.join(relative))
                .map(|text| text.to_ascii_lowercase().contains("flyway"))
                .unwrap_or(false)
        })
        || [
            "pom.xml",
            "backend/pom.xml",
            "service/pom.xml",
            "api/pom.xml",
        ]
        .into_iter()
        .any(|relative| {
            fs::read_to_string(project_root.join(relative))
                .map(|text| text.to_ascii_lowercase().contains("flyway"))
                .unwrap_or(false)
        })
}

fn node_package_manager(root: &Path) -> Option<PackageManager> {
    if root.join("pnpm-lock.yaml").exists() {
        Some(PackageManager::Pnpm)
    } else if root.join("yarn.lock").exists() {
        Some(PackageManager::Yarn)
    } else if root.join("bun.lockb").exists() {
        Some(PackageManager::Bun)
    } else if root.join("package.json").exists() {
        Some(PackageManager::Npm)
    } else {
        None
    }
}

fn read_server_port(properties_file: &Path) -> Option<u16> {
    let text = fs::read_to_string(properties_file).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(value) = trimmed.strip_prefix("server.port=") else {
            continue;
        };
        if let Ok(port) = value.trim().parse::<u16>() {
            return Some(port);
        }
    }
    None
}

fn package_manager_run(package_manager: PackageManager, script: &str) -> String {
    match package_manager {
        PackageManager::Pnpm => format!("pnpm {script}"),
        PackageManager::Yarn => format!("yarn {script}"),
        PackageManager::Bun => format!("bun run {script}"),
        _ => format!("npm run {script}"),
    }
}
