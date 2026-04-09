use crate::models::{ModelPricing, RequestCost};

#[derive(Debug, Clone, Default)]
pub struct OpenRouterCostInput {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    pub authoritative_total_cost: Option<f64>,
}

fn parse_price_or_zero(raw: &str) -> f64 {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
}

/// Calculate the cost for a request based on token counts and pricing.
///
/// OpenRouter pricing values are per token in USD, not per 1k tokens.
pub fn calculate_request_cost(
    prompt_tokens: u64,
    completion_tokens: u64,
    pricing: &ModelPricing,
) -> Option<RequestCost> {
    calculate_openrouter_request_cost(
        &OpenRouterCostInput {
            prompt_tokens,
            completion_tokens,
            ..Default::default()
        },
        pricing,
    )
}

pub fn calculate_openrouter_request_cost(
    input: &OpenRouterCostInput,
    pricing: &ModelPricing,
) -> Option<RequestCost> {
    let prompt_price_per_token = pricing.prompt.parse::<f64>().ok()?;
    let completion_price_per_token = pricing.completion.parse::<f64>().ok()?;

    let cache_read_price_per_token = parse_price_or_zero(&pricing.input_cache_read);
    let cache_write_price_per_token = {
        let parsed = parse_price_or_zero(&pricing.input_cache_write);
        if parsed > 0.0 {
            parsed
        } else {
            prompt_price_per_token
        }
    };
    let reasoning_price_per_token = parse_price_or_zero(&pricing.internal_reasoning);
    let request_price = parse_price_or_zero(&pricing.request);
    let web_search_price = parse_price_or_zero(&pricing.web_search);

    let cached_prompt_tokens = input.cached_prompt_tokens.min(input.prompt_tokens);
    let cache_write_tokens = input
        .cache_write_tokens
        .min(input.prompt_tokens.saturating_sub(cached_prompt_tokens));
    let regular_prompt_tokens = input
        .prompt_tokens
        .saturating_sub(cached_prompt_tokens + cache_write_tokens);

    let reasoning_tokens = input.reasoning_tokens.min(input.completion_tokens);
    let visible_completion_tokens = input.completion_tokens.saturating_sub(reasoning_tokens);

    let prompt_base_cost = regular_prompt_tokens as f64 * prompt_price_per_token;
    let cache_read_cost = cached_prompt_tokens as f64 * cache_read_price_per_token;
    let cache_write_cost = cache_write_tokens as f64 * cache_write_price_per_token;
    let prompt_cost = prompt_base_cost + cache_read_cost + cache_write_cost;

    let completion_base_cost = visible_completion_tokens as f64 * completion_price_per_token;
    let reasoning_cost = reasoning_tokens as f64 * reasoning_price_per_token;
    let request_cost = request_price;
    let web_search_cost = input.web_search_requests as f64 * web_search_price;
    let mut completion_cost =
        completion_base_cost + reasoning_cost + request_cost + web_search_cost;

    let mut total_cost = prompt_cost + completion_cost;

    if let Some(authoritative_total_cost) = input
        .authoritative_total_cost
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        let delta = authoritative_total_cost - total_cost;
        if delta.abs() > 1e-12 {
            completion_cost += delta;
            total_cost = authoritative_total_cost;
        }
    }

    Some(RequestCost {
        prompt_tokens: input.prompt_tokens,
        completion_tokens: input.completion_tokens,
        total_tokens: input.prompt_tokens + input.completion_tokens,
        regular_prompt_tokens,
        cached_prompt_tokens,
        cache_write_tokens,
        reasoning_tokens,
        web_search_requests: input.web_search_requests,
        prompt_cost,
        prompt_base_cost,
        cache_read_cost,
        cache_write_cost,
        completion_cost,
        completion_base_cost,
        reasoning_cost,
        request_cost,
        web_search_cost,
        total_cost,
        authoritative_total_cost: input.authoritative_total_cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelPricing;

    fn pricing() -> ModelPricing {
        ModelPricing {
            prompt: "0.000003".to_string(),
            completion: "0.000015".to_string(),
            request: "0".to_string(),
            image: "0".to_string(),
            image_output: "0".to_string(),
            web_search: "0".to_string(),
            internal_reasoning: "0".to_string(),
            input_cache_read: "0".to_string(),
            input_cache_write: "0".to_string(),
        }
    }

    #[test]
    fn test_calculate_request_cost_claude_sonnet() {
        let cost = calculate_request_cost(500_000, 178_000, &pricing()).unwrap();

        assert!((cost.prompt_cost - 1.5).abs() < 0.001);
        assert!((cost.completion_cost - 2.67).abs() < 0.001);
        assert!((cost.total_cost - 4.17).abs() < 0.01);
        assert_eq!(cost.total_tokens, 678_000);
    }

    #[test]
    fn test_calculate_request_cost_small_request() {
        let cost = calculate_request_cost(1000, 500, &pricing()).unwrap();

        assert!((cost.prompt_cost - 0.003).abs() < 0.0001);
        assert!((cost.completion_cost - 0.0075).abs() < 0.0001);
        assert!((cost.total_cost - 0.0105).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_request_cost_with_cache_and_reasoning_fees() {
        let cost = calculate_openrouter_request_cost(
            &OpenRouterCostInput {
                prompt_tokens: 1_000,
                completion_tokens: 500,
                cached_prompt_tokens: 200,
                cache_write_tokens: 100,
                reasoning_tokens: 50,
                web_search_requests: 2,
                authoritative_total_cost: None,
            },
            &ModelPricing {
                prompt: "0.000003".to_string(),
                completion: "0.000015".to_string(),
                request: "0.002".to_string(),
                image: "0".to_string(),
                image_output: "0".to_string(),
                web_search: "0.01".to_string(),
                internal_reasoning: "0.00002".to_string(),
                input_cache_read: "0.000001".to_string(),
                input_cache_write: "0.000004".to_string(),
            },
        )
        .unwrap();

        let expected_prompt = (700.0 * 0.000003) + (200.0 * 0.000001) + (100.0 * 0.000004);
        let expected_completion = (450.0 * 0.000015) + (50.0 * 0.00002) + 0.002 + (2.0 * 0.01);

        assert!((cost.prompt_cost - expected_prompt).abs() < 1e-9);
        assert!((cost.completion_cost - expected_completion).abs() < 1e-9);
        assert!((cost.total_cost - (expected_prompt + expected_completion)).abs() < 1e-9);
    }

    #[test]
    fn test_authoritative_total_overrides_estimate() {
        let cost = calculate_openrouter_request_cost(
            &OpenRouterCostInput {
                prompt_tokens: 1000,
                completion_tokens: 500,
                authoritative_total_cost: Some(0.2),
                ..Default::default()
            },
            &pricing(),
        )
        .unwrap();

        assert!((cost.total_cost - 0.2).abs() < 1e-12);
        assert!((cost.prompt_cost + cost.completion_cost - cost.total_cost).abs() < 1e-12);
    }
}
