//! The application API the desktop shell exposes as commands. Every call is
//! a plain async function over an `ApiContext` that takes and returns
//! `lettuce-contracts` DTOs and fails with an `ApiError`; repository work
//! runs on the blocking pool.

mod app;
mod assets;
mod characters;
mod context;
mod conversation_delete;
mod conversation_feed;
mod conversation_settings;
mod conversations;
mod error;
mod events;
mod file_kind;
mod files;
mod hugging_face;
mod image;
mod inspect;
mod jobs;
mod local_models;
mod mapping;
mod messages;
mod models;
mod memory_models;
mod embedding_health;
mod ollama;
mod scenes;
mod speech;
mod startup;
mod turns;
mod worker;

#[cfg(test)]
pub(crate) mod conversation_delete_tests;
#[cfg(test)]
mod conversation_list_tests;
#[cfg(test)]
mod conversation_settings_tests;
#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod help_me_reply_tests;
#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod inspect_tests;
#[cfg(test)]
mod legacy_regenerate_tests;
#[cfg(test)]
mod local_models_tests;
#[cfg(test)]
mod messages_tests;
#[cfg(test)]
mod models_tests;
#[cfg(test)]
mod scenes_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod turns_tests;

pub use app::{app_status, app_ui_state_update, purge_notice_dismiss, purge_notices_list};
pub use assets::{AssetBytes, AssetRange, AssetRead, OPEN_RANGE_CHUNK, read_asset};
pub use characters::characters_list;
pub use context::{ApiContext, ApiContextParts, ApiDatabaseFiles, ApiMediaStore};
pub use conversation_delete::conversation_delete;
pub use conversation_settings::{
    conversation_archive, conversation_participant_add, conversation_participant_update,
    conversation_rename, conversation_restore, conversation_settings_get,
    conversation_settings_update,
};
pub use conversations::{
    conversation_launch_direct, conversation_launch_group, conversation_messages,
    conversation_open, conversation_send, conversations_latest_by_character,
    conversations_latest_by_group, conversations_list, generation_cancel,
};
pub use events::{ApiEventSink, GenerationEventSink, JobEventSink};
pub use files::{
    FileAccess, FileAccessError, FileDescription, FileReader, assets_ingest, files_inspect,
};
pub use hugging_face::{
    hf_auth_clear, hf_auth_save, hf_auth_status, hf_author, hf_avatars, hf_download,
    hf_model_files, hf_readme, hf_recommendation, hf_runnability, hf_search,
};
pub use image::{
    avatar_gradient, avatar_prompt, civitai_auth_clear, civitai_auth_save, civitai_auth_status,
    civitai_lora_download, civitai_model, civitai_search, hf_image_bundle_files,
    hf_image_bundle_install, hf_image_bundle_profiles, hf_image_bundle_retry,
    hf_image_bundle_retry_registration, hf_image_bundle_search, image_capabilities,
    image_design_reference, image_generate, image_models_downloaded, image_upscale,
    lora_keywords_discover, loras_delete, loras_import, loras_list, loras_update_keywords,
    playground_history_delete, playground_history_list, sd_bundle_runnability, sd_catalog,
    sd_component_library, sd_compute_policy_get, sd_compute_policy_save, sd_detect_model_file,
    sd_disk_usage, sd_model_install, sd_model_repair, sd_model_uninstall, sd_models_installed,
    sd_runnability, sd_runtime_delete, sd_runtime_install, sd_runtime_inventory,
    sd_runtime_releases, sd_runtime_switch, sd_upscalers_install, sd_upscalers_list,
    sd_upscalers_remove,
};
pub use inspect::{
    conversation_participation_stats, conversation_speaker_selection_preview,
    message_companion_effect, message_prompt_snapshot,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use jobs::CatalogVariant;
pub use jobs::{
    ArtifactInstallHandler, ClaimedJob, ImageGenerateHandler, ImageToolHandler, InstallFinish,
    InstallSources, InstallWork, JobHandler, JobHandlers, JobLane, JobProgressSink, JobRunner,
    ModelPullHandler, ModelsFolderMoveHandler, NetworkInstallSources, TextFeatureHandler,
    SpeechTranscribeHandler, SpeechSynthesizeHandler,
    admit_install, conversation_help_me_reply, job_cancel, job_get, job_watch, jobs_list,
};
pub use local_models::{
    llama_chat_template, llama_context_info, llama_devices, llama_unload, local_file_runnability,
    local_model_adopt, local_model_delete, local_models_dir_get, local_models_dir_set,
    local_models_list,
};
pub use messages::{
    conversation_message_count, conversation_messages_around, conversation_pinned_messages,
    conversation_search, memory_rewind_retry, message_candidate_select, message_candidates,
    message_delete, message_edit, message_pin, message_revisions, message_scene_select,
    messages_delete_after,
};
pub use memory_models::{embedding_install, companion_emotion_install, embedding_status, embedding_choose, embedding_remove, embedding_compare, embedding_unload, companion_emotion_status, companion_emotion_remove};
pub use models::{InstalledModels, ModelLoad, ModelLoader, NoModels};
pub use ollama::{ollama_model_delete, ollama_models_list, ollama_pull};
pub use scenes::{
    message_scene_image_approve, message_scene_image_dismiss, message_scene_image_generate,
    message_scene_prompt_generate,
};
pub use speech::{
    message_speak,
    asr_voice_example_suggest, asr_learning_export, asr_vocabulary_list, asr_corrections_list, asr_ignored_suggestions_list, asr_voice_examples_list, asr_vocabulary_delete, asr_correction_delete, asr_voice_example_delete, asr_suggestions,
    kokoro_inventory, kokoro_variants, kokoro_voices_installed, kokoro_voices_available, kokoro_install_model, kokoro_install_voices, kokoro_uninstall_model, kokoro_uninstall_voice, kokoro_blend, kokoro_phonemize,
    tts_synthesize,
    voice_design_preview, audio_provider_voices_search, audio_providers_list, audio_provider_update, audio_provider_delete, user_voices_list, user_voice_update, user_voice_delete, audio_provider_verify, audio_provider_voices, audio_provider_voices_refresh, tts_models, tts_voice_design_models, tts_cache_stats, tts_cache_clear,
    InstalledSpeech, NoSpeech, SpeechHost, dictation_cancel, dictation_start, dictation_stop,
    transcribe_file, whisper_catalog, whisper_clear_cache, whisper_delete,
    whisper_dictation_model_set, whisper_download, whisper_models_list, whisper_preload,
};
pub use startup::{ApiWorkers, StartupStep, startup};
pub use turns::{
    conversation_add_user_message, conversation_continue, conversation_regenerate,
    conversation_retry,
};
pub use worker::ConversationGenerationWorker;
