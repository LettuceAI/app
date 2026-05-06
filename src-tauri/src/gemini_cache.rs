use std::collections::HashMap;
use std::sync::Mutex;

use lazy_static::lazy_static;
use reqwest::Method;
use serde_json::{json, Value};

use crate::api::ApiRequest;
use crate::serde_utils::parse_body_to_value;
use crate::transport;
use crate::utils::{log_info, log_warn};

const INTERNAL_ENABLED_KEY: &str = "_lettucePromptCachingEnabled";
const INTERNAL_TTL_KEY: &str = "_lettucePromptCachingTtl";

#[derive(Clone, Debug)]
struct GeminiCachedContentEntry {
    name: String,
    expires_at_epoch: i64,
}

lazy_static! {
    static ref GEMINI_CACHED_CONTENTS: Mutex<HashMap<String, GeminiCachedContentEntry>> =
        Mutex::new(HashMap::new());
}

pub fn is_gemini_provider(provider_id: Option<&str>) -> bool {
    matches!(provider_id, Some("gemini" | "google" | "google-gemini"))
}

pub async fn maybe_apply_gemini_explicit_cache(
    app: &tauri::AppHandle,
    req: &mut ApiRequest,
) -> Result<(), String> {
    if !is_gemini_provider(req.provider_id.as_deref()) {
        return Ok(());
    }

    let Some(body) = req.body.as_ref() else {
        return Ok(());
    };
    let Some(body_obj) = body.as_object() else {
        return Ok(());
    };

    let prompt_caching_enabled = body_obj
        .get(INTERNAL_ENABLED_KEY)
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let ttl_value = body_obj
        .get(INTERNAL_TTL_KEY)
        .and_then(|value| value.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "1h".to_string());

    if !prompt_caching_enabled {
        return Ok(());
    }

    let Some(contents) = body_obj.get("contents").and_then(|value| value.as_array()) else {
        return Ok(());
    };

    if contents.len() < 2 {
        log_info(
            app,
            "gemini_cache",
            "Skipping explicit Gemini cache: not enough content blocks to split prefix and live turn",
        );
        return Ok(());
    }

    let prefix_contents = contents[..contents.len() - 1].to_vec();
    let live_contents = vec![contents[contents.len() - 1].clone()];
    let system_instruction = body_obj.get("systemInstruction").cloned();
    let tools = body_obj.get("tools").cloned();

    let api_key = extract_gemini_api_key(req).ok_or_else(|| {
        "Gemini prompt caching requested, but no API key was available for cachedContents"
            .to_string()
    })?;
    let model_name = extract_gemini_model_name(&req.url).ok_or_else(|| {
        format!(
            "Gemini prompt caching requested, but model name could not be parsed from URL: {}",
            req.url
        )
    })?;
    let cache_endpoint = build_gemini_cached_contents_url(&req.url, &api_key).ok_or_else(|| {
        format!(
            "Gemini prompt caching requested, but cachedContents endpoint could not be derived from URL: {}",
            req.url
        )
    })?;
    let api_ttl = gemini_ttl_to_api(&ttl_value);
    let cache_key = build_cache_key(
        &api_key,
        &model_name,
        system_instruction.as_ref(),
        &prefix_contents,
        tools.as_ref(),
        &api_ttl,
    )?;

    let now_epoch = chrono::Utc::now().timestamp();
    let cached_name = {
        let mut cache = GEMINI_CACHED_CONTENTS
            .lock()
            .map_err(|e| format!("Gemini cache lock poisoned: {}", e))?;
        cache.retain(|_, entry| entry.expires_at_epoch > now_epoch);
        cache.get(&cache_key).map(|entry| entry.name.clone())
    };

    let cache_name = if let Some(existing) = cached_name {
        existing
    } else {
        let created = create_gemini_cached_content(
            app,
            req,
            &cache_endpoint,
            &model_name,
            system_instruction.as_ref(),
            &prefix_contents,
            &api_ttl,
        )
        .await?;

        let mut cache = GEMINI_CACHED_CONTENTS
            .lock()
            .map_err(|e| format!("Gemini cache lock poisoned: {}", e))?;
        cache.insert(
            cache_key,
            GeminiCachedContentEntry {
                name: created.clone(),
                expires_at_epoch: now_epoch + gemini_ttl_to_seconds(&api_ttl),
            },
        );
        created
    };

    let Some(body) = req.body.as_mut() else {
        return Ok(());
    };
    let Some(body_obj) = body.as_object_mut() else {
        return Ok(());
    };

    body_obj.remove(INTERNAL_ENABLED_KEY);
    body_obj.remove(INTERNAL_TTL_KEY);
    body_obj.insert("contents".to_string(), Value::Array(live_contents));
    body_obj.insert("cachedContent".to_string(), Value::String(cache_name));
    body_obj.remove("systemInstruction");

    Ok(())
}

fn extract_gemini_api_key(req: &ApiRequest) -> Option<String> {
    req.headers
        .as_ref()
        .and_then(|headers| headers.get("x-goog-api-key").cloned())
        .or_else(|| {
            req.url.split('?').nth(1).and_then(|query| {
                query.split('&').find_map(|segment| {
                    let (key, value) = segment.split_once('=')?;
                    (key == "key").then(|| value.to_string())
                })
            })
        })
}

fn extract_gemini_model_name(url: &str) -> Option<String> {
    let after_models = url.split("/models/").nth(1)?;
    let model = after_models
        .split(':')
        .next()
        .unwrap_or(after_models)
        .trim();
    (!model.is_empty()).then(|| model.to_string())
}

fn build_gemini_cached_contents_url(url: &str, api_key: &str) -> Option<String> {
    let base = url.split("/models/").next()?.trim_end_matches('/');
    Some(format!("{}/cachedContents?key={}", base, api_key))
}

fn gemini_ttl_to_api(ttl: &str) -> String {
    match ttl {
        "5min" => "300s".to_string(),
        "1h" => "3600s".to_string(),
        "300s" | "3600s" => ttl.to_string(),
        _ => "3600s".to_string(),
    }
}

fn gemini_ttl_to_seconds(ttl: &str) -> i64 {
    match ttl {
        "300s" | "5min" => 300,
        "3600s" | "1h" => 3600,
        _ => 3600,
    }
}

fn build_cache_key(
    api_key: &str,
    model_name: &str,
    system_instruction: Option<&Value>,
    prefix_contents: &[Value],
    tools: Option<&Value>,
    ttl: &str,
) -> Result<String, String> {
    let payload = serde_json::to_vec(&json!({
        "apiKey": api_key,
        "model": model_name,
        "systemInstruction": system_instruction,
        "contents": prefix_contents,
        "tools": tools,
        "ttl": ttl,
    }))
    .map_err(|e| format!("Failed to serialize Gemini cache key payload: {}", e))?;

    Ok(blake3::hash(&payload).to_hex().to_string())
}

async fn create_gemini_cached_content(
    app: &tauri::AppHandle,
    req: &ApiRequest,
    endpoint: &str,
    model_name: &str,
    system_instruction: Option<&Value>,
    prefix_contents: &[Value],
    ttl: &str,
) -> Result<String, String> {
    let client = transport::build_client(req.timeout_ms, false).map_err(|e| e.to_string())?;
    let mut builder = client.request(Method::POST, endpoint);

    if let Some(headers) = &req.headers {
        for (key, value) in headers {
            builder = builder.header(key, value);
        }
    }

    let mut payload = json!({
        "model": if model_name.starts_with("models/") {
            model_name.to_string()
        } else {
            format!("models/{}", model_name)
        },
        "contents": prefix_contents,
        "ttl": ttl,
    });

    if let Some(system_instruction) = system_instruction.cloned() {
        payload["systemInstruction"] = system_instruction;
    }

    builder = builder.json(&payload);

    log_info(
        app,
        "gemini_cache",
        format!(
            "Creating Gemini cached content for model={} ttl={} prefix_blocks={}",
            model_name,
            ttl,
            prefix_contents.len()
        ),
    );

    let response = transport::send_with_retries(app, "gemini_cache_create", builder, 2, None).await;
    let response = match response {
        Ok(response) => response,
        Err(err) => {
            log_warn(
                app,
                "gemini_cache",
                format!(
                    "Gemini cached content creation failed, falling back to uncached request: {}",
                    err
                ),
            );
            return Err(err.to_string());
        }
    };

    let status = response.status();
    let body_text = response.text().await.map_err(|err| err.to_string())?;
    let body = parse_body_to_value(&body_text);
    if !status.is_success() {
        log_warn(
            app,
            "gemini_cache",
            format!(
                "Gemini cached content creation returned status {}. Falling back to uncached request.",
                status
            ),
        );
        return Err(format!("Gemini cache create failed with status {}", status));
    }

    let cache_name = body
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| "Gemini cache create response did not include a cache name".to_string())?;

    Ok(cache_name)
}
