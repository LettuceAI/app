use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

use super::types::*;
use crate::utils;

/// Build an HTTP client with a reasonable timeout.
fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| utils::err_to_string(module_path!(), line!(), e))
}

/// HTTP client with a long timeout for LLM-backed operations (e.g. character boost).
fn http_client_long() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| utils::err_to_string(module_path!(), line!(), e))
}

/// Build headers for Engine requests.
fn engine_headers(api_key: &str) -> Result<reqwest::header::HeaderMap, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|e| utils::err_to_string(module_path!(), line!(), e))?,
    );
    if !api_key.is_empty() {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", api_key)
                .parse()
                .map_err(|e| utils::err_to_string(module_path!(), line!(), e))?,
        );
    }
    Ok(headers)
}

fn trim_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

// ── Health ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn engine_health(
    base_url: String,
    api_key: Option<String>,
) -> Result<HealthResponse, String> {
    let client = http_client()?;
    let url = format!("{}/health", trim_url(&base_url));
    let mut req = client.get(&url);
    if let Some(key) = &api_key {
        if !key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Failed to reach Engine: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("Engine health check failed (HTTP {})", status.as_u16()));
    }
    resp.json::<HealthResponse>()
        .await
        .map_err(|e| format!("Failed to parse health response: {}", e))
}

// ── Setup ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn engine_setup_status(
    base_url: String,
    api_key: Option<String>,
) -> Result<SetupStatusResponse, String> {
    let client = http_client()?;
    let url = format!("{}/setup/status", trim_url(&base_url));
    let mut req = client.get(&url);
    if let Some(key) = &api_key {
        if !key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
    }
    let resp = req.send().await.map_err(|e| format!("Engine unreachable: {}", e))?;
    resp.json::<SetupStatusResponse>()
        .await
        .map_err(|e| format!("Failed to parse setup status: {}", e))
}

#[tauri::command]
pub async fn engine_setup_complete(
    base_url: String,
    api_key: Option<String>,
) -> Result<SetupCompleteResponse, String> {
    let client = http_client()?;
    let url = format!("{}/setup/complete", trim_url(&base_url));
    let headers = engine_headers(api_key.as_deref().unwrap_or(""))?;
    let resp = client
        .post(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    resp.json::<SetupCompleteResponse>()
        .await
        .map_err(|e| format!("Failed to parse setup/complete response: {}", e))
}

// ── Config: LLM ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn engine_config_llm(
    base_url: String,
    api_key: String,
    provider: String,
    config: ConfigLlmRequest,
) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}/config/llm/{}", trim_url(&base_url), provider);
    let headers = engine_headers(&api_key)?;
    let resp = client
        .put(&url)
        .headers(headers)
        .json(&config)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = body
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Config LLM failed ({}): {}", status.as_u16(), detail));
    }
    Ok(body)
}

#[tauri::command]
pub async fn engine_config_llm_default(
    base_url: String,
    api_key: String,
    config: ConfigLlmDefaultRequest,
) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}/config/llm/default", trim_url(&base_url));
    let headers = engine_headers(&api_key)?;
    let resp = client
        .put(&url)
        .headers(headers)
        .json(&config)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = body
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!(
            "Config LLM default failed ({}): {}",
            status.as_u16(),
            detail
        ));
    }
    Ok(body)
}

// ── Config: Engine / Background / Memory ────────────────────────────────────

#[tauri::command]
pub async fn engine_config_engine(
    base_url: String,
    api_key: String,
    config: ConfigEngineRequest,
) -> Result<Value, String> {
    engine_put_config(&base_url, &api_key, "/config/engine", &config).await
}

#[tauri::command]
pub async fn engine_config_background(
    base_url: String,
    api_key: String,
    config: ConfigBackgroundRequest,
) -> Result<Value, String> {
    engine_put_config(&base_url, &api_key, "/config/background", &config).await
}

#[tauri::command]
pub async fn engine_config_memory(
    base_url: String,
    api_key: String,
    config: ConfigMemoryRequest,
) -> Result<Value, String> {
    engine_put_config(&base_url, &api_key, "/config/memory", &config).await
}

#[tauri::command]
pub async fn engine_config_safety(
    base_url: String,
    api_key: String,
    config: ConfigSafetyRequest,
) -> Result<Value, String> {
    engine_put_config(&base_url, &api_key, "/config/safety", &config).await
}

#[tauri::command]
pub async fn engine_config_research(
    base_url: String,
    api_key: String,
    config: ConfigResearchRequest,
) -> Result<Value, String> {
    engine_put_config(&base_url, &api_key, "/config/research", &config).await
}

#[tauri::command]
pub async fn engine_config_llm_delete(
    base_url: String,
    api_key: String,
    provider: String,
) -> Result<Value, String> {
    engine_delete(&base_url, &api_key, &format!("/config/llm/{}", provider)).await
}

async fn engine_put_config<T: serde::Serialize>(
    base_url: &str,
    api_key: &str,
    path: &str,
    body: &T,
) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}{}", trim_url(base_url), path);
    let headers = engine_headers(api_key)?;
    let resp = client
        .put(&url)
        .headers(headers)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let data = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = data
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Config update failed ({}): {}", status.as_u16(), detail));
    }
    Ok(data)
}

// ── Status & Usage ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn engine_status(base_url: String, api_key: String) -> Result<Value, String> {
    engine_get(&base_url, &api_key, "/status").await
}

#[tauri::command]
pub async fn engine_usage(base_url: String, api_key: String) -> Result<Value, String> {
    engine_get(&base_url, &api_key, "/usage").await
}

#[tauri::command]
pub async fn engine_get_config(base_url: String, api_key: String) -> Result<Value, String> {
    engine_get(&base_url, &api_key, "/config").await
}

// ── Characters ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn engine_characters_list(
    base_url: String,
    api_key: String,
) -> Result<Value, String> {
    engine_get(&base_url, &api_key, "/characters").await
}

#[tauri::command]
pub async fn engine_character_load(
    base_url: String,
    api_key: String,
    slug: String,
) -> Result<Value, String> {
    engine_post(&base_url, &api_key, &format!("/characters/{}/load", slug), &Value::Null).await
}

#[tauri::command]
pub async fn engine_character_unload(
    base_url: String,
    api_key: String,
    slug: String,
) -> Result<Value, String> {
    engine_post(&base_url, &api_key, &format!("/characters/{}/unload", slug), &Value::Null).await
}

#[tauri::command]
pub async fn engine_character_activity(
    base_url: String,
    api_key: String,
    slug: String,
) -> Result<Value, String> {
    engine_get(&base_url, &api_key, &format!("/characters/{}/activity", slug)).await
}

#[tauri::command]
pub async fn engine_character_template(
    base_url: String,
    api_key: String,
) -> Result<Value, String> {
    engine_get(&base_url, &api_key, "/characters/template").await
}

#[tauri::command]
pub async fn engine_character_boost(
    base_url: String,
    api_key: String,
    body: Value,
) -> Result<Value, String> {
    engine_post_long(&base_url, &api_key, "/characters/boost", &body).await
}

#[tauri::command]
pub async fn engine_character_create(
    base_url: String,
    api_key: String,
    body: Value,
) -> Result<Value, String> {
    engine_post(&base_url, &api_key, "/characters", &body).await
}

#[tauri::command]
pub async fn engine_character_full(
    base_url: String,
    api_key: String,
    slug: String,
) -> Result<Value, String> {
    engine_get(&base_url, &api_key, &format!("/characters/{}/full", slug)).await
}

#[tauri::command]
pub async fn engine_character_update(
    base_url: String,
    api_key: String,
    slug: String,
    body: Value,
) -> Result<Value, String> {
    engine_put(&base_url, &api_key, &format!("/characters/{}", slug), &body).await
}

#[tauri::command]
pub async fn engine_character_delete_cmd(
    base_url: String,
    api_key: String,
    slug: String,
) -> Result<Value, String> {
    engine_delete(&base_url, &api_key, &format!("/characters/{}", slug)).await
}

// ── Chat ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn engine_chat(
    base_url: String,
    api_key: String,
    slug: String,
    body: Value,
) -> Result<Value, String> {
    engine_post_long(&base_url, &api_key, &format!("/characters/{}/chat", slug), &body).await
}

#[tauri::command]
pub async fn engine_chat_history(
    base_url: String,
    api_key: String,
    slug: String,
    user_id: String,
    limit: Option<u32>,
) -> Result<Value, String> {
    let path = match limit {
        Some(n) => format!("/characters/{}/history/{}?limit={}", slug, user_id, n),
        None => format!("/characters/{}/history/{}", slug, user_id),
    };
    engine_get(&base_url, &api_key, &path).await
}

// ── Helpers ─────────────────────────────────────────────────────────────────

async fn engine_get(base_url: &str, api_key: &str, path: &str) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}{}", trim_url(base_url), path);
    let headers = engine_headers(api_key)?;
    let resp = client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let data = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = data
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Engine request failed ({}): {}", status.as_u16(), detail));
    }
    Ok(data)
}

async fn engine_post(
    base_url: &str,
    api_key: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    engine_post_with_client(http_client()?, base_url, api_key, path, body).await
}

async fn engine_post_long(
    base_url: &str,
    api_key: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    engine_post_with_client(http_client_long()?, base_url, api_key, path, body).await
}

async fn engine_post_with_client(
    client: Client,
    base_url: &str,
    api_key: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{}{}", trim_url(base_url), path);
    let headers = engine_headers(api_key)?;
    let resp = client
        .post(&url)
        .headers(headers)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let data = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = data
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Engine request failed ({}): {}", status.as_u16(), detail));
    }
    Ok(data)
}

async fn engine_put(
    base_url: &str,
    api_key: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}{}", trim_url(base_url), path);
    let headers = engine_headers(api_key)?;
    let resp = client
        .put(&url)
        .headers(headers)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let data = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = data
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Engine request failed ({}): {}", status.as_u16(), detail));
    }
    Ok(data)
}

async fn engine_delete(
    base_url: &str,
    api_key: &str,
    path: &str,
) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}{}", trim_url(base_url), path);
    let headers = engine_headers(api_key)?;
    let resp = client
        .delete(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Engine unreachable: {}", e))?;
    let status = resp.status();
    let data = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = data
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("Engine request failed ({}): {}", status.as_u16(), detail));
    }
    Ok(data)
}
