use super::util::{build_headers, build_verify_url, extract_error_message, resolve_base_url};
use crate::{chat_manager::types::ProviderId, utils::log_info};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyProviderApiKeyResult {
    pub provider_id: String,
    pub valid: bool,
    pub status: Option<u16>,
    pub error: Option<String>,
    pub details: Option<Value>,
}

#[tauri::command]
pub async fn verify_provider_api_key(
    app: tauri::AppHandle,
    provider_id: String,
    credential_id: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
) -> Result<VerifyProviderApiKeyResult, String> {
    let unsupported_providers: &[&str] = &["chutes", "chutes.ai"];

    if unsupported_providers.contains(&provider_id.as_str()) {
        return Ok(VerifyProviderApiKeyResult {
            provider_id,
            valid: true,
            status: None,
            error: None,
            details: None,
        });
    }

    log_info(
        &app,
        "verify_provider_api_key",
        format!(
            "start provider={} credential={:?}",
            provider_id, credential_id
        ),
    );

    let provided_key = api_key.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let resolved_key = if let Some(key) = provided_key {
        key
    } else {
        return Ok(VerifyProviderApiKeyResult {
            provider_id,
            valid: false,
            status: None,
            error: Some("Missing API key".into()),
            details: None,
        });
    };

    let pid: ProviderId = provider_id.clone().into();
    let base = match resolve_base_url(&app, &pid, base_url.clone(), credential_id.as_deref()) {
        Ok(url) => url,
        Err(err) => {
            return Ok(VerifyProviderApiKeyResult {
                provider_id,
                valid: false,
                status: None,
                error: Some(err),
                details: None,
            });
        }
    };

    let is_custom = matches!(pid.0.as_str(), "custom" | "custom-anthropic");
    let custom_adapter = if is_custom {
        let credential = credential_id
            .as_deref()
            .and_then(|id| {
                crate::storage_manager::providers::get_provider_credential(&app, id).ok()
            })
            .unwrap_or_else(|| crate::chat_manager::types::ProviderCredential {
                id: "verify-temp".to_string(),
                provider_id: pid.0.clone(),
                label: String::new(),
                api_key: None,
                base_url: Some(base.clone()),
                default_model: None,
                headers: None,
                config: None,
            });
        Some(crate::chat_manager::provider_adapter::adapter_for(
            &credential,
        ))
    } else {
        None
    };

    let verify_url = match &custom_adapter {
        Some(adapter) => adapter.build_url(&base, "", &resolved_key, false),
        None => build_verify_url(&pid, &base),
    };

    let mut builder = Client::builder().timeout(Duration::from_secs(10));
    if crate::tls::allow_invalid_tls_for_request(&app, Some(provider_id.as_str()), &verify_url) {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = crate::tls::apply_trusted_certificates(&app, builder)
        .build()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let headers = match &custom_adapter {
        Some(adapter) => {
            let mut map = reqwest::header::HeaderMap::new();
            for (key, value) in adapter.headers(&resolved_key, None) {
                if let (Ok(name), Ok(header_value)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    reqwest::header::HeaderValue::from_str(&value),
                ) {
                    map.insert(name, header_value);
                }
            }
            map
        }
        None => build_headers(&pid, &resolved_key)?,
    };
    let url = verify_url;

    let probe_with_post = pid.0 == "zai" || is_custom;
    let request = if probe_with_post {
        client.post(&url).headers(headers).json(&Value::Null)
    } else {
        client.get(&url).headers(headers)
    };
    let response = match request.send().await {
        Ok(resp) => resp,
        Err(err) => {
            return Ok(VerifyProviderApiKeyResult {
                provider_id,
                valid: false,
                status: None,
                error: Some(err.to_string()),
                details: None,
            });
        }
    };

    let status = response.status().as_u16();
    let json = response.json::<Value>().await.unwrap_or(Value::Null);

    let valid = if probe_with_post {
        !matches!(status, 401 | 403)
    } else {
        match status {
            200 => true,
            401 | 403 => false,
            _ => json
                .get("data")
                .and_then(|d| d.as_array())
                .map(|arr| !arr.is_empty())
                .unwrap_or(status == 200),
        }
    };

    let error = if valid {
        None
    } else {
        extract_error_message(&json)
    };

    if valid {
        log_info(
            &app,
            "verify_provider_api_key",
            format!("Verification successful for provider={}", provider_id),
        );
    } else {
        crate::utils::log_error(
            &app,
            "verify_provider_api_key",
            format!(
                "Verification failed for provider={}: status={} error={:?}",
                provider_id, status, error
            ),
        );
    }

    Ok(VerifyProviderApiKeyResult {
        provider_id,
        valid,
        status: Some(status),
        error,
        details: if valid { None } else { Some(json) },
    })
}
