use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::chat_manager::provider_adapter::adapter_for;
use crate::storage_manager::providers::get_provider_credential;

pub const NANOGPT_PROVIDER_ID: &str = "nanogpt";
pub const QUOTA_EVENT: &str = "nanogpt://quota";
const WARNINGS_META_KEY: &str = "nanogpt_quota_warnings";

const MIN_CHECK_INTERVAL: Duration = Duration::from_secs(300);
const WARN_THRESHOLDS: [u8; 3] = [100, 90, 75];

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NanoGptQuotaWindow {
    pub used: Option<f64>,
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub percent_used: Option<f64>,
    pub reset_at: Option<String>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NanoGptSubscriptionUsage {
    pub credential_id: String,
    pub credential_label: String,
    pub active: Option<bool>,
    pub state: Option<String>,
    pub weekly: Option<NanoGptQuotaWindow>,
    pub daily: Option<NanoGptQuotaWindow>,
    pub monthly: Option<NanoGptQuotaWindow>,
    pub current_period_end: Option<String>,
    pub grace_until: Option<String>,
    pub fetched_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NanoGptQuotaWarning {
    pub threshold: u8,
    pub percent: f64,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NanoGptQuotaEvent {
    pub usage: NanoGptSubscriptionUsage,
    pub warning: Option<NanoGptQuotaWarning>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WarnRecord {
    window: String,
    threshold: u8,
}

#[derive(Default)]
struct WatchState {
    last_checked: HashMap<String, Instant>,
    in_flight: HashSet<String>,
    warned: Option<HashMap<String, WarnRecord>>,
}

fn watch_state() -> &'static Mutex<WatchState> {
    static STATE: OnceLock<Mutex<WatchState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(WatchState::default()))
}

fn load_warnings(app: &AppHandle) -> HashMap<String, WarnRecord> {
    let Ok(conn) = crate::storage_manager::db::open_db(app) else {
        return HashMap::new();
    };
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        rusqlite::params![WARNINGS_META_KEY],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|raw| serde_json::from_str(&raw).ok())
    .unwrap_or_default()
}

fn save_warnings(app: &AppHandle, warned: &HashMap<String, WarnRecord>) {
    let Ok(conn) = crate::storage_manager::db::open_db(app) else {
        return;
    };
    let Ok(encoded) = serde_json::to_string(warned) else {
        return;
    };
    let _ = conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![WARNINGS_META_KEY, encoded],
    );
}

pub fn note_request(app: &AppHandle, provider_id: &str, credential_id: &str) {
    if provider_id != NANOGPT_PROVIDER_ID || credential_id.is_empty() {
        return;
    }
    {
        let Ok(mut state) = watch_state().lock() else {
            return;
        };
        if state.in_flight.contains(credential_id) {
            return;
        }
        let due = state
            .last_checked
            .get(credential_id)
            .is_none_or(|checked| checked.elapsed() >= MIN_CHECK_INTERVAL);
        if !due {
            return;
        }
        state.in_flight.insert(credential_id.to_string());
    }

    let app = app.clone();
    let credential_id = credential_id.to_string();
    tauri::async_runtime::spawn(async move {
        let result = fetch_subscription_usage(&app, &credential_id).await;
        if let Ok(mut state) = watch_state().lock() {
            state.in_flight.remove(&credential_id);
            state
                .last_checked
                .insert(credential_id.clone(), Instant::now());
        }
        match result {
            Ok(usage) => emit_quota_event(&app, usage),
            Err(error) => crate::utils::log_error_global(
                module_path!(),
                format!("NanoGPT quota check failed: {}", error),
            ),
        }
    });
}

fn emit_quota_event(app: &AppHandle, usage: NanoGptSubscriptionUsage) {
    let warning = pending_warning(app, &usage);
    let _ = app.emit(QUOTA_EVENT, NanoGptQuotaEvent { usage, warning });
}

fn primary_window(
    usage: &NanoGptSubscriptionUsage,
) -> Option<(&'static str, &NanoGptQuotaWindow)> {
    if let Some(window) = usage.weekly.as_ref() {
        return Some(("weekly", window));
    }
    if let Some(window) = usage.daily.as_ref() {
        return Some(("daily", window));
    }
    usage.monthly.as_ref().map(|window| ("monthly", window))
}

fn crossed_threshold(percent: f64) -> Option<u8> {
    WARN_THRESHOLDS
        .into_iter()
        .find(|threshold| percent * 100.0 >= f64::from(*threshold))
}

fn window_percent(window: &NanoGptQuotaWindow) -> Option<f64> {
    match (window.used, window.limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some((used / limit).clamp(0.0, 1.0)),
        _ => window
            .percent_used
            .map(|percent| percent.clamp(0.0, 1.0)),
    }
}

fn pending_warning(
    app: &AppHandle,
    usage: &NanoGptSubscriptionUsage,
) -> Option<NanoGptQuotaWarning> {
    let (kind, window) = primary_window(usage)?;
    let percent = window_percent(window)?;
    let threshold = crossed_threshold(percent)?;
    let window_key = window
        .reset_at
        .clone()
        .or_else(|| usage.current_period_end.clone())
        .unwrap_or_else(|| kind.to_string());

    let mut state = watch_state().lock().ok()?;
    let warned = state.warned.get_or_insert_with(|| load_warnings(app));
    if warned.get(&usage.credential_id).is_some_and(|record| {
        record.window == window_key && record.threshold >= threshold
    }) {
        return None;
    }
    warned.insert(
        usage.credential_id.clone(),
        WarnRecord {
            window: window_key,
            threshold,
        },
    );
    save_warnings(app, warned);

    Some(NanoGptQuotaWarning {
        threshold,
        percent,
        kind: kind.to_string(),
    })
}

#[tauri::command]
pub async fn nanogpt_subscription_usage(
    app: AppHandle,
    credential_id: String,
) -> Result<NanoGptSubscriptionUsage, String> {
    let usage = fetch_subscription_usage(&app, &credential_id).await?;
    if let Ok(mut state) = watch_state().lock() {
        state.last_checked.insert(credential_id, Instant::now());
    }
    emit_quota_event(&app, usage.clone());
    Ok(usage)
}

pub async fn fetch_subscription_usage(
    app: &AppHandle,
    credential_id: &str,
) -> Result<NanoGptSubscriptionUsage, String> {
    let credential = get_provider_credential(app, credential_id)?;
    if credential.provider_id != "nanogpt" {
        return Err("The selected credential is not a NanoGPT provider.".to_string());
    }

    let api_key = credential
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| "The selected NanoGPT provider does not have an API key.".to_string())?;
    let base_url = credential.base_url.clone().unwrap_or_else(|| {
        crate::providers::config::resolve_base_url(
            &crate::chat_manager::types::ProviderId("nanogpt".to_string()),
            None,
        )
    });
    let url = subscription_usage_url(&base_url);
    let client =
        crate::transport::build_client(app, Some(30_000), false, Some("nanogpt"), Some(&url))
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let adapter = adapter_for(&credential);
    let mut request = client.get(&url);
    for (name, value) in adapter.headers(api_key, credential.headers.as_ref()) {
        request = request.header(name, value);
    }

    let response = request
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = response.status();
    if !status.is_success() {
        let provider_message = response
            .text()
            .await
            .ok()
            .and_then(|body| extract_error_message(&body));
        return Err(match provider_message {
            Some(message) => format!("NanoGPT returned {}: {}", status, message),
            None => format!("NanoGPT returned {} while fetching subscription usage.", status),
        });
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(parse_subscription_usage(
        &credential.id,
        &credential.label,
        &payload,
    ))
}

fn subscription_usage_url(base_url: &str) -> String {
    let mut base = base_url.trim().trim_end_matches('/').to_string();
    for suffix in ["/subscription/v1", "/paid/v1", "/v1"] {
        if base.to_ascii_lowercase().ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    let base = base.trim_end_matches('/');
    if base.to_ascii_lowercase().ends_with("/api") {
        format!("{}/subscription/v1/usage", base)
    } else if base == "https://nano-gpt.com" || base == "http://nano-gpt.com" {
        format!("{}/api/subscription/v1/usage", base)
    } else {
        format!("{}/subscription/v1/usage", base)
    }
}

fn extract_error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    for path in [
        &["error", "message"][..],
        &["message"][..],
        &["detail"][..],
    ] {
        let mut current = &value;
        let mut found = true;
        for segment in path {
            let Some(next) = current.get(segment) else {
                found = false;
                break;
            };
            current = next;
        }
        if found {
            if let Some(message) = current.as_str() {
                return Some(message.chars().take(300).collect());
            }
        }
    }
    None
}

fn parse_subscription_usage(
    credential_id: &str,
    credential_label: &str,
    payload: &Value,
) -> NanoGptSubscriptionUsage {
    NanoGptSubscriptionUsage {
        credential_id: credential_id.to_string(),
        credential_label: credential_label.to_string(),
        active: find_value(payload, &["active"]).and_then(Value::as_bool),
        state: find_value(payload, &["state", "status"])
            .and_then(Value::as_str)
            .map(str::to_string),
        weekly: parse_window(
            payload,
            &[
                "weekly",
                "week",
                "weeklyUsage",
                "weekly_usage",
                "weeklyInputTokens",
                "weekly_input_tokens",
                "inputTokens",
                "input_tokens",
            ],
        ),
        daily: parse_window(payload, &["daily", "day", "dailyUsage", "daily_usage"]),
        monthly: parse_window(
            payload,
            &["monthly", "month", "monthlyUsage", "monthly_usage"],
        ),
        current_period_end: find_value(
            payload,
            &["currentPeriodEnd", "current_period_end", "periodEnd"],
        )
        .and_then(value_to_string),
        grace_until: find_value(payload, &["graceUntil", "grace_until"])
            .and_then(value_to_string),
        fetched_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

fn parse_window(payload: &Value, aliases: &[&str]) -> Option<NanoGptQuotaWindow> {
    let node = find_value(payload, aliases)?;
    let object = node.as_object()?;
    let mut window = NanoGptQuotaWindow {
        used: number_in(node, &["used", "usage", "consumed", "tokensUsed", "tokens_used"]),
        remaining: number_in(
            node,
            &[
                "remaining",
                "left",
                "tokensRemaining",
                "tokens_remaining",
            ],
        ),
        limit: number_in(
            node,
            &["limit", "quota", "total", "allowance", "tokensLimit", "tokens_limit"],
        ),
        percent_used: number_in(
            node,
            &["percentUsed", "percent_used", "percentage", "usagePercent"],
        ),
        reset_at: value_in(
            node,
            &["resetAt", "reset_at", "resetsAt", "resets_at", "reset"],
        )
        .and_then(value_to_string),
        unit: value_in(node, &["unit", "units", "metric"])
            .and_then(Value::as_str)
            .map(str::to_string),
    };

    if window.limit.is_none() {
        window.limit = payload
            .get("limits")
            .and_then(|limits| value_in(limits, aliases))
            .and_then(|value| {
                value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
            });
        if window.limit.is_none() {
            if let (Some(used), Some(remaining)) = (window.used, window.remaining) {
                window.limit = Some(used + remaining);
            }
        } else if object.is_empty() {
            return None;
        }
    }
    if window.remaining.is_none() {
        if let (Some(limit), Some(used)) = (window.limit, window.used) {
            window.remaining = Some((limit - used).max(0.0));
        }
    }
    if window.used.is_none() {
        if let (Some(limit), Some(remaining)) = (window.limit, window.remaining) {
            window.used = Some((limit - remaining).max(0.0));
        }
    }
    window.percent_used = match (window.used, window.limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some((used / limit).clamp(0.0, 1.0)),
        _ => window.percent_used.map(|percent| {
            let normalized = if percent > 1.0 {
                percent / 100.0
            } else {
                percent
            };
            normalized.clamp(0.0, 1.0)
        }),
    };

    Some(window)
}

fn find_value<'a>(value: &'a Value, aliases: &[&str]) -> Option<&'a Value> {
    if let Some(object) = value.as_object() {
        for alias in aliases {
            if let Some(found) = object.get(*alias) {
                return Some(found);
            }
        }
        for container in ["data", "usage", "subscription", "limits", "quota", "period"] {
            if let Some(found) = object
                .get(container)
                .and_then(|nested| find_value(nested, aliases))
            {
                return Some(found);
            }
        }
    }
    None
}

fn value_in<'a>(value: &'a Value, aliases: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    aliases.iter().find_map(|alias| object.get(*alias))
}

fn number_in(value: &Value, aliases: &[&str]) -> Option<f64> {
    value_in(value, aliases).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_known_base_urls() {
        assert_eq!(
            subscription_usage_url("https://nano-gpt.com/api/v1"),
            "https://nano-gpt.com/api/subscription/v1/usage"
        );
        assert_eq!(
            subscription_usage_url("https://nano-gpt.com/api"),
            "https://nano-gpt.com/api/subscription/v1/usage"
        );
        assert_eq!(
            subscription_usage_url("https://proxy.example/nano/v1/"),
            "https://proxy.example/nano/subscription/v1/usage"
        );
    }

    #[test]
    fn parses_current_weekly_shapes_and_normalizes_percent() {
        let parsed = parse_subscription_usage(
            "credential",
            "NanoGPT",
            &json!({
                "active": true,
                "usage": {
                    "weekly": {
                        "tokens_used": "45000000",
                        "tokens_remaining": 15000000,
                        "percent_used": 75,
                        "reset_at": "2026-07-27T00:00:00Z"
                    }
                }
            }),
        );
        let weekly = parsed.weekly.unwrap();
        assert_eq!(weekly.used, Some(45_000_000.0));
        assert_eq!(weekly.limit, Some(60_000_000.0));
        assert_eq!(weekly.percent_used, Some(0.75));
    }

    #[test]
    fn derives_full_usage_when_consumed_tokens_exceed_the_limit() {
        let parsed = parse_subscription_usage(
            "credential",
            "NanoGPT",
            &json!({
                "usage": {
                    "weekly": {
                        "tokens_used": 60_018_252,
                        "tokens_remaining": 0,
                        "percent_used": 1.0003042
                    }
                },
                "limits": { "weekly": 60_000_000 }
            }),
        );
        let weekly = parsed.weekly.unwrap();
        assert_eq!(weekly.used, Some(60_018_252.0));
        assert_eq!(weekly.remaining, Some(0.0));
        assert_eq!(weekly.limit, Some(60_000_000.0));
        assert_eq!(weekly.percent_used, Some(1.0));
        assert_eq!(window_percent(&weekly), Some(1.0));
    }

    #[test]
    fn reports_the_highest_crossed_threshold_only() {
        assert_eq!(crossed_threshold(0.0), None);
        assert_eq!(crossed_threshold(0.7499), None);
        assert_eq!(crossed_threshold(0.75), Some(75));
        assert_eq!(crossed_threshold(0.89), Some(75));
        assert_eq!(crossed_threshold(0.9), Some(90));
        assert_eq!(crossed_threshold(1.0), Some(100));
        assert_eq!(crossed_threshold(1.4), Some(100));
    }

    #[test]
    fn derives_window_percent_from_used_and_limit_when_absent() {
        let window = NanoGptQuotaWindow {
            used: Some(45.0),
            limit: Some(60.0),
            ..Default::default()
        };
        assert_eq!(window_percent(&window), Some(0.75));
        assert_eq!(
            window_percent(&NanoGptQuotaWindow {
                used: Some(60_018_252.0),
                limit: Some(60_000_000.0),
                percent_used: Some(0.010003042),
                ..Default::default()
            }),
            Some(1.0)
        );
        assert_eq!(
            window_percent(&NanoGptQuotaWindow {
                used: Some(1.0),
                limit: Some(0.0),
                ..Default::default()
            }),
            None
        );
    }

    #[test]
    fn preserves_legacy_daily_and_monthly_windows() {
        let parsed = parse_subscription_usage(
            "credential",
            "NanoGPT",
            &json!({
                "active": true,
                "limits": { "daily": 5000, "monthly": 60000 },
                "daily": { "used": 5, "remaining": 4995, "percentUsed": 0.001 },
                "monthly": { "used": 50, "remaining": 59950, "percentUsed": 0.00083 },
                "period": { "currentPeriodEnd": 1785196800 }
            }),
        );
        assert_eq!(parsed.daily.unwrap().limit, Some(5000.0));
        assert_eq!(parsed.monthly.unwrap().remaining, Some(59_950.0));
        assert_eq!(parsed.current_period_end.as_deref(), Some("1785196800"));
    }
}
