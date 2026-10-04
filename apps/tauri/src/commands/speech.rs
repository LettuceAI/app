use lettuce_app::api::ApiContext;
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn whisper_catalog(
    context: State<'_, ApiContext>,
) -> Result<dto::WhisperCatalog, ApiError> {
    lettuce_app::api::whisper_catalog(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_models_list(
    context: State<'_, ApiContext>,
) -> Result<dto::WhisperInstalledModels, ApiError> {
    lettuce_app::api::whisper_models_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_download(
    context: State<'_, ApiContext>,
    request: dto::WhisperModelRequest,
) -> Result<dto::JobAccepted, ApiError> {
    lettuce_app::api::whisper_download(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_delete(
    context: State<'_, ApiContext>,
    request: dto::WhisperModelRequest,
) -> Result<(), ApiError> {
    lettuce_app::api::whisper_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_preload(
    context: State<'_, ApiContext>,
    request: dto::WhisperPreloadRequest,
) -> Result<(), ApiError> {
    lettuce_app::api::whisper_preload(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_clear_cache(
    context: State<'_, ApiContext>,
) -> Result<dto::WhisperCacheCleared, ApiError> {
    lettuce_app::api::whisper_clear_cache(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_dictation_model_set(
    context: State<'_, ApiContext>,
    request: dto::DictationModelSetRequest,
) -> Result<(), ApiError> {
    lettuce_app::api::whisper_dictation_model_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn transcribe_file(
    context: State<'_, ApiContext>,
    request: dto::TranscribeFileRequest,
) -> Result<dto::JobAccepted, ApiError> {
    lettuce_app::api::transcribe_file(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn dictation_start(
    context: State<'_, ApiContext>,
    request: dto::DictationStartRequest,
) -> Result<dto::DictationStarted, ApiError> {
    lettuce_app::api::dictation_start(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn dictation_stop(
    context: State<'_, ApiContext>,
    request: dto::DictationStopRequest,
) -> Result<dto::JobAccepted, ApiError> {
    lettuce_app::api::dictation_stop(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn dictation_cancel(
    context: State<'_, ApiContext>,
    request: dto::DictationCancelRequest,
) -> Result<(), ApiError> {
    lettuce_app::api::dictation_cancel(&context, request).await
}


#[tauri::command]
#[specta::specta]
pub async fn audio_provider_verify(
    context: State<'_, ApiContext>,
    request: dto::AudioProviderVerifyRequest,
) -> Result<bool, ApiError> {
    lettuce_app::api::audio_provider_verify(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_voices(
    context: State<'_, ApiContext>,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    lettuce_app::api::audio_provider_voices(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_voices_refresh(
    context: State<'_, ApiContext>,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    lettuce_app::api::audio_provider_voices_refresh(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_models(
    context: State<'_, ApiContext>,
    request: dto::TtsModelsRequest,
) -> Result<Vec<dto::TtsModelView>, ApiError> {
    lettuce_app::api::tts_models(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_voice_design_models(
    context: State<'_, ApiContext>,
    request: dto::TtsModelsRequest,
) -> Result<Vec<dto::TtsModelView>, ApiError> {
    lettuce_app::api::tts_voice_design_models(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_cache_stats(
    context: State<'_, ApiContext>,
) -> Result<dto::TtsCacheStats, ApiError> {
    lettuce_app::api::tts_cache_stats(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_cache_clear(
    context: State<'_, ApiContext>,
) -> Result<(), ApiError> {
    lettuce_app::api::tts_cache_clear(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_synthesize(context: State<'_, ApiContext>, request: dto::TtsSynthesizeRequest) -> Result<dto::JobAccepted, ApiError> {
    lettuce_app::api::tts_synthesize(&context, request).await
}
