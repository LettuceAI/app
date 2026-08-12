//! Gathered from inline tests in src/chat_manager/provider_adapter/google_gemini.rs.

use lettuceai_lib::chat_manager::provider_adapter::google_gemini::{
    gemini_thinking_mode, GeminiThinkingMode, GoogleGeminiAdapter,
};
use lettuceai_lib::chat_manager::provider_adapter::ProviderAdapter;
use lettuceai_lib::chat_manager::tooling::{ToolChoice, ToolConfig, ToolDefinition};
use serde_json::json;

#[test]
fn moves_system_and_developer_messages_into_system_instruction() {
    let adapter = GoogleGeminiAdapter;
    let body = adapter.body(
        "gemini-2.5-flash",
        &vec![
            json!({ "role": "system", "content": "You are a terse assistant." }),
            json!({ "role": "developer", "content": "Stay in character." }),
            json!({ "role": "user", "content": "Say hello." }),
        ],
        None,
        Some(0.2),
        None,
        256,
        None,
        true,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
    );

    assert_eq!(
        body.get("systemInstruction"),
        Some(&json!({
            "parts": [{
                "text": "You are a terse assistant.\n\nStay in character."
            }]
        }))
    );
    assert_eq!(
        body.get("contents"),
        Some(&json!([
            {
                "role": "user",
                "parts": [{ "text": "Say hello." }]
            }
        ]))
    );
}

#[test]
fn keeps_visible_chat_system_messages_in_contents() {
    let adapter = GoogleGeminiAdapter;
    let body = adapter.body(
        "gemini-2.5-flash",
        &vec![
            json!({ "role": "system", "content": "Base instruction." }),
            json!({ "role": "system", "content": "Always reply with UwU no matter what.", "visible_in_chat": true }),
            json!({ "role": "user", "content": "Continue." }),
        ],
        None,
        Some(0.2),
        None,
        256,
        None,
        true,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
    );

    assert_eq!(
        body.get("systemInstruction"),
        Some(&json!({
            "parts": [{
                "text": "Base instruction."
            }]
        }))
    );
    assert_eq!(
        body.get("contents"),
        Some(&json!([
            {
                "role": "user",
                "parts": [{
                    "text": "Visible system message from the chat UI. Treat this as a high-priority instruction that remains in effect unless later context overrides it.\n\n<system-message>\nAlways reply with UwU no matter what.\n</system-message>"
                }]
            },
            {
                "role": "user",
                "parts": [{ "text": "Continue." }]
            }
        ]))
    );
}

#[test]
fn includes_tool_call_ids_and_tool_config() {
    let adapter = GoogleGeminiAdapter;
    let tool_config = ToolConfig {
        tools: vec![ToolDefinition {
            name: "lookup_weather".to_string(),
            description: Some("Get weather".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
        }],
        choice: Some(ToolChoice::Tool {
            name: "lookup_weather".to_string(),
        }),
    };

    let body = adapter.body(
        "gemini-2.5-flash",
        &vec![
            json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_123",
                    "function": {
                        "name": "lookup_weather",
                        "arguments": "{\"city\":\"Istanbul\"}"
                    }
                }]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_123",
                "content": "{\"temperature\":18}"
            }),
        ],
        Some("Use tools when needed.".to_string()),
        None,
        None,
        256,
        None,
        true,
        None,
        None,
        None,
        Some(&tool_config),
        false,
        None,
        None,
    );

    assert_eq!(
        body.get("toolConfig"),
        Some(&json!({
            "functionCallingConfig": {
                "mode": "ANY",
                "allowedFunctionNames": ["lookup_weather"]
            }
        }))
    );
    assert_eq!(
        body.get("tools"),
        Some(&json!([{
            "functionDeclarations": [{
                "name": "lookup_weather",
                "description": "Get weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    },
                    "required": ["city"]
                }
            }]
        }]))
    );

    let contents = body
        .get("contents")
        .and_then(|value| value.as_array())
        .expect("contents");
    assert_eq!(
        contents[0],
        json!({
            "role": "model",
            "parts": [{
                "functionCall": {
                    "id": "call_123",
                    "name": "lookup_weather",
                    "args": { "city": "Istanbul" }
                }
            }]
        })
    );
    assert_eq!(
        contents[1],
        json!({
            "role": "user",
            "parts": [{
                "functionResponse": {
                    "id": "call_123",
                    "name": "lookup_weather",
                    "response": { "temperature": 18 }
                }
            }]
        })
    );
}

#[test]
fn preserves_function_name_for_raw_gemini_tool_call_results() {
    let adapter = GoogleGeminiAdapter;
    let body = adapter.body(
        "gemini-3-flash-preview",
        &vec![
            json!({
                "role": "assistant",
                "gemini_content": {
                    "role": "model",
                    "parts": [{
                        "functionCall": {
                            "id": "call_123",
                            "name": "lookup_weather",
                            "args": { "city": "Istanbul" }
                        },
                        "thoughtSignature": "signature"
                    }]
                }
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_123",
                "content": "{\"temperature\":18}"
            }),
        ],
        None,
        None,
        None,
        256,
        None,
        false,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
    );

    assert_eq!(
        body["contents"][1]["parts"][0]["functionResponse"]["name"],
        json!("lookup_weather")
    );
}

#[test]
fn keeps_a_v1beta_base_url_intact() {
    let adapter = GoogleGeminiAdapter;
    assert_eq!(
        adapter.build_url(
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-3-flash-preview",
            "key",
            false,
        ),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key=key"
    );
}

#[test]
fn gemini_25_uses_budget_based_thinking() {
    let adapter = GoogleGeminiAdapter;
    let body = adapter.body(
        "gemini-2.5-flash",
        &vec![json!({ "role": "user", "content": "Think." })],
        None,
        None,
        None,
        256,
        None,
        false,
        None,
        None,
        None,
        None,
        true,
        Some("high".to_string()),
        Some(8192),
    );

    assert_eq!(
        body.pointer("/generationConfig/thinkingConfig"),
        Some(&json!({
            "includeThoughts": true,
            "thinkingBudget": 8192
        }))
    );
}

#[test]
fn gemini_3_uses_level_based_thinking() {
    let adapter = GoogleGeminiAdapter;
    let body = adapter.body(
        "gemini-3-flash-preview",
        &vec![json!({ "role": "user", "content": "Think." })],
        None,
        None,
        None,
        256,
        None,
        false,
        None,
        None,
        None,
        None,
        true,
        Some("medium".to_string()),
        Some(8192),
    );

    assert_eq!(
        body.pointer("/generationConfig/thinkingConfig"),
        Some(&json!({
            "includeThoughts": true,
            "thinkingLevel": "MEDIUM"
        }))
    );
}

#[test]
fn gemini_3_auto_omits_budget() {
    let adapter = GoogleGeminiAdapter;
    let body = adapter.body(
        "gemini-3-flash-preview",
        &vec![json!({ "role": "user", "content": "Think." })],
        None,
        None,
        None,
        256,
        None,
        false,
        None,
        None,
        None,
        None,
        true,
        None,
        Some(8192),
    );

    assert_eq!(
        body.pointer("/generationConfig/thinkingConfig"),
        Some(&json!({
            "includeThoughts": true
        }))
    );
}

#[test]
fn classifies_gemini_model_thinking_modes() {
    assert_eq!(
        gemini_thinking_mode("gemini-2.5-flash"),
        GeminiThinkingMode::Budget
    );
    assert_eq!(
        gemini_thinking_mode("gemini-3-flash-preview"),
        GeminiThinkingMode::Level
    );
    assert_eq!(
        gemini_thinking_mode("gemini-pro"),
        GeminiThinkingMode::Unknown
    );
}
