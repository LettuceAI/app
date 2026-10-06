//! The one place domain and runtime errors become `ApiError`.

use lettuce_contracts::{ApiError, ApiErrorCode, ApiErrorDetails, RequiredModel};
use lettuce_conversations::{ConversationRepositoryError, ValidationError};
use lettuce_jobs::StoreError;
use lettuce_media::MediaStoreError;

use crate::{
    CompanionTurnError, ConversationGenerationCancellationError,
    ConversationGenerationDispatchError, ConversationGenerationWorkerError,
    ConversationLaunchError,
};

pub(crate) fn api_error(code: ApiErrorCode, message: impl Into<String>) -> ApiError {
    ApiError {
        code,
        message: message.into(),
        details: None,
    }
}

pub(crate) fn invalid_field(field: &str, message: impl Into<String>) -> ApiError {
    ApiError {
        code: ApiErrorCode::InvalidInput,
        message: message.into(),
        details: Some(ApiErrorDetails::InvalidField {
            field: field.to_owned(),
        }),
    }
}

/// A chat needs `model`: `ModelRequired` when it is not installed,
/// `ModelUnavailable` when it cannot load.
pub(crate) fn model_error(code: ApiErrorCode, model: RequiredModel) -> ApiError {
    let what = match model {
        RequiredModel::Embedding => "the embedding model",
        RequiredModel::Emotion => "the emotion model",
    };
    let message = if code == ApiErrorCode::ModelRequired {
        format!("{what} is not installed")
    } else {
        format!("{what} cannot be loaded")
    };
    ApiError {
        code,
        message,
        details: Some(ApiErrorDetails::Model { model }),
    }
}

pub(crate) fn hf_failure(failure: &lettuce_model_hub::HfFailure) -> lettuce_contracts::HfFailure {
    use lettuce_contracts::HfFailure as Dto;
    use lettuce_model_hub::HfFailure;
    match failure {
        HfFailure::TokenMissing => Dto::TokenMissing,
        HfFailure::TokenInvalid => Dto::TokenInvalid,
        HfFailure::GatedAccess { model_id } => Dto::GatedAccess {
            model_id: model_id.clone(),
        },
        HfFailure::NotFound => Dto::NotFound,
        HfFailure::RateLimited => Dto::RateLimited,
        HfFailure::Offline => Dto::Offline,
    }
}

/// A Hugging Face error: typed details when the failure is one the UI can
/// act on (sign in, accept a license, wait, go online).
pub(crate) fn hf_error(error: lettuce_model_hub::HfBrowseError) -> ApiError {
    use lettuce_model_hub::HfFailure;
    let message = error.to_string();
    match error.failure() {
        Some(failure) => ApiError {
            code: match failure {
                HfFailure::NotFound => ApiErrorCode::NotFound,
                HfFailure::RateLimited => ApiErrorCode::Busy,
                HfFailure::TokenMissing
                | HfFailure::TokenInvalid
                | HfFailure::GatedAccess { .. }
                | HfFailure::Offline => ApiErrorCode::Unavailable,
            },
            message,
            details: Some(ApiErrorDetails::HuggingFace {
                failure: hf_failure(failure),
            }),
        },
        None => api_error(ApiErrorCode::Unavailable, message),
    }
}

/// Parses a contract id, naming the request field when it is not a UUID.
pub(crate) fn parse_id<T: std::str::FromStr>(value: &str, field: &str) -> Result<T, ApiError> {
    value
        .parse()
        .map_err(|_| invalid_field(field, format!("{field} is not a valid id")))
}

pub(crate) trait IntoApiError {
    fn into_api_error(self) -> ApiError;
}

impl IntoApiError for ValidationError {
    fn into_api_error(self) -> ApiError {
        let field = match &self {
            Self::ZeroRevision => "revision",
            Self::Blank { field }
            | Self::TooLarge { field }
            | Self::TooMany { field, .. }
            | Self::Duplicate { field }
            | Self::InvalidValue { field }
            | Self::InvalidReference { field }
            | Self::UnsupportedVersion { field, .. }
            | Self::InvalidTimestampOrder { field }
            | Self::Invariant { field }
            | Self::IllegalTransition { field }
            | Self::OutOfBounds { field } => field,
        };
        invalid_field(field, self.to_string())
    }
}

impl IntoApiError for ConversationRepositoryError {
    fn into_api_error(self) -> ApiError {
        let code = match &self {
            Self::Busy => ApiErrorCode::Busy,
            Self::NotFound => ApiErrorCode::NotFound,
            Self::StaleRevision { .. }
            | Self::Conflict
            | Self::JobAlreadyAttached
            | Self::JobInUse
            | Self::Dependency => ApiErrorCode::Conflict,
            Self::Invalid(error) => return error.clone().into_api_error(),
            Self::Unsupported => ApiErrorCode::Unsupported,
            Self::ArtifactReference(_) | Self::Storage => ApiErrorCode::Internal,
        };
        api_error(code, self.to_string())
    }
}

impl IntoApiError for lettuce_characters::RepositoryError {
    fn into_api_error(self) -> ApiError {
        let code = match &self {
            Self::NotFound => ApiErrorCode::NotFound,
            Self::AlreadyExists
            | Self::StaleRevision { .. }
            | Self::MissingDefaultRevision
            | Self::Archived
            | Self::AlreadyActive
            | Self::HasDependencies => ApiErrorCode::Conflict,
            Self::Invalid(_) => ApiErrorCode::InvalidInput,
            Self::Storage => ApiErrorCode::Internal,
        };
        api_error(code, self.to_string())
    }
}

impl IntoApiError for CompanionTurnError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Conversation(error) => error.into_api_error(),
            Self::Character(error) => error.into_api_error(),
            Self::CharacterMissing => api_error(ApiErrorCode::NotFound, self.to_string()),
            Self::Cancelled => api_error(ApiErrorCode::Cancelled, self.to_string()),
            Self::EmotionUnavailable => {
                model_error(ApiErrorCode::ModelUnavailable, RequiredModel::Emotion)
            }
            Self::EmotionRequired => {
                model_error(ApiErrorCode::ModelRequired, RequiredModel::Emotion)
            }
            Self::State(lettuce_companions::CompanionStateRepositoryError::Conflict)
            | Self::State(lettuce_companions::CompanionStateRepositoryError::OperationMismatch) => {
                api_error(ApiErrorCode::Conflict, self.to_string())
            }
            Self::Send(lettuce_companions::CompanionSendRepositoryError::Conversation(error)) => {
                error.into_api_error()
            }
            Self::Continue(_) | Self::State(_) | Self::Send(_) => {
                api_error(ApiErrorCode::Internal, self.to_string())
            }
        }
    }
}

impl IntoApiError for ConversationLaunchError {
    fn into_api_error(self) -> ApiError {
        use crate::LaunchSourceError as Source;
        let code = match &self {
            Self::InvalidRequest { field } => {
                return invalid_field(field, self.to_string());
            }
            Self::SceneNotOwned { .. } => return invalid_field("scene_id", self.to_string()),
            Self::StarterNotOwned { .. } => return invalid_field("starter_id", self.to_string()),
            Self::CharacterNotFound { .. }
            | Self::GroupNotFound { .. }
            | Self::MemberCharacterNotFound { .. }
            | Self::SceneNotFound { .. }
            | Self::StarterNotFound { .. }
            | Self::PersonaNotFound { .. }
            | Self::PromptNotFound { .. }
            | Self::LorebookNotFound { .. }
            | Self::ModelNotFound { .. }
            | Self::ProviderNotFound { .. } => ApiErrorCode::NotFound,
            Self::CharacterArchived { .. }
            | Self::GroupArchived { .. }
            | Self::MemberCharacterArchived { .. }
            | Self::TooFewMembers { .. }
            | Self::AllMembersMuted { .. }
            | Self::SceneNotOwnedByGroup { .. }
            | Self::PersonaInactive { .. }
            | Self::PromptWrongPurpose { .. }
            | Self::PromptArchived { .. }
            | Self::LorebookArchived { .. }
            | Self::ProviderDisabled { .. }
            | Self::NonChatModel { .. } => ApiErrorCode::InvalidInput,
            Self::SourceChanged { .. } | Self::AlreadyLaunched { .. } | Self::CreateConflict => {
                ApiErrorCode::Conflict
            }
            Self::Repository(Source::Conversation(error)) => {
                return error.clone().into_api_error();
            }
            Self::Repository(
                Source::Character(error) | Source::Group(error) | Source::Persona(error),
            ) => return error.clone().into_api_error(),
            Self::BuiltInPromptMissing { .. }
            | Self::ArtifactEncode(_)
            | Self::InvalidLaunch(_)
            | Self::Repository(_) => ApiErrorCode::Internal,
        };
        api_error(code, self.to_string())
    }
}

impl IntoApiError for StoreError {
    fn into_api_error(self) -> ApiError {
        let code = match self {
            Self::NotFound | Self::ParentNotFound => ApiErrorCode::NotFound,
            Self::IdempotencyConflict
            | Self::ParentTerminal
            | Self::AlreadyTerminal
            | Self::IllegalTransition
            | Self::TooLate
            | Self::NotCancellable
            | Self::NotClaimed
            | Self::StaleLease
            | Self::LeaseExpired => ApiErrorCode::Conflict,
            Self::ResourceUnavailable => ApiErrorCode::Busy,
            _ => ApiErrorCode::Internal,
        };
        api_error(code, self.to_string())
    }
}

impl IntoApiError for ConversationGenerationDispatchError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Jobs(error) => error.into_api_error(),
            Self::Repository(error) => error.into_api_error(),
            Self::Usage(_) | Self::InvalidWork => {
                api_error(ApiErrorCode::Internal, self.to_string())
            }
        }
    }
}

impl IntoApiError for ConversationGenerationCancellationError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Store(error) => error.into_api_error(),
            Self::Conversation(error) => error.into_api_error(),
            Self::WrongJobKind => api_error(ApiErrorCode::InvalidInput, self.to_string()),
            Self::Usage(_) | Self::Runtime(_) | Self::InvalidWork => {
                api_error(ApiErrorCode::Internal, self.to_string())
            }
        }
    }
}

impl IntoApiError for ConversationGenerationWorkerError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Store(error) => error.into_api_error(),
            Self::InvalidWork | Self::Execution(_) => {
                api_error(ApiErrorCode::Internal, self.to_string())
            }
        }
    }
}

impl IntoApiError for MediaStoreError {
    fn into_api_error(self) -> ApiError {
        let code = match self {
            Self::AssetNotFound | Self::BlobNotFound | Self::ObjectMissing => {
                ApiErrorCode::NotFound
            }
            Self::NotReady => ApiErrorCode::Unavailable,
            _ => ApiErrorCode::Internal,
        };
        api_error(code, self.to_string())
    }
}

impl IntoApiError for tokio::task::JoinError {
    fn into_api_error(self) -> ApiError {
        let code = if self.is_cancelled() {
            ApiErrorCode::Cancelled
        } else {
            ApiErrorCode::Internal
        };
        api_error(code, "the API worker task did not finish")
    }
}
