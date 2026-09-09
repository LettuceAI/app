use lettuce_settings::{SecretPurpose, SecretStore, SecretStoreError};
use lettuce_speech::{
    AudioProviderConfig, AudioProviderVerificationError, AudioProviderVerifier,
    TtsConfigurationRepository, TtsConfigurationRepositoryError,
};
use lettuce_types::AudioProviderId;

#[derive(Debug)]
pub struct TtsProviderVerificationCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secrets: &'a S,
}

impl<'a, R: ?Sized, S: ?Sized> TtsProviderVerificationCoordinator<'a, R, S> {
    #[must_use]
    pub const fn new(repository: &'a R, secrets: &'a S) -> Self {
        Self {
            repository,
            secrets,
        }
    }
}

impl<R, S> TtsProviderVerificationCoordinator<'_, R, S>
where
    R: TtsConfigurationRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    pub async fn verify<V: AudioProviderVerifier + ?Sized>(
        &self,
        provider_id: AudioProviderId,
        verifier: &V,
    ) -> Result<bool, TtsProviderVerificationError> {
        let provider = self
            .repository
            .get_audio_provider(provider_id)
            .map_err(TtsProviderVerificationError::Configuration)?
            .ok_or(TtsProviderVerificationError::Configuration(
                TtsConfigurationRepositoryError::NotFound,
            ))?;
        if !matches!(
            &provider.config,
            AudioProviderConfig::Elevenlabs
                | AudioProviderConfig::FishTts
                | AudioProviderConfig::Gemini { .. }
                | AudioProviderConfig::FishSpeech { .. }
        ) {
            return Err(TtsProviderVerificationError::InvalidInput);
        }
        let credential = match provider.api_key_ref {
            Some(reference) => Some(
                self.secrets
                    .load(
                        &reference,
                        &SecretPurpose::AudioApiKey {
                            owner: provider.secret_owner_id,
                        },
                    )
                    .await
                    .map_err(TtsProviderVerificationError::SecretStore)?,
            ),
            None => None,
        };
        verifier
            .verify_audio_provider(&provider, credential.as_ref())
            .await
            .map_err(TtsProviderVerificationError::Verification)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TtsProviderVerificationError {
    #[error("audio provider verification input is invalid")]
    InvalidInput,
    #[error("TTS configuration persistence failed: {0}")]
    Configuration(TtsConfigurationRepositoryError),
    #[error("TTS secret access failed: {0}")]
    SecretStore(SecretStoreError),
    #[error("audio provider verification failed: {0}")]
    Verification(AudioProviderVerificationError),
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_settings::{InMemorySecretStore, SecretValue};
    use lettuce_speech::{AudioProvider, AudioProviderConfig};
    use lettuce_types::TimestampMillis;

    use crate::TtsConfigurationCoordinator;

    use super::*;

    struct Verifier;

    #[async_trait]
    impl AudioProviderVerifier for Verifier {
        async fn verify_audio_provider(
            &self,
            provider: &AudioProvider,
            credential: Option<&SecretValue>,
        ) -> Result<bool, AudioProviderVerificationError> {
            assert!(matches!(
                provider.config,
                AudioProviderConfig::Elevenlabs
                    | AudioProviderConfig::FishTts
                    | AudioProviderConfig::Gemini { .. }
                    | AudioProviderConfig::FishSpeech { .. }
            ));
            if matches!(provider.config, AudioProviderConfig::FishSpeech { .. }) {
                assert!(credential.is_none());
            } else {
                credential
                    .expect("credential")
                    .with(|value| assert_eq!(value, "verification-secret-canary"));
            }
            Ok(true)
        }
    }

    #[tokio::test]
    async fn verifies_existing_provider_with_its_scoped_secret() {
        let database = Database::open_in_memory().expect("database");
        let secrets = InMemorySecretStore::new();
        let provider = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "ElevenLabs".into(),
                AudioProviderConfig::Elevenlabs,
                Some(SecretValue::new("verification-secret-canary").expect("secret")),
                TimestampMillis::new(1),
            )
            .await
            .expect("provider");
        assert!(
            TtsProviderVerificationCoordinator::new(&database, &secrets)
                .verify(provider.id, &Verifier)
                .await
                .expect("verification")
        );
        let fish = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "Fish Audio".into(),
                AudioProviderConfig::FishTts,
                Some(SecretValue::new("verification-secret-canary").expect("secret")),
                TimestampMillis::new(2),
            )
            .await
            .expect("Fish provider");
        assert!(
            TtsProviderVerificationCoordinator::new(&database, &secrets)
                .verify(fish.id, &Verifier)
                .await
                .expect("Fish verification")
        );
        let gemini = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "Gemini TTS".into(),
                AudioProviderConfig::Gemini {
                    project_id: Some("project-canary".into()),
                    location: "europe-west4".into(),
                },
                Some(SecretValue::new("verification-secret-canary").expect("secret")),
                TimestampMillis::new(3),
            )
            .await
            .expect("Gemini provider");
        assert!(
            TtsProviderVerificationCoordinator::new(&database, &secrets)
                .verify(gemini.id, &Verifier)
                .await
                .expect("Gemini verification")
        );
        let fish_speech = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "Fish Speech".into(),
                AudioProviderConfig::FishSpeech {
                    base_url: Some("http://127.0.0.1:8080".into()),
                    request_path: None,
                },
                None,
                TimestampMillis::new(4),
            )
            .await
            .expect("Fish Speech provider");
        assert!(
            TtsProviderVerificationCoordinator::new(&database, &secrets)
                .verify(fish_speech.id, &Verifier)
                .await
                .expect("Fish Speech verification")
        );
    }
}
