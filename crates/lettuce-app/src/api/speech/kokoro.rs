use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_model_hub::{KokoroInstallStore, KokoroVoiceInstallStore, KokoroModelVariant};
use crate::api::{ApiContext, InstallWork, admit_install};
use crate::api::error::{api_error, invalid_field};

fn failed(error: impl std::fmt::Display) -> ApiError { api_error(ApiErrorCode::Internal, error.to_string()) }
fn root(context: &ApiContext) -> Result<std::path::PathBuf, ApiError> {
    context.retained_model_roots()?.kokoro.map(std::path::PathBuf::from)
        .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "the Kokoro root is unavailable"))
}
fn variant(value: &str) -> Result<KokoroModelVariant, ApiError> {
    KokoroModelVariant::parse(value).map_err(|_| invalid_field("variant", "unknown Kokoro variant"))
}
fn artifact(value: crate::KokoroArtifactSummary) -> dto::KokoroArtifactView {
    dto::KokoroArtifactView { byte_size: value.byte_size, blake3: value.blake3.to_string() }
}
fn voice(value: crate::KokoroInstalledVoiceSummary) -> dto::KokoroInstalledVoiceView {
    dto::KokoroInstalledVoiceView { id: value.id, artifact: artifact(value.artifact) }
}
fn client(context: &ApiContext) -> Result<lettuce_network::JsonClient, ApiError> {
    let policy = context.backend().tls_policy().map_err(failed)?;
    lettuce_network::JsonClient::with_tls(&policy).map_err(failed)
}
fn guard_install(context: &ApiContext, root: &std::path::Path) -> Result<(), ApiError> {
    for (job, install_root) in context.jobs().install_roots() {
        if install_root == root { return Err(crate::api::local_models::busy(dto::LocalModelsBusyReason::InstallActive { job_id: job.to_string() })); }
    }
    if root.starts_with(crate::api::local_models::models_root(context)?) {
        crate::api::jobs::local::folder_move_active(context)?;
    }
    Ok(())
}

pub async fn kokoro_inventory(context: &ApiContext, request: dto::KokoroInventoryRequest) -> Result<dto::KokoroInventory, ApiError> {
    context.blocking(move |context| {
        variant(&request.variant)?;
        let mut inventory = crate::KokoroAssetInventoryCoordinator::open_managed(root(context)?).map_err(failed)?
            .inspect(&request.variant, request.selected_voice_id.as_deref()).map_err(failed)?;
        let store = KokoroInstallStore::open(root(context)?).map_err(failed)?;
        let pin = store.recorded_model(variant(&request.variant)?).map_err(failed)?;
        if let Some(model) = store.installed(&pin).map_err(failed)? {
            for file in model.artifacts {
                let summary = crate::KokoroArtifactSummary { byte_size: file.artifact.byte_size, blake3: file.artifact.blake3 };
                match file.role {
                    lettuce_model_hub::KokoroArtifactRole::Config => inventory.config = Some(summary),
                    lettuce_model_hub::KokoroArtifactRole::Tokenizer => inventory.tokenizer = Some(summary),
                    lettuce_model_hub::KokoroArtifactRole::TokenizerConfig => inventory.tokenizer_config = Some(summary),
                    lettuce_model_hub::KokoroArtifactRole::Model => inventory.model = Some(summary),
                }
            }
        }
        Ok(dto::KokoroInventory { variant: inventory.variant, variant_allowed_on_platform: inventory.variant_allowed_on_platform,
            model: inventory.model.map(artifact), config: inventory.config.map(artifact), tokenizer: inventory.tokenizer.map(artifact),
            tokenizer_config: inventory.tokenizer_config.map(artifact), installed_voices: inventory.installed_voices.into_iter().map(voice).collect(),
            selected_voice_installed: inventory.selected_voice_installed })
    }).await
}

pub async fn kokoro_variants(_context: &ApiContext) -> Result<Vec<dto::KokoroVariantView>, ApiError> {
    Ok(lettuce_model_hub::kokoro_supported_model_variants().into_iter().map(|variant| dto::KokoroVariantView {
        id: variant.id.into(), label: variant.label.into(), filename: variant.filename.into(), size_mb: variant.size_mb, mobile_supported: variant.mobile_supported,
    }).collect())
}

pub async fn kokoro_voices_installed(context: &ApiContext) -> Result<Vec<dto::KokoroInstalledVoiceView>, ApiError> {
    context.blocking(|context| crate::KokoroAssetInventoryCoordinator::open_managed(root(context)?).map_err(failed)?
        .installed_voices().map(|voices| voices.into_iter().map(voice).collect()).map_err(failed)).await
}

pub async fn kokoro_voices_available(context: &ApiContext) -> Result<Vec<dto::KokoroAvailableVoiceView>, ApiError> {
    context.blocking(|context| {
        let inventory = crate::KokoroAssetInventoryCoordinator::open_managed(root(context)?).map_err(failed)?;
        tokio::runtime::Handle::current().block_on(crate::KokoroRemoteVoiceCatalog::new(client(context)?).list(&inventory))
            .map(|voices| voices.into_iter().map(|voice| dto::KokoroAvailableVoiceView { id: voice.id, installed: voice.installed, source_revision: voice.source_revision, byte_size: voice.byte_size }).collect())
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))
    }).await
}

pub async fn kokoro_install_model(context: &ApiContext, request: dto::KokoroVariantRequest) -> Result<dto::JobAccepted, ApiError> {
    let (root, model) = context.blocking(move |context| {
        let root = root(context)?;
        let variant = variant(&request.variant)?;
        if !lettuce_model_hub::kokoro_platform_allows_variant(variant) { return Err(invalid_field("variant", "this Kokoro variant is not supported on this platform")); }
        if root.starts_with(crate::api::local_models::models_root(context)?) { crate::api::jobs::local::folder_move_active(context)?; }
        let model = tokio::runtime::Handle::current().block_on(lettuce_model_hub::pin_kokoro_model(&client(context)?, lettuce_model_hub::HUGGING_FACE_ENDPOINT, variant))
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        Ok((root, model))
    }).await?;
    admit_install(context, InstallWork::KokoroModel { model, install_root: root }).await
}

pub async fn kokoro_install_voices(context: &ApiContext, request: dto::KokoroVoicesInstallRequest) -> Result<dto::JobAccepted, ApiError> {
    if request.voice_ids.is_empty() { return Err(invalid_field("voice_ids", "select at least one voice")); }
    let (root, bundle) = context.blocking(move |context| {
        let root = root(context)?;
        let inventory = crate::KokoroAssetInventoryCoordinator::open_managed(&root).map_err(failed)?;
        let catalog = tokio::runtime::Handle::current().block_on(crate::KokoroRemoteVoiceCatalog::new(client(context)?).list(&inventory))
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        let bundle = crate::KokoroVoiceBundle::from_catalog_ids(&catalog, &request.voice_ids).map_err(|error| invalid_field("voice_ids", error.to_string()))?;
        Ok((root, bundle))
    }).await?;
    admit_install(context, InstallWork::KokoroVoices { bundle, install_root: root }).await
}

pub async fn kokoro_uninstall_model(context: &ApiContext, request: dto::KokoroVariantRequest) -> Result<bool, ApiError> {
    context.blocking(move |context| {
        let root = root(context)?; guard_install(context, &root)?;
        let store = KokoroInstallStore::open(&root).map_err(failed)?;
        let model = store.recorded_model(variant(&request.variant)?).map_err(failed)?;
        crate::remove_managed_kokoro_model(&store, &model).map(|removed| removed.removed).map_err(failed)
    }).await
}

pub async fn kokoro_uninstall_voice(context: &ApiContext, request: dto::KokoroVoiceRequest) -> Result<bool, ApiError> {
    context.blocking(move |context| {
        let root = root(context)?; guard_install(context, &root)?;
        let store = KokoroVoiceInstallStore::open(&root).map_err(failed)?;
        let Some(voice) = store.installed_descriptors().map_err(failed)?.into_iter().find(|voice| voice.id == request.voice_id) else { return Ok(false); };
        crate::remove_managed_kokoro_voice(&store, &voice).map(|removed| removed.removed).map_err(failed)
    }).await
}

pub async fn kokoro_blend(context: &ApiContext, request: dto::KokoroBlendRequest) -> Result<dto::KokoroBlendView, ApiError> {
    context.blocking(move |context| {
        let specs = request.voices.into_iter().map(|voice| lettuce_speech::KokoroVoiceBlendSpec { voice_id: voice.voice_id, weight: voice.weight }).collect::<Vec<_>>();
        lettuce_speech::normalize_kokoro_voice_blend(&specs).map_err(|error| invalid_field("voices", error.to_string()))?;
        let blended = crate::KokoroVoiceBlendCoordinator::new(KokoroVoiceInstallStore::open(root(context)?).map_err(failed)?)
            .blend_installed(&specs).map_err(|error| super::errors::speech_error(ApiErrorCode::Unavailable, dto::SpeechFailure::VoiceMissing, error.to_string()))?;
        Ok(dto::KokoroBlendView { voices: blended.normalized_specs().iter().map(|voice| dto::KokoroVoiceBlendInput { voice_id: voice.voice_id.clone(), weight: voice.weight }).collect(),
            style_rows: u32::try_from(blended.row_count()).map_err(failed)? })
    }).await
}

pub async fn kokoro_phonemize(context: &ApiContext, request: dto::KokoroPhonemizeRequest) -> Result<dto::KokoroPhonemizationView, ApiError> {
    if !["af_", "am_", "bf_", "bm_"].iter().any(|prefix| request.voice_id.starts_with(prefix)) { return Err(invalid_field("voice_id", "an English Kokoro voice is required")); }
    context.blocking(move |context| {
        let store = KokoroInstallStore::open(root(context)?).map_err(failed)?;
        let model = store.recorded_model(variant(&request.variant)?).map_err(failed)?;
        let lexicon = store.materialize_lexicon().map_err(failed)?.map(|material| lettuce_speech::parse_kokoro_lexicon(material.bytes(), lettuce_speech::kokoro_voice_language(&request.voice_id)))
            .transpose().map_err(|error| invalid_field("lexicon", error.to_string()))?.unwrap_or_default();
        let phonemes = crate::KokoroPhonemizationCoordinator::new(store).phonemize(&model, &lettuce_platform::EspeakNgProcess::from_path(),
            &lettuce_speech::KokoroPhonemizationInput { voice_id: request.voice_id, text: request.text, lexicon })
            .map_err(|error| match error {
                crate::KokoroPhonemizationCoordinatorError::Phonemization(lettuce_speech::KokoroPhonemizationError::Process(lettuce_platform::EspeakNgError::Unavailable)) =>
                    super::errors::speech_error(ApiErrorCode::Unavailable, dto::SpeechFailure::RuntimeMissing { runtime: dto::SpeechRuntimeKind::Espeak }, error.to_string()),
                crate::KokoroPhonemizationCoordinatorError::MissingAssets => super::errors::speech_error(ApiErrorCode::ModelRequired, dto::SpeechFailure::ModelRequired { model: dto::SpeechModelKind::Kokoro }, error.to_string()),
                error => invalid_field("text", error.to_string()),
            })?;
        let tokens = |tokens: Vec<i64>| tokens.into_iter().map(|token| i32::try_from(token).map_err(failed)).collect::<Result<Vec<_>, _>>();
        Ok(dto::KokoroPhonemizationView { normalized_text: phonemes.normalized_text, effective_text: phonemes.effective_text, language: phonemes.language,
            used_lexicon_entries: phonemes.used_lexicon_entries, token_ids: tokens(phonemes.token_ids)?,
            segments: phonemes.segments.into_iter().map(|segment| Ok(dto::KokoroPhonemizationSegmentView { kind: segment.kind, source_text: segment.source_text, ipa: segment.ipa, token_ids: tokens(segment.token_ids)? })).collect::<Result<Vec<_>, ApiError>>()? })
    }).await
}
