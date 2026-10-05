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
    pub async fn verify_draft<V: AudioProviderVerifier + ?Sized>(
        config: AudioProviderConfig,
        credential: Option<&lettuce_settings::SecretValue>,
        verifier: &V,
    ) -> Result<bool, TtsProviderVerificationError> {
        let provider = lettuce_speech::AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: lettuce_settings::SecretOwnerId::new(),
            label: "Draft".to_owned(),
            api_key_ref: None,
            config,
            revision: lettuce_types::Revision::INITIAL,
            created_at: lettuce_types::TimestampMillis::new(0),
            updated_at: lettuce_types::TimestampMillis::new(0),
        };
        provider.validate().map_err(|_| TtsProviderVerificationError::InvalidInput)?;
        if matches!(provider.config, AudioProviderConfig::Kokoro { .. }) {
            return if credential.is_none() { Ok(true) } else { Err(TtsProviderVerificationError::InvalidInput) };
        }
        verifier.verify_audio_provider(&provider, credential).await
            .map_err(TtsProviderVerificationError::Verification)
    }

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
        if matches!(provider.config, AudioProviderConfig::Kokoro { .. }) {
            return Ok(true);
        }
        if !matches!(
            &provider.config,
            AudioProviderConfig::Elevenlabs
                | AudioProviderConfig::FishTts
                | AudioProviderConfig::Gemini { .. }
                | AudioProviderConfig::FishSpeech { .. }
                | AudioProviderConfig::OpenAiCompatible { .. }
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
                    | AudioProviderConfig::OpenAiCompatible { .. }
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
        let open_ai = TtsConfigurationCoordinator::new(&database, &secrets)
            .create_audio_provider(
                "OpenAI-compatible".into(),
                AudioProviderConfig::OpenAiCompatible {
                    base_url: Some("https://speech.example".into()),
                    request_path: None,
                },
                Some(SecretValue::new("verification-secret-canary").expect("secret")),
                TimestampMillis::new(5),
            )
            .await
            .expect("OpenAI-compatible provider");
        assert!(
            TtsProviderVerificationCoordinator::new(&database, &secrets)
                .verify(open_ai.id, &Verifier)
                .await
                .expect("OpenAI-compatible verification")
        );
    }
    #[tokio::test]
    async fn verification_of_a_draft_does_not_save_configuration_or_secret() {
        let database = Database::open_in_memory().expect("database");
        let key = SecretValue::new("verification-secret-canary").expect("secret");
        assert!(TtsProviderVerificationCoordinator::<Database, InMemorySecretStore>::verify_draft(
            AudioProviderConfig::Elevenlabs, Some(&key), &Verifier,
        ).await.expect("draft verified"));
        assert!(lettuce_speech::TtsConfigurationRepository::list_audio_providers(&database)
            .expect("providers").is_empty());
    }

}
