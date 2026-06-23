pub mod action_result;
pub mod context;
pub mod error;
pub mod next_action;

pub use action_result::*;
pub use context::*;
pub use error::*;
pub use next_action::*;

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
