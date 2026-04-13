use std::sync::Arc;
use std::time::Duration;

use tauri::Manager;

use crate::{
    abort_manager, chat_manager, content_filter, dynamic_memory_run_manager, host_api, logger,
    migrations, storage_manager, sync, usage, utils,
};

use super::runtime::configure_onnxruntime_dylib;

#[derive(Clone)]
pub(crate) struct AnalyticsState {
    pub(crate) enabled: bool,
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
            content_filter::level_from_app_state(parsed.get("appState"))
        }
        Ok(None) => content_filter::PureModeLevel::Standard,
        Err(_) => content_filter::PureModeLevel::Standard,
    }
}

fn manage_core_state(app: &mut tauri::App) -> Arc<usage::app_activity::AppActiveUsageService> {
    let abort_registry = abort_manager::AbortRegistry::new();
    app.manage(abort_registry);

    let dynamic_memory_run_manager = dynamic_memory_run_manager::DynamicMemoryRunManager::new();
    app.manage(dynamic_memory_run_manager);

    let app_usage_service = Arc::new(usage::app_activity::AppActiveUsageService::new());
    app.manage(app_usage_service.clone());

    app.manage(sync::manager::SyncManagerState::new());
    app.manage(host_api::HostApiManager::default());

    app_usage_service
}

#[cfg(target_os = "android")]
fn initialize_android_state(app: &mut tauri::App) {
    use crate::android_monitor;

    let monitor_state = android_monitor::initialize(app.handle())
        .expect("Failed to initialize Android crash monitor state");
    app.manage(monitor_state);
    android_monitor::start_heartbeat_loop(app.handle().clone());
}

#[cfg(not(target_os = "android"))]
fn initialize_android_state(_app: &mut tauri::App) {}

fn initialize_logging(app: &mut tauri::App) {
    let log_manager =
        logger::LogManager::new(app.handle()).expect("Failed to initialize log manager");
    app.manage(log_manager);
    logger::set_global_app_handle(app.handle().clone());
    if let Err(err) = utils::init_tracing(app.handle().clone()) {
        eprintln!("Failed to initialize tracing: {}", err);
    }
    std::panic::set_hook(Box::new(|info| {
        let message = format!("{}", info);
        eprintln!("panic: {}", message);
        eprintln!("{}", std::backtrace::Backtrace::force_capture());
        utils::log_error_global("panic", message);
    }));
}

fn initialize_database(app: &mut tauri::App) {
    configure_onnxruntime_dylib(app.handle());

    match storage_manager::db::init_pool(app.handle()) {
        Ok(pool) => {
            let swappable = storage_manager::db::SwappablePool::new(pool);
            app.manage(swappable);
        }
        Err(err) => panic!("Failed to initialize database pool: {}", err),
    }
}

fn start_usage_flush_task(
    app: &tauri::AppHandle,
    usage_service: Arc<usage::app_activity::AppActiveUsageService>,
) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            usage_service.flush(&app_handle);
        }
    });
}

fn configure_runtime_state(app: &mut tauri::App, aptabase_plugin_enabled: bool) {
    let analytics_enabled = aptabase_plugin_enabled && read_analytics_enabled(app.handle());
    app.manage(AnalyticsState {
        enabled: analytics_enabled,
    });
    if analytics_enabled {
        use tauri_plugin_aptabase::EventTracker;

        if let Err(err) = app.track_event("app_started", None) {
            utils::log_error(
                app.handle(),
                "aptabase",
                format!("track_event(app_started) failed: {}", err),
            );
        }
    }

    let pure_mode_level = read_pure_mode_level(app.handle());
    app.manage(content_filter::ContentFilter::new(pure_mode_level));
}

fn run_bootstrap_tasks(app: &tauri::AppHandle) {
    if let Err(err) = storage_manager::importer::run_legacy_import(app) {
        utils::log_error(app, "bootstrap", format!("Legacy import error: {}", err));
    }

    if let Err(err) = migrations::run_migrations(app) {
        utils::log_error(app, "bootstrap", format!("Migration error: {}", err));
    }

    if let Err(err) = chat_manager::prompts::ensure_app_default_template(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure app default template: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_local_roleplay_template(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure local roleplay template: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_help_me_reply_template(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure help me reply template: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_dynamic_memory_templates(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure dynamic memory templates: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_group_chat_templates(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure group chat templates: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_avatar_image_templates(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure avatar image templates: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_scene_generation_template(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure scene generation template: {}", err),
        );
    }

    if let Err(err) = chat_manager::prompts::ensure_design_reference_template(app) {
        utils::log_error(
            app,
            "bootstrap",
            format!("Failed to ensure design reference template: {}", err),
        );
    }
}

#[tauri::command]
pub(crate) fn get_window_chrome_flags(app: tauri::AppHandle) -> Result<(bool, bool), String> {
    let flags = app.state::<WindowChromeFlags>();
    Ok((flags.os_decorations, flags.no_buttons))
}

#[derive(Clone, Copy)]
pub(crate) struct WindowChromeFlags {
    /// `--osdecorations`: re-enable OS titlebar, hide custom buttons.
    pub os_decorations: bool,
    /// `--nobuttons`: keep frameless window but hide custom buttons.
    pub no_buttons: bool,
}

impl WindowChromeFlags {
    pub fn from_env() -> Self {
        Self {
            os_decorations: true,
            no_buttons: true,
        }
    }
}

pub(crate) fn setup_app(
    app: &mut tauri::App,
    aptabase_plugin_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let chrome_flags = WindowChromeFlags::from_env();
    app.manage(chrome_flags);

    #[cfg(not(mobile))]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_decorations(true);
    }

    let app_usage_service = manage_core_state(app);
    initialize_android_state(app);
    initialize_logging(app);
    initialize_database(app);
    start_usage_flush_task(app.handle(), app_usage_service);
    configure_runtime_state(app, aptabase_plugin_enabled);
    run_bootstrap_tasks(app.handle());
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        host_api::maybe_start_from_settings(&app_handle).await;
    });

    Ok(())
}
