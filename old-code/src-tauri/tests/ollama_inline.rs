//! Gathered from inline tests in src/ollama/mod.rs.

use lettuceai_lib::chat_manager::{sse::SseDecoder, types::NormalizedEvent};
use lettuceai_lib::ollama::{
    normalize_assistant_tool_calls, normalize_base_url, normalize_request_body,
};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn strips_api_chat_suffix() {
    let url = normalize_base_url("http://127.0.0.1:11434/api/chat").expect("url");
    assert_eq!(url, "http://127.0.0.1:11434/");
}

#[test]
fn strips_v1_suffix() {
    let url = normalize_base_url("http://127.0.0.1:11434/v1").expect("url");
    assert_eq!(url, "http://127.0.0.1:11434/");
}

#[test]
fn normalizes_openai_style_tool_calls_for_ollama() {
    let input = vec![json!({
        "id": "call_weather",
        "type": "function",
        "function": {
            "name": "get_weather",
            "arguments": "{\"city\":\"Istanbul\"}"
        }
    })];
    let mut map = HashMap::new();

    let normalized = normalize_assistant_tool_calls(&input, &mut map);

    assert_eq!(
        map.get("call_weather").map(String::as_str),
        Some("get_weather")
    );
    assert_eq!(
        normalized,
        vec![json!({
            "type": "function",
            "function": {
                "index": 0,
                "name": "get_weather",
                "arguments": { "city": "Istanbul" }
            }
        })]
    );
}

#[tokio::test]
async fn infers_tool_name_from_tool_call_id_for_tool_messages() {
    let body = json!({
        "model": "qwen3",
        "messages": [
            {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "get_weather",
                        "arguments": { "city": "Istanbul" }
                    }
                }]
            },
            {
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"temperature\":\"20C\"}"
            }
        ]
    });

    let normalized = normalize_request_body(&body)
        .await
        .expect("normalized body");
    let messages = normalized
        .get("messages")
        .and_then(|value| value.as_array())
        .expect("messages array");

    assert_eq!(
        messages[1]
            .get("tool_name")
            .and_then(|value| value.as_str()),
        Some("get_weather")
    );
}

#[test]
fn ollama_stream_decoder_preserves_leading_spaces_in_deltas() {
    let mut decoder = SseDecoder::new();

    let first = decoder.feed(
        "{\"message\":{\"role\":\"assistant\",\"content\":\"Mirelle\"},\"done\":false}\n",
        Some("ollama"),
    );
    let second = decoder.feed(
        "{\"message\":{\"role\":\"assistant\",\"content\":\"'s eyes return to yours\"},\"done\":false}\n",
        Some("ollama"),
    );
    let third = decoder.feed(
        "{\"message\":{\"role\":\"assistant\",\"content\":\" as she presses her seal into each corner.\"},\"done\":true}\n",
        Some("ollama"),
    );

    assert_eq!(first.len(), 1);
    match &first[0] {
        NormalizedEvent::Delta { text } => assert_eq!(text, "Mirelle"),
        other => panic!("unexpected first event: {other:?}"),
    }

    assert_eq!(second.len(), 1);
    match &second[0] {
        NormalizedEvent::Delta { text } => assert_eq!(text, "'s eyes return to yours"),
        other => panic!("unexpected second event: {other:?}"),
    }

    assert_eq!(third.len(), 2);
    match &third[0] {
        NormalizedEvent::Delta { text } => {
            assert_eq!(text, " as she presses her seal into each corner.")
        }
        other => panic!("unexpected third delta: {other:?}"),
    }
    assert!(matches!(third[1], NormalizedEvent::Done));
}
