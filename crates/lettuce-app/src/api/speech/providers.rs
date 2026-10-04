use std::sync::Arc;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_network::JsonClient;
use lettuce_settings::SecretValue;
use lettuce_speech::{
    AudioProvider, AudioProviderConfig, AudioProviderKind, AudioProviderVerificationError,
    AudioProviderVerifier, DiscoveredVoiceDraft, TtsConfigurationRepository, VoiceDiscovery,
    VoiceDiscoveryError,
};
use lettuce_types::AudioProviderId;

use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};

fn configuration(value: dto::AudioProviderConfiguration) -> AudioProviderConfig {
    match value {
        dto::AudioProviderConfiguration::Gemini { project_id, location } => AudioProviderConfig::Gemini { project_id, location },
        dto::AudioProviderConfiguration::Elevenlabs => AudioProviderConfig::Elevenlabs,
        dto::AudioProviderConfiguration::FishTts => AudioProviderConfig::FishTts,
        dto::AudioProviderConfiguration::FishSpeech { base_url, request_path } => AudioProviderConfig::FishSpeech { base_url, request_path },
        dto::AudioProviderConfiguration::OpenAiCompatible { base_url, request_path } => AudioProviderConfig::OpenAiCompatible { base_url, request_path },
        dto::AudioProviderConfiguration::Kokoro { variant } => AudioProviderConfig::Kokoro { variant },
    }
}

fn provider_kind(value: dto::AudioProviderType) -> AudioProviderKind {
    match value {
        dto::AudioProviderType::Gemini => AudioProviderKind::GeminiTts,
        dto::AudioProviderType::Elevenlabs => AudioProviderKind::Elevenlabs,
        dto::AudioProviderType::FishTts => AudioProviderKind::FishTts,
        dto::AudioProviderType::FishSpeech => AudioProviderKind::FishSpeech,
        dto::AudioProviderType::OpenAiCompatible => AudioProviderKind::OpenAiTts,
        dto::AudioProviderType::Kokoro => AudioProviderKind::Kokoro,
    }
}

struct Control(Arc<JsonClient>);

impl Control {
    fn new(context: &ApiContext) -> Result<Self, ApiError> {
        let policy = context.backend().tls_policy()
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        Ok(Self(Arc::new(JsonClient::with_tls(&policy)
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?)))
    }
}

#[async_trait]
impl AudioProviderVerifier for Control {
    async fn verify_audio_provider(
        &self,
        provider: &AudioProvider,
        credential: Option<&SecretValue>,
    ) -> Result<bool, AudioProviderVerificationError> {
        match provider.config.provider_kind() {
            AudioProviderKind::Elevenlabs => lettuce_speech::ElevenLabsTtsRuntime::new(self.0.clone()).verify_audio_provider(provider, credential).await,
            AudioProviderKind::FishTts => lettuce_speech::FishTtsRuntime::new(self.0.clone()).verify_audio_provider(provider, credential).await,
            AudioProviderKind::FishSpeech => lettuce_speech::FishSpeechTtsRuntime::new(self.0.clone()).verify_audio_provider(provider, credential).await,
            AudioProviderKind::GeminiTts => lettuce_speech::GeminiTtsRuntime::new(self.0.clone()).verify_audio_provider(provider, credential).await,
            AudioProviderKind::OpenAiTts => lettuce_speech::OpenAiCompatibleTtsRuntime::new(self.0.clone()).verify_audio_provider(provider, credential).await,
            AudioProviderKind::Kokoro => Ok(true),
        }
    }
}

#[async_trait]
impl VoiceDiscovery for Control {
    async fn fetch_configured_voices(
        &self,
        provider: &AudioProvider,
        credential: &SecretValue,
    ) -> Result<Vec<DiscoveredVoiceDraft>, VoiceDiscoveryError> {
        match provider.config.provider_kind() {
            AudioProviderKind::Elevenlabs => lettuce_speech::ElevenLabsTtsRuntime::new(self.0.clone()).fetch_configured_voices(provider, credential).await,
            AudioProviderKind::FishTts => lettuce_speech::FishTtsRuntime::new(self.0.clone()).fetch_configured_voices(provider, credential).await,
            _ => Ok(Vec::new()),
        }
    }
}

impl IntoApiError for crate::TtsProviderVerificationError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Configuration(error) => error.into_api_error(),
            Self::SecretStore(_) => super::errors::secret_missing(),
            Self::InvalidInput | Self::Verification(AudioProviderVerificationError::InvalidInput) => invalid_field("draft", self.to_string()),
            Self::Verification(AudioProviderVerificationError::Unavailable) => api_error(ApiErrorCode::Unavailable, self.to_string()),
        }
    }
}

impl IntoApiError for crate::TtsVoiceRefreshError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Configuration(error) => error.into_api_error(),
            Self::SecretStore(_) => super::errors::secret_missing(),
            Self::InvalidInput => invalid_field("provider_id", self.to_string()),
            Self::Repository(_) => api_error(ApiErrorCode::Internal, self.to_string()),
            Self::Discovery(_) => api_error(ApiErrorCode::Unavailable, self.to_string()),
        }
    }
}

pub async fn audio_provider_verify(
    context: &ApiContext,
    request: dto::AudioProviderVerifyRequest,
) -> Result<bool, ApiError> {
    context.blocking(move |context| {
        let verifier = Control::new(context)?;
        tokio::runtime::Handle::current().block_on(async {
            match request {
                dto::AudioProviderVerifyRequest::Saved { provider_id } => {
                    let id: AudioProviderId = parse_id(&provider_id, "provider_id")?;
                    context.backend().tts_provider_verification(context.secret_store().as_ref())
                        .verify(id, &verifier).await.map_err(IntoApiError::into_api_error)
                }
                dto::AudioProviderVerifyRequest::Draft { draft } => {
                    let credential = draft.api_key.map(SecretValue::new).transpose()
                        .map_err(|_| invalid_field("draft.api_key", "the API key is empty"))?;
                    crate::TtsProviderVerificationCoordinator::<lettuce_database::Database, dyn lettuce_settings::SecretStore>::verify_draft(
                        configuration(draft.configuration), credential.as_ref(), &verifier,
                    ).await.map_err(IntoApiError::into_api_error)
                }
            }
        })
    }).await
}

fn voice_view(voice: lettuce_speech::DiscoveredVoice) -> dto::AudioVoiceView {
    dto::AudioVoiceView { voice_id: voice.voice_id, name: voice.name, preview_url: voice.preview_url, labels: voice.labels }
}

pub async fn audio_provider_voices_refresh(
    context: &ApiContext,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let id: AudioProviderId = parse_id(&request.provider_id, "provider_id")?;
    context.blocking(move |context| {
        let discovery = Control::new(context)?;
        tokio::runtime::Handle::current().block_on(
            context.backend().tts_voice_refresh(context.secret_store().as_ref())
                .refresh(id, &discovery, context.now()),
        ).map(|voices| voices.into_iter().map(voice_view).collect())
            .map_err(IntoApiError::into_api_error)
    }).await
}

pub async fn audio_provider_voices(
    context: &ApiContext,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let id: AudioProviderId = parse_id(&request.provider_id, "provider_id")?;
    context.blocking(move |context| {
        let provider = context.backend().database().get_audio_provider(id)
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the audio provider was not found"))?;
        if matches!(provider.config, AudioProviderConfig::Gemini { .. }) {
            return Ok(lettuce_speech::tts_catalog_voices(AudioProviderKind::GeminiTts)
                .into_iter().map(|voice| dto::AudioVoiceView {
                    voice_id: voice.id.to_owned(), name: voice.name.to_owned(), preview_url: None,
                    labels: voice.labels.into_iter().map(|(key, value)| (key.to_owned(), value.to_owned())).collect(),
                }).collect());
        }
        context.backend().tts_voice_refresh(context.secret_store().as_ref()).list(id)
            .map(|voices| voices.into_iter().map(voice_view).collect()).map_err(IntoApiError::into_api_error)
    }).await
}

pub async fn tts_models(_context: &ApiContext, request: dto::TtsModelsRequest) -> Result<Vec<dto::TtsModelView>, ApiError> {
    Ok(lettuce_speech::tts_catalog_models(provider_kind(request.provider_type))
        .into_iter().map(|model| dto::TtsModelView { id: model.id.to_owned(), name: model.name.to_owned() }).collect())
}

pub async fn tts_voice_design_models(_context: &ApiContext, request: dto::TtsModelsRequest) -> Result<Vec<dto::TtsModelView>, ApiError> {
    Ok(lettuce_speech::tts_voice_design_models(provider_kind(request.provider_type))
        .into_iter().map(|model| dto::TtsModelView { id: model.id.to_owned(), name: model.name.to_owned() }).collect())
}

pub async fn tts_cache_stats(context: &ApiContext) -> Result<dto::TtsCacheStats, ApiError> {
    context.blocking(|context| {
        let stats = crate::tts_audio_cache_stats(context.backend().database())
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        Ok(dto::TtsCacheStats { count: stats.count, size_bytes: stats.size_bytes })
    }).await
}

pub async fn tts_cache_clear(context: &ApiContext) -> Result<(), ApiError> {
    context.blocking(|context| {
        let media = context.media().ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
        crate::clear_tts_audio_cache(context.backend().database(), media, context.now())
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        Ok(())
    }).await
}
