use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[cfg(target_os = "android")]
use std::fs::{self, OpenOptions};
#[cfg(target_os = "android")]
use std::io::Write;
#[cfg(target_os = "android")]
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::Mutex;
#[cfg(target_os = "android")]
use std::time::Duration;
#[cfg(target_os = "android")]
use tauri::Manager;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MonitorSessionSnapshot {
    session_id: String,
    app_version: String,
    process_id: u32,
    started_at: String,
    started_at_ms: i64,
    last_heartbeat_at: String,
    last_heartbeat_ms: i64,
    route: Option<String>,
    clean_exit: bool,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MonitorCrashContext {
    session_id: String,
    recorded_at: String,
    reason: String,
    details: String,
    route: Option<String>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MonitorBreadcrumb {
    timestamp: String,
    session_id: String,
    category: String,
    message: String,
    route: Option<String>,
}

#[cfg(target_os = "android")]
pub struct AndroidMonitorState {
    monitor_dir: PathBuf,
    session_id: Mutex<String>,
    started_at: String,
    started_at_ms: i64,
    current_route: Mutex<Option<String>>,
    write_lock: Mutex<()>,
}

#[cfg(target_os = "android")]
impl AndroidMonitorState {
    pub fn initialize(app: &AppHandle) -> Result<Self, String> {
        let now = chrono::Utc::now();
        let monitor_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data dir: {e}"))?
            .join("monitor");

        fs::create_dir_all(monitor_dir.join("crashes"))
            .map_err(|e| format!("Failed to create monitor directory: {e}"))?;

        let state = Self {
            monitor_dir,
            session_id: Mutex::new(uuid::Uuid::new_v4().to_string()),
            started_at: now.to_rfc3339(),
            started_at_ms: now.timestamp_millis(),
            current_route: Mutex::new(None),
            write_lock: Mutex::new(()),
        };

        state.archive_previous_unclean_session()?;
        let _ = fs::remove_file(state.clean_exit_marker_path());
        state.write_snapshot(false)?;
        state.append_breadcrumb("lifecycle", "app_start".to_string())?;
        Ok(state)
    }

    fn session_file_path(&self) -> PathBuf {
        self.monitor_dir.join("current-session.json")
    }

    fn previous_session_file_path(&self) -> PathBuf {
        self.monitor_dir.join("previous-session.json")
    }

    fn crash_context_file_path(&self) -> PathBuf {
        self.monitor_dir.join("crash-context.json")
    }

    fn breadcrumbs_file_path(&self) -> PathBuf {
        self.monitor_dir.join("breadcrumbs.jsonl")
    }

    fn clean_exit_marker_path(&self) -> PathBuf {
        self.monitor_dir.join("clean-exit.marker")
    }

    fn archive_previous_unclean_session(&self) -> Result<(), String> {
        let session_path = self.session_file_path();
        if !session_path.exists() || self.clean_exit_marker_path().exists() {
            return Ok(());
        }

        fs::copy(&session_path, self.previous_session_file_path())
            .map_err(|e| format!("Failed to archive previous session snapshot: {e}"))?;

        Ok(())
    }

    fn snapshot(&self, clean_exit: bool) -> Result<MonitorSessionSnapshot, String> {
        let route = self
            .current_route
            .lock()
            .map_err(|e| format!("Failed to lock route state: {e}"))?
            .clone();
        let session_id = self
            .session_id
            .lock()
            .map_err(|e| format!("Failed to lock session state: {e}"))?
            .clone();
        let now = chrono::Utc::now();

        Ok(MonitorSessionSnapshot {
            session_id,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            process_id: std::process::id(),
            started_at: self.started_at.clone(),
            started_at_ms: self.started_at_ms,
            last_heartbeat_at: now.to_rfc3339(),
            last_heartbeat_ms: now.timestamp_millis(),
            route,
            clean_exit,
        })
    }

    fn write_snapshot(&self, clean_exit: bool) -> Result<(), String> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|e| format!("Failed to lock snapshot writer: {e}"))?;
        let snapshot = self.snapshot(clean_exit)?;
        let payload = serde_json::to_vec_pretty(&snapshot)
            .map_err(|e| format!("Failed to serialize session snapshot: {e}"))?;

        fs::write(self.session_file_path(), payload)
            .map_err(|e| format!("Failed to write session snapshot: {e}"))?;

        if clean_exit {
            let marker = format!(
                "session_id={}\ncompleted_at={}\n",
                snapshot.session_id, snapshot.last_heartbeat_at
            );
            fs::write(self.clean_exit_marker_path(), marker)
                .map_err(|e| format!("Failed to write clean exit marker: {e}"))?;
            let _ = fs::remove_file(self.crash_context_file_path());
        }

        Ok(())
    }

    fn set_route(&self, route: Option<String>) -> Result<(), String> {
        let mut current_route = self
            .current_route
            .lock()
            .map_err(|e| format!("Failed to lock route state: {e}"))?;
        let changed = *current_route != route;
        *current_route = route.clone();
        drop(current_route);

        if changed {
            self.append_breadcrumb(
                "route",
                route.clone().unwrap_or_else(|| "unknown".to_string()),
            )?;
        }

        Ok(())
    }

    fn append_breadcrumb(&self, category: &str, message: String) -> Result<(), String> {
        let route = self
            .current_route
            .lock()
            .map_err(|e| format!("Failed to lock route state: {e}"))?
            .clone();
        let session_id = self
            .session_id
            .lock()
            .map_err(|e| format!("Failed to lock session state: {e}"))?
            .clone();
        let breadcrumb = MonitorBreadcrumb {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id,
            category: category.to_string(),
            message,
            route,
        };
        let serialized = serde_json::to_string(&breadcrumb)
            .map_err(|e| format!("Failed to serialize monitor breadcrumb: {e}"))?;

        {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.breadcrumbs_file_path())
                .map_err(|e| format!("Failed to open monitor breadcrumb file: {e}"))?;
            writeln!(file, "{serialized}")
                .map_err(|e| format!("Failed to write monitor breadcrumb: {e}"))?;
        }

        self.trim_breadcrumbs_file()
    }

    fn trim_breadcrumbs_file(&self) -> Result<(), String> {
        let path = self.breadcrumbs_file_path();
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(format!("Failed to read monitor breadcrumbs: {err}")),
        };

        let lines: Vec<&str> = contents.lines().collect();
        const MAX_BREADCRUMBS: usize = 200;
        if lines.len() <= MAX_BREADCRUMBS {
            return Ok(());
        }

        let trimmed = lines[lines.len() - MAX_BREADCRUMBS..].join("\n") + "\n";
        fs::write(path, trimmed).map_err(|e| format!("Failed to trim monitor breadcrumbs: {e}"))
    }

    fn write_crash_context(&self, reason: &str, details: &str) -> Result<(), String> {
        let route = self
            .current_route
            .lock()
            .map_err(|e| format!("Failed to lock route state: {e}"))?
            .clone();
        let session_id = self
            .session_id
            .lock()
            .map_err(|e| format!("Failed to lock session state: {e}"))?
            .clone();
        let context = MonitorCrashContext {
            session_id,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            reason: reason.to_string(),
            details: details.to_string(),
            route,
        };
        let payload = serde_json::to_vec_pretty(&context)
            .map_err(|e| format!("Failed to serialize crash context: {e}"))?;
        fs::write(self.crash_context_file_path(), payload)
            .map_err(|e| format!("Failed to write crash context: {e}"))
    }
}

#[cfg(target_os = "android")]
pub fn initialize(app: &AppHandle) -> Result<AndroidMonitorState, String> {
    AndroidMonitorState::initialize(app)
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[cfg(not(target_os = "android"))]
pub struct AndroidMonitorState;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[cfg(not(target_os = "android"))]
pub fn initialize(_app: &AppHandle) -> Result<AndroidMonitorState, String> {
    Ok(AndroidMonitorState)
}

#[cfg(target_os = "android")]
pub fn start_heartbeat_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            let Some(state) = app.try_state::<AndroidMonitorState>() else {
                break;
            };
            let _ = state.write_snapshot(false);
        }
    });
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[cfg(not(target_os = "android"))]
pub fn start_heartbeat_loop(_app: AppHandle) {}

#[cfg(target_os = "android")]
pub fn mark_clean_exit(app: &AppHandle) {
    if let Some(state) = app.try_state::<AndroidMonitorState>() {
        let _ = state.append_breadcrumb("lifecycle", "app_exit".to_string());
        let _ = state.write_snapshot(true);
    }
}

#[cfg(not(target_os = "android"))]
pub fn mark_clean_exit(_app: &AppHandle) {}

#[tauri::command]
pub async fn android_monitor_set_route(app_handle: AppHandle, route: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        if let Some(state) = app_handle.try_state::<AndroidMonitorState>() {
            state.set_route(Some(route))?;
            state.write_snapshot(false)?;
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app_handle, route);
    }

    Ok(())
}

#[cfg(target_os = "android")]
#[allow(dead_code)]
pub fn record_breadcrumb(app: &AppHandle, category: &str, message: impl Into<String>) {
    if let Some(state) = app.try_state::<AndroidMonitorState>() {
        let _ = state.append_breadcrumb(category, message.into());
    }
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
pub fn record_breadcrumb(_app: &AppHandle, _category: &str, _message: impl Into<String>) {}

#[cfg(target_os = "android")]
#[allow(dead_code)]
pub fn record_crash_context(app: &AppHandle, reason: &str, details: impl Into<String>) {
    if let Some(state) = app.try_state::<AndroidMonitorState>() {
        let details = details.into();
        let _ = state.append_breadcrumb("crash-intent", format!("{reason}: {details}"));
        let _ = state.write_crash_context(reason, &details);
        let _ = state.write_snapshot(false);
    }
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
pub fn record_crash_context(_app: &AppHandle, _reason: &str, _details: impl Into<String>) {}
