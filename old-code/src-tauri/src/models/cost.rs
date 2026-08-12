use crate::models::pricing::{
    calculate_openrouter_request_cost as calc_openrouter_cost_internal,
    calculate_request_cost as calc_cost_internal,
    fetch_openrouter_generation_details as fetch_openrouter_generation_internal,
    fetch_openrouter_model_pricing as fetch_openrouter_pricing_internal,
    fetch_openrouter_provider_pricings as fetch_openrouter_provider_pricings_internal,
    find_openrouter_provider_pricing as find_openrouter_provider_pricing_internal,
    get_model_pricing as get_model_pricing_internal, OpenRouterGenerationDetails,
    OpenRouterProviderPricing,
};
use crate::models::{ModelPricing, RequestCost};
use tauri::AppHandle;

pub use crate::models::pricing::OpenRouterCostInput;

pub fn calculate_request_cost(
    prompt_tokens: u64,
    completion_tokens: u64,
    pricing: &ModelPricing,
) -> Option<RequestCost> {
    calc_cost_internal(prompt_tokens, completion_tokens, pricing)
}

pub fn calculate_openrouter_request_cost(
    input: &OpenRouterCostInput,
    pricing: &ModelPricing,
) -> Option<RequestCost> {
    calc_openrouter_cost_internal(input, pricing)
}

pub async fn fetch_openrouter_model_pricing(
    app: AppHandle,
    api_key: &str,
    model_id: &str,
) -> Result<Option<ModelPricing>, String> {
    fetch_openrouter_pricing_internal(app, api_key, model_id).await
}

pub async fn fetch_openrouter_provider_pricings(
    app: AppHandle,
    api_key: &str,
    model_id: &str,
) -> Result<Vec<OpenRouterProviderPricing>, String> {
    fetch_openrouter_provider_pricings_internal(app, api_key, model_id).await
}

pub async fn fetch_openrouter_generation_details(
    app: AppHandle,
    api_key: &str,
    generation_id: &str,
) -> Result<Option<OpenRouterGenerationDetails>, String> {
    fetch_openrouter_generation_internal(app, api_key, generation_id).await
}

pub fn find_openrouter_provider_pricing<'a>(
    provider_pricings: &'a [OpenRouterProviderPricing],
    provider_name: &str,
) -> Option<&'a OpenRouterProviderPricing> {
    find_openrouter_provider_pricing_internal(provider_pricings, provider_name)
}

pub async fn get_model_pricing(
    app: AppHandle,
    provider_id: &str,
    model_id: &str,
    api_key: Option<&str>,
) -> Result<Option<ModelPricing>, String> {
    get_model_pricing_internal(app, provider_id, model_id, api_key).await
}
