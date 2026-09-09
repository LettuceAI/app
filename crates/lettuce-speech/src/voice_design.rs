use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, MediaStoreError, RetentionClass,
};
use lettuce_settings::SecretValue;
use lettuce_types::{AssetId, ContentHash, RequestId, TimestampMillis};
use serde::{Deserialize, Serialize};

use crate::{AudioProvider, AudioProviderConfig};

const MIN_SAMPLE_SCALARS: usize = 100;
const MAX_SAMPLE_SCALARS: usize = 1_000;
const MIN_DESCRIPTION_SCALARS: usize = 20;
const MAX_DESCRIPTION_SCALARS: usize = 1_000;
const MAX_PREVIEWS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceDesignRequest {
    pub id: RequestId,
    pub provider: AudioProvider,
    pub text_sample: String,
    pub voice_description: String,
    pub model_id: Option<String>,
    pub num_previews: Option<u32>,
    pub expires_at: TimestampMillis,
    pub created_at: TimestampMillis,
}

impl VoiceDesignRequest {
    pub fn validate(&self) -> Result<(), VoiceDesignValidationError> {
        self.provider
            .validate()
            .map_err(|_| VoiceDesignValidationError::InvalidRequest)?;
        if !matches!(self.provider.config, AudioProviderConfig::Elevenlabs)
            || self.provider.api_key_ref.is_none()
            || !valid_trimmed_scalars(&self.text_sample, MIN_SAMPLE_SCALARS, MAX_SAMPLE_SCALARS)
            || !valid_trimmed_scalars(
                &self.voice_description,
                MIN_DESCRIPTION_SCALARS,
                MAX_DESCRIPTION_SCALARS,
            )
            || self.expires_at <= self.created_at
            || self
                .num_previews
                .is_some_and(|count| count == 0 || count > MAX_PREVIEWS)
        {
            return Err(VoiceDesignValidationError::InvalidRequest);
        }
        if let Some(model_id) = &self.model_id
            && !matches!(
                model_id.as_str(),
                "eleven_multilingual_ttv_v2" | "eleven_ttv_v3"
            )
        {
            return Err(VoiceDesignValidationError::InvalidRequest);
        }
        Ok(())
    }
}

pub struct RuntimeVoiceDesignPreview {
    pub generated_voice_id: String,
    pub bytes: Vec<u8>,
    pub duration_secs: f64,
    pub declared_mime_type: String,
}

impl std::fmt::Debug for RuntimeVoiceDesignPreview {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeVoiceDesignPreview")
            .field("generated_voice_id", &self.generated_voice_id)
            .field("byte_size", &self.bytes.len())
            .field("duration_secs", &self.duration_secs)
            .field("declared_mime_type", &self.declared_mime_type)
            .finish()
    }
}

impl RuntimeVoiceDesignPreview {
    pub fn validate(&self) -> Result<(), VoiceDesignValidationError> {
        if !valid_identifier(&self.generated_voice_id)
            || self.bytes.is_empty()
            || self.bytes.len() as u64 > lettuce_media::MAX_MEDIA_BLOB_BYTES
            || !self.duration_secs.is_finite()
            || self.duration_secs <= 0.0
            || self.declared_mime_type != "audio/mpeg"
        {
            return Err(VoiceDesignValidationError::InvalidResult);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceDesignPreview {
    pub request_id: RequestId,
    pub generated_voice_id: String,
    pub audio_asset_id: AssetId,
    pub content_hash: ContentHash,
    pub byte_size: u64,
    pub duration_secs: f64,
    pub mime_type: String,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceCreationRequest {
    pub provider: AudioProvider,
    pub voice_name: String,
    pub generated_voice_id: String,
    pub voice_description: String,
}

impl VoiceCreationRequest {
    pub fn validate(&self) -> Result<(), VoiceDesignValidationError> {
        self.provider
            .validate()
            .map_err(|_| VoiceDesignValidationError::InvalidRequest)?;
        if !matches!(self.provider.config, AudioProviderConfig::Elevenlabs)
            || self.provider.api_key_ref.is_none()
            || !valid_trimmed_scalars(&self.voice_name, 1, 256)
            || !valid_identifier(&self.generated_voice_id)
            || !valid_trimmed_scalars(
                &self.voice_description,
                MIN_DESCRIPTION_SCALARS,
                MAX_DESCRIPTION_SCALARS,
            )
        {
            return Err(VoiceDesignValidationError::InvalidRequest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatedVoice {
    pub voice_id: String,
}

impl CreatedVoice {
    pub fn validate(&self) -> Result<(), VoiceDesignValidationError> {
        if !valid_identifier(&self.voice_id) {
            return Err(VoiceDesignValidationError::InvalidResult);
        }
        Ok(())
    }
}

impl VoiceDesignPreview {
    pub fn validate_for(
        &self,
        request: &VoiceDesignRequest,
    ) -> Result<(), VoiceDesignValidationError> {
        if self.request_id != request.id
            || !valid_identifier(&self.generated_voice_id)
            || self.byte_size == 0
            || self.byte_size > lettuce_media::MAX_MEDIA_BLOB_BYTES
            || !self.duration_secs.is_finite()
            || self.duration_secs <= 0.0
            || self.mime_type != "audio/mpeg"
            || self.completed_at < request.created_at
        {
            return Err(VoiceDesignValidationError::InvalidResult);
        }
        Ok(())
    }
}

#[async_trait]
pub trait VoiceDesignRuntime: Send + Sync {
    async fn design_voice(
        &self,
        request: &VoiceDesignRequest,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<Vec<RuntimeVoiceDesignPreview>, VoiceDesignRuntimeError>;

    async fn create_voice(
        &self,
        request: &VoiceCreationRequest,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<CreatedVoice, VoiceDesignRuntimeError>;
}

pub trait VoiceDesignPreviewSink: Send + Sync {
    fn ingest_voice_design_preview(
        &self,
        request: &VoiceDesignRequest,
        output: RuntimeVoiceDesignPreview,
        completed_at: TimestampMillis,
    ) -> Result<VoiceDesignPreview, VoiceDesignAudioError>;
}

impl<BR, AR> VoiceDesignPreviewSink for LocalMediaBlobStore<BR, AR>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    fn ingest_voice_design_preview(
        &self,
        request: &VoiceDesignRequest,
        output: RuntimeVoiceDesignPreview,
        completed_at: TimestampMillis,
    ) -> Result<VoiceDesignPreview, VoiceDesignAudioError> {
        request.validate()?;
        output.validate()?;
        let generated_voice_id = output.generated_voice_id;
        let duration_secs = output.duration_secs;
        let ingested = self
            .ingest(
                output.bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::SynthesizedSpeech,
                    AssetOrigin::Synthesized,
                    RetentionClass::Temporary {
                        expires_at: request.expires_at,
                    },
                    AssetProvenanceV1 {
                        source_label: Some("elevenlabs-voice-design".into()),
                        ..AssetProvenanceV1::default()
                    },
                )
                .with_declared_mime_type(output.declared_mime_type),
            )
            .map_err(VoiceDesignAudioError::Media)?;
        let preview = VoiceDesignPreview {
            request_id: request.id,
            generated_voice_id,
            audio_asset_id: ingested.asset.id,
            content_hash: ingested.blob.content_hash,
            byte_size: ingested.blob.byte_size,
            duration_secs,
            mime_type: ingested.blob.mime_type,
            completed_at,
        };
        preview.validate_for(request)?;
        Ok(preview)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VoiceDesignValidationError {
    #[error("voice design request is invalid")]
    InvalidRequest,
    #[error("voice design result is invalid")]
    InvalidResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VoiceDesignRuntimeError {
    #[error("voice design was cancelled")]
    Cancelled,
    #[error("voice design runtime is unavailable")]
    Unavailable,
    #[error("voice design runtime rejected the request")]
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VoiceDesignAudioError {
    #[error("voice design output is invalid: {0}")]
    Invalid(#[from] VoiceDesignValidationError),
    #[error("voice design output media failed: {0}")]
    Media(MediaStoreError),
}

fn valid_trimmed_scalars(value: &str, min: usize, max: usize) -> bool {
    value.trim() == value && !value.contains('\0') && (min..=max).contains(&value.chars().count())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4_096 && value.trim() == value && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use lettuce_settings::{SecretOwnerId, SecretRef};
    use lettuce_types::{AudioProviderId, Revision};

    use super::*;

    fn request() -> VoiceDesignRequest {
        VoiceDesignRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "ElevenLabs".into(),
                api_key_ref: Some(SecretRef::new()),
                config: AudioProviderConfig::Elevenlabs,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            text_sample: "A".repeat(100),
            voice_description: "A warm and expressive narrator".into(),
            model_id: Some("eleven_ttv_v3".into()),
            num_previews: Some(1),
            expires_at: TimestampMillis::new(10_000),
            created_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn validates_legacy_voice_design_inputs_and_current_limits() {
        let mut request = request();
        assert_eq!(request.validate(), Ok(()));
        request.text_sample = "short".into();
        assert_eq!(
            request.validate(),
            Err(VoiceDesignValidationError::InvalidRequest)
        );
        request = self::request();
        request.voice_description = "too short".into();
        assert_eq!(
            request.validate(),
            Err(VoiceDesignValidationError::InvalidRequest)
        );
        request = self::request();
        request.num_previews = Some(4);
        assert_eq!(
            request.validate(),
            Err(VoiceDesignValidationError::InvalidRequest)
        );

        let creation = VoiceCreationRequest {
            provider: self::request().provider,
            voice_name: "Storyteller".into(),
            generated_voice_id: "generated-voice-1".into(),
            voice_description: "A warm and expressive narrator".into(),
        };
        assert_eq!(creation.validate(), Ok(()));
        assert_eq!(
            CreatedVoice {
                voice_id: "created-voice-1".into()
            }
            .validate(),
            Ok(())
        );
    }
}
