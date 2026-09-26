//! Token usage in an image response, searched for in any JSON shape.

use lettuce_conversations::{InferenceUsage, ProviderReportedCost};
use serde_json::{Map, Value};

#[derive(Debug, Default, Clone, PartialEq)]
struct UsageSummary {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    cached_prompt_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    image_tokens: Option<u64>,
    audio_tokens: Option<u64>,
    total_tokens: Option<u64>,
    web_search_requests: Option<u64>,
    api_cost: Option<f64>,
}

fn parse_token_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_float_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn take_first(map: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(parse_token_value))
}

fn take_first_f64(map: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(parse_float_value))
}

fn details<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    map.get(key).and_then(Value::as_object)
}

fn modality_token_count(details: Option<&Value>, modality: &str) -> Option<u64> {
    let entries = details?.as_array()?;
    let mut total = 0_u64;
    let mut found = false;
    for entry in entries {
        let entry_modality = entry
            .get("modality")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if entry_modality.eq_ignore_ascii_case(modality)
            && let Some(count) = entry
                .get("tokenCount")
                .or_else(|| entry.get("token_count"))
                .and_then(parse_token_value)
        {
            total += count;
            found = true;
        }
    }
    found.then_some(total)
}

fn usage_from_map(map: &Map<String, Value>) -> Option<UsageSummary> {
    let prompt_tokens = take_first(
        map,
        &[
            "prompt_eval_count",
            "prompt_tokens",
            "input_tokens",
            "promptTokens",
            "inputTokens",
        ],
    );
    let completion_tokens = take_first(
        map,
        &[
            "eval_count",
            "completion_tokens",
            "output_tokens",
            "completionTokens",
            "outputTokens",
        ],
    );
    let reasoning_tokens = take_first(
        map,
        &[
            "reasoning_tokens",
            "reasoningTokens",
            "thinking_tokens",
            "thinkingTokens",
        ],
    )
    .or_else(|| {
        details(map, "completion_tokens_details")
            .and_then(|details| take_first(details, &["reasoning_tokens", "reasoningTokens"]))
    });
    let image_tokens = take_first(map, &["image_tokens", "imageTokens"]).or_else(|| {
        details(map, "prompt_tokens_details")
            .and_then(|details| take_first(details, &["image_tokens", "imageTokens"]))
            .or_else(|| {
                details(map, "completion_tokens_details")
                    .and_then(|details| take_first(details, &["image_tokens", "imageTokens"]))
            })
    });
    let audio_tokens = take_first(map, &["audio_tokens", "audioTokens"])
        .or_else(|| {
            details(map, "prompt_tokens_details")
                .and_then(|details| take_first(details, &["audio_tokens", "audioTokens"]))
        })
        .or_else(|| {
            details(map, "completion_tokens_details")
                .and_then(|details| take_first(details, &["audio_tokens", "audioTokens"]))
        })
        .or_else(|| modality_token_count(map.get("promptTokensDetails"), "AUDIO"))
        .or_else(|| modality_token_count(map.get("candidatesTokensDetails"), "AUDIO"));
    let cached_prompt_tokens = take_first(
        map,
        &[
            "cached_content_token_count",
            "cachedContentTokenCount",
            "cache_read",
            "cacheRead",
        ],
    )
    .or_else(|| {
        details(map, "prompt_tokens_details")
            .and_then(|details| take_first(details, &["cached_tokens", "cachedTokens"]))
    });
    let cache_write_tokens = details(map, "prompt_tokens_details")
        .and_then(|details| take_first(details, &["cache_write_tokens", "cacheWriteTokens"]));
    let web_search_requests = details(map, "server_tool_use").and_then(|details| {
        take_first(
            details,
            &[
                "web_search_requests",
                "webSearchRequests",
                "search_requests",
            ],
        )
    });
    let api_cost = take_first_f64(map, &["cost", "total_cost", "totalCost"]);
    let total_tokens = take_first(map, &["total_tokens", "totalTokens"]);
    if prompt_tokens.is_none()
        && completion_tokens.is_none()
        && total_tokens.is_none()
        && reasoning_tokens.is_none()
        && image_tokens.is_none()
        && audio_tokens.is_none()
        && web_search_requests.is_none()
        && api_cost.is_none()
    {
        return None;
    }
    Some(UsageSummary {
        prompt_tokens,
        completion_tokens,
        cached_prompt_tokens,
        cache_write_tokens,
        reasoning_tokens,
        image_tokens,
        audio_tokens,
        total_tokens,
        web_search_requests,
        api_cost,
    })
}

fn enrich(mut summary: UsageSummary, map: &Map<String, Value>) -> UsageSummary {
    if summary.api_cost.is_none() {
        summary.api_cost = map
            .get("usage")
            .and_then(Value::as_object)
            .and_then(|usage| usage.get("cost"))
            .and_then(parse_float_value)
            .or_else(|| map.get("cost").and_then(parse_float_value));
    }
    summary
}

fn extract(data: &Value) -> Option<UsageSummary> {
    match data {
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .and_then(|parsed| extract(&parsed))
        }
        Value::Array(items) => items.iter().find_map(extract),
        Value::Object(map) => {
            if let Some(usage) = map.get("usage")
                && let Some(summary) = match usage {
                    Value::Object(object) => usage_from_map(object),
                    other => extract(other),
                }
            {
                return Some(enrich(summary, map));
            }
            if let Some(summary) = usage_from_map(map) {
                return Some(enrich(summary, map));
            }
            map.values().find_map(extract)
        }
        _ => None,
    }
}

/// The usage an image response reports, if any.
#[must_use]
pub(crate) fn extract_usage(data: &Value) -> Option<InferenceUsage> {
    let summary = extract(data)?;
    Some(InferenceUsage {
        provider_reported_cost: summary.api_cost.and_then(ProviderReportedCost::new),
        cache_write_tokens: summary.cache_write_tokens,
        web_search_requests: summary.web_search_requests,
        cached_input_tokens: summary.cached_prompt_tokens,
        reasoning_tokens: summary.reasoning_tokens,
        image_tokens: summary.image_tokens,
        audio_tokens: summary.audio_tokens,
        total_tokens: summary.total_tokens,
        input_tokens: summary.prompt_tokens.unwrap_or_default(),
        output_tokens: summary.completion_tokens.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_usage_is_found_in_openai_and_openrouter_shapes() {
        let usage = extract_usage(&serde_json::json!({
            "data": [{"b64_json": "aGk="}],
            "usage": {"input_tokens": 12, "output_tokens": 4, "total_tokens": 16}
        }))
        .expect("usage");
        assert_eq!((usage.input_tokens, usage.output_tokens), (12, 4));
        assert_eq!(usage.total_tokens, Some(16));
        let usage = extract_usage(&serde_json::json!({
            "usage": {"input_tokens": 5, "output_tokens": 0,
                      "input_tokens_details": {"image_tokens": 1},
                      "prompt_tokens_details": {"cached_tokens": 4, "audio_tokens": 2}}
        }))
        .expect("usage");
        assert_eq!(
            usage.image_tokens, None,
            "cached prompt tokens are not image tokens"
        );
        assert_eq!(usage.audio_tokens, Some(2));
        assert_eq!(usage.total_tokens, None);
        let usage = extract_usage(&serde_json::json!({"usage": {"image_tokens": 1056}}))
            .expect("image-only usage");
        assert_eq!(usage.image_tokens, Some(1056));
        let usage = extract_usage(&serde_json::json!({
            "choices": [],
            "usage": {"prompt_tokens": 3, "completion_tokens": 9, "cost": 0.04,
                      "completion_tokens_details": {"reasoning_tokens": 2}}
        }))
        .expect("usage");
        assert_eq!(usage.reasoning_tokens, Some(2));
        assert_eq!(
            usage.provider_reported_cost.map(ProviderReportedCost::get),
            Some(0.04)
        );
        assert_eq!(
            extract_usage(&serde_json::json!({"images": ["aGk="]})),
            None
        );
        assert_eq!(
            extract_usage(&serde_json::json!({"usageMetadata": {"promptTokenCount": 3}})),
            None,
            "legacy did not read Gemini's token counts for images"
        );
    }
}
