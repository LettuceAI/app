//! Gathered from inline tests in src/chat_manager/tooling.rs.

use lettuceai_lib::chat_manager::tooling::{
    gemini_tool_config, gemini_tools, parse_tool_calls, parse_tool_calls_from_text,
    strip_tool_call_blocks, ToolChoice, ToolConfig, ToolDefinition,
};
use serde_json::json;

#[test]
fn parses_ollama_non_streaming_tool_calls_from_message() {
    let payload = json!({
        "model": "qwen3",
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "function": {
                    "name": "get_weather",
                    "arguments": {
                        "city": "Istanbul"
                    }
                }
            }]
        },
        "done": true
    });

    let calls = parse_tool_calls("ollama", &payload);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "get_weather");
    assert_eq!(calls[0].arguments, json!({ "city": "Istanbul" }));
}

#[test]
fn parses_ollama_streaming_tool_calls_from_message() {
    let payload = json!({
        "message": {
            "role": "assistant",
            "tool_calls": [{
                "id": "call_1",
                "function": {
                    "name": "add_two_numbers",
                    "arguments": {
                        "a": 3,
                        "b": 1
                    }
                }
            }]
        },
        "done": false
    });

    let calls = parse_tool_calls("ollama", &payload);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_1");
    assert_eq!(calls[0].name, "add_two_numbers");
    assert_eq!(calls[0].arguments, json!({ "a": 3, "b": 1 }));
}

#[test]
fn parses_openai_legacy_function_call_from_message() {
    let payload = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "function_call": {
                    "name": "write_summary",
                    "arguments": "{\"summary\":\"short recap\"}"
                }
            }
        }]
    });

    let calls = parse_tool_calls("openai", &payload);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "write_summary");
    assert_eq!(calls[0].arguments, json!({ "summary": "short recap" }));
    assert_eq!(
        calls[0].raw_arguments.as_deref(),
        Some("{\"summary\":\"short recap\"}")
    );
}

#[test]
fn parses_openai_legacy_function_call_camel_case() {
    let payload = json!({
        "message": {
            "role": "assistant",
            "functionCall": {
                "name": "create_memory",
                "arguments": {
                    "text": "Likes tea",
                    "category": "preference"
                }
            }
        }
    });

    let calls = parse_tool_calls("local", &payload);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "create_memory");
    assert_eq!(
        calls[0].arguments,
        json!({ "text": "Likes tea", "category": "preference" })
    );
}

#[test]
fn gemini_tools_use_camel_case_fields() {
    let cfg = ToolConfig {
        tools: vec![ToolDefinition {
            name: "lookup_weather".to_string(),
            description: Some("Get current weather".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                }
            }),
        }],
        choice: Some(ToolChoice::Tool {
            name: "lookup_weather".to_string(),
        }),
    };

    let tools = gemini_tools(&cfg).expect("gemini tools");
    assert_eq!(
        tools,
        vec![json!([{
            "functionDeclarations": [{
                "name": "lookup_weather",
                "description": "Get current weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                }
            }]
        }])[0]
            .clone()]
    );

    let tool_config = gemini_tool_config(cfg.choice.as_ref()).expect("gemini tool config");
    assert_eq!(
        tool_config,
        json!({
            "functionCallingConfig": {
                "mode": "ANY",
                "allowedFunctionNames": ["lookup_weather"]
            }
        })
    );
}

#[test]
fn parses_tool_calls_from_xml_wrapped_json_blocks() {
    let raw = r#"<|im_start|>assistant
<tool_call>
{"name": "write_summary", "arguments": {"summary": "short recap"}}
</tool_call>"#;

    let calls = parse_tool_calls_from_text(raw);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "write_summary");
    assert_eq!(calls[0].arguments, json!({ "summary": "short recap" }));
}

#[test]
fn strips_tool_call_blocks_from_text_output() {
    let raw = r#"<|im_start|>assistant
Before
<tool_call>
{"name":"create_memory","arguments":{"text":"Likes tea"}}
</tool_call>
After<|im_end|>"#;

    assert_eq!(strip_tool_call_blocks(raw), "Before\n\nAfter");
}

#[test]
fn parses_tool_calls_from_plural_wrapper() {
    let raw = r#"<tool_calls>
[
  {"name":"create_memory","arguments":{"text":"Likes tea","category":"preference"}},
  {"function":{"name":"pin_memory","arguments":{"id":"abc"}}}
]
</tool_calls>"#;

    let calls = parse_tool_calls_from_text(raw);

    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "create_memory");
    assert_eq!(calls[1].name, "pin_memory");
}

#[test]
fn parses_tool_calls_from_json_tool_calls_object() {
    let raw = r#"{"tool_calls":[{"id":"call_1","function":{"name":"write_summary","arguments":"{\"summary\":\"done\"}"}}]}"#;

    let calls = parse_tool_calls_from_text(raw);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_1");
    assert_eq!(calls[0].name, "write_summary");
    assert_eq!(calls[0].arguments, json!({ "summary": "done" }));
}

#[test]
fn parses_standalone_function_tag_calls() {
    let raw = r#"<function=create_memory>{"text":"Likes tea","category":"preference"}</function>"#;

    let calls = parse_tool_calls_from_text(raw);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "create_memory");
    assert_eq!(
        calls[0].arguments,
        json!({ "text": "Likes tea", "category": "preference" })
    );
}

#[test]
fn parses_openai_tool_call_parameter_tags_into_arguments() {
    let payload = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "text_tool_call_1",
                    "type": "function",
                    "function": {
                        "name": "create_memory",
                        "arguments": "<parameter=category>\ncharacter_trait\n</parameter>\n<parameter=important>\ntrue\n</parameter>\n<parameter=text>\nMirelle is a ledger expert from House Cendre.\n</parameter>"
                    }
                }]
            }
        }]
    });

    let calls = parse_tool_calls("llamacpp", &payload);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "create_memory");
    assert_eq!(
        calls[0].arguments,
        json!({
            "category": "character_trait",
            "important": true,
            "text": "Mirelle is a ledger expert from House Cendre."
        })
    );
}

#[test]
fn parses_openai_summary_parameter_tag_argument() {
    let payload = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "text_tool_call_1",
                    "type": "function",
                    "function": {
                        "name": "write_summary",
                        "arguments": "<parameter=summary>\nshort recap\n</parameter>"
                    }
                }]
            }
        }]
    });

    let calls = parse_tool_calls("llamacpp", &payload);

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "write_summary");
    assert_eq!(calls[0].arguments, json!({ "summary": "short recap" }));
}
