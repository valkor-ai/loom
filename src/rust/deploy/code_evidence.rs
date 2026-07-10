use std::{collections::BTreeMap, fs, path::Path};

use contracts::{DependencyService, DependencyServiceKind, PackageManager, RuntimeKind};
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
    let frontend_root = detected_frontend_root(project_root);
    let frontend_support = frontend_probe_evidence(project_root, frontend_root.as_deref());
    let has_root_gradle = root_gradle.exists() || project_root.join("gradlew").exists();
    let has_root_maven = root_pom.exists() || project_root.join("mvnw").exists();
    let has_root_node = root_package.exists();
    let has_frontend = frontend_root.is_some();
    let env_defaults = collect_env_defaults(project_root);
    let spring_ddl_auto_validate = spring_ddl_auto_validate(project_root);
    let flyway_detected = flyway_detected(project_root);
    let services = collect_dependency_services(project_root);

    if let Some(backend) = backend {
        let package_manager = Some(backend.package_manager);
        let port = read_server_port(
            &project_root
                .join(&backend.root)
                .join("src/main/resources/application.properties"),
        )
        .unwrap_or(8080);
        let framework = Some("spring-boot".to_string());
        let build_command = Some(backend_build_command(&backend));
        let mut files = vec![
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
        ];
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Java,
            package_manager,
            has_lockfile: frontend_support.has_lockfile,
            framework,
            runtime_version: None,
            runtime_version_source: None,
            build_command,
            start_command: None,
            output_directory: frontend_support.output_directory.clone(),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                "multi_application",
                RuntimeKind::Java,
                package_manager,
                port,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if has_root_gradle || has_root_maven {
        let package_manager = if has_root_gradle {
            Some(PackageManager::Gradle)
        } else {
            Some(PackageManager::Maven)
        };
        let mut files = vec![
            file_fact(project_root, "build.gradle", "backend_build_file"),
            file_fact(project_root, "gradlew", "backend_build_wrapper"),
            file_fact(project_root, "pom.xml", "backend_build_file"),
            file_fact(project_root, "mvnw", "backend_build_wrapper"),
        ];
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Java,
            package_manager,
            has_lockfile: frontend_support.has_lockfile,
            framework: Some(java_framework(project_root, ".").to_string()),
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
            output_directory: frontend_support.output_directory.clone(),
            port: 8080,
            healthcheck_path: Some("/".to_string()),
            working_directory: None,
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                if has_frontend {
                    "multi_application"
                } else {
                    "single_service"
                },
                RuntimeKind::Java,
                package_manager,
                8080,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if let Some(root) = find_stack_root(
        project_root,
        &[
            "requirements.txt",
            "pyproject.toml",
            "uv.lock",
            "poetry.lock",
            "Pipfile",
            "manage.py",
            "main.py",
            "app.py",
            "server.py",
        ],
    ) {
        let package_root = project_root.join(&root);
        let package_manager = python_package_manager(&package_root);
        let framework = python_framework(&package_root);
        let port = python_default_port(framework.as_deref());
        let mut files = stack_file_facts(
            project_root,
            &root,
            &[
                "requirements.txt",
                "pyproject.toml",
                "uv.lock",
                "poetry.lock",
                "manage.py",
                "main.py",
                "app.py",
                "server.py",
            ],
            "python_runtime_file",
        );
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Python,
            package_manager,
            has_lockfile: has_python_lockfile(&package_root),
            framework,
            runtime_version: None,
            runtime_version_source: None,
            build_command: None,
            start_command: Some(python_start_command(&package_root, port)),
            output_directory: frontend_support.output_directory.clone(),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.clone()),
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                if has_frontend {
                    "multi_application"
                } else {
                    "single_service"
                },
                RuntimeKind::Python,
                package_manager,
                port,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if let Some(root) = find_stack_root(project_root, &["go.mod", "main.go"]) {
        let package_root = project_root.join(&root);
        let port = go_default_port(&package_root);
        let mut files = stack_file_facts(
            project_root,
            &root,
            &["go.mod", "go.sum", "main.go"],
            "go_runtime_file",
        );
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Go,
            package_manager: Some(PackageManager::Go),
            has_lockfile: project_root.join(&root).join("go.sum").exists(),
            framework: Some("go".to_string()),
            runtime_version: None,
            runtime_version_source: None,
            build_command: Some("go build -o /out/server .".to_string()),
            start_command: Some("/app/server".to_string()),
            output_directory: frontend_support.output_directory.clone(),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.clone()),
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                if has_frontend {
                    "multi_application"
                } else {
                    "single_service"
                },
                RuntimeKind::Go,
                Some(PackageManager::Go),
                port,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if let Some(root) = find_dotnet_root(project_root) {
        let package_root = project_root.join(&root);
        let port = dotnet_default_port(&package_root);
        let mut files = dotnet_file_facts(project_root, &root);
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Dotnet,
            package_manager: Some(PackageManager::Dotnet),
            has_lockfile: false,
            framework: dotnet_framework(&package_root),
            runtime_version: dotnet_runtime_version(&package_root),
            runtime_version_source: None,
            build_command: Some("dotnet publish -c Release -o /app/publish".to_string()),
            start_command: Some("dotnet \"$(find /app -maxdepth 1 -name '*.dll' ! -name '*.Views.dll' | sort | head -n 1)\"".to_string()),
            output_directory: frontend_support.output_directory.clone(),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.clone()),
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                if has_frontend {
                    "multi_application"
                } else {
                    "single_service"
                },
                RuntimeKind::Dotnet,
                Some(PackageManager::Dotnet),
                port,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if let Some(root) = find_stack_root(
        project_root,
        &["composer.json", "artisan", "public/index.php"],
    ) {
        let package_root = project_root.join(&root);
        let port = 8000;
        let mut files = stack_file_facts(
            project_root,
            &root,
            &[
                "composer.json",
                "composer.lock",
                "artisan",
                "public/index.php",
            ],
            "php_runtime_file",
        );
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Php,
            package_manager: Some(PackageManager::Composer),
            has_lockfile: package_root.join("composer.lock").exists(),
            framework: php_framework(&package_root),
            runtime_version: None,
            runtime_version_source: None,
            build_command: None,
            start_command: Some(php_start_command(&package_root, port)),
            output_directory: frontend_support.output_directory.clone(),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.clone()),
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                if has_frontend {
                    "multi_application"
                } else {
                    "single_service"
                },
                RuntimeKind::Php,
                Some(PackageManager::Composer),
                port,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if let Some(root) = find_stack_root(
        project_root,
        &["Gemfile", "config.ru", "config/application.rb"],
    ) {
        let package_root = project_root.join(&root);
        let port = 3000;
        let mut files = stack_file_facts(
            project_root,
            &root,
            &[
                "Gemfile",
                "Gemfile.lock",
                "config.ru",
                "config/application.rb",
            ],
            "ruby_runtime_file",
        );
        files.extend(frontend_support.files.clone());
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Ruby,
            package_manager: Some(PackageManager::Bundler),
            has_lockfile: package_root.join("Gemfile.lock").exists(),
            framework: ruby_framework(&package_root),
            runtime_version: ruby_runtime_version(&package_root),
            runtime_version_source: None,
            build_command: None,
            start_command: Some(ruby_start_command(&package_root, port)),
            output_directory: frontend_support.output_directory.clone(),
            port,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.clone()),
            workspace_package_json_paths: frontend_support.package_refs.clone(),
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                if has_frontend {
                    "multi_application"
                } else {
                    "single_service"
                },
                RuntimeKind::Ruby,
                Some(PackageManager::Bundler),
                port,
                files,
                frontend_support.package_refs,
            )?,
        });
    }

    if let Some(root) = find_static_root(project_root) {
        return Ok(DeploymentCodeProbe {
            kind: RuntimeKind::Static,
            package_manager: None,
            has_lockfile: false,
            framework: Some("static".to_string()),
            runtime_version: None,
            runtime_version_source: None,
            build_command: None,
            start_command: None,
            output_directory: Some(root.clone()),
            port: 80,
            healthcheck_path: Some("/".to_string()),
            working_directory: (root != ".").then_some(root.clone()),
            workspace_package_json_paths: vec![],
            services: services.clone(),
            env_defaults,
            spring_ddl_auto_validate,
            flyway_detected,
            evidence: evidence_value(
                project_root,
                "static_site",
                RuntimeKind::Static,
                None,
                80,
                stack_file_facts(
                    project_root,
                    &root,
                    &["index.html", "404.html"],
                    "static_runtime_file",
                ),
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
            services: services.clone(),
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

#[derive(Debug, Clone)]
struct FrontendProbeEvidence {
    package_refs: Vec<String>,
    files: Vec<Value>,
    has_lockfile: bool,
    output_directory: Option<String>,
}

fn detected_frontend_root(project_root: &Path) -> Option<String> {
    if root_frontend_detected(project_root) {
        Some(".".to_string())
    } else {
        find_frontend_root(project_root)
    }
}

fn frontend_probe_evidence(project_root: &Path, root: Option<&str>) -> FrontendProbeEvidence {
    let Some(root) = root else {
        return FrontendProbeEvidence {
            package_refs: vec![],
            files: vec![],
            has_lockfile: false,
            output_directory: None,
        };
    };
    let package_refs = vec![join_frontend_root(root, "package.json")];
    let frontend_path = root_path(project_root, root);
    let files = [
        ("package.json", "frontend_package_file"),
        ("package-lock.json", "frontend_lockfile"),
        ("pnpm-lock.yaml", "frontend_lockfile"),
        ("yarn.lock", "frontend_lockfile"),
        ("bun.lockb", "frontend_lockfile"),
        ("vite.config.ts", "frontend_config"),
        ("vite.config.js", "frontend_config"),
        ("next.config.js", "frontend_config"),
        ("next.config.mjs", "frontend_config"),
        ("angular.json", "frontend_config"),
        ("src/main.tsx", "frontend_entry"),
        ("src/main.ts", "frontend_entry"),
        ("src/App.tsx", "frontend_entry"),
        ("src/App.vue", "frontend_entry"),
    ]
    .into_iter()
    .map(|(relative, kind)| file_fact(project_root, &join_frontend_root(root, relative), kind))
    .collect::<Vec<_>>();
    FrontendProbeEvidence {
        package_refs,
        files,
        has_lockfile: has_node_lockfile(&frontend_path),
        output_directory: Some(join_frontend_root(root, "dist")),
    }
}

fn root_path(project_root: &Path, root: &str) -> std::path::PathBuf {
    if root == "." {
        project_root.to_path_buf()
    } else {
        project_root.join(root)
    }
}

fn join_frontend_root(root: &str, relative: &str) -> String {
    if root == "." || root.is_empty() {
        relative.to_string()
    } else {
        format!("{}/{}", root.trim_matches('/'), relative)
    }
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

fn find_stack_root(project_root: &Path, markers: &[&str]) -> Option<String> {
    candidate_roots(project_root).into_iter().find(|root| {
        markers
            .iter()
            .any(|marker| project_root.join(root).join(marker).exists())
    })
}

fn candidate_roots(project_root: &Path) -> Vec<String> {
    let mut roots = vec![
        ".".to_string(),
        "service".to_string(),
        "backend".to_string(),
        "api".to_string(),
        "server".to_string(),
        "app".to_string(),
    ];
    for parent in ["apps", "services", "packages"] {
        let dir = project_root.join(parent);
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                roots.push(format!("{parent}/{name}"));
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn find_dotnet_root(project_root: &Path) -> Option<String> {
    candidate_roots(project_root).into_iter().find(|root| {
        let path = project_root.join(root);
        path.join("global.json").exists()
            || fs::read_dir(&path)
                .map(|entries| {
                    entries.flatten().any(|entry| {
                        matches!(
                            entry
                                .path()
                                .extension()
                                .and_then(|extension| extension.to_str()),
                            Some("csproj" | "sln")
                        )
                    })
                })
                .unwrap_or(false)
    })
}

fn find_static_root(project_root: &Path) -> Option<String> {
    for root in ["dist", "build", "public", "out", "_site", "."] {
        if project_root.join(root).join("index.html").exists() {
            return Some(root.to_string());
        }
    }
    None
}

fn root_frontend_detected(project_root: &Path) -> bool {
    if !project_root.join("package.json").exists() {
        return false;
    }
    if [
        "vite.config.js",
        "vite.config.ts",
        "next.config.js",
        "next.config.mjs",
        "index.html",
    ]
    .iter()
    .any(|relative| project_root.join(relative).exists())
    {
        return true;
    }
    let Ok(package_json) = fs::read_to_string(project_root.join("package.json")) else {
        return false;
    };
    let lower = package_json.to_ascii_lowercase();
    [
        "\"vite\"",
        "\"next\"",
        "\"react\"",
        "\"vue\"",
        "\"svelte\"",
        "\"@angular/",
        "vite build",
        "next build",
        "react-scripts build",
        "ng build",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
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

fn java_framework(project_root: &Path, root: &str) -> &'static str {
    let base = if root == "." {
        project_root.to_path_buf()
    } else {
        project_root.join(root)
    };
    for relative in [
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "src/main/resources/application.yml",
        "src/main/resources/application.yaml",
        "src/main/resources/application.properties",
    ] {
        let Ok(text) = fs::read_to_string(base.join(relative)) else {
            continue;
        };
        let lower = text.to_ascii_lowercase();
        if lower.contains("spring-boot") || lower.contains("org.springframework") {
            return "spring-boot";
        }
    }
    "java"
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

fn python_package_manager(root: &Path) -> Option<PackageManager> {
    if root.join("uv.lock").exists()
        || file_contains(root.join("pyproject.toml").as_path(), "[tool.uv]")
    {
        Some(PackageManager::Uv)
    } else if root.join("poetry.lock").exists()
        || file_contains(root.join("pyproject.toml").as_path(), "[tool.poetry]")
    {
        Some(PackageManager::Poetry)
    } else {
        Some(PackageManager::Pip)
    }
}

fn has_python_lockfile(root: &Path) -> bool {
    ["uv.lock", "poetry.lock", "Pipfile.lock"]
        .iter()
        .any(|name| root.join(name).exists())
}

fn python_framework(root: &Path) -> Option<String> {
    let signals = read_stack_signals(
        root,
        &[
            "requirements.txt",
            "pyproject.toml",
            "Pipfile",
            "manage.py",
            "main.py",
            "app.py",
            "server.py",
        ],
    );
    if root.join("manage.py").exists() || signals.contains("django") {
        Some("django".to_string())
    } else if signals.contains("fastapi") || signals.contains("uvicorn") {
        Some("fastapi".to_string())
    } else if signals.contains("flask") {
        Some("flask".to_string())
    } else if signals.contains("streamlit") {
        Some("streamlit".to_string())
    } else {
        Some("python".to_string())
    }
}

fn python_default_port(framework: Option<&str>) -> u16 {
    if framework == Some("streamlit") {
        8501
    } else {
        8000
    }
}

fn python_start_command(root: &Path, port: u16) -> String {
    if root.join("manage.py").exists() {
        return format!("python manage.py runserver 0.0.0.0:${{PORT:-{port}}}");
    }
    let framework = python_framework(root).unwrap_or_else(|| "python".to_string());
    if framework == "fastapi" {
        let module = if root.join("main.py").exists() {
            "main"
        } else {
            "app"
        };
        return format!("python -m uvicorn {module}:app --host 0.0.0.0 --port ${{PORT:-{port}}}");
    }
    if framework == "flask" {
        let app = if root.join("app.py").exists() {
            "app.py"
        } else {
            "main.py"
        };
        return format!("python -m flask --app {app} run --host=0.0.0.0 --port=${{PORT:-{port}}}");
    }
    if framework == "streamlit" {
        let entry = if root.join("app.py").exists() {
            "app.py"
        } else {
            "main.py"
        };
        return format!(
            "streamlit run {entry} --server.address 0.0.0.0 --server.port ${{PORT:-{port}}}"
        );
    }
    for entry in ["server.py", "main.py", "app.py"] {
        if root.join(entry).exists() {
            return format!("python {entry} --host 0.0.0.0 --port ${{PORT:-{port}}}");
        }
    }
    "python -m http.server ${PORT:-8000} --bind 0.0.0.0".to_string()
}

fn go_default_port(root: &Path) -> u16 {
    let signals = read_stack_signals(root, &["main.go", "go.mod"]);
    extract_port_from_text(&signals).unwrap_or(8080)
}

fn dotnet_framework(root: &Path) -> Option<String> {
    let signals = read_dotnet_project_signals(root);
    if signals.contains("microsoft.net.sdk.web") || signals.contains("aspnetcore") {
        Some("aspnet".to_string())
    } else {
        Some("dotnet".to_string())
    }
}

fn dotnet_default_port(root: &Path) -> u16 {
    extract_port_from_text(&read_stack_signals(
        root,
        &["appsettings.json", "Properties/launchSettings.json"],
    ))
    .unwrap_or(8080)
}

fn dotnet_runtime_version(root: &Path) -> Option<String> {
    let signals = read_dotnet_project_signals(root);
    for major in ["10", "9", "8", "7", "6"] {
        if signals.contains(&format!("net{major}.0")) {
            return Some(major.to_string());
        }
    }
    None
}

fn php_framework(root: &Path) -> Option<String> {
    let signals = read_stack_signals(root, &["composer.json", "artisan", "public/index.php"]);
    if root.join("artisan").exists() || signals.contains("laravel") {
        Some("laravel".to_string())
    } else if signals.contains("symfony") {
        Some("symfony".to_string())
    } else {
        Some("php".to_string())
    }
}

fn php_start_command(root: &Path, port: u16) -> String {
    if root.join("artisan").exists() {
        return format!("php artisan serve --host=0.0.0.0 --port=${{PORT:-{port}}}");
    }
    let docroot = if root.join("public/index.php").exists() {
        "public"
    } else {
        "."
    };
    format!("php -S 0.0.0.0:${{PORT:-{port}}} -t {docroot}")
}

fn ruby_framework(root: &Path) -> Option<String> {
    let signals = read_stack_signals(root, &["Gemfile", "config.ru", "config/application.rb"]);
    if root.join("config/application.rb").exists() || signals.contains("rails") {
        Some("rails".to_string())
    } else if signals.contains("sinatra") {
        Some("sinatra".to_string())
    } else {
        Some("ruby".to_string())
    }
}

fn ruby_start_command(root: &Path, port: u16) -> String {
    if ruby_framework(root).as_deref() == Some("rails") {
        format!("bundle exec rails server -b 0.0.0.0 -p ${{PORT:-{port}}}")
    } else {
        format!("bundle exec rackup -o 0.0.0.0 -p ${{PORT:-{port}}}")
    }
}

fn ruby_runtime_version(root: &Path) -> Option<String> {
    fs::read_to_string(root.join(".ruby-version"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn collect_dependency_services(project_root: &Path) -> Vec<DependencyService> {
    let signals = dependency_signal_text(project_root);
    let mut services = Vec::new();
    for (kind, needles) in [
        (
            DependencyServiceKind::Postgres,
            &[
                "postgres",
                "postgresql",
                "jdbc:postgresql",
                "npgsql",
                "psycopg",
            ] as &[&str],
        ),
        (
            DependencyServiceKind::Mysql,
            &["mysql", "mariadb", "jdbc:mysql", "mysql2"] as &[&str],
        ),
        (
            DependencyServiceKind::Redis,
            &["redis", "ioredis", "lettuce", "jedis"] as &[&str],
        ),
    ] {
        if needles.iter().any(|needle| signals.contains(needle)) {
            services.push(crate::runtime_contract::service_definition(kind));
        }
    }
    services
}

fn dependency_signal_text(project_root: &Path) -> String {
    let mut text = String::new();
    for root in candidate_roots(project_root) {
        let base = project_root.join(root);
        for name in [
            "package.json",
            "pom.xml",
            "build.gradle",
            "build.gradle.kts",
            "requirements.txt",
            "pyproject.toml",
            "go.mod",
            "composer.json",
            "Gemfile",
            "appsettings.json",
            "application.properties",
            "src/main/resources/application.properties",
            "src/main/resources/application.yml",
            "src/main/resources/application.yaml",
        ] {
            if let Ok(value) = fs::read_to_string(base.join(name)) {
                text.push_str(&value);
                text.push('\n');
            }
        }
    }
    text.to_ascii_lowercase()
}

fn stack_file_facts(project_root: &Path, root: &str, files: &[&str], kind: &str) -> Vec<Value> {
    files
        .iter()
        .map(|file| file_fact(project_root, &join_root(root, file), kind))
        .collect()
}

fn dotnet_file_facts(project_root: &Path, root: &str) -> Vec<Value> {
    let mut facts = stack_file_facts(
        project_root,
        root,
        &[
            "global.json",
            "appsettings.json",
            "Properties/launchSettings.json",
        ],
        "dotnet_runtime_file",
    );
    let base = project_root.join(root);
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("csproj" | "sln")
            ) {
                if let Ok(relative) = to_project_relative(project_root, &path) {
                    facts.push(json!({ "path": relative, "kind": "dotnet_project_file" }));
                }
            }
        }
    }
    facts
}

fn read_dotnet_project_signals(root: &Path) -> String {
    let mut text = read_stack_signals(
        root,
        &[
            "global.json",
            "appsettings.json",
            "Properties/launchSettings.json",
        ],
    );
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("csproj" | "sln")
            ) {
                if let Ok(value) = fs::read_to_string(path) {
                    text.push_str(&value.to_ascii_lowercase());
                    text.push('\n');
                }
            }
        }
    }
    text
}

fn read_stack_signals(root: &Path, files: &[&str]) -> String {
    files
        .iter()
        .filter_map(|file| fs::read_to_string(root.join(file)).ok())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase()
}

fn file_contains(path: &Path, needle: &str) -> bool {
    fs::read_to_string(path)
        .map(|text| text.contains(needle))
        .unwrap_or(false)
}

fn extract_port_from_text(text: &str) -> Option<u16> {
    for marker in ["port", "PORT", "listen"] {
        let Some(index) = text.find(marker) else {
            continue;
        };
        let after = &text[index + marker.len()..];
        let digits = after
            .chars()
            .skip_while(|ch| !ch.is_ascii_digit())
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if let Ok(port) = digits.parse::<u16>() {
            if port > 0 {
                return Some(port);
            }
        }
    }
    None
}

fn join_root(root: &str, path: &str) -> String {
    if root == "." || root.is_empty() {
        path.to_string()
    } else {
        format!("{}/{}", root.trim_matches('/'), path)
    }
}
