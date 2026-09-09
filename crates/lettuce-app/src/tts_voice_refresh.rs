use lettuce_settings::{SecretPurpose, SecretStore, SecretStoreError};
use lettuce_speech::{
    AudioProviderConfig, DiscoveredVoice, DiscoveredVoiceRepository, TtsConfigurationRepository,
    TtsConfigurationRepositoryError, VoiceDiscovery, VoiceDiscoveryError,
    VoiceDiscoveryRepositoryError, materialize_discovered_voices,
};
use lettuce_types::{AudioProviderId, TimestampMillis};

#[derive(Debug)]
pub struct TtsVoiceRefreshCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secrets: &'a S,
}

impl<'a, R: ?Sized, S: ?Sized> TtsVoiceRefreshCoordinator<'a, R, S> {
    #[must_use]
    pub const fn new(repository: &'a R, secrets: &'a S) -> Self {
        Self {
            repository,
            secrets,
        }
    }
}

impl<R, S> TtsVoiceRefreshCoordinator<'_, R, S>
where
    R: TtsConfigurationRepository + DiscoveredVoiceRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    pub fn list(
        &self,
        provider_id: AudioProviderId,
    ) -> Result<Vec<DiscoveredVoice>, TtsVoiceRefreshError> {
        self.repository
            .list_discovered_voices(provider_id)
            .map_err(TtsVoiceRefreshError::Repository)
    }

    pub async fn refresh<D: VoiceDiscovery + ?Sized>(
        &self,
        provider_id: AudioProviderId,
        discovery: &D,
        now: TimestampMillis,
    ) -> Result<Vec<DiscoveredVoice>, TtsVoiceRefreshError> {
        let provider = self
            .repository
            .get_audio_provider(provider_id)
            .map_err(TtsVoiceRefreshError::Configuration)?
            .ok_or(TtsVoiceRefreshError::Configuration(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if !matches!(
            &provider.config,
            AudioProviderConfig::Elevenlabs | AudioProviderConfig::FishTts
        ) {
            return Err(TtsVoiceRefreshError::InvalidInput);
        }
        let reference = provider
            .api_key_ref
            .ok_or(TtsVoiceRefreshError::InvalidInput)?;
        let credential = self
            .secrets
            .load(
                &reference,
                &SecretPurpose::AudioApiKey {
                    owner: provider.secret_owner_id,
                },
            )
            .await
            .map_err(TtsVoiceRefreshError::SecretStore)?;
        let drafts = discovery
            .fetch_configured_voices(&provider, &credential)
            .await
            .map_err(TtsVoiceRefreshError::Discovery)?;
        let voices = materialize_discovered_voices(provider.id, drafts, now)
            .map_err(TtsVoiceRefreshError::Discovery)?;
        self.repository
            .replace_discovered_voices(provider.id, voices)
            .map_err(TtsVoiceRefreshError::Repository)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TtsVoiceRefreshError {
    #[error("voice refresh input is invalid")]
    InvalidInput,
    #[error("TTS configuration persistence failed: {0}")]
    Configuration(TtsConfigurationRepositoryError),
    #[error("TTS secret access failed: {0}")]
    SecretStore(SecretStoreError),
    #[error("voice discovery failed: {0}")]
    Discovery(VoiceDiscoveryError),
    #[error("voice cache persistence failed: {0}")]
    Repository(VoiceDiscoveryRepositoryError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_settings::{InMemorySecretStore, SecretValue};
    use lettuce_speech::{AudioProvider, DiscoveredVoiceDraft};

    use crate::TtsConfigurationCoordinator;

    use super::*;

    struct Discovery {
        outcome: Result<Vec<DiscoveredVoiceDraft>, VoiceDiscoveryError>,
    }

    #[async_trait]
    impl VoiceDiscovery for Discovery {
        async fn fetch_configured_voices(
            &self,
            _: &AudioProvider,
            credential: &SecretValue,
        ) -> Result<Vec<DiscoveredVoiceDraft>, VoiceDiscoveryError> {
            credential.with(|value| assert_eq!(value, "configured-secret-canary"));
            self.outcome.clone()
        }
    }

    fn draft(id: &str) -> DiscoveredVoiceDraft {
        DiscoveredVoiceDraft {
            voice_id: id.into(),
            name: format!("Voice {id}"),
            preview_url: Some(format!("https://audio.example/{id}.mp3")),
            labels: BTreeMap::from([("category".into(), "professional".into())]),
        }
    }

    #[tokio::test]
    async fn refreshes_supported_providers_and_retains_cache_on_fetch_failure() {
        let database = Database::open_in_memory().expect("database");
        let secrets = InMemorySecretStore::new();
        let provider = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "ElevenLabs".into(),
                AudioProviderConfig::Elevenlabs,
                Some(SecretValue::new("configured-secret-canary").expect("secret")),
                TimestampMillis::new(1),
            )
            .await
            .expect("provider");
        let coordinator = TtsVoiceRefreshCoordinator::new(&database, &secrets);
        let voices = coordinator
            .refresh(
                provider.id,
                &Discovery {
                    outcome: Ok(vec![draft("one"), draft("two")]),
                },
                TimestampMillis::new(2),
            )
            .await
            .expect("refresh");
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[0].ordinal, 0);
        assert_eq!(voices[1].ordinal, 1);
        assert_eq!(
            coordinator
                .refresh(
                    provider.id,
                    &Discovery {
                        outcome: Err(VoiceDiscoveryError::Unavailable),
                    },
                    TimestampMillis::new(3),
                )
                .await,
            Err(TtsVoiceRefreshError::Discovery(
                VoiceDiscoveryError::Unavailable
            ))
        );
        assert_eq!(
            coordinator.list(provider.id).expect("cached voices"),
            voices
        );
        let fish = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "Fish Audio".into(),
                AudioProviderConfig::FishTts,
                Some(SecretValue::new("configured-secret-canary").expect("secret")),
                TimestampMillis::new(4),
            )
            .await
            .expect("Fish provider");
        assert_eq!(
            coordinator
                .refresh(
                    fish.id,
                    &Discovery {
                        outcome: Ok(vec![draft("fish-voice")]),
                    },
                    TimestampMillis::new(5),
                )
                .await
                .expect("Fish refresh")[0]
                .voice_id,
            "fish-voice"
        );
    }
}
