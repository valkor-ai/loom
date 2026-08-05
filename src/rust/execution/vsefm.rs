use std::path::{Path, PathBuf};
use std::process::Command;

use delivery_core::{
    apply_delivery_index, current_phase, loom_home, loom_runtime_home, DomainDispatcher,
    LoomMcpActionResult, LoomMcpDoneResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpUserGateResult, ProjectStatus, RouteAction, RouteActionKind, TransitionEngine,
    TransitionStore,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state::lifecycle_store::FileTransitionStore;

const CONFIG_RELATIVE_PATH: &str = "plugins/shared/loom/references/verification/v-sefm.json";
const RECORD_FILE_NAME: &str = "v-sefm.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VsefmToolInput {
    pub project_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<VsefmDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VsefmDecision {
    Required,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VsefmConfig {
    content: String,
    url: String,
    #[serde(default)]
    auto_route_after_review: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VsefmRecord {
    schema_version: u32,
    delivery_id: Option<String>,
    phase_id: Option<String>,
    decision: Option<VsefmDecision>,
    status: String,
    trigger: String,
    config_url: String,
    app_key_present: bool,
    url_opened: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppKeyState {
    present: bool,
}

pub fn verify<D>(input: VsefmToolInput, dispatcher: D) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    let config = match load_config() {
        Ok(config) => config,
        Err(message) => return failed(&input.project_root, "VSEFM_CONFIG_INVALID", message),
    };
    let store = FileTransitionStore;
    let active = match store.load_status(&input.project_root) {
        Ok(status) => active_delivery(&store, &input.project_root, &status),
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_STATE_UNAVAILABLE",
                error.to_string(),
            )
        }
    };

    let (delivery_id, phase_id, resume_action, trigger) = match active {
        Some((delivery, phase)) => {
            let resume_action = phase
                .next_action
                .as_ref()
                .and_then(resume_action_from_onboarding)
                .or_else(|| phase.next_action.clone());
            let trigger = phase
                .next_action
                .as_ref()
                .and_then(|action| action.details.as_ref())
                .and_then(|details| details.get("trigger"))
                .and_then(Value::as_str)
                .unwrap_or("explicit")
                .to_string();
            (
                Some(delivery.delivery_id),
                Some(phase.phase_id),
                resume_action,
                trigger,
            )
        }
        None => (None, None, None, "explicit".to_string()),
    };

    let action = onboarding_action(
        &config,
        delivery_id.as_deref(),
        phase_id.as_deref(),
        resume_action.clone(),
        &trigger,
    );
    if let Err(message) = persist_pending_gate(
        &input.project_root,
        delivery_id.as_deref(),
        phase_id.as_deref(),
        &action,
        &config,
        &trigger,
    ) {
        return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", message);
    }

    match input.decision {
        None => onboarding_result(
            &input.project_root,
            delivery_id.as_deref(),
            phase_id.as_deref(),
            &config,
            &action,
        ),
        Some(decision) => resolve(
            &input.project_root,
            delivery_id.as_deref(),
            phase_id.as_deref(),
            decision,
            &config,
            &action,
            dispatcher,
        ),
    }
}

pub fn maybe_auto_route_after_review(
    _project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    resume_action: RouteAction,
) -> RouteAction {
    let Ok(config) = load_config() else {
        return resume_action;
    };
    if !matches!(
        resume_action.kind,
        RouteActionKind::Done | RouteActionKind::ContinueToNextPhase
    ) || !config.auto_route_after_review
    {
        return resume_action;
    }
    onboarding_action(
        &config,
        Some(delivery_id),
        Some(phase_id),
        Some(resume_action),
        "review",
    )
}

fn active_delivery(
    store: &FileTransitionStore,
    project_root: &str,
    status: &ProjectStatus,
) -> Option<(
    delivery_core::DeliveryIndex,
    delivery_core::DeliveryPhaseState,
)> {
    let delivery_id = status.active_delivery_id.as_ref()?;
    let delivery = store.load_delivery_index(project_root, delivery_id).ok()?;
    let phase = current_phase(&delivery)?.clone();
    Some((delivery, phase))
}

fn onboarding_action(
    config: &VsefmConfig,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    resume_action: Option<RouteAction>,
    trigger: &str,
) -> RouteAction {
    let gate_id = format!(
        "vsefm_{}_{}",
        delivery_id.unwrap_or("standalone"),
        phase_id.unwrap_or("verify")
    );
    RouteAction {
        kind: RouteActionKind::VsefmOnboarding,
        source: "vsefm".to_string(),
        reason: "V-SEFM verification choice is required.".to_string(),
        prompt: Some(config.content.clone()),
        accepted_responses: vec!["required".to_string(), "deferred".to_string()],
        request_ref: None,
        details: Some(json!({
            "gateId": gate_id,
            "kind": "vsefm_onboarding",
            "trigger": trigger,
            "url": config.url,
            "resumeAction": resume_action,
        })),
        target_phase_id: None,
    }
}

fn resume_action_from_onboarding(action: &RouteAction) -> Option<RouteAction> {
    if action.kind != RouteActionKind::VsefmOnboarding {
        return None;
    }
    action
        .details
        .as_ref()
        .and_then(|details| details.get("resumeAction"))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn persist_pending_gate(
    project_root: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    action: &RouteAction,
    config: &VsefmConfig,
    trigger: &str,
) -> Result<(), String> {
    let record = VsefmRecord {
        schema_version: 1,
        delivery_id: delivery_id.map(str::to_string),
        phase_id: phase_id.map(str::to_string),
        decision: None,
        status: "pending_user_choice".to_string(),
        trigger: trigger.to_string(),
        config_url: config.url.clone(),
        app_key_present: false,
        url_opened: false,
        warning: None,
        created_at: state::store::now_string(),
        updated_at: state::store::now_string(),
    };
    write_record(project_root, &record)?;
    if let (Some(delivery_id), Some(phase_id)) = (delivery_id, phase_id) {
        let store = FileTransitionStore;
        let mut status = store
            .load_status(project_root)
            .map_err(|error| error.to_string())?;
        let mut delivery = store
            .load_delivery_index(project_root, delivery_id)
            .map_err(|error| error.to_string())?;
        let phase = delivery
            .phases
            .iter_mut()
            .find(|phase| phase.phase_id == phase_id)
            .ok_or_else(|| format!("phase {phase_id} does not exist"))?;
        phase.next_action = Some(action.clone());
        delivery.updated_at = state::store::now_string();
        store
            .save_delivery_index(project_root, &delivery)
            .map_err(|error| error.to_string())?;
        apply_delivery_index(&mut status, &delivery);
        store
            .save_status(project_root, &status)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn onboarding_result(
    project_root: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    config: &VsefmConfig,
    action: &RouteAction,
) -> LoomMcpActionResult {
    LoomMcpActionResult::UserGate(
        LoomMcpUserGateResult::new(
            project_root.to_string(),
            action
                .prompt
                .clone()
                .unwrap_or_else(|| config.content.clone()),
            action.accepted_responses.clone(),
            None,
            delivery_id.map(str::to_string),
            phase_id.map(str::to_string),
            action.details.clone(),
        )
        .with_agent_instruction(
            "Present the V-SEFM onboarding content and wait for the user's required or deferred choice. Then call loom.verify with that decision. Loom opens the configured platform only when required and the local appkey is absent, records a warning if browser launch fails, and resumes immediately without waiting for an external V-SEFM result.",
        ),
    )
}

fn resolve<D>(
    project_root: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    decision: VsefmDecision,
    config: &VsefmConfig,
    action: &RouteAction,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    let mut warnings = Vec::new();
    let app_key = if decision == VsefmDecision::Required {
        match read_app_key_state() {
            Ok(state) => Some(state),
            Err(message) => return failed(project_root, "VSEFM_APPKEY_UNAVAILABLE", message),
        }
    } else {
        None
    };
    let mut url_opened = false;
    if decision == VsefmDecision::Required {
        let app_key = app_key
            .as_ref()
            .expect("required decision has appkey state");
        if !app_key.present {
            match open_url(&config.url) {
                Ok(()) => url_opened = true,
                Err(message) => warnings.push(message),
            }
        }
    }

    let resume_action = action
        .details
        .as_ref()
        .and_then(|details| details.get("resumeAction"))
        .and_then(|value| serde_json::from_value::<RouteAction>(value.clone()).ok());
    let now = state::store::now_string();
    let record = VsefmRecord {
        schema_version: 1,
        delivery_id: delivery_id.map(str::to_string),
        phase_id: phase_id.map(str::to_string),
        decision: Some(decision),
        status: if warnings.is_empty() {
            "completed".to_string()
        } else {
            "completed_with_warning".to_string()
        },
        trigger: action
            .details
            .as_ref()
            .and_then(|details| details.get("trigger"))
            .and_then(Value::as_str)
            .unwrap_or("explicit")
            .to_string(),
        config_url: config.url.clone(),
        app_key_present: app_key.as_ref().is_some_and(|state| state.present),
        url_opened,
        warning: warnings.first().cloned(),
        created_at: now.clone(),
        updated_at: now,
    };
    if let Err(message) = write_record(project_root, &record) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", message);
    }

    let Some((delivery_id, phase_id, resume_action)) = delivery_id
        .zip(phase_id)
        .zip(resume_action)
        .map(|((d, p), a)| (d, p, a))
    else {
        return LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: project_root.to_string(),
            summary: "V-SEFM verification choice recorded.".to_string(),
            details: Some(json!({
                "decision": decision,
                "urlOpened": url_opened,
                "record": ".loom/verification/v-sefm.json"
            })),
            warnings,
        });
    };

    let store = FileTransitionStore;
    let mut status = match store.load_status(project_root) {
        Ok(status) => status,
        Err(error) => return failed(project_root, "VSEFM_STATE_UNAVAILABLE", error.to_string()),
    };
    let mut delivery = match store.load_delivery_index(project_root, &delivery_id) {
        Ok(delivery) => delivery,
        Err(error) => return failed(project_root, "VSEFM_STATE_UNAVAILABLE", error.to_string()),
    };
    let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return failed(
            project_root,
            "VSEFM_STATE_UNAVAILABLE",
            format!("phase {phase_id} does not exist"),
        );
    };
    phase.next_action = Some(resume_action.clone());
    if resume_action.kind == RouteActionKind::Done {
        delivery.status = if resume_action.source == "manual_review_resolution" {
            delivery_core::DeliveryLifecycleStatus::CompletedWithOverride
        } else {
            delivery_core::DeliveryLifecycleStatus::Completed
        };
    } else {
        delivery.status = delivery_core::DeliveryLifecycleStatus::Executing;
    }
    delivery.updated_at = state::store::now_string();
    if let Err(error) = store.save_delivery_index(project_root, &delivery) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    apply_delivery_index(&mut status, &delivery);
    if let Err(error) = store.save_status(project_root, &status) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    if resume_action.kind == RouteActionKind::Done {
        return LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: project_root.to_string(),
            summary: "V-SEFM verification choice recorded and Loom delivery completed.".to_string(),
            details: Some(json!({
                "decision": decision,
                "urlOpened": url_opened,
                "record": ".loom/verification/v-sefm.json"
            })),
            warnings,
        });
    }
    let result = TransitionEngine { store, dispatcher }
        .continue_current(delivery_core::OperationContext {
            project_root: project_root.to_string(),
        })
        .unwrap_or_else(|error| failed(project_root, "VSEFM_RESUME_FAILED", error.to_string()));
    result.with_warnings(warnings)
}

fn write_record(project_root: &str, record: &VsefmRecord) -> Result<(), String> {
    let path = Path::new(project_root)
        .join(".loom")
        .join("verification")
        .join(RECORD_FILE_NAME);
    state::store::write_json_atomic(&path, record).map_err(|error| error.to_string())
}

fn load_config() -> Result<VsefmConfig, String> {
    let path = std::env::var_os("LOOM_VSEFM_CONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            loom_runtime_home()
                .ok()
                .map(|home| home.join(CONFIG_RELATIVE_PATH))
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(repository_config_path);
    let raw = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read V-SEFM config {}: {error}", path.display()))?;
    let config: VsefmConfig = serde_json::from_str(&raw)
        .map_err(|error| format!("cannot parse V-SEFM config {}: {error}", path.display()))?;
    validate_config(&config)?;
    Ok(config)
}

fn repository_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(CONFIG_RELATIVE_PATH)
}

fn validate_config(config: &VsefmConfig) -> Result<(), String> {
    if config.content.trim().is_empty() {
        return Err("V-SEFM config content must not be empty".to_string());
    }
    let url = config.url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://"))
        || url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || url.len() <= "https://".len()
    {
        return Err(
            "V-SEFM config url must be a non-empty http(s) URL without whitespace".to_string(),
        );
    }
    if url
        .chars()
        .any(|character| matches!(character, '&' | '|' | '<' | '>' | '^' | '"' | '\''))
    {
        return Err("V-SEFM config url contains a command-interpolation character".to_string());
    }
    Ok(())
}

fn read_app_key_state() -> Result<AppKeyState, String> {
    let home = loom_home()?;
    let directory = home.join("v-sefm");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    set_private_directory_permissions(&directory)?;
    let path = directory.join("appkey");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    drop(file);
    set_private_permissions(&path)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(AppKeyState {
        present: !content.trim().is_empty(),
    })
}

fn set_private_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?
            .permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| format!("cannot protect {}: {error}", path.display()))?;
    }
    Ok(())
}

fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| format!("cannot protect {}: {error}", path.display()))?;
    }
    Ok(())
}

fn open_url(url: &str) -> Result<(), String> {
    let (program, args): (&str, Vec<String>) = if cfg!(target_os = "macos") {
        ("open", vec![url.to_string()])
    } else if cfg!(target_os = "windows") {
        (
            "cmd.exe",
            vec![
                "/C".to_string(),
                "start".to_string(),
                "".to_string(),
                url.to_string(),
            ],
        )
    } else {
        ("xdg-open", vec![url.to_string()])
    };
    let status = Command::new(program)
        .args(&args)
        .status()
        .map_err(|error| format!("could not open V-SEFM URL with {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "could not open V-SEFM URL with {program} (exit status {status})"
        ))
    }
}

fn failed(project_root: &str, code: &str, message: impl Into<String>) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: code.to_string(),
            message: message.into(),
            target_batch: Some(9),
            domain: Some("verification".to_string()),
            route_action: Some("vsefm_onboarding".to_string()),
            recovery_tool: Some("loom.verify".to_string()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    #[test]
    fn review_auto_route_is_controlled_by_config_switch() {
        let _guard = env_lock();
        let config_path = std::env::temp_dir().join(format!(
            "loom-vsefm-config-{}-{}.json",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::write(
            &config_path,
            r#"{"content":"verify","url":"https://platform.example.test","autoRouteAfterReview":true}"#,
        )
        .expect("write test config");
        let previous = std::env::var_os("LOOM_VSEFM_CONFIG");
        std::env::set_var("LOOM_VSEFM_CONFIG", &config_path);

        let resume = RouteAction {
            kind: RouteActionKind::Done,
            source: "review_result".to_string(),
            reason: "approved".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some("review-result".to_string()),
            details: None,
            target_phase_id: None,
        };
        let routed = maybe_auto_route_after_review("/tmp/project", "delivery-1", "phase-1", resume);
        assert_eq!(routed.kind, RouteActionKind::VsefmOnboarding);
        assert!(routed.request_ref.is_none());
        assert_eq!(routed.details.as_ref().unwrap()["trigger"], "review");

        restore_env(previous);
        let _ = std::fs::remove_file(config_path);
    }

    #[test]
    fn review_auto_route_disabled_keeps_original_action() {
        let _guard = env_lock();
        let config_path = std::env::temp_dir().join(format!(
            "loom-vsefm-config-disabled-{}-{}.json",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::write(
            &config_path,
            r#"{"content":"verify","url":"https://platform.example.test","autoRouteAfterReview":false}"#,
        )
        .expect("write test config");
        let previous = std::env::var_os("LOOM_VSEFM_CONFIG");
        std::env::set_var("LOOM_VSEFM_CONFIG", &config_path);

        let resume = RouteAction::done("approved");
        let routed =
            maybe_auto_route_after_review("/tmp/project", "delivery-1", "phase-1", resume.clone());
        assert_eq!(routed, resume);

        restore_env(previous);
        let _ = std::fs::remove_file(config_path);
    }

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().expect("V-SEFM environment lock")
    }

    fn restore_env(previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            std::env::set_var("LOOM_VSEFM_CONFIG", value);
        } else {
            std::env::remove_var("LOOM_VSEFM_CONFIG");
        }
    }
}
