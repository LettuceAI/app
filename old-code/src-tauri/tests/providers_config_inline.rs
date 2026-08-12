//! Gathered from inline tests in src/providers/config.rs.

use lettuceai_lib::chat_manager::types::ProviderId;
use lettuceai_lib::providers::config::resolve_base_url;

#[test]
fn test_resolve_base_url_with_custom() {
    let result = resolve_base_url(&ProviderId("openai".into()), Some("https://custom.com"));
    assert_eq!(result, "https://custom.com");
}

#[test]
fn test_resolve_base_url_default() {
    let result = resolve_base_url(&ProviderId("openai".into()), None);
    assert_eq!(result, "https://api.openai.com");
}
