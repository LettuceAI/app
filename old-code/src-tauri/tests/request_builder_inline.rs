//! Gathered from inline tests in src/chat_manager/prompting/request_builder.rs.

use lettuceai_lib::chat_manager::prompting::request_builder::*;
use lettuceai_lib::chat_manager::types::ProviderCredential;
use serde_json::{json, Value};
use std::collections::HashMap;

fn credential(provider_id: &str) -> ProviderCredential {
    ProviderCredential {
        id: format!("{provider_id}-cred"),
        provider_id: provider_id.to_string(),
        label: provider_id.to_string(),
        api_key: Some("test-key".to_string()),
        base_url: None,
        default_model: None,
        headers: None,
        config: None,
    }
}

#[test]
fn strips_llama_fields_from_non_llama_requests() {
    let credential = credential("mistral");
    let extra_body_fields = HashMap::from([
        ("llamaSamplerOrder".to_string(), json!(["top_k"])),
        (
            "llamaDisableSamplerProfileDefaults".to_string(),
            json!(true),
        ),
        (
            "options".to_string(),
            json!({"num_ctx": 4096, "mirostat": 2}),
        ),
        ("min_p".to_string(), json!(0.0)),
        ("typical_p".to_string(), json!(0.0)),
        ("parallel_tool_calls".to_string(), json!(true)),
    ]);

    let built = build_chat_request(
        &credential,
        "test-key",
        "mistral-small-latest",
        &vec![json!({"role": "user", "content": "hello"})],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        Some(40),
        None,
        false,
        None,
        None,
        false,
        Some(extra_body_fields),
    );

    let body = built
        .body
        .as_object()
        .expect("request body should be an object");
    assert!(!body.contains_key("llamaSamplerOrder"));
    assert!(!body.contains_key("llamaDisableSamplerProfileDefaults"));
    assert!(!body.contains_key("options"));
    assert!(!body.contains_key("min_p"));
    assert!(!body.contains_key("typical_p"));
    assert!(!body.contains_key("parallel_tool_calls"));
}

#[test]
fn strips_internal_message_fields_from_non_system_messages() {
    let credential = credential("mistral");

    let built = build_chat_request(
        &credential,
        "test-key",
        "mistral-small-latest",
        &vec![
            json!({
                "role": "user",
                "content": "hello",
                "visible_in_chat": true,
                "uiExpanded": true
            }),
            json!({
                "role": "assistant",
                "content": "hi",
                "visible_in_chat": false
            }),
        ],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        None,
    );

    let messages = built
        .body
        .get("messages")
        .and_then(Value::as_array)
        .expect("request body should contain messages");

    let user_message = messages[0]
        .as_object()
        .expect("user message should be an object");
    assert_eq!(user_message.get("role"), Some(&json!("user")));
    assert_eq!(user_message.get("content"), Some(&json!("hello")));
    assert!(!user_message.contains_key("visible_in_chat"));
    assert!(!user_message.contains_key("uiExpanded"));

    let assistant_message = messages[1]
        .as_object()
        .expect("assistant message should be an object");
    assert_eq!(assistant_message.get("role"), Some(&json!("assistant")));
    assert_eq!(assistant_message.get("content"), Some(&json!("hi")));
    assert!(!assistant_message.contains_key("visible_in_chat"));
}

#[test]
fn strips_visible_system_message_metadata_for_raw_message_adapters() {
    let credential = credential("mistral");

    let built = build_chat_request(
        &credential,
        "test-key",
        "mistral-small-latest",
        &vec![json!({
            "role": "system",
            "content": "Stay in character.",
            "visible_in_chat": true,
            "uiExpanded": true
        })],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        None,
    );

    let messages = built
        .body
        .get("messages")
        .and_then(Value::as_array)
        .expect("request body should contain messages");
    let system_message = messages[0]
        .as_object()
        .expect("system message should be an object");
    assert_eq!(system_message.get("role"), Some(&json!("system")));
    assert!(!system_message.contains_key("visible_in_chat"));
    assert!(!system_message.contains_key("uiExpanded"));
}

#[test]
fn strips_visible_system_message_metadata_for_translating_adapters_too() {
    let credential = credential("anthropic");

    let built = build_chat_request(
        &credential,
        "test-key",
        "claude-sonnet",
        &vec![json!({
            "role": "system",
            "content": "Stay in character.",
            "visible_in_chat": true,
            "uiExpanded": true
        })],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        None,
    );

    let messages = built
        .body
        .get("messages")
        .and_then(Value::as_array)
        .expect("request body should contain messages");
    assert!(messages.is_empty());
    assert_eq!(built.body.get("system"), Some(&json!("Stay in character.")));
    let serialized = built.body.to_string();
    assert!(!serialized.contains("visible_in_chat"));
    assert!(!serialized.contains("uiExpanded"));
}

#[test]
fn keeps_llama_fields_for_llamacpp_requests() {
    let credential = credential("llamacpp");
    let extra_body_fields = HashMap::from([
        ("llamaSamplerOrder".to_string(), json!(["top_k"])),
        (
            "llamaDisableSamplerProfileDefaults".to_string(),
            json!(true),
        ),
    ]);

    let built = build_chat_request(
        &credential,
        "test-key",
        "local-model",
        &vec![json!({"role": "user", "content": "hello"})],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        Some(extra_body_fields),
    );

    let body = built
        .body
        .as_object()
        .expect("request body should be an object");
    assert_eq!(body.get("llamaSamplerOrder"), Some(&json!(["top_k"])));
    assert_eq!(
        body.get("llamaDisableSamplerProfileDefaults"),
        Some(&json!(true))
    );
}

#[test]
fn keeps_ollama_options_for_ollama_requests() {
    let credential = credential("ollama");
    let extra_body_fields = HashMap::from([(
        "options".to_string(),
        json!({"num_ctx": 4096, "mirostat": 2}),
    )]);

    let built = build_chat_request(
        &credential,
        "test-key",
        "local-model",
        &vec![json!({"role": "user", "content": "hello"})],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        Some(extra_body_fields),
    );

    let body = built
        .body
        .as_object()
        .expect("request body should be an object");
    assert_eq!(
        body.get("options"),
        Some(&json!({"num_ctx": 4096, "mirostat": 2}))
    );
}

#[test]
fn keeps_openai_prompt_cache_retention_internal_key() {
    let credential = credential("openai");
    let extra_body_fields = HashMap::from([("promptCachingTtl".to_string(), json!("24h"))]);

    let built = build_chat_request(
        &credential,
        "test-key",
        "gpt-5",
        &vec![json!({"role": "user", "content": "hello"})],
        None,
        Some(0.7),
        Some(0.95),
        128,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        true,
        Some(extra_body_fields),
    );

    let body = built
        .body
        .as_object()
        .expect("request body should be an object");
    assert_eq!(body.get("prompt_cache_retention"), Some(&json!("24h")));
    assert!(!body.contains_key("promptCachingTtl"));
}
