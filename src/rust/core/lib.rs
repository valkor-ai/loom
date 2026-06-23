pub mod action_result;
pub mod context;
pub mod domain_dispatcher;
pub mod error;
pub mod next_action;
pub mod operation_lease;
pub mod project_lifecycle;
pub mod read_protocol;
pub mod route_action;
pub mod status;
pub mod transition;
pub mod transition_diagnostics;

pub use action_result::*;
pub use context::*;
pub use domain_dispatcher::*;
pub use error::*;
pub use next_action::*;
pub use operation_lease::*;
pub use project_lifecycle::*;
pub use read_protocol::*;
pub use route_action::*;
pub use status::*;
pub use transition::*;
pub use transition_diagnostics::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_shape_is_stable() {
        let error = LoomError::new("SMOKE", "core is reachable");
        assert_eq!(error.code, "SMOKE");
        assert_eq!(error.message, "core is reachable");
    }
}
