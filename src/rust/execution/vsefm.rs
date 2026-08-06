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
    RetryRepair,
    ApproveOverride,
    RequestChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct VsefmVerificationCandidate {
    pub status: VsefmVerificationStatus,
    pub checks: Vec<VsefmCheckResult>,
    pub blocking_failures: Vec<VsefmBlockingFailure>,
    pub warnings: Vec<String>,
    pub unknown_checks: Vec<VsefmUnknownCheck>,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VsefmVerificationStatus {
    Pass,
    Blocked,
    Inconclusive,
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
    Unknown,
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
pub struct VsefmUnknownCheck {
    pub check_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct VsefmCheckPlanEntry {
    check_id: String,
    applicability: VsefmCheckApplicability,
    reason: String,
    hard_blocking: bool,
    required_evidence: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum VsefmCheckApplicability {
    Required,
    NotApplicable,
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

const VSEFM_HARD_BLOCKERS: &[&str] = &[
    "AUTH-HORIZONTAL",
    "AUTH-VERTICAL",
    "TENANT-ISOLATION",
    "IDEMPOTENCY",
    "STATE-MACHINE",
    "TRANSACTION",
];

fn collect_json_key_values<'a>(value: &'a Value, key: &str, output: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            if let Some(value) = object.get(key) {
                output.push(value);
            }
            for value in object.values() {
                collect_json_key_values(value, key, output);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_json_key_values(value, key, output);
            }
        }
        _ => {}
    }
}

fn json_key_has_non_empty_array(values: &[&Value]) -> bool {
    values
        .iter()
        .any(|value| value.as_array().is_some_and(|items| !items.is_empty()))
}

fn json_key_has_string(values: &[&Value], expected: &str) -> bool {
    values
        .iter()
        .any(|value| value.as_str().is_some_and(|actual| actual == expected))
}

fn json_key_has_object_field(values: &[&Value], field: &str, expected: &str) -> bool {
    values.iter().any(|value| {
        value
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|actual| actual == expected)
    })
}

fn json_key_has_any_object_field(values: &[&Value], fields: &[&str]) -> bool {
    values.iter().any(|value| {
        let Some(object) = value.as_object() else {
            return false;
        };
        fields.iter().any(|field| object.contains_key(*field))
    })
}

fn check_plan_entry(
    check_id: &str,
    applicable: bool,
    reason: &str,
    required_evidence: &str,
) -> Value {
    json!({
        "check_id": check_id,
        "applicability": if applicable { "required" } else { "not_applicable" },
        "reason": reason,
        "hard_blocking": VSEFM_HARD_BLOCKERS.contains(&check_id),
        "required_evidence": required_evidence
    })
}

fn derive_check_plan(accepted_artifacts: &[Value], changed_files: bool) -> Vec<Value> {
    if accepted_artifacts.is_empty() {
        return VSEFM_CHECK_IDS
            .iter()
            .map(|check_id| {
                check_plan_entry(
                    check_id,
                    true,
                    "No delivery facts are available; retain the full verifier catalog.",
                    "Read-only source and test evidence.",
                )
            })
            .collect();
    }

    let mut interfaces = Vec::new();
    let mut auth_policies = Vec::new();
    let mut operation_kinds = Vec::new();
    let mut methods = Vec::new();
    let mut state_machines = Vec::new();
    let mut data_models = Vec::new();
    let mut runtime_dependencies = Vec::new();
    let mut external_services = Vec::new();
    let mut runtime_deliveries = Vec::new();
    let mut pagination_policies = Vec::new();
    for artifact in accepted_artifacts {
        collect_json_key_values(artifact, "interfaces", &mut interfaces);
        collect_json_key_values(artifact, "authPolicy", &mut auth_policies);
        collect_json_key_values(artifact, "operationKind", &mut operation_kinds);
        collect_json_key_values(artifact, "method", &mut methods);
        collect_json_key_values(artifact, "stateMachines", &mut state_machines);
        collect_json_key_values(artifact, "dataModel", &mut data_models);
        collect_json_key_values(artifact, "runtimeDependencies", &mut runtime_dependencies);
        collect_json_key_values(artifact, "externalServices", &mut external_services);
        collect_json_key_values(artifact, "runtimeDelivery", &mut runtime_deliveries);
        collect_json_key_values(artifact, "paginationPolicy", &mut pagination_policies);
    }
    let mut dependency_kinds = Vec::new();
    for dependency in &runtime_dependencies {
        let mut kinds = Vec::new();
        collect_json_key_values(dependency, "kind", &mut kinds);
        dependency_kinds.extend(kinds.into_iter().filter_map(Value::as_str));
    }

    let has_api = json_key_has_non_empty_array(&interfaces);
    let has_auth = json_key_has_object_field(&auth_policies, "required", "required")
        || json_key_has_any_object_field(&auth_policies, &["actorRefs", "permissionRefs"]);
    let has_write = json_key_has_string(&operation_kinds, "create")
        || json_key_has_string(&operation_kinds, "update")
        || json_key_has_string(&operation_kinds, "transition")
        || json_key_has_string(&methods, "POST")
        || json_key_has_string(&methods, "PATCH")
        || json_key_has_string(&methods, "PUT")
        || json_key_has_string(&methods, "DELETE");
    let has_create =
        json_key_has_string(&operation_kinds, "create") || json_key_has_string(&methods, "POST");
    let has_state = json_key_has_non_empty_array(&state_machines)
        || json_key_has_string(&operation_kinds, "transition");
    let has_persistence = json_key_has_non_empty_array(&data_models)
        || dependency_kinds.iter().any(|kind| {
            matches!(
                kind.to_ascii_lowercase().as_str(),
                "storage" | "database" | "persistence"
            )
        });
    let has_external = external_services.iter().any(|value| {
        value
            .get("selection")
            .and_then(Value::as_str)
            .is_some_and(|selection| {
                !matches!(
                    selection.to_ascii_lowercase().as_str(),
                    "none" | "not_needed" | "不需要"
                )
            })
    }) || dependency_kinds.iter().any(|kind| {
        !matches!(
            kind.to_ascii_lowercase().as_str(),
            "storage" | "database" | "persistence"
        )
    });
    let has_runtime = json_key_has_non_empty_array(&runtime_deliveries)
        || accepted_artifacts.iter().any(|artifact| {
            artifact.get("runtimeDelivery").is_some() || artifact.get("runtimeSurfaces").is_some()
        });
    let has_list = json_key_has_string(&operation_kinds, "list")
        || json_key_has_string(&operation_kinds, "query")
        || pagination_policies.iter().any(|value| {
            value
                .get("strategy")
                .and_then(Value::as_str)
                .is_some_and(|strategy| strategy != "not_applicable")
        });
    let has_tenant = accepted_artifacts.iter().any(|artifact| {
        ["tenantId", "tenant_id", "workspaceId", "workspace_id"]
            .iter()
            .any(|key| {
                let mut values = Vec::new();
                collect_json_key_values(artifact, key, &mut values);
                !values.is_empty()
            })
    });
    let unknown_applicability = !has_api && !has_persistence && !has_runtime;

    VSEFM_CHECK_IDS
        .iter()
        .map(|check_id| match *check_id {
            "BUSINESS-INTENT" => check_plan_entry(
                check_id,
                true,
                "Every delivery has a declared business subject.",
                "Accepted requirement and acceptance evidence.",
            ),
            "AUTH-HORIZONTAL" => check_plan_entry(
                check_id,
                has_auth,
                if has_auth {
                    "The accepted architecture declares an authentication policy."
                } else {
                    "No structured authentication policy is declared."
                },
                "Identity and object ownership evidence.",
            ),
            "AUTH-VERTICAL" => check_plan_entry(
                check_id,
                has_auth && has_write,
                if has_auth && has_write {
                    "Authenticated write interfaces are declared."
                } else {
                    "No authenticated write interface is declared."
                },
                "Server-side authorization evidence for write operations.",
            ),
            "TENANT-ISOLATION" => check_plan_entry(
                check_id,
                has_tenant,
                if has_tenant {
                    "Tenant or workspace fields are present in accepted contracts."
                } else {
                    "No tenant or workspace boundary is present in accepted contracts."
                },
                "Cross-tenant access evidence.",
            ),
            "STATE-MACHINE" => check_plan_entry(
                check_id,
                has_state,
                if has_state {
                    "State-machine or transition behavior is declared."
                } else {
                    "No state-machine or transition behavior is declared."
                },
                "Legal and illegal transition evidence.",
            ),
            "IDEMPOTENCY" => check_plan_entry(
                check_id,
                has_create,
                if has_create {
                    "A create interface is declared and replay behavior must be evaluated."
                } else {
                    "No create interface is declared."
                },
                "Replay or idempotency-key evidence.",
            ),
            "CONCURRENCY" => check_plan_entry(
                check_id,
                has_persistence && has_write,
                if has_persistence && has_write {
                    "Persistent write behavior is declared."
                } else {
                    "No persistent write behavior is declared."
                },
                "Concurrent request evidence.",
            ),
            "TRANSACTION" => check_plan_entry(
                check_id,
                has_persistence && has_write,
                if has_persistence && has_write {
                    "Persistent mutations are declared."
                } else {
                    "No persistent mutation is declared."
                },
                "Commit, rollback, and atomicity evidence.",
            ),
            "DATA-INTEGRITY" => check_plan_entry(
                check_id,
                has_persistence,
                if has_persistence {
                    "A persistence model is declared."
                } else {
                    "No persistence model is declared."
                },
                "Schema and persistence evidence.",
            ),
            "API-COMPATIBILITY" => check_plan_entry(
                check_id,
                has_api,
                if has_api {
                    "HTTP interfaces are declared."
                } else {
                    "No HTTP interface is declared."
                },
                "Accepted API contract and observed responses.",
            ),
            "ERROR-RECOVERY" => check_plan_entry(
                check_id,
                has_api || has_runtime,
                if has_api || has_runtime {
                    "A runtime or HTTP surface is declared."
                } else {
                    "No runtime surface is declared."
                },
                "Actionable failure and recovery evidence.",
            ),
            "SECURITY-BOUNDARY" => check_plan_entry(
                check_id,
                has_api || has_runtime,
                if has_api || has_runtime {
                    "A callable runtime surface is declared."
                } else {
                    "No callable runtime surface is declared."
                },
                "Input, secret, and write-boundary evidence.",
            ),
            "RETRY-TIMEOUT-RATE-LIMIT" => check_plan_entry(
                check_id,
                has_external,
                if has_external {
                    "External or asynchronous dependencies are declared."
                } else {
                    "No external or asynchronous dependency is declared."
                },
                "Bounded timeout, retry, and rate-limit evidence.",
            ),
            "OBSERVABILITY-EVIDENCE" => check_plan_entry(
                check_id,
                has_api || has_runtime,
                if has_api || has_runtime {
                    "A runtime surface requires traceable evidence."
                } else {
                    "No runtime surface is declared."
                },
                "Request, mutation, and response trace evidence.",
            ),
            "REGRESSION-COMPATIBILITY" => check_plan_entry(
                check_id,
                changed_files,
                if changed_files {
                    "The subject contains changed files."
                } else {
                    "No changed files are declared."
                },
                "Existing test and compatibility evidence.",
            ),
            "PERFORMANCE-CAPACITY" => check_plan_entry(
                check_id,
                has_api && (has_list || has_persistence),
                if has_api && (has_list || has_persistence) {
                    "A query or persistent API surface is declared."
                } else {
                    "No scalable query or persistent API surface is declared."
                },
                "Bounded load or capacity evidence.",
            ),
            _ if unknown_applicability => check_plan_entry(
                check_id,
                true,
                "No structured delivery facts are available; retain the full verifier catalog.",
                "Read-only source and test evidence.",
            ),
            _ => check_plan_entry(
                check_id,
                false,
                "The accepted delivery facts do not declare this capability.",
                "No evidence required for a non-applicable check.",
            ),
        })
        .collect()
}

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
        "createdAt": state::store::now_string(),
        "updatedAt": state::store::now_string()
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
    let mut accepted_artifact_values = Vec::new();
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
                &mut accepted_artifact_values,
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
                    &mut accepted_artifact_values,
                );
            }
            add_latest_review_artifact(
                root,
                delivery_id,
                &phase,
                &mut accepted_paths,
                &mut source_refs,
                &mut accepted_artifacts,
                &mut accepted_artifact_values,
            );
            add_latest_task_result_artifacts(
                root,
                delivery_id,
                &phase,
                &mut accepted_paths,
                &mut source_refs,
                &mut accepted_artifacts,
                &mut accepted_artifact_values,
                &mut changed_files,
            );
        }
    }
    let has_changed_files = !changed_files.is_empty();
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
        "checkIds": VSEFM_CHECK_IDS,
        "checkPlan": derive_check_plan(&accepted_artifact_values, has_changed_files),
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
    accepted_artifact_values: &mut Vec<Value>,
) {
    if !accepted_paths.insert(reference.to_string()) {
        return;
    }
    let path = root.join(reference);
    let Ok(bytes) = std::fs::read(&path) else {
        accepted_paths.remove(reference);
        return;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        accepted_paths.remove(reference);
        return;
    };
    let hash = Sha256::digest(&bytes);
    source_refs.push(reference.to_string());
    accepted_artifacts.push(json!({
        "path": reference,
        "role": role,
        "phase_id": phase_id,
        "sha256": format!("{hash:x}"),
        "bytes": bytes.len()
    }));
    accepted_artifact_values.push(value);
}

fn add_latest_review_artifact(
    root: &Path,
    delivery_id: &str,
    phase_id: &str,
    accepted_paths: &mut BTreeSet<String>,
    source_refs: &mut Vec<String>,
    accepted_artifacts: &mut Vec<Value>,
    accepted_artifact_values: &mut Vec<Value>,
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
        accepted_artifact_values,
    );
}

fn add_latest_task_result_artifacts(
    root: &Path,
    delivery_id: &str,
    phase_id: &str,
    accepted_paths: &mut BTreeSet<String>,
    source_refs: &mut Vec<String>,
    accepted_artifacts: &mut Vec<Value>,
    accepted_artifact_values: &mut Vec<Value>,
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
            accepted_artifact_values,
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
    let check_plan = subject
        .get("checkPlan")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let required_check_ids = check_plan
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("applicability").and_then(Value::as_str) == Some("required"))
        .filter_map(|entry| entry.get("check_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let result_schema = serde_json::to_value(schemars::schema_for!(VsefmVerificationCandidate))
        .unwrap_or_else(|_| json!({"type": "object"}));
    let instruction = json!({
        "role": "software_delivery_verifier",
        "objective": "Verify the declared delivery subject against sefm-verify.md without modifying product or Loom files.",
        "steps": [
            "Read verification_execution_core, verification_prompt, verification_subject, and verification_result_contract.",
            "Read sefm-verify.md from promptRef.",
            "Read only the files listed by subject.changedFiles and subject.acceptedArtifacts; these are the complete accepted inputs for this verification.",
            "Evaluate only checkPlan entries whose applicability is required. Do not emit checks for not_applicable entries.",
            "Record concrete input, expected, observed, evidence, and timestamp for every required check.",
            "Use unknown when a required check cannot be established; include its reason in unknown_checks.",
            "Create one blocking_failure per distinct finding and reference the failed check with check_id; do not duplicate check evidence in blocking_failures.",
            "Write the result candidate and submit it with loom.vsefmVerificationAcceptFile."
        ],
        "hardBlockingRules": [
            "A failed checkPlan entry with hard_blocking=true requires status=blocked and a blocking_failure reference.",
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
            "requiredCheckIds": required_check_ids,
            "checkPlan": check_plan
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
            "agentOwnedFields": [
                "status",
                "checks",
                "blocking_failures",
                "warnings",
                "unknown_checks",
                "recommended_actions"
            ],
            "mcpOwnedFields": [
                "artifact_id",
                "verification_id",
                "scope",
                "source",
                "check_plan",
                "statistics",
                "attempts"
            ],
            "resultSchema": result_schema,
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
                    "prompt", "prompt.ref", "prompt.requiredCheckIds", "prompt.checkPlan"
                ].into_iter().map(str::to_string).collect(), format!("loom://vsefm/{verification_id}/prompt")),
                delivery_core::ReadGroupRef::new("verification_subject", 3, vec![
                    "subject", "subject.scope", "subject.phaseIds", "subject.requirementRefs", "subject.acceptedArtifacts", "subject.changedFiles", "subject.checkPlan"
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
    let subject_ref = match session.get("subjectRef").and_then(Value::as_str) {
        Some(reference) => reference,
        None => {
            return failed(
                &input.project_root,
                "VSEFM_SCOPE_BUILD_FAILED",
                "V-SEFM session is missing subjectRef.",
            )
        }
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
    let check_plan = match subject.get("checkPlan").cloned() {
        Some(check_plan) => check_plan,
        None => {
            return failed(
                &input.project_root,
                "VSEFM_SCOPE_BUILD_FAILED",
                "V-SEFM subject is missing generated checkPlan.",
            )
        }
    };
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
    let issues = validate_vsefm_candidate(&candidate, &check_plan);
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
    }
}

fn validate_vsefm_candidate(
    candidate: &VsefmVerificationCandidate,
    check_plan: &Value,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut check_statuses = std::collections::BTreeMap::new();
    let plan = match serde_json::from_value::<Vec<VsefmCheckPlanEntry>>(check_plan.clone()) {
        Ok(plan) => plan,
        Err(error) => {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_PLAN_INVALID",
                "checkPlan",
                &format!("The MCP-generated check plan is invalid: {error}"),
            ));
            return issues;
        }
    };
    let plan_by_id = plan
        .iter()
        .map(|entry| (entry.check_id.as_str(), entry))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (index, check) in candidate.checks.iter().enumerate() {
        let field_prefix = format!("checks[{index}]");
        let Some(plan_entry) = plan_by_id.get(check.check_id.as_str()) else {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_ID_INVALID",
                &format!("{field_prefix}.check_id"),
                "Check id is not in the generated check plan.",
            ));
            continue;
        };
        if plan_entry.applicability != VsefmCheckApplicability::Required {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_NOT_APPLICABLE",
                &format!("{field_prefix}.check_id"),
                "Do not emit a check whose generated applicability is not_applicable.",
            ));
        }
        if !seen.insert(check.check_id.clone()) {
            issues.push(vsefm_issue(
                "VSEFM_CHECK_DUPLICATE",
                &format!("{field_prefix}.check_id"),
                "Each canonical check id must appear once.",
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
                "Each check requires non-empty input, expected, and observed values.",
            ));
        }
        if check.evidence.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_EVIDENCE_REQUIRED",
                &format!("{field_prefix}.evidence"),
                "Every check requires evidence.",
            ));
        }
        if check.timestamp.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_TIMESTAMP_REQUIRED",
                &format!("{field_prefix}.timestamp"),
                "Every check requires a non-empty evidence timestamp.",
            ));
        }
        if plan_entry.hard_blocking
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
    let missing = plan
        .iter()
        .filter(|entry| {
            entry.applicability == VsefmCheckApplicability::Required
                && !seen.contains(&entry.check_id)
        })
        .map(|entry| entry.check_id.as_str())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        issues.push(vsefm_issue(
            "VSEFM_CHECK_COVERAGE_INCOMPLETE",
            "checks",
            &format!("Missing generated required checks: {}", missing.join(", ")),
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
    for check in &candidate.checks {
        let Some(plan_entry) = plan_by_id.get(check.check_id.as_str()) else {
            continue;
        };
        let has_finding = candidate
            .blocking_failures
            .iter()
            .any(|failure| failure.check_id == check.check_id);
        if plan_entry.hard_blocking && check.status == VsefmCheckStatus::Fail && !has_finding {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_FAILURE_MISSING",
                "blocking_failures",
                &format!(
                    "Hard-blocking failed check {} requires a blocking failure reference.",
                    check.check_id
                ),
            ));
        }
    }
    let mut unknown_ids = BTreeSet::new();
    for (index, unknown) in candidate.unknown_checks.iter().enumerate() {
        let field_prefix = format!("unknown_checks[{index}]");
        if !plan_by_id.contains_key(unknown.check_id.as_str()) {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_ID_INVALID",
                &format!("{field_prefix}.check_id"),
                "Unknown check id is not in the generated check plan.",
            ));
        } else if plan_by_id[unknown.check_id.as_str()].applicability
            != VsefmCheckApplicability::Required
        {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_NOT_APPLICABLE",
                &format!("{field_prefix}.check_id"),
                "Unknown checks may only reference a generated required check.",
            ));
        }
        if !unknown_ids.insert(unknown.check_id.clone()) || unknown.reason.trim().is_empty() {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_INVALID",
                &field_prefix,
                "Each unknown check must be unique and explain why it could not be established.",
            ));
        }
        if check_statuses.get(&unknown.check_id) != Some(&VsefmCheckStatus::Unknown) {
            issues.push(vsefm_issue(
                "VSEFM_UNKNOWN_CHECK_STATUS_INVALID",
                &format!("{field_prefix}.check_id"),
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
    let mut finding_ids = BTreeSet::new();
    for (index, failure) in candidate.blocking_failures.iter().enumerate() {
        let field_prefix = format!("blocking_failures[{index}]");
        let Some(plan_entry) = plan_by_id.get(failure.check_id.as_str()) else {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_CHECK_INVALID",
                &format!("{field_prefix}.check_id"),
                "Blocking failure must reference a generated check plan entry.",
            ));
            continue;
        };
        if check_statuses.get(&failure.check_id) != Some(&VsefmCheckStatus::Fail) {
            issues.push(vsefm_issue(
                "VSEFM_BLOCKING_CHECK_UNSUPPORTED",
                &format!("{field_prefix}.check_id"),
                "Blocking failure must reference a failed check.",
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
        if plan_entry.hard_blocking && candidate.status != VsefmVerificationStatus::Blocked {
            issues.push(vsefm_issue(
                "VSEFM_HARD_BLOCKER_STATUS_INVALID",
                "status",
                "A hard-blocking finding requires status=blocked.",
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
    if candidate.status == VsefmVerificationStatus::Pass && !candidate.blocking_failures.is_empty()
    {
        issues.push(vsefm_issue(
            "VSEFM_STATUS_INCONSISTENT",
            "status",
            "A pass result cannot contain blocking failures.",
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
    let subject =
        state::store::read_json_value(&subject_path).map_err(|error| error.to_string())?;
    let check_plan = subject
        .get("checkPlan")
        .cloned()
        .ok_or_else(|| "V-SEFM subject is missing generated checkPlan".to_string())?;
    serde_json::from_value::<Vec<VsefmCheckPlanEntry>>(check_plan.clone())
        .map_err(|error| format!("V-SEFM subject checkPlan is invalid: {error}"))?;
    let passed_checks = candidate
        .checks
        .iter()
        .filter(|check| check.status == VsefmCheckStatus::Pass)
        .count();
    Ok(json!({
        "schema_version": "1.0",
        "artifact_id": verification_id,
        "verification_id": verification_id,
        "status": candidate.status,
        "checks": candidate.checks,
        "blocking_failures": candidate.blocking_failures,
        "warnings": candidate.warnings,
        "unknown_checks": candidate.unknown_checks,
        "recommended_actions": candidate.recommended_actions,
        "check_plan": check_plan,
        "passed_checks": passed_checks,
        "failed_checks": candidate
            .checks
            .iter()
            .filter(|check| check.status == VsefmCheckStatus::Fail)
            .count(),
        "warning_count": candidate.warnings.len(),
        "unknown_count": candidate.unknown_checks.len(),
        "attempts": session.get("attempt").cloned().unwrap_or_else(|| json!(1)),
        "source": {
            "delivery_id": session.get("deliveryId"),
            "phase_id": session.get("phaseId"),
            "scope": session.get("scope"),
            "subject_ref": session.get("subjectRef"),
            "prompt_ref": session.get("promptRef"),
            "subject_sha256": format!("{subject_hash:x}")
        },
        "created_at": state::store::now_string()
    }))
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
    let choices = vec!["1".to_string(), "2".to_string()];
    let options = if status == "pass" {
        json!([
            {"value": "1", "label": "接受验证结果", "decision": "accept"},
            {"value": "2", "label": "转人工审查", "decision": "manual_review"}
        ])
    } else {
        json!([
            {"value": "1", "label": "让 Agent 自动修复", "decision": "repair"},
            {"value": "2", "label": "转人工审查", "decision": "manual_review"}
        ])
    };
    LoomMcpActionResult::UserGate(LoomMcpUserGateResult::new(
        project_root.to_string(),
        format!("V-SEFM 本地验证结果：{status}。请选择后续处理方式：\n结果文件：{result_ref}"),
        choices,
        None,
        delivery_id.map(str::to_string),
        phase_id.map(str::to_string),
        Some(json!({"kind": "vsefm_result", "verificationId": verification_id, "resultRef": result_ref, "status": status, "blockingFailures": result.get("blocking_failures").cloned().unwrap_or_else(|| json!([])), "recommendedActions": result.get("recommended_actions").cloned().unwrap_or_else(|| json!([])), "options": options})),
    ).with_agent_instruction("展示 V-SEFM 验证结果摘要和选项。用户选择 1 或 2 后，将序号映射为 gate.options 中的 decision，再调用 loom.vsefmVerificationResolve；不要调用 loom.continue、loom.inspectRequest 或知识工具。"))
}

const VSEFM_REPAIR_PROTECTED_PATHS: &[&str] = &[
    ".loom",
    ".git",
    "plugins/shared/loom/references/verification/sefm-verify.md",
    "plugins/shared/loom/references/verification/v-sefm.json",
];

fn repair_snapshot_file(root: &Path, path: &Path) -> Option<Value> {
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
    Some(json!({
        "sha256": format!("{digest:x}"),
        "bytes": bytes.len(),
        "path": relative
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
            if relative == ".git"
                || relative == "node_modules"
                || relative == "target"
                || relative == "dist"
                || relative == "build"
                || relative == ".venv"
                || relative == "__pycache__"
                || (!include_control_tree && relative == ".loom")
            {
                continue;
            }
            collect_repair_snapshot(root, &path, include_control_tree, files);
        } else if let Some(snapshot) = repair_snapshot_file(root, &path) {
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
    let source_changes = changed_snapshot_paths(
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
    changed_snapshot_paths(
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
            "objective": "Fix the blocking V-SEFM findings in the project and prepare it for re-verification.",
            "findings": result.get("blocking_failures").cloned().unwrap_or_else(|| json!([])),
            "steps": [
                "Read repair_core and repair_result_contract.",
                "Read each blocking finding and inspect the project files needed to find its root cause.",
                "Modify any ordinary project source, configuration, test, migration, build, or deployment file needed for the repair.",
                "Run bounded verification for the repaired findings; the command list is not part of the result contract.",
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
    fn check_plan_uses_structured_delivery_facts() {
        let artifacts = vec![json!({
            "authPolicy": {"required": "required"},
            "interfaces": [{"method": "POST", "path": "/items"}],
            "dataModel": [{"name": "item"}],
            "runtimeDependencies": [{"kind": "cache", "provider": "redis"}],
            "tenantId": "workspace_id"
        })];
        let plan = derive_check_plan(&artifacts, true);
        let entry = |check_id: &str| {
            plan.iter()
                .find(|entry| entry["check_id"] == check_id)
                .expect("check plan entry")
        };
        assert_eq!(entry("AUTH-HORIZONTAL")["applicability"], "required");
        assert_eq!(entry("AUTH-VERTICAL")["applicability"], "required");
        assert_eq!(entry("TENANT-ISOLATION")["applicability"], "required");
        assert_eq!(entry("DATA-INTEGRITY")["applicability"], "required");
        assert_eq!(
            entry("RETRY-TIMEOUT-RATE-LIMIT")["applicability"],
            "required"
        );
        assert_eq!(entry("STATE-MACHINE")["applicability"], "not_applicable");
        assert!(plan
            .iter()
            .all(|entry| entry.get("reason").and_then(Value::as_str).is_some()));
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
