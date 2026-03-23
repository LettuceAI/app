use crate::logger::{get_global_app_handle, LogEntry, LogManager};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use if_addrs::get_if_addrs;
use serde::Serialize;
use serde_json::{json, Value};
use std::cell::Cell;
use std::fmt;
use std::fs;
use std::panic::Location;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{Event, Subscriber};
use tracing_subscriber::field::Visit;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

pub const _SERVICE: &str = "1.0.0-beta-7";

pub fn lettuce_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let lettuce_path = base.join("lettuce");
    Ok(lettuce_path)
}

pub fn ensure_lettuce_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = lettuce_dir(app)?;
    fs::create_dir_all(&dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(dir)
}

pub fn now_millis() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .as_millis() as u64)
}

pub fn emit_debug(app: &AppHandle, phase: &str, payload: Value) {
    let event = json!({
        "state": phase,
        "payload": payload,
    });
    let _ = app.emit("chat://debug", event);
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

static MAX_LOG_CHARS: usize = 1200;
static MIN_LOG_LEVEL: OnceLock<LogLevel> = OnceLock::new();
static TRACING_INIT: OnceLock<()> = OnceLock::new();

thread_local! {
    static IN_TRACING_LAYER: Cell<bool> = const { Cell::new(false) };
}

fn level_rank(level: LogLevel) -> u8 {
    match level {
        LogLevel::Debug => 10,
        LogLevel::Info => 20,
        LogLevel::Warn => 30,
        LogLevel::Error => 40,
    }
}

fn parse_level(value: &str) -> Option<LogLevel> {
    match value.trim().to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" | "warning" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}

fn min_log_level() -> LogLevel {
    *MIN_LOG_LEVEL.get_or_init(|| {
        std::env::var("LETTUCE_LOG_LEVEL")
            .ok()
            .and_then(|value| parse_level(&value))
            .unwrap_or(LogLevel::Info)
    })
}

fn should_downgrade_to_debug(component: &str, message: &str) -> bool {
    let c = component.to_lowercase();
    let m = message.to_lowercase();
    if c == "prompt_engine" {
        return m.contains("template contains")
            || m.contains("before {{scene}}")
            || m.contains("scene_content length")
            || m.contains("direction length")
            || m.contains("template vars")
            || m.contains("system_prompt_built");
    }
    if c == "api_request" {
        return m.contains("adding header")
            || m.contains("all headers set")
            || m.contains("setting body as json")
            || m.contains("request body")
            || m.contains("response body");
    }
    false
}

fn redact_param_value(message: &str, param: &str) -> String {
    let mut output = String::with_capacity(message.len());
    let lower = message.to_lowercase();
    let needle = format!("{}=", param);
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&needle) {
        let start = i + pos;
        output.push_str(&message[i..start]);
        let value_start = start + needle.len();
        output.push_str(&message[start..value_start]);
        let mut end = value_start;
        let bytes = message.as_bytes();
        while end < message.len() {
            let b = bytes[end] as char;
            if b.is_whitespace() || b == '&' || b == '"' || b == '\'' || b == ')' || b == ']' {
                break;
            }
            end += 1;
        }
        output.push_str("***");
        i = end;
    }
    output.push_str(&message[i..]);
    output
}

fn redact_authorization(message: &str) -> String {
    let lower = message.to_lowercase();
    if !lower.contains("authorization") && !lower.contains("bearer") {
        return message.to_string();
    }
    let mut out = message.to_string();
    if let Some(idx) = lower.find("bearer ") {
        let start = idx + "bearer ".len();
        let mut end = start;
        let bytes = message.as_bytes();
        while end < message.len() {
            let c = bytes[end] as char;
            if c.is_whitespace() || c == '"' || c == '\'' || c == ',' {
                break;
            }
            end += 1;
        }
        out.replace_range(start..end, "***");
    }
    out
}

fn redact_body_payload(message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    let markers = [
        "request body:",
        "response body:",
        "setting body as json:",
        "request body",
    ];
    for marker in markers {
        if let Some(pos) = lower.find(marker) {
            let split_at = pos + marker.len();
            let (prefix, rest) = message.split_at(split_at);
            let len = rest.len();
            return Some(format!("{} <redacted body len={}>", prefix.trim_end(), len));
        }
    }
    None
}

fn sanitize_message(component: &str, message: &str) -> String {
    let mut msg = message.replace('\n', "\\n").replace('\r', "\\r");
    if let Some(redacted) = redact_body_payload(&msg) {
        msg = redacted;
    }
    msg = redact_authorization(&msg);
    for key in [
        "key",
        "api_key",
        "apikey",
        "access_token",
        "token",
        "authorization",
        "x-api-key",
    ] {
        msg = redact_param_value(&msg, key);
    }
    msg = redact_param_value(&msg, "key");

    if msg.len() > MAX_LOG_CHARS {
        let truncated = msg.chars().take(MAX_LOG_CHARS).collect::<String>();
        msg = format!("{}... <truncated>", truncated);
    }

    if component == "api_request" && msg.contains("full_url=") {
        msg = msg.replace("full_url=", "url=");
    }

    msg
}

#[derive(Default)]
struct EventFieldVisitor {
    component: Option<String>,
    message: Option<String>,
    caller_file: Option<String>,
    caller_line: Option<u32>,
    caller_column: Option<u32>,
}

impl Visit for EventFieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "component" => self.component = Some(value.to_string()),
            "message" | "msg" => self.message = Some(value.to_string()),
            "caller_file" => self.caller_file = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        match field.name() {
            "caller_line" => self.caller_line = Some(value as u32),
            "caller_column" => self.caller_column = Some(value as u32),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        let rendered = format!("{value:?}");
        match field.name() {
            "component" => self.component = Some(rendered),
            "message" | "msg" => self.message = Some(rendered),
            _ => {}
        }
    }
}

struct AppTracingLayer {
    app: AppHandle,
}

impl AppTracingLayer {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

fn span_scope_path<S>(ctx: Context<'_, S>, event: &Event<'_>) -> Option<String>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ctx.event_scope(event).map(|scope| {
        scope
            .from_root()
            .map(|span| span.metadata().name())
            .collect::<Vec<_>>()
            .join(" > ")
    })
}

impl<S> Layer<S> for AppTracingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let already_inside = IN_TRACING_LAYER.with(|flag| {
            if flag.get() {
                true
            } else {
                flag.set(true);
                false
            }
        });
        if already_inside {
            return;
        }

        let mut visitor = EventFieldVisitor::default();
        event.record(&mut visitor);

        let level = event.metadata().level().as_str().to_string();
        let component = visitor
            .component
            .unwrap_or_else(|| event.metadata().target().to_string());
        let function = visitor
            .caller_file
            .as_ref()
            .zip(visitor.caller_line)
            .map(|(file, line)| {
                format!("{}:{}:{}", file, line, visitor.caller_column.unwrap_or(1))
            });
        let span_path = span_scope_path(ctx, event);
        let message = sanitize_message(
            &component,
            visitor.message.as_deref().unwrap_or("(no message)"),
        );
        let stored_message = if let Some(path) = &span_path {
            format!("{message} [span={path}]")
        } else {
            message.clone()
        };
        let mut details = Vec::new();
        if let Some(path) = &span_path {
            details.push(format!("span={path}"));
        }
        if let Some(func) = &function {
            details.push(format!("at={func}"));
        }
        let display_message = if details.is_empty() {
            message.clone()
        } else {
            format!("{message} ({})", details.join(", "))
        };

        let now = chrono::Local::now();
        let formatted = format!(
            "[{}] {} {} | {}",
            now.format("%H:%M:%S"),
            level,
            component,
            display_message
        );
        let payload = json!({
            "state": component,
            "level": level,
            "message": formatted,
        });
        let _ = self.app.emit("chat://debug", payload);

        if let Some(log_manager) = self.app.try_state::<LogManager>() {
            let entry = LogEntry {
                timestamp: now.to_rfc3339(),
                level: level.clone(),
                component: component.clone(),
                function,
                message: stored_message,
            };
            let _ = log_manager.write_log(entry);
        }

        IN_TRACING_LAYER.with(|flag| flag.set(false));
    }
}

fn min_level_filter() -> LevelFilter {
    match min_log_level() {
        LogLevel::Debug => LevelFilter::DEBUG,
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Error => LevelFilter::ERROR,
    }
}

pub(crate) fn init_tracing(app: AppHandle) -> Result<(), String> {
    if TRACING_INIT.get().is_some() {
        return Ok(());
    }

    let subscriber = tracing_subscriber::registry()
        .with(min_level_filter())
        .with(AppTracingLayer::new(app));
    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to initialize tracing subscriber: {}", e),
        )
    })?;
    let _ = TRACING_INIT.set(());
    Ok(())
}

/// Structured backend logger that emits formatted log messages via event system.
/// Format: [HH:MM:SS] component[/function] LEVEL message
#[track_caller]
pub(crate) fn log_backend(
    _app: &AppHandle,
    component: &str,
    level: LogLevel,
    message: impl AsRef<str>,
) {
    let min_level = min_log_level();
    let raw_message = message.as_ref();
    let effective_level = if should_downgrade_to_debug(component, raw_message) {
        LogLevel::Debug
    } else {
        level
    };
    if level_rank(effective_level) < level_rank(min_level) {
        return;
    }

    let sanitized = sanitize_message(component, raw_message);
    let location = Location::caller();
    let caller_file = location.file();
    let caller_line = location.line() as u64;
    let caller_column = location.column() as u64;

    match effective_level {
        LogLevel::Debug => {
            tracing::event!(
                tracing::Level::DEBUG,
                component = component,
                message = sanitized.as_str(),
                caller_file = caller_file,
                caller_line = caller_line,
                caller_column = caller_column
            );
        }
        LogLevel::Info => {
            tracing::event!(
                tracing::Level::INFO,
                component = component,
                message = sanitized.as_str(),
                caller_file = caller_file,
                caller_line = caller_line,
                caller_column = caller_column
            );
        }
        LogLevel::Warn => {
            tracing::event!(
                tracing::Level::WARN,
                component = component,
                message = sanitized.as_str(),
                caller_file = caller_file,
                caller_line = caller_line,
                caller_column = caller_column
            );
        }
        LogLevel::Error => {
            tracing::event!(
                tracing::Level::ERROR,
                component = component,
                message = sanitized.as_str(),
                caller_file = caller_file,
                caller_line = caller_line,
                caller_column = caller_column
            );
        }
    }
}

/// Convenience wrappers for common log levels
#[track_caller]
pub(crate) fn log_info(app: &AppHandle, component: &str, message: impl AsRef<str>) {
    log_backend(app, component, LogLevel::Info, message);
}

#[track_caller]
pub(crate) fn log_warn(app: &AppHandle, component: &str, message: impl AsRef<str>) {
    log_backend(app, component, LogLevel::Warn, message);
}

#[track_caller]
pub(crate) fn log_error(app: &AppHandle, component: &str, message: impl AsRef<str>) {
    log_backend(app, component, LogLevel::Error, message);
}

#[allow(dead_code)]
#[track_caller]
pub(crate) fn log_debug(app: &AppHandle, component: &str, message: impl AsRef<str>) {
    log_backend(app, component, LogLevel::Debug, message);
}

#[track_caller]
pub(crate) fn log_debug_global(component: &str, message: impl AsRef<str>) {
    if let Some(app) = get_global_app_handle() {
        log_backend(&app, component, LogLevel::Debug, message);
    } else {
        eprintln!("[DEBUG] {} {}", component, message.as_ref());
    }
}

#[track_caller]
pub(crate) fn log_info_global(component: &str, message: impl AsRef<str>) {
    if let Some(app) = get_global_app_handle() {
        log_backend(&app, component, LogLevel::Info, message);
    } else {
        eprintln!("[INFO] {} {}", component, message.as_ref());
    }
}

#[allow(dead_code)]
#[track_caller]
pub(crate) fn log_warn_global(component: &str, message: impl AsRef<str>) {
    if let Some(app) = get_global_app_handle() {
        log_backend(&app, component, LogLevel::Warn, message);
    } else {
        eprintln!("[WARN] {} {}", component, message.as_ref());
    }
}

#[track_caller]
pub(crate) fn log_error_global(component: &str, message: impl AsRef<str>) {
    if let Some(app) = get_global_app_handle() {
        log_backend(&app, component, LogLevel::Error, message);
    } else {
        eprintln!("[ERROR] {} {}", component, message.as_ref());
    }
}

pub(crate) fn err_to_string<E: std::fmt::Display>(component: &str, line: u32, err: E) -> String {
    log_error_global(component, format!("line {}: {}", line, err));
    err.to_string()
}

pub(crate) fn err_msg(component: &str, line: u32, message: impl AsRef<str>) -> String {
    log_error_global(component, format!("line {}: {}", line, message.as_ref()));
    message.as_ref().to_string()
}

pub fn emit_toast(
    app: &AppHandle,
    variant: &str,
    title: impl AsRef<str>,
    description: Option<String>,
) {
    let payload = json!({
        "variant": variant,
        "title": title.as_ref(),
        "description": description,
    });
    let _ = app.emit("app://toast", payload);
}

pub(crate) fn app_version(app: &AppHandle) -> String {
    let mut version = app.package_info().version.to_string();
    if cfg!(feature = "llama-gpu-cuda") || cfg!(feature = "llama-gpu-cuda-no-vmm") {
        version.push_str("-cuda");
    } else if cfg!(feature = "llama-gpu-rocm") {
        version.push_str("-rocm");
    } else if cfg!(feature = "llama-gpu-vulkan") {
        version.push_str("-vulkan");
    }
    version
}

pub fn get_local_ip() -> Result<String, String> {
    if let Ok(ifaces) = get_if_addrs() {
        for iface in &ifaces {
            if !iface.is_loopback() {
                if let if_addrs::IfAddr::V4(v4) = &iface.addr {
                    let ip = v4.ip.to_string();
                    if ip.starts_with("192.168.") {
                        return Ok(ip);
                    }
                }
            }
        }

        for iface in &ifaces {
            if !iface.is_loopback() {
                if let if_addrs::IfAddr::V4(v4) = &iface.addr {
                    return Ok(v4.ip.to_string());
                }
            }
        }
    }

    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[derive(Clone, Serialize)]
pub struct AccessibilitySoundBase64 {
    pub send: String,
    pub success: String,
    pub failure: String,
}

static ACCESSIBILITY_SOUND_CACHE: OnceLock<Mutex<Option<AccessibilitySoundBase64>>> =
    OnceLock::new();

#[tauri::command]
pub fn accessibility_sound_base64(_app: AppHandle) -> Result<AccessibilitySoundBase64, String> {
    let cache = ACCESSIBILITY_SOUND_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| "Accessibility sound cache lock poisoned".to_string())?;
    if let Some(cached) = guard.as_ref() {
        return Ok(cached.clone());
    }

    let sounds = AccessibilitySoundBase64 {
        send: STANDARD.encode(include_bytes!("../../feedback_sounds/send.mp3")),
        success: STANDARD.encode(include_bytes!("../../feedback_sounds/success.mp3")),
        failure: STANDARD.encode(include_bytes!("../../feedback_sounds/fail.mp3")),
    };
    *guard = Some(sounds.clone());
    Ok(sounds)
}

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app_version(&app)
}

#[tauri::command]
pub fn developer_force_crash(app: AppHandle) -> Result<(), String> {
    log_error(
        &app,
        "developer",
        "Intentional crash triggered from developer settings",
    );
    #[cfg(target_os = "android")]
    crate::android_monitor::record_crash_context(
        &app,
        "developer_force_crash",
        "Crash App Now button pressed in developer settings",
    );
    std::thread::sleep(std::time::Duration::from_millis(150));

    #[cfg(target_os = "android")]
    {
        unsafe {
            let pid = libc::getpid();
            libc::kill(pid, libc::SIGABRT);
            libc::kill(pid, libc::SIGKILL);
            libc::_exit(134);
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        std::process::abort();
    }
}
