use lettuce_contracts::{
    ApiError, ApiErrorCode, ApiErrorDetails, SpeechFailure, SpeechModelKind,
};
use lettuce_speech::{
    TtsConfigurationError, AsrAudioError, AsrLearningError, AsrLearningRepositoryError, AsrRuntimeError,
    AsrValidationError, SynthesisRepositoryError, TranscriptionRepositoryError,
    TtsConfigurationRepositoryError,
};

use crate::api::error::{IntoApiError, api_error, invalid_field};
use crate::{
    SpeechTranscriptionError, TtsSynthesisError, WhisperModelCoordinatorError,
};

pub(crate) fn speech_error(
    code: ApiErrorCode,
    failure: SpeechFailure,
    message: impl Into<String>,
) -> ApiError {
    ApiError {
        code,
        message: message.into(),
        details: Some(ApiErrorDetails::Speech { failure }),
    }
}

/// The call needs a Whisper model and none is installed.
pub(crate) fn whisper_required() -> ApiError {
    speech_error(
        ApiErrorCode::ModelRequired,
        SpeechFailure::ModelRequired {
            model: SpeechModelKind::Whisper,
        },
        "no Whisper model is installed",
    )
}

pub(crate) fn secret_missing() -> ApiError {
    speech_error(
        ApiErrorCode::Unavailable,
        SpeechFailure::SecretMissing,
        "the provider's API key is not configured",
    )
}

impl IntoApiError for WhisperModelCoordinatorError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::NotInstalled => whisper_required(),
            Self::InvalidModelId => invalid_field("model_id", self.to_string()),
            Self::Model(_) | Self::Runtime(_) => speech_error(
                ApiErrorCode::ModelUnavailable,
                SpeechFailure::ModelRequired {
                    model: SpeechModelKind::Whisper,
                },
                self.to_string(),
            ),
            Self::Repository(_) => api_error(ApiErrorCode::Internal, self.to_string()),
        }
    }
}

impl IntoApiError for AsrValidationError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::UnsupportedLanguage => invalid_field("options.language", self.to_string()),
            _ => invalid_field("options", self.to_string()),
        }
    }
}

impl IntoApiError for SpeechTranscriptionError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Invalid(error) => error.into_api_error(),
            Self::Audio(AsrAudioError::UnsupportedFormat | AsrAudioError::InvalidAudio) => {
                invalid_field("source", "the audio is not a supported WAV file")
            }
            Self::Audio(AsrAudioError::TooLarge) => {
                invalid_field("source", "the audio is longer than a transcription accepts")
            }
            Self::Runtime(AsrRuntimeError::ModelUnavailable) => whisper_required(),
            Self::Repository(TranscriptionRepositoryError::Conflict) => api_error(
                ApiErrorCode::Conflict,
                "another transcription already uses this request id",
            ),
            Self::Repository(TranscriptionRepositoryError::NotFound) => {
                api_error(ApiErrorCode::NotFound, self.to_string())
            }
            Self::Jobs(error) => error.into_api_error(),
            _ => api_error(ApiErrorCode::Internal, self.to_string()),
        }
    }
}

impl IntoApiError for TtsSynthesisError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Invalid(error) => invalid_field("request", error.to_string()),
            Self::Repository(SynthesisRepositoryError::Conflict) => api_error(
                ApiErrorCode::Conflict,
                "another synthesis already uses this request id",
            ),
            Self::Repository(SynthesisRepositoryError::NotFound) => {
                api_error(ApiErrorCode::NotFound, self.to_string())
            }
            Self::Jobs(error) => error.into_api_error(),
            _ => api_error(ApiErrorCode::Internal, self.to_string()),
        }
    }
}

impl IntoApiError for AsrLearningRepositoryError {
    fn into_api_error(self) -> ApiError {
        api_error(ApiErrorCode::Internal, self.to_string())
    }
}

impl IntoApiError for AsrLearningError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::InvalidData => invalid_field("request", self.to_string()),
            error => api_error(ApiErrorCode::Internal, error.to_string()),
        }
    }
}

impl IntoApiError for TtsConfigurationError {
    fn into_api_error(self) -> ApiError {
        invalid_field("request", self.to_string())
    }
}

impl IntoApiError for TtsConfigurationRepositoryError {
    fn into_api_error(self) -> ApiError {
        let code = match self {
            Self::NotFound | Self::ProviderMissing => ApiErrorCode::NotFound,
            Self::StaleRevision | Self::AlreadyExists => ApiErrorCode::Conflict,
            Self::InvalidData => ApiErrorCode::InvalidInput,
            Self::Storage => ApiErrorCode::Internal,
        };
        api_error(code, self.to_string())
    }
}
