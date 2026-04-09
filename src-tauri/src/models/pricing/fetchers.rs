use crate::api::{api_request, ApiRequest};
use crate::models::ModelPricing;
use crate::pricing_cache;
use crate::utils::{log_error, log_info, log_warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterProviderPricing {
    pub provider_name: String,
    pub provider_display_name: Option<String>,
    pub pricing: ModelPricing,
}

#[derive(Debug, Clone, Default)]
pub struct OpenRouterGenerationDetails {
    pub generation_id: String,
    pub provider_name: Option<String>,
    pub native_prompt_tokens: Option<u64>,
    pub native_completion_tokens: Option<u64>,
    pub total_cost: Option<f64>,
}

fn parse_model_pricing(pricing_obj: &Value) -> Option<ModelPricing> {
    Some(ModelPricing {
        prompt: extract_pricing_value(pricing_obj.get("prompt"))?,
        completion: extract_pricing_value(pricing_obj.get("completion"))?,
        request: extract_pricing_value(pricing_obj.get("request")).unwrap_or_else(|| "0".into()),
        image: extract_pricing_value(pricing_obj.get("image")).unwrap_or_else(|| "0".into()),
        image_output: extract_pricing_value(pricing_obj.get("image_output"))
            .unwrap_or_else(|| "0".into()),
        web_search: extract_pricing_value(pricing_obj.get("web_search"))
            .unwrap_or_else(|| "0".into()),
        internal_reasoning: extract_pricing_value(pricing_obj.get("internal_reasoning"))
            .unwrap_or_else(|| "0".into()),
        input_cache_read: extract_pricing_value(pricing_obj.get("input_cache_read"))
            .unwrap_or_else(|| "0".into()),
        input_cache_write: extract_pricing_value(pricing_obj.get("input_cache_write"))
            .unwrap_or_else(|| "0".into()),
    })
}

fn build_openrouter_auth_headers(api_key: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    if !api_key.trim().is_empty() {
        headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
    }
    headers
}

fn parse_provider_pricings(endpoints: &[Value]) -> Vec<OpenRouterProviderPricing> {
    let mut out = Vec::new();
    for endpoint in endpoints {
        let Some(pricing_obj) = endpoint.get("pricing") else {
            continue;
        };
        let Some(provider_name) = endpoint.get("provider_name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(pricing) = parse_model_pricing(pricing_obj) else {
            continue;
        };

        out.push(OpenRouterProviderPricing {
            provider_name: provider_name.to_string(),
            provider_display_name: endpoint
                .get("provider_display_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            pricing,
        });
    }
    out
}

fn is_retryable_pricing_failure(status: Option<u16>, err: Option<&str>) -> bool {
    if status == Some(408) {
        return true;
    }

    err.map(|message| {
        let lower = message.to_ascii_lowercase();
        lower.contains("408")
            || lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("deadline has elapsed")
    })
    .unwrap_or(false)
}

pub async fn refresh_openrouter_endpoint_caches(
    app: AppHandle,
    api_key: &str,
    model_id: &str,
) -> Result<(Option<ModelPricing>, Vec<OpenRouterProviderPricing>), String> {
    let request = ApiRequest {
        url: format!("https://openrouter.ai/api/v1/models/{}/endpoints", model_id),
        method: Some("GET".to_string()),
        headers: Some(build_openrouter_auth_headers(api_key)),
        query: None,
        body: None,
        timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
        stream: None,
        request_id: None,
        provider_id: Some("openrouter".to_string()),
    };

    let response = api_request(app.clone(), request).await?;
    if !response.ok {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "OpenRouter endpoint pricing request failed with status {}",
                response.status
            ),
        ));
    }

    let endpoints = response
        .data
        .get("data")
        .and_then(|d| d.get("endpoints"))
        .and_then(|e| e.as_array())
        .ok_or_else(|| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                "OpenRouter endpoint response missing endpoints array",
            )
        })?;

    let provider_pricings = parse_provider_pricings(endpoints);
    let cheapest_pricing = get_cheapest_endpoint_pricing(endpoints);

    pricing_cache::cache_openrouter_provider_pricings(&app, model_id, &provider_pricings)?;
    pricing_cache::cache_model_pricing(&app, model_id, cheapest_pricing.clone())?;
    let _ = pricing_cache::clear_openrouter_endpoints_refresh(&app, model_id);

    Ok((cheapest_pricing, provider_pricings))
}

pub async fn fetch_openrouter_provider_pricings(
    app: AppHandle,
    api_key: &str,
    model_id: &str,
) -> Result<Vec<OpenRouterProviderPricing>, String> {
    if let Ok(Some(cached)) = pricing_cache::get_cached_openrouter_provider_pricings(&app, model_id)
    {
        log_info(
            &app,
            "cost_calculator",
            format!("Using cached OpenRouter provider pricing for {}", model_id),
        );
        return Ok(cached);
    }

    match refresh_openrouter_endpoint_caches(app.clone(), api_key, model_id).await {
        Ok((_, provider_pricings)) => Ok(provider_pricings),
        Err(err) => {
            if is_retryable_pricing_failure(None, Some(&err)) {
                let _ = pricing_cache::schedule_openrouter_endpoints_refresh(&app, model_id, &err);

                if let Ok(Some(cached)) =
                    pricing_cache::get_any_cached_openrouter_provider_pricings(&app, model_id)
                {
                    log_warn(
                        &app,
                        "cost_calculator",
                        format!(
                            "OpenRouter provider pricing refresh failed for {}; using stale cache: {}",
                            model_id, err
                        ),
                    );
                    return Ok(cached);
                }

                log_warn(
                    &app,
                    "cost_calculator",
                    format!(
                        "OpenRouter provider pricing refresh failed for {}; deferring retry: {}",
                        model_id, err
                    ),
                );
                return Ok(Vec::new());
            }

            Err(err)
        }
    }
}

pub async fn fetch_openrouter_generation_details(
    app: AppHandle,
    api_key: &str,
    generation_id: &str,
) -> Result<Option<OpenRouterGenerationDetails>, String> {
    let request = ApiRequest {
        url: "https://openrouter.ai/api/v1/generation".to_string(),
        method: Some("GET".to_string()),
        headers: Some(build_openrouter_auth_headers(api_key)),
        query: Some({
            let mut query = HashMap::new();
            query.insert("id".to_string(), Value::String(generation_id.to_string()));
            query
        }),
        body: None,
        timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
        stream: None,
        request_id: None,
        provider_id: Some("openrouter".to_string()),
    };

    let response = api_request(app.clone(), request).await?;
    if !response.ok {
        log_warn(
            &app,
            "cost_calculator",
            format!(
                "OpenRouter generation lookup failed for {} with status {}",
                generation_id, response.status
            ),
        );
        return Ok(None);
    }

    let root = response.data.get("data").unwrap_or(&response.data);

    Ok(Some(OpenRouterGenerationDetails {
        generation_id: generation_id.to_string(),
        provider_name: root
            .get("provider_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        native_prompt_tokens: root
            .get("native_tokens_prompt")
            .and_then(|v| v.as_u64())
            .or_else(|| root.get("tokens_prompt").and_then(|v| v.as_u64())),
        native_completion_tokens: root
            .get("native_tokens_completion")
            .and_then(|v| v.as_u64())
            .or_else(|| root.get("tokens_completion").and_then(|v| v.as_u64())),
        total_cost: root
            .get("total_cost")
            .and_then(|v| v.as_f64())
            .or_else(|| root.get("cost").and_then(|v| v.as_f64())),
    }))
}

fn normalize_provider_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn find_openrouter_provider_pricing<'a>(
    provider_pricings: &'a [OpenRouterProviderPricing],
    provider_name: &str,
) -> Option<&'a OpenRouterProviderPricing> {
    let normalized_target = normalize_provider_name(provider_name);

    provider_pricings.iter().find(|entry| {
        normalize_provider_name(&entry.provider_name) == normalized_target
            || entry
                .provider_display_name
                .as_deref()
                .map(normalize_provider_name)
                .as_deref()
                == Some(normalized_target.as_str())
    })
}

/// Fetch pricing for OpenRouter models using the endpoints endpoint
pub async fn fetch_openrouter_model_pricing(
    app: AppHandle,
    api_key: &str,
    model_id: &str,
) -> Result<Option<ModelPricing>, String> {
    if model_id.contains(":free") {
        log_warn(
            &app,
            "cost_calculator",
            format!("Skipping free model: {}", model_id),
        );
        return Ok(None);
    }

    if let Ok(Some(cached)) = pricing_cache::get_cached_pricing(&app, model_id) {
        log_info(
            &app,
            "cost_calculator",
            format!("Using cached pricing for {}", model_id),
        );
        return Ok(Some(cached));
    }

    log_info(
        &app,
        "cost_calculator",
        format!("Fetching pricing for OpenRouter model: {}", model_id),
    );

    match refresh_openrouter_endpoint_caches(app.clone(), api_key, model_id).await {
        Ok((pricing, _)) => {
            if let Some(ref pricing) = pricing {
                log_info(
                    &app,
                    "cost_calculator",
                    format!(
                        "Found pricing for {}: prompt={} completion={}",
                        model_id, pricing.prompt, pricing.completion
                    ),
                );
            } else {
                log_warn(
                    &app,
                    "cost_calculator",
                    format!(
                        "No pricing found for model {} in OpenRouter API response",
                        model_id
                    ),
                );
            }
            Ok(pricing)
        }
        Err(err) => {
            if is_retryable_pricing_failure(None, Some(&err)) {
                let _ = pricing_cache::schedule_openrouter_endpoints_refresh(&app, model_id, &err);

                if let Ok(Some(cached)) = pricing_cache::get_any_cached_pricing(&app, model_id) {
                    log_warn(
                        &app,
                        "cost_calculator",
                        format!(
                            "OpenRouter model pricing refresh failed for {}; using stale cache: {}",
                            model_id, err
                        ),
                    );
                    return Ok(Some(cached));
                }

                log_warn(
                    &app,
                    "cost_calculator",
                    format!(
                        "OpenRouter model pricing refresh failed for {}; deferring retry: {}",
                        model_id, err
                    ),
                );
                return Ok(None);
            }

            log_error(
                &app,
                "cost_calculator",
                format!("Failed to fetch OpenRouter pricing: {}", err),
            );
            Err(err)
        }
    }
}

/// Extract cheapest endpoint pricing from endpoints array
/// Prioritizes: lowest cost > highest uptime > first available
fn get_cheapest_endpoint_pricing(endpoints: &[serde_json::Value]) -> Option<ModelPricing> {
    let mut best_endpoint: Option<(f64, f64, &serde_json::Value)> = None;

    for endpoint in endpoints {
        if let Some(pricing_obj) = endpoint.get("pricing") {
            let prompt_str = extract_pricing_value(pricing_obj.get("prompt"));
            let completion_str = extract_pricing_value(pricing_obj.get("completion"));

            if let (Some(prompt_str), Some(completion_str)) = (prompt_str, completion_str) {
                if let (Ok(prompt_price), Ok(completion_price)) =
                    (prompt_str.parse::<f64>(), completion_str.parse::<f64>())
                {
                    let total_price = prompt_price + completion_price;

                    let is_better = match &best_endpoint {
                        None => true,
                        Some((best_total, _, _)) => total_price < *best_total * 0.99,
                    };

                    if is_better {
                        best_endpoint = Some((total_price, prompt_price, endpoint));
                    }
                }
            }
        }
    }

    // Extract pricing from best endpoint
    best_endpoint.and_then(|(_, _, endpoint)| {
        if let Some(pricing_obj) = endpoint.get("pricing") {
            parse_model_pricing(pricing_obj)
        } else {
            None
        }
    })
}

fn extract_pricing_value(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|v: &serde_json::Value| {
        if let Some(s) = v.as_str() {
            return Some(s.to_string());
        }
        if let Some(n) = v.as_f64() {
            return Some(n.to_string());
        }
        if let Some(n) = v.as_i64() {
            return Some(n.to_string());
        }
        None
    })
}

/// Get pricing for a model (with fallback for non-OpenRouter)
pub async fn get_model_pricing(
    app: AppHandle,
    provider_id: &str,
    model_id: &str,
    api_key: Option<&str>,
) -> Result<Option<ModelPricing>, String> {
    match provider_id {
        "openrouter" => {
            if let Some(key) = api_key {
                fetch_openrouter_model_pricing(app, key, model_id).await
            } else {
                log_error(
                    &app,
                    "cost_calculator",
                    "No API key for OpenRouter pricing lookup",
                );
                Ok(None)
            }
        }
        _ => {
            log_warn(
                &app,
                "cost_calculator",
                format!("Pricing not supported for provider: {}", provider_id),
            );
            Ok(None)
        }
    }
}
