use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    LoomMcpActionResult, LoomMcpRepairableErrorResult, OperationLease, ReadGroupRef, RepairIssue,
    RouteAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryLifecycleStatus {
    Planning,
    Executing,
    Reviewing,
    Repairing,
    Completed,
    CompletedWithOverride,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryPhaseState {
    pub phase_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub latest_refs: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<RouteAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_repair: Option<PendingRepair>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingRepair {
    pub request_ref: String,
    pub target_file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_ids: Vec<String>,
    pub issues: Vec<RepairIssue>,
    pub resubmit_tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_groups: Vec<ReadGroupRef>,
}

impl PendingRepair {
    pub fn from_result(
        request_ref: impl Into<String>,
        result: &LoomMcpRepairableErrorResult,
    ) -> Self {
        Self {
            request_ref: request_ref.into(),
            target_file: result.target_file.clone(),
            target_ids: result.target_ids.clone(),
            issues: result.issues.clone(),
            resubmit_tool: result.resubmit_tool.clone(),
            fix_scope: result.fix_scope.clone(),
            read_groups: result.read_groups.clone(),
        }
    }

    pub fn to_result(&self, project_root: impl Into<String>) -> LoomMcpActionResult {
        LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
            project_root: project_root.into(),
            stop_allowed: false,
            target_file: self.target_file.clone(),
            target_ids: self.target_ids.clone(),
            issues: self.issues.clone(),
            resubmit_tool: self.resubmit_tool.clone(),
            fix_scope: self.fix_scope.clone(),
            read_groups: self.read_groups.clone(),
            agent_instruction: crate::repairable_error_agent_instruction(&self.resubmit_tool),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryIndex {
    pub schema_version: u32,
    pub delivery_id: String,
    pub active_phase_id: String,
    pub status: DeliveryLifecycleStatus,
    pub phases: Vec<DeliveryPhaseState>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryStatusEntry {
    pub delivery_id: String,
    pub active_phase_id: Option<String>,
    pub status: DeliveryLifecycleStatus,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub schema_version: u32,
    pub active_delivery_id: Option<String>,
    pub last_completed_delivery_id: Option<String>,
    pub deliveries: Vec<DeliveryStatusEntry>,
    pub updated_at: String,
}

impl ProjectStatus {
    pub fn empty(now: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            active_delivery_id: None,
            last_completed_delivery_id: None,
            deliveries: vec![],
            updated_at: now.into(),
        }
    }
}

pub fn apply_delivery_index(status: &mut ProjectStatus, delivery: &DeliveryIndex) {
    let entry = DeliveryStatusEntry {
        delivery_id: delivery.delivery_id.clone(),
        active_phase_id: Some(delivery.active_phase_id.clone()),
        status: delivery.status.clone(),
        updated_at: delivery.updated_at.clone(),
    };
    if let Some(existing) = status
        .deliveries
        .iter_mut()
        .find(|existing| existing.delivery_id == delivery.delivery_id)
    {
        *existing = entry;
    } else {
        status.deliveries.push(entry);
    }
    match delivery.status {
        DeliveryLifecycleStatus::Completed | DeliveryLifecycleStatus::CompletedWithOverride => {
            status.active_delivery_id = None;
            status.last_completed_delivery_id = Some(delivery.delivery_id.clone());
        }
        _ => {
            status.active_delivery_id = Some(delivery.delivery_id.clone());
        }
    }
    status.updated_at = delivery.updated_at.clone();
}

pub fn status_details(
    status: &ProjectStatus,
    active_delivery: Option<&DeliveryIndex>,
    active_operation: Option<&OperationLease>,
    warnings: &[String],
) -> Value {
    let active_phase_id = active_delivery.map(|delivery| delivery.active_phase_id.clone());
    let delivery_status = active_delivery.map(|delivery| delivery.status.clone());
    let workflow_state = active_delivery
        .map(|delivery| json!(delivery.status.clone()))
        .unwrap_or_else(|| json!("idle"));
    let next_action = active_delivery
        .and_then(|delivery| current_phase(delivery))
        .and_then(|phase| phase.next_action.as_ref())
        .map(route_action_summary);
    let active_operation = active_operation.map(operation_summary);
    json!({
        "initialized": true,
        "workflowState": workflow_state,
        "hasActiveWorkflow": active_delivery.is_some(),
        "activeDeliveryId": status.active_delivery_id,
        "lastCompletedDeliveryId": status.last_completed_delivery_id,
        "activePhaseId": active_phase_id,
        "deliveryStatus": delivery_status,
        "nextAction": next_action,
        "activeOperation": active_operation,
        "warnings": warnings,
    })
}

pub fn current_phase(delivery: &DeliveryIndex) -> Option<&DeliveryPhaseState> {
    delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == delivery.active_phase_id)
}

pub fn route_action_summary(action: &RouteAction) -> Value {
    json!({
        "type": action.kind,
        "reason": action.reason,
        "targetPhaseId": action.target_phase_id,
    })
}

pub fn operation_summary(lease: &OperationLease) -> Value {
    json!({
        "operationId": lease.operation_id,
        "operationType": lease.operation_type,
        "deliveryId": lease.delivery_id,
        "phaseId": lease.phase_id,
        "expiresAt": lease.expires_at,
        "status": lease.status,
    })
}
