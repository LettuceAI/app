mod abort_manager;
mod api;
mod chat_appearance;
mod chat_manager;
mod content_filter;
mod creation_helper;
mod discovery;
mod embedding_model;
mod engine;
mod error;
mod group_chat_manager;
mod image_generator;
mod llama_cpp;
mod logger;
pub mod migrations;
pub mod models;
mod pricing_cache;
mod providers;
mod serde_utils;
pub mod storage_manager;
pub mod sync;
mod tokenizer;
mod transport;
mod tts_manager;
mod usage;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use std::sync::Arc;
    use std::time::Duration;
    use tauri::Manager;
    use tauri_plugin_aptabase::EventTracker;

    #[derive(Clone)]
    struct AnalyticsState {
        enabled: bool,
    }

    fn read_analytics_enabled(app: &tauri::AppHandle) -> bool {
        match crate::storage_manager::settings::internal_read_settings(app) {
            Ok(Some(settings_json)) => {
                let parsed: serde_json::Value = match serde_json::from_str(&settings_json) {
                    Ok(value) => value,
                    Err(err) => {
                        utils::log_error(
                            app,
                            "settings",
                            format!("Failed to parse settings JSON: {}", err),
                        );
                        return true;
                    }
                };
                parsed
                    .get("appState")
                    .and_then(|v| v.get("analyticsEnabled"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
            }
            Ok(None) => true,
            Err(err) => {
                utils::log_error(app, "settings", format!("Failed to read settings: {}", err));
                true
            }
        }
    }

    fn read_pure_mode_level(app: &tauri::AppHandle) -> content_filter::PureModeLevel {
        match crate::storage_manager::settings::internal_read_settings(app) {
            Ok(Some(settings_json)) => {
                let parsed: serde_json::Value = match serde_json::from_str(&settings_json) {
                    Ok(value) => value,
                    Err(_) => return content_filter::PureModeLevel::Standard,
                };
                let app_state = parsed.get("appState");
                // Try pureModeLevel first, fall back to pureModeEnabled boolean
                if let Some(level_str) = app_state
                    .and_then(|v| v.get("pureModeLevel"))
                    .and_then(|v| v.as_str())
                {
                    content_filter::PureModeLevel::from_str(level_str)
                } else if let Some(enabled) = app_state
                    .and_then(|v| v.get("pureModeEnabled"))
                    .and_then(|v| v.as_bool())
                {
                    if enabled {
                        content_filter::PureModeLevel::Standard
                    } else {
                        content_filter::PureModeLevel::Off
                    }
                } else {
                    content_filter::PureModeLevel::Standard
                }
            }
            Ok(None) => content_filter::PureModeLevel::Standard,
            Err(_) => content_filter::PureModeLevel::Standard,
        }
    }

    let aptabase_key = std::env::var("APTABASE_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| option_env!("APTABASE_KEY").map(|v| v.to_string()));
    let aptabase_plugin_enabled = aptabase_key.is_some();
    let aptabase_runtime = if aptabase_plugin_enabled {
        Some(tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for Aptabase"))
    } else {
        None
    };
    let _aptabase_runtime_guard = aptabase_runtime.as_ref().map(|rt| rt.enter());

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_tts::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    if let Some(key) = aptabase_key.as_deref() {
        builder = builder.plugin(tauri_plugin_aptabase::Builder::new(key).build());
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_haptics::init());

    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(tauri_plugin_android_fs::init())
        .plugin(tauri_plugin_barcode_scanner::init());

    builder
        .setup(move |app| {
            let abort_registry = abort_manager::AbortRegistry::new();
            app.manage(abort_registry);
            let app_usage_service = Arc::new(usage::app_activity::AppActiveUsageService::new());
            app.manage(app_usage_service.clone());

            let log_manager =
                logger::LogManager::new(app.handle()).expect("Failed to initialize log manager");
            app.manage(log_manager);
            logger::set_global_app_handle(app.handle().clone());
            if let Err(err) = utils::init_tracing(app.handle().clone()) {
                eprintln!("Failed to initialize tracing: {}", err);
            }
            std::panic::set_hook(Box::new(|info| {
                let message = format!("{}", info);
                utils::log_error_global("panic", message);
            }));

            configure_onnxruntime_dylib(app.handle());

            match storage_manager::db::init_pool(app.handle()) {
                Ok(pool) => {
                    let swappable = storage_manager::db::SwappablePool::new(pool);
                    app.manage(swappable);
                }
                Err(e) => panic!("Failed to initialize database pool: {}", e),
            }
            {
                let app_handle = app.handle().clone();
                let usage_service = app_usage_service.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(30));
                    loop {
                        interval.tick().await;
                        usage_service.flush(&app_handle);
                    }
                });
            }

            let analytics_enabled = aptabase_plugin_enabled && read_analytics_enabled(app.handle());
            app.manage(AnalyticsState {
                enabled: analytics_enabled,
            });
            if analytics_enabled {
                if let Err(e) = app.track_event("app_started", None) {
                    utils::log_error(
                        app.handle(),
                        "aptabase",
                        format!("track_event(app_started) failed: {}", e),
                    );
                }
            }

            let pure_mode_level = read_pure_mode_level(app.handle());
            app.manage(content_filter::ContentFilter::new(pure_mode_level));

            if let Err(e) = storage_manager::importer::run_legacy_import(app.handle()) {
                utils::log_error(
                    app.handle(),
                    "bootstrap",
                    format!("Legacy import error: {}", e),
                );
            }

            if let Err(e) = migrations::run_migrations(app.handle()) {
                utils::log_error(app.handle(), "bootstrap", format!("Migration error: {}", e));
            }

            if let Err(e) = chat_manager::prompts::ensure_app_default_template(app.handle()) {
                utils::log_error(
                    app.handle(),
                    "bootstrap",
                    format!("Failed to ensure app default template: {}", e),
                );
            }

            if let Err(e) = chat_manager::prompts::ensure_help_me_reply_template(app.handle()) {
                utils::log_error(
                    app.handle(),
                    "bootstrap",
                    format!("Failed to ensure help me reply template: {}", e),
                );
            }

            // Initialize Sync Manager
            app.manage(sync::manager::SyncManagerState::new());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::api_request,
            api::abort_request,
            sync::commands::start_driver,
            sync::commands::connect_as_passenger,
            sync::commands::stop_sync,
            sync::commands::get_sync_status,
            sync::commands::get_local_ip,
            sync::commands::approve_connection,
            sync::commands::start_sync_session,
            models::verify_model_exists,
            providers::verify_provider_api_key,
            providers::get_provider_configs,
            providers::commands::get_remote_models,
            providers::openrouter::get_openrouter_models,
            storage_manager::settings::storage_read_settings,
            storage_manager::settings::storage_write_settings,
            storage_manager::settings::settings_set_defaults,
            storage_manager::settings::analytics_is_available,
            storage_manager::db::storage_db_size,
            storage_manager::providers::provider_upsert,
            storage_manager::providers::provider_delete,
            storage_manager::models::model_upsert,
            storage_manager::models::model_delete,
            storage_manager::settings::settings_set_advanced,
            storage_manager::settings::settings_set_default_provider,
            storage_manager::settings::settings_set_default_model,
            storage_manager::settings::settings_set_app_state,
            storage_manager::settings::settings_set_prompt_template,
            storage_manager::settings::settings_set_system_prompt,
            storage_manager::settings::settings_set_migration_version,
            storage_manager::characters::characters_list,
            storage_manager::characters::character_upsert,
            storage_manager::characters::character_delete,
            storage_manager::lorebook::lorebooks_list,
            storage_manager::lorebook::lorebook_upsert,
            storage_manager::lorebook::lorebook_delete,
            storage_manager::lorebook::character_lorebooks_list,
            storage_manager::lorebook::character_lorebooks_set,
            storage_manager::lorebook::lorebook_entries_list,
            storage_manager::lorebook::lorebook_entry_get,
            storage_manager::lorebook::lorebook_entry_upsert,
            storage_manager::lorebook::lorebook_entry_delete,
            storage_manager::lorebook::lorebook_entry_create_blank,
            storage_manager::lorebook::lorebook_entries_reorder,
            storage_manager::lorebook::lorebook_export,
            storage_manager::lorebook::lorebook_import,
            storage_manager::entity_transfer::character_export,
            storage_manager::entity_transfer::character_export_with_format,
            storage_manager::entity_transfer::character_import,
            storage_manager::entity_transfer::character_import_preview,
            storage_manager::entity_transfer::character_list_formats,
            storage_manager::entity_transfer::character_detect_format,
            storage_manager::entity_transfer::convert_export_to_uec,
            storage_manager::entity_transfer::convert_export_to_format,
            storage_manager::entity_transfer::persona_export,
            storage_manager::entity_transfer::persona_import,
            storage_manager::entity_transfer::import_package,
            storage_manager::entity_transfer::save_json_to_downloads,
            storage_manager::personas::personas_list,
            storage_manager::personas::persona_upsert,
            storage_manager::personas::persona_delete,
            storage_manager::personas::persona_default_get,
            storage_manager::sessions::sessions_list_ids,
            storage_manager::sessions::sessions_list_previews,
            storage_manager::sessions::session_get,
            storage_manager::sessions::session_get_meta,
            storage_manager::sessions::session_message_count,
            storage_manager::sessions::messages_list,
            storage_manager::sessions::messages_list_pinned,
            storage_manager::sessions::session_upsert_meta,
            storage_manager::sessions::messages_upsert_batch,
            storage_manager::sessions::message_delete,
            storage_manager::sessions::messages_delete_after,
            storage_manager::sessions::session_upsert,
            storage_manager::sessions::session_delete,
            storage_manager::sessions::session_archive,
            storage_manager::sessions::session_update_title,
            storage_manager::sessions::message_toggle_pin,
            storage_manager::sessions::message_toggle_pin_state,
            storage_manager::sessions::session_add_memory,
            storage_manager::sessions::session_remove_memory,
            storage_manager::sessions::session_update_memory,
            storage_manager::sessions::session_toggle_memory_pin,
            storage_manager::sessions::session_set_memory_cold_state,
            storage_manager::usage::storage_clear_all,
            storage_manager::usage::storage_reset_database,
            storage_manager::usage::storage_usage_summary,
            storage_manager::media::storage_write_image,
            storage_manager::media::storage_get_image_path,
            storage_manager::media::storage_read_image,
            storage_manager::media::storage_delete_image,
            storage_manager::media::storage_save_avatar,
            storage_manager::media::storage_load_avatar,
            storage_manager::media::storage_delete_avatar,
            storage_manager::media::generate_avatar_gradient,
            storage_manager::media::storage_save_session_attachment,
            storage_manager::media::storage_load_session_attachment,
            storage_manager::media::storage_get_session_attachment_path,
            storage_manager::media::storage_delete_session_attachments,
            storage_manager::media::storage_session_attachment_exists,
            storage_manager::db::db_optimize,
            storage_manager::db::db_checkpoint,
            storage_manager::backup::backup_export,
            storage_manager::backup::backup_import,
            storage_manager::backup::backup_check_encrypted,
            storage_manager::backup::backup_verify_password,
            storage_manager::backup::backup_get_info,
            storage_manager::backup::backup_list,
            storage_manager::backup::backup_delete,
            storage_manager::backup::backup_get_info_from_bytes,
            storage_manager::backup::backup_check_encrypted_from_bytes,
            storage_manager::backup::backup_verify_password_from_bytes,
            storage_manager::backup::backup_import_from_bytes,
            storage_manager::backup::backup_check_dynamic_memory,
            storage_manager::backup::backup_check_dynamic_memory_from_bytes,
            storage_manager::backup::backup_disable_dynamic_memory,
            storage_manager::importer::legacy_backup_and_remove,
            storage_manager::legacy::get_storage_root,
            chat_manager::chat_completion,
            chat_manager::chat_regenerate,
            chat_manager::chat_continue,
            chat_manager::chat_add_message_attachment,
            chat_manager::get_default_character_rules,
            chat_manager::get_default_system_prompt_template,
            chat_manager::search_messages,
            chat_manager::chat_generate_user_reply,
            chat_manager::retry_dynamic_memory,
            chat_manager::trigger_dynamic_memory,
            chat_manager::list_prompt_templates,
            chat_manager::create_prompt_template,
            chat_manager::update_prompt_template,
            chat_manager::delete_prompt_template,
            chat_manager::get_prompt_template,
            chat_manager::get_app_default_template_id,
            chat_manager::is_app_default_template,
            chat_manager::reset_app_default_template,
            chat_manager::reset_dynamic_summary_template,
            chat_manager::reset_dynamic_memory_template,
            chat_manager::reset_help_me_reply_template,
            chat_manager::reset_help_me_reply_conversational_template,
            chat_manager::get_required_template_variables,
            chat_manager::validate_template_variables,
            chat_manager::render_prompt_preview,
            usage::usage_add_record,
            usage::usage_query_records,
            usage::usage_get_stats,
            usage::usage_clear_before,
            usage::usage_export_csv,
            usage::usage_save_csv,
            usage::usage_get_app_active_usage,
            usage::usage_recalculate_costs,
            utils::accessibility_sound_base64,
            utils::get_app_version,
            embedding_model::check_embedding_model,
            embedding_model::get_embedding_model_info,
            embedding_model::start_embedding_download,
            embedding_model::get_embedding_download_progress,
            embedding_model::cancel_embedding_download,
            embedding_model::compute_embedding,
            embedding_model::initialize_embedding_model,
            embedding_model::clear_embedding_runtime_cache,
            embedding_model::run_embedding_test,
            embedding_model::run_embedding_dev_benchmark,
            embedding_model::compare_custom_texts,
            embedding_model::delete_embedding_model,
            embedding_model::delete_embedding_model_version,
            image_generator::commands::generate_image,
            logger::log_to_file,
            logger::list_log_files,
            logger::read_log_file,
            logger::delete_log_file,
            logger::clear_all_logs,
            logger::get_log_dir_path,
            logger::save_log_to_downloads,
            tts_manager::commands::audio_provider_list,
            tts_manager::commands::audio_provider_upsert,
            tts_manager::commands::audio_provider_delete,
            tts_manager::commands::audio_models_list,
            tts_manager::commands::audio_voice_design_models_list,
            tts_manager::commands::audio_provider_voices,
            tts_manager::commands::audio_provider_refresh_voices,
            tts_manager::commands::user_voice_list,
            tts_manager::commands::user_voice_upsert,
            tts_manager::commands::user_voice_delete,
            tts_manager::commands::tts_preview,
            tts_manager::commands::audio_provider_verify,
            tts_manager::commands::audio_provider_search_voices,
            tts_manager::commands::voice_design_preview,
            tts_manager::commands::voice_design_create,
            tts_manager::audio_cache::tts_cache_key,
            tts_manager::audio_cache::tts_cache_exists,
            tts_manager::audio_cache::tts_cache_get,
            tts_manager::audio_cache::tts_cache_save,
            tts_manager::audio_cache::tts_cache_delete,
            tts_manager::audio_cache::tts_cache_clear,
            tts_manager::audio_cache::tts_cache_stats,
            creation_helper::creation_helper_start,
            creation_helper::creation_helper_get_session,
            creation_helper::creation_helper_get_latest_session,
            creation_helper::creation_helper_list_sessions,
            creation_helper::creation_helper_send_message,
            creation_helper::creation_helper_get_draft,
            creation_helper::creation_helper_cancel,
            creation_helper::creation_helper_complete,
            creation_helper::creation_helper_get_images,
            creation_helper::creation_helper_get_uploaded_image,
            creation_helper::creation_helper_regenerate,
            discovery::get_card_image,
            discovery::discovery_fetch_card_detail,
            discovery::discovery_fetch_cards,
            discovery::discovery_fetch_sections,
            discovery::discovery_search_cards,
            discovery::discovery_fetch_alternate_greetings,
            discovery::discovery_fetch_tags,
            discovery::discovery_fetch_author_info,
            discovery::discovery_import_character,
            llama_cpp::llamacpp_context_info,
            llama_cpp::llamacpp_unload,
            content_filter::set_content_filter_level,
            content_filter::debug_content_filter,
            content_filter::get_filter_log,
            content_filter::clear_filter_log,
            // Group chat commands
            storage_manager::group_sessions::group_sessions_list,
            storage_manager::group_sessions::group_sessions_list_all,
            storage_manager::group_sessions::group_session_create,
            storage_manager::group_sessions::group_session_get,
            storage_manager::group_sessions::group_session_update,
            storage_manager::group_sessions::group_session_delete,
            storage_manager::group_sessions::group_session_archive,
            storage_manager::group_sessions::group_session_update_title,
            storage_manager::group_sessions::group_session_duplicate,
            storage_manager::group_sessions::group_session_duplicate_with_messages,
            storage_manager::group_sessions::group_session_branch_to_character,
            storage_manager::group_sessions::group_session_add_character,
            storage_manager::group_sessions::group_session_remove_character,
            storage_manager::group_sessions::group_session_update_starting_scene,
            storage_manager::group_sessions::group_session_update_background_image,
            storage_manager::group_sessions::group_session_update_chat_type,
            storage_manager::group_sessions::group_session_update_speaker_selection_method,
            storage_manager::group_sessions::group_participation_stats,
            storage_manager::group_sessions::group_participation_increment,
            storage_manager::group_sessions::group_messages_list,
            storage_manager::group_sessions::group_message_upsert,
            storage_manager::group_sessions::group_message_delete,
            storage_manager::group_sessions::group_messages_delete_after,
            storage_manager::group_sessions::group_message_add_variant,
            storage_manager::group_sessions::group_message_select_variant,
            storage_manager::group_sessions::group_message_count,
            storage_manager::group_sessions::group_session_update_memories,
            storage_manager::group_sessions::group_session_update_manual_memories,
            storage_manager::group_sessions::group_session_add_memory,
            storage_manager::group_sessions::group_session_remove_memory,
            storage_manager::group_sessions::group_session_update_memory,
            storage_manager::group_sessions::group_session_toggle_memory_pin,
            storage_manager::group_sessions::group_session_set_memory_cold_state,
            group_chat_manager::group_chat_send,
            group_chat_manager::group_chat_regenerate,
            group_chat_manager::group_chat_continue,
            group_chat_manager::group_chat_get_selection_prompt,
            group_chat_manager::group_chat_generate_user_reply,
            group_chat_manager::group_chat_retry_dynamic_memory,
            // Engine commands
            engine::commands::engine_health,
            engine::commands::engine_setup_status,
            engine::commands::engine_setup_complete,
            engine::commands::engine_config_llm,
            engine::commands::engine_config_llm_default,
            engine::commands::engine_config_engine,
            engine::commands::engine_config_background,
            engine::commands::engine_config_memory,
            engine::commands::engine_config_safety,
            engine::commands::engine_config_research,
            engine::commands::engine_config_llm_delete,
            engine::commands::engine_status,
            engine::commands::engine_usage,
            engine::commands::engine_get_config,
            engine::commands::engine_characters_list,
            engine::commands::engine_character_load,
            engine::commands::engine_character_unload,
            engine::commands::engine_character_activity,
            engine::commands::engine_character_template,
            engine::commands::engine_character_boost,
            engine::commands::engine_character_create,
            engine::commands::engine_character_full,
            engine::commands::engine_character_update,
            engine::commands::engine_character_delete_cmd,
            engine::commands::engine_chat,
            engine::commands::engine_chat_history,
            chat_appearance::compute_chat_theme,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(move |handler, event| match event {
            tauri::RunEvent::Resumed => {
                if let Some(state) = handler
                    .try_state::<std::sync::Arc<usage::app_activity::AppActiveUsageService>>()
                {
                    state.on_resumed();
                }
            }
            tauri::RunEvent::WindowEvent { event, .. } => {
                if let tauri::WindowEvent::Focused(focused) = event {
                    if let Some(state) = handler
                        .try_state::<std::sync::Arc<usage::app_activity::AppActiveUsageService>>()
                    {
                        state.on_window_focus_changed(focused);
                    }
                }
            }
            tauri::RunEvent::ExitRequested { .. } => {
                if let Some(state) = handler
                    .try_state::<std::sync::Arc<usage::app_activity::AppActiveUsageService>>()
                {
                    state.flush(&handler);
                }
            }
            tauri::RunEvent::Exit { .. } => {
                if let Some(state) = handler
                    .try_state::<std::sync::Arc<usage::app_activity::AppActiveUsageService>>()
                {
                    state.flush(&handler);
                }
                let analytics_enabled = handler.state::<AnalyticsState>().enabled;
                if analytics_enabled {
                    if let Err(e) = handler.track_event("app_exited", None) {
                        utils::log_error(
                            &handler,
                            "aptabase",
                            format!("track_event(app_exited) failed: {}", e),
                        );
                    }
                    handler.flush_events_blocking();
                }
            }
            _ => {}
        });
}

fn configure_onnxruntime_dylib(app: &tauri::AppHandle) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use tauri::path::BaseDirectory;
        use tauri::Manager;

        if let Ok(value) = std::env::var("ORT_DYLIB_PATH") {
            if !value.trim().is_empty() {
                let _ = ort::util::preload_dylib(&value);
                utils::log_info(
                    app,
                    "embedding_debug",
                    format!("ORT_DYLIB_PATH already set to {}", value),
                );
                return;
            }
        }

        if let Some(value) = option_env!("ORT_DYLIB_PATH") {
            if !value.trim().is_empty() {
                std::env::set_var("ORT_DYLIB_PATH", value);
                let _ = ort::util::preload_dylib(value);
                utils::log_info(
                    app,
                    "embedding_debug",
                    format!("Set ORT_DYLIB_PATH from compile-time env: {}", value),
                );
                return;
            }
        }

        let lib_name = if cfg!(target_os = "windows") {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };

        match app
            .path()
            .resolve(format!("onnxruntime/{}", lib_name), BaseDirectory::Resource)
        {
            Ok(path) => {
                if path.exists() {
                    std::env::set_var("ORT_DYLIB_PATH", &path);
                    let _ = ort::util::preload_dylib(&path);
                    utils::log_info(
                        app,
                        "embedding_debug",
                        format!("Set ORT_DYLIB_PATH={}", path.display()),
                    );
                } else {
                    utils::log_warn(
                        app,
                        "embedding_debug",
                        format!("ONNX Runtime library not found at {}", path.display()),
                    );
                }
            }
            Err(err) => {
                utils::log_warn(
                    app,
                    "embedding_debug",
                    format!("Failed to resolve ONNX Runtime resource path: {}", err),
                );
            }
        }
    }
}
