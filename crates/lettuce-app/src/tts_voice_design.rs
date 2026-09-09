use lettuce_jobs::handle::CancellationToken;
use lettuce_settings::{SecretPurpose, SecretStore, SecretStoreError};
use lettuce_speech::{
    AudioProviderConfig, CreatedVoice, TtsConfigurationRepository, TtsConfigurationRepositoryError,
    VoiceCreationRequest, VoiceDesignAudioError, VoiceDesignPreview, VoiceDesignPreviewSink,
    VoiceDesignRequest, VoiceDesignRuntime, VoiceDesignRuntimeError, VoiceDesignValidationError,
};
use lettuce_types::{AudioProviderId, RequestId, TimestampMillis};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceDesignPreviewDraft {
    pub provider_id: AudioProviderId,
    pub text_sample: String,
    pub voice_description: String,
    pub model_id: Option<String>,
    pub num_previews: Option<u32>,
    pub expires_at: TimestampMillis,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCreationDraft {
    pub provider_id: AudioProviderId,
    pub voice_name: String,
    pub generated_voice_id: String,
    pub voice_description: String,
}

#[derive(Debug)]
pub struct TtsVoiceDesignCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secrets: &'a S,
}

impl<'a, R: ?Sized, S: ?Sized> TtsVoiceDesignCoordinator<'a, R, S> {
    #[must_use]
    pub const fn new(repository: &'a R, secrets: &'a S) -> Self {
        Self {
            repository,
            secrets,
        }
    }
}

impl<R, S> TtsVoiceDesignCoordinator<'_, R, S>
where
    R: TtsConfigurationRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    pub fn admit(
        &self,
        draft: VoiceDesignPreviewDraft,
    ) -> Result<VoiceDesignRequest, TtsVoiceDesignError> {
        let provider = self
            .repository
            .get_audio_provider(draft.provider_id)
            .map_err(TtsVoiceDesignError::Configuration)?
            .ok_or(TtsVoiceDesignError::Configuration(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if !matches!(provider.config, AudioProviderConfig::Elevenlabs) {
            return Err(TtsVoiceDesignError::InvalidInput);
        }
        let request = VoiceDesignRequest {
            id: RequestId::new(),
            provider,
            text_sample: draft.text_sample.trim().to_owned(),
            voice_description: draft.voice_description.trim().to_owned(),
            model_id: draft.model_id,
            num_previews: draft.num_previews,
            expires_at: draft.expires_at,
            created_at: draft.created_at,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn admit_creation(
        &self,
        draft: VoiceCreationDraft,
    ) -> Result<VoiceCreationRequest, TtsVoiceDesignError> {
        let provider = self
            .repository
            .get_audio_provider(draft.provider_id)
            .map_err(TtsVoiceDesignError::Configuration)?
            .ok_or(TtsVoiceDesignError::Configuration(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if !matches!(provider.config, AudioProviderConfig::Elevenlabs) {
            return Err(TtsVoiceDesignError::InvalidInput);
        }
        let request = VoiceCreationRequest {
            provider,
            voice_name: draft.voice_name.trim().to_owned(),
            generated_voice_id: draft.generated_voice_id.trim().to_owned(),
            voice_description: draft.voice_description.trim().to_owned(),
        };
        request.validate()?;
        Ok(request)
    }

    pub async fn preview<E, A>(
        &self,
        request: &VoiceDesignRequest,
        runtime: &E,
        audio: &A,
        cancellation: &CancellationToken,
        completed_at: TimestampMillis,
    ) -> Result<Vec<VoiceDesignPreview>, TtsVoiceDesignError>
    where
        E: VoiceDesignRuntime + ?Sized,
        A: VoiceDesignPreviewSink + ?Sized,
    {
        request.validate()?;
        if cancellation.is_cancelled() {
            return Err(TtsVoiceDesignError::Runtime(
                VoiceDesignRuntimeError::Cancelled,
            ));
        }
        let reference = request
            .provider
            .api_key_ref
            .ok_or(TtsVoiceDesignError::InvalidInput)?;
        let credential = self
            .secrets
            .load(
                &reference,
                &SecretPurpose::AudioApiKey {
                    owner: request.provider.secret_owner_id,
                },
            )
            .await
            .map_err(TtsVoiceDesignError::SecretStore)?;
        let outputs = runtime
            .design_voice(request, &credential, cancellation)
            .await
            .map_err(TtsVoiceDesignError::Runtime)?;
        if outputs.is_empty()
            || outputs.len() > 3
            || request
                .num_previews
                .is_some_and(|count| outputs.len() != count as usize)
        {
            return Err(TtsVoiceDesignError::Runtime(
                VoiceDesignRuntimeError::Rejected,
            ));
        }
        let mut previews = Vec::with_capacity(outputs.len());
        for output in outputs {
            if cancellation.is_cancelled() {
                return Err(TtsVoiceDesignError::Runtime(
                    VoiceDesignRuntimeError::Cancelled,
                ));
            }
            let preview = audio
                .ingest_voice_design_preview(request, output, completed_at)
                .map_err(TtsVoiceDesignError::Audio)?;
            preview.validate_for(request)?;
            previews.push(preview);
        }
        Ok(previews)
    }

    pub async fn create_voice<E: VoiceDesignRuntime + ?Sized>(
        &self,
        request: &VoiceCreationRequest,
        runtime: &E,
        cancellation: &CancellationToken,
    ) -> Result<CreatedVoice, TtsVoiceDesignError> {
        request.validate()?;
        if cancellation.is_cancelled() {
            return Err(TtsVoiceDesignError::Runtime(
                VoiceDesignRuntimeError::Cancelled,
            ));
        }
        let reference = request
            .provider
            .api_key_ref
            .ok_or(TtsVoiceDesignError::InvalidInput)?;
        let credential = self
            .secrets
            .load(
                &reference,
                &SecretPurpose::AudioApiKey {
                    owner: request.provider.secret_owner_id,
                },
            )
            .await
            .map_err(TtsVoiceDesignError::SecretStore)?;
        let created = runtime
            .create_voice(request, &credential, cancellation)
            .await
            .map_err(TtsVoiceDesignError::Runtime)?;
        created.validate()?;
        Ok(created)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TtsVoiceDesignError {
    #[error("voice design input is invalid")]
    InvalidInput,
    #[error("voice design input or result is invalid: {0}")]
    Validation(#[from] VoiceDesignValidationError),
    #[error("TTS configuration persistence failed: {0}")]
    Configuration(TtsConfigurationRepositoryError),
    #[error("TTS secret access failed: {0}")]
    SecretStore(SecretStoreError),
    #[error("voice design runtime failed: {0}")]
    Runtime(VoiceDesignRuntimeError),
    #[error("voice design preview media failed: {0}")]
    Audio(VoiceDesignAudioError),
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_media::{LocalMediaBlobStore, MediaAssetRepository, RetentionClass};
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_settings::{InMemorySecretStore, SecretValue};
    use lettuce_speech::{AudioProviderConfig, RuntimeVoiceDesignPreview};

    use crate::TtsConfigurationCoordinator;

    use super::*;

    struct Runtime;

    #[async_trait]
    impl VoiceDesignRuntime for Runtime {
        async fn design_voice(
            &self,
            request: &VoiceDesignRequest,
            credential: &SecretValue,
            _: &CancellationToken,
        ) -> Result<Vec<RuntimeVoiceDesignPreview>, VoiceDesignRuntimeError> {
            assert_eq!(request.model_id.as_deref(), Some("eleven_ttv_v3"));
            credential.with(|value| assert_eq!(value, "voice-design-secret-canary"));
            Ok(vec![RuntimeVoiceDesignPreview {
                generated_voice_id: "generated-voice-1".into(),
                bytes: vec![0xff, 0xfb, 0x90, 0x64, 0, 0, 0, 0],
                duration_secs: 2.5,
                declared_mime_type: "audio/mpeg".into(),
            }])
        }

        async fn create_voice(
            &self,
            request: &VoiceCreationRequest,
            credential: &SecretValue,
            _: &CancellationToken,
        ) -> Result<CreatedVoice, VoiceDesignRuntimeError> {
            assert_eq!(request.voice_name, "Storyteller");
            assert_eq!(request.generated_voice_id, "generated-voice-1");
            credential.with(|value| assert_eq!(value, "voice-design-secret-canary"));
            Ok(CreatedVoice {
                voice_id: "created-voice-1".into(),
            })
        }
    }

    fn media_store(
        database_path: &std::path::Path,
        root: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let snapshot = DirectorySnapshot::new(root).expect("directory snapshot");
        let authority = FilesystemAuthority::new(&snapshot).expect("filesystem authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(database_path).expect("blob database"),
            Database::open(database_path).expect("asset database"),
        )
    }

    #[tokio::test]
    async fn admits_only_elevenlabs_and_resolves_its_scoped_secret() {
        let root = std::env::temp_dir().join(format!("lettuce-voice-design-{}", RequestId::new()));
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("app.sqlite3");
        let database = Database::open(&path).expect("database");
        let media = media_store(&path, &root.join("media"));
        let secrets = InMemorySecretStore::new();
        let provider = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "ElevenLabs".into(),
                AudioProviderConfig::Elevenlabs,
                Some(SecretValue::new("voice-design-secret-canary").expect("secret")),
                TimestampMillis::new(1),
            )
            .await
            .expect("provider");
        let coordinator = TtsVoiceDesignCoordinator::new(&database, &secrets);
        let request = coordinator
            .admit(VoiceDesignPreviewDraft {
                provider_id: provider.id,
                text_sample: format!("  {}  ", "A".repeat(100)),
                voice_description: "  A warm and expressive narrator  ".into(),
                model_id: Some("eleven_ttv_v3".into()),
                num_previews: Some(1),
                expires_at: TimestampMillis::new(10_000),
                created_at: TimestampMillis::new(2),
            })
            .expect("admission");
        assert_eq!(request.text_sample, "A".repeat(100));
        assert_eq!(request.voice_description, "A warm and expressive narrator");
        let previews = coordinator
            .preview(
                &request,
                &Runtime,
                &media,
                &CancellationToken::new(),
                TimestampMillis::new(3),
            )
            .await
            .expect("preview");
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].generated_voice_id, "generated-voice-1");
        assert_eq!(previews[0].mime_type, "audio/mpeg");
        let asset = database
            .get(previews[0].audio_asset_id)
            .expect("asset lookup")
            .expect("managed preview asset");
        assert_eq!(
            asset.retention,
            RetentionClass::Temporary {
                expires_at: TimestampMillis::new(10_000)
            }
        );
        let creation = coordinator
            .admit_creation(VoiceCreationDraft {
                provider_id: provider.id,
                voice_name: "  Storyteller  ".into(),
                generated_voice_id: "  generated-voice-1  ".into(),
                voice_description: "  A warm and expressive narrator  ".into(),
            })
            .expect("creation admission");
        let created = coordinator
            .create_voice(&creation, &Runtime, &CancellationToken::new())
            .await
            .expect("created voice");
        assert_eq!(created.voice_id, "created-voice-1");

        let wrong_provider = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "Fish Audio".into(),
                AudioProviderConfig::FishTts,
                Some(SecretValue::new("voice-design-secret-canary").expect("secret")),
                TimestampMillis::new(4),
            )
            .await
            .expect("provider");
        assert_eq!(
            coordinator.admit(VoiceDesignPreviewDraft {
                provider_id: wrong_provider.id,
                text_sample: "A".repeat(100),
                voice_description: "A warm and expressive narrator".into(),
                model_id: None,
                num_previews: Some(1),
                expires_at: TimestampMillis::new(10_000),
                created_at: TimestampMillis::new(5),
            }),
            Err(TtsVoiceDesignError::InvalidInput)
        );
    }
}
