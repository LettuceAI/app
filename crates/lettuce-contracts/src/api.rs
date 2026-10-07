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

/// Why a Hugging Face request failed, for the UI to act on: ask for a
/// token, replace it, accept the repository's license, or wait.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HfFailure {
    TokenMissing,
    TokenInvalid,
    GatedAccess { model_id: String },
    NotFound,
    RateLimited,
    Offline,
}

/// Why an Ollama server request failed: it could not be reached (worth a
/// retry), the account's credentials could not be read or were refused, the
/// server answered with its own error, or a pull ended before it completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OllamaFailure {
    Offline,
    CredentialsUnavailable,
    CredentialsRefused,
    ServerError { message: String },
    Incomplete,
}

/// What keeps the local models folder busy: an install into it, a move of
/// it, or a model llama.cpp holds open from it (the UI offers to unload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalModelsBusyReason {
    InstallActive { job_id: String },
    SpeechWorkActive { job_id: String },
    FolderMoveActive { job_id: String },
    ModelLoaded { path: String },
    ImageWorkActive { job_id: Option<String> },
}

/// Why a branch cannot be deleted: it is the conversation's first branch or
/// the one currently selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum BranchDeleteRefusal {
    RootBranch,
    SelectedBranch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApiErrorDetails {
    InvalidField {
        field: String,
    },
    CapturedAudio {
        audio: crate::AssetRef,
    },
    AudioProviderInUse {
        characters: Vec<CharacterReferenceView>,
    },
    OperationAppliedRecordDeleted {
        command: String,
        record_id: String,
    },
    Model {
        model: RequiredModel,
    },
    HuggingFace {
        failure: HfFailure,
    },
    Ollama {
        failure: OllamaFailure,
    },
    LocalModelsBusy {
        reason: LocalModelsBusyReason,
    },
    Image {
        failure: crate::ImageFailureKind,
    },
    Speech {
        failure: crate::SpeechFailure,
    },
    PendingMemoryRewind {
        conversation_id: String,
    },
    BranchDeleteRefused {
        reason: BranchDeleteRefusal,
    },
    MemoryGate {
        gate: crate::MemoryGateReason,
    },
    MemoryCycleDependent {
        later_run_id: String,
    },
    MemoryCycleUserEdited {
        memory_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterReferenceView {
    pub id: String,
    pub name: String,
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
/// adoption (open views re-read their missing models). `MessageEffectSettled`
/// and `MessageSceneImageChanged` follow a message's companion effect and
/// scene image follow-up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApiEvent {
    GenerationSettled {
        conversation_id: String,
        turn_id: String,
    },
    JobUpdated {
        job: Box<crate::JobView>,
    },
    ConversationChanged {
        conversation_id: String,
    },
    ConversationRemoved {
        conversation_id: String,
    },
    /// What `memory_get` shows for the conversation changed: its items,
    /// summary, revision, cycle, approval or dismissal state. Every chat that
    /// shares the memory gets one.
    MemoryChanged {
        conversation_id: String,
    },
    RequiredModelsChanged,
    /// The companion effect of a reply settled (`message_companion_effect`
    /// reads it).
    MessageEffectSettled {
        conversation_id: String,
        message_id: String,
    },
    /// A reply's scene image follow-up changed state.
    MessageSceneImageChanged {
        conversation_id: String,
        message_id: String,
    },
    /// The input level of a running dictation in thousandths, from 0 to
    /// 1000. Sent only while it captures, at most about every 50 ms.
    DictationLevel {
        capture_id: String,
        level: u16,
    },
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
