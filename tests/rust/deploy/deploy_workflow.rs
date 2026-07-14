use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use contracts::{
    DeployProvider, DeploymentContractAuthority, DeploymentErrorWindow, DeploymentFailedContract,
    DeploymentFailureDiagnostic, DeploymentFailureKind, DeploymentFailureOwner,
    DeploymentFailureReport, DeploymentProviderPolicy, DeploymentRepairAction,
    DeploymentRepairRoute, DeploymentRuntimePort, DeploymentShape, DeploymentSpec, PackageManager,
    RuntimeKind, SourceModelSource, SourceServiceRole,
};
use delivery_core::{
    DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState, DeliveryStatusEntry,
    FileSubmitInput, InspectRequestInput, ProjectStatus, ReadFieldGroupInput,
    ReadRequestFieldsInput,
};
use deploy::{
    accept_deploy_execution_repair_file, deploy_bootstrap, deploy_inspect, deploy_prepare,
    deploy_repair, deploy_run, deploy_status, deploy_up, deploy_validate, DeployBootstrapInput,
    DeployToolInput,
};
use serde_json::{json, Value};
use state::store::{ensure_dir, now_millis, now_string, read_json, read_text, write_json_atomic};

#[test]
fn prepare_uses_runtime_delivery_source_model_topology_without_single_node_collapse() {
    let fixture = Fixture::new("deploy-composite");
    fixture.write_runtime_delivery(runtime_delivery());

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("result json");
    assert_eq!(value["state"], "done", "{value:#}");
    let reference_profile = &value["details"]["deployReferenceProfile"];
    for expected in [
        "deploy.providers",
        "deploy.matrix",
        "deploy.source-model",
        "deploy.topology",
        "deploy.compose",
        "deploy.dockerfile",
        "deploy.environment",
        "deploy.workspaces",
        "deploy.stacks.node",
        "deploy.stacks.java",
    ] {
        assert_reference_load_plan_contains(reference_profile, expected);
    }
    assert_deploy_reference_profile_shape(reference_profile);

    let spec: DeploymentSpec = read_json(&fixture.root.join(".loom/deployment/specs/local.json"))
        .expect("deployment spec");
    assert_eq!(
        spec.runtime_contract.deployment_shape,
        Some(DeploymentShape::FrontendAndBackend)
    );
    assert_eq!(spec.source_model.shape, DeploymentShape::FrontendAndBackend);
    assert_eq!(spec.source_model.primary_service_id, "backend");
    assert_eq!(spec.source_model.preview_service_id, "frontend");
    assert!(spec
        .source_model
        .services
        .iter()
        .any(|service| service.service_id == "frontend"));
    assert!(spec
        .source_model
        .services
        .iter()
        .any(|service| service.service_id == "backend"));
    assert!(spec
        .source_model
        .dependencies
        .iter()
        .any(|dependency| dependency.service_name == "postgres"));
    assert_eq!(spec.topology.public_entry_service_id, "frontend");
    assert_eq!(
        serde_json::to_value(spec.facts.topology_class).unwrap(),
        json!("frontend_gateway_backend_api")
    );
    assert_eq!(
        serde_json::to_value(spec.facts.layout_kind).unwrap(),
        json!("workspace_app")
    );
    assert_eq!(spec.facts.public_entry_service_id, "frontend");
    assert_eq!(spec.facts.primary_service_id, "backend");
    assert!(serde_json::to_string(&spec.topology)
        .unwrap()
        .contains("\"kind\":\"http-proxy\""));
    assert!(serde_json::to_string(&spec.topology)
        .unwrap()
        .contains("\"publicPath\":\"/api\""));
    let frontend_port = runtime_port(&spec, "frontend").expect("frontend runtime port");
    assert_eq!(frontend_port.purpose, "preview");
    assert_eq!(frontend_port.container_port, 80);
    assert_eq!(frontend_port.preferred_host_port, Some(4173));
    assert!(frontend_port.host_port.is_some(), "{frontend_port:?}");
    assert!(!frontend_port.internal_only);
    let backend_port = runtime_port(&spec, "backend").expect("backend runtime port");
    assert_eq!(backend_port.purpose, "api");
    assert_eq!(backend_port.container_port, 8080);
    assert_eq!(backend_port.host_port, None);
    assert!(backend_port.internal_only);
    let postgres_port = runtime_port(&spec, "postgres").expect("postgres runtime port");
    assert_eq!(postgres_port.purpose, "dependency");
    assert_eq!(postgres_port.host_port, None);
    assert!(postgres_port.internal_only);

    let nginx = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/nginx.frontend.conf"),
    )
    .expect("nginx config");
    let proxy = nginx
        .find("proxy_pass http://backend:8080")
        .expect("proxy route");
    let spa = nginx
        .find("try_files $uri $uri/ /index.html")
        .expect("spa fallback");
    assert!(proxy < spa, "{nginx}");

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(
        compose.starts_with(&format!("name: {}\nservices:\n", spec.service_name)),
        "{compose}"
    );
    assert!(compose.contains("  frontend:"));
    assert!(
        compose.contains("      dockerfile: .loom/deployment/specs/generated/Dockerfile.frontend")
    );
    assert!(
        compose.contains("      dockerfile: .loom/deployment/specs/generated/Dockerfile.backend")
    );
    assert!(compose.contains("  backend:"));
    assert!(compose.contains("  postgres:"));
    assert!(compose.contains("      backend:\n        condition: service_healthy"));
    assert!(compose.contains("      - postgres"));
    assert!(compose.contains("  frontend:\n    build:"), "{compose}");
    assert!(compose.contains("    ports:\n"), "{compose}");
    let backend_block = compose_service_block(&compose, "backend").expect("backend compose block");
    assert!(!backend_block.contains("ports:"), "{backend_block}");

    let frontend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("frontend dockerfile");
    assert!(
        !frontend_dockerfile.contains("API_BASE_URL"),
        "{frontend_dockerfile}"
    );
    assert_eq!(spec.frontend_api_binding.status, "not_applicable");
}

#[test]
fn prepare_uses_root_frontend_and_python_backend_source_model() {
    let fixture = Fixture::new("deploy-root-vite-fastapi");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "web_frontend_plus_api_service",
        "deploymentShape": "frontend-and-backend",
        "build": {
            "command": "frontend: npm run build; backend: uvicorn app.main:app --host 0.0.0.0 --port 8000"
        },
        "start": {
            "command": "frontend: npm run dev; backend: uvicorn app.main:app --reload --port 8000",
            "port": 8000
        },
        "httpProbes": {
            "previewPath": "/",
            "apiPaths": ["/api/tickets", "/api/tickets/{ticketId}"]
        }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","dev":"vite --host 0.0.0.0"},"dependencies":{"@vitejs/plugin-vue":"latest","vite":"latest","vue":"latest"}}"#,
    );
    fixture.write_text("vite.config.js", "export default {}\n");
    fixture.write_text("src/main.ts", "import './style.css'\n");
    fixture.write_text(
        "pyproject.toml",
        r#"[project]
name = "loom-fastapi-fixture"
version = "0.1.0"
dependencies = ["fastapi", "uvicorn"]
"#,
    );
    fixture.write_text(
        "app/main.py",
        "from fastapi import FastAPI\napp = FastAPI()\n@app.get('/api/tickets')\ndef tickets():\n    return []\n",
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    let frontend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "frontend")
        .expect("frontend service");
    assert_eq!(frontend.root, ".");
    assert_eq!(frontend.manifest_refs, vec!["package.json"]);
    assert_eq!(frontend.output_directory.as_deref(), Some("dist"));
    assert_eq!(frontend.artifact_refs, vec!["dist"]);
    assert_eq!(frontend.runtime_kind, RuntimeKind::Node);

    let backend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "backend")
        .expect("backend service");
    assert_eq!(backend.root, ".");
    assert_eq!(backend.runtime_kind, RuntimeKind::Python);
    assert!(backend
        .manifest_refs
        .iter()
        .any(|item| item == "pyproject.toml"));
    assert!(backend
        .start_command
        .as_deref()
        .is_some_and(|command| command.contains("uvicorn app.main:app")));

    let frontend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("frontend dockerfile");
    assert!(
        frontend_dockerfile.contains("COPY package.json ./"),
        "{frontend_dockerfile}"
    );
    assert!(
        frontend_dockerfile.contains("COPY . ."),
        "{frontend_dockerfile}"
    );
    assert!(
        frontend_dockerfile.contains("COPY --from=builder /workspace/dist /usr/share/nginx/html"),
        "{frontend_dockerfile}"
    );
    assert!(
        !frontend_dockerfile.contains("frontend/package.json"),
        "{frontend_dockerfile}"
    );

    let backend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.backend"),
    )
    .expect("backend dockerfile");
    assert!(
        backend_dockerfile.contains("pip install --no-cache-dir ."),
        "{backend_dockerfile}"
    );
    assert!(
        !backend_dockerfile.contains("FROM node:"),
        "{backend_dockerfile}"
    );

    let validate = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let validate_value = serde_json::to_value(validate).expect("validate json");
    assert_eq!(
        validate_value["details"]["assetIssues"],
        json!([]),
        "{validate_value:#}"
    );
}

#[test]
fn deploy_binds_frontend_requests_to_accepted_api_interface_paths() {
    let fixture = Fixture::new("deploy-api-contract-binding");
    let mut runtime = runtime_delivery();
    runtime["api"]["preservePath"] = json!(false);
    fixture.write_runtime_delivery_with_interfaces(
        runtime,
        json!([{
            "interfaceId": "api.accounts.list",
            "name": "List accounts",
            "type": "http_api",
            "method": "GET",
            "path": "/api/accounts"
        }]),
    );
    fixture.write_text(
        "apps/frontend/package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text(
        "apps/frontend/src/lib/api.js",
        r#"const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
export function loadAccounts() {
  return fetch(`${API_BASE_URL}/api/accounts`);
}
"#,
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert!(spec
        .runtime_contract
        .safe_http_probes
        .iter()
        .any(|probe| probe.path == "/api/accounts"));
    assert_eq!(
        spec.runtime_contract
            .api_contract
            .as_ref()
            .expect("api contract")
            .interfaces[0]
            .path,
        "/api/accounts"
    );
    assert_eq!(spec.frontend_api_binding.status, "resolved");
    assert_eq!(
        spec.frontend_api_binding.environment_key.as_deref(),
        Some("VITE_API_BASE_URL")
    );
    assert_eq!(
        spec.frontend_api_binding.injected_value.as_deref(),
        Some("")
    );
    assert!(serde_json::to_string(&spec.topology)
        .expect("topology json")
        .contains("\"preservePath\":false"));
    let nginx = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/nginx.frontend.conf"),
    )
    .expect("frontend nginx config");
    assert!(
        nginx.contains("proxy_pass http://backend:8080/;"),
        "{nginx}"
    );

    let dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("frontend dockerfile");
    assert!(
        dockerfile.contains("ENV VITE_API_BASE_URL=\n"),
        "{dockerfile}"
    );
    assert!(
        !dockerfile.contains("ENV FRONTEND_API_BASE_URL"),
        "{dockerfile}"
    );
    assert!(
        !dockerfile.contains("ENV REACT_APP_API_BASE_URL"),
        "{dockerfile}"
    );
    assert!(
        !dockerfile.contains("ENV NEXT_PUBLIC_API_BASE_URL"),
        "{dockerfile}"
    );

    fixture.write_text(
        "apps/frontend/src/lib/api.js",
        r#"const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
export function loadAccounts() {
  return fetch(`${API_BASE_URL}/accounts`);
}
"#,
    );
    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("relative prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    let spec = fixture.read_spec();
    assert_eq!(spec.frontend_api_binding.mode, "relative_to_public_base");
    assert_eq!(
        spec.frontend_api_binding.injected_value.as_deref(),
        Some("/api")
    );
    let dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("relative frontend dockerfile");
    assert!(
        dockerfile.contains("ENV VITE_API_BASE_URL=/api\n"),
        "{dockerfile}"
    );
    assert!(
        !dockerfile.contains("ENV FRONTEND_API_BASE_URL"),
        "{dockerfile}"
    );
}

#[test]
fn deploy_binds_integrated_frontend_source_without_assuming_a_framework_env_name() {
    let fixture = Fixture::new("deploy-integrated-api-contract-binding");
    fixture.write_runtime_delivery_with_interfaces(
        json!({
            "status": "modified",
            "runtimeKind": "spring-boot-served-static-frontend",
            "deploymentShape": "single-service",
            "build": { "command": "./mvnw -DskipTests package" },
            "start": { "command": "java -jar target/app.jar", "port": 8080 },
            "httpProbes": { "previewPath": "/", "expectedStatus": "2xx_or_3xx" },
            "frontend": {
                "required": true,
                "kind": "vite-react",
                "sourceRoot": ".",
                "outputDir": "dist",
                "buildCommand": "npm run build",
                "servedBy": "springboot-static"
            },
        "api": {
            "required": true,
            "kind": "spring-boot",
            "entry": "pom.xml",
            "basePath": "/api"
            }
        }),
        json!([{
            "interfaceId": "api.tickets.list",
            "type": "http_api",
            "method": "GET",
            "path": "/api/tickets"
        }]),
    );
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("package-lock.json", "{}\n");
    fixture.write_text("vite.config.js", "export default {}\n");
    fixture.write_text(
        "src/lib/api.js",
        r#"const base = import.meta.env.CUSTOM_API_URL || "";
export function loadTickets() { return fetch(`${base}/tickets`); }
"#,
    );
    fixture.write_text(
        "pom.xml",
        "<project><artifactId>tickets</artifactId></project>\n",
    );
    fixture.write_text("mvnw", "#!/bin/sh\n");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert_eq!(spec.source_model.services.len(), 1);
    assert_eq!(spec.source_model.services[0].role, SourceServiceRole::App);
    assert_eq!(spec.frontend_api_binding.status, "resolved");
    assert_eq!(
        spec.frontend_api_binding.environment_key.as_deref(),
        Some("CUSTOM_API_URL")
    );
    assert_eq!(
        spec.frontend_api_binding.injected_value.as_deref(),
        Some("/api")
    );
    let dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.app"),
    )
    .expect("integrated app dockerfile");
    assert!(
        dockerfile.contains("ENV CUSTOM_API_URL=/api\n"),
        "{dockerfile}"
    );
}

#[test]
fn deploy_validate_applies_controlled_model_repair_over_local_snapshot() {
    let fixture = Fixture::new("deploy-source-model-sidecar-overlay");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "web_frontend_plus_api_service",
        "deploymentShape": "frontend-and-backend",
        "build": {
            "command": "frontend: npm run build; backend: uvicorn app.main:app --host 0.0.0.0 --port 8000"
        },
        "start": {
            "command": "frontend: npm run dev; backend: uvicorn app.main:app --reload --port 8000",
            "port": 8000
        },
        "httpProbes": {
            "previewPath": "/",
            "apiPaths": ["/api/tickets"]
        }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","dev":"vite --host 0.0.0.0"},"dependencies":{"vite":"latest","vue":"latest"}}"#,
    );
    fixture.write_text("vite.config.js", "export default {}\n");
    fixture.write_text("src/main.ts", "console.log('fixture')\n");
    fixture.write_text(
        "pyproject.toml",
        r#"[project]
name = "loom-fastapi-fixture"
version = "0.1.0"
dependencies = ["fastapi", "uvicorn"]
"#,
    );
    fixture.write_text(
        "app/main.py",
        "from fastapi import FastAPI\napp = FastAPI()\n",
    );

    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");
    let mut source_model = fixture.read_spec().source_model;
    let correct_source_model = source_model.clone();
    let frontend = source_model
        .services
        .iter_mut()
        .find(|service| service.service_id == "frontend")
        .expect("frontend service");
    frontend.root = "frontend".to_string();
    frontend.manifest_refs = vec!["frontend/package.json".to_string()];
    frontend.output_directory = Some("frontend/dist".to_string());
    frontend.artifact_refs = vec!["frontend/dist".to_string()];
    write_json_atomic(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/model-repair.json"),
        &json!({
            "schemaVersion": "1.0",
            "status": "accepted",
            "baseFingerprint": fixture.read_spec().input_fingerprint,
            "reason": "test controlled source model correction",
            "sourceModel": source_model,
            "topology": null
        }),
    )
    .expect("write bad model repair");

    let bad_validate = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let bad_value = serde_json::to_value(bad_validate).expect("bad validate json");
    let bad_issues = bad_value["details"]["assetIssues"]
        .as_array()
        .expect("bad asset issues")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        bad_issues.iter().any(
            |issue| issue.contains("sourceModel service frontend root frontend does not exist")
        ),
        "{bad_value:#}"
    );
    let applied_repair: Value = read_json(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/model-repair.json"),
    )
    .expect("applied model repair");
    assert_eq!(applied_repair["status"], "applied");

    write_json_atomic(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/model-repair.json"),
        &json!({
            "schemaVersion": "1.0",
            "status": "accepted",
            "baseFingerprint": fixture.read_spec().input_fingerprint,
            "reason": "test controlled source model correction",
            "sourceModel": correct_source_model,
            "topology": null
        }),
    )
    .expect("write corrected model repair");
    let good_validate = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let good_value = serde_json::to_value(good_validate).expect("good validate json");
    assert_eq!(
        good_value["details"]["assetIssues"],
        json!([]),
        "{good_value:#}"
    );
}

#[cfg(unix)]
#[test]
fn deploy_up_asset_repair_exposes_generated_sidecars_as_editable() {
    let fixture = Fixture::new("deploy-sidecar-editable-repair");
    write_root_vite_fastapi_delivery(&fixture);
    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");
    let mut source_model = fixture.read_spec().source_model;
    let frontend = source_model
        .services
        .iter_mut()
        .find(|service| service.service_id == "frontend")
        .expect("frontend service");
    frontend.root = "frontend".to_string();
    frontend.manifest_refs = vec!["frontend/package.json".to_string()];
    write_json_atomic(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/model-repair.json"),
        &json!({
            "schemaVersion": "1.0",
            "status": "accepted",
            "baseFingerprint": fixture.read_spec().input_fingerprint,
            "reason": "test controlled source model correction",
            "sourceModel": source_model,
            "topology": null
        }),
    )
    .expect("write bad model repair");
    fixture.write_mock_docker(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
exit 0
"#,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
    assert_eq!(value["next"]["kind"], "deploy_repair_assets", "{value:#}");
    let editable = value["next"]["editableFiles"]
        .as_array()
        .expect("editable files")
        .iter()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for expected in ["model-repair.json"] {
        assert!(
            editable.iter().any(|item| item.ends_with(expected)),
            "deploy asset repair must allow generated sidecar {expected}: {value:#}"
        );
    }
}

#[test]
fn source_model_matrix_handles_root_frontend_with_backend_stack_roots() {
    for case in backend_source_model_matrix_cases() {
        let fixture = Fixture::new(&format!("deploy-source-model-matrix-{}", case.name));
        fixture.write_runtime_delivery(json!({
            "status": "modified",
            "runtimeKind": "web_frontend_plus_api_service",
            "deploymentShape": "frontend-and-backend",
            "build": { "command": format!("frontend: npm run build; backend: {}", case.build_command) },
            "start": { "command": format!("frontend: npm run dev; backend: {}", case.start_command), "port": 8000 },
            "httpProbes": {
                "previewPath": "/",
                "apiPaths": ["/api/health"]
            },
            "api": {
                "required": true,
                "kind": case.api_kind,
                "entry": case.api_entry,
                "buildCommand": case.build_command,
                "basePath": "/api",
                "probePaths": ["/api/health"]
            }
        }));
        write_root_frontend_fixture(&fixture);
        for (relative, text) in &case.files {
            fixture.write_text(relative, text);
        }

        let prepare = deploy_prepare(DeployToolInput {
            project_root: fixture.root_str(),
            app_path: None,
            healthcheck: None,
            provider_policy: None,
        });
        let prepare_value = serde_json::to_value(prepare).expect("prepare json");
        assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");

        let spec = fixture.read_spec();
        let frontend = spec
            .source_model
            .services
            .iter()
            .find(|service| service.service_id == "frontend")
            .expect("frontend service");
        assert_eq!(frontend.root, ".", "{case:?}");
        assert_eq!(frontend.manifest_refs, vec!["package.json"], "{case:?}");
        assert_eq!(
            frontend.output_directory.as_deref(),
            Some("dist"),
            "{case:?}"
        );

        let backend = spec
            .source_model
            .services
            .iter()
            .find(|service| service.service_id == "backend")
            .expect("backend service");
        assert_eq!(backend.root, case.expected_root, "{case:?}");
        assert_eq!(backend.runtime_kind, case.expected_kind, "{case:?}");
        assert!(
            backend
                .manifest_refs
                .iter()
                .any(|item| item == case.expected_manifest
                    || item == &format!("{}/{}", case.expected_root, case.expected_manifest)),
            "backend manifest refs should include {} for {case:?}: {:?}",
            case.expected_manifest,
            backend.manifest_refs
        );
        assert_eq!(
            backend.build_command.as_deref(),
            Some(case.expected_build_command),
            "{case:?}"
        );
        assert_eq!(
            backend.start_command.as_deref(),
            Some(case.expected_start_command),
            "{case:?}"
        );

        let validate = deploy_validate(DeployToolInput {
            project_root: fixture.root_str(),
            app_path: None,
            healthcheck: None,
            provider_policy: None,
        });
        let validate_value = serde_json::to_value(validate).expect("validate json");
        assert_eq!(
            validate_value["details"]["assetIssues"],
            json!([]),
            "asset validation failed for {case:?}: {validate_value:#}"
        );
    }
}

#[test]
fn deploy_prepare_applies_healthcheck_input_to_single_service_spec_and_compose() {
    let fixture = Fixture::new("deploy-healthcheck-input-single");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","start":"node server.js"},"dependencies":{"express":"latest"}}"#,
    );
    fixture.write_text("package-lock.json", "{}\n");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: Some("http://localhost:8080/ready?deep=true".to_string()),
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert_eq!(spec.runtime_contract.health_path.as_deref(), Some("/ready"));
    let app = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "app")
        .expect("app service");
    assert_eq!(app.healthcheck_path.as_deref(), Some("/ready"));
    assert_eq!(spec.topology.validation.preview_paths, vec!["/ready"]);

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(
        compose.contains("curl -fsS http://127.0.0.1:8080/ready || exit 1"),
        "{compose}"
    );
}

#[test]
fn deploy_prepare_applies_backend_healthcheck_without_inventing_public_gateway_probe() {
    let fixture = Fixture::new("deploy-healthcheck-input-composite");
    fixture.write_runtime_delivery(runtime_delivery());

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: Some("actuator/health".to_string()),
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert_eq!(
        spec.runtime_contract.health_path.as_deref(),
        Some("/actuator/health")
    );
    let backend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "backend")
        .expect("backend service");
    assert_eq!(
        backend.healthcheck_path.as_deref(),
        Some("/actuator/health")
    );
    assert_eq!(spec.topology.validation.preview_paths, vec!["/"]);
    assert!(!spec
        .topology
        .validation
        .api_probes
        .iter()
        .any(|probe| probe.path == "/actuator/health"));

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(
        compose.contains("curl -fsS http://127.0.0.1:8080/actuator/health || exit 1"),
        "{compose}"
    );
}

#[test]
fn prepare_uses_repository_code_evidence_for_gradle_vite_workspace() {
    let fixture = Fixture::new("deploy-gradle-vite");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "spring boot service with vite web",
        "deploymentShape": "single-service",
        "build": { "command": "service: ./mvnw test && ./mvnw package; web: npm run build" },
        "start": { "command": "service: ./mvnw spring-boot:run; web: npm run dev", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "service/build.gradle",
        "plugins { id 'org.springframework.boot' version '3.5.6' }\n",
    );
    fixture.write_text("service/gradlew", "#!/bin/sh\n");
    fixture.write_text(
        "service/src/main/resources/application.properties",
        "server.port=8080\n",
    );
    fixture.write_text(
        "web/package.json",
        r#"{"scripts":{"build":"vite"},"dependencies":{"react":"latest"}}"#,
    );
    fixture.write_text("web/package-lock.json", "{}\n");
    fixture.write_text("web/vite.config.ts", "export default {}\n");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("result json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec: DeploymentSpec = read_json(&fixture.root.join(".loom/deployment/specs/local.json"))
        .expect("deployment spec");
    let service = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "app")
        .expect("app service");
    assert_eq!(spec.source_model.source, SourceModelSource::RuntimeContract);
    assert_eq!(service.package_manager, Some(PackageManager::Gradle));
    assert_eq!(service.framework.as_deref(), Some("spring-boot"));
    assert_eq!(
        service.build_command.as_deref(),
        Some("cd service && chmod +x ./gradlew && ./gradlew bootJar --no-daemon")
    );
    assert_eq!(
        service.workspace_package_json_paths,
        vec!["web/package.json"]
    );
    assert_eq!(service.output_directory.as_deref(), Some("web/dist"));
    assert_eq!(spec.source_model.build_context_path, "../../../..");

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(compose.contains("context: ../../../.."), "{compose}");
    assert!(
        compose.contains("dockerfile: .loom/deployment/specs/generated/Dockerfile.app"),
        "{compose}"
    );
    assert!(!compose.contains("additional_contexts"), "{compose}");

    let dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.app"),
    )
    .expect("dockerfile");
    assert!(
        dockerfile.contains("FROM node:22-bookworm-slim AS web-builder"),
        "{dockerfile}"
    );
    assert!(
        dockerfile.contains("cd service && chmod +x ./gradlew && ./gradlew bootJar --no-daemon"),
        "{dockerfile}"
    );
    assert!(
        dockerfile.contains("jar --update --file /tmp/app.jar"),
        "{dockerfile}"
    );
    assert!(!dockerfile.contains("./mvnw"), "{dockerfile}");
    assert!(fixture
        .root
        .join(".loom/deployment/specs/generated/Dockerfile.app.dockerignore")
        .exists());
    assert_eq!(
        spec.files.dockerignore_paths["app"],
        ".loom/deployment/specs/generated/Dockerfile.app.dockerignore"
    );
    let spec_json: Value =
        read_json(&fixture.root.join(".loom/deployment/specs/local.json")).expect("spec json");
    assert!(
        spec_json["files"].get("dockerignorePath").is_none(),
        "{spec_json:#}"
    );

    let evidence: Value = read_json(
        &fixture
            .root
            .join(".loom/deployment/evidence/latest-code-evidence.json"),
    )
    .expect("code evidence");
    assert_eq!(evidence["source"], "code_probe");
    assert!(serde_json::to_string(&evidence)
        .unwrap()
        .contains("service/build.gradle"));
}

#[test]
fn prepare_uses_previous_phase_runtime_delivery_when_active_phase_has_no_aac() {
    let fixture = Fixture::new("deploy-previous-phase-runtime");
    fixture.write_runtime_delivery(plugin_style_dual_service_runtime_delivery());
    fixture.write_text(
        "frontend/package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"@vitejs/plugin-react":"latest","vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("frontend/package-lock.json", "{}\n");
    fixture.write_text("frontend/vite.config.js", "export default {}\n");
    fixture.write_text(
        "backend/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>demo</artifactId></project>"#,
    );
    fixture.write_text("backend/mvnw", "#!/bin/sh\n");
    fixture.write_text(
        "backend/src/main/resources/application.yml",
        r#"spring:
  datasource:
    url: ${DATABASE_URL:jdbc:sqlite:purchase-approval.db}
  jpa:
    hibernate:
      ddl-auto: validate
  flyway:
    enabled: true
"#,
    );

    let index_path = fixture.root.join(".loom/deliveries/delivery-1/index.json");
    let mut index: Value = read_json(&index_path).expect("delivery index");
    index["activePhaseId"] = json!("phase-2");
    index["phases"].as_array_mut().expect("phases").push(json!({
        "phaseId": "phase-2",
        "latestRefs": {}
    }));
    write_json_atomic(&index_path, &index).expect("write phase-2 index");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert_eq!(
        spec.runtime_contract.authority,
        DeploymentContractAuthority::AcceptedContract
    );
    assert!(spec
        .runtime_contract
        .r#ref
        .as_deref()
        .expect("runtime contract ref")
        .contains("contracts/architecture/phase-1/aac.json#/runtimeDelivery"));
    assert_eq!(
        spec.runtime_contract.deployment_shape,
        Some(DeploymentShape::FrontendAndBackend)
    );
    assert_eq!(spec.source_model.shape, DeploymentShape::FrontendAndBackend);
    let frontend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "frontend")
        .expect("frontend service");
    let backend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "backend")
        .expect("backend service");
    assert_eq!(frontend.root, "frontend");
    assert_eq!(frontend.output_directory.as_deref(), Some("frontend/dist"));
    assert_eq!(frontend.build_command.as_deref(), Some("npm run build"));
    assert_eq!(frontend.package_manager, Some(PackageManager::Npm));
    assert!(frontend.has_lockfile);
    assert_eq!(frontend.framework.as_deref(), Some("vite"));
    assert_eq!(backend.root, "backend");
    assert_eq!(backend.runtime_kind, RuntimeKind::Java);
    assert_eq!(backend.package_manager, Some(PackageManager::Maven));
    assert!(!backend.has_lockfile);
    assert_eq!(backend.framework.as_deref(), Some("spring-boot"));
    assert_eq!(backend.build_command.as_deref(), Some("./mvnw package"));
    assert_eq!(backend.start_command, None);
    assert_eq!(backend.healthcheck_path.as_deref(), Some("/api/health"));
    assert_eq!(
        spec.environment.generated["DATABASE_URL"],
        "jdbc:sqlite:/app/data/purchase-approval.db"
    );
    assert_eq!(
        spec.environment.generated["SPRING_JPA_HIBERNATE_DDL_AUTO"],
        "none"
    );
    assert!(
        spec.environment.missing.is_empty(),
        "{:#?}",
        spec.environment
    );

    let dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.backend"),
    )
    .expect("backend dockerfile");
    assert!(dockerfile.contains("FROM maven:3-eclipse-temurin-21 AS builder"));
    assert!(dockerfile.contains("WORKDIR /workspace/backend"));
    assert!(dockerfile.contains("COPY backend/pom.xml ./"));
    assert!(dockerfile.contains("COPY backend/src ./src"));
    assert!(dockerfile.contains("RUN mvn -DskipTests package"));
    assert!(!dockerfile.contains("RUN ./mvnw package"), "{dockerfile}");

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(
        compose.contains("curl -fsS http://127.0.0.1:8080/api/health || exit 1"),
        "{compose}"
    );
    assert!(
        compose.contains("      backend:\n        condition: service_healthy"),
        "{compose}"
    );
    assert!(
        compose.contains("DATABASE_URL: jdbc:sqlite:/app/data/purchase-approval.db"),
        "{compose}"
    );
    assert!(
        compose.contains("SPRING_JPA_HIBERNATE_DDL_AUTO: none"),
        "{compose}"
    );
    assert!(
        compose.contains("      - backend-data:/app/data"),
        "{compose}"
    );
    assert!(compose.contains("volumes:\n  backend-data:"), "{compose}");

    let frontend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("frontend dockerfile");
    assert!(
        !frontend_dockerfile.contains("API_BASE_URL"),
        "{frontend_dockerfile}"
    );
    assert_eq!(spec.frontend_api_binding.status, "not_applicable");
}

#[test]
fn prepare_promotes_single_service_frontend_api_contract_to_proxy_topology() {
    let fixture = Fixture::new("deploy-root-composite-single-shape");
    fixture.write_runtime_delivery_with_interfaces(
        json!({
            "status": "modified",
            "runtimeKind": "web_frontend_plus_api",
            "deploymentShape": "single-service",
            "build": {
                "command": "frontend: npm run build ; backend: ./mvnw -DskipTests package",
                "workingDirectory": "."
            },
            "start": {
                "command": "frontend: npm run dev ; backend: ./mvnw spring-boot:run",
                "workingDirectory": "."
            },
            "httpProbes": {
                "previewPath": "/",
                "apiPaths": ["/api/health", "/api/purchase-requests"],
                "expectedStatus": "2xx_or_3xx"
            },
            "runtimeSurfaces": [
                { "surfaceId": "runtime_surface_frontend", "kind": "frontend", "urlPath": "/" },
                { "surfaceId": "runtime_surface_api", "kind": "api", "urlPath": "/api" }
            ],
            "environment": {
                "required": ["DATABASE_URL"],
                "optional": ["SERVER_PORT", "SPRING_PROFILES_ACTIVE"]
            }
        }),
        json!([
            {
                "interfaceId": "api.purchase.health",
                "type": "http_api",
                "method": "GET",
                "path": "/api/health"
            },
            {
                "interfaceId": "api.purchase.list",
                "type": "http_api",
                "method": "GET",
                "path": "/api/purchase-requests"
            }
        ]),
    );
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","dev":"vite --host 0.0.0.0"},"dependencies":{"@vitejs/plugin-react":"latest","vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("package-lock.json", "{}\n");
    fixture.write_text("vite.config.js", "export default {}\n");
    fixture.write_text(
        "pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>purchase</artifactId><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies></project>"#,
    );
    fixture.write_text("mvnw", "#!/bin/sh\n");
    fixture.write_text(
        "src/main/resources/application.yml",
        "spring:\n  application:\n    name: purchase\n",
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert_eq!(
        spec.runtime_contract.deployment_shape,
        Some(DeploymentShape::FrontendAndBackend)
    );
    assert_eq!(spec.source_model.shape, DeploymentShape::FrontendAndBackend);
    assert_eq!(spec.source_model.primary_service_id, "backend");
    assert_eq!(spec.source_model.preview_service_id, "frontend");
    assert_eq!(spec.topology.public_entry_service_id, "frontend");
    assert!(serde_json::to_string(&spec.topology)
        .unwrap()
        .contains("\"kind\":\"http-proxy\""));
    assert!(serde_json::to_string(&spec.topology)
        .unwrap()
        .contains("\"publicPath\":\"/api\""));

    let frontend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "frontend")
        .expect("frontend service");
    let backend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "backend")
        .expect("backend service");
    assert_eq!(frontend.root, ".");
    assert_eq!(frontend.manifest_refs, vec!["package.json"]);
    assert_eq!(frontend.lockfile_refs, vec!["package-lock.json"]);
    assert_eq!(frontend.output_directory.as_deref(), Some("dist"));
    assert_eq!(frontend.build_command.as_deref(), Some("npm run build"));
    assert_eq!(frontend.package_manager, Some(PackageManager::Npm));
    assert!(frontend.has_lockfile);
    assert_eq!(backend.root, ".");
    assert_eq!(backend.manifest_refs, vec!["pom.xml"]);
    assert_eq!(
        backend.build_command.as_deref(),
        Some("./mvnw -DskipTests package")
    );
    assert_eq!(backend.package_manager, Some(PackageManager::Maven));
    assert_eq!(backend.framework.as_deref(), Some("spring-boot"));
    assert_eq!(backend.healthcheck_path.as_deref(), Some("/api/health"));

    let frontend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("frontend dockerfile");
    assert!(frontend_dockerfile.contains("WORKDIR /workspace"));
    assert!(frontend_dockerfile.contains("COPY package-lock.json package.json ./"));
    assert!(frontend_dockerfile.contains("RUN npm run build"));
    assert!(
        !frontend_dockerfile.contains("./mvnw"),
        "{frontend_dockerfile}"
    );

    let backend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.backend"),
    )
    .expect("backend dockerfile");
    assert!(backend_dockerfile.contains("COPY pom.xml ./"));
    assert!(backend_dockerfile.contains("RUN mvn -DskipTests package"));
    assert!(
        !backend_dockerfile.contains("npm run build"),
        "{backend_dockerfile}"
    );

    let nginx = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/nginx.frontend.conf"),
    )
    .expect("nginx config");
    assert!(nginx.contains("location = /api"), "{nginx}");
    assert!(nginx.contains("proxy_pass http://backend:8080;"), "{nginx}");

    let validate = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let validate_value = serde_json::to_value(validate).expect("validate json");
    assert_eq!(
        validate_value["details"]["assetIssues"],
        json!([]),
        "{validate_value:#}"
    );

    let evidence: Value = read_json(
        &fixture
            .root
            .join(".loom/deployment/evidence/latest-code-evidence.json"),
    )
    .expect("code evidence");
    assert_eq!(evidence["repositoryShape"], "multi_application");
    assert!(serde_json::to_string(&evidence)
        .unwrap()
        .contains("frontend_package_file"));
}

#[test]
fn deploy_validate_allows_single_app_api_paths_without_proxy_topology() {
    let fixture = Fixture::new("deploy-api-single-service-no-proxy");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "spring boot api",
        "deploymentShape": "single-service",
        "build": { "command": "./mvnw -DskipTests package" },
        "start": { "command": "./mvnw spring-boot:run", "port": 8080 },
        "httpProbes": {
            "previewPath": "/api/health",
            "apiPaths": ["/api/health"]
        },
        "runtimeSurfaces": [
            { "surfaceId": "runtime_surface_api", "kind": "api", "urlPath": "/api" }
        ],
        "environment": {
            "required": [],
            "optional": []
        }
    }));
    fixture.write_text(
        "pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>api</artifactId><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies></project>"#,
    );
    fixture.write_text("mvnw", "#!/bin/sh\n");
    fixture.write_text("src/main/java/app/App.java", "class App {}\n");

    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");

    let spec = fixture.read_spec();
    assert_eq!(spec.source_model.shape, DeploymentShape::SingleService);
    assert!(spec.topology.routes.is_empty(), "{:#?}", spec.topology);

    let validate = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let validate_value = serde_json::to_value(validate).expect("validate json");
    assert!(
        !validate_value["details"]["assetIssues"]
            .as_array()
            .expect("asset issues")
            .iter()
            .any(|issue| issue
                .as_str()
                .unwrap_or_default()
                .contains("topology has API probes but no http-proxy route")),
        "{validate_value:#}"
    );
}

#[test]
fn prepare_builds_deploy_facts_for_supported_single_service_stacks() {
    assert_probe_stack(
        "deploy-python-matrix",
        &[
            (
                "pyproject.toml",
                "[project]\ndependencies = ['fastapi', 'uvicorn']\n",
            ),
            ("main.py", "from fastapi import FastAPI\napp = FastAPI()\n"),
        ],
        RuntimeKind::Python,
        "deploy.stacks.python",
        "single_service_app",
    );
    assert_probe_stack(
        "deploy-go-matrix",
        &[
            ("services/api/go.mod", "module example.com/app\n"),
            ("services/api/main.go", "package main\nfunc main() {}\n"),
        ],
        RuntimeKind::Go,
        "deploy.stacks.go",
        "single_service_app",
    );
    assert_probe_stack(
        "deploy-dotnet-matrix",
        &[
            (
                "apps/api/App.csproj",
                "<Project Sdk=\"Microsoft.NET.Sdk.Web\"><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>\n",
            ),
            (
                "apps/api/Program.cs",
                "var builder = WebApplication.CreateBuilder(args);\n",
            ),
        ],
        RuntimeKind::Dotnet,
        "deploy.stacks.dotnet",
        "single_service_app",
    );
    assert_probe_stack(
        "deploy-php-matrix",
        &[
            (
                "composer.json",
                "{\"require\":{\"laravel/framework\":\"^11.0\"}}\n",
            ),
            ("public/index.php", "<?php echo 'ok';\n"),
        ],
        RuntimeKind::Php,
        "deploy.stacks.php",
        "single_service_app",
    );
    assert_probe_stack(
        "deploy-ruby-matrix",
        &[
            ("Gemfile", "gem 'rails'\n"),
            (
                "config/application.rb",
                "module Demo; class Application < Rails::Application; end; end\n",
            ),
        ],
        RuntimeKind::Ruby,
        "deploy.stacks.ruby",
        "single_service_app",
    );
    assert_probe_stack(
        "deploy-static-matrix",
        &[("index.html", "<!doctype html><div>ok</div>\n")],
        RuntimeKind::Static,
        "deploy.stacks.static",
        "static_site",
    );
}

#[test]
fn prepare_splits_cd_combined_commands_into_service_level_model() {
    let fixture = Fixture::new("deploy-cd-combined-service-model");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "react web plus spring boot api",
        "deploymentShape": "frontend-and-backend",
        "build": { "command": "cd web && npm run build && cd ../server && ./mvnw -q package -DskipTests" },
        "start": { "command": "cd server && ./mvnw spring-boot:run & cd ../web && npm run dev", "port": 8080 },
        "httpProbes": {
            "previewPath": "/requests",
            "apiPaths": ["/api/health", "/api/replenishment-requests"]
        },
        "environment": {
            "required": ["DATABASE_URL"],
            "optional": ["APP_PORT", "WEB_PORT"]
        }
    }));
    fixture.write_text(
        "web/package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("web/package-lock.json", "{}\n");
    fixture.write_text("web/vite.config.ts", "export default {}\n");
    fixture.write_text(
        "server/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>replenishment</artifactId></project>"#,
    );
    fixture.write_text("server/mvnw", "#!/bin/sh\n");
    fixture.write_text("server/src/main/java/app/App.java", "class App {}\n");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    let frontend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "frontend")
        .expect("frontend service");
    let backend = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "backend")
        .expect("backend service");
    assert_eq!(frontend.root, "web");
    assert_eq!(frontend.build_command.as_deref(), Some("npm run build"));
    assert_eq!(frontend.manifest_refs, vec!["web/package.json"]);
    assert_eq!(frontend.lockfile_refs, vec!["web/package-lock.json"]);
    assert_eq!(frontend.artifact_refs, vec!["web/dist"]);
    assert_eq!(backend.root, "server");
    assert_eq!(
        backend.build_command.as_deref(),
        Some("./mvnw -q package -DskipTests")
    );
    assert_eq!(backend.package_manager, Some(PackageManager::Maven));
    assert_eq!(backend.manifest_refs, vec!["server/pom.xml"]);
    assert!(backend
        .artifact_refs
        .iter()
        .any(|item| item == "server/target/*.jar"));

    let frontend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.frontend"),
    )
    .expect("frontend dockerfile");
    assert!(frontend_dockerfile.contains("WORKDIR /workspace/web"));
    assert!(frontend_dockerfile.contains("COPY web/package-lock.json web/package.json ./"));
    assert!(frontend_dockerfile.contains("COPY web/ ./"));
    assert!(frontend_dockerfile.contains("RUN npm run build"));
    assert!(
        frontend_dockerfile
            .contains("COPY --from=builder /workspace/web/dist /usr/share/nginx/html"),
        "{frontend_dockerfile}"
    );

    let backend_dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.backend"),
    )
    .expect("backend dockerfile");
    assert!(backend_dockerfile.contains("WORKDIR /workspace/server"));
    assert!(backend_dockerfile.contains("COPY server/pom.xml ./"));
    assert!(backend_dockerfile.contains("COPY server/src ./src"));
    assert!(backend_dockerfile.contains("RUN mvn -q package -DskipTests"));
    assert!(
        !backend_dockerfile.contains("npm run build"),
        "{backend_dockerfile}"
    );
}

#[test]
fn prepare_generates_file_database_env_from_code_probe_when_contract_omits_it() {
    let fixture = Fixture::new("deploy-code-probe-file-db-env");
    let mut runtime = plugin_style_dual_service_runtime_delivery();
    runtime["environment"]["required"] = json!([]);
    fixture.write_runtime_delivery(runtime);
    fixture.write_text(
        "frontend/package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"@vitejs/plugin-react":"latest","vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("frontend/package-lock.json", "{}\n");
    fixture.write_text("frontend/vite.config.js", "export default {}\n");
    fixture.write_text(
        "backend/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>demo</artifactId></project>"#,
    );
    fixture.write_text("backend/mvnw", "#!/bin/sh\n");
    fixture.write_text(
        "backend/src/main/resources/application.yml",
        r#"spring:
  datasource:
    url: ${DATABASE_URL:jdbc:h2:file:./var/purchase-approval;MODE=PostgreSQL}
  jpa:
    hibernate:
      ddl-auto: validate
  flyway:
    enabled: true
"#,
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert!(
        spec.environment
            .required
            .iter()
            .all(|variable| variable.name != "DATABASE_URL"),
        "{:#?}",
        spec.environment.required
    );
    assert_eq!(
        spec.environment.generated["DATABASE_URL"],
        "jdbc:h2:file:/app/data/purchase-approval;MODE=PostgreSQL"
    );
    assert_eq!(
        spec.environment.generated["SPRING_JPA_HIBERNATE_DDL_AUTO"],
        "none"
    );

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(
        compose.contains("DATABASE_URL: jdbc:h2:file:/app/data/purchase-approval;MODE=PostgreSQL"),
        "{compose}"
    );
    assert!(
        compose.contains("SPRING_JPA_HIBERNATE_DDL_AUTO: none"),
        "{compose}"
    );
    assert!(
        compose.contains("      - backend-data:/app/data"),
        "{compose}"
    );
}

#[test]
fn prepare_without_loom_delivery_uses_repository_code_probe() {
    let fixture = Fixture::new("deploy-no-delivery-code-probe");
    fixture.write_text(
        "frontend/package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("frontend/package-lock.json", "{}\n");
    fixture.write_text("frontend/vite.config.js", "export default {}\n");
    fixture.write_text(
        "backend/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>demo</artifactId></project>"#,
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    assert_eq!(
        spec.runtime_contract.authority,
        DeploymentContractAuthority::RepositoryHeuristic
    );
    assert_eq!(spec.runtime_contract.r#ref, None);
    assert_eq!(spec.source_model.source, SourceModelSource::CodeProbe);
    let service = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "app")
        .expect("app service");
    assert_eq!(service.package_manager, Some(PackageManager::Maven));
    assert_eq!(
        service.build_command.as_deref(),
        Some("cd backend && mvn -DskipTests package")
    );
    assert_eq!(
        service.workspace_package_json_paths,
        vec!["frontend/package.json"]
    );
    assert_eq!(service.output_directory.as_deref(), Some("frontend/dist"));
}

#[test]
fn prepare_promotes_composite_runtime_above_preview_app_path() {
    let fixture = Fixture::new("deploy-composite-app-path");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "spring boot service with vite web",
        "deploymentShape": "single-service",
        "build": { "command": "service: ./gradlew test && ./gradlew bootJar; web: npm run build" },
        "start": { "command": "service: ./gradlew bootRun; web: npm run dev", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "service/build.gradle",
        "plugins { id 'org.springframework.boot' version '3.5.6' }\n",
    );
    fixture.write_text("service/gradlew", "#!/bin/sh\n");
    fixture.write_text(
        "web/package.json",
        r#"{"scripts":{"build":"vite"},"dependencies":{"react":"latest"}}"#,
    );
    fixture.write_text("web/package-lock.json", "{}\n");
    fixture.write_text("web/vite.config.ts", "export default {}\n");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: Some("web".to_string()),
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("result json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec: DeploymentSpec = read_json(&fixture.root.join(".loom/deployment/specs/local.json"))
        .expect("deployment spec");
    let service = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == "app")
        .expect("app service");
    assert_eq!(spec.source_model.build_context_path, "../../../..");
    assert_eq!(service.runtime_kind, contracts::RuntimeKind::Java);
    assert_eq!(service.package_manager, Some(PackageManager::Gradle));
    assert_eq!(service.framework.as_deref(), Some("spring-boot"));
    assert_eq!(
        service.workspace_package_json_paths,
        vec!["web/package.json"]
    );
    assert_eq!(service.output_directory.as_deref(), Some("web/dist"));
    assert_eq!(
        service.build_command.as_deref(),
        Some("cd service && chmod +x ./gradlew && ./gradlew bootJar --no-daemon")
    );
    assert!(!serde_json::to_string(&spec.source_model)
        .unwrap()
        .contains("npm run preview"));

    let compose = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/compose.yaml"),
    )
    .expect("compose");
    assert!(compose.contains("context: ../../../.."), "{compose}");

    let dockerfile = read_text(
        &fixture
            .root
            .join(".loom/deployment/specs/generated/Dockerfile.app"),
    )
    .expect("dockerfile");
    assert!(
        dockerfile.contains("FROM node:22-bookworm-slim AS web-builder"),
        "{dockerfile}"
    );
    assert!(
        dockerfile.contains("FROM eclipse-temurin:21-jdk AS service-builder"),
        "{dockerfile}"
    );
    assert!(
        dockerfile.contains("COPY --from=service-builder /tmp/app.jar /app/app.jar"),
        "{dockerfile}"
    );
    assert!(!dockerfile.contains("FROM nginx"), "{dockerfile}");

    let evidence: Value = read_json(
        &fixture
            .root
            .join(".loom/deployment/evidence/latest-code-evidence.json"),
    )
    .expect("code evidence");
    assert_eq!(evidence["projectRoot"], fixture.root_str());
    assert_eq!(evidence["repositoryShape"], "multi_application");
}

#[test]
fn deploy_prepare_returns_refs_and_compact_summaries_without_full_spec_sections() {
    let fixture = Fixture::new("deploy-prepare-compact-output");
    fixture.write_runtime_delivery(runtime_delivery());

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");

    assert_eq!(value["state"], "done", "{value:#}");
    let details = &value["details"];
    assert!(details["specRef"].as_str().is_some(), "{value:#}");
    assert!(details["sourceModelRef"].as_str().is_some(), "{value:#}");
    assert!(details["topologyRef"].as_str().is_some(), "{value:#}");
    assert!(details["codeEvidenceRef"].as_str().is_some(), "{value:#}");
    assert!(details["factsRef"].as_str().is_some(), "{value:#}");
    assert!(details["deployFactsSummary"].is_object(), "{value:#}");
    assert!(details["sourceModelSummary"].is_object(), "{value:#}");
    assert!(details["topologySummary"].is_object(), "{value:#}");
    assert!(details["generatedFileRefs"].is_array(), "{value:#}");
    assert!(details["primaryUrl"].as_str().is_some(), "{value:#}");
    assert!(details["ports"].as_array().is_some(), "{value:#}");
    assert!(
        details.get("sourceModel").is_none()
            && details.get("topology").is_none()
            && details.get("generatedFiles").is_none(),
        "deploy prepare must not inline full deploy spec sections: {value:#}"
    );
    assert_forbidden_cli_fields_absent(&value);
}

#[test]
fn deploy_prepare_auto_resolves_occupied_preferred_host_port() {
    let fixture = Fixture::new("deploy-port-fallback");
    let occupied = TcpListener::bind(("127.0.0.1", 0)).expect("bind occupied test port");
    let preferred_port = occupied.local_addr().expect("occupied test addr").port();
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "httpProbes": { "previewPath": "/" },
        "start": { "command": "npm run start", "port": preferred_port }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite","start":"vite --host 0.0.0.0"}}"#,
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");

    let spec = fixture.read_spec();
    let preview = runtime_port(&spec, "app").expect("app runtime port");
    assert_eq!(preview.preferred_host_port, Some(preferred_port));
    assert_ne!(preview.host_port, Some(preferred_port));
    assert!(preview.host_port.is_some());
    assert_ne!(
        runtime_primary_url(&spec),
        format!("http://localhost:{preferred_port}")
    );
}

#[test]
fn deploy_prepare_prefers_existing_compose_without_inlining_or_editing_it() {
    let fixture = Fixture::new("deploy-compose-existing");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "compose.yaml",
        "services:\n  web:\n    build: .\n    ports:\n      - \"5555:8080\"\n",
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(
        value["details"]["provider"], "compose-existing",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["composeSummary"]["selectedService"], "web",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["generatedFileRefs"],
        json!([]),
        "{value:#}"
    );
    assert_eq!(value["details"]["reusedFileRefs"], json!(["compose.yaml"]));

    let spec = fixture.read_spec();
    assert_eq!(spec.provider, DeployProvider::ComposeExisting);
    assert_eq!(spec.files.compose_path, "compose.yaml");
    let preview_port = spec
        .runtime
        .ports
        .iter()
        .find(|port| port.purpose == "preview")
        .expect("preview port");
    assert_eq!(preview_port.host_port, Some(5555));
    assert_eq!(preview_port.container_port, 8080);
    assert!(!fixture
        .root
        .join(".loom/deployment/specs/generated/compose.yaml")
        .exists());
}

#[test]
fn deploy_prepare_reuses_existing_dockerfile_and_generates_only_wrapper_assets() {
    let fixture = Fixture::new("deploy-dockerfile-existing");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "Dockerfile",
        "FROM node:22\nCMD [\"node\", \"server.js\"]\n",
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(
        value["details"]["provider"], "dockerfile-existing",
        "{value:#}"
    );
    assert_eq!(value["details"]["reusedFileRefs"], json!(["Dockerfile"]));
    assert!(value["details"]["generatedFileRefs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == ".loom/deployment/specs/generated/compose.yaml"));
    assert!(!value["details"]["generatedFileRefs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == ".loom/deployment/specs/generated/Dockerfile.app.dockerignore"));

    let spec = fixture.read_spec();
    assert_eq!(spec.provider, DeployProvider::DockerfileExisting);
    assert_eq!(spec.files.dockerfile_paths["app"], "Dockerfile");
    assert!(spec.files.dockerignore_paths.is_empty());
    assert!(fixture
        .root
        .join(".loom/deployment/specs/generated/compose.yaml")
        .exists());
    assert!(!fixture
        .root
        .join(".loom/deployment/specs/generated/Dockerfile.app")
        .exists());
}

#[test]
fn deploy_prepare_force_generate_ignores_existing_assets() {
    let fixture = Fixture::new("deploy-force-generated");
    fixture.write_runtime_delivery(runtime_delivery());
    fixture.write_text("compose.yaml", "services:\n  web:\n    image: nginx\n");
    fixture.write_text("Dockerfile", "FROM nginx\n");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: Some(DeploymentProviderPolicy {
            provider: Some(DeployProvider::Generated),
            reuse_existing: false,
            force_generate: true,
        }),
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(value["details"]["provider"], "generated", "{value:#}");
    assert_eq!(value["details"]["reusedFileRefs"], json!([]));

    let spec = fixture.read_spec();
    assert_eq!(spec.provider, DeployProvider::Generated);
    assert!(fixture
        .root
        .join(".loom/deployment/specs/generated/Dockerfile.frontend")
        .exists());
}

#[test]
fn deploy_prepare_uses_app_path_when_selecting_existing_assets() {
    let fixture = Fixture::new("deploy-app-path-existing");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text("apps/api/Dockerfile", "FROM node:22\n");
    fixture.write_text(
        "apps/api/package.json",
        r#"{"scripts":{"start":"node server.js"}}"#,
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: Some("apps/api".to_string()),
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(
        value["details"]["provider"], "dockerfile-existing",
        "{value:#}"
    );

    let spec = fixture.read_spec();
    assert_eq!(spec.provider, DeployProvider::DockerfileExisting);
    assert_eq!(spec.files.reused, vec!["apps/api/Dockerfile"]);
    assert_eq!(spec.source_model.build_context_path, "../../../../apps/api");
}

#[cfg(unix)]
#[test]
fn deploy_up_auto_falls_back_from_existing_provider_to_generated() {
    let fixture = Fixture::new("deploy-existing-fallback-generated");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text("compose.yaml", "services:\n  web:\n    image: nginx\n");
    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["details"]["provider"], "compose-existing");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then
  echo "compose config failed for $3" >&2
  exit 2
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(fixture.read_spec().provider, DeployProvider::Generated);
    let reference_profile = &value["next"]["deployReferenceProfile"];
    for expected in [
        "deploy.providers",
        "deploy.compose",
        "deploy.dockerfile",
        "deploy.repair",
        "deploy.stacks.node",
    ] {
        assert_reference_load_plan_contains(reference_profile, expected);
    }
    assert_reference_load_plan_excludes(reference_profile, "deploy.external-references");
    assert_deploy_reference_profile_shape(reference_profile);
    let repair = fixture.repair_action_value();
    assert!(!repair["protectedFiles"]
        .as_array()
        .map(|items| items.iter().any(|item| item == "compose.yaml"))
        .unwrap_or(false));
    assert!(!repair["editableFiles"]
        .as_array()
        .map(|items| items.iter().any(|item| item == "compose.yaml"))
        .unwrap_or(false));
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
}

#[cfg(unix)]
#[test]
fn deploy_up_respects_forced_existing_provider_without_fallback() {
    let fixture = Fixture::new("deploy-forced-existing-no-fallback");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text("compose.yaml", "services:\n  web:\n    image: nginx\n");
    let policy = DeploymentProviderPolicy {
        provider: Some(DeployProvider::ComposeExisting),
        reuse_existing: true,
        force_generate: false,
    };
    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: Some(policy.clone()),
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["details"]["provider"], "compose-existing");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then
  echo "forced compose config failed" >&2
  exit 2
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: Some(policy),
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(
        fixture.read_spec().provider,
        DeployProvider::ComposeExisting
    );
    let repair = fixture.repair_action_value();
    assert_eq!(repair["repairRoute"], "none");
    assert!(repair["editableFiles"]
        .as_array()
        .map(Vec::is_empty)
        .unwrap_or(true));
    assert_eq!(repair["protectedFiles"], json!(["compose.yaml"]));
    assert_eq!(value["state"], "blocked", "{value:#}");
}

#[cfg(unix)]
#[test]
fn deploy_up_updates_active_operation_phase_and_streams_docker_logs() {
    let fixture = Fixture::new("deploy-active-operation-logs");
    let _heartbeat_guard = EnvVarGuard::set("LOOM_DEPLOY_OPERATION_HEARTBEAT_MS", "100");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite","start":"vite --host 0.0.0.0"}}"#,
    );
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then
  echo "config ok"
  exit 0
fi
if [ "$1" = "compose" ] && [ "$4" = "up" ]; then
  echo "build started"
  sleep 1
  echo "build done"
  exit 0
fi
if [ "$1" = "compose" ] && [ "$4" = "logs" ]; then
  echo "app did not become reachable"
  exit 0
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();
    let root = fixture.root_str();
    let handle = thread::spawn(move || {
        deploy_up(DeployToolInput {
            project_root: root,
            app_path: None,
            healthcheck: None,
            provider_policy: Some(DeploymentProviderPolicy {
                provider: Some(DeployProvider::Generated),
                reuse_existing: false,
                force_generate: true,
            }),
        })
    });

    let active_file = fixture
        .root
        .join(".loom/deployment/state/active-operation.json");
    let log_file = fixture.root.join(".loom/deployment/logs/local.log");
    let mut observed_building = false;
    let mut observed_log = false;
    let mut first_building_updated_at: Option<u128> = None;
    let mut observed_building_heartbeat = false;
    for _ in 0..40 {
        if active_file.exists() {
            if let Ok(active) = read_json::<Value>(&active_file) {
                observed_building |= active["phase"] == "building";
                if active["phase"] == "building" {
                    if let Some(updated_at) = active["updatedAt"]
                        .as_str()
                        .and_then(|value| value.parse::<u128>().ok())
                    {
                        if let Some(first) = first_building_updated_at {
                            observed_building_heartbeat |= updated_at > first;
                        } else {
                            first_building_updated_at = Some(updated_at);
                        }
                    }
                }
            }
        }
        if log_file.exists() {
            if let Ok(log) = read_text(&log_file) {
                observed_log |= log.contains("phase=building command=docker compose")
                    && log.contains("build started");
            }
        }
        if observed_building && observed_log && observed_building_heartbeat {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = handle.join().expect("deploy up thread");
    let log = read_text(&log_file).expect("deploy log");
    assert!(observed_building, "{log}");
    assert!(observed_log, "{log}");
    assert!(observed_building_heartbeat, "{log}");
    assert!(
        log.contains("phase=checking_compose command=docker compose"),
        "{log}"
    );
    assert!(log.contains("config ok"), "{log}");
}

#[cfg(unix)]
#[test]
fn deploy_run_reuses_existing_spec_after_asset_repair_without_regenerating() {
    let fixture = Fixture::new("deploy-run-no-regenerate-after-repair");
    fixture.write_runtime_delivery(plugin_style_dual_service_runtime_delivery());
    fixture.write_text(
        "frontend/package.json",
        r#"{"scripts":{"build":"vite build"},"dependencies":{"vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("frontend/package-lock.json", "{}\n");
    fixture.write_text("frontend/vite.config.ts", "export default {}\n");
    fixture.write_text(
        "backend/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><artifactId>demo</artifactId></project>"#,
    );
    fixture.write_text("backend/mvnw", "#!/bin/sh\n");
    fixture.write_text("backend/src/main/java/app/App.java", "class App {}\n");
    let prepared = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    assert_eq!(serde_json::to_value(prepared).unwrap()["state"], "done");
    let dockerfile_path = fixture
        .root
        .join(".loom/deployment/specs/generated/Dockerfile.backend");
    let patched =
        read_text(&dockerfile_path).expect("backend dockerfile") + "\n# patched-by-repair\n";
    std::fs::write(&dockerfile_path, patched).expect("patch generated dockerfile");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "up" ]; then
  echo "failed to connect to the docker API at unix:///tmp/docker.sock" >&2
  exit 1
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_run(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy run json");
    assert_eq!(value["state"], "blocked", "{value:#}");
    let dockerfile = read_text(&dockerfile_path).expect("backend dockerfile after deployRun");
    assert!(
        dockerfile.contains("# patched-by-repair"),
        "deployRun must not regenerate generated assets when a spec already exists: {dockerfile}"
    );
}

#[cfg(unix)]
#[test]
fn deploy_up_prevalidates_generated_assets_before_docker_build() {
    let fixture = Fixture::new("deploy-up-asset-preflight");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run preview", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","preview":"vite preview"},"dependencies":{"vite":"latest"}}"#,
    );
    let prepared = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    assert_eq!(serde_json::to_value(prepared).unwrap()["state"], "done");
    let dockerfile_path = fixture
        .root
        .join(".loom/deployment/specs/generated/Dockerfile.app");
    let dockerfile = read_text(&dockerfile_path)
        .expect("dockerfile")
        .replace("COPY package.json ./", "COPY missing/package.json ./");
    std::fs::write(&dockerfile_path, dockerfile).expect("write invalid dockerfile");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "up" ]; then
  echo "compose up should not run after asset preflight failure" >&2
  exit 99
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
    assert_eq!(value["next"]["kind"], "deploy_repair_assets");
    assert_eq!(
        fixture.repair_action_value()["failureKind"],
        "deploy_asset_invalid"
    );
    assert!(fixture.repair_action_value()["errorWindow"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line
            .as_str()
            .is_some_and(|text| text.contains("missing/package.json"))));
}

#[test]
fn deploy_validate_success_writes_state_and_clears_failure_artifacts() {
    let fixture = Fixture::new("deploy-validate-clears-failure");
    let preferred_port = free_test_port();
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "httpProbes": { "previewPath": "/" },
        "start": { "port": preferred_port }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite","preview":"vite preview"}}"#,
    );
    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    let spec: DeploymentSpec =
        read_json(&fixture.root.join(".loom/deployment/specs/local.json")).expect("spec");
    let _server = spawn_one_shot_http_server(runtime_public_port(&spec));
    fixture.write_text(".loom/deployment/state/latest-failure.json", "{}\n");
    fixture.write_text(".loom/deployment/state/repair-action.json", "{}\n");

    let result = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("validate json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(value["details"]["valid"], true, "{value:#}");
    assert!(fixture
        .root
        .join(".loom/deployment/state/local.json")
        .exists());
    assert!(!fixture
        .root
        .join(".loom/deployment/state/latest-failure.json")
        .exists());
    assert!(!fixture
        .root
        .join(".loom/deployment/state/repair-action.json")
        .exists());
}

#[test]
fn deploy_validate_flags_compose_dockerfile_paths_that_do_not_resolve() {
    let fixture = Fixture::new("deploy-validate-dockerfile-path");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "httpProbes": { "previewPath": "/" },
        "start": { "port": 8080 }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite","preview":"vite preview"}}"#,
    );
    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    let compose_path = fixture
        .root
        .join(".loom/deployment/specs/generated/compose.yaml");
    let compose = read_text(&compose_path).expect("compose");
    assert!(
        compose.contains("dockerfile: .loom/deployment/specs/generated/Dockerfile.app"),
        "{compose}"
    );
    std::fs::write(
        &compose_path,
        compose.replace(
            "dockerfile: .loom/deployment/specs/generated/Dockerfile.app",
            "dockerfile: Dockerfile.app",
        ),
    )
    .expect("write bad compose");

    let result = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("validate json");

    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(value["details"]["valid"], false, "{value:#}");
    assert!(value["details"]["assetIssues"]
        .as_array()
        .expect("asset issues")
        .iter()
        .any(|issue| issue
            .as_str()
            .is_some_and(|text| text.contains("compose dockerfile path"))));
}

#[test]
fn deploy_validate_accepts_json_array_copy_sources_in_existing_dockerfile() {
    let fixture = Fixture::new("deploy-validate-json-copy");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "compose.yaml",
        "services:\n  web:\n    build:\n      context: .\n      dockerfile: Dockerfile\n    ports:\n      - \"5555:8080\"\n",
    );
    fixture.write_text(
        "Dockerfile",
        r#"FROM node:22-alpine
WORKDIR /app
COPY ["package.json", "package-lock.json", "./"]
RUN npm ci
COPY ["server.js", "./"]
CMD ["node", "server.js"]
"#,
    );
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"start":"node server.js"},"dependencies":{}}"#,
    );
    fixture.write_text("package-lock.json", "{}\n");
    fixture.write_text("server.js", "require('http').createServer().listen(8080)\n");

    let prepared = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    assert_eq!(serde_json::to_value(prepared).unwrap()["state"], "done");
    assert_eq!(
        fixture.read_spec().provider,
        DeployProvider::ComposeExisting
    );

    let result = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("validate json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(value["details"]["composeValid"], true, "{value:#}");
    assert_eq!(value["details"]["assetIssues"], json!([]), "{value:#}");
}

#[test]
fn deploy_status_does_not_echo_project_root_inside_state_details() {
    let fixture = Fixture::new("deploy-status-trim");
    let state_dir = fixture.root.join(".loom/deployment/state");
    ensure_dir(&state_dir).expect("state dir");
    write_json_atomic(
        &state_dir.join("local.json"),
        &json!({
            "schemaVersion": 1,
            "projectRoot": fixture.root_str(),
            "running": true,
            "url": "http://127.0.0.1:4173"
        }),
    )
    .expect("write deployment state");

    let result = deploy_status(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("status result json");
    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(value["projectRoot"], fixture.root_str());
    assert!(
        value["details"]["state"].get("projectRoot").is_none(),
        "{value:#}"
    );
}

#[test]
fn deploy_inspect_returns_refs_without_inlining_spec_or_repair_action() {
    let fixture = Fixture::new("deploy-inspect-compact-output");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_repair_action(
        DeploymentRepairRoute::DeployRepair,
        DeploymentFailureOwner::DeploymentAssets,
    );

    let result = deploy_inspect(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("inspect json");

    assert_eq!(value["state"], "done", "{value:#}");
    let details = &value["details"];
    assert_eq!(details["prepared"], true, "{value:#}");
    assert!(details["specRef"].as_str().is_some(), "{value:#}");
    assert!(details["repairRef"].as_str().is_some(), "{value:#}");
    assert_eq!(
        details["repairSummary"]["failureKind"], "api_route_not_verified",
        "{value:#}"
    );
    assert_eq!(
        details["repairSummary"]["repairRoute"], "deploy_repair",
        "{value:#}"
    );
    assert_eq!(
        details["repairSummary"]["nextAction"], "repair_deployment_assets",
        "{value:#}"
    );
    assert_eq!(
        details["repairSummary"]["primaryReason"],
        "api_route_not_verified: Generated API proxy route failed.",
        "{value:#}"
    );
    assert_eq!(
        details["repairSummary"]["errorWindow"]["lines"],
        json!(["failed"]),
        "{value:#}"
    );
    assert!(details["repairSummary"]["sourceRefs"]["repairActionRef"]
        .as_str()
        .is_some());
    assert!(details["sourceModelSummary"].is_object(), "{value:#}");
    assert!(details["topologySummary"].is_object(), "{value:#}");
    assert!(details["generatedFileRefs"].is_array(), "{value:#}");
    assert!(
        details.get("sourceModel").is_none()
            && details.get("topology").is_none()
            && details.get("files").is_none()
            && details.get("repair").is_none(),
        "deploy inspect must not inline full spec or repair action: {value:#}"
    );
    assert!(
        details["repairSummary"].get("projectRoot").is_none()
            && details["repairSummary"].get("specRef").is_none()
            && details["repairSummary"].get("command").is_none(),
        "deploy inspect repair summary must stay compact: {value:#}"
    );
    assert_forbidden_cli_fields_absent(&value);
}

#[test]
fn deploy_active_operation_returns_structured_observation_policy() {
    let fixture = Fixture::new("deploy-active-operation");
    let state_dir = fixture.root.join(".loom/deployment/state");
    ensure_dir(&state_dir).expect("state dir");
    ensure_dir(&fixture.root.join(".loom/deployment/logs")).expect("logs dir");
    write_json_atomic(
        &state_dir.join("active-operation.json"),
        &json!({
            "schemaVersion": 1,
            "operationId": "deploy-op-live",
            "command": "deploy.run",
            "phase": "building",
            "pid": 999999,
            "projectRoot": fixture.root_str(),
            "startedAt": now_string(),
            "updatedAt": now_string(),
            "logRef": ".loom/deployment/logs/local.log",
            "specRef": ".loom/deployment/specs/local.json",
            "status": "running"
        }),
    )
    .expect("write active operation");

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("active operation json");

    assert_eq!(value["state"], "active_operation", "{value:#}");
    assert_eq!(
        value["allowedObservationTools"],
        json!(["loom.deployStatus", "loom.deployInspect", "loom.deployLogs"]),
        "{value:#}"
    );
    assert_eq!(value["observationPolicy"]["quietMode"], true, "{value:#}");
    assert_eq!(
        value["observationPolicy"]["initialQuietWindowMs"], 120_000,
        "{value:#}"
    );
    assert_eq!(
        value["observationPolicy"]["minNextObservationIntervalMs"], 60_000,
        "{value:#}"
    );
    assert_eq!(
        value["observationPolicy"]["finalResponsePolicy"], "forbidden_while_operation_active",
        "{value:#}"
    );
    let forbidden_actions = value["forbiddenActions"]
        .as_array()
        .expect("forbidden actions")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(forbidden_actions
        .iter()
        .any(|action| action.contains("another deploy command")));
    assert!(forbidden_actions
        .iter()
        .any(|action| action.contains("raw Docker")));
    assert!(forbidden_actions
        .iter()
        .any(|action| action.contains("kill")));
    assert!(forbidden_actions
        .iter()
        .any(|action| action.contains("unchanged logs")));
    assert_forbidden_cli_fields_absent(&value);
}

#[test]
fn deploy_repair_assets_next_exposes_refs_and_no_retry_argv() {
    let fixture = Fixture::new("deploy-repair-assets");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_repair_action(
        DeploymentRepairRoute::DeployRepair,
        DeploymentFailureOwner::DeploymentAssets,
    );

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
    assert_eq!(value["next"]["kind"], "deploy_repair_assets");
    assert_eq!(value["next"]["retryTool"], "loom.deployUp");
    assert_eq!(
        value["next"]["primaryReason"], "api_route_not_verified: Generated API proxy route failed.",
        "{value:#}"
    );
    assert_eq!(
        value["next"]["diagnostics"][0]["code"], "api_route_not_verified",
        "{value:#}"
    );
    assert_eq!(
        value["next"]["suggestedActions"],
        json!(["Repair generated API proxy route."]),
        "{value:#}"
    );
    assert!(value["next"]["readPolicy"]["firstRead"]
        .as_str()
        .unwrap()
        .contains("next.primaryReason"));
    assert!(value["next"].get("repairSummary").is_none(), "{value:#}");
    assert!(value["next"]["sourceModelRef"]
        .as_str()
        .unwrap()
        .ends_with("source-model.json"));
    assert!(value["next"]["topologyRef"]
        .as_str()
        .unwrap()
        .ends_with("topology.json"));
    let generated_refs = value["next"]["generatedFileRefs"]
        .as_array()
        .expect("generated file refs");
    let unique_refs = generated_refs
        .iter()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for expected in ["source-model.json", "topology.json", "facts.json"] {
        assert!(
            unique_refs.iter().any(|item| item.ends_with(expected)),
            "deploy repair generated refs must expose generated sidecar {expected}: {value:#}"
        );
    }
    assert_eq!(
        generated_refs.len(),
        unique_refs.len(),
        "deploy repair asset refs must not include duplicates: {value:#}"
    );
    assert_forbidden_cli_fields_absent(&value);
}

#[test]
fn deploy_execution_repair_next_is_request_scoped_and_retries_deploy_after_submit() {
    let fixture = Fixture::new("deploy-execution-repair");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_failure_report();
    fixture.write_repair_action(
        DeploymentRepairRoute::ExecutionRepair,
        DeploymentFailureOwner::ApplicationCode,
    );

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
    assert_eq!(value["next"]["kind"], "execute_task");
    assert_eq!(value["next"]["executionKind"], "deploy_execution_repair");
    assert_eq!(value["next"]["repairOrigin"], "deploy_failure");
    assert_eq!(value["next"]["submitTool"], "loom.repairSubmitFile");
    assert_eq!(value["next"]["postSubmit"], "retry_deploy");
    assert!(value["next"]["repairContext"]["deploymentFailureRef"]
        .as_str()
        .unwrap()
        .ends_with("latest-failure.json"));
    assert_forbidden_cli_fields_absent(&value);

    let request_ref = value["next"]["requestRef"].as_str().unwrap().to_string();
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str(),
        request_ref: request_ref.clone(),
    })
    .expect("inspect deploy repair request");
    let failure_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str(),
        request_ref: request_ref.clone(),
        group_id: "deploy_failure_context".to_string(),
    })
    .expect("read deploy failure context");
    assert_eq!(
        failure_group.fields["repairContext.failedAt"].value,
        json!("runtime_application_startup")
    );
    assert_eq!(
        failure_group.fields["repairContext.failedContract.field"].value,
        json!("runtime.startup")
    );
    assert_eq!(
        failure_group.fields["repairContext.failedContract.command"].value,
        json!("java -jar app.jar")
    );
    assert_eq!(
        failure_group.fields["repairContext.failedContract.workingDirectory"].value,
        json!("service")
    );
    assert_eq!(
        failure_group.fields["repairContext.deployCommand"].value,
        json!(["docker", "compose", "up"])
    );
    assert_eq!(
        failure_group.fields["repairContext.exitCode"].value,
        json!(1)
    );
    assert_eq!(
        failure_group.fields["repairContext.fullLogRef"].value,
        json!(".loom/deployment/logs/local.log")
    );
    assert!(!inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "repairContext"));
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str(),
        request_ref: request_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read deploy repair request fields")
    .fields;
    assert!(fields["outputContract.resultTemplate"].value["runtimeDeliveryEvidence"].is_object());
    assert!(inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "outputContract.resultTemplate"));
    assert!(!inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field.starts_with("outputContract.schemaShape")));
    let second_result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let second_value = serde_json::to_value(second_result).expect("second repair json");
    assert_eq!(
        second_value["next"]["requestRef"].as_str(),
        Some(request_ref.as_str())
    );
    let result_file = value["next"]["resultFile"].as_str().unwrap().to_string();
    write_json_atomic(
        &fixture.root.join(&result_file),
        &json!({
            "schemaVersion": "1.0",
            "repairId": "deploy-repair-1",
            "status": "completed",
            "deploymentFailureRef": ".loom/deployment/state/latest-failure.json",
            "changedFiles": ["apps/backend/src/main/java/example/App.java"],
            "runtimeDeliveryEvidence": {
                "source": "deploy_failure_repair",
                "addressedFailedContractFields": ["runtime.startup"],
                "codeLevelChecks": [{
                    "checkId": "check_runtime_startup",
                    "status": "passed",
                    "evidence": "Adjusted application startup wiring."
                }],
                "commandsRun": [],
                "unverifiedItems": []
            },
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_passed",
                "progressObserved": true
            },
            "notes": []
        }),
    )
    .expect("write deploy repair result");
    let submit_input = FileSubmitInput {
        project_root: fixture.root_str(),
        request_ref,
        written_target_ids: None,
    };
    let authorized = state::authorize_write_targets(&submit_input, "loom.repairSubmitFile")
        .expect("authorized deploy repair result");
    let submitted = accept_deploy_execution_repair_file(&submit_input, &authorized);
    let submitted_value = serde_json::to_value(submitted).expect("submitted result json");
    assert_ne!(submitted_value["state"], "failed", "{submitted_value:#}");
    assert_forbidden_cli_fields_absent(&submitted_value);
}

#[test]
fn deploy_execution_repair_invalid_result_returns_repairable_error() {
    let fixture = Fixture::new("deploy-execution-repair-invalid");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_failure_report();
    fixture.write_repair_action(
        DeploymentRepairRoute::ExecutionRepair,
        DeploymentFailureOwner::ApplicationCode,
    );

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");
    let request_ref = value["next"]["requestRef"].as_str().unwrap().to_string();
    let result_file = value["next"]["resultFile"].as_str().unwrap().to_string();
    write_json_atomic(
        &fixture.root.join(&result_file),
        &json!({ "schemaVersion": "1.0" }),
    )
    .expect("write invalid deploy repair result");

    let submit_input = FileSubmitInput {
        project_root: fixture.root_str(),
        request_ref,
        written_target_ids: None,
    };
    let authorized = state::authorize_write_targets(&submit_input, "loom.repairSubmitFile")
        .expect("authorized deploy repair result");
    let submitted = accept_deploy_execution_repair_file(&submit_input, &authorized);
    let submitted_value = serde_json::to_value(submitted).expect("submitted result json");

    assert_eq!(
        submitted_value["state"], "repairable_error",
        "{submitted_value:#}"
    );
    assert_eq!(submitted_value["targetFile"], result_file);
    assert_eq!(submitted_value["resubmitTool"], "loom.repairSubmitFile");
    assert_eq!(
        submitted_value["fixScope"],
        "deploy_execution_repair_result_only"
    );
}

#[test]
fn deploy_execution_repair_rejects_invalid_status() {
    let fixture = Fixture::new("deploy-execution-repair-invalid-status");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_failure_report();
    fixture.write_repair_action(
        DeploymentRepairRoute::ExecutionRepair,
        DeploymentFailureOwner::ApplicationCode,
    );

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");
    let request_ref = value["next"]["requestRef"].as_str().unwrap().to_string();
    let result_file = value["next"]["resultFile"].as_str().unwrap().to_string();
    write_json_atomic(
        &fixture.root.join(&result_file),
        &json!({
            "schemaVersion": "1.0",
            "repairId": "deploy-repair-1",
            "status": "done",
            "deploymentFailureRef": ".loom/deployment/state/latest-failure.json",
            "changedFiles": ["package.json"],
            "runtimeDeliveryEvidence": {
                "addressedFailedContractFields": ["runtime.startup"],
                "codeLevelChecks": [{
                    "checkId": "check_runtime_startup",
                    "status": "passed",
                    "evidence": "Adjusted application startup wiring."
                }],
                "commandsRun": [],
                "unverifiedItems": []
            },
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_passed",
                "progressObserved": true
            },
            "notes": []
        }),
    )
    .expect("write deploy repair result");

    let submit_input = FileSubmitInput {
        project_root: fixture.root_str(),
        request_ref,
        written_target_ids: None,
    };
    let authorized = state::authorize_write_targets(&submit_input, "loom.repairSubmitFile")
        .expect("authorized deploy repair result");
    let submitted = accept_deploy_execution_repair_file(&submit_input, &authorized);
    let submitted_value = serde_json::to_value(submitted).expect("submitted result json");

    assert_eq!(
        submitted_value["state"], "repairable_error",
        "{submitted_value:#}"
    );
    assert!(submitted_value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| {
            issue["code"] == "DEPLOY_REPAIR_STATUS_INVALID" && issue["fieldPath"] == "status"
        }));
}

#[test]
fn deploy_execution_repair_completed_result_requires_changed_files() {
    let fixture = Fixture::new("deploy-execution-repair-empty-changed-files");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_failure_report();
    fixture.write_repair_action(
        DeploymentRepairRoute::ExecutionRepair,
        DeploymentFailureOwner::ApplicationCode,
    );

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");
    let request_ref = value["next"]["requestRef"].as_str().unwrap().to_string();
    let result_file = value["next"]["resultFile"].as_str().unwrap().to_string();
    write_json_atomic(
        &fixture.root.join(&result_file),
        &json!({
            "schemaVersion": "1.0",
            "repairId": "deploy-repair-1",
            "status": "completed",
            "deploymentFailureRef": ".loom/deployment/state/latest-failure.json",
            "changedFiles": [],
            "runtimeDeliveryEvidence": {
                "addressedFailedContractFields": ["runtime.startup"],
                "codeLevelChecks": [{
                    "checkId": "check_runtime_startup",
                    "status": "passed",
                    "evidence": "Adjusted application startup wiring."
                }],
                "commandsRun": [],
                "unverifiedItems": []
            },
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_passed",
                "progressObserved": true
            },
            "notes": []
        }),
    )
    .expect("write deploy repair result");

    let submit_input = FileSubmitInput {
        project_root: fixture.root_str(),
        request_ref,
        written_target_ids: None,
    };
    let authorized = state::authorize_write_targets(&submit_input, "loom.repairSubmitFile")
        .expect("authorized deploy repair result");
    let submitted = accept_deploy_execution_repair_file(&submit_input, &authorized);
    let submitted_value = serde_json::to_value(submitted).expect("submitted result json");

    assert_eq!(
        submitted_value["state"], "repairable_error",
        "{submitted_value:#}"
    );
    assert!(submitted_value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| {
            issue["code"] == "DEPLOY_REPAIR_CHANGED_FILES_REQUIRED"
                && issue["fieldPath"] == "changedFiles"
        }));
}

#[test]
fn deploy_execution_repair_completed_result_rejects_failed_code_level_checks() {
    let fixture = Fixture::new("deploy-execution-repair-failed-check");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_failure_report();
    fixture.write_repair_action(
        DeploymentRepairRoute::ExecutionRepair,
        DeploymentFailureOwner::ApplicationCode,
    );

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");
    let request_ref = value["next"]["requestRef"].as_str().unwrap().to_string();
    let result_file = value["next"]["resultFile"].as_str().unwrap().to_string();
    write_json_atomic(
        &fixture.root.join(&result_file),
        &json!({
            "schemaVersion": "1.0",
            "repairId": "deploy-repair-1",
            "status": "completed",
            "deploymentFailureRef": ".loom/deployment/state/latest-failure.json",
            "changedFiles": ["package.json"],
            "runtimeDeliveryEvidence": {
                "addressedFailedContractFields": ["runtime.startup"],
                "codeLevelChecks": [{
                    "checkId": "check_runtime_startup",
                    "status": "failed",
                    "evidence": "Startup still fails."
                }],
                "commandsRun": [],
                "unverifiedItems": []
            },
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_passed",
                "progressObserved": true
            },
            "notes": []
        }),
    )
    .expect("write deploy repair result");

    let submit_input = FileSubmitInput {
        project_root: fixture.root_str(),
        request_ref,
        written_target_ids: None,
    };
    let authorized = state::authorize_write_targets(&submit_input, "loom.repairSubmitFile")
        .expect("authorized deploy repair result");
    let submitted = accept_deploy_execution_repair_file(&submit_input, &authorized);
    let submitted_value = serde_json::to_value(submitted).expect("submitted result json");

    assert_eq!(
        submitted_value["state"], "repairable_error",
        "{submitted_value:#}"
    );
    assert!(submitted_value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| {
            issue["code"] == "DEPLOY_REPAIR_RUNTIME_EVIDENCE_INVALID"
                && issue["fieldPath"] == "runtimeDeliveryEvidence"
        }));
}

#[cfg(unix)]
#[test]
fn deploy_up_routes_runtime_build_failure_to_execution_repair_and_counts_retry_attempts() {
    let fixture = Fixture::new("deploy-build-failure-route");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","start":"node dist/server.js"},"dependencies":{"vite":"latest"}}"#,
    );
    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "up" ]; then
  echo "#13 RUN npm run build"
  echo "#13 1.262 src/App.tsx(1,1): error TS7006: Parameter 'value' implicitly has an 'any' type." >&2
  echo "failed to solve: process \"/bin/sh -c npm run build\" did not complete successfully: exit code: 2" >&2
  exit 1
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
    assert_eq!(value["next"]["executionKind"], "deploy_execution_repair");
    assert_eq!(
        value["next"]["repairContext"]["issues"],
        json!(["build_command_failed"]),
        "{value:#}"
    );
    assert_eq!(
        fixture.repair_action_value()["failureKind"],
        "build_command_failed"
    );
    assert_eq!(fixture.repair_action_value()["attempts"], 0);
    let failure_report: Value = read_json(
        &fixture
            .root
            .join(".loom/deployment/state/latest-failure.json"),
    )
    .expect("latest failure report");
    assert_eq!(failure_report["failedAt"], json!("runtime_build_command"));
    assert_eq!(
        failure_report["failedContract"]["field"],
        json!("build.command")
    );
    assert!(
        failure_report["failedContract"]["command"]
            .as_str()
            .map(|command| command.contains("npm run build"))
            .unwrap_or(false),
        "{failure_report:#}"
    );
    assert_eq!(
        failure_report["deployCommand"],
        json!(["docker", "compose", "up"])
    );
    assert_eq!(failure_report["exitCode"], json!(1));
    assert!(failure_report["fullLogRef"]
        .as_str()
        .unwrap()
        .ends_with("local.log"));
    assert_forbidden_cli_fields_absent(&value);

    let request_ref = value["next"]["requestRef"].as_str().unwrap().to_string();
    let result_file = value["next"]["resultFile"].as_str().unwrap().to_string();
    write_json_atomic(
        &fixture.root.join(&result_file),
        &json!({
            "schemaVersion": "1.0",
            "repairId": fixture.repair_action_value()["repairId"],
            "status": "completed",
            "deploymentFailureRef": ".loom/deployment/state/latest-failure.json",
            "changedFiles": ["package.json"],
            "runtimeDeliveryEvidence": {
                "addressedFailedContractFields": ["build.command"],
                "codeLevelChecks": [{
                    "checkId": "check_build_command",
                    "status": "passed",
                    "evidence": "Adjusted the runtime build script."
                }],
                "commandsRun": [],
                "unverifiedItems": []
            },
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_passed",
                "progressObserved": true
            },
            "notes": []
        }),
    )
    .expect("write deploy repair result");
    let submit_input = FileSubmitInput {
        project_root: fixture.root_str(),
        request_ref,
        written_target_ids: None,
    };
    let authorized = state::authorize_write_targets(&submit_input, "loom.repairSubmitFile")
        .expect("authorized deploy repair result");
    let submitted = accept_deploy_execution_repair_file(&submit_input, &authorized);
    let submitted_value = serde_json::to_value(submitted).expect("submitted result json");

    assert_eq!(
        submitted_value["state"], "auto_runnable",
        "{submitted_value:#}"
    );
    assert_eq!(fixture.repair_action_value()["attempts"], 1);
    assert_eq!(
        submitted_value["next"]["repairContext"]["issues"],
        json!(["build_command_failed"]),
        "{submitted_value:#}"
    );
    assert_forbidden_cli_fields_absent(&submitted_value);
}

#[cfg(unix)]
#[test]
fn deploy_up_classifies_registry_network_without_source_repair() {
    let fixture = Fixture::new("deploy-registry-network");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","start":"node dist/server.js"},"dependencies":{"vite":"latest"}}"#,
    );
    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "up" ]; then
  echo "failed to fetch oauth token: Get https://auth.docker.io/token: net/http: TLS handshake timeout" >&2
  exit 1
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(value["state"], "blocked", "{value:#}");
    assert_eq!(value["recommendedTool"], "loom.deployStatus");
    assert_eq!(
        value["details"]["repairSummary"]["failureKind"], "registry_network",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["repairSummary"]["failureOwner"], "external_system",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["repairSummary"]["repairRoute"], "none",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["repairSummary"]["nextAction"], "fix_external_system_then_retry",
        "{value:#}"
    );
    assert!(value["details"]["repairSummary"]["primaryReason"]
        .as_str()
        .unwrap()
        .contains("registry_network"));
    assert!(
        value["details"].get("projectRoot").is_none()
            && value["details"].get("suggestedActions").is_none()
            && value["details"].get("errorWindow").is_none(),
        "blocked output must expose compact repairSummary instead of full repair action: {value:#}"
    );
    let repair_action = fixture.repair_action_value();
    assert_eq!(repair_action["failureKind"], "registry_network");
    assert_eq!(repair_action["failureOwner"], "external_system");
    assert_eq!(repair_action["repairRoute"], "none");
    assert!(repair_action["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "registry_network"));
    assert!(repair_action["suggestedActions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .any(|action| action.contains("registry_network")));
    assert_forbidden_cli_fields_absent(&value);
}

#[cfg(unix)]
#[test]
fn deploy_up_classifies_docker_socket_failure_as_environment_blocker() {
    let fixture = Fixture::new("deploy-docker-socket-environment");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run start", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","start":"node dist/server.js"},"dependencies":{"vite":"latest"}}"#,
    );
    let prepare = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let prepare_value = serde_json::to_value(prepare).expect("prepare json");
    assert_eq!(prepare_value["state"], "done", "{prepare_value:#}");
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then echo "Docker version 25.0.0"; exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "config" ]; then exit 0; fi
if [ "$1" = "compose" ] && [ "$4" = "up" ]; then
  echo "unable to get image 'demo:latest': failed to connect to the docker API at unix:///Users/test/.docker/run/docker.sock; check if the path is correct and if the daemon is running: dial unix /Users/test/.docker/run/docker.sock: connect: no such file or directory" >&2
  exit 1
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_up(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("deploy up json");
    assert_eq!(value["state"], "blocked", "{value:#}");
    assert_eq!(
        value["details"]["repairSummary"]["failureKind"], "docker_unavailable",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["repairSummary"]["failureOwner"], "environment",
        "{value:#}"
    );
    assert_eq!(
        value["details"]["repairSummary"]["repairRoute"], "none",
        "{value:#}"
    );
    let repair_action = fixture.repair_action_value();
    assert!(repair_action
        .get("editableFiles")
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        .unwrap_or(true));
    assert!(repair_action["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "docker_unavailable"));
}

#[test]
fn deploy_prepare_detects_bootstrap_tasks_without_executing_them() {
    let fixture = Fixture::new("deploy-bootstrap-detect");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run preview", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","preview":"vite preview"}}"#,
    );
    fixture.write_text(
        "prisma/schema.prisma",
        "datasource db { provider = \"sqlite\" }\n",
    );

    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");

    assert_eq!(value["state"], "done", "{value:#}");
    let spec: DeploymentSpec =
        read_json(&fixture.root.join(".loom/deployment/specs/local.json")).expect("spec");
    assert_eq!(spec.bootstrap.tasks.len(), 1);
    assert_eq!(spec.bootstrap.tasks[0].kind, "prisma");
    assert_eq!(spec.bootstrap.tasks[0].command, "npx prisma migrate deploy");
    assert!(!spec.bootstrap.tasks[0].automatic);
    assert!(
        !fixture.root.join("bootstrap-ran.txt").exists(),
        "prepare must only detect bootstrap tasks"
    );
}

#[test]
fn deploy_bootstrap_returns_confirmation_gate_with_declared_tasks() {
    let fixture = Fixture::new("deploy-bootstrap-gate");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run preview", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","preview":"vite preview"}}"#,
    );
    fixture.write_text(
        "prisma/schema.prisma",
        "datasource db { provider = \"sqlite\" }\n",
    );
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });

    let result = deploy_bootstrap(DeployBootstrapInput {
        project_root: fixture.root_str(),
        confirm: false,
        kind: Some("prisma".to_string()),
    });
    let value = serde_json::to_value(result).expect("bootstrap json");

    assert_eq!(value["state"], "user_gate", "{value:#}");
    assert_eq!(value["gate"]["confirmRequired"], true);
    assert_eq!(value["gate"]["tasks"][0]["kind"], "prisma");
    assert_eq!(
        value["gate"]["tasks"][0]["command"],
        "npx prisma migrate deploy"
    );
    assert_forbidden_cli_fields_absent(&value);
}

#[cfg(unix)]
#[test]
fn deploy_bootstrap_executes_confirmed_task_inside_compose_service() {
    let fixture = Fixture::new("deploy-bootstrap-compose-exec");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run preview", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","preview":"vite preview"}}"#,
    );
    fixture.write_text(
        "prisma/schema.prisma",
        "datasource db { provider = \"sqlite\" }\n",
    );
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_mock_docker(
        r##"#!/bin/sh
printf '%s\n' "$@" >> docker-args.txt
if [ "$1" = "compose" ] && [ "$4" = "ps" ]; then
  echo "app"
  exit 0
fi
if [ "$1" = "compose" ] && [ "$4" = "exec" ]; then
  if [ "$5" = "-T" ] && [ "$6" = "app" ] && [ "$7" = "sh" ] && [ "$8" = "-lc" ]; then
    echo "$9" > bootstrap-command.txt
    echo "migration ok"
    exit 0
  fi
  echo "unexpected exec args: $*" >&2
  exit 12
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_bootstrap(DeployBootstrapInput {
        project_root: fixture.root_str(),
        confirm: true,
        kind: Some("prisma".to_string()),
    });
    let value = serde_json::to_value(result).expect("bootstrap json");

    assert_eq!(value["state"], "done", "{value:#}");
    assert_eq!(value["details"]["executed"][0]["kind"], "prisma");
    assert_eq!(
        value["details"]["executed"][0]["command"],
        "npx prisma migrate deploy"
    );
    assert_eq!(value["details"]["executed"][0]["serviceId"], "app");
    assert_eq!(value["details"]["executed"][0]["status"], "passed");
    assert_eq!(
        read_text(&fixture.root.join("bootstrap-command.txt")).expect("bootstrap command"),
        "npx prisma migrate deploy\n"
    );
    let docker_args = read_text(&fixture.root.join("docker-args.txt")).expect("docker args");
    assert!(
        docker_args.contains("exec\n-T\napp\nsh\n-lc\nnpx prisma migrate deploy"),
        "{docker_args}"
    );
}

#[cfg(unix)]
#[test]
fn deploy_bootstrap_blocks_when_compose_service_is_not_running() {
    let fixture = Fixture::new("deploy-bootstrap-service-not-running");
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "node",
        "deploymentShape": "single-service",
        "build": { "command": "npm run build" },
        "start": { "command": "npm run preview", "port": 8080 },
        "httpProbes": { "previewPath": "/" }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","preview":"vite preview"}}"#,
    );
    fixture.write_text(
        "prisma/schema.prisma",
        "datasource db { provider = \"sqlite\" }\n",
    );
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_mock_docker(
        r##"#!/bin/sh
if [ "$1" = "compose" ] && [ "$4" = "ps" ]; then
  exit 0
fi
if [ "$1" = "compose" ] && [ "$4" = "exec" ]; then
  echo "unexpected exec" > bootstrap-command.txt
  exit 13
fi
exit 0
"##,
    );
    let _path_guard = fixture.prepend_mock_bin_to_path();

    let result = deploy_bootstrap(DeployBootstrapInput {
        project_root: fixture.root_str(),
        confirm: true,
        kind: Some("prisma".to_string()),
    });
    let value = serde_json::to_value(result).expect("bootstrap json");

    assert_eq!(value["state"], "blocked", "{value:#}");
    assert_eq!(value["recommendedTool"], "loom.deployUp");
    assert_eq!(value["details"]["serviceId"], "app");
    assert_eq!(value["details"]["status"], "not_running");
    assert!(
        !fixture.root.join("bootstrap-command.txt").exists(),
        "bootstrap must not exec when the service is not running"
    );
}

#[test]
fn deploy_repair_blocks_when_attempt_limit_is_reached() {
    let fixture = Fixture::new("deploy-repair-attempt-limit");
    fixture.write_runtime_delivery(runtime_delivery());
    let _ = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    fixture.write_failure_report();
    fixture.write_repair_action(
        DeploymentRepairRoute::ExecutionRepair,
        DeploymentFailureOwner::ApplicationCode,
    );
    let mut action = fixture.repair_action_value();
    action["attempts"] = json!(2);
    action["maxAttempts"] = json!(2);
    write_json_atomic(
        &fixture
            .root
            .join(".loom/deployment/state/repair-action.json"),
        &action,
    )
    .expect("write limited repair action");

    let result = deploy_repair(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("repair result json");

    assert_eq!(value["state"], "blocked", "{value:#}");
    assert_eq!(value["recommendedTool"], "loom.deployInspect");
    assert_eq!(value["details"]["repairSummary"]["attempts"], 2);
    assert_eq!(value["details"]["repairSummary"]["maxAttempts"], 2);
    assert_eq!(
        value["details"]["repairSummary"]["nextAction"],
        "inspect_attempt_limit"
    );
    assert!(
        value["details"].get("projectRoot").is_none()
            && value["details"].get("suggestedActions").is_none()
            && value["details"].get("errorWindow").is_none(),
        "attempt-limit blocker must expose compact repairSummary: {value:#}"
    );
    assert_forbidden_cli_fields_absent(&value);
}

fn runtime_delivery() -> Value {
    json!({
        "status": "modified",
        "runtimeKind": "vite react frontend plus spring boot postgres backend",
        "deploymentShape": "frontend-and-backend",
        "build": { "command": "npm --prefix apps/frontend run build && mvn -f apps/backend/pom.xml package" },
        "start": { "command": "java -jar target/app.jar", "port": 8080 },
        "httpProbes": {
            "previewPath": "/",
            "healthPath": "/actuator/health",
            "apiPaths": ["/api/accounts"]
        },
        "frontend": {
            "required": true,
            "kind": "vite-react",
            "sourceRoot": "apps/frontend",
            "outputDir": "apps/frontend/dist",
            "buildCommand": "npm --prefix apps/frontend run build"
        },
        "api": {
            "required": true,
            "kind": "spring-boot",
            "entry": "apps/backend/pom.xml",
            "buildCommand": "mvn -f apps/backend/pom.xml package",
            "basePath": "/api",
            "probePaths": ["/api/accounts"]
        },
        "environment": {
            "required": ["SPRING_DATASOURCE_URL"],
            "optional": ["postgres"]
        }
    })
}

fn plugin_style_dual_service_runtime_delivery() -> Value {
    json!({
        "status": "modified",
        "runtimeKind": "react_vite_plus_spring_boot",
        "deploymentShape": "dual-service",
        "build": {
            "command": "frontend: npm run build; backend: ./mvnw package",
            "outputs": ["frontend/dist", "backend/target/*.jar"],
            "workingDirectory": "."
        },
        "start": {
            "command": "frontend: npm run dev; backend: ./mvnw spring-boot:run",
            "workingDirectory": "."
        },
        "httpProbes": {
            "previewPath": "/",
            "apiPaths": ["/api/health", "/api/purchase-requests"],
            "expectedStatus": "2xx_or_3xx"
        },
        "runtimeSurfaces": [
            { "surfaceId": "runtime_surface_1", "kind": "frontend", "urlPath": "/" },
            { "surfaceId": "runtime_surface_2", "kind": "api", "urlPath": "/api" }
        ],
        "environment": {
            "required": ["DATABASE_URL"],
            "optional": ["SPRING_PROFILES_ACTIVE"]
        }
    })
}

fn write_root_vite_fastapi_delivery(fixture: &Fixture) {
    fixture.write_runtime_delivery(json!({
        "status": "modified",
        "runtimeKind": "web_frontend_plus_api_service",
        "deploymentShape": "frontend-and-backend",
        "build": {
            "command": "frontend: npm run build; backend: uvicorn app.main:app --host 0.0.0.0 --port 8000"
        },
        "start": {
            "command": "frontend: npm run dev; backend: uvicorn app.main:app --reload --port 8000",
            "port": 8000
        },
        "httpProbes": {
            "previewPath": "/",
            "apiPaths": ["/api/tickets"]
        }
    }));
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","dev":"vite --host 0.0.0.0"},"dependencies":{"vite":"latest","vue":"latest"}}"#,
    );
    fixture.write_text("vite.config.js", "export default {}\n");
    fixture.write_text("src/main.ts", "console.log('fixture')\n");
    fixture.write_text(
        "pyproject.toml",
        r#"[project]
name = "loom-fastapi-fixture"
version = "0.1.0"
dependencies = ["fastapi", "uvicorn"]
"#,
    );
    fixture.write_text(
        "app/main.py",
        "from fastapi import FastAPI\napp = FastAPI()\n",
    );
}

#[derive(Debug)]
struct BackendSourceModelCase {
    name: &'static str,
    api_kind: &'static str,
    api_entry: &'static str,
    build_command: &'static str,
    start_command: &'static str,
    expected_root: &'static str,
    expected_kind: RuntimeKind,
    expected_manifest: &'static str,
    expected_build_command: &'static str,
    expected_start_command: &'static str,
    files: Vec<(&'static str, &'static str)>,
}

fn backend_source_model_matrix_cases() -> Vec<BackendSourceModelCase> {
    vec![
        BackendSourceModelCase {
            name: "java",
            api_kind: "spring-boot",
            api_entry: "server/pom.xml",
            build_command: "mvn -f server/pom.xml package",
            start_command: "java -jar target/app.jar",
            expected_root: "server",
            expected_kind: RuntimeKind::Java,
            expected_manifest: "pom.xml",
            expected_build_command: "mvn -f pom.xml package",
            expected_start_command: "java -jar target/app.jar",
            files: vec![
                (
                    "server/pom.xml",
                    r#"<project><modelVersion>4.0.0</modelVersion><artifactId>server</artifactId><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies></project>"#,
                ),
                (
                    "server/src/main/java/com/acme/App.java",
                    "package com.acme; public class App {}\n",
                ),
            ],
        },
        BackendSourceModelCase {
            name: "python",
            api_kind: "fastapi",
            api_entry: "server/pyproject.toml",
            build_command: "cd server && python -m compileall app",
            start_command: "cd server && python -m uvicorn app.main:app --host 0.0.0.0 --port 8000",
            expected_root: "server",
            expected_kind: RuntimeKind::Python,
            expected_manifest: "pyproject.toml",
            expected_build_command: "python -m compileall app",
            expected_start_command: "python -m uvicorn app.main:app --host 0.0.0.0 --port 8000",
            files: vec![
                (
                    "server/pyproject.toml",
                    r#"[project]
name = "server"
version = "0.1.0"
dependencies = ["fastapi", "uvicorn"]
"#,
                ),
                (
                    "server/app/main.py",
                    "from fastapi import FastAPI\napp = FastAPI()\n",
                ),
            ],
        },
        BackendSourceModelCase {
            name: "go",
            api_kind: "go",
            api_entry: "server/go.mod",
            build_command: "cd server && go build -o /out/server .",
            start_command: "/app/server",
            expected_root: "server",
            expected_kind: RuntimeKind::Go,
            expected_manifest: "go.mod",
            expected_build_command: "go build -o /out/server .",
            expected_start_command: "/app/server",
            files: vec![
                ("server/go.mod", "module example.com/server\n\ngo 1.23\n"),
                ("server/main.go", "package main\nfunc main() {}\n"),
            ],
        },
        BackendSourceModelCase {
            name: "dotnet",
            api_kind: "aspnet",
            api_entry: "server/Server.csproj",
            build_command: "dotnet publish server/Server.csproj -c Release -o /app/publish",
            start_command: "dotnet Server.dll",
            expected_root: "server",
            expected_kind: RuntimeKind::Dotnet,
            expected_manifest: "*.csproj",
            expected_build_command: "dotnet publish Server.csproj -c Release -o /app/publish",
            expected_start_command: "dotnet Server.dll",
            files: vec![(
                "server/Server.csproj",
                r#"<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>"#,
            )],
        },
        BackendSourceModelCase {
            name: "node",
            api_kind: "express",
            api_entry: "server/package.json",
            build_command: "npm --prefix server run build",
            start_command: "npm --prefix server run start",
            expected_root: "server",
            expected_kind: RuntimeKind::Node,
            expected_manifest: "package.json",
            expected_build_command: "npm run build",
            expected_start_command: "npm run start",
            files: vec![
                (
                    "server/package.json",
                    r#"{"scripts":{"build":"node build.js","start":"node index.js"},"dependencies":{"express":"latest"}}"#,
                ),
                (
                    "server/index.js",
                    "require('express')().listen(process.env.PORT || 8000)\n",
                ),
                ("server/build.js", "console.log('build')\n"),
            ],
        },
        BackendSourceModelCase {
            name: "php",
            api_kind: "laravel",
            api_entry: "server/composer.json",
            build_command: "cd server && composer install --no-dev",
            start_command: "cd server && php -S 0.0.0.0:8000 -t public",
            expected_root: "server",
            expected_kind: RuntimeKind::Php,
            expected_manifest: "composer.json",
            expected_build_command: "composer install --no-dev",
            expected_start_command: "php -S 0.0.0.0:8000 -t public",
            files: vec![
                (
                    "server/composer.json",
                    r#"{"require":{"php":"^8.3","laravel/framework":"^11.0"}}"#,
                ),
                ("server/public/index.php", "<?php echo 'ok';\n"),
            ],
        },
        BackendSourceModelCase {
            name: "ruby",
            api_kind: "rails",
            api_entry: "server/Gemfile",
            build_command: "cd server && bundle install",
            start_command: "cd server && bundle exec rails server -b 0.0.0.0 -p 8000",
            expected_root: "server",
            expected_kind: RuntimeKind::Ruby,
            expected_manifest: "Gemfile",
            expected_build_command: "bundle install",
            expected_start_command: "bundle exec rails server -b 0.0.0.0 -p 8000",
            files: vec![
                (
                    "server/Gemfile",
                    "source 'https://rubygems.org'\ngem 'rails'\n",
                ),
                ("server/config.ru", "run ->(_env) { [200, {}, ['ok']] }\n"),
            ],
        },
    ]
}

fn write_root_frontend_fixture(fixture: &Fixture) {
    fixture.write_text(
        "package.json",
        r#"{"scripts":{"build":"vite build","dev":"vite --host 0.0.0.0"},"dependencies":{"vite":"latest","react":"latest"}}"#,
    );
    fixture.write_text("package-lock.json", "{}\n");
    fixture.write_text("vite.config.js", "export default {}\n");
    fixture.write_text("src/main.tsx", "console.log('frontend')\n");
}

fn assert_forbidden_cli_fields_absent(value: &Value) {
    match value {
        Value::Object(object) => {
            for forbidden in [
                "run_cli",
                "next-task",
                "submitCommand",
                "retryCommand",
                "commandInvocation",
                "argv",
                "readCommand",
                "fallbackRule",
            ] {
                assert!(
                    !object.contains_key(forbidden),
                    "forbidden key {forbidden} appears in {value:#}"
                );
            }
            for child in object.values() {
                assert_forbidden_cli_fields_absent(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_forbidden_cli_fields_absent(item);
            }
        }
        _ => {}
    }
}

fn assert_deploy_reference_profile_shape(profile: &Value) {
    assert_eq!(
        profile["loadMode"], "mcp_reference_load_plan",
        "{profile:#}"
    );
    assert!(
        profile.get("referenceIds").is_none(),
        "deployReferenceProfile must not expose legacy referenceIds: {profile:#}"
    );
    let plan = profile["referenceLoadPlan"]
        .as_array()
        .expect("deployReferenceProfile.referenceLoadPlan array");
    assert!(
        !plan.is_empty(),
        "deployReferenceProfile.referenceLoadPlan must not be empty"
    );
    for item in plan {
        assert!(item["refId"].as_str().is_some(), "missing refId: {item:#}");
        assert!(item["path"].as_str().is_some(), "missing path: {item:#}");
        assert!(
            item["reason"].as_str().is_some(),
            "missing reason: {item:#}"
        );
    }
}

fn assert_reference_load_plan_contains(profile: &Value, expected_ref_id: &str) {
    let expected_path = expected_deploy_reference_path(expected_ref_id);
    let contains = profile["referenceLoadPlan"]
        .as_array()
        .map(|items| {
            items.iter().any(|item| {
                item["refId"] == expected_ref_id
                    && item["path"] == expected_path
                    && item["reason"]
                        .as_str()
                        .is_some_and(|reason| !reason.is_empty())
            })
        })
        .unwrap_or(false);
    assert!(
        contains,
        "expected {expected_ref_id} -> {expected_path} in {profile:#}"
    );
}

fn assert_reference_load_plan_excludes(profile: &Value, forbidden_ref_id: &str) {
    let contains = profile["referenceLoadPlan"]
        .as_array()
        .map(|items| items.iter().any(|item| item["refId"] == forbidden_ref_id))
        .unwrap_or(false);
    assert!(
        !contains,
        "forbidden {forbidden_ref_id} appears in {profile:#}"
    );
}

fn expected_deploy_reference_path(ref_id: &str) -> &'static str {
    match ref_id {
        "deploy.providers" => "providers.md",
        "deploy.matrix" => "matrix.md",
        "deploy.source-model" => "source-model.md",
        "deploy.topology" => "topology.md",
        "deploy.compose" => "compose.md",
        "deploy.dockerfile" => "dockerfile.md",
        "deploy.environment" => "environment.md",
        "deploy.workspaces" => "workspaces.md",
        "deploy.bootstrap" => "bootstrap.md",
        "deploy.repair" => "repair.md",
        "deploy.stacks.node" => "node.md",
        "deploy.stacks.python" => "python.md",
        "deploy.stacks.go" => "go.md",
        "deploy.stacks.java" => "java.md",
        "deploy.stacks.dotnet" => "dotnet.md",
        "deploy.stacks.php" => "php.md",
        "deploy.stacks.ruby" => "ruby.md",
        "deploy.stacks.static" => "static.md",
        other => panic!("no expected deploy reference path for {other}"),
    }
}

fn assert_probe_stack(
    name: &str,
    files: &[(&str, &str)],
    expected_kind: RuntimeKind,
    expected_reference_id: &str,
    expected_topology_class: &str,
) {
    let fixture = Fixture::new(name);
    for (path, text) in files {
        fixture.write_text(path, text);
    }
    let result = deploy_prepare(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let value = serde_json::to_value(result).expect("prepare json");
    assert_eq!(value["state"], "done", "{value:#}");
    let reference_profile = &value["details"]["deployReferenceProfile"];
    assert_deploy_reference_profile_shape(reference_profile);
    assert_reference_load_plan_contains(reference_profile, "deploy.matrix");
    assert_reference_load_plan_contains(reference_profile, "deploy.source-model");
    assert_reference_load_plan_contains(reference_profile, expected_reference_id);

    let spec = fixture.read_spec();
    assert!(
        spec.facts.stack_kinds.contains(&expected_kind),
        "{:#?}",
        spec.facts
    );
    assert_eq!(
        serde_json::to_value(spec.facts.topology_class).unwrap(),
        json!(expected_topology_class)
    );
    let service = &spec.source_model.services[0];
    assert_eq!(service.runtime_kind, expected_kind);
    if service.root != "." && matches!(expected_kind, RuntimeKind::Go | RuntimeKind::Dotnet) {
        let dockerfile = read_text(
            &fixture
                .root
                .join(".loom/deployment/specs/generated/Dockerfile.app"),
        )
        .expect("generated dockerfile");
        assert!(
            dockerfile.contains(&format!("WORKDIR /src/{}", service.root)),
            "{dockerfile}"
        );
    }
    let validate = deploy_validate(DeployToolInput {
        project_root: fixture.root_str(),
        app_path: None,
        healthcheck: None,
        provider_policy: None,
    });
    let validate_value = serde_json::to_value(validate).expect("validate json");
    assert_eq!(
        validate_value["details"]["assetIssues"],
        json!([]),
        "{validate_value:#}"
    );
}

struct Fixture {
    root: PathBuf,
    _env_guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let env_guard = test_env_lock().lock().expect("lock deploy test env");
        let root = std::env::temp_dir().join(format!("loom-rust-{name}-{}", now_millis()));
        std::fs::create_dir_all(&root).expect("create fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        state::lifecycle_store::init_project_state(root.to_str().unwrap()).expect("init project");
        Self {
            root,
            _env_guard: env_guard,
        }
    }

    fn root_str(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    fn write_text(&self, relative: &str, text: &str) {
        let file = self.root.join(relative);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(file, text).expect("write text fixture");
    }

    #[cfg(unix)]
    fn write_mock_docker(&self, script: &str) {
        let docker = self.root.join("mock-bin/docker");
        std::fs::create_dir_all(docker.parent().unwrap()).expect("create mock bin");
        std::fs::write(&docker, script).expect("write mock docker");
        let mut permissions = std::fs::metadata(&docker)
            .expect("mock docker metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&docker, permissions).expect("chmod mock docker");
    }

    #[cfg(unix)]
    fn prepend_mock_bin_to_path(&self) -> PathEnvGuard {
        let previous = std::env::var("PATH").unwrap_or_default();
        let mock_bin = self.root.join("mock-bin");
        std::env::set_var("PATH", format!("{}:{previous}", mock_bin.display()));
        PathEnvGuard { previous }
    }

    fn repair_action_value(&self) -> Value {
        read_json(&self.root.join(".loom/deployment/state/repair-action.json"))
            .expect("repair action")
    }

    fn write_runtime_delivery(&self, runtime_delivery: Value) {
        let delivery_id = "delivery-1";
        let phase_id = "phase-1";
        let aac_ref = ".loom/deliveries/delivery-1/contracts/architecture/phase-1/aac.json";
        let aac_file = self.root.join(aac_ref);
        ensure_dir(aac_file.parent().unwrap()).expect("aac dir");
        let api_paths = runtime_delivery
            .pointer("/httpProbes/apiPaths")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let api_interfaces = api_paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| {
                path.as_str().map(|path| {
                    json!({
                        "interfaceId": format!("api.test.{index}"),
                        "type": "http_api",
                        "method": "GET",
                        "path": path
                    })
                })
            })
            .collect::<Vec<_>>();
        let mut aac = json!({
            "schemaVersion": "1.0",
            "architectureArtifactContractId": "aac-1",
            "runtimeDelivery": runtime_delivery
        });
        if !api_interfaces.is_empty() {
            let api_contract_ref = ".loom/deliveries/delivery-1/contracts/api/current.json";
            aac["interfaces"] = json!(api_interfaces.clone());
            aac["apiContractRef"] = json!(api_contract_ref);
            aac["currentPhaseInterfaceRefs"] = json!(api_interfaces
                .iter()
                .filter_map(|interface| interface["interfaceId"].as_str())
                .collect::<Vec<_>>());
            let runtime = aac["runtimeDelivery"].clone();
            let base_path = runtime
                .pointer("/api/basePath")
                .cloned()
                .unwrap_or_else(|| json!("/api"));
            ensure_dir(self.root.join(api_contract_ref).parent().unwrap()).expect("api dir");
            write_json_atomic(
                &self.root.join(api_contract_ref),
                &json!({
                    "schemaVersion": "1.0",
                    "apiContractId": "api-1",
                    "deliveryId": delivery_id,
                    "currentPhaseId": phase_id,
                    "interfaces": api_interfaces,
                    "publicExposure": { "basePath": base_path, "preservePath": true },
                    "browserBinding": { "mode": "same_origin", "baseUrl": "" }
                }),
            )
            .expect("write api contract");
        }
        write_json_atomic(&aac_file, &aac).expect("write aac");
        write_json_atomic(
            &self.root.join(".loom/deliveries/delivery-1/index.json"),
            &DeliveryIndex {
                schema_version: 1,
                delivery_id: delivery_id.to_string(),
                active_phase_id: phase_id.to_string(),
                status: DeliveryLifecycleStatus::Executing,
                phases: vec![DeliveryPhaseState {
                    phase_id: phase_id.to_string(),
                    latest_refs: [("architectureArtifact".to_string(), aac_ref.to_string())]
                        .into_iter()
                        .collect(),
                    next_action: None,
                }],
                updated_at: now_string(),
            },
        )
        .expect("write delivery index");
        write_json_atomic(
            &self.root.join(".loom/status.json"),
            &ProjectStatus {
                schema_version: 1,
                active_delivery_id: Some(delivery_id.to_string()),
                last_completed_delivery_id: None,
                deliveries: vec![DeliveryStatusEntry {
                    delivery_id: delivery_id.to_string(),
                    active_phase_id: Some(phase_id.to_string()),
                    status: DeliveryLifecycleStatus::Executing,
                    updated_at: now_string(),
                }],
                updated_at: now_string(),
            },
        )
        .expect("write status");
    }

    fn write_runtime_delivery_with_interfaces(&self, runtime_delivery: Value, interfaces: Value) {
        self.write_runtime_delivery(runtime_delivery);
        let aac_path = self
            .root
            .join(".loom/deliveries/delivery-1/contracts/architecture/phase-1/aac.json");
        let mut aac: Value = read_json(&aac_path).expect("aac");
        aac["interfaces"] = interfaces.clone();
        let http_interfaces = interfaces
            .as_array()
            .into_iter()
            .flatten()
            .filter(|interface| interface["type"] == "http_api")
            .cloned()
            .collect::<Vec<_>>();
        if !http_interfaces.is_empty() {
            let api_contract_ref = ".loom/deliveries/delivery-1/contracts/api/current.json";
            let runtime = aac.get("runtimeDelivery").cloned().unwrap_or_default();
            let base_path = runtime
                .pointer("/api/basePath")
                .cloned()
                .unwrap_or_else(|| json!("/api"));
            let preserve_path = runtime
                .pointer("/api/preservePath")
                .cloned()
                .unwrap_or_else(|| json!(true));
            let api_contract = json!({
                "schemaVersion": "1.0",
                "apiContractId": "api-1",
                "deliveryId": "delivery-1",
                "currentPhaseId": "phase-1",
                "interfaces": http_interfaces,
                "publicExposure": { "basePath": base_path, "preservePath": preserve_path },
                "browserBinding": { "mode": "same_origin", "baseUrl": "" }
            });
            ensure_dir(self.root.join(api_contract_ref).parent().unwrap()).expect("api dir");
            write_json_atomic(&self.root.join(api_contract_ref), &api_contract)
                .expect("write api contract");
            aac["apiContractRef"] = json!(api_contract_ref);
            aac["currentPhaseInterfaceRefs"] = json!(interfaces
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|interface| interface["interfaceId"].as_str())
                .collect::<Vec<_>>());
        }
        write_json_atomic(&aac_path, &aac).expect("write api contract interfaces");
    }

    fn read_spec(&self) -> DeploymentSpec {
        read_json(&self.root.join(".loom/deployment/specs/local.json")).expect("spec")
    }

    fn write_repair_action(&self, route: DeploymentRepairRoute, owner: DeploymentFailureOwner) {
        let spec = self.read_spec();
        let paths = self.root.join(".loom/deployment/state");
        ensure_dir(&paths).expect("repair state dir");
        write_json_atomic(
            &paths.join("repair-action.json"),
            &DeploymentRepairAction {
                schema_version: 1,
                repair_id: "deploy-repair-1".to_string(),
                created_at: now_string(),
                project_root: self.root_str(),
                spec_ref: ".loom/deployment/specs/local.json".to_string(),
                failure_kind: if route == DeploymentRepairRoute::ExecutionRepair {
                    DeploymentFailureKind::ApplicationStartupFailed
                } else {
                    DeploymentFailureKind::ApiRouteNotVerified
                },
                failure_owner: owner,
                repair_route: route,
                failure_ref: (route == DeploymentRepairRoute::ExecutionRepair)
                    .then_some(".loom/deployment/state/latest-failure.json".to_string()),
                command: vec![],
                exit_code: 1,
                full_log_ref: Some(".loom/deployment/logs/local.log".to_string()),
                error_window: Some(DeploymentErrorWindow {
                    lines: vec!["failed".to_string()],
                    truncated: false,
                    total_line_count: 1,
                    matched_patterns: vec!["error".to_string()],
                }),
                diagnostics: vec![DeploymentFailureDiagnostic {
                    code: "api_route_not_verified".to_string(),
                    severity: "error".to_string(),
                    message: "Generated API proxy route failed.".to_string(),
                    evidence: vec!["GET /api/health returned frontend HTML.".to_string()],
                    suggested_action: "Repair generated API proxy route.".to_string(),
                }],
                suggested_actions: vec!["Repair generated API proxy route.".to_string()],
                editable_files: vec![
                    spec.files.compose_path,
                    spec.files
                        .nginx_config_paths
                        .values()
                        .next()
                        .unwrap()
                        .clone(),
                ],
                protected_files: vec![],
                instruction: "repair".to_string(),
                max_attempts: 2,
                attempts: 0,
                status: "pending".to_string(),
            },
        )
        .expect("write repair action");
    }

    fn write_failure_report(&self) {
        ensure_dir(&self.root.join(".loom/deployment/state")).expect("state dir");
        write_json_atomic(
            &self.root.join(".loom/deployment/state/latest-failure.json"),
            &DeploymentFailureReport {
                schema_version: "1.0".to_string(),
                failure_id: "deploy-failure-1".to_string(),
                source: "deploy".to_string(),
                created_at: now_string(),
                deployment_attempt_id: "deploy-attempt-1".to_string(),
                failure_kind: DeploymentFailureKind::ApplicationStartupFailed,
                failure_owner: DeploymentFailureOwner::ApplicationCode,
                repair_route: DeploymentRepairRoute::ExecutionRepair,
                runtime_delivery_ref: Some(".loom/deliveries/delivery-1/contracts/architecture/phase-1/aac.json#/runtimeDelivery".to_string()),
                deployment_spec_ref: ".loom/deployment/specs/local.json".to_string(),
                failed_at: Some("runtime_application_startup".to_string()),
                failed_contract: Some(DeploymentFailedContract {
                    field: "runtime.startup".to_string(),
                    command: Some("java -jar app.jar".to_string()),
                    working_directory: "service".to_string(),
                }),
                deploy_command: vec![
                    "docker".to_string(),
                    "compose".to_string(),
                    "up".to_string(),
                ],
                exit_code: Some(1),
                full_log_ref: Some(".loom/deployment/logs/local.log".to_string()),
                failed_contract_fields: vec!["runtime.startup".to_string()],
                required_code_level_checks: vec!["check_runtime_startup".to_string()],
                error_window: DeploymentErrorWindow {
                    lines: vec!["backend failed to start".to_string()],
                    truncated: false,
                    total_line_count: 1,
                    matched_patterns: vec!["error".to_string()],
                },
                must_not_edit: vec![
                    ".loom".to_string(),
                    ".loom/deployment/specs/generated/compose.yaml".to_string(),
                    ".loom/deployment/specs/generated/nginx.frontend.conf".to_string(),
                ],
                attempt: 1,
                max_attempts: 2,
            },
        )
        .expect("write failure report");
    }
}

#[cfg(unix)]
struct PathEnvGuard {
    previous: String,
}

#[cfg(unix)]
impl Drop for PathEnvGuard {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.previous);
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn spawn_one_shot_http_server(port: u16) -> thread::JoinHandle<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind test http server");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            let body = "<!doctype html><html><body>Loom deploy test</body></html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    })
}

fn free_test_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind dynamic test port")
        .local_addr()
        .expect("test port addr")
        .port()
}

fn runtime_port<'a>(
    spec: &'a DeploymentSpec,
    service_id: &str,
) -> Option<&'a DeploymentRuntimePort> {
    spec.runtime
        .ports
        .iter()
        .find(|port| port.service_id == service_id)
}

fn runtime_public_port(spec: &DeploymentSpec) -> u16 {
    spec.runtime
        .ports
        .iter()
        .find(|port| port.service_id == spec.runtime.primary_service_id && !port.internal_only)
        .and_then(|port| port.host_port)
        .or_else(|| {
            spec.runtime
                .ports
                .iter()
                .find(|port| !port.internal_only)
                .and_then(|port| port.host_port)
        })
        .expect("public runtime host port")
}

fn runtime_primary_url(spec: &DeploymentSpec) -> String {
    spec.runtime
        .ports
        .iter()
        .find(|port| port.service_id == spec.runtime.primary_service_id && !port.internal_only)
        .and_then(|port| port.url.clone())
        .or_else(|| {
            spec.runtime
                .ports
                .iter()
                .find(|port| !port.internal_only)
                .and_then(|port| port.url.clone())
        })
        .expect("runtime primary url")
}

fn compose_service_block<'a>(compose: &'a str, service_id: &str) -> Option<&'a str> {
    let marker = format!("  {service_id}:");
    let start = compose.find(&marker)?;
    let rest = &compose[start + marker.len()..];
    let end = rest
        .find("\n  ")
        .map(|index| start + marker.len() + index)
        .unwrap_or(compose.len());
    Some(&compose[start..end])
}

fn test_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
