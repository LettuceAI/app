//! Gathered from inline tests in src/creation_helper/agent/structured_fallback.rs.

use lettuceai_lib::creation_helper::agent::structured_fallback::{
    parse, CreationHelperFallbackFormat,
};

#[test]
fn parses_json_calls() {
    let raw = r#"{"calls":[{"name":"SET_NAME","arguments":{"name":"Mira"}},{"name":"WRITE_DEFINITION","arguments":{"definition":"A pirate."}}]}"#;
    let parsed = parse(CreationHelperFallbackFormat::Json, raw).unwrap();
    assert_eq!(parsed.calls.len(), 2);
    assert_eq!(parsed.calls[0].name, "SET_NAME");
}

#[test]
fn extracts_reply_from_json() {
    let raw = r#"{"calls":[{"name":"reply","arguments":{"message":"Done."}}]}"#;
    let parsed = parse(CreationHelperFallbackFormat::Json, raw).unwrap();
    assert!(parsed.calls.is_empty());
    assert_eq!(parsed.reply.as_deref(), Some("Done."));
}

#[test]
fn parses_xml_calls() {
    let raw = r#"<calls>
        <call name="SET_NAME"><arg name="name">Mira</arg></call>
        <call name="reply"><arg name="message">Hi</arg></call>
    </calls>"#;
    let parsed = parse(CreationHelperFallbackFormat::Xml, raw).unwrap();
    assert_eq!(parsed.calls.len(), 1);
    assert_eq!(parsed.reply.as_deref(), Some("Hi"));
}

#[test]
fn tolerates_markdown_fence_and_preamble() {
    let raw = "Sure!\n```json\n{\"calls\":[{\"name\":\"DONE\",\"arguments\":{}}]}\n```";
    let parsed = parse(CreationHelperFallbackFormat::Json, raw).unwrap();
    assert_eq!(parsed.calls.len(), 1);
    assert_eq!(parsed.calls[0].name, "DONE");
}
