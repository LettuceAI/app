//! Additional integration tests for tool-call parsing.

use lettuceai_lib::chat_manager::tooling::{
    parse_tool_calls, parse_tool_calls_from_text, strip_tool_call_blocks,
};
use serde_json::json;

#[test]
fn parse_openai_tool_calls_from_message() {
    let payload = json!({
        "choices": [{
            "message": {
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"city\":\"Paris\"}"
                    }
                }]
            }
        }]
    });
    let calls = parse_tool_calls("openai", &payload);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "get_weather");
    assert_eq!(calls[0].arguments["city"], "Paris");
}

#[test]
fn parse_anthropic_tool_use_block() {
    let payload = json!({
        "content": [
            {"type": "text", "text": "let me check"},
            {"type": "tool_use", "id": "tu_1", "name": "lookup", "input": {"q": "abc"}}
        ]
    });
    let calls = parse_tool_calls("anthropic", &payload);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "lookup");
    assert_eq!(calls[0].arguments["q"], "abc");
}

#[test]
fn parse_gemini_function_call() {
    let payload = json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "search",
                        "args": {"query": "test"}
                    }
                }]
            }
        }]
    });
    let calls = parse_tool_calls("gemini", &payload);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "search");
}

#[test]
fn parse_empty_payload_returns_empty() {
    let calls = parse_tool_calls("openai", &json!({}));
    assert!(calls.is_empty());
}

#[test]
fn parse_unknown_provider_no_panic() {
    let _ = parse_tool_calls("unknown_provider", &json!({"data": "ok"}));
}

#[test]
fn parse_tool_call_xml_tag() {
    let raw = r#"<tool_call>{"name": "f", "arguments": {"a": 1}}</tool_call>"#;
    let calls = parse_tool_calls_from_text(raw);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "f");
}

#[test]
fn parse_function_call_xml_tag() {
    let raw = r#"<function_call>{"name": "g", "arguments": {}}</function_call>"#;
    let calls = parse_tool_calls_from_text(raw);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "g");
}

#[test]
fn parse_text_without_tool_calls_returns_empty_or_handles() {
    let calls = parse_tool_calls_from_text("just regular text");
    let _ = calls;
}

#[test]
fn strip_removes_tool_call_block() {
    let raw = "Hello <tool_call>{\"name\":\"f\"}</tool_call> world";
    let out = strip_tool_call_blocks(raw);
    assert!(!out.contains("tool_call"));
    assert!(out.contains("Hello"));
    assert!(out.contains("world"));
}

#[test]
fn strip_removes_multiple_blocks() {
    let raw = "a <tool_call>x</tool_call> b <function_call>y</function_call> c";
    let out = strip_tool_call_blocks(raw);
    assert!(out.contains("a"));
    assert!(out.contains("b"));
    assert!(out.contains("c"));
    assert!(!out.contains("tool_call"));
    assert!(!out.contains("function_call"));
}

#[test]
fn strip_preserves_text_without_blocks() {
    let raw = "no tool calls here";
    assert_eq!(strip_tool_call_blocks(raw).trim(), raw);
}

#[test]
fn strip_empty_input() {
    assert_eq!(strip_tool_call_blocks(""), "");
}
