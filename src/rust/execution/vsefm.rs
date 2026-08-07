use std::collections::{BTreeMap, BTreeSet};
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
    SupplementalVerification,
    RetryRepair,
    ApproveOverride,
    RequestChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct VsefmVerificationCandidate {
    pub checks: Vec<VsefmCheckResult>,
    #[serde(default)]
    pub not_applicable_checks: Vec<VsefmNotApplicableCheck>,
    #[serde(default)]
    pub environment_blocked_checks: Vec<VsefmEnvironmentBlockedCheck>,
    pub blocking_failures: Vec<VsefmBlockingFailure>,
    pub warnings: Vec<String>,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VsefmVerificationStatus {
    Pass,
    Blocked,
    EnvironmentBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VsefmCheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct VsefmBlockingFailure {
    pub finding_id: String,
    pub check_id: String,
    pub severity: String,
    pub summary: String,
    pub remediation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct VsefmNotApplicableCheck {
    pub check_id: String,
    pub reason: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct VsefmEnvironmentBlockedCheck {
    pub check_id: String,
    pub attempted: String,
    pub reason: String,
    pub required_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SefmRuleCatalog {
    schema_version: String,
    rules: Vec<SefmRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SefmRule {
    id: String,
    section: String,
    blocking: bool,
    #[serde(skip)]
    title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct VsefmRepairCandidate {
    pub status: VsefmRepairStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attempt: Option<u64>,
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
        verification_id: None,
        result_ref: None,
        attempt: None,
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
            None,
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
        verification_id: None,
        result_ref: None,
        attempt: None,
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

fn sync_vsefm_record(
    project_root: &str,
    session: &Value,
    status: &str,
    result_ref: Option<&str>,
) -> Result<(), String> {
    let path = Path::new(project_root)
        .join(".loom")
        .join("verification")
        .join(RECORD_FILE_NAME);
    let mut record = state::store::read_json_value(&path).map_err(|error| error.to_string())?;
    let Some(object) = record.as_object_mut() else {
        return Err("V-SEFM record must be a JSON object".to_string());
    };
    object.insert("status".to_string(), json!(status));
    if let Some(value) = session.get("verificationId") {
        object.insert("verificationId".to_string(), value.clone());
    }
    if let Some(result_ref) = result_ref {
        object.insert("resultRef".to_string(), json!(result_ref));
    } else if let Some(value) = session.get("resultRef") {
        object.insert("resultRef".to_string(), value.clone());
    }
    if let Some(value) = session.get("attempt") {
        object.insert("attempt".to_string(), value.clone());
    }
    object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    state::store::write_json_atomic(&path, &record).map_err(|error| error.to_string())
}

const RULE_CATALOG_START: &str = "<!-- loom-rule-catalog:start -->";
const RULE_CATALOG_END: &str = "<!-- loom-rule-catalog:end -->";

fn read_rule_catalog(prompt_ref: &str) -> Result<SefmRuleCatalog, String> {
    let content = std::fs::read_to_string(prompt_ref)
        .map_err(|error| format!("cannot read V-SEFM prompt: {error}"))?;
    let start = content
        .find(RULE_CATALOG_START)
        .ok_or_else(|| "V-SEFM prompt is missing the rule catalog start marker.".to_string())?
        + RULE_CATALOG_START.len();
    let end = content[start..]
        .find(RULE_CATALOG_END)
        .map(|offset| start + offset)
        .ok_or_else(|| "V-SEFM prompt is missing the rule catalog end marker.".to_string())?;
    let mut catalog: SefmRuleCatalog = serde_json::from_str(content[start..end].trim())
        .map_err(|error| format!("V-SEFM prompt rule catalog is invalid: {error}"))?;
    if catalog.schema_version != "1.0" || catalog.rules.is_empty() {
        return Err(
            "V-SEFM prompt rule catalog must use schemaVersion 1.0 and contain rules.".to_string(),
        );
    }
    let mut ids = BTreeSet::new();
    for rule in &mut catalog.rules {
        if rule.id.trim().is_empty() || rule.section.trim().is_empty() || !ids.insert(&rule.id) {
            return Err(
                "V-SEFM prompt rule catalog contains an empty or duplicate rule id.".to_string(),
            );
        }
        rule.title = content
            .lines()
            .find_map(|line| {
                let heading = line.trim().strip_prefix("## ")?;
                let (section, title) = heading.split_once('.')?;
                (section.trim() == rule.section && !title.trim().is_empty())
                    .then(|| title.trim().to_string())
            })
            .unwrap_or_else(|| rule.id.clone());
    }
    Ok(catalog)
}

fn rule_catalog_hash(prompt_ref: &str) -> Result<String, String> {
    let content =
        std::fs::read(prompt_ref).map_err(|error| format!("cannot read V-SEFM prompt: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(content)))
}

fn rule_by_id<'a>(catalog: &'a SefmRuleCatalog, check_id: &str) -> Option<&'a SefmRule> {
    catalog.rules.iter().find(|rule| rule.id == check_id)
}

fn start_local_verification(
    project_root: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
    action: &RouteAction,
    config: &VsefmConfig,
    warnings: Vec<String>,
    subject_override: Option<Value>,
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
    let scope = action
        .details
        .as_ref()
        .and_then(|details| details.get("scope"))
        .and_then(Value::as_str)
        .unwrap_or(if trigger == "review" {
            "current_phase"
        } else {
            "completed_phases"
        });
    let supplemental_check_ids = action
        .details
        .as_ref()
        .and_then(|details| details.get("supplementalRuleIds"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let supplemental_baseline_ref = action
        .details
        .as_ref()
        .and_then(|details| details.get("supplementalBaselineRef"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let subject = match subject_override {
        Some(subject) => subject,
        None => match build_verification_subject(
            project_root,
            effective_delivery_id.as_deref(),
            phase_id,
            scope,
        ) {
            Ok(subject) => subject,
            Err(error) => return failed(project_root, "VSEFM_SCOPE_BUILD_FAILED", error),
        },
    };
    let subject_path = session_dir.join("subject.json");
    if let Err(error) = state::store::write_json_atomic(&subject_path, &subject) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    let prompt_ref = match verification_prompt_path() {
        Ok(path) => path,
        Err(error) => return failed(project_root, "VSEFM_PROMPT_UNAVAILABLE", error),
    };
    let rule_catalog = match read_rule_catalog(&prompt_ref) {
        Ok(catalog) => catalog,
        Err(error) => return failed(project_root, "VSEFM_PROMPT_INVALID", error),
    };
    let rule_catalog_hash = match rule_catalog_hash(&prompt_ref) {
        Ok(hash) => hash,
        Err(error) => return failed(project_root, "VSEFM_PROMPT_INVALID", error),
    };
    let runtime_provenance = verification_runtime_provenance(&prompt_ref);
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
        &runtime_provenance,
        &rule_catalog,
        &rule_catalog_hash,
        &supplemental_check_ids,
        supplemental_baseline_ref.as_deref(),
    );
    let stored = match state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id,
            request_kind: if supplemental_check_ids.is_empty() {
                "vsefm_local_verification"
            } else {
                "vsefm_supplemental_verification"
            }
            .to_string(),
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
    let mut session = json!({
        "schemaVersion": "1.0",
        "verificationId": verification_id,
        "trigger": trigger,
        "deliveryId": effective_delivery_id,
        "phaseId": phase_id,
        "scope": scope,
        "subjectRef": subject_ref,
        "promptRef": prompt_ref,
        "ruleCatalogHash": rule_catalog_hash,
        "requestRef": stored.request_ref,
        "resultFile": result_file,
        "runtimeProvenance": runtime_provenance,
        "resumeAction": action.details.as_ref().and_then(|details| details.get("resumeAction")).cloned(),
        "status": "awaiting_agent",
        "attempt": 1,
        "createdAt": state::store::now_string(),
        "updatedAt": state::store::now_string()
    });
    if let Some(object) = session.as_object_mut() {
        if !supplemental_check_ids.is_empty() {
            object.insert(
                "supplementalRuleIds".to_string(),
                json!(supplemental_check_ids),
            );
            if let Some(baseline_ref) = supplemental_baseline_ref {
                object.insert("supplementalBaselineRef".to_string(), json!(baseline_ref));
            }
        }
    }
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
            verification_id: Some(verification_id.clone()),
            result_ref: None,
            attempt: Some(1),
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

fn verification_runtime_provenance(prompt_ref: &str) -> Value {
    let verification_rules_sha256 = std::fs::read(prompt_ref)
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_else(|| "unavailable".to_string());
    let runtime_manifest_sha256 = loom_runtime_home()
        .ok()
        .map(|home| home.join("manifest.json"))
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_else(|| "unavailable".to_string());
    let mcp_binary_sha256 = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_else(|| "unavailable".to_string());
    let build_fingerprint = std::env::var("LOOM_BUILD_FINGERPRINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unavailable".to_string());
    let fingerprint_input = format!(
        "{}\n{}\n{}\n{}\n{}",
        delivery_core::VERSION,
        verification_rules_sha256,
        runtime_manifest_sha256,
        mcp_binary_sha256,
        build_fingerprint
    );
    let runtime_fingerprint = format!("{:x}", Sha256::digest(fingerprint_input.as_bytes()));
    json!({
        "loomVersion": delivery_core::VERSION,
        "verificationRulesSha256": verification_rules_sha256,
        "runtimeManifestSha256": runtime_manifest_sha256,
        "mcpBinarySha256": mcp_binary_sha256,
        "buildFingerprint": build_fingerprint,
        "runtimeFingerprint": runtime_fingerprint
    })
}

fn runtime_provenance_warning(session: &Value) -> Option<String> {
    let prompt_ref = session.get("promptRef").and_then(Value::as_str)?;
    let Some(recorded) = session
        .get("runtimeProvenance")
        .and_then(|value| value.get("runtimeFingerprint"))
        .and_then(Value::as_str)
    else {
        return Some(
            "V-SEFM request has no runtime provenance because it was generated before provenance tracking; the request remains immutable and this run is continuing with an explicit provenance warning."
                .to_string(),
        );
    };
    let current = verification_runtime_provenance(prompt_ref);
    let current_fingerprint = current.get("runtimeFingerprint").and_then(Value::as_str)?;
    if recorded == current_fingerprint {
        None
    } else {
        Some(format!(
            "V-SEFM request was generated by a different Loom runtime (recorded fingerprint {recorded}, current fingerprint {current_fingerprint}); the request remains immutable and this run is continuing with an explicit provenance warning."
        ))
    }
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
        | (RouteActionKind::VsefmVerification, "repair_incomplete")
        | (RouteActionKind::VsefmRepair, "repairing")
        | (RouteActionKind::VsefmResultGate, "awaiting_user_resolution")
        | (RouteActionKind::VsefmResultGate, "manual_review")
        | (RouteActionKind::VsefmResultGate, "repair_incomplete") => {
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
                "awaiting_agent"
                    | "awaiting_user_resolution"
                    | "manual_review"
                    | "repair_incomplete"
                    | "repairing"
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
    } else if session.get("status").and_then(Value::as_str) == Some("repair_incomplete") {
        RouteActionKind::VsefmResultGate
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
            let result = match read_vsefm_result(Path::new(project_root), session) {
                Ok(result) => result,
                Err(error) => return failed(project_root, "VSEFM_RESULT_READ_FAILED", error),
            };
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
        (RouteActionKind::VsefmVerification, "repair_incomplete")
        | (RouteActionKind::VsefmResultGate, "repair_incomplete") => {
            vsefm_repair_incomplete_gate(project_root, session)
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
    let subject_ref = session
        .get("subjectRef")
        .and_then(Value::as_str)
        .filter(|reference| !reference.is_empty())
        .ok_or_else(|| "V-SEFM verification session is missing subjectRef.".to_string());
    let subject_ref = match subject_ref {
        Ok(subject_ref) => subject_ref,
        Err(error) => return failed(project_root, "VSEFM_RESUME_CONTEXT_MISSING", error),
    };
    let subject_path =
        match state::paths::from_project_relative(Path::new(project_root), subject_ref) {
            Ok(path) => path,
            Err(error) => {
                return failed(
                    project_root,
                    "VSEFM_RESUME_CONTEXT_MISSING",
                    error.to_string(),
                )
            }
        };
    let subject = match state::store::read_json_value(&subject_path) {
        Ok(subject) => subject,
        Err(error) => {
            return failed(
                project_root,
                "VSEFM_RESUME_CONTEXT_MISSING",
                error.to_string(),
            )
        }
    };
    let warnings = runtime_provenance_warning(session)
        .into_iter()
        .collect::<Vec<_>>();
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
    .with_warnings(warnings)
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
    let scope_hints = session
        .get("repairScopeHints")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect::<Vec<_>>();
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
            scope_hints,
            protected_paths: VSEFM_REPAIR_PROTECTED_PATHS
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
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
    let mut accepted_artifacts = Vec::new();
    let mut accepted_paths = BTreeSet::new();
    if let Some(delivery_id) = delivery_id {
        let delivery = FileTransitionStore
            .load_delivery_index(project_root, delivery_id)
            .map_err(|error| error.to_string())?;
        for (reference, role) in [
            (
                format!(".loom/deliveries/{delivery_id}/requirements/context.json"),
                "requirement",
            ),
            (
                format!(".loom/deliveries/{delivery_id}/contracts/technical-baseline.json"),
                "technical_baseline",
            ),
            (
                format!(".loom/deliveries/{delivery_id}/contracts/api/current.json"),
                "api_contract",
            ),
        ] {
            add_accepted_artifact(
                root,
                &reference,
                role,
                None,
                &mut accepted_paths,
                &mut source_refs,
                &mut accepted_artifacts,
            );
        }
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
            for (reference, role) in [
                (
                    format!(".loom/deliveries/{delivery_id}/contracts/planning/{phase}/pgc.json"),
                    "planning_contract",
                ),
                (
                    format!(
                        ".loom/deliveries/{delivery_id}/contracts/architecture/{phase}/aac.json"
                    ),
                    "architecture_contract",
                ),
                (
                    format!(".loom/deliveries/{delivery_id}/tasks/{phase}/taskplans/latest.json"),
                    "task_plan",
                ),
            ] {
                add_accepted_artifact(
                    root,
                    &reference,
                    role,
                    Some(&phase),
                    &mut accepted_paths,
                    &mut source_refs,
                    &mut accepted_artifacts,
                );
            }
            add_latest_review_artifact(
                root,
                delivery_id,
                &phase,
                &mut accepted_paths,
                &mut source_refs,
                &mut accepted_artifacts,
            );
            add_latest_task_result_artifacts(
                root,
                delivery_id,
                &phase,
                &mut accepted_paths,
                &mut source_refs,
                &mut accepted_artifacts,
                &mut changed_files,
            );
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
        "acceptedArtifacts": accepted_artifacts,
        "changedFiles": files,
        "generatedAt": state::store::now_string()
    }))
}

fn add_accepted_artifact(
    root: &Path,
    reference: &str,
    role: &str,
    phase_id: Option<&str>,
    accepted_paths: &mut BTreeSet<String>,
    source_refs: &mut Vec<String>,
    accepted_artifacts: &mut Vec<Value>,
) {
    if !accepted_paths.insert(reference.to_string()) {
        return;
    }
    let path = root.join(reference);
    let Ok(bytes) = std::fs::read(&path) else {
        accepted_paths.remove(reference);
        return;
    };
    if serde_json::from_slice::<Value>(&bytes).is_err() {
        accepted_paths.remove(reference);
        return;
    }
    let hash = Sha256::digest(&bytes);
    source_refs.push(reference.to_string());
    accepted_artifacts.push(json!({
        "path": reference,
        "role": role,
        "phase_id": phase_id,
        "sha256": format!("{hash:x}"),
        "bytes": bytes.len()
    }));
}

fn add_latest_review_artifact(
    root: &Path,
    delivery_id: &str,
    phase_id: &str,
    accepted_paths: &mut BTreeSet<String>,
    source_refs: &mut Vec<String>,
    accepted_artifacts: &mut Vec<Value>,
) {
    let latest = root.join(format!(
        ".loom/deliveries/{delivery_id}/reviews/{phase_id}/latest.json"
    ));
    let Ok(value) = state::store::read_json_value(&latest) else {
        return;
    };
    let Some(reference) = value.get("reviewResultRef").and_then(Value::as_str) else {
        return;
    };
    add_accepted_artifact(
        root,
        reference,
        "review_result",
        Some(phase_id),
        accepted_paths,
        source_refs,
        accepted_artifacts,
    );
}

fn add_latest_task_result_artifacts(
    root: &Path,
    delivery_id: &str,
    phase_id: &str,
    accepted_paths: &mut BTreeSet<String>,
    source_refs: &mut Vec<String>,
    accepted_artifacts: &mut Vec<Value>,
    changed_files: &mut BTreeSet<String>,
) {
    let latest_run = root.join(format!(
        ".loom/deliveries/{delivery_id}/tasks/{phase_id}/runs/latest.json"
    ));
    let Ok(latest_run_value) = state::store::read_json_value(&latest_run) else {
        return;
    };
    let Some(run_reference) = latest_run_value.get("runRef").and_then(Value::as_str) else {
        return;
    };
    let run_path = root.join(run_reference);
    let Ok(run) = state::store::read_json_value(&run_path) else {
        return;
    };
    let Some(task_states) = run.get("taskStates").and_then(Value::as_array) else {
        return;
    };
    let results_root = root.join(format!(
        ".loom/deliveries/{delivery_id}/tasks/{phase_id}/results"
    ));
    for result_id in task_states
        .iter()
        .filter_map(|task| task.get("resultId").and_then(Value::as_str))
    {
        let Some(result_path) = find_named_file(&results_root, result_id) else {
            continue;
        };
        let Ok(result) = state::store::read_json_value(&result_path) else {
            continue;
        };
        if let Some(files) = result.get("changedFiles").and_then(Value::as_array) {
            for file in files.iter().filter_map(Value::as_str) {
                if is_safe_verification_path(file) && root.join(file).is_file() {
                    changed_files.insert(file.to_string());
                }
            }
        }
        let Ok(reference) = result_path.strip_prefix(root) else {
            continue;
        };
        let reference = reference
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        add_accepted_artifact(
            root,
            &reference,
            "task_result",
            Some(phase_id),
            accepted_paths,
            source_refs,
            accepted_artifacts,
        );
    }
}

fn find_named_file(root: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_named_file(&path, name) {
                return Some(found);
            }
        } else if path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == name)
        {
            return Some(path);
        }
    }
    None
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

fn verification_result_schema(
    catalog: &SefmRuleCatalog,
    supplemental_check_ids: &[String],
) -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(VsefmVerificationCandidate))
        .unwrap_or_else(|_| json!({"type": "object"}));
    let ids = if supplemental_check_ids.is_empty() {
        catalog
            .rules
            .iter()
            .map(|rule| json!(rule.id))
            .collect::<Vec<_>>()
    } else {
        supplemental_check_ids
            .iter()
            .map(|check_id| json!(check_id))
            .collect::<Vec<_>>()
    };
    for definition in [
        "VsefmCheckResult",
        "VsefmNotApplicableCheck",
        "VsefmEnvironmentBlockedCheck",
        "VsefmBlockingFailure",
    ] {
        if let Some(check_id) =
            schema.pointer_mut(&format!("/$defs/{definition}/properties/check_id"))
        {
            check_id["enum"] = json!(ids);
        }
    }
    schema
}

fn verification_result_template(
    _catalog: &SefmRuleCatalog,
    _supplemental_check_ids: &[String],
) -> Value {
    json!({
        "checks": [],
        "not_applicable_checks": [],
        "environment_blocked_checks": [],
        "blocking_failures": [],
        "warnings": [],
        "recommended_actions": []
    })
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
    runtime_provenance: &Value,
    rule_catalog: &SefmRuleCatalog,
    rule_catalog_hash: &str,
    supplemental_check_ids: &[String],
    supplemental_baseline_ref: Option<&str>,
) -> Value {
    let result_schema = verification_result_schema(rule_catalog, supplemental_check_ids);
    let result_template = verification_result_template(rule_catalog, supplemental_check_ids);
    let mut steps = vec![
        "Read verification_execution_core, verification_prompt, verification_subject, and verification_result_contract.".to_string(),
        "Read the complete sefm-verify.md from promptRef, including its machine-readable rule catalog, before evaluating any delivery artifact.".to_string(),
        "Read only the files listed by subject.changedFiles and subject.acceptedArtifacts; these are the complete accepted inputs for this verification.".to_string(),
        "Use the rule ids and rule text from sefm-verify.md as the only verification catalog. The verification Agent decides applicability from the accepted artifacts; MCP does not decide semantic applicability. For each rule, produce exactly one entry in checks (status pass or fail), not_applicable_checks, or environment_blocked_checks.".to_string(),
        "Keep each check within its rule section: AUTH checks own identity and authorization; SECURITY-BOUNDARY owns input, secret, path, command, and network boundaries; OBSERVABILITY-EVIDENCE owns request, mutation, and response traceability; BROWSER-QUALITY owns Playwright browser checks.".to_string(),
        "For AUTH checks, accept identity only from a server-verified session, token, or identity-provider context. A client-provided identity header, form value, query value, resource owner id, or similar request field is not authentication evidence; test the server-side verification and authorization path instead.".to_string(),
        "Record concrete input, expected, observed, evidence, and timestamp for every applicable check. For every non-applicable rule, record its rule id, reason, and evidence in not_applicable_checks.".to_string(),
        "This initial verification pass must attempt every applicable rule before writing the result. Follow the concrete checks in sefm-verify.md; for concurrency, observability, and performance, run bounded checks with the available project tools instead of deferring the check.".to_string(),
        "Every applicable rule must end as pass or fail. A missing dedicated test is not a reason to defer a rule; inspect the implementation and run a bounded verification that can establish the result.".to_string(),
        "Use environment_blocked_checks only after an actual attempt was made and a concrete environment or tool capability prevented completion. Record the attempted action, exact blocking reason, and required capability. Do not use environment_blocked_checks for omitted or inconvenient work.".to_string(),
        "Do not put an environment-blocked result in checks, do not put a confirmed failure in environment_blocked_checks, and do not copy a finding from one check into another check with a different generated scope.".to_string(),
        "The outer status is normalized by Loom after structural acceptance. Do not resubmit a structurally valid result only to change status from pass, blocked, or environment_blocked.".to_string(),
        "Create one blocking_failure per distinct finding and reference the failed check with check_id; do not duplicate check evidence in blocking_failures.".to_string(),
    ];
    if supplemental_check_ids.is_empty() {
        steps.push(
            "Account for every rule id in the prompt catalog exactly once across checks, not_applicable_checks, and environment_blocked_checks. There must be no missing rule ids and no unknown result group."
                .to_string(),
        );
    } else {
        steps.push(format!(
            "This is an environment-retry verification. Execute only these environment-blocked check ids: {} after the required capability is available. Submit exactly one checks, not_applicable_checks, or environment_blocked_checks entry for each supplied id; Loom will merge them with the previously accepted canonical result.",
            supplemental_check_ids.join(", ")
        ));
        steps.push(
            "Do not modify product files, do not rerun unrelated checks, and do not copy previously accepted checks into this result.".to_string(),
        );
    }
    steps.push(
        "Write the result candidate and submit it with loom.vsefmVerificationAcceptFile."
            .to_string(),
    );
    let mut source = json!({
        "trigger": trigger,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "scope": scope
    });
    if !supplemental_check_ids.is_empty() {
        if let Some(object) = source.as_object_mut() {
            object.insert(
                "supplementalRuleIds".to_string(),
                json!(supplemental_check_ids),
            );
            if let Some(baseline_ref) = supplemental_baseline_ref {
                object.insert("baselineResultRef".to_string(), json!(baseline_ref));
            }
        }
    }
    let instruction = json!({
        "role": "software_delivery_verifier",
        "objective": if supplemental_check_ids.is_empty() {
            "Verify the declared delivery subject against sefm-verify.md without modifying product or Loom files."
        } else {
            "Collect only the missing V-SEFM evidence listed in source.supplementalRuleIds without modifying product or Loom files."
        },
        "steps": steps,
        "hardBlockingRules": [
            "A failed rule marked blocking=true in the prompt catalog requires a blocking_failure reference; Loom derives the outer status after acceptance.",
            "Do not write a top-level status field. Loom derives the outer status from the per-rule result groups after acceptance.",
            "In checks, status=pass means the rule applies, every requirement in its sefm-verify.md section was evaluated, and the evidence proves the requirement. status=fail means the rule applies but the requirement is violated or the evidence does not establish it.",
            "A rule that does not apply must be written only to not_applicable_checks with a reason and evidence; it must not be written to checks with status=pass.",
            "Never claim pass without reproducible evidence.",
            "Attempt every applicable rule in the initial pass. Use environment_blocked_checks only after an actual attempt is blocked by a concrete environment or tool limitation; do not use it to defer work or hide an established defect.",
            "Do not add fields, duplicate rule plans, or use natural-language keywords as a substitute for the generated rule catalog."
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
        "requestType": if supplemental_check_ids.is_empty() {
            "vsefm_local_verification"
        } else {
            "vsefm_supplemental_verification"
        },
        "verificationId": verification_id,
        "source": source,
        "agentInstruction": instruction,
            "prompt": {
            "ref": prompt_ref,
            "ruleCatalogHash": rule_catalog_hash
        },
        "runtimeProvenance": runtime_provenance,
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
            "agentOwnedFields": [
                "checks",
                "not_applicable_checks",
                "environment_blocked_checks",
                "blocking_failures",
                "warnings",
                "recommended_actions"
            ],
            "mcpOwnedFields": [
                "status",
                "artifact_id",
                "verification_id",
                "scope",
                "source",
                "rule_catalog_hash",
                "statistics",
                "attempts"
            ],
            "mcpOwnedPaths": [
                "checks[*].check_id",
                "not_applicable_checks[*].check_id",
                "environment_blocked_checks[*].check_id",
                "blocking_failures[*].check_id"
            ],
            "ruleCatalog": rule_catalog,
            "resultSchema": result_schema,
            "resultTemplate": result_template
        },
        "requestReadPlan": {
            "groups": [
                delivery_core::ReadGroupRef::new("verification_execution_core", 1, vec![
                    "agentInstruction", "source", "completionBarrier", "boundaryRules"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/execution")),
                delivery_core::ReadGroupRef::new("verification_prompt", 2, vec![
                    "prompt", "prompt.ref"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/prompt")),
                delivery_core::ReadGroupRef::new("verification_subject", 3, vec![
                    "subject", "subject.scope", "subject.phaseIds", "subject.requirementRefs", "subject.acceptedArtifacts", "subject.changedFiles"
                ].into_iter().map(str::to_string).collect(), subject_ref),
                delivery_core::ReadGroupRef::new("verification_result_contract", 4, vec![
                    "outputContract"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/result-contract"))
            ]
        }
    })
}

pub fn accept_vsefm_verification_file<D: DomainDispatcher>(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
    dispatcher: D,
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
    let prompt_ref = match session.get("promptRef").and_then(Value::as_str) {
        Some(prompt_ref) => prompt_ref,
        None => {
            return failed(
                &input.project_root,
                "VSEFM_PROMPT_UNAVAILABLE",
                "V-SEFM session is missing promptRef.",
            )
        }
    };
    let rule_catalog = match read_rule_catalog(prompt_ref) {
        Ok(catalog) => catalog,
        Err(error) => return failed(&input.project_root, "VSEFM_PROMPT_INVALID", error),
    };
    let supplemental_ids = session
        .get("supplementalRuleIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let candidate: VsefmVerificationCandidate = match deserialize_vsefm_candidate(raw.clone()) {
        Ok(candidate) => candidate,
        Err(issue) => {
            let repeated = match record_vsefm_submit_attempt(
                root,
                &session_path,
                &session,
                &raw,
                std::slice::from_ref(&issue),
                false,
            ) {
                Ok(repeated) => repeated,
                Err(error) => {
                    return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error);
                }
            };
            if repeated {
                return finalize_vsefm_contract_fault(
                    &input.project_root,
                    &session_path,
                    &session,
                    &raw,
                    std::slice::from_ref(&issue),
                    "verification_result",
                    dispatcher,
                );
            }
            return vsefm_result_repair_with_issues(input, authorized, vec![issue]);
        }
    };
    let mut issues = if supplemental_ids.is_empty() {
        validate_vsefm_candidate(&candidate, &rule_catalog, None)
    } else {
        validate_vsefm_candidate(&candidate, &rule_catalog, Some(&supplemental_ids))
    };
    let candidate = if issues.is_empty() && !supplemental_ids.is_empty() {
        let baseline_ref = session
            .get("supplementalBaselineRef")
            .and_then(Value::as_str)
            .ok_or_else(|| "supplemental V-SEFM session is missing baselineResultRef".to_string());
        match baseline_ref {
            Ok(baseline_ref) => match state::paths::from_project_relative(root, baseline_ref) {
                Ok(path) => match state::store::read_json_value(&path) {
                    Ok(baseline) => match merge_supplemental_candidate(
                        &baseline,
                        candidate.clone(),
                        &supplemental_ids,
                    ) {
                        Ok(merged) => merged,
                        Err(error) => {
                            issues.push(vsefm_issue(
                                "VSEFM_SUPPLEMENTAL_MERGE_FAILED",
                                "checks",
                                &error,
                            ));
                            candidate
                        }
                    },
                    Err(error) => {
                        issues.push(vsefm_issue(
                            "VSEFM_SUPPLEMENTAL_BASELINE_READ_FAILED",
                            "source.baselineResultRef",
                            &error.to_string(),
                        ));
                        candidate
                    }
                },
                Err(error) => {
                    issues.push(vsefm_issue(
                        "VSEFM_SUPPLEMENTAL_BASELINE_REF_INVALID",
                        "source.baselineResultRef",
                        &error.to_string(),
                    ));
                    candidate
                }
            },
            Err(error) => {
                issues.push(vsefm_issue(
                    "VSEFM_SUPPLEMENTAL_BASELINE_MISSING",
                    "source.baselineResultRef",
                    &error,
                ));
                candidate
            }
        }
    } else {
        candidate
    };
    if issues.is_empty() && !supplemental_ids.is_empty() {
        // The supplemental candidate is scope-checked before merging. Once merged,
        // the canonical result must be validated as the complete catalog rather
        // than treating the retained baseline checks as out of scope.
        issues = validate_vsefm_candidate(&candidate, &rule_catalog, None);
    }
    if !issues.is_empty() {
        let repeated = match record_vsefm_submit_attempt(
            root,
            &session_path,
            &session,
            &raw,
            &issues,
            false,
        ) {
            Ok(repeated) => repeated,
            Err(error) => {
                return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error);
            }
        };
        if repeated {
            return finalize_vsefm_contract_fault(
                &input.project_root,
                &session_path,
                &session,
                &raw,
                &issues,
                "verification_result",
                dispatcher,
            );
        }
        return vsefm_result_repair_with_issues(input, authorized, issues);
    }
    if let Err(error) = record_vsefm_submit_attempt(root, &session_path, &session, &raw, &[], true)
    {
        return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error);
    }
    let result = match canonical_vsefm_result(root, &verification_id, &candidate, &session) {
        Ok(result) => result,
        Err(error) => {
            return failed(&input.project_root, "VSEFM_CANONICAL_RESULT_FAILED", error);
        }
    };
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
    if let Err(error) = sync_vsefm_record(
        &input.project_root,
        &updated_session,
        "awaiting_user_resolution",
        Some(&format!(
            ".loom/verification/results/{verification_id}.json"
        )),
    ) {
        return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error);
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

fn deserialize_vsefm_candidate(
    raw: Value,
) -> Result<VsefmVerificationCandidate, delivery_core::RepairIssue> {
    let text = serde_json::to_string(&raw).map_err(|error| {
        vsefm_issue(
            "VSEFM_RESULT_SCHEMA_INVALID",
            "$",
            &format!("V-SEFM result could not be prepared for schema validation: {error}"),
        )
    })?;
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let field_path = if path == "." || path.is_empty() {
            "$".to_string()
        } else {
            path.trim_start_matches('.').to_string()
        };
        vsefm_issue(
            "VSEFM_RESULT_SCHEMA_INVALID",
            &field_path,
            &format!("V-SEFM result field does not match the generated resultSchema: {error}"),
        )
    })
}

fn record_vsefm_submit_attempt(
    root: &Path,
    session_path: &Path,
    session: &Value,
    raw: &Value,
    issues: &[delivery_core::RepairIssue],
    accepted: bool,
) -> Result<bool, String> {
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .ok_or_else(|| "V-SEFM session is missing verificationId".to_string())?;
    let attempt = session.get("attempt").and_then(Value::as_u64).unwrap_or(1);
    let candidate_hash = Sha256::digest(
        serde_json::to_vec(raw).map_err(|error| format!("cannot hash V-SEFM result: {error}"))?,
    );
    let candidate_hash = format!("{candidate_hash:x}");
    let issue_fingerprint = Sha256::digest(
        serde_json::to_vec(issues)
            .map_err(|error| format!("cannot hash V-SEFM issues: {error}"))?,
    );
    let issue_fingerprint = format!("{issue_fingerprint:x}");
    let audit_path = root
        .join(".loom/verification/sessions")
        .join(verification_id)
        .join("submit-attempts.jsonl");
    let previous_same = std::fs::read_to_string(&audit_path)
        .ok()
        .into_iter()
        .flat_map(|content| {
            content
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .any(|entry| {
            entry.get("candidateSha256").and_then(Value::as_str) == Some(candidate_hash.as_str())
                && entry.get("issueFingerprint").and_then(Value::as_str)
                    == Some(issue_fingerprint.as_str())
                && entry.get("accepted").and_then(Value::as_bool) == Some(false)
        });
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let entry = json!({
        "verificationId": verification_id,
        "attempt": attempt,
        "candidateSha256": candidate_hash,
        "issueFingerprint": issue_fingerprint,
        "issues": issues,
        "accepted": accepted,
        "submittedAt": state::store::now_string()
    });
    use std::io::Write;
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_path)
        .map_err(|error| error.to_string())?;
    writeln!(
        handle,
        "{}",
        serde_json::to_string(&entry).map_err(|error| error.to_string())?
    )
    .map_err(|error| error.to_string())?;

    let mut updated_session = session.clone();
    if let Some(object) = updated_session.as_object_mut() {
        if !accepted {
            object.insert("attempt".to_string(), json!(attempt.saturating_add(1)));
        }
        object.insert("lastSubmitSha256".to_string(), json!(candidate_hash));
        object.insert("lastSubmitIssues".to_string(), json!(issues));
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    state::store::write_json_atomic(session_path, &updated_session)
        .map_err(|error| error.to_string())?;
    Ok(previous_same)
}

fn vsefm_result_repair_with_issues(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
    issues: Vec<delivery_core::RepairIssue>,
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
        issues,
        resubmit_tool: "loom.vsefmVerificationAcceptFile".to_string(),
        fix_scope: Some("Edit only the Agent-owned V-SEFM result candidate.".to_string()),
        read_groups: authorized.read_groups.clone(),
        agent_instruction: delivery_core::repairable_error_agent_instruction(
            "loom.vsefmVerificationAcceptFile",
        ),
    })
}

fn finalize_vsefm_contract_fault<D: DomainDispatcher>(
    project_root: &str,
    session_path: &Path,
    session: &Value,
    raw: &Value,
    issues: &[delivery_core::RepairIssue],
    stage: &str,
    dispatcher: D,
) -> LoomMcpActionResult {
    let candidate_hash = Sha256::digest(serde_json::to_vec(raw).unwrap_or_default());
    let mut updated =
        state::store::read_json_value(session_path).unwrap_or_else(|_| session.clone());
    if let Some(object) = updated.as_object_mut() {
        object.insert("status".to_string(), json!("verification_unavailable"));
        object.insert(
            "contractFault".to_string(),
            json!({
                "stage": stage,
                "code": "VSEFM_CONTRACT_FAULT",
                "candidateSha256": format!("{candidate_hash:x}"),
                "issues": issues,
                "recordedAt": state::store::now_string()
            }),
        );
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    if let Err(error) = state::store::write_json_atomic(session_path, &updated) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    if let Err(error) = sync_vsefm_record(project_root, &updated, "verification_unavailable", None)
    {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
    }
    resume_after_vsefm(project_root, &updated, "verification_unavailable", dispatcher).with_warnings(vec![
        "V-SEFM could not consume the same result after the contract error repeated. The raw result and contract fault were preserved; delivery resumed without treating the verification as passed or blocked.".to_string(),
    ])
}

fn deserialize_vsefm_repair_candidate(
    raw: Value,
) -> Result<VsefmRepairCandidate, delivery_core::RepairIssue> {
    let text = serde_json::to_string(&raw).map_err(|error| {
        vsefm_issue(
            "VSEFM_REPAIR_SCHEMA_INVALID",
            "$",
            &format!("V-SEFM repair result could not be prepared for schema validation: {error}"),
        )
    })?;
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let field_path = if path == "." || path.is_empty() {
            "$".to_string()
        } else {
            path.trim_start_matches('.').to_string()
        };
        vsefm_issue(
            "VSEFM_REPAIR_SCHEMA_INVALID",
            &field_path,
            &format!("V-SEFM repair field does not match the generated resultSchema: {error}"),
        )
    })
}

fn record_vsefm_repair_submit_attempt(
    root: &Path,
    session_path: &Path,
    session: &Value,
    raw: &Value,
    issues: &[delivery_core::RepairIssue],
    accepted: bool,
) -> Result<bool, String> {
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .ok_or_else(|| "V-SEFM session is missing verificationId".to_string())?;
    let attempt = session
        .get("repairAttempt")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let candidate_hash = Sha256::digest(
        serde_json::to_vec(raw)
            .map_err(|error| format!("cannot hash V-SEFM repair result: {error}"))?,
    );
    let candidate_hash = format!("{candidate_hash:x}");
    let issue_fingerprint = Sha256::digest(
        serde_json::to_vec(issues)
            .map_err(|error| format!("cannot hash V-SEFM repair issues: {error}"))?,
    );
    let issue_fingerprint = format!("{issue_fingerprint:x}");
    let audit_path = root
        .join(".loom/verification/sessions")
        .join(verification_id)
        .join("repair-submit-attempts.jsonl");
    let previous_same = std::fs::read_to_string(&audit_path)
        .ok()
        .into_iter()
        .flat_map(|content| {
            content
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .any(|entry| {
            entry.get("candidateSha256").and_then(Value::as_str) == Some(candidate_hash.as_str())
                && entry.get("issueFingerprint").and_then(Value::as_str)
                    == Some(issue_fingerprint.as_str())
                && entry.get("accepted").and_then(Value::as_bool) == Some(false)
        });
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let entry = json!({
        "verificationId": verification_id,
        "repairId": session.get("repairId"),
        "attempt": attempt,
        "candidateSha256": candidate_hash,
        "issueFingerprint": issue_fingerprint,
        "issues": issues,
        "accepted": accepted,
        "submittedAt": state::store::now_string()
    });
    use std::io::Write;
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_path)
        .map_err(|error| error.to_string())?;
    writeln!(
        handle,
        "{}",
        serde_json::to_string(&entry).map_err(|error| error.to_string())?
    )
    .map_err(|error| error.to_string())?;

    let mut updated_session = session.clone();
    if let Some(object) = updated_session.as_object_mut() {
        if !accepted {
            object.insert(
                "repairAttempt".to_string(),
                json!(attempt.saturating_add(1)),
            );
        }
        object.insert("lastRepairSubmitSha256".to_string(), json!(candidate_hash));
        object.insert("lastRepairSubmitIssues".to_string(), json!(issues));
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    state::store::write_json_atomic(session_path, &updated_session)
        .map_err(|error| error.to_string())?;
    Ok(previous_same)
}

fn vsefm_repair_result_repair(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
    issues: Vec<delivery_core::RepairIssue>,
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
        issues,
        resubmit_tool: "loom.vsefmRepairAcceptFile".to_string(),
        fix_scope: Some("Edit only the Agent-owned V-SEFM repair result candidate.".to_string()),
        read_groups: authorized.read_groups.clone(),
        agent_instruction: delivery_core::repairable_error_agent_instruction(
            "loom.vsefmRepairAcceptFile",
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
    if status == "repair_incomplete" {
        return match input.decision {
            VsefmVerificationResolution::RetryRepair => {
                materialize_vsefm_repair(&input.project_root, &session)
            }
            VsefmVerificationResolution::ManualReview => {
                finish_vsefm_session(&input.project_root, &session, "manual_review", dispatcher)
            }
            _ => failed(
                &input.project_root,
                "VSEFM_RESOLUTION_INVALID",
                "An incomplete Agent repair requires retry_repair or manual_review.",
            ),
        };
    }
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
    if input.decision == VsefmVerificationResolution::SupplementalVerification {
        let result = match read_vsefm_result(root, &session) {
            Ok(result) => result,
            Err(error) => return failed(&input.project_root, "VSEFM_RESULT_READ_FAILED", error),
        };
        let status = result
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if status != "environment_blocked" {
            return failed(
                &input.project_root,
                "VSEFM_RESOLUTION_INVALID",
                "Environment retry is only valid for an environment-blocked result.",
            );
        }
        let blocked_rule_ids = supplemental_rule_ids(&result);
        if blocked_rule_ids.is_empty() {
            return failed(
                &input.project_root,
                "VSEFM_RESOLUTION_INVALID",
                "Environment retry requires at least one environment-blocked rule.",
            );
        }
        let baseline_result_ref = session
            .get("resultRef")
            .and_then(Value::as_str)
            .map(str::to_string);
        let subject_ref = session
            .get("subjectRef")
            .and_then(Value::as_str)
            .ok_or_else(|| "V-SEFM session is missing subjectRef".to_string());
        let subject_ref = match subject_ref {
            Ok(subject_ref) => subject_ref,
            Err(error) => return failed(&input.project_root, "VSEFM_SCOPE_BUILD_FAILED", error),
        };
        let subject_path = match state::paths::from_project_relative(root, subject_ref) {
            Ok(path) => path,
            Err(error) => {
                return failed(
                    &input.project_root,
                    "VSEFM_SCOPE_BUILD_FAILED",
                    error.to_string(),
                )
            }
        };
        let subject = match state::store::read_json_value(&subject_path) {
            Ok(subject) => subject,
            Err(error) => {
                return failed(
                    &input.project_root,
                    "VSEFM_SCOPE_BUILD_FAILED",
                    error.to_string(),
                )
            }
        };
        let mut superseded = session.clone();
        if let Some(object) = superseded.as_object_mut() {
            object.insert("status".to_string(), json!("supplemental_started"));
            object.insert(
                "supplementalReason".to_string(),
                json!("The result contains environment-blocked checks; retry only those checks after the required capability is available and merge them with the accepted result."),
            );
            object.insert("updatedAt".to_string(), json!(state::store::now_string()));
        }
        if let Err(error) = state::store::write_json_atomic(&session_path, &superseded) {
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
            source: "vsefm_supplemental".to_string(),
            reason: "Collect supplemental V-SEFM evidence using the accepted prompt rule catalog."
                .to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: Some(json!({
                "trigger": "supplemental",
                "scope": session.get("scope").cloned().unwrap_or_else(|| json!("completed_phases")),
                "resumeAction": session.get("resumeAction").cloned().unwrap_or(Value::Null),
                "supplementalRuleIds": blocked_rule_ids,
                "supplementalBaselineRef": baseline_result_ref
            })),
            target_phase_id: None,
        };
        return start_local_verification(
            &input.project_root,
            session.get("deliveryId").and_then(Value::as_str),
            session.get("phaseId").and_then(Value::as_str),
            &action,
            &config,
            vec![],
            Some(subject),
        );
    }
    match input.decision {
        VsefmVerificationResolution::Accept => {
            let result = match read_vsefm_result(root, &session) {
                Ok(result) => result,
                Err(error) => {
                    return failed(&input.project_root, "VSEFM_RESULT_READ_FAILED", error)
                }
            };
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
        | VsefmVerificationResolution::RequestChanges
        | VsefmVerificationResolution::RetryRepair => failed(
            &input.project_root,
            "VSEFM_RESOLUTION_INVALID",
            "The selected manual review resolution is not valid for this verification result.",
        ),
        VsefmVerificationResolution::SupplementalVerification => failed(
            &input.project_root,
            "VSEFM_RESOLUTION_INVALID",
            "Supplemental verification was not handled for the current result state.",
        ),
    }
}

fn validate_vsefm_candidate(
    candidate: &VsefmVerificationCandidate,
    catalog: &SefmRuleCatalog,
    supplemental_rule_ids: Option<&BTreeSet<String>>,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut check_statuses = BTreeMap::new();

    for (index, check) in candidate.checks.iter().enumerate() {
        let field_prefix = format!("checks[{index}]");
        if rule_by_id(catalog, &check.check_id).is_none() {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_ID_INVALID",
                &format!("{field_prefix}.check_id"),
                "check_id is not present in the sefm-verify.md rule catalog.",
            ));
            continue;
        }
        if let Some(scope) = supplemental_rule_ids {
            if !scope.contains(&check.check_id) {
                issues.push(vsefm_issue(
                    "VSEFM_SUPPLEMENTAL_RULE_OUT_OF_SCOPE",
                    &format!("{field_prefix}.check_id"),
                    "A supplemental result may only contain the rule ids supplied by Loom.",
                ));
            }
        }
        if !seen.insert(check.check_id.clone()) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_DUPLICATE",
                &format!("{field_prefix}.check_id"),
                "Each rule id may appear only once across the result.",
            ));
        }
        check_statuses.insert(check.check_id.clone(), check.status);
        if check.category.trim().is_empty()
            || check.rule.trim().is_empty()
            || check.input.trim().is_empty()
            || check.expected.trim().is_empty()
            || check.observed.trim().is_empty()
        {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_CONTEXT_REQUIRED",
                &field_prefix,
                "Each applicable check requires non-empty input, expected, and observed values.",
            ));
        }
        if check.evidence.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_EVIDENCE_REQUIRED",
                &format!("{field_prefix}.evidence"),
                "Every applicable check requires evidence.",
            ));
        }
        if check.timestamp.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_TIMESTAMP_REQUIRED",
                &format!("{field_prefix}.timestamp"),
                "Every applicable check requires a non-empty evidence timestamp.",
            ));
        }
    }

    for (index, check) in candidate.not_applicable_checks.iter().enumerate() {
        let field_prefix = format!("not_applicable_checks[{index}]");
        if rule_by_id(catalog, &check.check_id).is_none() {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_ID_INVALID",
                &format!("{field_prefix}.check_id"),
                "check_id is not present in the sefm-verify.md rule catalog.",
            ));
            continue;
        }
        if let Some(scope) = supplemental_rule_ids {
            if !scope.contains(&check.check_id) {
                issues.push(vsefm_issue(
                    "VSEFM_SUPPLEMENTAL_RULE_OUT_OF_SCOPE",
                    &format!("{field_prefix}.check_id"),
                    "A supplemental result may only contain the rule ids supplied by Loom.",
                ));
            }
        }
        if !seen.insert(check.check_id.clone()) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_DUPLICATE",
                &format!("{field_prefix}.check_id"),
                "Each rule id may appear only once across the result.",
            ));
        }
        if check.reason.trim().is_empty() || check.evidence.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_NOT_APPLICABLE_CONTEXT_REQUIRED",
                &field_prefix,
                "Each not-applicable rule requires a reason and evidence.",
            ));
        }
    }

    for (index, blocked) in candidate.environment_blocked_checks.iter().enumerate() {
        let field_prefix = format!("environment_blocked_checks[{index}]");
        if rule_by_id(catalog, &blocked.check_id).is_none() {
            issues.push(vsefm_issue(
                "VSEFM_ENVIRONMENT_BLOCKED_CHECK_ID_INVALID",
                &format!("{field_prefix}.check_id"),
                "environment-blocked check id is not present in the sefm-verify.md rule catalog.",
            ));
            continue;
        }
        if let Some(scope) = supplemental_rule_ids {
            if !scope.contains(&blocked.check_id) {
                issues.push(vsefm_issue(
                    "VSEFM_SUPPLEMENTAL_RULE_OUT_OF_SCOPE",
                    &format!("{field_prefix}.check_id"),
                    "A supplemental result may only reference the supplied rule ids.",
                ));
            }
        }
        if !seen.insert(blocked.check_id.clone()) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_DUPLICATE",
                &format!("{field_prefix}.check_id"),
                "Each rule id may appear only once across the result.",
            ));
        }
        if blocked.attempted.trim().is_empty()
            || blocked.reason.trim().is_empty()
            || blocked.required_capability.trim().is_empty()
        {
            issues.push(vsefm_issue(
                "VSEFM_ENVIRONMENT_BLOCKED_CONTEXT_REQUIRED",
                &field_prefix,
                "Each environment-blocked check requires the attempted action, blocking reason, and required capability.",
            ));
        }
    }

    let expected_ids = supplemental_rule_ids
        .cloned()
        .unwrap_or_else(|| catalog.rules.iter().map(|rule| rule.id.clone()).collect());
    for check_id in expected_ids {
        if !seen.contains(&check_id) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_COVERAGE_MISSING",
                "result",
                &format!("Rule {check_id} must have exactly one pass, fail, not-applicable, or environment-blocked result."),
            ));
        }
    }

    let blocking_failure_check_ids = candidate
        .blocking_failures
        .iter()
        .map(|failure| failure.check_id.as_str())
        .collect::<BTreeSet<_>>();
    for (index, check) in candidate.checks.iter().enumerate() {
        if check.status == VsefmCheckStatus::Fail
            && rule_by_id(catalog, &check.check_id).is_some_and(|rule| rule.blocking)
            && !blocking_failure_check_ids.contains(check.check_id.as_str())
        {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_FINDING_REQUIRED",
                &format!("checks[{index}].check_id"),
                "A failed blocking rule requires one blocking_failure with the same check_id.",
            ));
        }
    }

    let mut finding_ids = BTreeSet::new();
    for (index, failure) in candidate.blocking_failures.iter().enumerate() {
        let field_prefix = format!("blocking_failures[{index}]");
        let Some(rule) = rule_by_id(catalog, &failure.check_id) else {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_CHECK_INVALID",
                &format!("{field_prefix}.check_id"),
                "blocking failure must reference a rule in the sefm-verify.md catalog.",
            ));
            continue;
        };
        if let Some(scope) = supplemental_rule_ids {
            if !scope.contains(&failure.check_id) {
                issues.push(vsefm_issue(
                    "VSEFM_SUPPLEMENTAL_RULE_OUT_OF_SCOPE",
                    &format!("{field_prefix}.check_id"),
                    "A supplemental result may only reference the supplied rule ids.",
                ));
            }
        }
        if check_statuses.get(&failure.check_id) != Some(&VsefmCheckStatus::Fail) {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_CHECK_UNSUPPORTED",
                &format!("{field_prefix}.check_id"),
                "blocking failure must reference a failed check.",
            ));
        }
        if !rule.blocking {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_RULE_MISMATCH",
                &format!("{field_prefix}.check_id"),
                "A blocking failure may only reference a rule marked blocking in the prompt catalog.",
            ));
        }
        if !finding_ids.insert(failure.finding_id.clone())
            || failure.finding_id.trim().is_empty()
            || failure.severity.trim().is_empty()
            || failure.summary.trim().is_empty()
            || failure.remediation.trim().is_empty()
        {
            issues.push(vsefm_issue(
                "VSEFM_FINDING_INVALID",
                &field_prefix,
                "Each finding requires a unique finding_id, severity, summary, and remediation.",
            ));
        }
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

fn candidate_coverage(
    catalog: &SefmRuleCatalog,
    candidate: &VsefmVerificationCandidate,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let checked_ids = candidate
        .checks
        .iter()
        .map(|check| check.check_id.clone())
        .collect::<BTreeSet<_>>();
    let not_applicable_ids = candidate
        .not_applicable_checks
        .iter()
        .map(|check| check.check_id.clone())
        .collect::<BTreeSet<_>>();
    let environment_blocked_ids = candidate
        .environment_blocked_checks
        .iter()
        .map(|check| check.check_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_ids = catalog
        .rules
        .iter()
        .filter(|rule| {
            !checked_ids.contains(&rule.id)
                && !not_applicable_ids.contains(&rule.id)
                && !environment_blocked_ids.contains(&rule.id)
        })
        .map(|rule| rule.id.clone())
        .collect::<Vec<_>>();
    (
        checked_ids.into_iter().collect(),
        not_applicable_ids.into_iter().collect(),
        environment_blocked_ids.into_iter().collect(),
        missing_ids,
    )
}

fn canonical_vsefm_result(
    root: &Path,
    verification_id: &str,
    candidate: &VsefmVerificationCandidate,
    session: &Value,
) -> Result<Value, String> {
    let subject_ref = session
        .get("subjectRef")
        .and_then(Value::as_str)
        .ok_or_else(|| "V-SEFM session is missing subjectRef".to_string())?;
    let subject_path = state::paths::from_project_relative(root, subject_ref)
        .map_err(|error| error.to_string())?;
    let subject_bytes = std::fs::read(&subject_path).map_err(|error| error.to_string())?;
    let subject_hash = Sha256::digest(&subject_bytes);
    let prompt_ref = session
        .get("promptRef")
        .and_then(Value::as_str)
        .ok_or_else(|| "V-SEFM session is missing promptRef".to_string())?;
    let catalog = read_rule_catalog(prompt_ref)?;
    let (checked_ids, not_applicable_ids, environment_blocked_ids, missing_ids) =
        candidate_coverage(&catalog, candidate);
    let status = canonical_vsefm_status(candidate);
    let runtime_provenance = session
        .get("runtimeProvenance")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let current_runtime_provenance = session
        .get("promptRef")
        .and_then(Value::as_str)
        .map(verification_runtime_provenance)
        .unwrap_or_else(|| json!({}));
    let runtime_fingerprint_matches = runtime_provenance
        .get("runtimeFingerprint")
        .and_then(Value::as_str)
        .zip(
            current_runtime_provenance
                .get("runtimeFingerprint")
                .and_then(Value::as_str),
        )
        .is_some_and(|(recorded, current)| recorded == current);
    let passed_checks = candidate
        .checks
        .iter()
        .filter(|check| check.status == VsefmCheckStatus::Pass)
        .count();
    Ok(json!({
        "schema_version": "1.0",
        "artifact_id": verification_id,
        "verification_id": verification_id,
        "status": status,
        "checks": candidate.checks,
        "not_applicable_checks": candidate.not_applicable_checks,
        "environment_blocked_checks": candidate.environment_blocked_checks,
        "blocking_failures": candidate.blocking_failures,
        "warnings": candidate.warnings,
        "recommended_actions": candidate.recommended_actions,
        "rule_catalog_hash": session.get("ruleCatalogHash"),
        "coverage": {
            "checked_rule_ids": checked_ids,
            "not_applicable_rule_ids": not_applicable_ids,
            "environment_blocked_rule_ids": environment_blocked_ids,
            "missing_rule_ids": missing_ids
        },
        "passed_checks": passed_checks,
        "failed_checks": candidate
            .checks
            .iter()
            .filter(|check| check.status == VsefmCheckStatus::Fail)
            .count(),
        "warning_count": candidate.warnings.len(),
        "environment_blocked_count": candidate.environment_blocked_checks.len(),
        "attempts": session.get("attempt").cloned().unwrap_or_else(|| json!(1)),
        "source": {
            "delivery_id": session.get("deliveryId"),
            "phase_id": session.get("phaseId"),
            "scope": session.get("scope"),
            "subject_ref": session.get("subjectRef"),
            "prompt_ref": session.get("promptRef"),
            "subject_sha256": format!("{subject_hash:x}"),
            "runtime_provenance": runtime_provenance
        },
        "runtime_provenance_check": {
            "matches_current_runtime": runtime_fingerprint_matches,
            "current_runtime": current_runtime_provenance
        },
        "created_at": state::store::now_string()
    }))
}

fn canonical_vsefm_status(candidate: &VsefmVerificationCandidate) -> VsefmVerificationStatus {
    if candidate
        .checks
        .iter()
        .any(|check| check.status == VsefmCheckStatus::Fail)
    {
        VsefmVerificationStatus::Blocked
    } else if !candidate.environment_blocked_checks.is_empty() {
        VsefmVerificationStatus::EnvironmentBlocked
    } else {
        VsefmVerificationStatus::Pass
    }
}

fn read_vsefm_result(root: &Path, session: &Value) -> Result<Value, String> {
    let reference = session
        .get("resultRef")
        .and_then(Value::as_str)
        .ok_or_else(|| "V-SEFM session is missing resultRef".to_string())?;
    let path =
        state::paths::from_project_relative(root, reference).map_err(|error| error.to_string())?;
    state::store::read_json_value(&path).map_err(|error| error.to_string())
}

fn supplemental_rule_ids(result: &Value) -> Vec<String> {
    result
        .get("environment_blocked_checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("check_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn canonical_result_candidate(result: &Value) -> Result<VsefmVerificationCandidate, String> {
    serde_json::from_value(json!({
        "checks": result.get("checks").cloned().unwrap_or_else(|| json!([])),
        "not_applicable_checks": result.get("not_applicable_checks").cloned().unwrap_or_else(|| json!([])),
        "environment_blocked_checks": result.get("environment_blocked_checks").cloned().unwrap_or_else(|| json!([])),
        "blocking_failures": result.get("blocking_failures").cloned().unwrap_or_else(|| json!([])),
        "warnings": result.get("warnings").cloned().unwrap_or_else(|| json!([])),
        "recommended_actions": result.get("recommended_actions").cloned().unwrap_or_else(|| json!([]))
    }))
    .map_err(|error| format!("previous V-SEFM result cannot be merged: {error}"))
}

fn merge_supplemental_candidate(
    baseline: &Value,
    supplemental: VsefmVerificationCandidate,
    supplemental_rule_ids: &BTreeSet<String>,
) -> Result<VsefmVerificationCandidate, String> {
    let mut merged = canonical_result_candidate(baseline)?;
    merged
        .checks
        .retain(|check| !supplemental_rule_ids.contains(&check.check_id));
    merged
        .not_applicable_checks
        .retain(|check| !supplemental_rule_ids.contains(&check.check_id));
    merged
        .environment_blocked_checks
        .retain(|check| !supplemental_rule_ids.contains(&check.check_id));
    merged.checks.extend(supplemental.checks);
    merged
        .not_applicable_checks
        .extend(supplemental.not_applicable_checks);
    merged
        .environment_blocked_checks
        .extend(supplemental.environment_blocked_checks);
    merged
        .blocking_failures
        .retain(|finding| !supplemental_rule_ids.contains(&finding.check_id));
    merged
        .blocking_failures
        .extend(supplemental.blocking_failures);
    for warning in supplemental.warnings {
        if !merged.warnings.contains(&warning) {
            merged.warnings.push(warning);
        }
    }
    for action in supplemental.recommended_actions {
        if !merged.recommended_actions.contains(&action) {
            merged.recommended_actions.push(action);
        }
    }
    Ok(merged)
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
    if let Err(error) = sync_vsefm_record(project_root, &updated, status, None) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
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
                "V-SEFM 验证需要人工审查，请选择后续处理方式：\n1. 确认放行\n2. 返回自动修复\n结果文件：{result_ref}"
            ),
            vec!["1".to_string(), "2".to_string()],
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
                "resultRef": result_ref,
                "options": [
                    {"value": "1", "label": "确认放行", "decision": "approve_override"},
                    {"value": "2", "label": "返回自动修复", "decision": "request_changes"}
                ]
            })),
        )
        .with_agent_instruction(
            "展示 V-SEFM 阻断项并等待用户选择。用户选择 1 时调用 loom.vsefmVerificationResolve，decision=approve_override；选择 2 时使用 decision=request_changes。不要在用户选择前调用 loom.continue 或知识工具。",
        ),
    );
    if let Err(error) = sync_vsefm_record(project_root, session, "manual_review", None) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
    }
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

fn vsefm_repair_incomplete_gate(project_root: &str, session: &Value) -> LoomMcpActionResult {
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or("verification");
    let result_ref = session
        .get("repairResultFile")
        .and_then(Value::as_str)
        .unwrap_or(".loom/agent-writable/vsefm-repair-result.json");
    let gate = LoomMcpActionResult::UserGate(
        LoomMcpUserGateResult::new(
            project_root.to_string(),
            format!(
                "Agent 未能完成 V-SEFM 修复，请选择后续处理方式：\n1. 重新让 Agent 修复\n2. 转人工审查\n修复结果：{result_ref}"
            ),
            vec!["1".to_string(), "2".to_string()],
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
                "kind": "vsefm_repair_incomplete",
                "verificationId": verification_id,
                "resultRef": result_ref,
                "options": [
                    {"value": "1", "label": "重新让 Agent 修复", "decision": "retry_repair"},
                    {"value": "2", "label": "转人工审查", "decision": "manual_review"}
                ]
            })),
        )
        .with_agent_instruction(
            "展示修复未完成的结果并等待用户选择。用户选择 1 时调用 loom.vsefmVerificationResolve，decision=retry_repair；选择 2 时使用 decision=manual_review。不要调用 loom.continue 或知识工具。",
        ),
    );
    if let Err(error) = sync_vsefm_record(project_root, session, "repair_incomplete", None) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
    }
    if let (Some(delivery_id), Some(phase_id)) = (
        session.get("deliveryId").and_then(Value::as_str),
        session.get("phaseId").and_then(Value::as_str),
    ) {
        if let Err(error) = persist_vsefm_gate(project_root, delivery_id, phase_id, &gate) {
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

fn vsefm_rule_title(catalog: &SefmRuleCatalog, check_id: &str) -> String {
    rule_by_id(catalog, check_id)
        .map(|rule| rule.title.clone())
        .unwrap_or_else(|| check_id.to_string())
}

fn vsefm_result_ids(result: &Value, path: &str) -> Vec<String> {
    result
        .pointer(path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            entry
                .as_str()
                .or_else(|| entry.get("check_id").and_then(Value::as_str))
        })
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn vsefm_result_failed_check_ids(result: &Value) -> Vec<String> {
    result
        .get("checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|check| check.get("status").and_then(Value::as_str) == Some("fail"))
        .filter_map(|check| check.get("check_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn vsefm_result_gate_model(
    verification_id: &str,
    result_ref: &str,
    result: &Value,
    catalog: &SefmRuleCatalog,
) -> (Value, bool) {
    let status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pass");
    let failed_check_ids = vsefm_result_failed_check_ids(result);
    let blocking_failure_ids = vsefm_result_ids(result, "/blocking_failures");
    let confirmed_ids = failed_check_ids
        .iter()
        .chain(blocking_failure_ids.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let environment_blocked_ids = vsefm_result_ids(result, "/environment_blocked_checks");
    let not_applicable_ids = vsefm_result_ids(result, "/not_applicable_checks");
    let has_confirmed_failure = !confirmed_ids.is_empty();
    let can_supplement = status == "environment_blocked"
        && !has_confirmed_failure
        && !environment_blocked_ids.is_empty();

    let mut failure_by_check = BTreeMap::new();
    if let Some(failures) = result.get("blocking_failures").and_then(Value::as_array) {
        for failure in failures {
            if let Some(check_id) = failure.get("check_id").and_then(Value::as_str) {
                failure_by_check.insert(check_id.to_string(), failure);
            }
        }
    }
    let mut check_by_id = BTreeMap::new();
    if let Some(checks) = result.get("checks").and_then(Value::as_array) {
        for check in checks {
            if let Some(check_id) = check.get("check_id").and_then(Value::as_str) {
                check_by_id.insert(check_id.to_string(), check);
            }
        }
    }
    let confirmed_findings = confirmed_ids
        .iter()
        .map(|check_id| {
            let check = check_by_id.get(check_id).copied();
            let failure = failure_by_check.get(check_id).copied();
            let summary = failure
                .and_then(|value| value.get("summary").and_then(Value::as_str))
                .or_else(|| check.and_then(|value| value.get("observed").and_then(Value::as_str)))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("验证未通过");
            let remediation = failure
                .and_then(|value| value.get("remediation").and_then(Value::as_str))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("修复该检查项后重新执行 V-SEFM 验证");
            json!({
                "checkId": check_id,
                "title": vsefm_rule_title(catalog, check_id),
                "severity": failure
                    .and_then(|value| value.get("severity").and_then(Value::as_str))
                    .unwrap_or("confirmed"),
                "summary": summary,
                "remediation": remediation,
                "input": check
                    .and_then(|value| value.get("input").and_then(Value::as_str))
                    .unwrap_or("已提交验证输入"),
                "expected": check
                    .and_then(|value| value.get("expected").and_then(Value::as_str))
                    .unwrap_or("满足对应规则"),
                "observed": check
                    .and_then(|value| value.get("observed").and_then(Value::as_str))
                    .unwrap_or(summary),
                "evidence": check
                    .and_then(|value| value.get("evidence").and_then(Value::as_str))
                    .unwrap_or("已提交验证证据")
            })
        })
        .collect::<Vec<_>>();

    let passed_checks = check_by_id
        .values()
        .filter(|check| check.get("status").and_then(Value::as_str) == Some("pass"))
        .filter_map(|check| {
            let check_id = check.get("check_id").and_then(Value::as_str)?;
            Some(json!({
                "checkId": check_id,
                "title": vsefm_rule_title(catalog, check_id),
                "summary": check
                    .get("observed")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("检查通过"),
                "evidence": check
                    .get("evidence")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("已提交验证证据")
            }))
        })
        .collect::<Vec<_>>();

    let not_applicable_checks = result
        .get("not_applicable_checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|check| {
            let check_id = check.get("check_id").and_then(Value::as_str)?;
            Some(json!({
                "checkId": check_id,
                "title": vsefm_rule_title(catalog, check_id),
                "reason": check
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("该检查项不适用于当前交付"),
                "evidence": check
                    .get("evidence")
                    .and_then(Value::as_str)
                    .unwrap_or("已提交适用性依据")
            }))
        })
        .collect::<Vec<_>>();

    let environment_blocked_checks = result
        .get("environment_blocked_checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|check| {
            let check_id = check.get("check_id").and_then(Value::as_str)?;
            Some(json!({
                "checkId": check_id,
                "title": vsefm_rule_title(catalog, check_id),
                "attempted": check
                    .get("attempted")
                    .and_then(Value::as_str)
                    .unwrap_or("已尝试执行验证"),
                "reason": check
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("验证被环境阻断"),
                "requiredCapability": check
                    .get("required_capability")
                    .and_then(Value::as_str)
                    .unwrap_or("完成该检查所需的环境或工具能力")
            }))
        })
        .collect::<Vec<_>>();

    let passed_count = result
        .get("checks")
        .and_then(Value::as_array)
        .map(|checks| {
            checks
                .iter()
                .filter(|check| check.get("status").and_then(Value::as_str) == Some("pass"))
                .count()
        })
        .unwrap_or(0);
    let failed_count = failed_check_ids.len();
    let options = if has_confirmed_failure {
        json!([
            {"value": "1", "label": "修复已确认问题并重新验证", "decision": "repair"},
            {"value": "2", "label": "转人工复核", "decision": "manual_review"}
        ])
    } else if can_supplement {
        json!([
            {"value": "1", "label": "环境准备完成后重试受阻检查", "decision": "supplemental_verification"},
            {"value": "2", "label": "转人工复核", "decision": "manual_review"}
        ])
    } else {
        json!([
            {"value": "1", "label": "接受验证结果并结束", "decision": "accept"},
            {"value": "2", "label": "转人工复核", "decision": "manual_review"}
        ])
    };
    (
        json!({
            "kind": "vsefm_result",
            "verificationId": verification_id,
            "resultRef": result_ref,
            "status": status,
            "counts": {
                "passed": passed_count,
                "failed": failed_count,
                "notApplicable": not_applicable_ids.len(),
                "environmentBlocked": environment_blocked_ids.len()
            },
            "confirmedFindings": confirmed_findings,
            "passedChecks": passed_checks,
            "notApplicableChecks": not_applicable_checks,
            "environmentBlockedChecks": environment_blocked_checks,
            "recommendedActions": result.get("recommended_actions").cloned().unwrap_or_else(|| json!([])),
            "options": options
        }),
        can_supplement,
    )
}

fn vsefm_result_gate(
    project_root: &str,
    verification_id: &str,
    result: &Value,
    result_ref: &str,
    delivery_id: Option<&str>,
    phase_id: Option<&str>,
) -> LoomMcpActionResult {
    let catalog = result
        .get("source")
        .and_then(|source| source.get("prompt_ref"))
        .and_then(Value::as_str)
        .and_then(|prompt_ref| read_rule_catalog(prompt_ref).ok())
        .unwrap_or_else(|| SefmRuleCatalog {
            schema_version: "1.0".to_string(),
            rules: vec![],
        });
    let (gate_model, _can_supplement) =
        vsefm_result_gate_model(verification_id, result_ref, result, &catalog);
    let choices = vec!["1".to_string(), "2".to_string()];
    let agent_instruction = "读取 gate 结构化结果并用用户当前语言完整展示：先展示 status 和 counts；然后逐项展示 passedChecks 中每个通过项的 title、summary 和 evidence，展示 notApplicableChecks 中每个不适用项的 title、reason 和 evidence，展示 environmentBlockedChecks 中每个环境阻断项的 title、attempted、reason 和 requiredCapability，再展示 confirmedFindings 中每个已确认问题的 title、severity、summary、input、expected、observed、evidence 和 remediation。不得只展示计数，不得省略任何通过项、不适用项、环境阻断项或已确认问题。不要使用 unknown 或 not_evaluated 这样的结论。严格使用 gate.options 的 value、label 和 decision，用户选择后调用 loom.vsefmVerificationResolve；不要自行计算路由、增加选项或调用 loom.continue、loom.inspectRequest、loom.readFieldGroup 或知识工具。用户选择 1 时必须使用 gate.options[0].decision，选择 2 时必须使用 gate.options[1].decision。".to_string();
    let warnings = if result
        .pointer("/runtime_provenance_check/matches_current_runtime")
        .and_then(Value::as_bool)
        == Some(false)
    {
        vec![
            "V-SEFM result was produced under a different Loom runtime fingerprint; the result was preserved and is not being rejected solely for this provenance mismatch."
                .to_string(),
        ]
    } else {
        vec![]
    };
    let message =
        "V-SEFM 验证结果已生成，请根据 gate 结构化结果向用户说明并等待选择 1 或 2。".to_string();
    LoomMcpActionResult::UserGate(
        LoomMcpUserGateResult::new(
            project_root.to_string(),
            message,
            choices,
            None,
            delivery_id.map(str::to_string),
            phase_id.map(str::to_string),
            Some(gate_model),
        )
        .with_agent_instruction(agent_instruction),
    )
    .with_warnings(warnings)
}

const VSEFM_REPAIR_PROTECTED_PATHS: &[&str] = &[
    ".loom",
    ".git",
    "plugins/shared/loom/references/verification/sefm-verify.md",
    "plugins/shared/loom/references/verification/v-sefm.json",
];

const REPAIR_RUNTIME_ARTIFACT_COMPONENTS: &[&str] = &[
    "node_modules",
    "__pycache__",
    ".vite",
    ".pytest_cache",
    "target",
    "dist",
    "build",
    ".venv",
];

fn is_repair_runtime_artifact_path(path: &str) -> bool {
    let components = path.split('/').filter(|component| !component.is_empty());
    if components
        .clone()
        .any(|component| REPAIR_RUNTIME_ARTIFACT_COMPONENTS.contains(&component))
    {
        return true;
    }
    let Some(file_name) = path.rsplit('/').next() else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".pyc")
        || lower.ends_with(".pyo")
        || lower.ends_with(".sqlite")
        || lower.ends_with(".sqlite3")
        || lower.ends_with(".db")
        || lower.ends_with(".db-journal")
        || lower.ends_with(".db-shm")
        || lower.ends_with(".db-wal")
        || lower.ends_with(".log")
        || lower.ends_with(".tmp")
}

fn repair_snapshot_file(root: &Path, path: &Path, include_control_tree: bool) -> Option<Value> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let relative = path
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let digest = Sha256::digest(&bytes);
    let kind = if !include_control_tree && is_repair_runtime_artifact_path(&relative) {
        "runtime_artifact"
    } else {
        "source"
    };
    Some(json!({
        "sha256": format!("{digest:x}"),
        "bytes": bytes.len(),
        "path": relative,
        "kind": kind
    }))
}

fn collect_repair_snapshot(
    root: &Path,
    current: &Path,
    include_control_tree: bool,
    files: &mut BTreeMap<String, Value>,
) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .ok()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if relative.is_empty() {
            continue;
        }
        if path.is_dir() {
            let control_tree_path = relative == ".loom"
                || relative.starts_with(".loom/")
                || relative.split('/').any(|component| component == ".loom");
            if relative.split('/').any(|component| component == ".git")
                || (!include_control_tree && control_tree_path)
                || (!include_control_tree && is_repair_runtime_artifact_path(&relative))
            {
                continue;
            }
            collect_repair_snapshot(root, &path, include_control_tree, files);
        } else if !include_control_tree && is_repair_runtime_artifact_path(&relative) {
            let Some(snapshot) = repair_snapshot_file(root, &path, include_control_tree) else {
                continue;
            };
            files.insert(relative, snapshot);
        } else if let Some(snapshot) = repair_snapshot_file(root, &path, include_control_tree) {
            files.insert(relative, snapshot);
        }
    }
}

fn build_repair_snapshot(root: &Path, include_control_tree: bool) -> Value {
    let mut files = BTreeMap::new();
    let snapshot_root = if include_control_tree {
        root.join(".loom")
    } else {
        root.to_path_buf()
    };
    if snapshot_root.is_dir() {
        collect_repair_snapshot(root, &snapshot_root, include_control_tree, &mut files);
    }
    let files = Value::Object(files.into_iter().collect());
    let digest = Sha256::digest(serde_json::to_vec(&files).unwrap_or_default());
    json!({
        "schemaVersion": 1,
        "files": files,
        "sha256": format!("{digest:x}")
    })
}

fn snapshot_file_map(snapshot: &Value) -> BTreeMap<String, Value> {
    snapshot
        .get("files")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn changed_snapshot_paths(before: &Value, after: &Value) -> Vec<String> {
    let before = snapshot_file_map(before);
    let after = snapshot_file_map(after);
    before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| path.to_string())
        .collect()
}

fn changed_source_snapshot_paths(before: &Value, after: &Value) -> Vec<String> {
    let before_map = snapshot_file_map(before);
    let after_map = snapshot_file_map(after);
    changed_snapshot_paths(before, after)
        .into_iter()
        .filter(|path| {
            [before_map.get(path), after_map.get(path)]
                .into_iter()
                .flatten()
                .any(|entry| entry.get("kind").and_then(Value::as_str) != Some("runtime_artifact"))
        })
        .collect()
}

fn repair_baseline_path(root: &Path, verification_id: &str, repair_id: &str) -> PathBuf {
    root.join(".loom/verification/sessions")
        .join(verification_id)
        .join(format!("{repair_id}-baseline.json"))
}

fn protected_path_changed(path: &str, changed_paths: &[String]) -> bool {
    changed_paths.iter().any(|changed| {
        changed == path
            || changed.starts_with(&format!("{path}/"))
            || path == ".loom" && (changed == ".loom" || changed.starts_with(".loom/"))
    })
}

fn repair_boundary_error(root: &Path, session: &Value, result_file: &str) -> Result<(), String> {
    let baseline_ref = session
        .get("repairBaselineRef")
        .and_then(Value::as_str)
        .ok_or_else(|| "V-SEFM repair session is missing repairBaselineRef".to_string())?;
    let baseline_path = state::paths::from_project_relative(root, baseline_ref)
        .map_err(|error| error.to_string())?;
    let baseline =
        state::store::read_json_value(&baseline_path).map_err(|error| error.to_string())?;
    let current_source = build_repair_snapshot(root, false);
    let source_changes = changed_source_snapshot_paths(
        baseline.get("sourceSnapshot").unwrap_or(&Value::Null),
        &current_source,
    );
    let current_control = build_repair_snapshot(root, true);
    let control_changes = changed_snapshot_paths(
        baseline.get("controlSnapshot").unwrap_or(&Value::Null),
        &current_control,
    );
    let allowed_control = [baseline_ref, result_file]
        .into_iter()
        .map(|path| path.trim_start_matches('/').to_string())
        .collect::<BTreeSet<_>>();
    let verification_id = session
        .get("verificationId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let allowed_control_prefix = format!(".loom/verification/sessions/{verification_id}/");
    let unauthorized_control = control_changes
        .into_iter()
        .filter(|path| {
            !allowed_control.contains(path)
                && path != ".loom/metrics/field-read-audit.jsonl"
                && !path.starts_with(".loom/metrics/")
                && *path != format!("{allowed_control_prefix}state.json")
                && *path != format!("{allowed_control_prefix}repair-submit-attempts.jsonl")
        })
        .collect::<Vec<_>>();
    if !unauthorized_control.is_empty() {
        return Err(format!(
            "protected Loom files changed during V-SEFM repair: {}",
            unauthorized_control.join(", ")
        ));
    }
    let protected_changes = source_changes
        .iter()
        .filter(|path| {
            VSEFM_REPAIR_PROTECTED_PATHS
                .iter()
                .any(|protected| protected_path_changed(protected, std::slice::from_ref(path)))
        })
        .cloned()
        .collect::<Vec<_>>();
    if !protected_changes.is_empty() {
        return Err(format!(
            "protected V-SEFM files changed during repair: {}",
            protected_changes.join(", ")
        ));
    }
    Ok(())
}

fn repair_changed_files(root: &Path, session: &Value) -> Vec<String> {
    let Some(baseline_ref) = session.get("repairBaselineRef").and_then(Value::as_str) else {
        return vec![];
    };
    let Ok(baseline_path) = state::paths::from_project_relative(root, baseline_ref) else {
        return vec![];
    };
    let Ok(baseline) = state::store::read_json_value(&baseline_path) else {
        return vec![];
    };
    changed_source_snapshot_paths(
        baseline.get("sourceSnapshot").unwrap_or(&Value::Null),
        &build_repair_snapshot(root, false),
    )
    .into_iter()
    .filter(|path| !path.starts_with(".loom/") && !path.starts_with(".git/"))
    .collect()
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
    let result = match read_vsefm_result(Path::new(project_root), session) {
        Ok(result) => result,
        Err(error) => return failed(project_root, "VSEFM_RESULT_READ_FAILED", error),
    };
    let subject_ref = session
        .get("subjectRef")
        .and_then(Value::as_str)
        .filter(|reference| !reference.is_empty())
        .ok_or_else(|| "V-SEFM session is missing subjectRef".to_string());
    let subject_ref = match subject_ref {
        Ok(subject_ref) => subject_ref,
        Err(error) => return failed(project_root, "VSEFM_SCOPE_BUILD_FAILED", error),
    };
    let subject_path =
        match state::paths::from_project_relative(Path::new(project_root), subject_ref) {
            Ok(path) => path,
            Err(error) => {
                return failed(project_root, "VSEFM_SCOPE_BUILD_FAILED", error.to_string())
            }
        };
    let subject = match state::store::read_json_value(&subject_path) {
        Ok(subject) => subject,
        Err(error) => return failed(project_root, "VSEFM_SCOPE_BUILD_FAILED", error.to_string()),
    };
    let scope_hints = subject
        .get("changedFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let result_file = format!(".loom/agent-writable/{repair_id}/vsefm-repair-result.json");
    let request_file = format!(".loom/verification/sessions/{verification_id}/repair-request.json");
    let baseline_ref =
        format!(".loom/verification/sessions/{verification_id}/{repair_id}-baseline.json");
    let prompt_ref = result
        .get("source")
        .and_then(|source| source.get("prompt_ref"))
        .and_then(Value::as_str)
        .filter(|prompt_ref| !prompt_ref.is_empty())
        .ok_or_else(|| "V-SEFM result is missing source.prompt_ref".to_string());
    let prompt_ref = match prompt_ref {
        Ok(prompt_ref) => prompt_ref,
        Err(error) => return failed(project_root, "VSEFM_PROMPT_UNAVAILABLE", error),
    };
    let catalog = match read_rule_catalog(prompt_ref) {
        Ok(catalog) => catalog,
        Err(error) => return failed(project_root, "VSEFM_PROMPT_INVALID", error),
    };
    let repair_findings = vsefm_result_gate_model(
        &verification_id,
        session
            .get("resultRef")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        &result,
        &catalog,
    )
    .0
    .get("confirmedFindings")
    .cloned()
    .unwrap_or_else(|| json!([]));
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
            "objective": "Fix the confirmed V-SEFM findings in the project and prepare it for re-verification.",
            "findings": repair_findings,
            "steps": [
                "Read repair_core and repair_result_contract.",
                "Read each confirmed finding and inspect the project files needed to find its root cause.",
                "Modify any ordinary project source, configuration, test, migration, build, or deployment file needed for the repair.",
                "Run bounded verification for every repaired finding; the command list is not part of the result contract.",
                "Write repair result and submit it with loom.vsefmRepairAcceptFile."
            ],
            "boundaryRules": [
                "Do not edit .loom canonical artifacts.",
                "Do not edit .git or V-SEFM verification rules and configuration.",
                "Do not claim a finding is ready without completing the implementation work."
            ]
        },
        "repairWriteBoundary": {
            "root": ".",
            "scopeHints": scope_hints.clone(),
            "protectedPaths": VSEFM_REPAIR_PROTECTED_PATHS
        },
        "outputContract": {
            "artifactKind": "vsefm_repair_result",
            "writeMode": "single_json",
            "submitTool": "loom.vsefmRepairAcceptFile",
            "resultFile": result_file,
            "agentOwnedFields": [
                "status",
                "summary",
                "details"
            ],
            "resultSchema": serde_json::to_value(schemars::schema_for!(VsefmRepairCandidate))
                .unwrap_or_else(|_| json!({"type": "object"})),
            "writeTargets": [{"targetId": "result", "path": result_file, "required": true, "description": "Write the minimal Agent-owned V-SEFM repair result."}],
            "resultTemplate": {"status": "ready", "summary": "", "details": {}}
        },
        "requestReadPlan": {"groups": [
            delivery_core::ReadGroupRef::new("repair_core", 1, vec!["agentInstruction", "source", "repairWriteBoundary"].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/repair")),
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
        object.insert("repairAttempt".to_string(), json!(1));
        object.insert(
            "repairRequestRef".to_string(),
            json!(stored.request_ref.clone()),
        );
        object.insert("repairResultFile".to_string(), json!(result_file));
        object.insert("repairBaselineRef".to_string(), json!(baseline_ref));
        object.insert("repairScopeHints".to_string(), json!(scope_hints.clone()));
        object.insert(
            "repairProtectedPaths".to_string(),
            json!(VSEFM_REPAIR_PROTECTED_PATHS),
        );
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    let state_path = session_dir.join("state.json");
    if let Err(error) = state::store::write_json_atomic(&state_path, &updated) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
    }
    if let Err(error) = sync_vsefm_record(project_root, &updated, "repairing", None) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error);
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
    let baseline_path = repair_baseline_path(Path::new(project_root), &verification_id, &repair_id);
    let baseline = json!({
        "schemaVersion": 1,
        "sourceSnapshot": build_repair_snapshot(Path::new(project_root), false),
        "controlSnapshot": build_repair_snapshot(Path::new(project_root), true),
        "scopeHints": scope_hints.clone(),
        "protectedPaths": VSEFM_REPAIR_PROTECTED_PATHS,
        "createdAt": state::store::now_string()
    });
    if let Err(error) = state::store::write_json_atomic(&baseline_path, &baseline) {
        return failed(project_root, "VSEFM_STATE_WRITE_FAILED", error.to_string());
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
            scope_hints,
            protected_paths: VSEFM_REPAIR_PROTECTED_PATHS
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
        }),
    ))
}

pub fn accept_vsefm_repair_file<D: DomainDispatcher>(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
    dispatcher: D,
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
    if let Err(error) = repair_boundary_error(root, &session, &target.path) {
        return failed(&input.project_root, "VSEFM_REPAIR_PROTECTED_CHANGE", error);
    }
    let candidate = match deserialize_vsefm_repair_candidate(raw.clone()) {
        Ok(candidate) => candidate,
        Err(issue) => {
            let repeated = match record_vsefm_repair_submit_attempt(
                root,
                &session_path,
                &session,
                &raw,
                std::slice::from_ref(&issue),
                false,
            ) {
                Ok(repeated) => repeated,
                Err(error) => {
                    return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error)
                }
            };
            if repeated {
                return finalize_vsefm_contract_fault(
                    &input.project_root,
                    &session_path,
                    &session,
                    &raw,
                    std::slice::from_ref(&issue),
                    "repair_result",
                    dispatcher,
                );
            }
            return vsefm_repair_result_repair(input, authorized, vec![issue]);
        }
    };
    let mut issues = Vec::new();
    if candidate.summary.trim().is_empty() {
        issues.push(vsefm_issue(
            "VSEFM_REPAIR_SUMMARY_REQUIRED",
            "summary",
            "V-SEFM repair must include a non-empty summary.",
        ));
    }
    if !issues.is_empty() {
        let repeated = match record_vsefm_repair_submit_attempt(
            root,
            &session_path,
            &session,
            &raw,
            &issues,
            false,
        ) {
            Ok(repeated) => repeated,
            Err(error) => return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error),
        };
        if repeated {
            return finalize_vsefm_contract_fault(
                &input.project_root,
                &session_path,
                &session,
                &raw,
                &issues,
                "repair_result",
                dispatcher,
            );
        }
        return vsefm_repair_result_repair(input, authorized, issues);
    }
    if let Err(error) =
        record_vsefm_repair_submit_attempt(root, &session_path, &session, &raw, &[], true)
    {
        return failed(&input.project_root, "VSEFM_STATE_WRITE_FAILED", error);
    }
    let changed_files = repair_changed_files(root, &session);
    let mut updated = session.clone();
    if let Some(object) = updated.as_object_mut() {
        object.insert(
            "status".to_string(),
            json!(if candidate.status == VsefmRepairStatus::Blocked {
                "repair_incomplete"
            } else {
                "reverification_started"
            }),
        );
        object.insert("repairResult".to_string(), raw.clone());
        object.insert("repairChangedFiles".to_string(), json!(changed_files));
        object.insert("updatedAt".to_string(), json!(state::store::now_string()));
    }
    if let Err(error) = state::store::write_json_atomic(&session_path, &updated) {
        return failed(
            &input.project_root,
            "VSEFM_STATE_WRITE_FAILED",
            error.to_string(),
        );
    }
    if candidate.status == VsefmRepairStatus::Blocked {
        return vsefm_repair_incomplete_gate(&input.project_root, &updated);
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
        None,
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
            "V-SEFM 验证需要人工审查，请选择 1 确认放行，或选择 2 返回自动修复。".to_string(),
        ),
        accepted_responses: vec!["1".to_string(), "2".to_string()],
        request_ref: None,
        details: Some(json!({
            "kind": "vsefm_manual_review",
            "verificationId": verification_id,
            "resultRef": result_ref,
            "options": [
                {"value": "1", "label": "确认放行", "decision": "approve_override"},
                {"value": "2", "label": "返回自动修复", "decision": "request_changes"}
            ]
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

    #[test]
    fn prompt_rule_catalog_is_the_single_check_id_source() {
        let prompt = repository_prompt_path();
        let prompt_text = std::fs::read_to_string(&prompt).expect("prompt text");
        assert!(!prompt_text.contains("Loom 本地验证输出合同"));
        assert!(!prompt_text.contains("Loom 本地验证修复输出合同"));
        assert!(!prompt_text.contains("建议的验证分类"));
        assert!(!prompt_text.contains("最终验收结果建议"));
        assert!(!prompt_text.contains("AUTH-OBJECT-001"));
        let catalog = read_rule_catalog(&prompt).expect("rule catalog");
        assert_eq!(catalog.schema_version, "1.0");
        assert_eq!(catalog.rules.len(), 17);
        assert!(catalog
            .rules
            .iter()
            .any(|rule| rule.id == "CONCURRENCY" && rule.section == "7"));
        assert!(catalog
            .rules
            .iter()
            .any(|rule| rule.id == "BROWSER-QUALITY" && rule.section == "17"));
        let ids = catalog
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), catalog.rules.len());
    }

    #[test]
    fn candidate_coverage_separates_checked_not_applicable_and_missing_rules() {
        let prompt = repository_prompt_path();
        let catalog = read_rule_catalog(&prompt).expect("rule catalog");
        let candidate: VsefmVerificationCandidate = serde_json::from_value(json!({
            "checks": [],
            "not_applicable_checks": [{
                "check_id": "TENANT-ISOLATION",
                "reason": "No tenant boundary is declared.",
                "evidence": "accepted architecture artifact"
            }],
            "environment_blocked_checks": [{
                "check_id": "CONCURRENCY",
                "attempted": "Ran the bounded concurrent-write verification.",
                "reason": "The local runner is unavailable.",
                "required_capability": "A working concurrency test runner."
            }],
            "blocking_failures": [],
            "warnings": [],
            "recommended_actions": []
        }))
        .expect("candidate");
        let (checked, not_applicable, environment_blocked, missing) =
            candidate_coverage(&catalog, &candidate);
        assert!(checked.is_empty());
        assert_eq!(not_applicable, vec!["TENANT-ISOLATION".to_string()]);
        assert_eq!(environment_blocked, vec!["CONCURRENCY".to_string()]);
        assert_eq!(missing.len(), 15);
        assert_eq!(
            canonical_vsefm_status(&candidate),
            VsefmVerificationStatus::EnvironmentBlocked
        );
    }

    #[test]
    fn repair_snapshot_excludes_nested_runtime_directories_and_classifies_runtime_files() {
        let root = std::env::temp_dir().join(format!(
            "loom-vsefm-snapshot-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::create_dir_all(root.join("api/app/__pycache__")).expect("cache directory");
        std::fs::create_dir_all(root.join("web/node_modules/.vite")).expect("node cache");
        std::fs::create_dir_all(root.join("src")).expect("source directory");
        std::fs::write(root.join("api/app/__pycache__/service.pyc"), "cache")
            .expect("python cache");
        std::fs::write(root.join("web/node_modules/.vite/deps.js"), "cache").expect("vite cache");
        std::fs::write(root.join("api/test_ticket_system.db"), "runtime database")
            .expect("runtime database");
        std::fs::write(root.join("src/main.rs"), "fn main() {}").expect("source file");

        let snapshot = build_repair_snapshot(&root, false);
        let files = snapshot["files"].as_object().expect("snapshot files");
        assert!(!files.contains_key("api/app/__pycache__/service.pyc"));
        assert!(!files.contains_key("web/node_modules/.vite/deps.js"));
        assert_eq!(
            files["api/test_ticket_system.db"]["kind"],
            "runtime_artifact"
        );
        assert_eq!(files["src/main.rs"]["kind"], "source");

        let _ = std::fs::remove_dir_all(root);
    }

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().expect("V-SEFM environment lock")
    }

    fn repository_prompt_path() -> String {
        let _guard = env_lock();
        let previous = std::env::var_os("LOOM_RUNTIME_HOME");
        std::env::set_var(
            "LOOM_RUNTIME_HOME",
            std::env::temp_dir().join(format!(
                "loom-vsefm-missing-runtime-{}-{}",
                std::process::id(),
                state::store::now_millis()
            )),
        );
        let prompt = verification_prompt_path().expect("verification prompt");
        if let Some(value) = previous {
            std::env::set_var("LOOM_RUNTIME_HOME", value);
        } else {
            std::env::remove_var("LOOM_RUNTIME_HOME");
        }
        prompt
    }

    fn restore_env(previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            std::env::set_var("LOOM_VSEFM_CONFIG", value);
        } else {
            std::env::remove_var("LOOM_VSEFM_CONFIG");
        }
    }
}
