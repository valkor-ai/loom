use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use delivery_core::{
    apply_delivery_index, current_phase, loom_home, loom_runtime_home, DomainDispatcher,
    FileSubmitInput, LoomMcpActionResult, LoomMcpAutoRunnableResult, LoomMcpDoneResult,
    LoomMcpFailure, LoomMcpFailureResult, LoomMcpRepairableErrorResult, LoomMcpUserGateResult,
    ProjectStatus, RouteAction, RouteActionKind, TransitionEngine, TransitionStore,
    VsefmVerificationNext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VsefmVerificationResolveInput {
    pub project_root: String,
    pub verification_id: String,
    pub decision: VsefmVerificationResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VsefmVerificationResolution {
    Accept,
    Repair,
    ManualReview,
    ApproveOverride,
    RequestChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VsefmVerificationCandidate {
    pub status: VsefmVerificationStatus,
    #[serde(default)]
    pub checks: Vec<VsefmCheckResult>,
    #[serde(default)]
    pub blocking_failures: Vec<VsefmBlockingFailure>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub unknown_checks: Vec<VsefmUnknownCheck>,
    #[serde(default)]
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VsefmVerificationStatus {
    Pass,
    Blocked,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VsefmCheckResult {
    pub check_id: String,
    pub category: String,
    pub rule: String,
    pub status: VsefmCheckStatus,
    pub input: String,
    pub expected: String,
    pub observed: String,
    pub evidence: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VsefmCheckStatus {
    Pass,
    Fail,
    Unknown,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VsefmBlockingFailure {
    pub category: String,
    pub rule: String,
    pub severity: String,
    pub evidence: String,
    pub reproduction: String,
    pub expected: String,
    pub observed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VsefmUnknownCheck {
    pub check_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VsefmRepairCandidate {
    pub status: VsefmRepairStatus,
    pub summary: String,
    #[serde(default)]
    pub resolved_failure_refs: Vec<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub verification_commands: Vec<String>,
    #[serde(default)]
    pub remaining_findings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VsefmRepairStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VsefmDecision {
    #[serde(alias = "1")]
    Required,
    #[serde(alias = "2")]
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
        Some((delivery, phase)) => match pending_onboarding_context(phase.next_action.as_ref()) {
            Some((resume_action, trigger)) => (
                Some(delivery.delivery_id),
                Some(phase.phase_id),
                resume_action,
                trigger,
            ),
            None => (
                Some(delivery.delivery_id),
                None,
                None,
                "explicit".to_string(),
            ),
        },
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
        accepted_responses: vec!["1".to_string(), "2".to_string()],
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

fn pending_onboarding_context(
    action: Option<&RouteAction>,
) -> Option<(Option<RouteAction>, String)> {
    let action = action?;
    if action.kind != RouteActionKind::VsefmOnboarding {
        return None;
    }
    let trigger = action
        .details
        .as_ref()
        .and_then(|details| details.get("trigger"))
        .and_then(Value::as_str)
        .unwrap_or("explicit")
        .to_string();
    if trigger != "review" {
        return None;
    }
    let resume_action = resume_action_from_onboarding(action);
    Some((resume_action, trigger))
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
            "Present the V-SEFM onboarding content and wait for the user's choice: 1 means start verification and 2 means defer verification. Then call loom.verify with decision=required for 1 or decision=deferred for 2. Loom opens the configured platform only for choice 1 when the local appkey is absent, records a warning if browser launch fails, and resumes immediately without waiting for an external V-SEFM result.",
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

    if decision == VsefmDecision::Required && app_key.as_ref().is_some_and(|state| state.present) {
        return start_local_verification(
            project_root,
            delivery_id,
            phase_id,
            action,
            config,
            warnings,
        );
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

const VSEFM_CHECK_IDS: &[&str] = &[
    "BUSINESS-INTENT",
    "AUTH-HORIZONTAL",
    "AUTH-VERTICAL",
    "TENANT-ISOLATION",
    "STATE-MACHINE",
    "IDEMPOTENCY",
    "CONCURRENCY",
    "TRANSACTION",
    "DATA-INTEGRITY",
    "API-COMPATIBILITY",
    "ERROR-RECOVERY",
    "SECURITY-BOUNDARY",
    "RETRY-TIMEOUT-RATE-LIMIT",
    "OBSERVABILITY-EVIDENCE",
    "REGRESSION-COMPATIBILITY",
    "PERFORMANCE-CAPACITY",
];

fn start_local_verification(
    project_root: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    action: &RouteAction,
    config: &VsefmConfig,
    warnings: Vec<String>,
) -> LoomMcpActionResult {
    let store = FileTransitionStore;
    let status = match store.load_status(project_root) {
        Ok(status) => status,
        Err(error) => return failed(project_root, "VSEFM_STATE_UNAVAILABLE", error.to_string()),
    };
    let effective_delivery_id = delivery_id
        .map(str::to_string)
        .or(status.last_completed_delivery_id.clone());
    let trigger = action
        .details
        .as_ref()
        .and_then(|details| details.get("trigger"))
        .and_then(Value::as_str)
        .unwrap_or("explicit")
        .to_string();
    let verification_id = format!("vsefm_verify_{}", state::store::now_millis());
    let session_dir = Path::new(project_root)
        .join(".loom")
        .join("verification")
        .join("sessions")
        .join(&verification_id);
    if let Err(error) = std::fs::create_dir_all(&session_dir) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    let scope = if trigger == "review" {
        "current_phase"
    } else {
        "completed_phases"
    };
    let subject = match build_verification_subject(
        project_root,
        effective_delivery_id.as_deref(),
        phase_id,
        scope,
    ) {
        Ok(subject) => subject,
        Err(error) => return failed(project_root, "VSEFM_SCOPE_BUILD_FAILED", error),
    };
    let subject_path = session_dir.join("subject.json");
    if let Err(error) = state::store::write_json_atomic(&subject_path, &subject) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    let prompt_ref = match verification_prompt_path() {
        Ok(path) => path,
        Err(error) => return failed(project_root, "VSEFM_PROMPT_UNAVAILABLE", error),
    };
    let result_file = format!(".loom/agent-writable/{verification_id}/verification-result.json");
    let request_file = format!(".loom/verification/sessions/{verification_id}/request.json");
    let subject_ref = format!(".loom/verification/sessions/{verification_id}/subject.json");
    let request_id = verification_id.clone();
    let request = verification_request(
        &verification_id,
        &trigger,
        effective_delivery_id.as_deref(),
        phase_id,
        scope,
        &prompt_ref,
        &subject_ref,
        &result_file,
        &subject,
    );
    let stored = match state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id,
            request_kind: "vsefm_local_verification".to_string(),
            request_file: Some(request_file),
            delivery_id: effective_delivery_id.clone(),
            phase_id: phase_id.map(str::to_string),
            root: request,
        },
    ) {
        Ok(stored) => stored,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_REQUEST_WRITE_FAILED",
                error.to_string(),
            )
        }
    };
    let session = json!({
        "schemaVersion": "1.0",
        "verificationId": verification_id,
        "trigger": trigger,
        "deliveryId": effective_delivery_id,
        "phaseId": phase_id,
        "scope": scope,
        "subjectRef": subject_ref,
        "promptRef": prompt_ref,
        "requestRef": stored.request_ref,
        "resultFile": result_file,
        "resumeAction": action.details.as_ref().and_then(|details| details.get("resumeAction")).cloned(),
        "status": "awaiting_agent",
        "attempt": 1,
        "createdAt": state::store::now_string()
    });
    if let Err(error) = state::store::write_json_atomic(&session_dir.join("state.json"), &session) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    let now = state::store::now_string();
    if let Err(error) = write_record(
        project_root,
        &VsefmRecord {
            schema_version: 1,
            delivery_id: effective_delivery_id.clone(),
            phase_id: phase_id.map(str::to_string),
            decision: Some(VsefmDecision::Required),
            status: "local_verification_pending".to_string(),
            trigger: trigger.clone(),
            config_url: config.url.clone(),
            app_key_present: true,
            url_opened: false,
            warning: warnings.first().cloned(),
            created_at: now.clone(),
            updated_at: now,
        },
    ) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
    }
    if let (Some(delivery_id), Some(phase_id)) = (effective_delivery_id.as_deref(), phase_id) {
        if let Err(error) = persist_vsefm_verification_action(
            project_root,
            delivery_id,
            phase_id,
            &verification_id,
            &stored.request_ref,
            &trigger,
        ) {
            return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
        }
    }
    let inspected = match state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: stored.request_ref.clone(),
    }) {
        Ok(inspected) => inspected,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_REQUEST_INSPECT_FAILED",
                error.to_string(),
            )
        }
    };
    let next = VsefmVerificationNext {
        verification_id,
        request_ref: stored.request_ref,
        result_file,
        prompt_ref: prompt_ref.clone(),
        subject_ref,
        scope: scope.to_string(),
        read_groups: inspected.read_groups,
        submit_tool: "loom.vsefmVerificationAcceptFile".to_string(),
        allowed_paths: subject
            .get("changedFiles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("path").and_then(Value::as_str))
            .map(str::to_string)
            .collect(),
        protected_paths: vec![".loom".to_string(), prompt_ref.clone()],
    };
    LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
        project_root.to_string(),
        delivery_core::LoomMcpNextAction::RunVsefmVerification(next),
    ))
    .with_warnings(warnings)
}

fn verification_prompt_path() -> Result<String, String> {
    let path = loom_runtime_home()
        .ok()
        .map(|home| home.join(CONFIG_RELATIVE_PATH.replace("v-sefm.json", "sefm-verify.md")))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join("plugins/shared/loom/references/verification/sefm-verify.md")
        });
    if !path.is_file() {
        return Err(format!(
            "verification prompt does not exist: {}",
            path.display()
        ));
    }
    Ok(path.to_string_lossy().to_string())
}

pub fn resume_vsefm_route_action(
    project_root: &str,
    _delivery_id: &str,
    _phase_id: &str,
    action: &RouteAction,
) -> LoomMcpActionResult {
    let verification_id = action
        .details
        .as_ref()
        .and_then(|details| details.get("verificationId"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if verification_id.is_empty() {
        return failed(
            project_root,
            "VSEFM_RESUME_CONTEXT_MISSING",
            "V-SEFM route action is missing verificationId.",
        );
    }
    let session_path = Path::new(project_root)
        .join(".loom/verification/sessions")
        .join(verification_id)
        .join("state.json");
    let session = match state::store::read_json_value(&session_path) {
        Ok(session) => session,
        Err(error) => return failed(project_root, "VSEFM_SESSION_NOT_FOUND", error.to_string()),
    };
    let status = session
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match (action.kind.clone(), status) {
        (RouteActionKind::VsefmVerification, "awaiting_agent")
        | (RouteActionKind::VsefmVerification, "awaiting_user_resolution")
        | (RouteActionKind::VsefmVerification, "manual_review")
        | (RouteActionKind::VsefmRepair, "repairing")
        | (RouteActionKind::VsefmResultGate, "awaiting_user_resolution")
        | (RouteActionKind::VsefmResultGate, "manual_review") => {
            resume_vsefm_session_state(project_root, &session, &action.kind)
        }
        (_, "completed") | (_, "completed_with_override") => {
            LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: project_root.to_string(),
                summary: "V-SEFM local verification is already complete.".to_string(),
                details: Some(json!({"verificationId": verification_id, "status": status})),
                warnings: vec![],
            })
        }
        _ => failed(
            project_root,
            "VSEFM_RESUME_STATE_INVALID",
            format!("cannot resume V-SEFM action in session state {status}"),
        ),
    }
}

pub fn resume_unattached_vsefm(project_root: &str) -> Option<LoomMcpActionResult> {
    let sessions_dir = Path::new(project_root).join(".loom/verification/sessions");
    let mut pending = std::fs::read_dir(sessions_dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let session_path = entry.path().join("state.json");
            let session = state::store::read_json_value(&session_path).ok()?;
            let status = session.get("status").and_then(Value::as_str)?;
            if !matches!(
                status,
                "awaiting_agent" | "awaiting_user_resolution" | "manual_review" | "repairing"
            ) {
                return None;
            }
            let updated_at = session
                .get("updatedAt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some((updated_at, session))
        })
        .collect::<Vec<_>>();
    pending.sort_by(|left, right| left.0.cmp(&right.0));
    let (_, session) = pending.pop()?;
    let action_kind = if session.get("status").and_then(Value::as_str) == Some("repairing") {
        RouteActionKind::VsefmRepair
    } else {
        RouteActionKind::VsefmVerification
    };
    Some(resume_vsefm_session_state(
        project_root,
        &session,
        &action_kind,
    ))
}

fn resume_vsefm_session_state(
    project_root: &str,
    session: &Value,
    action_kind: &RouteActionKind,
) -> LoomMcpActionResult {
    let status = session
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match (action_kind, status) {
        (RouteActionKind::VsefmVerification, "awaiting_agent") => {
            verification_next_from_session(project_root, session)
        }
        (RouteActionKind::VsefmVerification, "awaiting_user_resolution")
        | (RouteActionKind::VsefmResultGate, "awaiting_user_resolution") => {
            let verification_id = session
                .get("verificationId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let result = read_vsefm_result(Path::new(project_root), session);
            let result_ref = session
                .get("resultRef")
                .and_then(Value::as_str)
                .unwrap_or_default();
            vsefm_result_gate(
                project_root,
                verification_id,
                &result,
                result_ref,
                session.get("deliveryId").and_then(Value::as_str),
                session.get("phaseId").and_then(Value::as_str),
            )
        }
        (RouteActionKind::VsefmVerification, "manual_review")
        | (RouteActionKind::VsefmResultGate, "manual_review") => {
            vsefm_manual_review_gate(project_root, session)
        }
        (RouteActionKind::VsefmRepair, "repairing") => {
            repair_next_from_session(project_root, session)
        }
        _ => failed(
            project_root,
            "VSEFM_RESUME_STATE_INVALID",
            format!("cannot resume V-SEFM action in session state {status}"),
        ),
    }
}

fn verification_next_from_session(project_root: &str, session: &Value) -> LoomMcpActionResult {
    let request_ref = session
        .get("requestRef")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if request_ref.is_empty() || verification_id.is_empty() {
        return failed(
            project_root,
            "VSEFM_RESUME_CONTEXT_MISSING",
            "V-SEFM verification session is missing requestRef or verificationId.",
        );
    }
    let inspected = match state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    }) {
        Ok(inspected) => inspected,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_REQUEST_INSPECT_FAILED",
                error.to_string(),
            )
        }
    };
    let subject = session
        .get("subjectRef")
        .and_then(Value::as_str)
        .and_then(|reference| {
            state::paths::from_project_relative(Path::new(project_root), reference).ok()
        })
        .and_then(|path| state::store::read_json_value(&path).ok())
        .unwrap_or_else(|| json!({}));
    LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
        project_root.to_string(),
        delivery_core::LoomMcpNextAction::RunVsefmVerification(VsefmVerificationNext {
            verification_id: verification_id.to_string(),
            request_ref: request_ref.to_string(),
            result_file: session
                .get("resultFile")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            prompt_ref: session
                .get("promptRef")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            subject_ref: session
                .get("subjectRef")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            scope: session
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            read_groups: inspected.read_groups,
            submit_tool: "loom.vsefmVerificationAcceptFile".to_string(),
            allowed_paths: subject
                .get("changedFiles")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect(),
            protected_paths: vec![
                ".loom".to_string(),
                session
                    .get("promptRef")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ],
        }),
    ))
}

fn repair_next_from_session(project_root: &str, session: &Value) -> LoomMcpActionResult {
    let request_ref = session
        .get("repairRequestRef")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let repair_id = session
        .get("repairId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if request_ref.is_empty() || repair_id.is_empty() || verification_id.is_empty() {
        return failed(
            project_root,
            "VSEFM_RESUME_CONTEXT_MISSING",
            "V-SEFM repair session is missing repairId, verificationId, or requestRef.",
        );
    }
    let inspected = match state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    }) {
        Ok(inspected) => inspected,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_REPAIR_REQUEST_INSPECT_FAILED",
                error.to_string(),
            )
        }
    };
    let allowed_paths = session
        .get("subjectRef")
        .and_then(Value::as_str)
        .and_then(|reference| {
            state::paths::from_project_relative(Path::new(project_root), reference).ok()
        })
        .and_then(|path| state::store::read_json_value(&path).ok())
        .and_then(|subject| subject.get("changedFiles").cloned())
        .and_then(|files| files.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.get("path").and_then(Value::as_str).map(str::to_string))
        .collect();
    LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
        project_root.to_string(),
        delivery_core::LoomMcpNextAction::RunVsefmRepair(delivery_core::VsefmRepairNext {
            repair_id: repair_id.to_string(),
            verification_id: verification_id.to_string(),
            request_ref: request_ref.to_string(),
            result_file: session
                .get("repairResultFile")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            read_groups: inspected.read_groups,
            submit_tool: "loom.vsefmRepairAcceptFile".to_string(),
            allowed_paths,
            protected_paths: vec![
                ".loom".to_string(),
                "plugins/shared/loom/references/verification/sefm-verify.md".to_string(),
            ],
        }),
    ))
}

fn build_verification_subject(
    project_root: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    scope: &str,
) -> Result<Value, String> {
    let root = Path::new(project_root);
    let mut phases = Vec::new();
    let mut changed_files = BTreeSet::new();
    let mut source_refs = Vec::new();
    if let Some(delivery_id) = delivery_id {
        let delivery = FileTransitionStore
            .load_delivery_index(project_root, delivery_id)
            .map_err(|error| error.to_string())?;
        let selected = if scope == "current_phase" {
            phase_id
                .map(|phase| vec![phase.to_string()])
                .unwrap_or_default()
        } else {
            let active_index = delivery
                .phases
                .iter()
                .position(|phase| phase.phase_id == delivery.active_phase_id)
                .unwrap_or(delivery.phases.len());
            let include_active = matches!(
                delivery.status,
                delivery_core::DeliveryLifecycleStatus::Completed
                    | delivery_core::DeliveryLifecycleStatus::CompletedWithOverride
            ) || delivery
                .phases
                .get(active_index)
                .and_then(|phase| phase.next_action.as_ref())
                .is_some_and(|action| {
                    matches!(
                        action.kind,
                        RouteActionKind::ContinueToNextPhase | RouteActionKind::Done
                    )
                });
            delivery
                .phases
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    *index < active_index || (*index == active_index && include_active)
                })
                .map(|(_, phase)| phase.phase_id.clone())
                .collect()
        };
        for phase in selected {
            phases.push(phase.clone());
            let phase_root = root
                .join(".loom")
                .join("deliveries")
                .join(delivery_id)
                .join("tasks")
                .join(&phase)
                .join("results");
            collect_changed_files(&phase_root, root, &mut changed_files);
            for reference in [
                format!(".loom/deliveries/{delivery_id}/contracts/planning/{phase}/pgc.json"),
                format!(".loom/deliveries/{delivery_id}/contracts/architecture/{phase}/aac.json"),
            ] {
                if root.join(&reference).is_file() {
                    source_refs.push(reference);
                }
            }
        }
    }
    let files = changed_files
        .into_iter()
        .filter_map(|path| {
            let absolute = root.join(&path);
            let metadata = std::fs::metadata(&absolute).ok()?;
            let bytes = std::fs::read(&absolute).ok()?;
            let hash = Sha256::digest(bytes);
            Some(json!({
                "path": path,
                "sha256": format!("{hash:x}"),
                "bytes": metadata.len()
            }))
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schemaVersion": "1.0",
        "scope": scope,
        "deliveryId": delivery_id,
        "phaseIds": phases,
        "requirementRefs": source_refs,
        "changedFiles": files,
        "checkIds": VSEFM_CHECK_IDS,
        "generatedAt": state::store::now_string()
    }))
}

fn collect_changed_files(dir: &Path, root: &Path, output: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_changed_files(&path, root, output);
            continue;
        }
        let Ok(value) = state::store::read_json_value(&path) else {
            continue;
        };
        if let Some(files) = value.get("changedFiles").and_then(Value::as_array) {
            for file in files.iter().filter_map(Value::as_str) {
                if is_safe_verification_path(file) && root.join(file).is_file() {
                    output.insert(file.to_string());
                }
            }
        }
    }
}

fn is_safe_verification_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        && !path.starts_with(".loom")
        && !path.to_string_lossy().contains(".env")
        && !path.to_string_lossy().contains("secret")
        && !path.to_string_lossy().contains("appkey")
}

fn verification_request(
    verification_id: &str,
    trigger: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    scope: &str,
    prompt_ref: &str,
    subject_ref: &str,
    result_file: &str,
    subject: &Value,
) -> Value {
    let instruction = json!({
        "role": "software_delivery_verifier",
        "objective": "Verify the declared delivery subject against sefm-verify.md without modifying product or Loom files.",
        "steps": [
            "Read verification_execution_core, verification_prompt, verification_subject, and verification_result_contract.",
            "Read sefm-verify.md from promptRef.",
            "Read only subject.changedFiles and the declared accepted artifact references.",
            "Evaluate every requested checkId and record concrete input, expected, observed, evidence, and timestamp.",
            "Write the result candidate and submit it with loom.vsefmVerificationAcceptFile."
        ],
        "hardBlockingRules": [
            "AUTH-HORIZONTAL, AUTH-VERTICAL, TENANT-ISOLATION, IDEMPOTENCY, STATE-MACHINE, and TRANSACTION failures require status=blocked.",
            "Never claim pass without reproducible evidence.",
            "Use unknown when the subject or environment does not establish a conclusion."
        ],
        "completionBarrier": {
            "resultFile": result_file,
            "submitTool": "loom.vsefmVerificationAcceptFile"
        },
        "boundaryRules": [
            "Read-only verification; do not edit product files.",
            "Do not edit .loom canonical artifacts.",
            "Do not read secrets or appkey.",
            "Do not run unbounded servers, watchers, or interactive commands."
        ]
    });
    json!({
        "schemaVersion": "1.0",
        "requestType": "vsefm_local_verification",
        "verificationId": verification_id,
        "source": {
            "trigger": trigger,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "scope": scope
        },
        "agentInstruction": instruction,
        "prompt": {
            "ref": prompt_ref,
            "requiredCheckIds": VSEFM_CHECK_IDS
        },
        "subject": subject,
        "outputContract": {
            "artifactKind": "vsefm_verification_result",
            "writeMode": "single_json",
            "submitTool": "loom.vsefmVerificationAcceptFile",
            "resultFile": result_file,
            "writeTargets": [{
                "targetId": "result",
                "path": result_file,
                "required": true,
                "description": "Write the Agent-owned V-SEFM verification result candidate."
            }],
            "resultTemplate": {
                "status": "inconclusive",
                "checks": [],
                "blocking_failures": [],
                "warnings": [],
                "unknown_checks": [],
                "recommended_actions": []
            }
        },
        "requestReadPlan": {
            "groups": [
                delivery_core::ReadGroupRef::new("verification_execution_core", 1, vec![
                    "agentInstruction", "source", "completionBarrier", "boundaryRules"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/execution")),
                delivery_core::ReadGroupRef::new("verification_prompt", 2, vec![
                    "prompt", "prompt.ref", "prompt.requiredCheckIds"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/prompt")),
                delivery_core::ReadGroupRef::new("verification_subject", 3, vec![
                    "subject", "subject.scope", "subject.phaseIds", "subject.requirementRefs", "subject.changedFiles"
                ].into_iter().map(str::to_string).collect(), subject_ref),
                delivery_core::ReadGroupRef::new("verification_result_contract", 4, vec![
                    "outputContract"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/result-contract"))
            ]
        }
    })
}

pub fn accept_vsefm_verification_file(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
) -> LoomMcpActionResult {
    let Some(target) = authorized
        .targets
        .iter()
        .find(|target| target.target_id == "result")
    else {
        return failed(
            &input.project_root,
            "VSEFM_RESULT_TARGET_MISSING",
            "V-SEFM result target is missing.",
        );
    };
    let root = Path::new(&input.project_root);
    let candidate_path = match state::paths::from_project_relative(root, &target.path) {
        Ok(path) => path,
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_RESULT_PATH_INVALID",
                error.to_string(),
            )
        }
    };
    let raw = match state::store::read_json_value(&candidate_path) {
        Ok(value) => value,
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_RESULT_READ_FAILED",
                error.to_string(),
            )
        }
    };
    let candidate: VsefmVerificationCandidate = match serde_json::from_value(raw) {
        Ok(candidate) => candidate,
        Err(error) => {
            return vsefm_result_repair(
                input,
                authorized,
                format!("V-SEFM result does not match sefm-verify.md: {error}"),
            )
        }
    };
    let issues = validate_vsefm_candidate(&candidate);
    if !issues.is_empty() {
        return LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
            project_root: input.project_root.clone(),
            stop_allowed: false,
            target_file: target.path.clone(),
            target_ids: vec![target.target_id.clone()],
            issues,
            resubmit_tool: "loom.vsefmVerificationAcceptFile".to_string(),
            fix_scope: Some("Edit only the Agent-owned V-SEFM result candidate.".to_string()),
            read_groups: authorized.read_groups.clone(),
            agent_instruction: delivery_core::repairable_error_agent_instruction(
                "loom.vsefmVerificationAcceptFile",
            ),
        });
    }
    let verification_id = authorized.request_id.clone();
    let session_path = root
        .join(".loom/verification/sessions")
        .join(&verification_id)
        .join("state.json");
    let session = match state::store::read_json_value(&session_path) {
        Ok(session) => session,
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_SESSION_NOT_FOUND",
                error.to_string(),
            )
        }
    };
    if session.get("status").and_then(Value::as_str) != Some("awaiting_agent") {
        return failed(
            &input.project_root,
            "VSEFM_SUBMIT_STATE_INVALID",
            "V-SEFM verification result is not awaiting an Agent result.",
        );
    }
    let result = canonical_vsefm_result(&verification_id, &candidate, &session);
    let result_path = root
        .join(".loom/verification/results")
        .join(format!("{verification_id}.json"));
    if let Some(parent) = result_path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return failed(
                &input.project_root,
                "VSEFM_STATE_WRITE_FAILED",
                error.to_string(),
            );
        }
    }
    if let Err(error) = state::store::write_json_atomic(&result_path, &result) {
        return failed(
            &input.project_root,
            "VSEFM_STATE_WRITE_FAILED",
            error.to_string(),
        );
    }
    let mut updated_session = session.clone();
    if let Some(object) = updated_session.as_object_mut() {
        object.insert("status".to_string(), json!("awaiting_user_resolution"));
        object.insert(
            "resultRef".to_string(),
            json!(format!(".loom/verification/results/{verification_id}.json")),
        );
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    if let Err(error) = state::store::write_json_atomic(&session_path, &updated_session) {
        return failed(
            &input.project_root,
            "VSEFM_STATE_WRITE_FAILED",
            error.to_string(),
        );
    }
    let result_ref = format!(".loom/verification/results/{verification_id}.json");
    let delivery_id = session
        .get("deliveryId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let phase_id = session
        .get("phaseId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let gate = vsefm_result_gate(
        &input.project_root,
        &verification_id,
        &result,
        &result_ref,
        delivery_id.as_deref(),
        phase_id.as_deref(),
    );
    if let (Some(delivery_id), Some(phase_id)) = (delivery_id, phase_id) {
        if let Err(error) = persist_vsefm_gate(&input.project_root, &delivery_id, &phase_id, &gate)
        {
            return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error);
        }
    }
    gate
}

fn vsefm_result_repair(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
    message: String,
) -> LoomMcpActionResult {
    LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
        project_root: input.project_root.clone(),
        stop_allowed: false,
        target_file: authorized
            .targets
            .iter()
            .find(|target| target.target_id == "result")
            .map(|target| target.path.clone())
            .unwrap_or_default(),
        target_ids: authorized
            .targets
            .iter()
            .filter(|target| target.target_id == "result")
            .map(|target| target.target_id.clone())
            .collect(),
        issues: vec![vsefm_issue("VSEFM_RESULT_SCHEMA_INVALID", "$", &message)],
        resubmit_tool: "loom.vsefmVerificationAcceptFile".to_string(),
        fix_scope: Some("Edit only the Agent-owned V-SEFM result candidate.".to_string()),
        read_groups: authorized.read_groups.clone(),
        agent_instruction: delivery_core::repairable_error_agent_instruction(
            "loom.vsefmVerificationAcceptFile",
        ),
    })
}

pub fn resolve_vsefm_verification<D>(
    input: VsefmVerificationResolveInput,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    let root = Path::new(&input.project_root);
    let session_path = root
        .join(".loom/verification/sessions")
        .join(&input.verification_id)
        .join("state.json");
    let session = match state::store::read_json_value(&session_path) {
        Ok(session) => session,
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_SESSION_NOT_FOUND",
                error.to_string(),
            )
        }
    };
    let status = session
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if status == "manual_review" {
        return match input.decision {
            VsefmVerificationResolution::ApproveOverride => finish_vsefm_session(
                &input.project_root,
                &session,
                "completed_with_override",
                dispatcher,
            ),
            VsefmVerificationResolution::RequestChanges => {
                materialize_vsefm_repair(&input.project_root, &session)
            }
            _ => failed(
                &input.project_root,
                "VSEFM_RESOLUTION_INVALID",
                "Manual V-SEFM review requires approve_override or request_changes.",
            ),
        };
    }
    if status != "awaiting_user_resolution" {
        return failed(
            &input.project_root,
            "VSEFM_RESOLUTION_INVALID",
            "V-SEFM result is not awaiting user resolution.",
        );
    }
    match input.decision {
        VsefmVerificationResolution::Accept => {
            let result = read_vsefm_result(root, &session);
            if result.get("status").and_then(Value::as_str) != Some("pass") {
                return failed(
                    &input.project_root,
                    "VSEFM_RESOLUTION_INVALID",
                    "Only a passing result can be accepted.",
                );
            }
            finish_vsefm_session(&input.project_root, &session, "completed", dispatcher)
        }
        VsefmVerificationResolution::Repair => finish_vsefm_session(
            &input.project_root,
            &session,
            "repair_requested",
            dispatcher,
        ),
        VsefmVerificationResolution::ManualReview => {
            finish_vsefm_session(&input.project_root, &session, "manual_review", dispatcher)
        }
        VsefmVerificationResolution::ApproveOverride
        | VsefmVerificationResolution::RequestChanges => failed(
            &input.project_root,
            "VSEFM_RESOLUTION_INVALID",
            "The selected manual review resolution is not valid for this verification result.",
        ),
    }
}

fn validate_vsefm_candidate(
    candidate: &VsefmVerificationCandidate,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut check_statuses = std::collections::BTreeMap::new();
    let hard_blockers = [
        "AUTH-HORIZONTAL",
        "AUTH-VERTICAL",
        "TENANT-ISOLATION",
        "IDEMPOTENCY",
        "STATE-MACHINE",
        "TRANSACTION",
    ];
    for check in &candidate.checks {
        if !VSEFM_CHECK_IDS.contains(&check.check_id.as_str()) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_ID_INVALID",
                "checks[].check_id",
                "Check id is not in the canonical V-SEFM catalog.",
            ));
        }
        if !seen.insert(check.check_id.clone()) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_DUPLICATE",
                "checks[].check_id",
                "Each canonical check id must appear once.",
            ));
        }
        check_statuses.insert(check.check_id.clone(), check.status);
        if check.input.trim().is_empty()
            || check.expected.trim().is_empty()
            || check.observed.trim().is_empty()
        {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_CONTEXT_REQUIRED",
                "checks[]",
                "Each check requires non-empty input, expected, and observed values.",
            ));
        }
        if check.evidence.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_EVIDENCE_REQUIRED",
                "checks[].evidence",
                "Every check requires evidence.",
            ));
        }
        if hard_blockers.contains(&check.check_id.as_str())
            && check.status == VsefmCheckStatus::Fail
            && candidate.status != VsefmVerificationStatus::Blocked
        {
            issues.push(vsefm_issue(
                "VSEFM_HARD_BLOCKER_STATUS_INVALID",
                "status",
                "A hard-blocking check failure requires status=blocked.",
            ));
        }
    }
    let missing = VSEFM_CHECK_IDS
        .iter()
        .filter(|check_id| !seen.contains(**check_id))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        issues.push(vsefm_issue(
            "VSEFM_CHECK_COVERAGE_INCOMPLETE",
            "checks",
            &format!("Missing canonical checks: {}", missing.join(", ")),
        ));
    }
    if candidate.status == VsefmVerificationStatus::Blocked
        && candidate.blocking_failures.is_empty()
    {
        issues.push(vsefm_issue(
            "VSEFM_BLOCKING_FAILURE_REQUIRED",
            "blocking_failures",
            "Blocked verification requires a blocking failure.",
        ));
    }
    let mut unknown_ids = BTreeSet::new();
    for unknown in &candidate.unknown_checks {
        if !VSEFM_CHECK_IDS.contains(&unknown.check_id.as_str()) {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_ID_INVALID",
                "unknown_checks[].check_id",
                "Unknown check id is not in the canonical V-SEFM catalog.",
            ));
        }
        if !unknown_ids.insert(unknown.check_id.clone()) || unknown.reason.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_INVALID",
                "unknown_checks[]",
                "Each unknown check must be unique and explain why it could not be established.",
            ));
        }
        if check_statuses.get(&unknown.check_id) != Some(&VsefmCheckStatus::Unknown) {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_STATUS_INVALID",
                "unknown_checks[].check_id",
                "unknown_checks must reference a check whose status is unknown.",
            ));
        }
    }
    for check in &candidate.checks {
        if check.status == VsefmCheckStatus::Unknown && !unknown_ids.contains(&check.check_id) {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_MISSING",
                "unknown_checks",
                "Every unknown check must include an explanatory unknown_checks entry.",
            ));
        }
    }
    for failure in &candidate.blocking_failures {
        if !VSEFM_CHECK_IDS.contains(&failure.rule.as_str()) {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_RULE_INVALID",
                "blocking_failures[].rule",
                "Blocking failure rule must reference a canonical check id.",
            ));
        } else if check_statuses.get(&failure.rule) != Some(&VsefmCheckStatus::Fail) {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_RULE_UNSUPPORTED",
                "blocking_failures[].rule",
                "Blocking failure rule must reference a failed check.",
            ));
        }
    }
    if candidate.status == VsefmVerificationStatus::Pass
        && candidate.checks.iter().any(|check| {
            matches!(
                check.status,
                VsefmCheckStatus::Fail | VsefmCheckStatus::Unknown
            )
        })
    {
        issues.push(vsefm_issue(
            "VSEFM_STATUS_INCONSISTENT",
            "status",
            "A pass result cannot contain failed or unknown checks.",
        ));
    }
    issues
}

fn vsefm_issue(code: &str, field_path: &str, message: &str) -> delivery_core::RepairIssue {
    delivery_core::RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("result".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

fn canonical_vsefm_result(
    verification_id: &str,
    candidate: &VsefmVerificationCandidate,
    session: &Value,
) -> Value {
    let passed_checks = candidate
        .checks
        .iter()
        .filter(|check| check.status == VsefmCheckStatus::Pass)
        .count();
    json!({
        "schema_version": "1.0",
        "artifact_id": verification_id,
        "verification_id": verification_id,
        "status": candidate.status,
        "checks": candidate.checks,
        "blocking_failures": candidate.blocking_failures,
        "warnings": candidate.warnings,
        "unknown_checks": candidate.unknown_checks,
        "recommended_actions": candidate.recommended_actions,
        "passed_checks": passed_checks,
        "warning_count": candidate.warnings.len(),
        "unknown_count": candidate.unknown_checks.len(),
        "source": {
            "delivery_id": session.get("deliveryId"),
            "phase_id": session.get("phaseId"),
            "scope": session.get("scope"),
            "subject_ref": session.get("subjectRef"),
            "prompt_ref": session.get("promptRef")
        },
        "created_at": state::store::now_string()
    })
}

fn read_vsefm_result(root: &Path, session: &Value) -> Value {
    session
        .get("resultRef")
        .and_then(Value::as_str)
        .and_then(|reference| state::paths::from_project_relative(root, reference).ok())
        .and_then(|path| state::store::read_json_value(&path).ok())
        .unwrap_or_else(|| json!({}))
}

fn finish_vsefm_session<D>(
    project_root: &str,
    session: &Value,
    status: &str,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or("verification");
    let path = Path::new(project_root)
        .join(".loom/verification/sessions")
        .join(verification_id)
        .join("state.json");
    let mut updated = session.clone();
    if let Some(object) = updated.as_object_mut() {
        object.insert("status".to_string(), json!(status));
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    if let Err(error) = state::store::write_json_atomic(&path, &updated) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    match status {
        "repair_requested" => materialize_vsefm_repair(project_root, session),
        "manual_review" => vsefm_manual_review_gate(project_root, session),
        "completed" | "completed_with_override" => {
            resume_after_vsefm(project_root, session, status, dispatcher)
        }
        _ => failed(
            project_root,
            "VSEFM_STATE_INVALID",
            format!("unsupported V-SEFM session completion state: {status}"),
        ),
    }
}

fn vsefm_manual_review_gate(project_root: &str, session: &Value) -> LoomMcpActionResult {
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or("verification");
    let result_ref = session
        .get("resultRef")
        .and_then(Value::as_str)
        .unwrap_or(".loom/verification/results/unknown.json");
    let gate = LoomMcpActionResult::UserGate(
        LoomMcpUserGateResult::new(
            project_root.to_string(),
            format!(
                "V-SEFM verification requires manual review. Choose approve_override or request_changes.\nResult: {result_ref}"
            ),
            vec!["approve_override".to_string(), "request_changes".to_string()],
            None,
            session
                .get("deliveryId")
                .and_then(Value::as_str)
                .map(str::to_string),
            session
                .get("phaseId")
                .and_then(Value::as_str)
                .map(str::to_string),
            Some(json!({
                "kind": "vsefm_manual_review",
                "verificationId": verification_id,
                "resultRef": result_ref
            })),
        )
        .with_agent_instruction(
            "Present the V-SEFM findings and wait for the manual review decision. Then call loom.vsefmVerificationResolve with decision=approve_override or decision=request_changes. Do not call loom.continue or knowledge tools before the user chooses.",
        ),
    );
    if let (Some(delivery_id), Some(phase_id)) = (
        session.get("deliveryId").and_then(Value::as_str),
        session.get("phaseId").and_then(Value::as_str),
    ) {
        if let Err(error) = persist_vsefm_manual_review_gate(
            project_root,
            delivery_id,
            phase_id,
            verification_id,
            result_ref,
        ) {
            return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
        }
    }
    gate
}

fn resume_after_vsefm<D>(
    project_root: &str,
    session: &Value,
    status: &str,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher,
{
    let resume_action = session
        .get("resumeAction")
        .and_then(|value| serde_json::from_value::<RouteAction>(value.clone()).ok());
    let Some(resume_action) = resume_action else {
        return LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: project_root.to_string(),
            summary: "V-SEFM local verification completed.".to_string(),
            details: Some(json!({
                "verificationId": session.get("verificationId"),
                "status": status
            })),
            warnings: vec![],
        });
    };
    let Some(delivery_id) = session.get("deliveryId").and_then(Value::as_str) else {
        return LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: project_root.to_string(),
            summary: "V-SEFM local verification completed.".to_string(),
            details: Some(json!({
                "verificationId": session.get("verificationId"),
                "status": status
            })),
            warnings: vec![],
        });
    };
    let Some(phase_id) = session.get("phaseId").and_then(Value::as_str) else {
        return failed(
            project_root,
            "VSEFM_RESUME_CONTEXT_MISSING",
            "A routed V-SEFM verification is missing phaseId.",
        );
    };
    let store = FileTransitionStore;
    let mut project_status = match store.load_status(project_root) {
        Ok(status) => status,
        Err(error) => return failed(project_root, "VSEFM_STATE_UNAVAILABLE", error.to_string()),
    };
    let mut delivery = match store.load_delivery_index(project_root, delivery_id) {
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
            "VSEFM_RESUME_CONTEXT_MISSING",
            format!("phase {phase_id} does not exist in delivery {delivery_id}"),
        );
    };
    phase.next_action = Some(resume_action.clone());
    delivery.status = if resume_action.kind == RouteActionKind::Done {
        if status == "completed_with_override" {
            delivery_core::DeliveryLifecycleStatus::CompletedWithOverride
        } else {
            delivery_core::DeliveryLifecycleStatus::Completed
        }
    } else {
        delivery_core::DeliveryLifecycleStatus::Executing
    };
    delivery.updated_at = state::store::now_string();
    if let Err(error) = store.save_delivery_index(project_root, &delivery) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    apply_delivery_index(&mut project_status, &delivery);
    if let Err(error) = store.save_status(project_root, &project_status) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    if resume_action.kind == RouteActionKind::Done {
        return LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: project_root.to_string(),
            summary: "V-SEFM verification completed and Loom delivery resumed.".to_string(),
            details: Some(json!({
                "verificationId": session.get("verificationId"),
                "status": status
            })),
            warnings: vec![],
        });
    }
    TransitionEngine { store, dispatcher }
        .continue_current(delivery_core::OperationContext {
            project_root: project_root.to_string(),
        })
        .unwrap_or_else(|error| failed(project_root, "VSEFM_RESUME_FAILED", error.to_string()))
}

fn vsefm_result_gate(
    project_root: &str,
    verification_id: &str,
    result: &Value,
    result_ref: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
) -> LoomMcpActionResult {
    let status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("inconclusive");
    let choices = if status == "pass" {
        vec!["accept".to_string(), "manual_review".to_string()]
    } else {
        vec!["repair".to_string(), "manual_review".to_string()]
    };
    LoomMcpActionResult::UserGate(LoomMcpUserGateResult::new(
        project_root.to_string(),
        format!("V-SEFM 本地验证结果：{status}。请查看结果并选择后续处理方式。\n结果文件：{result_ref}"),
        choices,
        None,
        delivery_id.map(str::to_string),
        phase_id.map(str::to_string),
        Some(json!({"kind": "vsefm_result", "verificationId": verification_id, "resultRef": result_ref, "status": status, "blockingFailures": result.get("blocking_failures").cloned().unwrap_or_else(|| json!([])), "recommendedActions": result.get("recommended_actions").cloned().unwrap_or_else(|| json!([]))})),
    ).with_agent_instruction("展示 V-SEFM 验证结果摘要并等待用户选择。选择后调用 loom.vsefmVerificationResolve；不要调用 loom.continue、loom.inspectRequest 或知识工具。"))
}

fn materialize_vsefm_repair(project_root: &str, session: &Value) -> LoomMcpActionResult {
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or("verification")
        .to_string();
    let repair_id = format!("vsefm_repair_{}", state::store::now_millis());
    let session_dir = Path::new(project_root)
        .join(".loom/verification/sessions")
        .join(&verification_id);
    let result = session
        .get("resultRef")
        .and_then(Value::as_str)
        .and_then(|reference| {
            state::paths::from_project_relative(Path::new(project_root), reference).ok()
        })
        .and_then(|path| state::store::read_json_value(&path).ok())
        .unwrap_or_else(|| json!({}));
    let subject_ref = session
        .get("subjectRef")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let subject = state::paths::from_project_relative(Path::new(project_root), subject_ref)
        .ok()
        .and_then(|path| state::store::read_json_value(&path).ok())
        .unwrap_or_else(|| json!({}));
    let allowed_paths = subject
        .get("changedFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let result_file = format!(".loom/agent-writable/{repair_id}/vsefm-repair-result.json");
    let request_file = format!(".loom/verification/sessions/{verification_id}/repair-request.json");
    let request = json!({
        "schemaVersion": "1.0",
        "requestType": "vsefm_repair",
        "repairId": repair_id,
        "verificationId": verification_id,
        "source": {
            "resultRef": session.get("resultRef"),
            "subjectRef": subject_ref,
            "deliveryId": session.get("deliveryId"),
            "phaseId": session.get("phaseId")
        },
        "agentInstruction": {
            "objective": "Fix only the blocking V-SEFM findings in the declared changed files.",
            "findings": result.get("blocking_failures").cloned().unwrap_or_else(|| json!([])),
            "steps": [
                "Read repair_core and repair_result_contract.",
                "Read each blocking finding and the allowed source file.",
                "Modify only allowedPaths.",
                "Run bounded verification commands for the repaired findings.",
                "Write repair result and submit it with loom.vsefmRepairAcceptFile."
            ],
            "boundaryRules": [
                "Do not edit .loom canonical artifacts.",
                "Do not edit files outside allowedPaths.",
                "Do not claim a finding is resolved without verification evidence."
            ]
        },
        "allowedPaths": allowed_paths,
        "protectedPaths": [".loom", "plugins/shared/loom/references/verification/sefm-verify.md"],
        "outputContract": {
            "artifactKind": "vsefm_repair_result",
            "writeMode": "single_json",
            "submitTool": "loom.vsefmRepairAcceptFile",
            "resultFile": result_file,
            "writeTargets": [{"targetId": "result", "path": result_file, "required": true, "description": "Write the V-SEFM repair result."}],
            "resultTemplate": {"status": "ready", "summary": "", "resolved_failure_refs": [], "changed_files": [], "verification_commands": [], "remaining_findings": []}
        },
        "requestReadPlan": {"groups": [
            delivery_core::ReadGroupRef::new("repair_core", 1, vec!["agentInstruction", "source", "allowedPaths", "protectedPaths"].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/repair")),
            delivery_core::ReadGroupRef::new("repair_result_contract", 2, vec!["outputContract"].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/repair-result"))
        ]}
    });
    let stored = match state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id: repair_id.clone(),
            request_kind: "vsefm_repair".to_string(),
            request_file: Some(request_file),
            delivery_id: session
                .get("deliveryId")
                .and_then(Value::as_str)
                .map(str::to_string),
            phase_id: session
                .get("phaseId")
                .and_then(Value::as_str)
                .map(str::to_string),
            root: request,
        },
    ) {
        Ok(stored) => stored,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_REPAIR_REQUEST_FAILED",
                error.to_string(),
            )
        }
    };
    let mut updated = session.clone();
    if let Some(object) = updated.as_object_mut() {
        object.insert("status".to_string(), json!("repairing"));
        object.insert("repairId".to_string(), json!(repair_id));
        object.insert(
            "repairRequestRef".to_string(),
            json!(stored.request_ref.clone()),
        );
        object.insert("repairResultFile".to_string(), json!(result_file));
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    let state_path = session_dir.join("state.json");
    if let Err(error) = state::store::write_json_atomic(&state_path, &updated) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    if let (Some(delivery_id), Some(phase_id)) = (
        session.get("deliveryId").and_then(Value::as_str),
        session.get("phaseId").and_then(Value::as_str),
    ) {
        if let Err(error) = persist_vsefm_repair_action(
            project_root,
            delivery_id,
            phase_id,
            &verification_id,
            &repair_id,
            &stored.request_ref,
        ) {
            return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
        }
    }
    let inspected = match state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: stored.request_ref.clone(),
    }) {
        Ok(inspected) => inspected,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_REPAIR_REQUEST_INSPECT_FAILED",
                error.to_string(),
            )
        }
    };
    LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
        project_root.to_string(),
        delivery_core::LoomMcpNextAction::RunVsefmRepair(delivery_core::VsefmRepairNext {
            repair_id,
            verification_id,
            request_ref: stored.request_ref,
            result_file,
            read_groups: inspected.read_groups,
            submit_tool: "loom.vsefmRepairAcceptFile".to_string(),
            allowed_paths,
            protected_paths: vec![
                ".loom".to_string(),
                "plugins/shared/loom/references/verification/sefm-verify.md".to_string(),
            ],
        }),
    ))
}

pub fn accept_vsefm_repair_file(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
) -> LoomMcpActionResult {
    let Some(target) = authorized
        .targets
        .iter()
        .find(|target| target.target_id == "result")
    else {
        return failed(
            &input.project_root,
            "VSEFM_REPAIR_TARGET_MISSING",
            "V-SEFM repair result target is missing.",
        );
    };
    let root = Path::new(&input.project_root);
    let path = match state::paths::from_project_relative(root, &target.path) {
        Ok(path) => path,
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_REPAIR_PATH_INVALID",
                error.to_string(),
            )
        }
    };
    let raw = match state::store::read_json_value(&path) {
        Ok(value) => value,
        Err(error) => {
            return failed(
                &input.project_root,
                "VSEFM_REPAIR_READ_FAILED",
                error.to_string(),
            )
        }
    };
    let candidate: VsefmRepairCandidate = match serde_json::from_value(raw) {
        Ok(candidate) => candidate,
        Err(error) => {
            return vsefm_result_repair(
                input,
                authorized,
                format!("V-SEFM repair result is invalid: {error}"),
            )
        }
    };
    if candidate.status != VsefmRepairStatus::Ready || candidate.summary.trim().is_empty() {
        return vsefm_result_repair(
            input,
            authorized,
            "V-SEFM repair must be ready and include a summary.".to_string(),
        );
    }
    let (verification_id, session) = match find_vsefm_repair_session(root, &authorized.request_id) {
        Ok(session) => session,
        Err(error) => return failed(&input.project_root, "VSEFM_SESSION_NOT_FOUND", error),
    };
    if session.get("status").and_then(Value::as_str) != Some("repairing")
        || session.get("repairId").and_then(Value::as_str) != Some(authorized.request_id.as_str())
    {
        return failed(
            &input.project_root,
            "VSEFM_REPAIR_STATE_INVALID",
            "V-SEFM repair result does not match the active repair request.",
        );
    }
    let session_path = root
        .join(".loom/verification/sessions")
        .join(&verification_id)
        .join("state.json");
    let mut updated = session.clone();
    if let Some(object) = updated.as_object_mut() {
        object.insert("status".to_string(), json!("awaiting_reverification"));
        object.insert(
            "repairResult".to_string(),
            serde_json::to_value(&candidate).unwrap_or_else(|_| json!({})),
        );
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    if let Err(error) = state::store::write_json_atomic(&session_path, &updated) {
        return failed(
            &input.project_root,
            "VSEFM_STATE_WRITE_FAILED",
            error.to_string(),
        );
    }
    let config = match load_config() {
        Ok(config) => config,
        Err(error) => return failed(&input.project_root, "VSEFM_CONFIG_INVALID", error),
    };
    let action = RouteAction {
        kind: RouteActionKind::VsefmVerification,
        source: "vsefm_repair".to_string(),
        reason: "Re-run V-SEFM after repair.".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: None,
        details: Some(json!({
            "trigger": session.get("trigger").and_then(Value::as_str).unwrap_or("explicit"),
            "resumeAction": session.get("resumeAction").cloned().unwrap_or(Value::Null)
        })),
        target_phase_id: None,
    };
    start_local_verification(
        &input.project_root,
        session.get("deliveryId").and_then(Value::as_str),
        session.get("phaseId").and_then(Value::as_str),
        &action,
        &config,
        vec![],
    )
}

fn find_vsefm_repair_session(root: &Path, repair_id: &str) -> Result<(String, Value), String> {
    let sessions_dir = root.join(".loom/verification/sessions");
    let entries = std::fs::read_dir(&sessions_dir)
        .map_err(|error| format!("cannot read {}: {error}", sessions_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path().join("state.json");
        let Ok(session) = state::store::read_json_value(&path) else {
            continue;
        };
        if session.get("repairId").and_then(Value::as_str) == Some(repair_id) {
            let Some(verification_id) = session
                .get("verificationId")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                return Err("V-SEFM repair session is missing verificationId.".to_string());
            };
            return Ok((verification_id, session));
        }
    }
    Err(format!("repair session for {repair_id} does not exist"))
}

fn persist_vsefm_gate(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    gate: &LoomMcpActionResult,
) -> Result<(), String> {
    let LoomMcpActionResult::UserGate(gate) = gate else {
        return Ok(());
    };
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
    phase.next_action = Some(RouteAction {
        kind: RouteActionKind::VsefmResultGate,
        source: "vsefm_verification".to_string(),
        reason: "V-SEFM result requires user resolution.".to_string(),
        prompt: Some(gate.prompt.clone()),
        accepted_responses: gate.accepted_responses.clone(),
        request_ref: None,
        details: gate.gate.clone(),
        target_phase_id: None,
    });
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(|error| error.to_string())?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(|error| error.to_string())
}

fn persist_vsefm_verification_action(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    verification_id: &str,
    request_ref: &str,
    trigger: &str,
) -> Result<(), String> {
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
    phase.next_action = Some(RouteAction {
        kind: RouteActionKind::VsefmVerification,
        source: "vsefm_verification".to_string(),
        reason: "Run local V-SEFM verification.".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(request_ref.to_string()),
        details: Some(json!({"verificationId": verification_id, "trigger": trigger})),
        target_phase_id: None,
    });
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(|error| error.to_string())?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(|error| error.to_string())
}

fn persist_vsefm_repair_action(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    verification_id: &str,
    repair_id: &str,
    request_ref: &str,
) -> Result<(), String> {
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
    phase.next_action = Some(RouteAction {
        kind: RouteActionKind::VsefmRepair,
        source: "vsefm_verification".to_string(),
        reason: "Repair V-SEFM blocking findings before re-verification.".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: Some(request_ref.to_string()),
        details: Some(json!({
            "verificationId": verification_id,
            "repairId": repair_id
        })),
        target_phase_id: None,
    });
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(|error| error.to_string())?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(|error| error.to_string())
}

fn persist_vsefm_manual_review_gate(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    verification_id: &str,
    result_ref: &str,
) -> Result<(), String> {
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
    phase.next_action = Some(RouteAction {
        kind: RouteActionKind::VsefmResultGate,
        source: "vsefm_verification".to_string(),
        reason: "V-SEFM verification requires manual review.".to_string(),
        prompt: Some(
            "V-SEFM verification requires manual review. Choose approve_override or request_changes."
                .to_string(),
        ),
        accepted_responses: vec!["approve_override".to_string(), "request_changes".to_string()],
        request_ref: None,
        details: Some(json!({
            "kind": "vsefm_manual_review",
            "verificationId": verification_id,
            "resultRef": result_ref
        })),
        target_phase_id: None,
    });
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(project_root, &delivery)
        .map_err(|error| error.to_string())?;
    apply_delivery_index(&mut status, &delivery);
    store
        .save_status(project_root, &status)
        .map_err(|error| error.to_string())
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
            target_batch: Some(10),
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

    #[test]
    fn explicit_verify_does_not_capture_an_unrelated_active_route() {
        let action = RouteAction::done("continue the active delivery");
        assert!(pending_onboarding_context(Some(&action)).is_none());
    }

    #[test]
    fn explicit_onboarding_does_not_capture_a_review_resume_route() {
        let config = VsefmConfig {
            content: "verify".to_string(),
            url: "https://platform.example.test".to_string(),
            auto_route_after_review: false,
        };
        let onboarding = onboarding_action(
            &config,
            Some("delivery-1"),
            Some("phase-1"),
            Some(RouteAction::done("unrelated route")),
            "explicit",
        );
        assert!(pending_onboarding_context(Some(&onboarding)).is_none());
    }

    #[test]
    fn review_onboarding_preserves_only_its_declared_resume_route() {
        let config = VsefmConfig {
            content: "verify".to_string(),
            url: "https://platform.example.test".to_string(),
            auto_route_after_review: true,
        };
        let resume = RouteAction::done("review approved");
        let onboarding = onboarding_action(
            &config,
            Some("delivery-1"),
            Some("phase-1"),
            Some(resume.clone()),
            "review",
        );
        let (captured, trigger) =
            pending_onboarding_context(Some(&onboarding)).expect("review onboarding context");
        assert_eq!(captured, Some(resume));
        assert_eq!(trigger, "review");
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
