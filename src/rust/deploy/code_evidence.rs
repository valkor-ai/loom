use std::{fs, path::Path};

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
    let service_gradle = project_root.join("service/build.gradle");
    let service_gradlew = project_root.join("service/gradlew");
    let service_pom = project_root.join("service/pom.xml");
    let service_mvnw = project_root.join("service/mvnw");
    let web_package = project_root.join("web/package.json");
    let root_package = project_root.join("package.json");
    let root_pom = project_root.join("pom.xml");
    let root_gradle = project_root.join("build.gradle");

    let has_service_gradle = service_gradle.exists() || service_gradlew.exists();
    let has_service_maven = service_pom.exists() || service_mvnw.exists();
    let has_root_gradle = root_gradle.exists() || project_root.join("gradlew").exists();
    let has_root_maven = root_pom.exists() || project_root.join("mvnw").exists();
    let has_web = web_package.exists();
    let has_root_node = root_package.exists();

    if has_service_gradle || has_service_maven {
        let package_manager = if has_service_gradle {
            Some(PackageManager::Gradle)
        } else {
            Some(PackageManager::Maven)
        };
        let has_lockfile = has_node_lockfile(project_root.join("web").as_path());
        let port = read_server_port(
            &project_root.join("service/src/main/resources/application.properties"),
        )
        .unwrap_or(8080);
        let frontend_package_refs = if has_web {
            vec!["web/package.json".to_string()]
        } else {
            vec![]
        };
        let framework = if has_web {
            Some("spring-boot+vite-react".to_string())
        } else {
            Some("spring-boot".to_string())
        };
        let build_command = if has_service_gradle {
            Some("cd service && chmod +x ./gradlew && ./gradlew bootJar --no-daemon".to_string())
        } else {
            Some("cd service && ./mvnw -DskipTests package".to_string())
        };
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Java,
            package_manager,
            has_lockfile,
            framework,
            runtime_version: None,
            runtime_version_source: None,
            build_command,
            start_command: None,
            output_directory: has_web.then_some("web/dist".to_string()),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: frontend_package_refs.clone(),
            services: vec![],
            evidence: evidence_value(
                project_root,
                "multi_application",
                RuntimeKind::Java,
                package_manager,
                port,
                vec![
                    file_fact(project_root, "service/build.gradle", "backend_build_file"),
                    file_fact(project_root, "service/gradlew", "backend_build_wrapper"),
                    file_fact(project_root, "service/pom.xml", "backend_build_file"),
                    file_fact(project_root, "service/mvnw", "backend_build_wrapper"),
                    file_fact(project_root, "web/package.json", "frontend_package_file"),
                    file_fact(project_root, "web/package-lock.json", "frontend_lockfile"),
                    file_fact(project_root, "web/vite.config.ts", "frontend_config"),
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
            build_command: Some(if has_root_gradle {
                "chmod +x ./gradlew && ./gradlew bootJar --no-daemon".to_string()
            } else {
                "./mvnw -DskipTests package".to_string()
            }),
            start_command: None,
            output_directory: None,
            port: 8080,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: vec![],
            services: vec![],
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

    if has_root_node || has_web {
        let root = if has_root_node { "." } else { "web" };
        let package_root = project_root.join(root);
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
            output_directory: Some("dist".to_string()),
            port: 5173,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.to_string()),
            workspace_package_json_paths: vec![package_path.clone()],
            services: vec![],
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
                ],
                vec![package_path],
            )?,
        });
    }

    Ok(DeploymentCodeProbe::unknown())
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
