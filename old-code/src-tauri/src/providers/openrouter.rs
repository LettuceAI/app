use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterEndpoint {
    pub id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub prompt_price: String,
    pub completion_price: String,
    pub context_length: Option<u64>,
    pub uptime_last_30m: Option<f64>,
    pub supports_prompt_caching: bool,
    pub cache_read_price: Option<String>,
    pub cache_write_price: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    pub name: String,
    pub architecture: OpenRouterArchitecture,
    pub input_scopes: Vec<String>,
    pub output_scopes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterArchitecture {
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterApiResponse {
    data: Vec<OpenRouterApiModel>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterApiModel {
    id: String,
    name: String,
    architecture: OpenRouterArchitecture,
}

#[derive(Debug, Deserialize)]
struct OpenRouterEndpointsResponse {
    data: OpenRouterEndpointsData,
}

#[derive(Debug, Deserialize)]
struct OpenRouterEndpointsData {
    endpoints: Vec<OpenRouterApiEndpoint>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterApiEndpoint {
    tag: String,
    provider_name: String,
    pricing: OpenRouterEndpointPricing,
    context_length: Option<u64>,
    uptime_last_30m: Option<f64>,
    #[serde(default)]
    supports_implicit_caching: bool,
}

#[derive(Debug, Deserialize)]
struct OpenRouterEndpointPricing {
    prompt: String,
    completion: String,
    input_cache_read: Option<String>,
    input_cache_write: Option<String>,
}

fn openrouter_provider_logo_url(endpoint_tag: &str, provider_name: &str) -> String {
    let base_tag = endpoint_tag.split('/').next().unwrap_or(endpoint_tag);
    let icon_name = match base_tag {
        "amazon-bedrock" => "Bedrock".to_string(),
        "google-vertex" => "GoogleVertex".to_string(),
        "google-ai-studio" => "GoogleAIStudio".to_string(),
        _ => provider_name.replace(' ', "%20"),
    };
    format!("https://openrouter.ai/images/icons/{icon_name}.svg")
}

#[tauri::command]
pub async fn get_openrouter_endpoints(model_id: String) -> Result<Vec<OpenRouterEndpoint>, String> {
    let model_id = model_id.trim();
    if model_id.is_empty() || !model_id.contains('/') {
        return Err("A valid OpenRouter model ID is required".to_string());
    }

    let response = reqwest::Client::new()
        .get(format!(
            "https://openrouter.ai/api/v1/models/{}/endpoints",
            model_id
        ))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch OpenRouter endpoints: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "OpenRouter endpoint API returned {}",
            response.status()
        ));
    }

    let payload: OpenRouterEndpointsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenRouter endpoints: {e}"))?;

    Ok(payload
        .data
        .endpoints
        .into_iter()
        .map(|endpoint| {
            let logo_url = openrouter_provider_logo_url(&endpoint.tag, &endpoint.provider_name);
            let supports_prompt_caching = endpoint.supports_implicit_caching
                || endpoint.pricing.input_cache_read.is_some()
                || endpoint.pricing.input_cache_write.is_some();
            let cache_read_price = endpoint.pricing.input_cache_read.clone();
            let cache_write_price = endpoint.pricing.input_cache_write.clone();
            OpenRouterEndpoint {
                id: endpoint.tag,
                logo_url: Some(logo_url),
                name: endpoint.provider_name,
                prompt_price: endpoint.pricing.prompt,
                completion_price: endpoint.pricing.completion,
                context_length: endpoint.context_length,
                uptime_last_30m: endpoint.uptime_last_30m,
                supports_prompt_caching,
                cache_read_price,
                cache_write_price,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn get_openrouter_models(_app: AppHandle) -> Result<Vec<OpenRouterModel>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to fetch OpenRouter models: {}", e),
            )
        })?;

    if !response.status().is_success() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("OpenRouter API error: {}", response.status()),
        ));
    }

    let api_response: OpenRouterApiResponse = response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse OpenRouter response: {}", e),
        )
    })?;

    let models = api_response
        .data
        .into_iter()
        .map(|m| {
            let mut input_scopes = m.architecture.input_modalities.clone();
            if input_scopes.is_empty() {
                input_scopes.push("text".to_string());
            }
            let mut output_scopes = m.architecture.output_modalities.clone();
            if output_scopes.is_empty() {
                output_scopes.push("text".to_string());
            }

            OpenRouterModel {
                id: m.id,
                name: m.name,
                architecture: m.architecture,
                input_scopes,
                output_scopes,
            }
        })
        .collect();

    Ok(models)
}
