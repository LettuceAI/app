use serde::{Deserialize, Serialize};

/// Stable error category the frontend localizes; the message never reaches
/// the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    NotFound,
    Conflict,
    InvalidInput,
    Unsupported,
    Unavailable,
    Cancelled,
    Busy,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApiErrorDetails {
    InvalidField { field: String },
}

/// The error every API call returns. `message` is English diagnostic text
/// for logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    pub details: Option<ApiErrorDetails>,
}

/// Application-wide events the host broadcasts to every window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApiEvent {
    GenerationSettled {
        conversation_id: String,
        turn_id: String,
    },
}

/// A stored media asset, served by the host under its asset URI scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AssetRef {
    pub asset_id: String,
}
