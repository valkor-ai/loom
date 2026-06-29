use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
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

pub type LoomResult<T> = Result<T, LoomCoreError>;

#[derive(Debug, Error)]
pub enum LoomCoreError {
    #[error("{0}: {1}")]
    Failure(String, String),
}

impl LoomCoreError {
    pub fn failure(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Failure(code.into(), message.into())
    }

    pub fn code(&self) -> &str {
        match self {
            Self::Failure(code, _) => code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Failure(_, message) => message,
        }
    }
}
