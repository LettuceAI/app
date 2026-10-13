//! The application API the desktop shell exposes as commands. Every call is
//! a plain async function over an `ApiContext` that takes and returns
//! `lettuce-contracts` DTOs and fails with an `ApiError`; repository work
//! runs on the blocking pool.

mod app;
mod library;
pub use app::content_filter::{content_filter_clear, content_filter_log};
pub use app::settings::{settings_get, settings_sampler_defaults_update, settings_update};
pub use library::delete::{character_delete, group_delete, persona_delete};
pub(crate) mod companion;
mod context;
pub(crate) mod conversations;
mod error;
pub(crate) mod events;
mod files;
mod image;
mod jobs;
mod lorebooks;
mod mapping;
pub use app::maintenance_jobs::storage_optimize;
mod memory;
pub(crate) mod models;
mod speech;
mod worker;

#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod tests;

pub use app::{
    app_status, app_ui_state_update, app_usage_days, purge_notice_dismiss, purge_notices_list,
};
pub use files::assets::{AssetBytes, AssetRange, AssetRead, OPEN_RANGE_CHUNK, read_asset};
pub use conversations::branches::{
    conversation_branch_delete, conversation_branch_fork, conversation_branch_rename,
    conversation_branch_select, conversation_branches,
};
pub use library::character_list::characters_list;
pub use companion::{
    companion_notes_active_preview, companion_notes_delete, companion_notes_list,
    companion_notes_upsert, companion_soul_get, companion_soul_growth_clear,
    companion_soul_growth_lock, companion_soul_growth_remove, companion_soul_writer_run,
};
pub use context::{ApiContext, ApiContextParts, ApiDatabaseFiles, ApiMediaStore};
pub use conversations::delete::conversation_delete;
pub use conversations::settings::{
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
    ExportProtection, FileAccess, FileAccessError, FileDescription, FileReader, PickFilter,
    assets_ingest, files_inspect, files_pick_open, files_pick_save,
};
pub use models::hugging_face::{
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
pub use app::inspect::{
    conversation_participation_stats, conversation_speaker_selection_preview,
    message_companion_effect, message_prompt_snapshot,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use jobs::CatalogVariant;
pub(crate) use jobs::MemoryJobOutput;
pub use jobs::{
    ArtifactInstallHandler, ClaimedJob, ImageGenerateHandler, ImageToolHandler, InstallFinish,
    InstallSources, InstallWork, JobHandler, JobHandlers, JobLane, JobProgressSink, JobRunner,
    MemoryExtractionHandler, ModelPullHandler, ModelsFolderMoveHandler, NetworkInstallSources,
    SoulWriterHandler, SpeechSynthesizeHandler, SpeechTranscribeHandler, TextFeatureHandler,
    VoiceCreationHandler, admit_install, conversation_help_me_reply, job_cancel, job_get,
    job_watch, jobs_list, voice_design_create,
};
pub use models::local::{
    llama_chat_template, llama_context_info, llama_devices, llama_unload, local_file_runnability,
    local_model_adopt, local_model_delete, local_models_dir_get, local_models_dir_set,
    local_models_list,
};
pub use lorebooks::{
    lorebook_archive, lorebook_create, lorebook_delete, lorebook_entries_mutate, lorebook_get,
    lorebook_restore, lorebook_trigger_preview, lorebook_update_metadata, lorebooks_list,
    tokens_count,
};
pub use memory::models::{
    companion_emotion_install, companion_emotion_remove, companion_emotion_status,
    embedding_choose, embedding_compare, embedding_install, embedding_remove, embedding_status,
    embedding_unload,
};
pub use conversations::messages::{
    conversation_message_count, conversation_messages_around, conversation_pinned_messages,
    conversation_search, memory_rewind_retry, message_candidate_select, message_candidates,
    message_delete, message_edit, message_pin, message_revisions, message_scene_select,
    messages_delete_after,
};
pub use models::{InstalledModels, ModelLoad, ModelLoader, NoModels};
pub use models::ollama::{ollama_model_delete, ollama_models_list, ollama_pull};
pub use library::prompts::{
    prompt_app_default_set, prompt_builtin_reset, prompt_create,
    prompt_delete, prompt_get, prompt_placeholders, prompt_preview, prompt_update, prompt_validate,
    prompts_list,
};
pub use conversations::scenes::{
    message_scene_image_approve, message_scene_image_dismiss, message_scene_image_generate,
    message_scene_prompt_generate,
};
pub use speech::{
    InstalledSpeech, NoSpeech, SpeechHost, asr_correction_delete, asr_correction_save,
    asr_corrections_list, asr_ignored_suggestions_list, asr_learning_export, asr_learning_import,
    asr_suggestion_approve, asr_suggestion_ignore, asr_suggestions, asr_vocabulary_delete,
    asr_vocabulary_list, asr_vocabulary_save, asr_voice_example_delete, asr_voice_example_save,
    asr_voice_example_suggest, asr_voice_examples_list, audio_provider_api_key_rotate,
    audio_provider_create, audio_provider_credential_status, audio_provider_delete,
    audio_provider_update, audio_provider_verify, audio_provider_voices,
    audio_provider_voices_refresh, audio_provider_voices_search, audio_providers_list,
    dictation_cancel, dictation_start, dictation_stop, kokoro_blend, kokoro_install_model,
    kokoro_install_voices, kokoro_inventory, kokoro_phonemize, kokoro_tokenize_preview,
    kokoro_uninstall_model, kokoro_uninstall_voice, kokoro_variants, kokoro_voices_available,
    kokoro_voices_installed, message_speak, transcribe_file, tts_cache_clear, tts_cache_stats,
    tts_models, tts_synthesize, tts_voice_design_models, user_voice_create, user_voice_delete,
    user_voice_update, user_voices_list, voice_design_preview, whisper_catalog,
    whisper_clear_cache, whisper_delete, whisper_dictation_model_set, whisper_download,
    whisper_models_list, whisper_preload,
};
pub use app::startup::{ApiWorkers, StartupStep, startup};
pub use conversations::turns::{
    conversation_add_user_message, conversation_continue, conversation_regenerate,
    conversation_retry,
};
pub use worker::ConversationGenerationWorker;

pub use conversations::copies::{
    conversation_branch_direct_to_character, conversation_branch_direct_to_group,
    conversation_branch_to_character, conversation_branch_to_character_from_message,
    conversation_duplicate,
};

pub use memory::{
    memory_add, memory_delete, memory_pin, memory_set_temperature, memory_summary_update,
    memory_update,
};

pub use memory::control::{
    memory_cycle_revert, memory_cycles, memory_error_dismiss, memory_retry, memory_skip,
    memory_trigger,
};
pub use memory::read::memory_get;


mod serial_events;

pub use jobs::LorebookHandler;
pub use lorebooks::jobs::{lorebook_entry_draft, lorebook_keywords_draft};
pub use lorebooks::projects::*;


mod providers;
pub use providers::{
    certificates_list, provider_accounts_list, provider_catalog, provider_openrouter_endpoints,
    provider_verify,
};


pub use providers::mutations::{
    certificates_import, certificates_remove, provider_account_delete, provider_account_save,
};
pub use providers::{provider_model_verify, provider_models};


pub use models::profiles::{
    model_default_set, model_delete, model_duplicate, model_get, model_save, model_usage,
    models_list,
};

pub use models::nanogpt::provider_nanogpt_usage;


pub use app::diagnostics::logs_diagnostics_report;
pub use app::logs::{
    log_append, log_delete, log_export, log_read_page, log_relevant_lines, log_search, logs_clear,
    logs_list,
};



pub use app::metrics::{llm_metrics_clear, llm_metrics_for_message, llm_metrics_get, llm_metrics_list};

mod usage;
pub use usage::billing::usage_recalculate_costs;
pub use usage::{usage_clear_before, usage_export_csv, usage_query, usage_stats};

pub use app::reset::{AppResetHost, app_data_reset};
pub use app::storage::{storage_database_file_delete, storage_database_files_list, storage_summary};

pub use library::media_library::{media_library_list, media_library_remove, media_save_to};



pub use library::bindings::{character_lorebooks_list, character_lorebooks_bind, character_lorebooks_unbind, character_lorebooks_enable, character_lorebooks_reorder, persona_lorebooks_list, persona_lorebooks_bind, persona_lorebooks_unbind, persona_lorebooks_enable, persona_lorebooks_reorder, group_lorebooks_list, group_lorebooks_bind, group_lorebooks_unbind, group_lorebooks_enable, group_lorebooks_reorder};



pub use library::duplicate::character_duplicate;


pub use library::personas::{persona_get, persona_save, personas_list};


pub use library::persona_default::{persona_default_set, persona_default_clear};



pub use library::characters::{character_archive, character_get, character_restore, character_save};


pub use library::groups::{group_get, group_save, groups_list};


pub use library::media::{character_media_set, persona_media_set};


pub use app::legacy_state::legacy_state;

#[cfg(test)]
mod context_slot_tests;

mod context_slot;
pub use context_slot::ApiContextSlot;
mod backup;
pub use backup::{backup_inspect, backups_list, backup_forget, backup_delete};

pub use backup::export_jobs::backup_export;
pub use backup::jobs::{backup_restore, legacy_backup_restore, legacy_database_import};


mod sync;
pub use sync::{sync_share_start, sync_share_approve, sync_share_begin, sync_connect, sync_stop, sync_status};
pub use sync::conflicts::{sync_conflicts_list, sync_conflict_resolve, sync_fork_resolve, legacy_conflict_dismiss};

mod entity_files;
pub use entity_files::{import_preview, import_commit, character_export, persona_export, lorebook_export, prompt_export, chat_template_export, model_export, chat_export};
