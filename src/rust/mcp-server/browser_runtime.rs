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
    let versions = execution::browser_runtime_package_specs(project_root);
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
            return failed(
                project_root_display,
                "BROWSER_RUNTIME_SETUP_FAILED",
                format!("loom-setup could not start: {error}"),
            )
        }
    };
    let _ = write_process_log(&paths.log_file, &output.stdout, &output.stderr);
    if !output.status.success() {
        return failed(
            project_root_display,
            "BROWSER_RUNTIME_SETUP_FAILED",
            format!(
                "Playwright runtime preparation failed with exit status {}: {}",
                output.status.code().unwrap_or(-1),
                bounded_text(&output.stderr)
            ),
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
    let details = json!({
        "runtime": report,
        "browserEnvironment": {
            "PLAYWRIGHT_BROWSERS_PATH": report.get("browsersPath").cloned().unwrap_or(Value::Null)
        },
        "projectDependencyPolicy": "Keep @playwright/test in the project package manifest and lockfile. Reuse only Loom's shared browser cache across projects.",
        "requestedProjectVersions": versions
    });
    let _ = state::store::write_json_atomic(&paths.latest_file, &details);
    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root_display.to_string(),
        summary: "Playwright browser runtime is ready.".to_string(),
        details: Some(details),
        warnings: vec![],
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
            done.details.as_ref().unwrap()["requestedProjectVersions"],
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
}
