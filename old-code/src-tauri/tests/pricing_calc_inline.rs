//! Gathered from inline tests in src/models/pricing/calc.rs.

use lettuceai_lib::models::pricing::calc::{
    calculate_openrouter_request_cost, calculate_request_cost, OpenRouterCostInput,
};
use lettuceai_lib::models::ModelPricing;

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
    let expected_completion = 450.0 * 0.000015;
    let expected_reasoning = 50.0 * 0.00002;
    let expected_request = 0.002;
    let expected_web_search = 2.0 * 0.01;
    let expected_total = expected_prompt
        + expected_completion
        + expected_reasoning
        + expected_request
        + expected_web_search;

    assert!((cost.prompt_cost - expected_prompt).abs() < 1e-9);
    assert!((cost.completion_cost - expected_completion).abs() < 1e-9);
    assert!((cost.reasoning_cost - expected_reasoning).abs() < 1e-9);
    assert!((cost.request_cost - expected_request).abs() < 1e-9);
    assert!((cost.web_search_cost - expected_web_search).abs() < 1e-9);
    assert!((cost.total_cost - expected_total).abs() < 1e-9);
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

#[test]
fn test_authoritative_total_cannot_make_completion_negative() {
    let cost = calculate_openrouter_request_cost(
        &OpenRouterCostInput {
            prompt_tokens: 17_898,
            completion_tokens: 16_113,
            reasoning_tokens: 1_166,
            web_search_requests: 1,
            authoritative_total_cost: Some(0.0),
            ..Default::default()
        },
        &ModelPricing {
            prompt: "0.000002".to_string(),
            completion: "0.000012".to_string(),
            request: "0".to_string(),
            image: "0".to_string(),
            image_output: "0".to_string(),
            web_search: "0.014".to_string(),
            internal_reasoning: "0.000012".to_string(),
            input_cache_read: "0.0000002".to_string(),
            input_cache_write: "0.000000375".to_string(),
        },
    )
    .unwrap();

    assert!(cost.completion_cost >= 0.0);
    assert!((cost.prompt_cost - 0.035796).abs() < 1e-9);
    assert!((cost.reasoning_cost - 0.013992).abs() < 1e-9);
    assert!((cost.web_search_cost - 0.014).abs() < 1e-9);
    assert!((cost.total_cost - 0.243152).abs() < 1e-9);
}
