use serde_json::Value;

use crate::chat_manager::thinking::{
    normalize_thinking_content, ThinkingSplit, ThinkingTagStreamParser,
};
use crate::chat_manager::tooling::ToolCall;

use super::tooling::parse_tool_calls;
use super::types::{NormalizedEvent, UsageSummary};

pub fn accumulate_text_from_sse(raw: &str, provider_id: Option<&str>) -> Option<String> {
    let split = accumulate_thinking_split_from_sse(raw, provider_id);
    (!split.content.is_empty()).then_some(split.content)
}

pub fn accumulate_reasoning_from_sse(raw: &str, provider_id: Option<&str>) -> Option<String> {
    let split = accumulate_thinking_split_from_sse(raw, provider_id);
    (!split.reasoning.is_empty()).then_some(split.reasoning)
}

fn accumulate_thinking_split_from_sse(raw: &str, provider_id: Option<&str>) -> ThinkingSplit {
    let mut split = ThinkingSplit::default();
    let mut parser = ThinkingTagStreamParser::default();

    for line in raw.lines() {
        let l = line.trim();
        if !l.starts_with("data:") {
            continue;
        }

        let payload = l[5..].trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }

        let Ok(v) = serde_json::from_str::<Value>(payload) else {
            continue;
        };

        // HARD FILTER: skip tool frames
        if let Some(pid) = provider_id {
            if !parse_tool_calls(pid, &v).is_empty() {
                continue;
            }
        }

        if let Some(piece) = extract_text_from_value(&v) {
            let parsed = parser.feed(&piece);
            split.content.push_str(&parsed.content);
            split.reasoning.push_str(&parsed.reasoning);
        }
        if let Some(piece) = extract_reasoning_from_value(&v) {
            split.reasoning.push_str(&piece);
        }
    }

    let tail = parser.finish();
    split.content.push_str(&tail.content);
    split.reasoning.push_str(&tail.reasoning);
    split.content = split.content.trim().to_string();
    split.reasoning = split.reasoning.trim().to_string();
    split
}

pub fn accumulate_image_data_urls_from_sse(raw: &str) -> Vec<String> {
    let mut out = Vec::new();

    for line in raw.lines() {
        let l = line.trim();
        if !l.starts_with("data:") {
            continue;
        }

        let payload = l[5..].trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }

        if let Ok(v) = serde_json::from_str::<Value>(payload) {
            extract_image_data_urls_from_value(&v, &mut out);
        }
    }

    out
}

pub fn accumulate_tool_calls_from_sse(raw: &str, provider_id: &str) -> Vec<ToolCall> {
    let mut out: Vec<ToolCall> = Vec::new();

    for line in raw.lines() {
        let l = line.trim();
        if !l.starts_with("data:") {
            continue;
        }

        let payload = l[5..].trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }

        let Ok(v) = serde_json::from_str::<Value>(payload) else {
            continue;
        };

        let calls = parse_tool_calls(provider_id, &v);
        for call in calls {
            if let Some(existing) = out.iter_mut().find(|c| c.id == call.id) {
                if let (Value::String(a), Value::String(b)) =
                    (&mut existing.arguments, &call.arguments)
                {
                    a.push_str(b);
                }
            } else {
                out.push(call);
            }
        }
    }
    out
}

pub fn usage_from_sse(raw: &str) -> Option<UsageSummary> {
    let mut last = None;

    for line in raw.lines() {
        let l = line.trim();
        if !l.starts_with("data:") {
            continue;
        }

        let payload = l[5..].trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }

        if let Ok(v) = serde_json::from_str::<Value>(payload) {
            if let Some(u) = usage_from_value(&v) {
                last = Some(u);
            }
        }
    }

    last
}

//
// ------------------------------------------------------------
//  Streaming decoder
// ------------------------------------------------------------
//

#[derive(Default)]
pub struct SseDecoder {
    buffer: String,
    thinking_parser: ThinkingTagStreamParser,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            thinking_parser: ThinkingTagStreamParser::default(),
        }
    }

    pub fn feed(&mut self, chunk: &str, provider_id: Option<&str>) -> Vec<NormalizedEvent> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        let mut last_newline = 0;
        for (idx, ch) in self.buffer.char_indices() {
            if ch != '\n' {
                continue;
            }

            let line = &self.buffer[last_newline..idx];
            last_newline = idx + 1;

            let l = line.trim();
            if l.is_empty() {
                continue;
            }

            let payload = if provider_id == Some("ollama") {
                if let Some(rest) = l.strip_prefix("data:") {
                    rest.trim()
                } else {
                    l
                }
            } else {
                let Some(rest) = l.strip_prefix("data:") else {
                    continue;
                };
                rest.trim()
            };

            if payload.is_empty() {
                continue;
            }

            if payload == "[DONE]" {
                let tail = self.thinking_parser.finish();
                if !tail.content.is_empty() {
                    events.push(NormalizedEvent::Delta { text: tail.content });
                }
                if !tail.reasoning.is_empty() {
                    events.push(NormalizedEvent::Reasoning {
                        text: tail.reasoning,
                    });
                }
                events.push(NormalizedEvent::Done);
                continue;
            }

            let Ok(v) = serde_json::from_str::<Value>(payload) else {
                continue;
            };

            // 1. Errors always win
            if let Some(err) = extract_gemini_error(&v) {
                events.push(NormalizedEvent::Error {
                    envelope: super::types::ErrorEnvelope {
                        code: Some("CONTENT_BLOCKED".to_string()),
                        message: err,
                        provider_id: Some("gemini".to_string()),
                        request_id: None,
                        retryable: Some(false),
                        status: Some(400),
                    },
                });
                continue;
            }

            let is_done = v.get("done").and_then(|d| d.as_bool()).unwrap_or(false);

            // 2. Tool calls – HARD FILTER POINT
            if let Some(provider) = provider_id {
                let calls = parse_tool_calls(provider, &v);
                if !calls.is_empty() {
                    events.push(NormalizedEvent::ToolCall { calls });
                    continue; // nothing else is allowed through
                }
            }

            // 3. Text
            if let Some(piece) = extract_text_from_value(&v) {
                let parsed = self.thinking_parser.feed(&piece);
                if !parsed.content.is_empty() {
                    events.push(NormalizedEvent::Delta {
                        text: parsed.content,
                    });
                }
                if !parsed.reasoning.is_empty() {
                    events.push(NormalizedEvent::Reasoning {
                        text: parsed.reasoning,
                    });
                }
            }

            // 4. Reasoning
            if let Some(reasoning) = extract_reasoning_from_value(&v) {
                if !reasoning.is_empty() {
                    events.push(NormalizedEvent::Reasoning { text: reasoning });
                }
            }

            // 5. Usage
            if let Some(usage) = usage_from_value(&v) {
                events.push(NormalizedEvent::Usage { usage });
            }

            if is_done {
                let tail = self.thinking_parser.finish();
                if !tail.content.is_empty() {
                    events.push(NormalizedEvent::Delta { text: tail.content });
                }
                if !tail.reasoning.is_empty() {
                    events.push(NormalizedEvent::Reasoning {
                        text: tail.reasoning,
                    });
                }
                events.push(NormalizedEvent::Done);
            }
        }

        if last_newline > 0 {
            self.buffer.drain(..last_newline);
        }

        events
    }
}

fn extract_text_from_value(v: &Value) -> Option<String> {
    if let Some(s) = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    if let Some(s) = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    // Anthropic Messages API streaming: content_block_delta -> delta -> text
    if v.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
        if let Some(s) = v
            .get("delta")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            return Some(s.to_string());
        }
    }

    // Gemini-style: candidates[].content.parts[].text (skip thought=true parts)
    if let Some(candidates) = v.get("candidates").and_then(|c| c.as_array()) {
        let mut combined = String::new();
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for part in parts {
                    let is_thought = part
                        .get("thought")
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false);
                    if is_thought {
                        continue;
                    }
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        combined.push_str(text);
                    }
                }
            }
        }
        if !combined.is_empty() {
            return Some(combined);
        }
    }
    if let Some(s) = v.get("content").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    if let Some(s) = v.get("text").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    None
}

/// Extract reasoning tokens from thinking models
/// The reasoning content is found in choices[0].delta.reasoning or choices[0].delta.reasoning_content
fn extract_reasoning_from_value(v: &Value) -> Option<String> {
    // OpenAI/OpenRouter style: choices[0].delta.reasoning
    if let Some(s) = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("reasoning"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    // Some models use reasoning_content instead
    if let Some(s) = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("reasoning_content"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    // Streaming decoders must preserve per-chunk whitespace. Do not trim these direct
    // reasoning fields, because providers like Ollama emit leading spaces in later chunks.
    if let Some(s) = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("reasoning"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    if let Some(s) = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("reasoning_content"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    if let Some(s) = v
        .get("message")
        .and_then(|m| m.get("thinking"))
        .and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    // Gemini-style: candidates[].content.parts[] with thought=true
    if let Some(candidates) = v.get("candidates").and_then(|c| c.as_array()) {
        let mut combined = String::new();
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for part in parts {
                    let is_thought = part
                        .get("thought")
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false);
                    if is_thought {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            combined.push_str(text);
                        }
                    }
                }
            }
        }
        if !combined.is_empty() {
            return Some(combined);
        }
    }

    let fallback_text = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .or_else(|| {
            v.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str())
        })
        .or_else(|| v.get("content").and_then(|t| t.as_str()))
        .or_else(|| v.get("text").and_then(|t| t.as_str()));
    if let Some(text) = fallback_text {
        let split = normalize_thinking_content(Some(text), None);
        return (!split.reasoning.is_empty()).then_some(split.reasoning);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::SseDecoder;
    use crate::chat_manager::types::NormalizedEvent;

    #[test]
    fn ollama_reasoning_stream_preserves_leading_spaces() {
        let mut decoder = SseDecoder::new();

        let first = decoder.feed(
            "{\"message\":{\"thinking\":\"ThinkingProcess: 1.\"},\"done\":false}\n",
            Some("ollama"),
        );
        let second = decoder.feed(
            "{\"message\":{\"thinking\":\" Analyze the Request\"},\"done\":false}\n",
            Some("ollama"),
        );

        assert_eq!(first.len(), 1);
        match &first[0] {
            NormalizedEvent::Reasoning { text } => assert_eq!(text, "ThinkingProcess: 1."),
            other => panic!("unexpected first event: {other:?}"),
        }

        assert_eq!(second.len(), 1);
        match &second[0] {
            NormalizedEvent::Reasoning { text } => assert_eq!(text, " Analyze the Request"),
            other => panic!("unexpected second event: {other:?}"),
        }
    }
}

fn extract_image_data_urls_from_value(v: &Value, out: &mut Vec<String>) {
    // OpenAI-style streaming: choices[].delta.images[].image_url.url
    if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
        for choice in choices {
            let images = choice
                .get("delta")
                .and_then(|d| d.get("images"))
                .and_then(|i| i.as_array())
                .or_else(|| {
                    choice
                        .get("message")
                        .and_then(|m| m.get("images"))
                        .and_then(|i| i.as_array())
                });

            if let Some(images) = images {
                for img in images {
                    if let Some(url) = img
                        .get("image_url")
                        .and_then(|iu| iu.get("url"))
                        .and_then(|u| u.as_str())
                    {
                        if url.starts_with("data:image/") {
                            out.push(url.to_string());
                        }
                    }
                }
            }
        }
    }
}

pub fn usage_from_value(v: &Value) -> Option<UsageSummary> {
    // Support both snake_case "usage" (OpenAI) and camelCase "usageMetadata" (Gemini)
    let u = v.get("usage").or_else(|| v.get("usageMetadata"));

    let (
        prompt_tokens,
        completion_tokens,
        reasoning_tokens,
        image_tokens,
        cached_prompt_tokens,
        cache_write_tokens,
        web_search_requests,
        api_cost,
        total_tokens,
    ) = if let Some(u) = u {
        // Log the usage metadata for debugging
        crate::utils::log_debug_global("sse_usage", format!("Usage metadata received: {:?}", u));

        let prompt_tokens = take_first(
            u,
            &[
                "prompt_tokens",
                "input_tokens",
                "promptTokens",
                "inputTokens",
                "promptTokenCount", // Gemini-specific
            ],
        );
        let completion_tokens = take_first(
            u,
            &[
                "completion_tokens",
                "output_tokens",
                "completionTokens",
                "outputTokens",
                "candidatesTokenCount", // Gemini-specific
            ],
        );
        let reasoning_tokens = take_first(
            u,
            &[
                "reasoning_tokens",
                "reasoningTokens",
                "thinking_tokens",
                "thinkingTokens",
                "thoughtsTokenCount", // Gemini-specific
            ],
        )
        .or_else(|| {
            u.get("completion_tokens_details")
                .and_then(|d| take_first(d, &["reasoning_tokens", "reasoningTokens"]))
        });
        let image_tokens = take_first(u, &["image_tokens", "imageTokens"]).or_else(|| {
            u.get("prompt_tokens_details")
                .and_then(|d| take_first(d, &["image_tokens", "imageTokens", "cached_tokens"]))
        });
        let cached_prompt_tokens = u
            .get("prompt_tokens_details")
            .and_then(|d| take_first(d, &["cached_tokens", "cachedTokens"]));
        let cache_write_tokens = u
            .get("prompt_tokens_details")
            .and_then(|d| take_first(d, &["cache_write_tokens", "cacheWriteTokens"]));
        let web_search_requests = u.get("server_tool_use").and_then(|d| {
            take_first(
                d,
                &[
                    "web_search_requests",
                    "webSearchRequests",
                    "search_requests",
                ],
            )
        });
        let api_cost = take_first_f64(u, &["cost", "total_cost", "totalCost"]);
        let total_tokens = take_first(u, &["total_tokens", "totalTokens", "totalTokenCount"])
            .or_else(|| match (prompt_tokens, completion_tokens) {
                (Some(p), Some(c)) => Some(p + c),
                _ => None,
            });

        (
            prompt_tokens,
            completion_tokens,
            reasoning_tokens,
            image_tokens,
            cached_prompt_tokens,
            cache_write_tokens,
            web_search_requests,
            api_cost,
            total_tokens,
        )
    } else {
        let prompt_tokens = take_first(v, &["prompt_eval_count", "prompt_tokens"]);
        let completion_tokens = take_first(v, &["eval_count", "completion_tokens"]);
        let total_tokens = match (prompt_tokens, completion_tokens) {
            (Some(p), Some(c)) => Some(p + c),
            _ => None,
        };
        (
            prompt_tokens,
            completion_tokens,
            None,
            None,
            None,
            None,
            None,
            None,
            total_tokens,
        )
    };

    let finish_reason = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finish_reason"))
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            v.get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("finishReason"))
                .and_then(|r| r.as_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            // Anthropic/SSE specific if available in the same value
            v.get("stop_reason")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string())
        });
    let first_token_ms = take_first(v, &["first_token_ms", "firstTokenMs", "ttft_ms", "ttftMs"]);
    let tokens_per_second = take_first_f64(
        v,
        &[
            "tokens_per_second",
            "tokensPerSecond",
            "token_speed",
            "tokenSpeed",
            "tps",
        ],
    );

    crate::utils::log_debug_global(
        "sse_usage",
        format!(
            "Parsed usage: prompt={:?}, completion={:?}, total={:?}, reasoning={:?}",
            prompt_tokens, completion_tokens, total_tokens, reasoning_tokens
        ),
    );

    if prompt_tokens.is_none() && completion_tokens.is_none() && total_tokens.is_none() {
        None
    } else {
        Some(UsageSummary {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cached_prompt_tokens,
            cache_write_tokens,
            reasoning_tokens,
            image_tokens,
            web_search_requests,
            api_cost,
            response_id: v
                .get("id")
                .or_else(|| v.get("generation_id"))
                .and_then(|id| id.as_str())
                .map(|id| id.to_string()),
            first_token_ms,
            tokens_per_second,
            finish_reason,
        })
    }
}

fn take_first(map: &Value, keys: &[&str]) -> Option<u64> {
    for k in keys {
        if let Some(val) = map.get(*k) {
            if let Some(n) = val.as_u64() {
                return Some(n);
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.trim().parse::<u64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn take_first_f64(map: &Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(val) = map.get(*k) {
            if let Some(n) = val.as_f64() {
                return Some(n);
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.trim().parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn extract_gemini_error(v: &Value) -> Option<String> {
    if let Some(prompt_feedback) = v.get("promptFeedback") {
        if let Some(block_reason) = prompt_feedback.get("blockReason").and_then(|r| r.as_str()) {
            return Some(format_gemini_block_reason(block_reason));
        }
    }

    if let Some(candidates) = v.get("candidates").and_then(|c| c.as_array()) {
        for candidate in candidates {
            if let Some(finish_reason) = candidate.get("finishReason").and_then(|r| r.as_str()) {
                if let Some(err) = format_gemini_finish_reason_error(finish_reason) {
                    return Some(err);
                }
            }
        }
    }

    None
}

fn format_gemini_block_reason(reason: &str) -> String {
    match reason {
        "BLOCK_REASON_UNSPECIFIED" => {
            "Content was blocked by Gemini for an unspecified reason.".to_string()
        }
        "SAFETY" => {
            "Content was blocked by Gemini safety filters. Try adjusting your prompt or safety settings.".to_string()
        }
        "OTHER" => "Content was blocked by Gemini for an uncategorized reason.".to_string(),
        "BLOCKLIST" => {
            "Content was blocked: the prompt contains terms from the blocklist.".to_string()
        }
        "PROHIBITED_CONTENT" => {
            "Content was blocked by Gemini: prohibited content detected (e.g., CSAM or policy violation).".to_string()
        }
        "IMAGE_SAFETY" => {
            "Content was blocked by Gemini: the image failed safety checks.".to_string()
        }
        _ => format!(
            "Content was blocked by Gemini: {}",
            reason.replace('_', " ").to_lowercase()
        ),
    }
}

fn format_gemini_finish_reason_error(reason: &str) -> Option<String> {
    match reason {
        "STOP" | "MAX_TOKENS" | "FINISH_REASON_UNSPECIFIED" => None,
        "SAFETY" => Some("Response was blocked by Gemini safety filters.".to_string()),
        "RECITATION" => Some(
            "Response was blocked due to recitation concerns (potential copyright issues)."
                .to_string(),
        ),
        "LANGUAGE" => Some("Response was blocked: unsupported language detected.".to_string()),
        "OTHER" => Some("Response was blocked by Gemini for an uncategorized reason.".to_string()),
        "BLOCKLIST" => {
            Some("Response was blocked: output contains terms from the blocklist.".to_string())
        }
        "PROHIBITED_CONTENT" => {
            Some("Response was blocked: prohibited content detected.".to_string())
        }
        "SPII" => Some(
            "Response was blocked: sensitive personally identifiable information (SPII) detected."
                .to_string(),
        ),
        "MALFORMED_FUNCTION_CALL" => {
            Some("Response generation failed: malformed function call.".to_string())
        }
        "IMAGE_SAFETY" => Some("Image generation was blocked by safety filters.".to_string()),
        "IMAGE_PROHIBITED_CONTENT" => {
            Some("Image generation was blocked: prohibited content detected.".to_string())
        }
        "IMAGE_OTHER" => {
            Some("Image generation was blocked for an uncategorized reason.".to_string())
        }
        "NO_IMAGE" => Some("Image generation failed: no image was produced.".to_string()),
        "IMAGE_RECITATION" => {
            Some("Image generation was blocked due to recitation concerns.".to_string())
        }
        "UNEXPECTED_TOOL_CALL" => {
            Some("Response generation failed: unexpected tool call.".to_string())
        }
        "TOO_MANY_TOOL_CALLS" => {
            Some("Response generation failed: too many tool calls.".to_string())
        }
        "MISSING_THOUGHT_SIGNATURE" => {
            Some("Response generation failed: missing thought signature.".to_string())
        }
        _ => Some(format!(
            "Response was blocked by Gemini: {}",
            reason.replace('_', " ").to_lowercase()
        )),
    }
}
