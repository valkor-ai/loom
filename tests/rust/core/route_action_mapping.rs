use delivery_core::{HostKind, LoomError, LoomMcpRuntimeContext, RouteActionKind};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn error_shape_is_stable() {
    let error = LoomError::new("SMOKE", "core is reachable");
    assert_eq!(error.code, "SMOKE");
    assert_eq!(error.message, "core is reachable");
}

#[test]
fn runtime_context_reads_only_current_host_env_name() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let original_host = std::env::var("LOOM_HOST").ok();
    let original_legacy_host = std::env::var("LOOM_MCP_HOST").ok();
    std::env::remove_var("LOOM_HOST");
    std::env::set_var("LOOM_MCP_HOST", "claude-code");

    let context = LoomMcpRuntimeContext::from_env();

    restore_env("LOOM_HOST", original_host);
    restore_env("LOOM_MCP_HOST", original_legacy_host);
    assert_eq!(context.host, HostKind::Codex);
}

fn restore_env(key: &str, value: Option<String>) {
    if let Some(value) = value {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

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
