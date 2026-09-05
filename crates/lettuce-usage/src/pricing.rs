/// Pricing information for a model (values are USD costs expressed as strings).
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Price per input token in USD.
    pub prompt: String,
    /// Price per output token in USD.
    pub completion: String,
    /// Flat price per request in USD.
    pub request: String,
    /// Price per image-related unit in USD.
    pub image: String,
    /// Price per output image-related unit in USD.
    pub image_output: String,
    /// Price per web search
    pub web_search: String,
    /// Price per internal reasoning token
    pub internal_reasoning: String,
    /// Price per cached prompt token read in USD.
    pub input_cache_read: String,
    /// Price per cached prompt token write in USD.
    pub input_cache_write: String,
}

/// Cost calculation result for a single request.
#[derive(Debug, Clone)]
pub struct RequestCost {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub regular_prompt_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    /// Cost for prompt tokens
    pub prompt_cost: f64,
    /// Prompt cost before cache discounts/writes.
    pub prompt_base_cost: f64,
    /// Cost for reading cached prompt tokens.
    pub cache_read_cost: f64,
    /// Cost for writing cacheable prompt tokens.
    pub cache_write_cost: f64,
    /// Cost for completion tokens
    pub completion_cost: f64,
    /// Visible completion token cost before reasoning/request/search charges.
    pub completion_base_cost: f64,
    /// Cost of billed reasoning tokens.
    pub reasoning_cost: f64,
    /// Flat per-request charge.
    pub request_cost: f64,
    /// Cost from web-search tool calls.
    pub web_search_cost: f64,
    /// Total cost in USD
    pub total_cost: f64,
    /// Provider-reported total, when available.
    pub authoritative_total_cost: Option<f64>,
}

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
        .filter(|v| v.is_finite() && *v >= 0.0)
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
    if !prompt_price_per_token.is_finite()
        || prompt_price_per_token < 0.0
        || !completion_price_per_token.is_finite()
        || completion_price_per_token < 0.0
    {
        return None;
    }
    let total_tokens = input.prompt_tokens.checked_add(input.completion_tokens)?;

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
    let mut completion_cost = completion_base_cost;

    let mut total_cost =
        prompt_cost + completion_cost + reasoning_cost + request_cost + web_search_cost;
    if !total_cost.is_finite() {
        return None;
    }

    if let Some(authoritative_total_cost) = input
        .authoritative_total_cost
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        let non_completion_cost = prompt_cost + reasoning_cost + request_cost + web_search_cost;
        if authoritative_total_cost + 1e-12 >= non_completion_cost {
            completion_cost = (authoritative_total_cost - non_completion_cost).max(0.0);
            total_cost = authoritative_total_cost;
        }
    }

    Some(RequestCost {
        prompt_tokens: input.prompt_tokens,
        completion_tokens: input.completion_tokens,
        total_tokens,
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

    fn pricing() -> ModelPricing {
        ModelPricing {
            prompt: "0.01".into(),
            completion: "0.02".into(),
            request: "0.1".into(),
            image: String::new(),
            image_output: String::new(),
            web_search: "0.2".into(),
            internal_reasoning: "0.03".into(),
            input_cache_read: "0.001".into(),
            input_cache_write: "0.015".into(),
        }
    }

    fn near(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
    }

    #[test]
    fn legacy_cache_reasoning_and_request_breakdown() {
        let input = OpenRouterCostInput {
            prompt_tokens: 100,
            completion_tokens: 50,
            cached_prompt_tokens: 20,
            cache_write_tokens: 10,
            reasoning_tokens: 5,
            web_search_requests: 2,
            authoritative_total_cost: None,
        };
        let cost = calculate_openrouter_request_cost(&input, &pricing()).expect("cost");
        assert_eq!(cost.total_tokens, 150);
        assert_eq!(cost.regular_prompt_tokens, 70);
        near(cost.prompt_base_cost, 0.7);
        near(cost.cache_read_cost, 0.02);
        near(cost.cache_write_cost, 0.15);
        near(cost.completion_base_cost, 0.9);
        near(cost.reasoning_cost, 0.15);
        near(cost.total_cost, 2.42);
        let adjusted = calculate_openrouter_request_cost(
            &OpenRouterCostInput {
                authoritative_total_cost: Some(3.0),
                ..input.clone()
            },
            &pricing(),
        )
        .expect("authoritative cost");
        near(adjusted.total_cost, 3.0);
        near(adjusted.completion_cost, 1.48);
        for total in [0.1, -1.0, f64::NAN, f64::INFINITY] {
            let ignored = calculate_openrouter_request_cost(
                &OpenRouterCostInput {
                    authoritative_total_cost: Some(total),
                    ..input.clone()
                },
                &pricing(),
            )
            .expect("fallback estimate");
            near(ignored.total_cost, cost.total_cost);
            assert!(ignored.completion_cost >= 0.0);
        }
    }

    #[test]
    fn legacy_counter_clamping_and_cache_write_fallback() {
        let mut prices = pricing();
        prices.input_cache_write = String::new();
        let cost = calculate_openrouter_request_cost(
            &OpenRouterCostInput {
                prompt_tokens: 10,
                completion_tokens: 2,
                cached_prompt_tokens: 4,
                cache_write_tokens: 99,
                reasoning_tokens: 99,
                ..Default::default()
            },
            &prices,
        )
        .expect("clamped cost");
        assert_eq!(cost.regular_prompt_tokens, 0);
        assert_eq!(cost.cache_write_tokens, 6);
        assert_eq!(cost.reasoning_tokens, 2);
        near(cost.cache_write_cost, 0.06);
        near(cost.completion_base_cost, 0.0);
        let simple = calculate_request_cost(10, 2, &prices).expect("simple cost");
        near(simple.total_cost, 0.24);
    }

    #[test]
    fn invalid_prices_and_overflow_do_not_produce_costs() {
        for value in ["invalid", "NaN", "inf", "-0.1"] {
            let mut prices = pricing();
            prices.prompt = value.into();
            assert!(calculate_request_cost(1, 1, &prices).is_none());
        }
        assert!(calculate_request_cost(u64::MAX, 1, &pricing()).is_none());
        let mut prices = pricing();
        prices.prompt = f64::MAX.to_string();
        assert!(calculate_request_cost(2, 0, &prices).is_none());
    }
}
