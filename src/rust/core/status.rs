use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{OperationLease, RouteAction};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryLifecycleStatus {
    Planning,
    Executing,
    Reviewing,
    Repairing,
    Completed,
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
        DeliveryLifecycleStatus::Completed => {
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
    let next_action = active_delivery
        .and_then(|delivery| current_phase(delivery))
        .and_then(|phase| phase.next_action.as_ref())
        .map(route_action_summary);
    let active_operation = active_operation.map(operation_summary);
    json!({
        "initialized": true,
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
