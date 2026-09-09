use async_trait::async_trait;
use lettuce_settings::SecretValue;

use crate::AudioProvider;

#[async_trait]
pub trait AudioProviderVerifier: Send + Sync {
    async fn verify_audio_provider(
        &self,
        provider: &AudioProvider,
        credential: Option<&SecretValue>,
    ) -> Result<bool, AudioProviderVerificationError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AudioProviderVerificationError {
    #[error("audio provider verification input is invalid")]
    InvalidInput,
    #[error("audio provider verification is temporarily unavailable")]
    Unavailable,
}
