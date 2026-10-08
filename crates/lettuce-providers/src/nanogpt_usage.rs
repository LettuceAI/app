use lettuce_models::ProviderAccount;
use lettuce_network::{JsonClient, RequestPolicy, RequestTimeout};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::{ACCEPT_ONLY, AuthPlan, Credentials, load_auth, load_secret_headers};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QuotaWindow {
    pub used: Option<f64>,
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub percent_used: Option<f64>,
    pub reset_at: Option<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NanoGptUsage {
    pub active: Option<bool>,
    pub state: Option<String>,
    pub weekly: Option<QuotaWindow>,
    pub daily: Option<QuotaWindow>,
    pub monthly: Option<QuotaWindow>,
    pub current_period_end: Option<String>,
    pub grace_until: Option<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NanoGptUsageError {
    #[error("account is not NanoGPT")]
    WrongProvider,
    #[error("NanoGPT API key is missing")]
    MissingApiKey,
    #[error("NanoGPT credentials cannot be read")]
    CredentialsUnavailable,
    #[error("NanoGPT usage transport failed")]
    Transport,
    #[error("NanoGPT usage response was rejected")]
    ProviderRejected {
        status: u16,
        message: Option<String>,
    },
    #[error("NanoGPT usage response is malformed")]
    Malformed,
}

pub(crate) async fn fetch<S: SecretStore + ?Sized>(
    store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
) -> Result<NanoGptUsage, NanoGptUsageError> {
    if account.provider_kind != "nanogpt" {
        return Err(NanoGptUsageError::WrongProvider);
    }
    if account.api_key_ref.is_none() {
        return Err(NanoGptUsageError::MissingApiKey);
    }
    let credentials = Credentials::from(account);
    let auth = load_auth(AuthPlan::Bearer, store, &credentials)
        .await
        .map_err(|error| match error {
            crate::common::AdapterError::CredentialRejected => NanoGptUsageError::MissingApiKey,
            _ => NanoGptUsageError::CredentialsUnavailable,
        })?;
    let headers = load_secret_headers(store, &credentials)
        .await
        .map_err(|_| NanoGptUsageError::CredentialsUnavailable)?;
    let mut sensitive = Vec::new();
    if let lettuce_network::JsonAuth::Bearer(value) = &auth {
        sensitive.push(
            value
                .with(|secret| lettuce_settings::SecretValue::new(secret))
                .map_err(|_| NanoGptUsageError::CredentialsUnavailable)?,
        );
    }
    for header in &headers {
        sensitive.push(
            header
                .value
                .with(|secret| lettuce_settings::SecretValue::new(secret))
                .map_err(|_| NanoGptUsageError::CredentialsUnavailable)?,
        );
    }
    let url = usage_url(
        account
            .endpoint
            .as_deref()
            .unwrap_or("https://nano-gpt.com/api"),
    );
    let endpoint = url
        .strip_suffix("/usage")
        .ok_or(NanoGptUsageError::Malformed)?;
    let response = network
        .get_json(
            endpoint,
            "/usage",
            &ACCEPT_ONLY,
            auth,
            headers,
            RequestPolicy {
                timeout: RequestTimeout::Browse,
                allow_invalid_tls: false,
            },
        )
        .await
        .map_err(|_| NanoGptUsageError::Transport)?;
    if !(200..300).contains(&response.status) {
        let mut message = crate::verify::provider_error(&response.body).or_else(|| {
            String::from_utf8(response.body)
                .ok()
                .filter(|body| !body.trim().is_empty())
        });
        if let Some(message) = &mut message {
            for value in sensitive {
                value.with(|secret| crate::verify::redact(message, secret));
            }
        }
        return Err(NanoGptUsageError::ProviderRejected {
            status: response.status,
            message,
        });
    }
    let payload =
        serde_json::from_slice(&response.body).map_err(|_| NanoGptUsageError::Malformed)?;
    parse_usage(&payload)
}

pub(crate) fn parse_usage(payload: &Value) -> Result<NanoGptUsage, NanoGptUsageError> {
    if !payload.is_object() {
        return Err(NanoGptUsageError::Malformed);
    }
    let usage = NanoGptUsage {
        active: find_value(payload, &["active"]).and_then(Value::as_bool),
        state: find_value(payload, &["state", "status"])
            .and_then(Value::as_str)
            .map(str::to_owned),
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
        )?,
        daily: parse_window(payload, &["daily", "day", "dailyUsage", "daily_usage"])?,
        monthly: parse_window(
            payload,
            &["monthly", "month", "monthlyUsage", "monthly_usage"],
        )?,
        current_period_end: find_value(
            payload,
            &["currentPeriodEnd", "current_period_end", "periodEnd"],
        )
        .and_then(value_to_string),
        grace_until: find_value(payload, &["graceUntil", "grace_until"]).and_then(value_to_string),
    };
    if [&usage.weekly, &usage.daily, &usage.monthly]
        .iter()
        .all(|window| window.is_none())
        && usage.active.is_none()
        && usage.state.is_none()
        && usage.current_period_end.is_none()
        && usage.grace_until.is_none()
    {
        return Err(NanoGptUsageError::Malformed);
    }
    Ok(usage)
}

pub(crate) fn usage_url(base_url: &str) -> String {
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

fn parse_window(
    payload: &Value,
    aliases: &[&str],
) -> Result<Option<QuotaWindow>, NanoGptUsageError> {
    Ok(parse_window_value(payload, aliases))
}

fn parse_window_value(payload: &Value, aliases: &[&str]) -> Option<QuotaWindow> {
    let node = find_value(payload, aliases)?;
    let object = node.as_object()?;
    let mut window = QuotaWindow {
        used: number_in(
            node,
            &["used", "usage", "consumed", "tokensUsed", "tokens_used"],
        ),
        remaining: number_in(
            node,
            &["remaining", "left", "tokensRemaining", "tokens_remaining"],
        ),
        limit: number_in(
            node,
            &[
                "limit",
                "quota",
                "total",
                "allowance",
                "tokensLimit",
                "tokens_limit",
            ],
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

    if window.used.is_none()
        && window.remaining.is_none()
        && window.limit.is_none()
        && window.percent_used.is_none()
        && window.reset_at.is_none()
        && window.unit.is_none()
    {
        None
    } else {
        Some(window)
    }
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
