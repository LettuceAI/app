use crate::storage_manager;
use crate::utils::{log_error, log_info};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyModelResponse {
    pub provider_id: String,
    pub credential_id: String,
    pub model: String,
    pub exists: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn verify_model_exists(
    app: tauri::AppHandle,
    provider_id: String,
    credential_id: String,
    model: String,
) -> Result<VerifyModelResponse, String> {
    log_info(
        &app,
        "verify_model",
        format!(
            "Verifying model exists: provider={} credential={} model={}",
            provider_id, credential_id, model
        ),
    );
    let supports_model_list = matches!(
        provider_id.as_str(),
        "openai"
            | "cerebras"
            | "anthropic"
            | "deepseek"
            | "qwen"
            | "moonshot"
            | "xai"
            | "featherless"
            | "nanogpt"
            | "anannas"
            | "gemini"
            | "groq"
            | "mistral"
            | "openrouter"
            | "literouter"
            | "pollinations"
    );

    if !supports_model_list {
        return Ok(VerifyModelResponse {
            provider_id,
            credential_id,
            model,
            exists: true,
            error: None,
        });
    }

    let settings_json = match storage_manager::internal_read_settings(&app)? {
        None => Value::Null,
        Some(txt) => serde_json::from_str::<Value>(&txt)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    };
    let mut base_url: Option<String> = None;
    if let Some(creds) = settings_json
        .get("providerCredentials")
        .and_then(|v| v.as_array())
    {
        for cred in creds {
            if cred.get("id").and_then(|v| v.as_str()) == Some(&credential_id) {
                base_url = cred
                    .get("baseUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }

    let mut api_key: Option<String> = None;
    if let Some(creds) = settings_json
        .get("providerCredentials")
        .and_then(|v| v.as_array())
    {
        for cred in creds {
            let cid = cred.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if cid == credential_id.as_str() {
                api_key = cred
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }
    let api_key = match api_key {
        Some(k) if !k.is_empty() => k,
        _ => {
            return Ok(VerifyModelResponse {
                provider_id,
                credential_id,
                model,
                exists: false,
                error: Some("Missing API key".into()),
            });
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let pid = crate::chat_manager::types::ProviderId(provider_id.clone());

    let base =
        crate::providers::util::resolve_base_url(&app, &pid, base_url, Some(&credential_id))?;

    let url = crate::providers::util::build_verify_url(&pid, &base);

    let headers = crate::providers::util::build_headers(&pid, &api_key).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to build headers: {}", e),
        )
    })?;

    let resp = match client.get(&url).headers(headers).send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(VerifyModelResponse {
                provider_id,
                credential_id,
                model,
                exists: false,
                error: Some(format!("request error: {}", e)),
            });
        }
    };

    if !resp.status().is_success() {
        return Ok(VerifyModelResponse {
            provider_id,
            credential_id,
            model,
            exists: false,
            error: Some(format!("HTTP {}", resp.status())),
        });
    }

    let json: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return Ok(VerifyModelResponse {
                provider_id,
                credential_id,
                model,
                exists: false,
                error: Some(format!("invalid json: {}", e)),
            });
        }
    };
    let mut exists = false;

    if provider_id == "gemini" {
        if let Some(arr) = json.get("models").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    if name == model.as_str() || name.ends_with(&format!("/{}", model)) {
                        exists = true;
                        break;
                    }
                }
            }
        }
    } else {
        if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                if item.get("id").and_then(|v| v.as_str()) == Some(model.as_str()) {
                    exists = true;
                    break;
                }
            }
        }
    }

    if exists {
        log_info(
            &app,
            "verify_model",
            format!("Model {} exists on provider {}", model, provider_id),
        );
    } else {
        log_error(
            &app,
            "verify_model",
            format!("Model {} not found on provider {}", model, provider_id),
        );
    }

    Ok(VerifyModelResponse {
        provider_id,
        credential_id,
        model,
        exists,
        error: if exists {
            None
        } else {
            Some("Model not found".into())
        },
    })
}
