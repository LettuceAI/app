use crate::{chat_manager::types::ProviderId, storage_manager};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION};
use serde_json::Value;

pub fn default_base_url(provider_id: &ProviderId) -> Option<String> {
    crate::providers::config::get_provider_config(provider_id).map(|cfg| cfg.default_base_url)
}

pub fn resolve_base_url(
    app: &tauri::AppHandle,
    provider_id: &ProviderId,
    base_override: Option<String>,
    credential_id: Option<&str>,
) -> Result<String, String> {
    if let Some(explicit) = base_override.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        return Ok(explicit);
    }

    if let Some(cred_id) = credential_id {
        if let Some(raw) = storage_manager::internal_read_settings(app)? {
            if let Ok(json) = serde_json::from_str::<Value>(&raw) {
                if let Some(creds) = json.get("providerCredentials").and_then(|v| v.as_array()) {
                    for cred in creds {
                        if cred.get("id").and_then(|v| v.as_str()) == Some(cred_id) {
                            if let Some(base) = cred
                                .get("baseUrl")
                                .and_then(|v| v.as_str())
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                            {
                                return Ok(base.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    default_base_url(provider_id)
        .map(|url| url.to_string())
        .ok_or_else(|| format!("Unsupported provider {}", provider_id.0))
}

pub fn build_headers(provider_id: &ProviderId, api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    match provider_id.0.as_str() {
        "anthropic" => {
            headers.insert(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static("2023-06-01"),
            );
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(api_key).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("invalid x-api-key header: {e}"),
                    )
                })?,
            );
        }
        "mistral" => {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(api_key).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("invalid x-api-key header: {e}"),
                    )
                })?,
            );
        }
        "gemini" => {
            // Gemini can use x-goog-api-key header, but typically uses query param
            // We'll support header-based auth as an alternative
            headers.insert(
                HeaderName::from_static("x-goog-api-key"),
                HeaderValue::from_str(api_key).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("invalid x-goog-api-key header: {e}"),
                    )
                })?,
            );
        }
        _ => {
            // Standard Bearer token for most providers
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", api_key)).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("invalid authorization header: {e}"),
                    )
                })?,
            );
        }
    }
    Ok(headers)
}

pub fn build_verify_url(provider_id: &ProviderId, base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    match provider_id.0.as_str() {
        "openrouter" => format!("{}/v1/key", trimmed),
        "groq" => format!("{}/openai/v1/models", trimmed),
        "gemini" => {
            format!("{}/models", trimmed)
        }
        "zai" => {
            if trimmed.ends_with("/v1") {
                format!("{}/llm", trimmed)
            } else {
                format!("{}/v1/llm", trimmed)
            }
        }
        "pollinations-image" => format!("{}/image/models", trimmed),
        _ => {
            if trimmed.ends_with("/v1") || trimmed.ends_with("/v1beta") {
                format!("{}/models", trimmed)
            } else {
                format!("{}/v1/models", trimmed)
            }
        }
    }
}

pub fn extract_error_message(payload: &Value) -> Option<String> {
    if let Some(prompt_feedback) = payload.get("promptFeedback") {
        if let Some(block_reason) = prompt_feedback.get("blockReason").and_then(|v| v.as_str()) {
            let message = match block_reason {
                "PROHIBITED_CONTENT" => {
                    "Content was blocked by Gemini safety filters: prohibited content detected."
                        .to_string()
                }
                "SAFETY" => "Content was blocked by Gemini safety filters.".to_string(),
                "BLOCKLIST" => {
                    "Content was blocked: input contains terms from the blocklist.".to_string()
                }
                "OTHER" => "Content was blocked by Gemini for an unspecified reason.".to_string(),
                reason => format!(
                    "Content was blocked by Gemini: {}",
                    reason.replace('_', " ").to_lowercase()
                ),
            };
            return Some(message);
        }
    }
    if let Some(error) = payload.get("error") {
        match error {
            Value::String(s) => Some(s.clone()),
            Value::Object(map) => {
                if let Some(Value::String(message)) = map.get("message") {
                    Some(message.clone())
                } else if let Some(Value::String(typ)) = map.get("type") {
                    Some(typ.clone())
                } else {
                    Some(error.to_string())
                }
            }
            other => Some(other.to_string()),
        }
    } else if let Some(Value::String(message)) = payload.get("message") {
        Some(message.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::AUTHORIZATION;

    #[test]
    fn featherless_uses_standard_authorization_header() {
        let headers = build_headers(&ProviderId("featherless".into()), "test-key").unwrap();

        assert_eq!(
            headers.get(AUTHORIZATION).unwrap(),
            &HeaderValue::from_static("Bearer test-key")
        );
        assert!(headers.get("authentication").is_none());
    }
}
