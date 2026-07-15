use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use delivery_core::{
    normalize_project_root, ActiveOperationObservationPolicy, ActiveOperationRef,
    LoomMcpActionResult, LoomMcpActiveOperationResult, LoomMcpDoneResult, LoomMcpFailure,
    LoomMcpFailureResult, ProjectToolInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);
const OPERATION_STALE_AFTER_MS: u128 = 30 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserRuntimeOperation {
    schema_version: u32,
    operation_id: String,
    phase: String,
    status: String,
    started_at: String,
    updated_at: String,
    log_ref: String,
}

pub(crate) fn prepare(input: ProjectToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let setup = match locate_setup_binary() {
        Ok(path) => path,
        Err(message) => {
            return failed(
                &normalized.display,
                "BROWSER_RUNTIME_SETUP_MISSING",
                message,
            )
        }
    };
    prepare_with_setup(&normalized.path, &normalized.display, &setup)
}

fn prepare_with_setup(
    project_root: &Path,
    project_root_display: &str,
    setup: &Path,
) -> LoomMcpActionResult {
    let paths = BrowserRuntimePaths::new(project_root);
    if let Ok(Some(operation)) = live_operation(&paths) {
        return active_operation(project_root_display, operation);
    }
    let guard = match BrowserRuntimeOperationGuard::acquire(&paths) {
        Ok(guard) => guard,
        Err(message) => {
            return failed(
                project_root_display,
                "BROWSER_RUNTIME_OPERATION_FAILED",
                message,
            )
        }
    };
    let targets = execution::browser_runtime_targets(project_root);
    let versions = targets
        .iter()
        .map(|target| target.package_spec().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let mut command = Command::new(setup);
    command
        .current_dir(project_root)
        .args(["browser-runtime", "prepare"]);
    for version in &versions {
        command.arg("--playwright-version").arg(version);
    }
    let running = Arc::new(AtomicBool::new(true));
    let heartbeat = spawn_heartbeat(paths.clone(), running.clone());
    let output = command.output();
    running.store(false, Ordering::Release);
    heartbeat.thread().unpark();
    let _ = heartbeat.join();
    drop(guard);
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return unavailable_after_setup_failure(
                &paths,
                project_root_display,
                "BROWSER_RUNTIME_SETUP_FAILED",
                format!("loom-setup could not start: {error}"),
                &targets,
                &versions,
            )
        }
    };
    let _ = write_process_log(&paths.log_file, &output.stdout, &output.stderr);
    if !output.status.success() {
        return unavailable_after_setup_failure(
            &paths,
            project_root_display,
            "BROWSER_RUNTIME_SETUP_FAILED",
            format!(
                "Playwright runtime preparation failed with exit status {}: {}",
                output.status.code().unwrap_or(-1),
                bounded_text(&output.stderr)
            ),
            &targets,
            &versions,
        );
    }
    let report: Value = match serde_json::from_slice(&output.stdout) {
        Ok(report) => report,
        Err(error) => {
            return failed(
                project_root_display,
                "BROWSER_RUNTIME_REPORT_INVALID",
                format!("loom-setup returned invalid browser runtime JSON: {error}"),
            )
        }
    };
    let runtime_status = report
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unavailable")
        .to_string();
    let host_browser_path = report.get("browsersPath").cloned().unwrap_or(Value::Null);
    let runtime_environments = report
        .get("runtimes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|runtime| {
            let backend = runtime
                .get("backend")
                .and_then(Value::as_str)
                .unwrap_or("host");
            let browser_path = if backend == "managed_container" {
                runtime
                    .pointer("/managedContainer/browserPath")
                    .cloned()
                    .unwrap_or_else(|| json!("/ms-playwright"))
            } else {
                host_browser_path.clone()
            };
            json!({
                "runtimeId": runtime.get("runtimeId").cloned().unwrap_or(Value::Null),
                "requestedVersion": runtime.get("requestedVersion").cloned().unwrap_or(Value::Null),
                "resolvedVersion": runtime.get("resolvedVersion").cloned().unwrap_or(Value::Null),
                "status": runtime.get("status").cloned().unwrap_or(Value::Null),
                "backend": backend,
                "runnerPath": runtime.get("runnerPath").cloned().unwrap_or(Value::Null),
                "browserEnvironment": {"PLAYWRIGHT_BROWSERS_PATH": browser_path},
                "managedContainer": runtime.get("managedContainer").cloned().unwrap_or(Value::Null)
            })
        })
        .collect::<Vec<_>>();
    let details = json!({
        "status": runtime_status.clone(),
        "runtime": report,
        "runtimeEnvironments": runtime_environments,
        "projectDependencyPolicy": "Keep @playwright/test in the project package manifest and lockfile. Reuse only Loom's shared browser cache across projects.",
        "projectTargets": targets,
        "requestedRuntimeVersions": versions
    });
    let _ = state::store::write_json_atomic(&paths.latest_file, &details);
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root_display.to_string(),
        summary: match runtime_status.as_str() {
            "ready" => "Playwright browser runtime is ready.".to_string(),
            "partial" => {
                "Playwright browser runtime is ready for part of the project target matrix."
                    .to_string()
            }
            _ => "Playwright browser runtime is unavailable on both host and managed container."
                .to_string(),
        },
        details: Some(details),
        warnings: match runtime_status.as_str() {
            "ready" => vec![],
            "partial" => vec![
                "Some project Playwright targets are unavailable; the browser closure must use the runtime matching each project runner and report only affected checks as blocked."
                    .to_string(),
            ],
            _ => vec!["Browser environment evidence requires manual resolution.".to_string()],
        },
    })
}

fn unavailable_after_setup_failure(
    paths: &BrowserRuntimePaths,
    project_root: &str,
    code: &str,
    message: String,
    targets: &[execution::BrowserRuntimeTarget],
    versions: &std::collections::BTreeSet<String>,
) -> LoomMcpActionResult {
    let message = if message.trim().is_empty() {
        "Playwright runtime preparation failed before a browser could be launched.".to_string()
    } else {
        message
    };
    let attempted_versions = if versions.is_empty() {
        vec![Value::Null]
    } else {
        versions.iter().cloned().map(Value::String).collect()
    };
    let details = json!({
        "status": "unavailable",
        "runtime": {
            "status": "unavailable",
            "runtimes": attempted_versions.into_iter().map(|version| json!({
                "status": "unavailable",
                "backend": "unavailable",
                "requestedVersion": version,
                "doctorChecks": [{
                    "checkId": "runtime_prepare",
                    "scope": "environment",
                    "status": "failed",
                    "summary": message,
                    "failureCode": code,
                    "diagnostic": message,
                    "remediation": "Repair the package, registry, Node.js, browser, or container environment before selecting retry_browser_environment."
                }]
            })).collect::<Vec<_>>()
        },
        "runtimeEnvironments": [],
        "projectDependencyPolicy": "Keep @playwright/test in the project package manifest and lockfile. Reuse only Loom's shared browser cache across projects.",
        "projectTargets": targets,
        "requestedRuntimeVersions": versions
    });
    let _ = state::store::write_json_atomic(&paths.latest_file, &details);
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root.to_string(),
        summary:
            "Playwright browser runtime is unavailable in the supported execution environments."
                .to_string(),
        details: Some(details),
        warnings: vec![message],
    })
}

fn locate_setup_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("LOOM_SETUP_BIN")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Ok(path);
    }
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let sibling = parent.join(executable_name("loom-setup"));
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }
    if let Some(home) = std::env::var_os("LOOM_HOME").map(PathBuf::from) {
        let installed = home
            .join("runtime/current/bin")
            .join(executable_name("loom-setup"));
        if installed.is_file() {
            return Ok(installed);
        }
    }
    Ok(PathBuf::from(executable_name("loom-setup")))
}

fn executable_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

#[derive(Debug, Clone)]
struct BrowserRuntimePaths {
    state_dir: PathBuf,
    active_file: PathBuf,
    lock_file: PathBuf,
    latest_file: PathBuf,
    log_file: PathBuf,
}

impl BrowserRuntimePaths {
    fn new(project_root: &Path) -> Self {
        let state_dir = project_root.join(".loom/runtime/browser-automation");
        Self {
            active_file: state_dir.join("active-operation.json"),
            lock_file: state_dir.join("active-operation.lock"),
            latest_file: state_dir.join("latest.json"),
            log_file: state_dir.join("prepare.log"),
            state_dir,
        }
    }
}

struct BrowserRuntimeOperationGuard {
    paths: BrowserRuntimePaths,
}

impl BrowserRuntimeOperationGuard {
    fn acquire(paths: &BrowserRuntimePaths) -> Result<Self, String> {
        fs::create_dir_all(&paths.state_dir).map_err(|error| error.to_string())?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.lock_file)
            .map_err(|error| format!("browser runtime preparation is already active: {error}"))?;
        let operation = BrowserRuntimeOperation {
            schema_version: 1,
            operation_id: format!("browser_runtime_{}", state::store::now_millis()),
            phase: "preparing_shared_runtime".to_string(),
            status: "running".to_string(),
            started_at: state::store::now_string(),
            updated_at: state::store::now_string(),
            log_ref: ".loom/runtime/browser-automation/prepare.log".to_string(),
        };
        if let Err(error) = state::store::write_json_atomic(&paths.active_file, &operation) {
            let _ = fs::remove_file(&paths.lock_file);
            return Err(error.to_string());
        }
        Ok(Self {
            paths: paths.clone(),
        })
    }
}

impl Drop for BrowserRuntimeOperationGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.paths.active_file);
        let _ = fs::remove_file(&self.paths.lock_file);
    }
}

fn spawn_heartbeat(paths: BrowserRuntimePaths, running: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::Acquire) {
            thread::park_timeout(HEARTBEAT_INTERVAL);
            if !running.load(Ordering::Acquire) {
                break;
            }
            if let Ok(mut operation) =
                state::store::read_json::<BrowserRuntimeOperation>(&paths.active_file)
            {
                operation.updated_at = state::store::now_string();
                let _ = state::store::write_json_atomic(&paths.active_file, &operation);
            }
        }
    })
}

fn live_operation(paths: &BrowserRuntimePaths) -> Result<Option<BrowserRuntimeOperation>, String> {
    if !paths.active_file.is_file() {
        if paths.lock_file.is_file() {
            let stale = fs::metadata(&paths.lock_file)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age.as_millis() > OPERATION_STALE_AFTER_MS);
            if stale {
                let _ = fs::remove_file(&paths.lock_file);
            } else {
                let updated_at = fs::metadata(&paths.lock_file)
                    .and_then(|metadata| metadata.modified())
                    .ok()
                    .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis().to_string())
                    .unwrap_or_else(state::store::now_string);
                return Ok(Some(BrowserRuntimeOperation {
                    schema_version: 1,
                    operation_id: "browser_runtime_starting".to_string(),
                    phase: "acquiring_runtime_lock".to_string(),
                    status: "running".to_string(),
                    started_at: updated_at.clone(),
                    updated_at,
                    log_ref: ".loom/runtime/browser-automation/prepare.log".to_string(),
                }));
            }
        }
        return Ok(None);
    }
    let operation: BrowserRuntimeOperation =
        state::store::read_json(&paths.active_file).map_err(|error| error.to_string())?;
    let updated = operation.updated_at.parse::<u128>().unwrap_or(0);
    if updated > 0 && state::store::now_millis().saturating_sub(updated) > OPERATION_STALE_AFTER_MS
    {
        let _ = fs::remove_file(&paths.active_file);
        let _ = fs::remove_file(&paths.lock_file);
        return Ok(None);
    }
    Ok(Some(operation))
}

fn active_operation(project_root: &str, operation: BrowserRuntimeOperation) -> LoomMcpActionResult {
    LoomMcpActionResult::ActiveOperation(LoomMcpActiveOperationResult {
        project_root: project_root.to_string(),
        operation: ActiveOperationRef {
            operation_id: operation.operation_id,
            operation_type: "browser_runtime_prepare".to_string(),
            delivery_id: None,
            phase_id: None,
            started_at: operation.started_at,
            expires_at: (state::store::now_millis() + OPERATION_STALE_AFTER_MS).to_string(),
        },
        allowed_observation_tools: vec!["loom.browserRuntimePrepare".to_string()],
        observation_policy: Some(ActiveOperationObservationPolicy {
            quiet_mode: true,
            initial_quiet_window_ms: 60_000,
            min_next_observation_interval_ms: 30_000,
            logs_policy: "read_only_after_repeated_unchanged_status_or_user_request".to_string(),
            user_visible_update_policy: "terminal_result_or_phase_change_only".to_string(),
            final_response_policy: "forbidden_while_operation_active".to_string(),
        }),
        forbidden_actions: vec![
            "Do not start another Playwright runtime installation while this operation is active."
                .to_string(),
            "Do not delete or modify the shared Loom runtime cache during preparation.".to_string(),
        ],
        progress_summary: Some(json!({
            "phase": operation.phase,
            "status": operation.status,
            "updatedAt": operation.updated_at,
            "logRef": operation.log_ref
        })),
    })
}

fn write_process_log(path: &Path, stdout: &[u8], stderr: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    file.write_all(stdout)?;
    if !stderr.is_empty() {
        file.write_all(b"\n[stderr]\n")?;
        file.write_all(stderr)?;
    }
    file.flush()
}

fn bounded_text(bytes: &[u8]) -> String {
    const LIMIT: usize = 8 * 1024;
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).trim().to_string()
}

fn failed(project_root: &str, code: &str, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: code.to_string(),
            message,
            target_batch: Some(8),
            domain: Some("browser_runtime".to_string()),
            route_action: Some("browser_runtime_prepare".to_string()),
            recovery_tool: Some("loom.browserRuntimePrepare".to_string()),
        },
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn mcp_runtime_prepare_derives_versions_and_invokes_setup_binary() {
        let root = std::env::temp_dir().join(format!(
            "loom-browser-runtime-mcp-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"@playwright/test":"^1.55.0"}}"#,
        )
        .unwrap();
        let setup = root.join("fake-loom-setup.sh");
        fs::write(
            &setup,
            r#"#!/bin/sh
set -eu
printf '%s\n' "$@" > setup-args.txt
printf '{"status":"ready","cacheRoot":"/tmp/loom-cache","browsersPath":"/tmp/loom-cache/browsers","runtimes":[{"runtimeId":"pw-test","requestedVersion":"^1.55.0","resolvedVersion":"1.55.1","runnerPath":"/tmp/loom-cache/runner","manifestPath":"/tmp/loom-cache/manifest.json","reused":false,"doctorChecks":[]}]}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&setup).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&setup, permissions).unwrap();

        let result = prepare_with_setup(&root, &root.to_string_lossy(), &setup);

        let LoomMcpActionResult::Done(done) = result else {
            panic!("expected done result");
        };
        assert_eq!(done.summary, "Playwright browser runtime is ready.");
        assert_eq!(
            done.details.as_ref().unwrap()["requestedRuntimeVersions"],
            json!(["^1.55.0"])
        );
        let args = fs::read_to_string(root.join("setup-args.txt")).unwrap();
        assert!(args.contains("browser-runtime\nprepare"));
        assert!(args.contains("--playwright-version\n^1.55.0"));
        assert!(root
            .join(".loom/runtime/browser-automation/latest.json")
            .is_file());
        assert!(!root
            .join(".loom/runtime/browser-automation/active-operation.json")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_prepare_heartbeats_and_cancels_polling_after_completion() {
        let root = std::env::temp_dir().join(format!(
            "loom-browser-runtime-heartbeat-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        fs::create_dir_all(&root).unwrap();
        let setup = root.join("slow-loom-setup.sh");
        fs::write(
            &setup,
            r#"#!/bin/sh
set -eu
sleep 4
printf '{"status":"ready","cacheRoot":"/tmp/loom-cache","browsersPath":"/tmp/loom-cache/browsers","runtimes":[]}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&setup).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&setup, permissions).unwrap();
        let thread_root = root.clone();
        let thread_setup = setup.clone();
        let handle = thread::spawn(move || {
            prepare_with_setup(&thread_root, &thread_root.to_string_lossy(), &thread_setup)
        });
        let active_file = root.join(".loom/runtime/browser-automation/active-operation.json");
        for _ in 0..40 {
            if active_file.is_file() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let first: BrowserRuntimeOperation = state::store::read_json(&active_file).unwrap();
        thread::sleep(Duration::from_millis(3300));
        let refreshed: BrowserRuntimeOperation = state::store::read_json(&active_file).unwrap();
        assert!(
            refreshed.updated_at.parse::<u128>().unwrap()
                > first.updated_at.parse::<u128>().unwrap()
        );
        let result = handle.join().unwrap();
        assert!(matches!(result, LoomMcpActionResult::Done(_)));
        assert!(!active_file.exists());
        thread::sleep(Duration::from_millis(100));
        assert!(!active_file.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn setup_failure_records_unavailable_state_for_workflow_review() {
        let root = std::env::temp_dir().join(format!(
            "loom-browser-runtime-failed-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"@playwright/test":"1.55.0"}}"#,
        )
        .unwrap();
        let setup = root.join("failed-loom-setup.sh");
        fs::write(
            &setup,
            "#!/bin/sh\necho 'registry unavailable' >&2\nexit 1\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&setup).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&setup, permissions).unwrap();

        let result = prepare_with_setup(&root, &root.to_string_lossy(), &setup);

        let LoomMcpActionResult::Done(done) = result else {
            panic!("setup environment failure must become a reviewable unavailable state");
        };
        assert!(done.summary.contains("unavailable"));
        let latest: Value =
            state::store::read_json(&root.join(".loom/runtime/browser-automation/latest.json"))
                .unwrap();
        assert_eq!(latest["status"], "unavailable");
        assert_eq!(
            latest["runtime"]["runtimes"][0]["doctorChecks"][0]["status"],
            "failed"
        );
        assert_eq!(latest["projectTargets"][0]["resolvedVersion"], "1.55.0");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_runtime_matrix_remains_runnable_with_target_warning() {
        let root = std::env::temp_dir().join(format!(
            "loom-browser-runtime-partial-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        fs::create_dir_all(&root).unwrap();
        let setup = root.join("partial-loom-setup.sh");
        fs::write(
            &setup,
            r#"#!/bin/sh
printf '{"status":"partial","cacheRoot":"/tmp/loom-cache","browsersPath":"/tmp/loom-cache/browsers","runtimes":[{"status":"ready","backend":"host","runtimeId":"pw-ready","requestedVersion":"1.55.0","resolvedVersion":"1.55.0","runnerPath":"/tmp/loom-cache/runner","doctorChecks":[]},{"status":"unavailable","backend":"unavailable","runtimeId":"pw-blocked","requestedVersion":"1.56.0","resolvedVersion":"1.56.0","runnerPath":"/tmp/loom-cache/runner-2","doctorChecks":[{"status":"failed","summary":"Browser launch failed."}]}]}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&setup).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&setup, permissions).unwrap();

        let result = prepare_with_setup(&root, &root.to_string_lossy(), &setup);

        let LoomMcpActionResult::Done(done) = result else {
            panic!("partial runtime matrix must remain runnable");
        };
        assert!(done.summary.contains("part of the project target matrix"));
        assert_eq!(done.warnings.len(), 1);
        let latest: Value =
            state::store::read_json(&root.join(".loom/runtime/browser-automation/latest.json"))
                .unwrap();
        assert_eq!(latest["status"], "partial");
        assert_eq!(latest["runtimeEnvironments"].as_array().unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
