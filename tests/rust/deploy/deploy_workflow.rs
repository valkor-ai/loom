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
    DeploymentErrorWindow, DeploymentFailedContract, DeploymentFailureKind, DeploymentFailureOwner,
    DeploymentFailureReport, DeploymentRepairAction, DeploymentRepairRoute, DeploymentShape,
    DeploymentSpec, PackageManager, SourceModelSource,
};
use delivery_core::{
    DeliveryIndex, DeliveryLifecycleStatus, DeliveryPhaseState, DeliveryStatusEntry,
    FileSubmitInput, InspectRequestInput, ProjectStatus, ReadFieldGroupInput,
    ReadRequestFieldsInput,
};
use deploy::{
    accept_deploy_execution_repair_file, deploy_prepare, deploy_repair, deploy_status, deploy_up,
    deploy_validate, DeployToolInput,
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
    assert!(compose.contains("      dockerfile: Dockerfile.frontend"));
    assert!(compose.contains("      dockerfile: Dockerfile.backend"));
    assert!(compose.contains("  backend:"));
    assert!(compose.contains("  postgres:"));
    assert!(compose.contains("      - backend"));
    assert!(compose.contains("      - postgres"));
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
    assert!(compose.contains("dockerfile: Dockerfile.app"), "{compose}");
    assert!(
        !compose.contains("dockerfile: .loom/deployment/specs/generated/Dockerfile.app"),
        "{compose}"
    );

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
fn deploy_validate_success_writes_state_and_clears_failure_artifacts() {
    let fixture = Fixture::new("deploy-validate-clears-failure");
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
    let spec: DeploymentSpec =
        read_json(&fixture.root.join(".loom/deployment/specs/local.json")).expect("spec");
    let _server = spawn_one_shot_http_server(spec.runtime.host_port);
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
    assert!(compose.contains("dockerfile: Dockerfile.app"), "{compose}");
    std::fs::write(
        &compose_path,
        compose.replace(
            "dockerfile: Dockerfile.app",
            "dockerfile: .loom/deployment/specs/generated/Dockerfile.app",
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
        .flat_map(|group| group.fields.iter())
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
        .flat_map(|group| group.fields.iter())
        .any(|field| field == "outputContract.resultTemplate"));
    assert!(!inspected
        .read_groups
        .iter()
        .flat_map(|group| group.fields.iter())
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
    assert_eq!(value["details"]["attempts"], 2);
    assert_eq!(value["details"]["maxAttempts"], 2);
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

fn test_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
