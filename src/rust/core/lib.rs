use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoomError {
    pub code: String,
    pub message: String,
}

impl LoomError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub type LoomResult<T> = Result<T, LoomError>;

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
