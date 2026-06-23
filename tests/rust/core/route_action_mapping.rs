use delivery_core::RouteActionKind;

#[test]
fn route_action_target_batches_are_stable() {
    assert_eq!(RouteActionKind::BrainstormStart.target_batch(), Some(7));
    assert_eq!(
        RouteActionKind::TechnicalBaselineRequest.target_batch(),
        Some(8)
    );
    assert_eq!(RouteActionKind::Review.target_batch(), Some(9));
    assert_eq!(RouteActionKind::TaskResultRepair.target_batch(), Some(5));
    assert_eq!(RouteActionKind::Done.target_batch(), None);
}

#[test]
fn route_action_domains_and_user_gate_flags_are_stable() {
    assert_eq!(
        RouteActionKind::BrainstormConfirmation.domain(),
        Some("brainstorm")
    );
    assert_eq!(
        RouteActionKind::TaskplanGeneration.domain(),
        Some("planning")
    );
    assert_eq!(
        RouteActionKind::ContinueExecution.domain(),
        Some("execution")
    );
    assert_eq!(
        RouteActionKind::ArchitectureArtifactRepair.domain(),
        Some("repair")
    );
    assert!(RouteActionKind::ManualReview.is_user_gate());
    assert!(!RouteActionKind::ExecutionRepair.is_user_gate());
}
