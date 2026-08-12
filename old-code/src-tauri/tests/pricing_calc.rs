//! Additional integration tests for cost calculation.

use lettuceai_lib::models::pricing::calc::{
    calculate_openrouter_request_cost, calculate_request_cost, OpenRouterCostInput,
};
use lettuceai_lib::models::ModelPricing;

fn pricing(prompt: &str, completion: &str) -> ModelPricing {
    ModelPricing {
        prompt: prompt.into(),
        completion: completion.into(),
        request: String::new(),
        image: String::new(),
        image_output: String::new(),
        web_search: String::new(),
        internal_reasoning: String::new(),
        input_cache_read: String::new(),
        input_cache_write: String::new(),
    }
}

#[test]
fn basic_cost_calculation() {
    let p = pricing("0.000001", "0.000002");
    let cost = calculate_request_cost(1000, 500, &p).expect("ok");
    assert!((cost.prompt_cost - 0.001).abs() < 1e-9);
    assert!((cost.completion_cost - 0.001).abs() < 1e-9);
    assert!((cost.total_cost - 0.002).abs() < 1e-9);
    assert_eq!(cost.total_tokens, 1500);
}

#[test]
fn zero_tokens_yields_zero_cost() {
    let p = pricing("0.000001", "0.000002");
    let cost = calculate_request_cost(0, 0, &p).expect("ok");
    assert_eq!(cost.prompt_cost, 0.0);
    assert_eq!(cost.completion_cost, 0.0);
    assert_eq!(cost.total_cost, 0.0);
}

#[test]
fn invalid_prompt_price_returns_none() {
    let p = pricing("invalid", "0.000001");
    assert!(calculate_request_cost(100, 100, &p).is_none());
}

#[test]
fn invalid_completion_price_returns_none() {
    let p = pricing("0.000001", "not_a_number");
    assert!(calculate_request_cost(100, 100, &p).is_none());
}

#[test]
fn cache_read_discount_applies() {
    let mut p = pricing("0.00001", "0.00001");
    p.input_cache_read = "0.000001".into();
    let input = OpenRouterCostInput {
        prompt_tokens: 1000,
        completion_tokens: 0,
        cached_prompt_tokens: 500,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        web_search_requests: 0,
        authoritative_total_cost: None,
    };
    let cost = calculate_openrouter_request_cost(&input, &p).expect("ok");
    assert!(cost.cache_read_cost > 0.0);
    // 500 cached at $0.000001 = $0.0005
    assert!((cost.cache_read_cost - 0.0005).abs() < 1e-9);
}

#[test]
fn cached_tokens_clamped_to_prompt_tokens() {
    let mut p = pricing("0.00001", "0.00001");
    p.input_cache_read = "0.000001".into();
    let input = OpenRouterCostInput {
        prompt_tokens: 100,
        cached_prompt_tokens: 9999,
        completion_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        web_search_requests: 0,
        authoritative_total_cost: None,
    };
    let cost = calculate_openrouter_request_cost(&input, &p).expect("ok");
    assert!(cost.cached_prompt_tokens <= 100);
}

#[test]
fn reasoning_tokens_billed() {
    let mut p = pricing("0.00001", "0.00001");
    p.internal_reasoning = "0.000005".into();
    let input = OpenRouterCostInput {
        prompt_tokens: 0,
        completion_tokens: 500,
        cached_prompt_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 200,
        web_search_requests: 0,
        authoritative_total_cost: None,
    };
    let cost = calculate_openrouter_request_cost(&input, &p).expect("ok");
    assert!(cost.reasoning_cost > 0.0);
    assert!((cost.completion_cost - 0.003).abs() < 1e-9);
    assert!((cost.total_cost - 0.004).abs() < 1e-9);
}

#[test]
fn request_fee_added() {
    let mut p = pricing("0.0", "0.0");
    p.request = "0.001".into();
    let input = OpenRouterCostInput {
        prompt_tokens: 0,
        completion_tokens: 0,
        ..Default::default()
    };
    let cost = calculate_openrouter_request_cost(&input, &p).expect("ok");
    assert!((cost.request_cost - 0.001).abs() < 1e-9);
}

#[test]
fn authoritative_total_overrides_calculated() {
    let p = pricing("0.000001", "0.000002");
    let input = OpenRouterCostInput {
        prompt_tokens: 100,
        completion_tokens: 100,
        cached_prompt_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        web_search_requests: 0,
        authoritative_total_cost: Some(0.5),
    };
    let cost = calculate_openrouter_request_cost(&input, &p).expect("ok");
    assert!((cost.total_cost - 0.5).abs() < 1e-9);
    assert!(cost.completion_cost >= 0.0);
}

#[test]
fn authoritative_total_below_known_costs_does_not_make_completion_negative() {
    let mut p = pricing("0.000002", "0.000012");
    p.internal_reasoning = "0.000012".into();
    p.web_search = "0.014".into();

    let input = OpenRouterCostInput {
        prompt_tokens: 17_898,
        completion_tokens: 16_113,
        reasoning_tokens: 1_166,
        web_search_requests: 1,
        authoritative_total_cost: Some(0.0),
        ..Default::default()
    };

    let cost = calculate_openrouter_request_cost(&input, &p).expect("ok");

    assert!(cost.completion_cost >= 0.0);
    assert!((cost.prompt_cost - 0.035796).abs() < 1e-9);
    assert!((cost.reasoning_cost - 0.013992).abs() < 1e-9);
    assert!((cost.web_search_cost - 0.014).abs() < 1e-9);
    assert!((cost.total_cost - 0.243152).abs() < 1e-9);
}

#[test]
fn negative_authoritative_total_handled() {
    let p = pricing("0.000001", "0.000002");
    let input = OpenRouterCostInput {
        prompt_tokens: 100,
        completion_tokens: 100,
        authoritative_total_cost: Some(-0.5),
        ..Default::default()
    };
    let _ = calculate_openrouter_request_cost(&input, &p);
}

#[test]
fn empty_price_strings_treated_as_zero() {
    let p = pricing("0", "0");
    let cost = calculate_request_cost(1000, 1000, &p).expect("ok");
    assert_eq!(cost.prompt_cost, 0.0);
    assert_eq!(cost.completion_cost, 0.0);
    assert_eq!(cost.total_cost, 0.0);
}

#[test]
fn whitespace_in_prompt_price_rejected() {
    let p = pricing("  0.000001  ", "0.000002");
    let cost = calculate_request_cost(1000, 1000, &p);
    assert!(
        cost.is_none(),
        "untrimmed prompt price should fail to parse"
    );
}
