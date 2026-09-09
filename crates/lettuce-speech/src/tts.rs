use lettuce_settings::{SecretOwnerId, SecretRef};
use lettuce_types::{AudioProviderId, Revision, TimestampMillis, VoiceProfileId};
use serde::{Deserialize, Serialize};

const MAX_LABEL_BYTES: usize = 256;
const MAX_VALUE_BYTES: usize = 4096;
const MAX_PROMPT_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioProviderKind {
    GeminiTts,
    Elevenlabs,
    FishTts,
    FishSpeech,
    OpenAiTts,
    Kokoro,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioProviderConfig {
    Gemini {
        project_id: Option<String>,
        location: String,
    },
    Elevenlabs,
    FishTts,
    FishSpeech {
        base_url: Option<String>,
        request_path: Option<String>,
    },
    OpenAiCompatible {
        base_url: Option<String>,
        request_path: Option<String>,
    },
    Kokoro {
        variant: Option<String>,
    },
}

impl AudioProviderConfig {
    #[must_use]
    pub const fn provider_kind(&self) -> AudioProviderKind {
        match self {
            Self::Gemini { .. } => AudioProviderKind::GeminiTts,
            Self::Elevenlabs => AudioProviderKind::Elevenlabs,
            Self::FishTts => AudioProviderKind::FishTts,
            Self::FishSpeech { .. } => AudioProviderKind::FishSpeech,
            Self::OpenAiCompatible { .. } => AudioProviderKind::OpenAiTts,
            Self::Kokoro { .. } => AudioProviderKind::Kokoro,
        }
    }

    fn validate(&self) -> Result<(), TtsConfigurationError> {
        match self {
            Self::Gemini {
                project_id,
                location,
            } => {
                validate_optional(project_id)?;
                validate_required(location, MAX_LABEL_BYTES)
            }
            Self::FishSpeech {
                base_url,
                request_path,
            }
            | Self::OpenAiCompatible {
                base_url,
                request_path,
            } => {
                validate_optional(base_url)?;
                validate_optional(request_path)
            }
            Self::Kokoro { variant } => validate_optional(variant),
            Self::Elevenlabs | Self::FishTts => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioProvider {
    pub id: AudioProviderId,
    pub secret_owner_id: SecretOwnerId,
    pub label: String,
    pub api_key_ref: Option<SecretRef>,
    pub config: AudioProviderConfig,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl AudioProvider {
    pub fn validate(&self) -> Result<(), TtsConfigurationError> {
        validate_required(&self.label, MAX_LABEL_BYTES)?;
        self.config.validate()?;
        if self.created_at > self.updated_at
            || (self.config.provider_kind() == AudioProviderKind::Kokoro
                && self.api_key_ref.is_some())
        {
            return Err(TtsConfigurationError::InvalidData);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVoice {
    pub id: VoiceProfileId,
    pub provider_id: AudioProviderId,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl UserVoice {
    pub fn validate(&self) -> Result<(), TtsConfigurationError> {
        validate_required(&self.name, MAX_LABEL_BYTES)?;
        validate_required(&self.model_id, MAX_VALUE_BYTES)?;
        validate_required(&self.voice_id, MAX_VALUE_BYTES)?;
        if let Some(prompt) = &self.prompt {
            if prompt.len() > MAX_PROMPT_BYTES || prompt.contains('\0') {
                return Err(TtsConfigurationError::InvalidData);
            }
        }
        if self.created_at > self.updated_at {
            return Err(TtsConfigurationError::InvalidData);
        }
        Ok(())
    }
}

pub trait TtsConfigurationRepository: Send + Sync {
    fn upsert_audio_provider(
        &self,
        provider: AudioProvider,
        expected_revision: Option<Revision>,
    ) -> Result<AudioProvider, TtsConfigurationRepositoryError>;
    fn get_audio_provider(
        &self,
        id: AudioProviderId,
    ) -> Result<Option<AudioProvider>, TtsConfigurationRepositoryError>;
    fn list_audio_providers(&self) -> Result<Vec<AudioProvider>, TtsConfigurationRepositoryError>;
    fn delete_audio_provider(
        &self,
        id: AudioProviderId,
    ) -> Result<AudioProvider, TtsConfigurationRepositoryError>;
    fn upsert_user_voice(
        &self,
        voice: UserVoice,
        expected_revision: Option<Revision>,
    ) -> Result<UserVoice, TtsConfigurationRepositoryError>;
    fn get_user_voice(
        &self,
        id: VoiceProfileId,
    ) -> Result<Option<UserVoice>, TtsConfigurationRepositoryError>;
    fn list_user_voices(&self) -> Result<Vec<UserVoice>, TtsConfigurationRepositoryError>;
    fn delete_user_voice(&self, id: VoiceProfileId) -> Result<(), TtsConfigurationRepositoryError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TtsConfigurationRepositoryError {
    #[error("TTS configuration was not found")]
    NotFound,
    #[error("TTS configuration has a stale revision")]
    StaleRevision,
    #[error("TTS configuration already exists")]
    AlreadyExists,
    #[error("TTS provider does not exist")]
    ProviderMissing,
    #[error("TTS configuration is invalid")]
    InvalidData,
    #[error("TTS configuration storage failed")]
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TtsConfigurationError {
    #[error("TTS configuration is invalid")]
    InvalidData,
}

fn validate_required(value: &str, max_bytes: usize) -> Result<(), TtsConfigurationError> {
    if value.is_empty() || value.len() > max_bytes || value.trim() != value || value.contains('\0')
    {
        return Err(TtsConfigurationError::InvalidData);
    }
    Ok(())
}

fn validate_optional(value: &Option<String>) -> Result<(), TtsConfigurationError> {
    if let Some(value) = value {
        validate_required(value, MAX_VALUE_BYTES)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(config: AudioProviderConfig) -> AudioProvider {
        AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: SecretOwnerId::new(),
            label: "Primary voice provider".into(),
            api_key_ref: Some(SecretRef::new()),
            config,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn all_legacy_provider_kinds_have_typed_configs() {
        let configs = [
            AudioProviderConfig::Gemini {
                project_id: Some("project".into()),
                location: "us-central1".into(),
            },
            AudioProviderConfig::Elevenlabs,
            AudioProviderConfig::FishTts,
            AudioProviderConfig::FishSpeech {
                base_url: Some("https://speech.example".into()),
                request_path: Some("/v1/tts".into()),
            },
            AudioProviderConfig::OpenAiCompatible {
                base_url: Some("https://audio.example".into()),
                request_path: Some("/v1/audio/speech".into()),
            },
            AudioProviderConfig::Kokoro {
                variant: Some("model-quantized".into()),
            },
        ];
        let kinds = configs.map(|config| config.provider_kind());
        assert_eq!(
            kinds,
            [
                AudioProviderKind::GeminiTts,
                AudioProviderKind::Elevenlabs,
                AudioProviderKind::FishTts,
                AudioProviderKind::FishSpeech,
                AudioProviderKind::OpenAiTts,
                AudioProviderKind::Kokoro,
            ]
        );
    }

    #[test]
    fn local_provider_rejects_secret_reference() {
        assert_eq!(
            provider(AudioProviderConfig::Kokoro { variant: None }).validate(),
            Err(TtsConfigurationError::InvalidData)
        );
    }
}
