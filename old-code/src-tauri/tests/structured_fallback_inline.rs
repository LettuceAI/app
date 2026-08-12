//! Gathered from inline tests in src/chat_manager/memory/structured_fallback.rs.

use lettuceai_lib::chat_manager::memory::structured_fallback::{
    parse_memory_operations_from_text, parse_memory_tag_repairs_from_text,
};
use lettuceai_lib::chat_manager::types::DynamicMemoryStructuredFallbackFormat;
use serde_json::json;

#[test]
fn parses_operations_with_wrapper_text_and_entities() {
    let raw = r#"Here you go:
```xml
<memory_ops>
  <create_memory important="true">
<text>Sam &amp; Elias reconciled</text>
<category>relationship</category>
  </create_memory>
  <done summary="all set" />
</memory_ops>
```"#;

    let calls = parse_memory_operations_from_text(raw, DynamicMemoryStructuredFallbackFormat::Xml)
        .expect("should parse xml");

    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "create_memory");
    assert_eq!(
        calls[0].arguments,
        json!({
            "important": true,
            "text": "Sam & Elias reconciled",
            "category": "relationship"
        })
    );
    assert_eq!(calls[1].name, "done");
    assert_eq!(calls[1].arguments, json!({ "summary": "all set" }));
}

#[test]
fn parses_repairs_with_prose_around_xml() {
    let raw = r#"I fixed the categories.
<memory_repairs>
  <item>
<text>Likes tea</text>
<category>preference</category>
  </item>
</memory_repairs>"#;

    let repaired = parse_memory_tag_repairs_from_text(
        raw,
        &["preference", "other"],
        DynamicMemoryStructuredFallbackFormat::Xml,
    )
    .expect("should parse repairs");

    assert_eq!(repaired.get("Likes tea"), Some(&"preference".to_string()));
}

#[test]
fn parses_attribute_entities() {
    let raw = r#"<memory_ops><done summary="Sam &amp; Elias synced" /></memory_ops>"#;

    let calls = parse_memory_operations_from_text(raw, DynamicMemoryStructuredFallbackFormat::Xml)
        .expect("should parse xml");

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].arguments,
        json!({ "summary": "Sam & Elias synced" })
    );
}

#[test]
fn parses_operations_from_wrapped_json() {
    let raw = r#"Answer:
```json
{"operations":[{"name":"create_memory","arguments":{"text":"Sam apologized","category":"plot_event","important":true}},{"name":"done","arguments":{"summary":"captured"}}]}
```"#;

    let calls = parse_memory_operations_from_text(raw, DynamicMemoryStructuredFallbackFormat::Json)
        .expect("should parse json");

    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "create_memory");
    assert_eq!(
        calls[0].arguments,
        json!({
            "text": "Sam apologized",
            "category": "plot_event",
            "important": true
        })
    );
    assert_eq!(calls[1].name, "done");
}

#[test]
fn parses_repairs_from_json() {
    let raw = r#"{"items":[{"text":"Likes tea","category":"preference"}]}"#;

    let repaired = parse_memory_tag_repairs_from_text(
        raw,
        &["preference", "other"],
        DynamicMemoryStructuredFallbackFormat::Json,
    )
    .expect("should parse repairs");

    assert_eq!(repaired.get("Likes tea"), Some(&"preference".to_string()));
}
