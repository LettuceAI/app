use std::sync::Arc;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_network::JsonClient;
use lettuce_settings::SecretValue;
use lettuce_speech::{
    AudioProvider, AudioProviderConfig, AudioProviderKind, AudioProviderVerificationError,
    AudioProviderVerifier, DiscoveredVoiceDraft, TtsConfigurationRepository, VoiceDiscovery,
    VoiceDiscoveryError, VoiceSearch,
};
use lettuce_types::AudioProviderId;

use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};

fn configuration(value: dto::AudioProviderConfiguration) -> AudioProviderConfig {
    match value {
        dto::AudioProviderConfiguration::Gemini {
            project_id,
            location,
        } => AudioProviderConfig::Gemini {
            project_id,
            location,
        },
        dto::AudioProviderConfiguration::Elevenlabs => AudioProviderConfig::Elevenlabs,
        dto::AudioProviderConfiguration::FishTts => AudioProviderConfig::FishTts,
        dto::AudioProviderConfiguration::FishSpeech {
            base_url,
            request_path,
        } => AudioProviderConfig::FishSpeech {
            base_url,
            request_path,
        },
        dto::AudioProviderConfiguration::OpenAiCompatible {
            base_url,
            request_path,
        } => AudioProviderConfig::OpenAiCompatible {
            base_url,
            request_path,
        },
        dto::AudioProviderConfiguration::Kokoro { variant } => {
            AudioProviderConfig::Kokoro { variant }
        }
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
        let policy = context
            .backend()
            .tls_policy()
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        Ok(Self(Arc::new(JsonClient::with_tls(&policy).map_err(
            |error| api_error(ApiErrorCode::Unavailable, error.to_string()),
        )?)))
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
            AudioProviderKind::Elevenlabs => {
                lettuce_speech::ElevenLabsTtsRuntime::new(self.0.clone())
                    .verify_audio_provider(provider, credential)
                    .await
            }
            AudioProviderKind::FishTts => {
                lettuce_speech::FishTtsRuntime::new(self.0.clone())
                    .verify_audio_provider(provider, credential)
                    .await
            }
            AudioProviderKind::FishSpeech => {
                lettuce_speech::FishSpeechTtsRuntime::new(self.0.clone())
                    .verify_audio_provider(provider, credential)
                    .await
            }
            AudioProviderKind::GeminiTts => {
                lettuce_speech::GeminiTtsRuntime::new(self.0.clone())
                    .verify_audio_provider(provider, credential)
                    .await
            }
            AudioProviderKind::OpenAiTts => {
                lettuce_speech::OpenAiCompatibleTtsRuntime::new(self.0.clone())
                    .verify_audio_provider(provider, credential)
                    .await
            }
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
            AudioProviderKind::Elevenlabs => {
                lettuce_speech::ElevenLabsTtsRuntime::new(self.0.clone())
                    .fetch_configured_voices(provider, credential)
                    .await
            }
            AudioProviderKind::FishTts => {
                lettuce_speech::FishTtsRuntime::new(self.0.clone())
                    .fetch_configured_voices(provider, credential)
                    .await
            }
            _ => Ok(Vec::new()),
        }
    }
}

impl IntoApiError for crate::TtsProviderVerificationError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Configuration(error) => error.into_api_error(),
            Self::SecretStore(_) => super::errors::secret_missing(),
            Self::InvalidInput
            | Self::Verification(AudioProviderVerificationError::InvalidInput) => {
                invalid_field("draft", self.to_string())
            }
            Self::Verification(AudioProviderVerificationError::Unavailable) => {
                api_error(ApiErrorCode::Unavailable, self.to_string())
            }
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
    context
        .blocking(move |context| {
            let verifier = Control::new(context)?;
            tokio::runtime::Handle::current().block_on(async {
                match request {
                    dto::AudioProviderVerifyRequest::Saved { provider_id } => {
                        let id: AudioProviderId = parse_id(&provider_id, "provider_id")?;
                        context
                            .backend()
                            .tts_provider_verification(context.secret_store().as_ref())
                            .verify(id, &verifier)
                            .await
                            .map_err(IntoApiError::into_api_error)
                    }
                    dto::AudioProviderVerifyRequest::Draft { draft } => {
                        let credential = draft
                            .api_key
                            .map(SecretValue::new)
                            .transpose()
                            .map_err(|_| invalid_field("draft.api_key", "the API key is empty"))?;
                        crate::TtsProviderVerificationCoordinator::<
                            lettuce_database::Database,
                            dyn lettuce_settings::SecretStore,
                        >::verify_draft(
                            configuration(draft.configuration),
                            credential.as_ref(),
                            &verifier,
                        )
                        .await
                        .map_err(IntoApiError::into_api_error)
                    }
                }
            })
        })
        .await
}

fn voice_view(voice: lettuce_speech::DiscoveredVoice) -> dto::AudioVoiceView {
    dto::AudioVoiceView {
        voice_id: voice.voice_id,
        name: voice.name,
        preview_url: voice.preview_url,
        labels: voice.labels,
    }
}

fn built_in_provider_voices(
    context: &ApiContext,
    provider: &AudioProvider,
) -> Result<Option<Vec<dto::AudioVoiceView>>, ApiError> {
    match &provider.config {
        AudioProviderConfig::Gemini { .. } => Ok(Some(
            lettuce_speech::tts_catalog_voices(AudioProviderKind::GeminiTts)
                .into_iter()
                .map(|voice| dto::AudioVoiceView {
                    voice_id: voice.id.to_owned(),
                    name: voice.name.to_owned(),
                    preview_url: None,
                    labels: voice
                        .labels
                        .into_iter()
                        .map(|(key, value)| (key.to_owned(), value.to_owned()))
                        .collect(),
                })
                .collect(),
        )),
        AudioProviderConfig::Kokoro { .. } => {
            let root = context.retained_model_roots()?.kokoro.ok_or_else(|| {
                api_error(ApiErrorCode::Unavailable, "the Kokoro root is unavailable")
            })?;
            let voices = crate::KokoroAssetInventoryCoordinator::open_managed(root)
                .and_then(|inventory| inventory.installed_voices())
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            Ok(Some(
                voices
                    .into_iter()
                    .map(|voice| dto::AudioVoiceView {
                        name: voice.id.clone(),
                        voice_id: voice.id,
                        preview_url: None,
                        labels: [
                            ("category".into(), "library".into()),
                            ("engine".into(), "kokoro".into()),
                        ]
                        .into(),
                    })
                    .collect(),
            ))
        }
        AudioProviderConfig::OpenAiCompatible { .. } | AudioProviderConfig::FishSpeech { .. } => {
            Ok(Some(Vec::new()))
        }
        AudioProviderConfig::Elevenlabs | AudioProviderConfig::FishTts => Ok(None),
    }
}

pub async fn audio_provider_voices_refresh(
    context: &ApiContext,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let id: AudioProviderId = parse_id(&request.provider_id, "provider_id")?;
    context
        .blocking(move |context| {
            let provider = context
                .backend()
                .database()
                .get_audio_provider(id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::NotFound, "the audio provider was not found")
                })?;
            if let Some(voices) = built_in_provider_voices(context, &provider)? {
                return Ok(voices);
            }
            let discovery = Control::new(context)?;
            tokio::runtime::Handle::current()
                .block_on(
                    context
                        .backend()
                        .tts_voice_refresh(context.secret_store().as_ref())
                        .refresh(id, &discovery, context.now()),
                )
                .map(|voices| voices.into_iter().map(voice_view).collect())
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn audio_provider_voices(
    context: &ApiContext,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let id: AudioProviderId = parse_id(&request.provider_id, "provider_id")?;
    context
        .blocking(move |context| {
            let provider = context
                .backend()
                .database()
                .get_audio_provider(id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::NotFound, "the audio provider was not found")
                })?;
            if let Some(voices) = built_in_provider_voices(context, &provider)? {
                return Ok(voices);
            }
            context
                .backend()
                .tts_voice_refresh(context.secret_store().as_ref())
                .list(id)
                .map(|voices| voices.into_iter().map(voice_view).collect())
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn tts_models(
    _context: &ApiContext,
    request: dto::TtsModelsRequest,
) -> Result<Vec<dto::TtsModelView>, ApiError> {
    Ok(
        lettuce_speech::tts_catalog_models(provider_kind(request.provider_type))
            .into_iter()
            .map(|model| dto::TtsModelView {
                id: model.id.to_owned(),
                name: model.name.to_owned(),
            })
            .collect(),
    )
}

pub async fn tts_voice_design_models(
    _context: &ApiContext,
    request: dto::TtsModelsRequest,
) -> Result<Vec<dto::TtsModelView>, ApiError> {
    Ok(
        lettuce_speech::tts_voice_design_models(provider_kind(request.provider_type))
            .into_iter()
            .map(|model| dto::TtsModelView {
                id: model.id.to_owned(),
                name: model.name.to_owned(),
            })
            .collect(),
    )
}

pub async fn tts_cache_stats(context: &ApiContext) -> Result<dto::TtsCacheStats, ApiError> {
    context
        .blocking(|context| {
            let stats = crate::tts_audio_cache_stats(context.backend().database())
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            Ok(dto::TtsCacheStats {
                count: stats.count,
                size_bytes: stats.size_bytes,
            })
        })
        .await
}

pub async fn tts_cache_clear(context: &ApiContext) -> Result<(), ApiError> {
    context
        .blocking(|context| {
            let media = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            crate::clear_tts_audio_cache(context.backend().database(), media, context.now())
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            Ok(())
        })
        .await
}

impl IntoApiError for crate::TtsConfigurationCoordinatorError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(error) => error.into_api_error(),
            Self::InvalidInput => invalid_field("request", self.to_string()),
            Self::SecretStore(lettuce_settings::SecretStoreError::StaleGeneration) => api_error(
                ApiErrorCode::Conflict,
                "the audio credential generation changed",
            ),
            Self::SecretStore(_) => super::errors::secret_missing(),
            Self::CleanupPending { .. } | Self::CompensationFailed { .. } => {
                api_error(ApiErrorCode::Unavailable, self.to_string())
            }
        }
    }
}

fn configuration_view(value: AudioProviderConfig) -> dto::AudioProviderConfiguration {
    match value {
        AudioProviderConfig::Gemini {
            project_id,
            location,
        } => dto::AudioProviderConfiguration::Gemini {
            project_id,
            location,
        },
        AudioProviderConfig::Elevenlabs => dto::AudioProviderConfiguration::Elevenlabs,
        AudioProviderConfig::FishTts => dto::AudioProviderConfiguration::FishTts,
        AudioProviderConfig::FishSpeech {
            base_url,
            request_path,
        } => dto::AudioProviderConfiguration::FishSpeech {
            base_url,
            request_path,
        },
        AudioProviderConfig::OpenAiCompatible {
            base_url,
            request_path,
        } => dto::AudioProviderConfiguration::OpenAiCompatible {
            base_url,
            request_path,
        },
        AudioProviderConfig::Kokoro { variant } => {
            dto::AudioProviderConfiguration::Kokoro { variant }
        }
    }
}

fn provider_view(provider: AudioProvider) -> dto::AudioProviderView {
    dto::AudioProviderView {
        id: provider.id.to_string(),
        label: provider.label,
        configuration: configuration_view(provider.config),
        has_api_key: provider.api_key_ref.is_some(),
        revision: provider.revision.get(),
    }
}

fn user_voice_view(voice: lettuce_speech::UserVoice) -> dto::UserVoiceView {
    dto::UserVoiceView {
        id: voice.id.to_string(),
        provider_id: voice.provider_id.to_string(),
        name: voice.name,
        model_id: voice.model_id,
        voice_id: voice.voice_id,
        prompt: voice.prompt,
        revision: voice.revision.get(),
    }
}

fn revision(value: u64) -> Result<lettuce_types::Revision, ApiError> {
    if value == 0 {
        return Err(invalid_field(
            "expected_revision",
            "revision must be positive",
        ));
    }
    Ok(lettuce_types::Revision::new(value))
}

pub async fn audio_providers_list(
    context: &ApiContext,
) -> Result<Vec<dto::AudioProviderView>, ApiError> {
    context
        .blocking(|context| {
            context
                .backend()
                .database()
                .list_audio_providers()
                .map(|providers| providers.into_iter().map(provider_view).collect())
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn audio_provider_update(
    context: &ApiContext,
    request: dto::AudioProviderUpdateRequest,
) -> Result<dto::AudioProviderView, ApiError> {
    let id = parse_id(&request.provider_id, "provider_id")?;
    let expected = revision(request.expected_revision)?;
    context
        .blocking(move |context| {
            context
                .backend()
                .tts_configuration(context.secret_store().as_ref())
                .update_audio_provider(
                    id,
                    expected,
                    request.label,
                    configuration(request.configuration),
                    context.now(),
                )
                .map(provider_view)
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn audio_provider_delete(
    context: &ApiContext,
    request: dto::AudioProviderDeleteRequest,
) -> Result<(), ApiError> {
    let id = parse_id(&request.provider_id, "provider_id")?;
    let expected = revision(request.expected_revision)?;
    context
        .blocking(move |context| {
            tokio::runtime::Handle::current()
                .block_on(
                    context
                        .backend()
                        .tts_configuration(context.secret_store().as_ref())
                        .delete_audio_provider(id, expected),
                )
                .map(|_| ())
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn user_voices_list(context: &ApiContext) -> Result<Vec<dto::UserVoiceView>, ApiError> {
    context
        .blocking(|context| {
            context
                .backend()
                .database()
                .list_user_voices()
                .map(|voices| voices.into_iter().map(user_voice_view).collect())
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn user_voice_update(
    context: &ApiContext,
    request: dto::UserVoiceUpdateRequest,
) -> Result<dto::UserVoiceView, ApiError> {
    let id = parse_id(&request.id, "id")?;
    let provider_id = parse_id(&request.provider_id, "provider_id")?;
    let expected_revision = revision(request.expected_revision)?;
    context
        .blocking(move |context| {
            context
                .backend()
                .tts_configuration(context.secret_store().as_ref())
                .update_user_voice(crate::UpdateUserVoiceRequest {
                    id,
                    provider_id,
                    expected_revision,
                    name: request.name,
                    model_id: request.model_id,
                    voice_id: request.voice_id,
                    prompt: request.prompt,
                    now: context.now(),
                })
                .map(user_voice_view)
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn user_voice_delete(
    context: &ApiContext,
    request: dto::UserVoiceRequest,
) -> Result<(), ApiError> {
    let id = parse_id(&request.voice_id, "voice_id")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .tts_configuration(context.secret_store().as_ref())
                .delete_user_voice(id)
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

#[async_trait]
impl VoiceSearch for Control {
    async fn search_voices(
        &self,
        provider: &AudioProvider,
        credential: &SecretValue,
        search: &str,
    ) -> Result<Vec<DiscoveredVoiceDraft>, VoiceDiscoveryError> {
        lettuce_speech::ElevenLabsTtsRuntime::new(self.0.clone())
            .search_voices(provider, credential, search)
            .await
    }
}

pub async fn audio_provider_voices_search(
    context: &ApiContext,
    request: dto::AudioProviderVoiceSearchRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let id: AudioProviderId = parse_id(&request.provider_id, "provider_id")?;
    context
        .blocking(move |context| {
            let searcher = Control::new(context)?;
            tokio::runtime::Handle::current()
                .block_on(
                    context
                        .backend()
                        .tts_voice_refresh(context.secret_store().as_ref())
                        .search(id, &request.search, &searcher, context.now()),
                )
                .map(|voices| voices.into_iter().map(voice_view).collect())
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

impl IntoApiError for crate::TtsVoiceDesignError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::InvalidInput | Self::Validation(_) => invalid_field("request", self.to_string()),
            Self::Configuration(error) => error.into_api_error(),
            Self::SecretStore(_) => super::errors::secret_missing(),
            Self::Runtime(lettuce_speech::VoiceDesignRuntimeError::Cancelled) => {
                api_error(ApiErrorCode::Cancelled, self.to_string())
            }
            Self::Runtime(_) => api_error(ApiErrorCode::Unavailable, self.to_string()),
            Self::Audio(_) => api_error(ApiErrorCode::Internal, self.to_string()),
        }
    }
}

pub async fn voice_design_preview(
    context: &ApiContext,
    request: dto::VoiceDesignPreviewRequest,
) -> Result<Vec<dto::VoiceDesignPreviewView>, ApiError> {
    let provider_id = parse_id(&request.provider_id, "provider_id")?;
    context
        .blocking(move |context| {
            let media = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            let now = context.now();
            let coordinator = context
                .backend()
                .tts_voice_design(context.secret_store().as_ref());
            let admitted = coordinator
                .admit(crate::VoiceDesignPreviewDraft {
                    provider_id,
                    text_sample: request.text_sample,
                    voice_description: request.voice_description,
                    model_id: request.model_id,
                    num_previews: request.num_previews,
                    created_at: now,
                    expires_at: lettuce_types::TimestampMillis::new(
                        now.get()
                            .saturating_add(super::synthesize::PREVIEW_LIFETIME_MS),
                    ),
                })
                .map_err(IntoApiError::into_api_error)?;
            let client = Control::new(context)?;
            let runtime = lettuce_speech::ElevenLabsTtsRuntime::new(client.0);
            tokio::runtime::Handle::current()
                .block_on(coordinator.preview(
                    &admitted,
                    &runtime,
                    media,
                    context.shutdown_token(),
                    now,
                ))
                .map(|previews| {
                    previews
                        .into_iter()
                        .map(|preview| dto::VoiceDesignPreviewView {
                            generated_voice_id: preview.generated_voice_id,
                            audio: context.asset_ref(preview.audio_asset_id),
                            duration_secs: preview.duration_secs,
                        })
                        .collect()
                })
                .map_err(IntoApiError::into_api_error)
        })
        .await
}

pub async fn audio_provider_credential_status(
    context: &ApiContext,
    request: dto::AudioProviderRequest,
) -> Result<dto::AudioProviderCredentialStatus, ApiError> {
    let id = parse_id(&request.provider_id, "provider_id")?;
    context
        .blocking(move |context| {
            let provider = context
                .backend()
                .database()
                .get_audio_provider(id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::NotFound, "the audio provider was not found")
                })?;
            let Some(reference) = provider.api_key_ref else {
                return Ok(dto::AudioProviderCredentialStatus {
                    generation: 0,
                    available: false,
                });
            };
            let status = tokio::runtime::Handle::current()
                .block_on(context.secret_store().status(
                    &reference,
                    &lettuce_settings::SecretPurpose::AudioApiKey {
                        owner: provider.secret_owner_id,
                    },
                ))
                .map_err(|_| super::errors::secret_missing())?;
            Ok(dto::AudioProviderCredentialStatus {
                generation: status.generation,
                available: matches!(status.state, lettuce_settings::SecretState::Present),
            })
        })
        .await
}

pub async fn audio_provider_api_key_rotate(
    context: &ApiContext,
    request: dto::AudioProviderApiKeyRotateRequest,
) -> Result<dto::AudioProviderCredentialStatus, ApiError> {
    let id = parse_id(&request.provider_id, "provider_id")?;
    if request.expected_generation == 0 {
        return Err(invalid_field(
            "expected_generation",
            "the credential generation must be positive",
        ));
    }
    let value = SecretValue::new(request.api_key)
        .map_err(|_| invalid_field("api_key", "the API key is empty"))?;
    context
        .blocking(move |context| {
            let generation = tokio::runtime::Handle::current()
                .block_on(
                    context
                        .backend()
                        .tts_configuration(context.secret_store().as_ref())
                        .rotate_audio_api_key(id, value, request.expected_generation),
                )
                .map_err(IntoApiError::into_api_error)?;
            Ok(dto::AudioProviderCredentialStatus {
                generation,
                available: true,
            })
        })
        .await
}

pub async fn user_voice_create(
    context: &ApiContext,
    request: dto::UserVoiceCreateRequest,
) -> Result<dto::UserVoiceView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context
        .blocking(move |context| {
            let record = super::operations::commit(
                context,
                "user_voice_create",
                &request.client_operation_id,
                &digest,
                |transaction| {
                    let voice = lettuce_speech::UserVoice {
                        id: lettuce_types::VoiceProfileId::new(),
                        provider_id: parse_id(&request.provider_id, "provider_id")?,
                        name: request.name,
                        model_id: request.model_id,
                        voice_id: request.voice_id,
                        prompt: request.prompt,
                        revision: lettuce_types::Revision::INITIAL,
                        created_at: context.now(),
                        updated_at: context.now(),
                    };
                    transaction
                        .create_user_voice(&voice)
                        .map_err(IntoApiError::into_api_error)?;
                    Ok(super::operations::StoredRecord::new(
                        voice.id,
                        Some(voice.revision.get()),
                    ))
                },
            )?;
            context
                .backend()
                .database()
                .get_user_voice(parse_id(&record.id, "id")?)
                .map_err(IntoApiError::into_api_error)?
                .map(user_voice_view)
                .ok_or_else(|| super::operations::applied_deleted("user_voice_create", &record.id))
        })
        .await
}

const SPEECH_CREATE_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x6c65_7474_7563_6553_7065_6563_6843_7274);

pub(super) fn provider_create_ids(
    key: &str,
) -> (
    AudioProviderId,
    lettuce_settings::SecretOwnerId,
    lettuce_settings::SecretRef,
) {
    let id = uuid::Uuid::new_v5(&SPEECH_CREATE_NAMESPACE, key.as_bytes());
    let reference = uuid::Uuid::new_v5(&id, b"audio-api-key");
    (
        AudioProviderId::from_uuid(id),
        lettuce_settings::SecretOwnerId::from_uuid(id),
        lettuce_settings::SecretRef::from_uuid(reference),
    )
}

fn secret_store_failure(error: lettuce_settings::SecretStoreError) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::Speech {
            failure: dto::SpeechFailure::SecretStoreUnavailable,
        }),
    }
}

fn fill_missing_provider_key(
    context: &ApiContext,
    provider: &AudioProvider,
    credential: Option<SecretValue>,
) -> Result<(), ApiError> {
    let (Some(reference), Some(value)) = (provider.api_key_ref, credential) else {
        return Ok(());
    };
    tokio::runtime::Handle::current().block_on(async {
        let store = context.secret_store();
        let purpose = lettuce_settings::SecretPurpose::AudioApiKey {
            owner: provider.secret_owner_id,
        };
        let status = store
            .status(&reference, &purpose)
            .await
            .map_err(secret_store_failure)?;
        if status.state == lettuce_settings::SecretState::Missing {
            store
                .put(
                    lettuce_settings::SecretRecord::new(reference, purpose),
                    value,
                    None,
                )
                .await
                .map_err(secret_store_failure)?;
        }
        Ok(())
    })
}

pub async fn audio_provider_create(
    context: &ApiContext,
    request: dto::AudioProviderCreateRequest,
) -> Result<dto::AudioProviderView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context
        .blocking(move |context| {
            let _creation = context.speech_state().provider_creation();
            let prior = super::operations::replay::<super::operations::StoredRecord>(
                context,
                "audio_provider_create",
                &request.client_operation_id,
                &digest,
            )?;
            let credential = request
                .draft
                .api_key
                .map(SecretValue::new)
                .transpose()
                .map_err(|_| invalid_field("api_key", "the credential is invalid"))?;
            if let Some(result) = prior {
                let provider = context
                    .backend()
                    .database()
                    .get_audio_provider(parse_id(&result.id, "id")?)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| {
                        super::operations::applied_deleted("audio_provider_create", &result.id)
                    })?;
                fill_missing_provider_key(context, &provider, credential)?;
                return Ok(provider_view(provider));
            }
            let config = configuration(request.draft.configuration);
            crate::speech::tts_configuration::validate_secret_choice(&config, credential.is_some())
                .map_err(IntoApiError::into_api_error)?;
            let (id, owner, reference) = provider_create_ids(&request.client_operation_id);
            let provider = AudioProvider {
                id,
                secret_owner_id: owner,
                api_key_ref: credential.as_ref().map(|_| reference),
                label: request.label,
                config,
                revision: lettuce_types::Revision::INITIAL,
                created_at: context.now(),
                updated_at: context.now(),
            };
            provider
                .validate()
                .map_err(|error| invalid_field("draft", error.to_string()))?;
            let result = super::operations::commit(
                context,
                "audio_provider_create",
                &request.client_operation_id,
                &digest,
                |transaction| {
                    transaction
                        .create_audio_provider(&provider)
                        .map_err(IntoApiError::into_api_error)?;
                    Ok(super::operations::StoredRecord::new(
                        provider.id,
                        Some(provider.revision.get()),
                    ))
                },
            )?;
            fill_missing_provider_key(context, &provider, credential)?;
            let current = context
                .backend()
                .database()
                .get_audio_provider(parse_id(&result.id, "id")?)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    super::operations::applied_deleted("audio_provider_create", &result.id)
                })?;
            Ok(provider_view(current))
        })
        .await
}
