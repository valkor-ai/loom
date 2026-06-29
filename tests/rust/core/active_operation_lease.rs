use delivery_core::{OperationLease, OperationLeaseStatus, OperationType};
use serde_json::json;

#[test]
fn operation_type_ttls_match_batch_four_contract() {
    assert_eq!(
        OperationType::TechnicalBaselineGeneration.ttl_seconds(),
        900
    );
    assert_eq!(OperationType::ArchitectureGeneration.ttl_seconds(), 1200);
    assert_eq!(OperationType::TaskExecution.ttl_seconds(), 1800);
    assert_eq!(OperationType::TaskResultRepair.ttl_seconds(), 600);
}

#[test]
fn stale_recovered_lease_keeps_identity_and_refreshes_status() {
    let mut lease = OperationLease {
        schema_version: "1.0".to_string(),
        operation_id: "op_1".to_string(),
        delivery_id: "delivery_1".to_string(),
        phase_id: "phase_1".to_string(),
        operation_type: OperationType::TaskExecution,
        status: OperationLeaseStatus::Active,
        started_at: "10".to_string(),
        heartbeat_at: "10".to_string(),
        expires_at: "11".to_string(),
        refs: json!({ "requestRef": "loom://projects/p/requests/r" }),
    };

    assert!(lease.is_fresh_at(10));
    assert!(!lease.is_fresh_at(11));

    lease.mark_stale_recovered("12");
    assert_eq!(lease.status, OperationLeaseStatus::StaleRecovered);
    assert_eq!(lease.heartbeat_at, "12");
    assert_eq!(lease.operation_id, "op_1");
}
