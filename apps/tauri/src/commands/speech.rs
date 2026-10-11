use lettuce_app::api::ApiContextSlot;
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn whisper_catalog(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::WhisperCatalog, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_catalog(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_models_list(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::WhisperInstalledModels, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_models_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_download(
    context: State<'_, ApiContextSlot>,
    request: dto::WhisperModelRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_download(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::WhisperModelRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_preload(
    context: State<'_, ApiContextSlot>,
    request: dto::WhisperPreloadRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_preload(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_clear_cache(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::WhisperCacheCleared, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_clear_cache(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn whisper_dictation_model_set(
    context: State<'_, ApiContextSlot>,
    request: dto::DictationModelSetRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::whisper_dictation_model_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn transcribe_file(
    context: State<'_, ApiContextSlot>,
    request: dto::TranscribeFileRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::transcribe_file(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn dictation_start(
    context: State<'_, ApiContextSlot>,
    request: dto::DictationStartRequest,
) -> Result<dto::DictationStarted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::dictation_start(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn dictation_stop(
    context: State<'_, ApiContextSlot>,
    request: dto::DictationStopRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::dictation_stop(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn dictation_cancel(
    context: State<'_, ApiContextSlot>,
    request: dto::DictationCancelRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::dictation_cancel(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_verify(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderVerifyRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_verify(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_voices(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_voices(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_voices_refresh(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_voices_refresh(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_models(
    context: State<'_, ApiContextSlot>,
    request: dto::TtsModelsRequest,
) -> Result<Vec<dto::TtsModelView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::tts_models(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_voice_design_models(
    context: State<'_, ApiContextSlot>,
    request: dto::TtsModelsRequest,
) -> Result<Vec<dto::TtsModelView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::tts_voice_design_models(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_cache_stats(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::TtsCacheStats, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::tts_cache_stats(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_cache_clear(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::tts_cache_clear(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_synthesize(
    context: State<'_, ApiContextSlot>,
    request: dto::TtsSynthesizeRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::tts_synthesize(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_inventory(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroInventoryRequest,
) -> Result<dto::KokoroInventory, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_inventory(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_variants(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::KokoroVariantView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_variants(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_voices_installed(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::KokoroInstalledVoiceView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_voices_installed(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_voices_available(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::KokoroAvailableVoiceView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_voices_available(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_install_model(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroVariantRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_install_model(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_install_voices(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroVoicesInstallRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_install_voices(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_uninstall_model(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroVariantRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_uninstall_model(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_uninstall_voice(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroVoiceRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_uninstall_voice(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_blend(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroBlendRequest,
) -> Result<dto::KokoroBlendView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_blend(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_phonemize(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroPhonemizeRequest,
) -> Result<dto::KokoroPhonemizationView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_phonemize(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_providers_list(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::AudioProviderView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_providers_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_update(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderUpdateRequest,
) -> Result<dto::AudioProviderView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn user_voices_list(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::UserVoiceView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::user_voices_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn user_voice_update(
    context: State<'_, ApiContextSlot>,
    request: dto::UserVoiceUpdateRequest,
) -> Result<dto::UserVoiceView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::user_voice_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn user_voice_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::UserVoiceRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::user_voice_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_vocabulary_list(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningFilter,
) -> Result<Vec<dto::AsrVocabularyView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_vocabulary_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_corrections_list(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningFilter,
) -> Result<Vec<dto::AsrCorrectionView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_corrections_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_ignored_suggestions_list(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningFilter,
) -> Result<Vec<dto::AsrIgnoredSuggestionView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_ignored_suggestions_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_voice_examples_list(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningFilter,
) -> Result<Vec<dto::AsrVoiceExampleView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_voice_examples_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_vocabulary_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningItemRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_vocabulary_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_correction_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningItemRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_correction_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_voice_example_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningItemRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_voice_example_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_suggestions(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrSuggestionsRequest,
) -> Result<Vec<dto::AsrSuggestionView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_suggestions(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_learning_export(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningExportRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_learning_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_voices_search(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderVoiceSearchRequest,
) -> Result<Vec<dto::AudioVoiceView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_voices_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn voice_design_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::VoiceDesignPreviewRequest,
) -> Result<Vec<dto::VoiceDesignPreviewView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::voice_design_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_speak(
    context: State<'_, ApiContextSlot>,
    request: dto::MessageSpeakRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::message_speak(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_voice_example_suggest(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningItemRequest,
) -> Result<Option<dto::AsrSuggestionView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_voice_example_suggest(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn kokoro_tokenize_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::KokoroTokenizePreviewRequest,
) -> Result<dto::KokoroPhonemizationView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::kokoro_tokenize_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_credential_status(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderRequest,
) -> Result<dto::AudioProviderCredentialStatus, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_credential_status(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_api_key_rotate(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderApiKeyRotateRequest,
) -> Result<dto::AudioProviderCredentialStatus, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_api_key_rotate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn user_voice_create(
    context: State<'_, ApiContextSlot>,
    request: dto::UserVoiceCreateRequest,
) -> Result<dto::UserVoiceView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::user_voice_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_vocabulary_save(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrVocabularySaveRequest,
) -> Result<dto::AsrVocabularyView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_vocabulary_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_correction_save(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrCorrectionSaveRequest,
) -> Result<dto::AsrCorrectionView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_correction_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_suggestion_approve(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrSuggestionWriteRequest,
) -> Result<dto::AsrCorrectionView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_suggestion_approve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_suggestion_ignore(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrSuggestionWriteRequest,
) -> Result<dto::AsrIgnoredSuggestionView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_suggestion_ignore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_voice_example_save(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrVoiceExampleSaveRequest,
) -> Result<dto::AsrVoiceExampleView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_voice_example_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn asr_learning_import(
    context: State<'_, ApiContextSlot>,
    request: dto::AsrLearningImportRequest,
) -> Result<dto::AsrLearningImportView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::asr_learning_import(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn audio_provider_create(
    context: State<'_, ApiContextSlot>,
    request: dto::AudioProviderCreateRequest,
) -> Result<dto::AudioProviderView, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::audio_provider_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn voice_design_create(
    context: State<'_, ApiContextSlot>,
    request: dto::VoiceDesignCreateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::voice_design_create(&context, request).await
}
