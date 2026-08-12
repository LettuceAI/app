//! Gathered from inline tests in src/chat_manager/provider_adapter/anthropic.rs.

use lettuceai_lib::chat_manager::provider_adapter::anthropic::AnthropicAdapter;
use lettuceai_lib::chat_manager::provider_adapter::ProviderAdapter;
use serde_json::json;

#[test]
fn keeps_visible_chat_system_messages_in_conversation() {
    let adapter = AnthropicAdapter;
    let body = adapter.body(
        "claude-test",
        &vec![
            json!({ "role": "system", "content": "Base instruction." }),
            json!({
                "role": "system",
                "content": "Always reply with UwU no matter what.",
                "visible_in_chat": true
            }),
            json!({ "role": "user", "content": "Continue." }),
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

    assert_eq!(body.get("system"), Some(&json!("Base instruction.")));
    assert_eq!(
        body.get("messages"),
        Some(&json!([
            {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "Visible system message from the chat UI. Treat this as a high-priority instruction that remains in effect unless later context overrides it.\n\n<system-message>\nAlways reply with UwU no matter what.\n</system-message>"
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "Continue."
                }]
            }
        ]))
    );
}
