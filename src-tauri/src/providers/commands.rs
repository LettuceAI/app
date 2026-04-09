use tauri::AppHandle;

use crate::chat_manager::provider_adapter::{adapter_for, ModelInfo};
use crate::storage_manager::providers::get_provider_credential;
use crate::utils::{log_error, log_info};
use serde_json::Value;

#[tauri::command]
pub async fn get_remote_models(
    app: AppHandle,
    credential_id: String,
) -> Result<Vec<ModelInfo>, String> {
    log_info(
        &app,
        "get_remote_models",
        format!("Fetching models for credential_id={}", credential_id),
    );

    // 1. Fetch credential
    let credential = get_provider_credential(&app, &credential_id)?;
    if credential.provider_id == "llamacpp" {
        return Ok(Vec::new());
    }
    if credential.provider_id == "ollama" {
        return crate::ollama::list_models(&app, &credential).await;
    }

    let is_custom_provider =
        credential.provider_id == "custom" || credential.provider_id == "custom-anthropic";
    if is_custom_provider {
        let fetch_enabled = credential
            .config
            .as_ref()
            .and_then(|c| c.get("fetchModelsEnabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !fetch_enabled {
            return Err("Model fetching is disabled for this custom provider.".to_string());
        }
        let models_endpoint = credential
            .config
            .as_ref()
            .and_then(|c| c.get("modelsEndpoint"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .unwrap_or("");
        if models_endpoint.is_empty() {
            return Err(
                "Models endpoint is required when model fetching is enabled for custom providers."
                    .to_string(),
            );
        }
    }

    // 2. Get adapter and endpoint
    let adapter = adapter_for(&credential);
    // Use the credential's base_url or default
    let base_url = credential.base_url.clone().unwrap_or_else(|| {
        crate::providers::config::resolve_base_url(
            &crate::chat_manager::types::ProviderId(credential.provider_id.clone()),
            None,
        )
    });

    let mut url = adapter.list_models_endpoint(&base_url);
    if is_custom_provider {
        let auth_mode = credential
            .config
            .as_ref()
            .and_then(|c| c.get("authMode"))
            .and_then(|v| v.as_str())
            .unwrap_or("bearer")
            .to_lowercase();
        if auth_mode == "query" {
            let query_key = credential
                .config
                .as_ref()
                .and_then(|c| c.get("authQueryParamName"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("api_key");
            let api_key = credential.api_key.as_deref().unwrap_or("");
            if !api_key.trim().is_empty() {
                let separator = if url.contains('?') { '&' } else { '?' };
                url = format!("{}{}{}={}", url, separator, query_key, api_key);
            }
        }
    }
    log_info(&app, "get_remote_models", format!("Endpoint: {}", url));

    // 3. Prepare request
    let api_key = credential.api_key.as_deref().unwrap_or("");
    let headers = adapter.headers(api_key, None);

    let client = reqwest::Client::new();
    let mut req_builder = client.get(&url);

    for (k, v) in headers {
        req_builder = req_builder.header(k, v);
    }

    // 4. Send request
    log_info(
        &app,
        "get_remote_models",
        format!("Sending request to {}", url),
    );
    let resp = req_builder
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();

    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        log_error(
            &app,
            "get_remote_models",
            format!("Error {}: {}", status, text),
        );
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Provider returned error {}: {}", status, text),
        ));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // 5. Parse response
    let models = if is_custom_provider {
        parse_custom_models_list(credential.config.as_ref(), &json)
            .unwrap_or_else(|| adapter.parse_models_list(json))
    } else {
        adapter.parse_models_list(json)
    };
    log_info(
        &app,
        "get_remote_models",
        format!("Found {} models", models.len()),
    );

    Ok(models)
}

fn parse_custom_models_list(config: Option<&Value>, payload: &Value) -> Option<Vec<ModelInfo>> {
    let cfg = config?;
    let list_path = cfg
        .get("modelsListPath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("data");
    let id_path = cfg
        .get("modelsIdPath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("id");
    let display_path = cfg
        .get("modelsDisplayNamePath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let description_path = cfg
        .get("modelsDescriptionPath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let context_path = cfg
        .get("modelsContextLengthPath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let list_node = select_path(payload, list_path)?;
    let arr = list_node.as_array()?;

    let mut out = Vec::new();
    for item in arr {
        let id = select_path(item, id_path).and_then(value_to_string)?;
        if id.trim().is_empty() {
            continue;
        }
        let display_name = display_path
            .and_then(|p| select_path(item, p))
            .and_then(value_to_string)
            .filter(|s| !s.trim().is_empty());
        let description = description_path
            .and_then(|p| select_path(item, p))
            .and_then(value_to_string)
            .filter(|s| !s.trim().is_empty());
        let context_length = context_path
            .and_then(|p| select_path(item, p))
            .and_then(value_to_u64);
        out.push(ModelInfo {
            id,
            display_name,
            description,
            context_length,
            input_price: None,
            output_price: None,
        });
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn value_to_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn select_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        let mut rem = segment;
        while !rem.is_empty() {
            if let Some(start) = rem.find('[') {
                let key = &rem[..start];
                if !key.is_empty() {
                    current = current.get(key)?;
                }
                let end = rem[start + 1..].find(']')? + start + 1;
                let idx_str = &rem[start + 1..end];
                let idx = idx_str.parse::<usize>().ok()?;
                current = current.get(idx)?;
                rem = &rem[end + 1..];
            } else {
                current = current.get(rem)?;
                rem = "";
            }
        }
    }
    Some(current)
}
