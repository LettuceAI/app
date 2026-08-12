use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::storage_manager::settings::settings_increment_app_active_usage_ms;
use crate::utils::log_warn;

#[derive(Default)]
struct TrackerState {
    active_since_ms: Option<u64>,
    pending_active_ms: u64,
}

pub struct AppActiveUsageService {
    state: Mutex<TrackerState>,
}

impl AppActiveUsageService {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(TrackerState {
                active_since_ms: Some(now_ms()),
                pending_active_ms: 0,
            }),
        }
    }

    pub fn on_resumed(&self) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if state.active_since_ms.is_none() {
            state.active_since_ms = Some(now_ms());
        }
    }

    pub fn on_window_focus_changed(&self, focused: bool) {
        if focused {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            if state.active_since_ms.is_none() {
                state.active_since_ms = Some(now_ms());
            }
        }
    }

    pub fn flush(&self, app: &AppHandle) {
        let pending = {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            // Checkpoint active time without pausing tracking, so periodic flushes
            // can persist progress while the app stays focused.
            if let Some(started_ms) = state.active_since_ms {
                let now = now_ms();
                let elapsed = now.saturating_sub(started_ms);
                state.pending_active_ms = state.pending_active_ms.saturating_add(elapsed);
                state.active_since_ms = Some(now);
            }

            crate::utils::log_info(
                app,
                "app_active_usage",
                format!(
                    "Flushing active usage, pending={}ms",
                    state.pending_active_ms
                ),
            );

            state.pending_active_ms
        };

        if pending == 0 {
            return;
        }

        if crate::sync::manager::is_sync_database_active() {
            crate::utils::log_info(
                app,
                "app_active_usage",
                format!(
                    "Deferring {}ms of active usage while local sync owns the database",
                    pending
                ),
            );
            return;
        }

        match settings_increment_app_active_usage_ms(app, pending) {
            Ok(()) => {
                if let Ok(mut state) = self.state.lock() {
                    state.pending_active_ms = 0;
                }
            }
            Err(err) => {
                log_warn(
                    app,
                    "app_active_usage",
                    format!("Failed to persist active app usage: {}", err),
                );
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
