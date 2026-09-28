use serde::{Deserialize, Serialize};

/// Stable error category the frontend localizes; the message never reaches
/// the user. `ModelRequired` means the chat needs an optional model that is
/// not installed, `ModelUnavailable` one that is installed but cannot load;
/// both name the model in `ApiErrorDetails::Model`.
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
    ModelRequired,
    ModelUnavailable,
}

/// An optional model some chats need: the embedding model for dynamic
/// memory, the emotion model (Lettuce Thymos) for companion chats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RequiredModel {
    Embedding,
    Emotion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApiErrorDetails {
    InvalidField {
        field: String,
    },
    Model {
        model: RequiredModel,
    },
    PendingMemoryRewind {
        conversation_id: String,
    },
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
/// `ConversationChanged` follows a committed write to the conversation (lists
/// and open views re-read it), `ConversationRemoved` its purge, and
/// `RequiredModelsChanged` an optional model's install, switch, removal or
/// adoption (open views re-read their missing models).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApiEvent {
    GenerationSettled {
        conversation_id: String,
        turn_id: String,
    },
    JobUpdated {
        job: crate::JobView,
    },
    ConversationChanged {
        conversation_id: String,
    },
    ConversationRemoved {
        conversation_id: String,
    },
    RequiredModelsChanged,
}

/// A stored media asset and the URL the host serves it at; the UI loads
/// `url` as is and never builds one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AssetRef {
    pub asset_id: String,
    pub url: String,
}
