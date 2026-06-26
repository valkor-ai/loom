use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock},
};

use contracts::{
    DeploymentErrorWindow, DeploymentFailureKind, DeploymentFailureOwner, DeploymentFailureReport,
    DeploymentRepairAction, DeploymentRepairRoute, DeploymentShape, DeploymentSpec,
};
use delivery_core::{
    DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState, DeliveryStatusEntry,
    FileSubmitInput, InspectRequestInput, ProjectStatus, ReadRequestFieldsInput,
};
use deploy::{
    accept_deploy_execution_repair_file, deploy_prepare, deploy_repair, deploy_status,
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
    assert!(serde_json::to_string(&spec.topology)
        .unwrap()
        .contains("\"kind\":\"http-proxy\""));
    assert!(serde_json::to_string(&spec.topology)
        .unwrap()
        .contains("\"publicPath\":\"/api\""));

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
    assert!(compose.contains("  frontend:"));
    assert!(compose.contains("  backend:"));
    assert!(compose.contains("  postgres:"));
    assert!(compose.contains("      - backend"));
    assert!(compose.contains("      - postgres"));
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
    assert!(value["next"]["sourceModelRef"]
        .as_str()
        .unwrap()
        .ends_with("source-model.json"));
    assert!(value["next"]["topologyRef"]
        .as_str()
        .unwrap()
        .ends_with("topology.json"));
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
        .flat_map(|group| group.fields.iter())
        .any(|field| field == "outputContract.resultTemplate"));
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

    fn write_runtime_delivery(&self, runtime_delivery: Value) {
        let delivery_id = "delivery-1";
        let phase_id = "phase-1";
        let aac_ref = ".loom/deliveries/delivery-1/contracts/architecture/phase-1/aac.json";
        let aac_file = self.root.join(aac_ref);
        ensure_dir(aac_file.parent().unwrap()).expect("aac dir");
        write_json_atomic(
            &aac_file,
            &json!({
                "schemaVersion": "1.0",
                "architectureArtifactContractId": "aac-1",
                "runtimeDelivery": runtime_delivery
            }),
        )
        .expect("write aac");
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
                diagnostics: vec![],
                suggested_actions: vec![],
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

fn test_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
