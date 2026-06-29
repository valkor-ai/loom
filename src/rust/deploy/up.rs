use std::{
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use contracts::DeploymentFailureKind;
use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{
    paths::from_project_relative,
    store::{path_exists, StateResult},
};

use crate::{
    active_operation::{acquire_operation, active_operation_result},
    paths::deployment_paths,
    prepare::{deploy_prepare_inner, read_spec},
    repair::write_repair_action,
    runtime_state::write_success_state,
    validate::{deploy_validate_inner, DeploymentValidationResult},
    DeployToolInput,
};

pub fn deploy_up(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root_buf = PathBuf::from(&input.project_root);
    let project_root = project_root_buf.as_path();
    let guard = match acquire_operation(project_root, "deploy.up", "building") {
        Ok(Ok(guard)) => guard,
        Ok(Err(operation)) => return active_operation_result(project_root, operation),
        Err(error) => {
            return LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: input.project_root,
                summary: "Deployment up could not acquire operation.".to_string(),
                details: Some(json!({ "error": error.to_string() })),
                warnings: vec![error.to_string()],
            })
        }
    };
    let result = deploy_up_inner(project_root, input);
    drop(guard);
    result
}

pub fn deploy_up_inner(project_root: &Path, input: DeployToolInput) -> LoomMcpActionResult {
    let paths = deployment_paths(project_root);
    if !path_exists(&paths.spec_file) {
        match deploy_prepare_inner(project_root, input.clone()) {
            Ok(LoomMcpActionResult::Done(_)) => {}
            Ok(result) => return result,
            Err(error) => {
                return LoomMcpActionResult::Blocked(delivery_core::LoomMcpBlockedResult {
                    project_root: project_root.to_string_lossy().into_owned(),
                    blockers: vec![error.to_string()],
                    recommended_tool: Some("loom.continue".to_string()),
                    details: Some(json!({ "failureKind": "runtime_contract_missing" })),
                })
            }
        }
    }
    let spec = match read_spec(project_root) {
        Ok(spec) => spec,
        Err(error) => {
            return LoomMcpActionResult::Blocked(delivery_core::LoomMcpBlockedResult {
                project_root: project_root.to_string_lossy().into_owned(),
                blockers: vec![error.to_string()],
                recommended_tool: Some("loom.deployPrepare".to_string()),
                details: None,
            })
        }
    };
    if let Err(result) = docker_available(project_root, &spec) {
        return result;
    }
    let compose_file = match from_project_relative(project_root, &spec.files.compose_path) {
        Ok(file) => file,
        Err(error) => {
            return write_repair_action(
                project_root,
                &spec,
                DeploymentFailureKind::ComposeConfig,
                vec![],
                1,
                "",
                &error.to_string(),
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
    };
    let compose_config = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_file)
        .args(["config", "--quiet"])
        .output();
    match compose_config {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            return write_repair_action(
                project_root,
                &spec,
                DeploymentFailureKind::ComposeConfig,
                vec![
                    "docker".to_string(),
                    "compose".to_string(),
                    "config".to_string(),
                ],
                output.status.code().unwrap_or(1),
                &String::from_utf8_lossy(&output.stdout),
                &String::from_utf8_lossy(&output.stderr),
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
        Err(error) => {
            return write_repair_action(
                project_root,
                &spec,
                DeploymentFailureKind::DockerUnavailable,
                vec![
                    "docker".to_string(),
                    "compose".to_string(),
                    "config".to_string(),
                ],
                1,
                "",
                &error.to_string(),
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
    }
    let up = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_file)
        .args(["up", "-d", "--build"])
        .output();
    match up {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let kind = classify_compose_up_failure(&spec, &stdout, &stderr);
            return write_repair_action(
                project_root,
                &spec,
                kind,
                vec![
                    "docker".to_string(),
                    "compose".to_string(),
                    "up".to_string(),
                ],
                output.status.code().unwrap_or(1),
                &stdout,
                &stderr,
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
        Err(error) => {
            return write_repair_action(
                project_root,
                &spec,
                DeploymentFailureKind::DockerUnavailable,
                vec![
                    "docker".to_string(),
                    "compose".to_string(),
                    "up".to_string(),
                ],
                1,
                "",
                &error.to_string(),
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
    }
    let validation = match wait_for_valid_deployment(project_root) {
        Ok(validation) => validation,
        Err(error) => {
            return write_repair_action(
                project_root,
                &spec,
                DeploymentFailureKind::DeployAssetInvalid,
                vec!["loom.deployValidate".to_string()],
                1,
                "",
                &error.to_string(),
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
    };
    if !validation.valid {
        let logs = compose_logs(&compose_file).unwrap_or_default();
        let kind = validation_failure_kind(&spec, &validation, &logs);
        let stdout = serde_json::to_string_pretty(&validation).unwrap_or_default();
        let stderr = if logs.trim().is_empty() {
            String::new()
        } else {
            format!("docker compose logs --tail=120\n{logs}")
        };
        return write_repair_action(
            project_root,
            &spec,
            kind,
            vec!["loom.deployValidate".to_string()],
            1,
            &stdout,
            &stderr,
        )
        .unwrap_or_else(|error| failed(project_root, error.to_string()));
    }
    let state_ref = match write_success_state(project_root, &spec, &validation) {
        Ok(state_ref) => state_ref,
        Err(error) => return failed(project_root, error.to_string()),
    };
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root.to_string_lossy().into_owned(),
        summary: "Deployment is running and validation passed.".to_string(),
        details: Some(json!({
            "url": spec.runtime.url,
            "preview": validation.preview,
            "apiRoutes": validation.api_routes,
            "stateRef": state_ref
        })),
        warnings: vec![],
    })
}

fn wait_for_valid_deployment(project_root: &Path) -> StateResult<DeploymentValidationResult> {
    let mut last = deploy_validate_inner(project_root)?;
    if last.valid {
        return Ok(last);
    }
    for _ in 0..11 {
        if !validation_is_retryable_startup(&last) {
            return Ok(last);
        }
        thread::sleep(Duration::from_millis(1500));
        last = deploy_validate_inner(project_root)?;
        if last.valid {
            return Ok(last);
        }
    }
    Ok(last)
}

fn validation_is_retryable_startup(validation: &DeploymentValidationResult) -> bool {
    validation.asset_issues.is_empty()
        && validation
            .preview
            .iter()
            .chain(validation.api_routes.iter())
            .any(|probe| {
                probe.status == "unreachable"
                    || probe
                        .error
                        .as_deref()
                        .map(|error| {
                            let lower = error.to_ascii_lowercase();
                            lower.contains("connection reset")
                                || lower.contains("connection refused")
                                || lower.contains("timed out")
                                || lower.contains("eof")
                        })
                        .unwrap_or(false)
            })
}

fn docker_available(
    project_root: &Path,
    spec: &contracts::DeploymentSpec,
) -> Result<(), LoomMcpActionResult> {
    let output = Command::new("docker").arg("--version").output();
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(write_repair_action(
            project_root,
            spec,
            DeploymentFailureKind::DockerUnavailable,
            vec!["docker".to_string(), "--version".to_string()],
            output.status.code().unwrap_or(1),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        )
        .unwrap_or_else(|error| failed(project_root, error.to_string()))),
        Err(error) => Err(write_repair_action(
            project_root,
            spec,
            DeploymentFailureKind::DockerUnavailable,
            vec!["docker".to_string(), "--version".to_string()],
            1,
            "",
            &error.to_string(),
        )
        .unwrap_or_else(|error| failed(project_root, error.to_string()))),
    }
}

fn classify_compose_up_failure(
    spec: &contracts::DeploymentSpec,
    stdout: &str,
    stderr: &str,
) -> DeploymentFailureKind {
    let text = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    if text.contains("network") || text.contains("no such host") || text.contains("tls handshake") {
        DeploymentFailureKind::RegistryNetwork
    } else if is_runtime_build_command_failure(spec, &text) {
        DeploymentFailureKind::BuildCommandFailed
    } else if text.contains("failed to solve")
        || text.contains("build")
        || text.contains("dockerfile")
    {
        DeploymentFailureKind::ImageBuild
    } else {
        DeploymentFailureKind::ContainerStart
    }
}

fn validation_failure_kind(
    spec: &contracts::DeploymentSpec,
    validation: &DeploymentValidationResult,
    logs: &str,
) -> DeploymentFailureKind {
    if !validation.asset_issues.is_empty() {
        return DeploymentFailureKind::DeployAssetInvalid;
    }
    if validation.preview.iter().any(|probe| probe.status != "ok") {
        if let Some(kind) = classify_startup_log_failure(spec, logs) {
            return kind;
        }
        return DeploymentFailureKind::PreviewNotVerified;
    }
    if validation
        .api_routes
        .iter()
        .any(|probe| probe.status != "ok" || probe.html_fallback || probe.status_code == Some(404))
    {
        return DeploymentFailureKind::ApiRouteNotVerified;
    }
    DeploymentFailureKind::PreviewNotVerified
}

fn compose_logs(compose_file: &Path) -> Option<String> {
    let output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .args(["logs", "--tail=120"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    (!combined.trim().is_empty()).then_some(combined)
}

fn is_runtime_build_command_failure(spec: &contracts::DeploymentSpec, text: &str) -> bool {
    let build_command = spec
        .runtime_contract
        .build_command
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let build_command_seen = !build_command.is_empty() && text.contains(&build_command);
    let known_build_step_seen = text.contains("npm run build")
        || text.contains("pnpm run build")
        || text.contains("yarn build")
        || text.contains("vite build")
        || text.contains("tsc -p")
        || text.contains("gradle")
        || text.contains("./gradlew")
        || text.contains("mvn ")
        || text.contains("maven");
    let compile_signal = text.contains("error ts")
        || text.contains("typescript")
        || text.contains("failed to compile")
        || text.contains("compilation failed")
        || text.contains("compilation failure")
        || text.contains("test failed")
        || text.contains("build failed");
    (build_command_seen || known_build_step_seen) && compile_signal
}

fn classify_startup_log_failure(
    spec: &contracts::DeploymentSpec,
    logs: &str,
) -> Option<DeploymentFailureKind> {
    let text = logs.to_ascii_lowercase();
    if text.trim().is_empty() {
        return None;
    }
    let start_command = spec.runtime_contract.start_command.as_deref().or_else(|| {
        spec.source_model
            .services
            .iter()
            .find(|service| service.service_id == spec.source_model.primary_service_id)
            .and_then(|service| service.start_command.as_deref())
    });
    let script_name = start_command.and_then(package_script_name_from_command);
    let missing_script = missing_script_name(&text);
    if start_command.is_some()
        && (text.contains("npm error missing script")
            || text.contains("npm err! missing script")
            || script_name
                .as_deref()
                .zip(missing_script.as_deref())
                .map(|(expected, actual)| expected == actual)
                .unwrap_or(false))
    {
        return Some(DeploymentFailureKind::StartCommandFailed);
    }
    if is_application_startup_failure(&text) {
        return Some(DeploymentFailureKind::ApplicationStartupFailed);
    }
    None
}

fn package_script_name_from_command(command: &str) -> Option<String> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        if matches!(*part, "npm" | "pnpm" | "bun") {
            let next = parts.get(index + 1).copied();
            let script = if next == Some("run") {
                parts.get(index + 2).copied()
            } else {
                next
            };
            return valid_script_name(script);
        }
        if *part == "yarn" {
            return valid_script_name(parts.get(index + 1).copied());
        }
    }
    None
}

fn valid_script_name(script: Option<&str>) -> Option<String> {
    let script = script?;
    if script == "--" || script == "run" || script.starts_with('-') {
        return None;
    }
    Some(
        script
            .trim_matches('"')
            .trim_matches('\'')
            .to_ascii_lowercase(),
    )
}

fn missing_script_name(text: &str) -> Option<String> {
    let marker = "missing script:";
    let start = text.find(marker)? + marker.len();
    let value = text[start..]
        .trim_start()
        .trim_matches('"')
        .trim_matches('\'')
        .split(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\'')
        .next()?;
    (!value.is_empty()).then(|| value.to_ascii_lowercase())
}

fn is_application_startup_failure(text: &str) -> bool {
    [
        "application failed to start",
        "beancreationexception",
        "unsatisfieddependencyexception",
        "applicationcontextexception",
        "webserverexception",
        "flywayexception",
        "liquibaseexception",
        "hibernateexception",
        "schemamanagementexception",
        "psqlexception",
        "communications link failure",
        "unable to obtain jdbc connection",
        "prisma",
        "django.db.utils",
        "improperlyconfigured",
        "sqlstate[",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn failed(project_root: &Path, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(delivery_core::LoomMcpFailureResult {
        project_root: project_root.to_string_lossy().into_owned(),
        error: delivery_core::LoomMcpFailure {
            code: "DEPLOY_UP_FAILED".to_string(),
            message,
            target_batch: Some(10),
            domain: Some("deploy".to_string()),
            route_action: None,
            recovery_tool: Some("loom.deployRepair".to_string()),
        },
    })
}
