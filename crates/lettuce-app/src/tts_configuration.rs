use lettuce_settings::{
    SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStore,
    SecretStoreError, SecretValue,
};
use lettuce_speech::{
    AudioProvider, AudioProviderConfig, AudioProviderKind, TtsConfigurationRepository,
    TtsConfigurationRepositoryError, UserVoice,
};
use lettuce_types::{AudioProviderId, Revision, TimestampMillis, VoiceProfileId};

#[derive(Debug)]
pub struct TtsConfigurationCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secret_store: &'a S,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtsSecretCleanup {
    reference: SecretRef,
    owner: SecretOwnerId,
    generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateUserVoiceRequest {
    pub id: VoiceProfileId,
    pub expected_revision: Revision,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TtsConfigurationCoordinatorError {
    #[error("TTS configuration is invalid")]
    InvalidInput,
    #[error("TTS configuration persistence failed: {0}")]
    Repository(TtsConfigurationRepositoryError),
    #[error("TTS secret operation failed: {0}")]
    SecretStore(SecretStoreError),
    #[error("TTS metadata changed but secret cleanup remains pending")]
    CleanupPending {
        cleanup: TtsSecretCleanup,
        source: SecretStoreError,
    },
    #[error("TTS metadata write failed and new secret cleanup also failed")]
    CompensationFailed {
        repository: TtsConfigurationRepositoryError,
        cleanup: TtsSecretCleanup,
        source: SecretStoreError,
    },
}

impl<'a, R: ?Sized, S: ?Sized> TtsConfigurationCoordinator<'a, R, S> {
    #[must_use]
    pub const fn new(repository: &'a R, secret_store: &'a S) -> Self {
        Self {
            repository,
            secret_store,
        }
    }
}

impl<R, S> TtsConfigurationCoordinator<'_, R, S>
where
    R: TtsConfigurationRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    pub fn list_audio_providers(
        &self,
    ) -> Result<Vec<AudioProvider>, TtsConfigurationCoordinatorError> {
        self.repository
            .list_audio_providers()
            .map_err(TtsConfigurationCoordinatorError::Repository)
    }

    pub fn list_user_voices(&self) -> Result<Vec<UserVoice>, TtsConfigurationCoordinatorError> {
        self.repository
            .list_user_voices()
            .map_err(TtsConfigurationCoordinatorError::Repository)
    }

    pub async fn create_audio_provider(
        &self,
        label: String,
        config: AudioProviderConfig,
        api_key: Option<SecretValue>,
        now: TimestampMillis,
    ) -> Result<AudioProvider, TtsConfigurationCoordinatorError> {
        validate_secret_choice(&config, api_key.is_some())?;
        let owner = SecretOwnerId::new();
        let mut cleanup = None;
        let api_key_ref = if let Some(value) = api_key {
            let reference = SecretRef::new();
            let status = self
                .secret_store
                .put(
                    SecretRecord::new(reference, audio_purpose(owner)),
                    value,
                    None,
                )
                .await
                .map_err(TtsConfigurationCoordinatorError::SecretStore)?;
            cleanup = Some(TtsSecretCleanup {
                reference,
                owner,
                generation: status.generation,
            });
            Some(reference)
        } else {
            None
        };
        let provider = AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: owner,
            label,
            api_key_ref,
            config,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        match self.repository.upsert_audio_provider(provider, None) {
            Ok(provider) => Ok(provider),
            Err(repository) => {
                if let Some(cleanup) = cleanup {
                    if let Err(source) = self.delete_secret(cleanup).await {
                        return Err(TtsConfigurationCoordinatorError::CompensationFailed {
                            repository,
                            cleanup,
                            source,
                        });
                    }
                }
                Err(TtsConfigurationCoordinatorError::Repository(repository))
            }
        }
    }

    pub fn update_audio_provider(
        &self,
        id: AudioProviderId,
        expected_revision: Revision,
        label: String,
        config: AudioProviderConfig,
        now: TimestampMillis,
    ) -> Result<AudioProvider, TtsConfigurationCoordinatorError> {
        let mut provider = self
            .repository
            .get_audio_provider(id)
            .map_err(TtsConfigurationCoordinatorError::Repository)?
            .ok_or(TtsConfigurationCoordinatorError::Repository(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if provider.revision != expected_revision
            || provider.config.provider_kind() != config.provider_kind()
        {
            return Err(TtsConfigurationCoordinatorError::InvalidInput);
        }
        provider.label = label;
        provider.config = config;
        provider.updated_at = now;
        self.repository
            .upsert_audio_provider(provider, Some(expected_revision))
            .map_err(TtsConfigurationCoordinatorError::Repository)
    }

    pub async fn rotate_audio_api_key(
        &self,
        id: AudioProviderId,
        value: SecretValue,
        expected_generation: u64,
    ) -> Result<u64, TtsConfigurationCoordinatorError> {
        let provider = self
            .repository
            .get_audio_provider(id)
            .map_err(TtsConfigurationCoordinatorError::Repository)?
            .ok_or(TtsConfigurationCoordinatorError::Repository(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        let reference = provider
            .api_key_ref
            .ok_or(TtsConfigurationCoordinatorError::InvalidInput)?;
        self.secret_store
            .put(
                SecretRecord::new(reference, audio_purpose(provider.secret_owner_id)),
                value,
                Some(expected_generation),
            )
            .await
            .map(|status| status.generation)
            .map_err(TtsConfigurationCoordinatorError::SecretStore)
    }

    pub async fn delete_audio_provider(
        &self,
        id: AudioProviderId,
        expected_revision: Revision,
    ) -> Result<AudioProvider, TtsConfigurationCoordinatorError> {
        let provider = self
            .repository
            .get_audio_provider(id)
            .map_err(TtsConfigurationCoordinatorError::Repository)?
            .ok_or(TtsConfigurationCoordinatorError::Repository(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if provider.revision != expected_revision {
            return Err(TtsConfigurationCoordinatorError::InvalidInput);
        }
        let cleanup = if let Some(reference) = provider.api_key_ref {
            let purpose = audio_purpose(provider.secret_owner_id);
            let status = self
                .secret_store
                .status(&reference, &purpose)
                .await
                .map_err(TtsConfigurationCoordinatorError::SecretStore)?;
            match status.state {
                SecretState::Present => Some(TtsSecretCleanup {
                    reference,
                    owner: provider.secret_owner_id,
                    generation: status.generation,
                }),
                SecretState::Missing => None,
                SecretState::Unavailable { reason } => {
                    return Err(TtsConfigurationCoordinatorError::SecretStore(
                        SecretStoreError::Unavailable(reason),
                    ));
                }
            }
        } else {
            None
        };
        let removed = self
            .repository
            .delete_audio_provider(id, expected_revision)
            .map_err(TtsConfigurationCoordinatorError::Repository)?;
        if removed != provider {
            return Err(TtsConfigurationCoordinatorError::Repository(
                TtsConfigurationRepositoryError::InvalidData,
            ));
        }
        if let Some(cleanup) = cleanup {
            if let Err(source) = self.delete_secret(cleanup).await {
                return Err(TtsConfigurationCoordinatorError::CleanupPending { cleanup, source });
            }
        }
        Ok(removed)
    }

    pub async fn retry_secret_cleanup(
        &self,
        cleanup: TtsSecretCleanup,
    ) -> Result<(), TtsConfigurationCoordinatorError> {
        self.delete_secret(cleanup)
            .await
            .map_err(TtsConfigurationCoordinatorError::SecretStore)
    }

    pub fn create_user_voice(
        &self,
        provider_id: AudioProviderId,
        name: String,
        model_id: String,
        voice_id: String,
        prompt: Option<String>,
        now: TimestampMillis,
    ) -> Result<UserVoice, TtsConfigurationCoordinatorError> {
        self.repository
            .upsert_user_voice(
                UserVoice {
                    id: VoiceProfileId::new(),
                    provider_id,
                    name,
                    model_id,
                    voice_id,
                    prompt,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
                None,
            )
            .map_err(TtsConfigurationCoordinatorError::Repository)
    }

    pub fn update_user_voice(
        &self,
        request: UpdateUserVoiceRequest,
    ) -> Result<UserVoice, TtsConfigurationCoordinatorError> {
        let mut voice = self
            .repository
            .get_user_voice(request.id)
            .map_err(TtsConfigurationCoordinatorError::Repository)?
            .ok_or(TtsConfigurationCoordinatorError::Repository(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if voice.revision != request.expected_revision {
            return Err(TtsConfigurationCoordinatorError::InvalidInput);
        }
        voice.name = request.name;
        voice.model_id = request.model_id;
        voice.voice_id = request.voice_id;
        voice.prompt = request.prompt;
        voice.updated_at = request.now;
        self.repository
            .upsert_user_voice(voice, Some(request.expected_revision))
            .map_err(TtsConfigurationCoordinatorError::Repository)
    }

    pub fn delete_user_voice(
        &self,
        id: VoiceProfileId,
    ) -> Result<(), TtsConfigurationCoordinatorError> {
        self.repository
            .delete_user_voice(id)
            .map_err(TtsConfigurationCoordinatorError::Repository)
    }

    async fn delete_secret(&self, cleanup: TtsSecretCleanup) -> Result<(), SecretStoreError> {
        let status = self
            .secret_store
            .delete(
                &cleanup.reference,
                &audio_purpose(cleanup.owner),
                Some(cleanup.generation),
            )
            .await?;
        match status.state {
            SecretState::Present => Err(SecretStoreError::Backend(
                lettuce_settings::SecretBackendError::Corrupt,
            )),
            SecretState::Missing => Ok(()),
            SecretState::Unavailable { reason } => Err(SecretStoreError::Unavailable(reason)),
        }
    }
}

fn audio_purpose(owner: SecretOwnerId) -> SecretPurpose {
    SecretPurpose::AudioApiKey { owner }
}

fn validate_secret_choice(
    config: &AudioProviderConfig,
    has_api_key: bool,
) -> Result<(), TtsConfigurationCoordinatorError> {
    let valid = match config.provider_kind() {
        AudioProviderKind::Kokoro => !has_api_key,
        AudioProviderKind::FishSpeech => true,
        AudioProviderKind::GeminiTts
        | AudioProviderKind::Elevenlabs
        | AudioProviderKind::FishTts
        | AudioProviderKind::OpenAiTts => has_api_key,
    };
    if valid {
        Ok(())
    } else {
        Err(TtsConfigurationCoordinatorError::InvalidInput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_settings::{InMemorySecretStore, SecretStatus};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FailingDeleteSecretStore {
        inner: InMemorySecretStore,
        fail_next_delete: AtomicBool,
    }

    impl FailingDeleteSecretStore {
        fn new() -> Self {
            Self {
                inner: InMemorySecretStore::new(),
                fail_next_delete: AtomicBool::new(true),
            }
        }
    }

    #[async_trait]
    impl SecretStore for FailingDeleteSecretStore {
        async fn put(
            &self,
            record: SecretRecord,
            value: SecretValue,
            expected_generation: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            self.inner.put(record, value, expected_generation).await
        }

        async fn load(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretValue, SecretStoreError> {
            self.inner.load(reference, purpose).await
        }

        async fn status(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretStatus, SecretStoreError> {
            self.inner.status(reference, purpose).await
        }

        async fn delete(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
            expected_generation: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            if self.fail_next_delete.swap(false, Ordering::SeqCst) {
                return Err(SecretStoreError::Backend(
                    lettuce_settings::SecretBackendError::Unavailable,
                ));
            }
            self.inner
                .delete(reference, purpose, expected_generation)
                .await
        }
    }

    fn remote_config() -> AudioProviderConfig {
        AudioProviderConfig::OpenAiCompatible {
            base_url: Some("https://audio.example".into()),
            request_path: Some("/v1/audio/speech".into()),
        }
    }

    #[tokio::test]
    async fn remote_provider_secret_rotates_and_deletes_with_voices() {
        let database = Database::open_in_memory().expect("database");
        let secrets = InMemorySecretStore::new();
        let coordinator = TtsConfigurationCoordinator::new(&database, &secrets);
        let provider = coordinator
            .create_audio_provider(
                "Primary".into(),
                remote_config(),
                Some(SecretValue::new("first-key").expect("secret")),
                TimestampMillis::new(10),
            )
            .await
            .expect("provider");
        let reference = provider.api_key_ref.expect("secret reference");
        let purpose = audio_purpose(provider.secret_owner_id);
        let status = secrets.status(&reference, &purpose).await.expect("status");
        assert_eq!(status.generation, 1);
        assert_eq!(
            secrets
                .load(&reference, &purpose)
                .await
                .expect("load")
                .with(|value| value.to_owned()),
            "first-key"
        );

        let voice = coordinator
            .create_user_voice(
                provider.id,
                "Narrator".into(),
                "voice-model".into(),
                "voice-one".into(),
                None,
                TimestampMillis::new(11),
            )
            .expect("voice");
        assert_eq!(
            coordinator
                .rotate_audio_api_key(
                    provider.id,
                    SecretValue::new("second-key").expect("secret"),
                    1,
                )
                .await
                .expect("rotate"),
            2
        );
        coordinator
            .delete_audio_provider(provider.id, Revision::INITIAL)
            .await
            .expect("delete provider");
        assert_eq!(
            secrets
                .status(&reference, &purpose)
                .await
                .expect("status")
                .state,
            SecretState::Missing
        );
        assert_eq!(database.get_user_voice(voice.id).expect("voice"), None);
    }

    #[tokio::test]
    async fn local_provider_refuses_plaintext_secret() {
        let database = Database::open_in_memory().expect("database");
        let secrets = InMemorySecretStore::new();
        let coordinator = TtsConfigurationCoordinator::new(&database, &secrets);
        assert_eq!(
            coordinator
                .create_audio_provider(
                    "Local".into(),
                    AudioProviderConfig::Kokoro {
                        variant: Some("fp16".into()),
                    },
                    Some(SecretValue::new("not-allowed").expect("secret")),
                    TimestampMillis::new(1),
                )
                .await,
            Err(TtsConfigurationCoordinatorError::InvalidInput)
        );
        assert!(
            coordinator
                .list_audio_providers()
                .expect("providers")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn failed_delete_returns_a_receipt_that_finishes_exact_cleanup() {
        let database = Database::open_in_memory().expect("database");
        let secrets = FailingDeleteSecretStore::new();
        let coordinator = TtsConfigurationCoordinator::new(&database, &secrets);
        let provider = coordinator
            .create_audio_provider(
                "Primary".into(),
                remote_config(),
                Some(SecretValue::new("api-key").expect("secret")),
                TimestampMillis::new(1),
            )
            .await
            .expect("provider");
        let reference = provider.api_key_ref.expect("reference");
        let purpose = audio_purpose(provider.secret_owner_id);
        let error = coordinator
            .delete_audio_provider(provider.id, provider.revision)
            .await
            .expect_err("first cleanup fails");
        let TtsConfigurationCoordinatorError::CleanupPending { cleanup, source } = error else {
            panic!("expected pending cleanup");
        };
        assert_eq!(
            source,
            SecretStoreError::Backend(lettuce_settings::SecretBackendError::Unavailable)
        );
        assert_eq!(
            database.get_audio_provider(provider.id).expect("provider"),
            None
        );
        assert_eq!(
            secrets
                .status(&reference, &purpose)
                .await
                .expect("status")
                .state,
            SecretState::Present
        );
        coordinator
            .retry_secret_cleanup(cleanup)
            .await
            .expect("cleanup retry");
        assert_eq!(
            secrets
                .status(&reference, &purpose)
                .await
                .expect("status")
                .state,
            SecretState::Missing
        );
    }
}
