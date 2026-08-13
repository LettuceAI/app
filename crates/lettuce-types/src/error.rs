use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Cancelled,
    Conflict,
    CorruptData,
    Forbidden,
    Internal,
    InvalidInput,
    NotFound,
    NotReady,
    ResourceBusy,
    StorageUnavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl AppError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            details: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for AppError {}

#[cfg(test)]
mod tests {
    use super::{AppError, ErrorCode};

    #[test]
    fn error_builder_preserves_safe_details() {
        let error = AppError::new(ErrorCode::Conflict, "the record changed")
            .retryable(true)
            .with_detail("expected_revision", "4");

        assert!(error.retryable);
        assert_eq!(error.details["expected_revision"], "4");
    }
}
