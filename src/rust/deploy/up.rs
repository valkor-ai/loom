use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use contracts::{DeployProvider, DeploymentFailureKind, DeploymentProviderPolicy};
use delivery_core::{LoomMcpActionResult, LoomMcpDoneResult};
use serde_json::json;
use state::{
    paths::from_project_relative,
    store::{path_exists, StateResult},
};

use crate::{
    active_operation::{
        acquire_operation, active_operation_result, touch_operation, update_operation_phase,
    },
    paths::deployment_paths,
    port_plan::primary_url,
    prepare::{deploy_prepare_inner, read_spec},
    repair::write_repair_action,
    runtime_state::write_success_state,
    validate::{deploy_validate_inner, validate_generated_assets, DeploymentValidationResult},
    DeployToolInput,
};

const DEPLOY_STARTUP_VALIDATION_ATTEMPTS: usize = 24;
const DEPLOY_STARTUP_VALIDATION_INTERVAL: Duration = Duration::from_millis(1500);
const DEPLOY_OPERATION_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const DEPLOY_OPERATION_HEARTBEAT_POLL_INTERVAL: Duration = Duration::from_secs(3);

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
        let _ = update_operation_phase(project_root, "preparing", "running");
        match deploy_prepare_inner(project_root, input.clone()) {
            Ok(LoomMcpActionResult::Done(_)) => {}
            Ok(result) => return result,
            Err(error) => {
                return LoomMcpActionResult::Blocked(delivery_core::LoomMcpBlockedResult {
                    project_root: project_root.to_string_lossy().into_owned(),
                    blockers: vec![error.to_string()],
                    recommended_tool: Some("loom.deployInspect".to_string()),
                    details: Some(json!({ "failureKind": "deploy_prepare_failed" })),
                })
            }
        }
    }
    let _ = update_operation_phase(project_root, "checking_docker", "running");
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
    match validate_generated_assets(project_root, &spec) {
        Ok(asset_issues) if asset_issues.is_empty() => {}
        Ok(asset_issues) => {
            if should_fallback_to_generated(&spec) {
                return fallback_to_generated(
                    project_root,
                    input,
                    "existing deployment assets failed consistency validation",
                );
            }
            return write_repair_action(
                project_root,
                &spec,
                DeploymentFailureKind::DeployAssetInvalid,
                vec!["loom.deployValidate".to_string()],
                1,
                "",
                &asset_issues.join("\n"),
            )
            .unwrap_or_else(|error| failed(project_root, error.to_string()));
        }
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
    }
    let _ = update_operation_phase(project_root, "checking_compose", "running");
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
    let compose_file_arg = compose_file.to_string_lossy().into_owned();
    let compose_config_args = vec![
        "compose".to_string(),
        "-f".to_string(),
        compose_file_arg.clone(),
        "config".to_string(),
        "--quiet".to_string(),
    ];
    let compose_config = run_logged_command(
        project_root,
        "checking_compose",
        "docker",
        &compose_config_args,
    );
    match compose_config {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            if should_fallback_to_generated(&spec) {
                return fallback_to_generated(
                    project_root,
                    input,
                    "existing Compose config failed",
                );
            }
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
            if should_fallback_to_generated(&spec) {
                return fallback_to_generated(
                    project_root,
                    input,
                    "existing Compose config could not run",
                );
            }
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
    let _ = update_operation_phase(project_root, "building", "running");
    let up_args = vec![
        "compose".to_string(),
        "-f".to_string(),
        compose_file_arg,
        "up".to_string(),
        "-d".to_string(),
        "--build".to_string(),
    ];
    let up = run_logged_command(project_root, "building", "docker", &up_args);
    match up {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let kind = classify_compose_up_failure(&spec, &stdout, &stderr);
            if should_fallback_to_generated(&spec) {
                return fallback_to_generated(
                    project_root,
                    input,
                    "existing deployment provider failed",
                );
            }
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
            if should_fallback_to_generated(&spec) {
                return fallback_to_generated(
                    project_root,
                    input,
                    "existing deployment provider could not run",
                );
            }
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
    let _ = update_operation_phase(project_root, "validating", "running");
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
        if should_fallback_to_generated(&spec) {
            return fallback_to_generated(
                project_root,
                input,
                "existing deployment validation failed",
            );
        }
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
            "primaryUrl": primary_url(&spec.runtime),
            "ports": spec.runtime.ports,
            "preview": validation.preview,
            "apiRoutes": validation.api_routes,
            "stateRef": state_ref
        })),
        warnings: vec![],
    })
}

fn run_logged_command(
    project_root: &Path,
    phase: &str,
    program: &str,
    args: &[String],
) -> io::Result<Output> {
    let paths = deployment_paths(project_root);
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)?;
    let log = Arc::new(Mutex::new(log_file));
    write_log_line(
        &log,
        &format!(
            "\n[{}] phase={} command={}",
            state::store::now_string(),
            phase,
            shell_display(program, args)
        ),
    )?;
    let mut child = match Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = write_log_line(
                &log,
                &format!("[{}] spawn-error={}", state::store::now_string(), error),
            );
            return Err(error);
        }
    };

    let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
    let stderr_buffer = Arc::new(Mutex::new(Vec::new()));
    let heartbeat = OperationHeartbeat::new(project_root);
    let stdout_handle = child.stdout.take().map(|stdout| {
        tee_stream(
            stdout,
            "stdout",
            log.clone(),
            stdout_buffer.clone(),
            heartbeat.clone(),
        )
    });
    let stderr_handle = child.stderr.take().map(|stderr| {
        tee_stream(
            stderr,
            "stderr",
            log.clone(),
            stderr_buffer.clone(),
            heartbeat.clone(),
        )
    });
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        heartbeat.touch_if_due();
        thread::sleep(heartbeat.poll_interval());
    };
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }
    write_log_line(
        &log,
        &format!(
            "[{}] phase={} exit={}",
            state::store::now_string(),
            phase,
            status.code().unwrap_or(-1)
        ),
    )?;
    let stdout = stdout_buffer
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();
    let stderr = stderr_buffer
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn tee_stream<R: Read + Send + 'static>(
    mut reader: R,
    label: &'static str,
    log: Arc<Mutex<File>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    heartbeat: OperationHeartbeat,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut chunk = [0_u8; 8192];
        let mut wrote_label = false;
        loop {
            let Ok(read) = reader.read(&mut chunk) else {
                break;
            };
            if read == 0 {
                break;
            }
            if let Ok(mut output) = buffer.lock() {
                output.extend_from_slice(&chunk[..read]);
            }
            heartbeat.touch_if_due();
            if let Ok(mut file) = log.lock() {
                if !wrote_label {
                    let _ = writeln!(file, "\n[{label}]");
                    wrote_label = true;
                }
                let _ = file.write_all(&chunk[..read]);
                let _ = file.flush();
            }
        }
    })
}

#[derive(Clone)]
struct OperationHeartbeat {
    project_root: PathBuf,
    interval: Duration,
    last_touch: Arc<Mutex<Instant>>,
}

impl OperationHeartbeat {
    fn new(project_root: &Path) -> Self {
        let interval = operation_heartbeat_interval();
        let now = Instant::now();
        Self {
            project_root: project_root.to_path_buf(),
            interval,
            last_touch: Arc::new(Mutex::new(now.checked_sub(interval).unwrap_or(now))),
        }
    }

    fn poll_interval(&self) -> Duration {
        self.interval.min(DEPLOY_OPERATION_HEARTBEAT_POLL_INTERVAL)
    }

    fn touch_if_due(&self) {
        let Ok(mut last_touch) = self.last_touch.lock() else {
            return;
        };
        if last_touch.elapsed() < self.interval {
            return;
        }
        if touch_operation(&self.project_root).is_ok() {
            *last_touch = Instant::now();
        }
    }
}

fn operation_heartbeat_interval() -> Duration {
    std::env::var("LOOM_DEPLOY_OPERATION_HEARTBEAT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value >= 50)
        .map(Duration::from_millis)
        .unwrap_or(DEPLOY_OPERATION_HEARTBEAT_INTERVAL)
}

fn write_log_line(log: &Arc<Mutex<File>>, line: &str) -> io::Result<()> {
    let mut file = log
        .lock()
        .map_err(|_| io::Error::other("deploy log mutex poisoned"))?;
    writeln!(file, "{line}")?;
    file.flush()
}

fn shell_display(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| {
            if arg
                .chars()
                .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\'' | '$' | '\\'))
            {
                format!("'{}'", arg.replace('\'', "'\\''"))
            } else {
                arg.clone()
            }
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

fn should_fallback_to_generated(spec: &contracts::DeploymentSpec) -> bool {
    matches!(
        spec.provider,
        DeployProvider::ComposeExisting | DeployProvider::DockerfileExisting
    ) && !spec.provider_policy.force_generate
        && spec.provider_policy.provider.is_none()
}

fn fallback_to_generated(
    project_root: &Path,
    mut input: DeployToolInput,
    _reason: &str,
) -> LoomMcpActionResult {
    input.provider_policy = Some(DeploymentProviderPolicy {
        provider: Some(DeployProvider::Generated),
        reuse_existing: false,
        force_generate: true,
    });
    match deploy_prepare_inner(project_root, input.clone()) {
        Ok(LoomMcpActionResult::Done(_)) => deploy_up_inner(project_root, input),
        Ok(result) => result,
        Err(error) => failed(project_root, error.to_string()),
    }
}

fn wait_for_valid_deployment(project_root: &Path) -> StateResult<DeploymentValidationResult> {
    let mut last = deploy_validate_inner(project_root)?;
    if last.valid {
        return Ok(last);
    }
    for _ in 1..DEPLOY_STARTUP_VALIDATION_ATTEMPTS {
        if !validation_is_retryable_startup(&last) {
            return Ok(last);
        }
        thread::sleep(DEPLOY_STARTUP_VALIDATION_INTERVAL);
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
    if looks_like_docker_unavailable_failure(&text) {
        DeploymentFailureKind::DockerUnavailable
    } else if looks_like_registry_network_failure(&text) {
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

fn looks_like_docker_unavailable_failure(text: &str) -> bool {
    [
        "failed to connect to the docker api",
        "cannot connect to the docker daemon",
        "is the docker daemon running",
        "docker daemon is not running",
        "dial unix",
        "docker.sock",
        "error during connect",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn looks_like_registry_network_failure(text: &str) -> bool {
    [
        "failed to fetch oauth token",
        "failed to authorize",
        "deadlineexceeded",
        "i/o timeout",
        "tls handshake timeout",
        "temporary failure in name resolution",
        "no such host",
        "connection timed out",
        "network is unreachable",
        "registry-1.docker.io",
        "auth.docker.io",
    ]
    .iter()
    .any(|needle| text.contains(needle))
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
