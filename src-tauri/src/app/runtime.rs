use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_aptabase::EventTracker;

use crate::{abort_manager, android_monitor, usage, utils};

use super::bootstrap::AnalyticsState;

fn flush_usage_state(handler: &tauri::AppHandle) {
    if let Some(state) = handler.try_state::<Arc<usage::app_activity::AppActiveUsageService>>() {
        state.flush(handler);
    }
}

fn abort_pending_work(handler: &tauri::AppHandle) {
    if let Some(registry) = handler.try_state::<abort_manager::AbortRegistry>() {
        registry.abort_all();
    }
}

pub(crate) fn handle_run_event(handler: &tauri::AppHandle, event: tauri::RunEvent) {
    match event {
        tauri::RunEvent::Resumed => {
            if let Some(state) =
                handler.try_state::<Arc<usage::app_activity::AppActiveUsageService>>()
            {
                state.on_resumed();
            }
        }
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::Focused(focused),
            ..
        } => {
            if let Some(state) =
                handler.try_state::<Arc<usage::app_activity::AppActiveUsageService>>()
            {
                state.on_window_focus_changed(focused);
            }
        }
        tauri::RunEvent::ExitRequested { .. } => {
            abort_pending_work(handler);
            flush_usage_state(handler);
        }
        tauri::RunEvent::Exit => {
            abort_pending_work(handler);
            flush_usage_state(handler);
            android_monitor::mark_clean_exit(handler);
            let analytics_enabled = handler.state::<AnalyticsState>().enabled;
            if analytics_enabled {
                if let Err(err) = handler.track_event("app_exited", None) {
                    utils::log_error(
                        handler,
                        "aptabase",
                        format!("track_event(app_exited) failed: {}", err),
                    );
                }
                handler.flush_events_blocking();
            }
        }
        _ => {}
    }
}

pub(crate) fn configure_onnxruntime_dylib(_app: &tauri::AppHandle) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use tauri::path::BaseDirectory;
        use tauri::Manager;

        if let Ok(value) = std::env::var("ORT_DYLIB_PATH") {
            if !value.trim().is_empty() {
                let _ = ort::util::preload_dylib(&value);
                utils::log_info(
                    _app,
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
                    _app,
                    "embedding_debug",
                    format!("Set ORT_DYLIB_PATH from compile-time env: {}", value),
                );
                return;
            }
        }

        let candidates = if cfg!(target_os = "windows") {
            vec![
                "onnxruntime/onnxruntime.dll".to_string(),
                "onnxruntime.dll".to_string(),
            ]
        } else if cfg!(target_os = "macos") {
            let versioned = format!("libonnxruntime.{}.dylib", crate::embedding::ORT_VERSION);
            vec![
                format!("onnxruntime/{}", versioned),
                versioned,
                "onnxruntime/libonnxruntime.dylib".to_string(),
                "libonnxruntime.dylib".to_string(),
            ]
        } else {
            vec![
                "onnxruntime/libonnxruntime.so".to_string(),
                "libonnxruntime.so".to_string(),
            ]
        };
        let resolved_path = candidates.into_iter().find_map(|candidate| {
            _app.path()
                .resolve(candidate, BaseDirectory::Resource)
                .ok()
                .filter(|path| path.exists())
        });

        match resolved_path {
            Some(path) => {
                if path.exists() {
                    std::env::set_var("ORT_DYLIB_PATH", &path);
                    let _ = ort::util::preload_dylib(&path);
                    utils::log_info(
                        _app,
                        "embedding_debug",
                        format!("Set ORT_DYLIB_PATH={}", path.display()),
                    );
                } else {
                    utils::log_warn(
                        _app,
                        "embedding_debug",
                        format!("ONNX Runtime library not found at {}", path.display()),
                    );
                }
            }
            None => {
                utils::log_warn(
                    _app,
                    "embedding_debug",
                    "Failed to resolve ONNX Runtime resource path in bundled resources",
                );
            }
        }
    }
}
