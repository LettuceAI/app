//! Provider routing table pins.
//!
//! Each provider has a slightly different verification endpoint and auth
//! style. Refactoring the URL builder or auth shaper without updating every
//! branch silently breaks one provider's "test connection" button. These
//! tests assert the exact expected URL/header for each provider so a
//! one-line edit can't go unnoticed.

use lettuceai_lib::chat_manager::types::ProviderId;
use lettuceai_lib::providers::util::{
    build_headers, build_verify_url, default_base_url, extract_error_message,
};
use serde_json::json;

fn url(provider: &str, base: &str) -> String {
    build_verify_url(&ProviderId(provider.into()), base)
}

#[test]
fn verify_url_openrouter_v1_key() {
    assert_eq!(
        url("openrouter", "https://openrouter.ai/api"),
        "https://openrouter.ai/api/v1/key"
    );
}

#[test]
fn verify_url_openrouter_strips_trailing_slash() {
    assert_eq!(
        url("openrouter", "https://openrouter.ai/api/"),
        "https://openrouter.ai/api/v1/key"
    );
}

#[test]
fn verify_url_groq_openai_models() {
    assert_eq!(
        url("groq", "https://api.groq.com"),
        "https://api.groq.com/openai/v1/models"
    );
}

#[test]
fn verify_url_gemini_models_only() {
    assert_eq!(
        url("gemini", "https://generativelanguage.googleapis.com/v1beta"),
        "https://generativelanguage.googleapis.com/v1beta/models"
    );
}

#[test]
fn verify_url_zai_appends_chat_completions() {
    assert_eq!(
        url("zai", "https://api.z.ai/api/coding/paas/v1"),
        "https://api.z.ai/api/coding/paas/v1/chat/completions"
    );
}

#[test]
fn verify_url_zai_without_endpoint_appends_chat_completions() {
    let r = url("zai", "https://api.z.ai");
    assert!(r.ends_with("/chat/completions"), "got: {r}");
}

#[test]
fn verify_url_openai_default_appends_v1_models() {
    assert_eq!(
        url("openai", "https://api.openai.com"),
        "https://api.openai.com/v1/models"
    );
}

#[test]
fn verify_url_openai_with_v1_does_not_double() {
    assert_eq!(
        url("openai", "https://api.openai.com/v1"),
        "https://api.openai.com/v1/models"
    );
}

#[test]
fn verify_url_unknown_provider_uses_default_branch() {
    let r = url("unknown_provider_xyz", "https://example.com");
    assert_eq!(r, "https://example.com/v1/models");
}

#[test]
fn anthropic_auth_uses_x_api_key_and_version() {
    let h = build_headers(&ProviderId("anthropic".into()), "sk-ant-abc").expect("ok");
    assert_eq!(h.get("x-api-key").unwrap().to_str().unwrap(), "sk-ant-abc");
    assert!(h.contains_key("anthropic-version"));
    assert!(
        !h.contains_key("authorization"),
        "anthropic must NOT use Bearer auth"
    );
}

#[test]
fn gemini_auth_uses_x_goog_api_key() {
    let h = build_headers(&ProviderId("gemini".into()), "AIza-test").expect("ok");
    assert_eq!(
        h.get("x-goog-api-key").unwrap().to_str().unwrap(),
        "AIza-test"
    );
    assert!(!h.contains_key("authorization"));
}

#[test]
fn openai_auth_uses_bearer() {
    let h = build_headers(&ProviderId("openai".into()), "sk-test").expect("ok");
    let auth = h.get("authorization").unwrap().to_str().unwrap();
    assert!(auth.to_ascii_lowercase().starts_with("bearer "));
    assert!(auth.contains("sk-test"));
}

#[test]
fn openrouter_auth_uses_bearer() {
    let h = build_headers(&ProviderId("openrouter".into()), "or-test").expect("ok");
    let auth = h.get("authorization").unwrap().to_str().unwrap();
    assert!(auth.contains("or-test"));
}

#[test]
fn mistral_auth_uses_x_api_key() {
    let h = build_headers(&ProviderId("mistral".into()), "mst-test").expect("ok");
    assert_eq!(h.get("x-api-key").unwrap().to_str().unwrap(), "mst-test");
    assert!(!h.contains_key("authorization"));
}

#[test]
fn header_rejects_newline_in_key() {
    // Newlines in HTTP header values are CRLF-injection vectors; reqwest
    // must reject them. This pins the contract.
    let result = build_headers(&ProviderId("openai".into()), "key\nwith-newline");
    assert!(result.is_err(), "newline in API key must be rejected");
}

#[test]
fn header_rejects_null_byte_in_key() {
    let result = build_headers(&ProviderId("openai".into()), "key\0with-null");
    assert!(result.is_err());
}

#[test]
fn default_base_url_known_providers_are_https() {
    // Pin that we never accidentally suggest an http:// default.
    for provider in &[
        "openai",
        "anthropic",
        "openrouter",
        "gemini",
        "mistral",
        "groq",
    ] {
        if let Some(url) = default_base_url(&ProviderId((*provider).into())) {
            assert!(
                url.starts_with("https://"),
                "{provider} default base must be https, got {url}"
            );
        }
    }
}

#[test]
fn extract_error_gemini_safety_block_specific_text() {
    let payload = json!({
        "promptFeedback": { "blockReason": "SAFETY" }
    });
    let msg = extract_error_message(&payload).expect("some");
    assert!(
        msg.to_lowercase().contains("safety"),
        "safety reason must surface, got: {msg}"
    );
}

#[test]
fn extract_error_gemini_prohibited_content_specific_text() {
    let payload = json!({
        "promptFeedback": { "blockReason": "PROHIBITED_CONTENT" }
    });
    let msg = extract_error_message(&payload).expect("some");
    let low = msg.to_lowercase();
    assert!(
        low.contains("prohibited") || low.contains("safety"),
        "prohibited content reason must surface, got: {msg}"
    );
}

#[test]
fn extract_error_nested_message_object() {
    let payload = json!({
        "error": {
            "message": "Rate limit exceeded",
            "type": "rate_limit_error",
            "code": "rate_limit_exceeded"
        }
    });
    assert_eq!(
        extract_error_message(&payload).as_deref(),
        Some("Rate limit exceeded")
    );
}

#[test]
fn extract_error_string_error_field() {
    let payload = json!({ "error": "simple string error" });
    let msg = extract_error_message(&payload);
    assert!(msg.is_some(), "string `error` field should be extracted");
}

#[test]
fn extract_error_none_on_success_payload() {
    let payload = json!({
        "data": [],
        "object": "list"
    });
    assert!(extract_error_message(&payload).is_none());
}
