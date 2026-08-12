//! Gathered from inline tests in src/creation_helper/agent/exec.rs.

use lettuceai_lib::creation_helper::agent::exec::parse_args;

#[test]
fn parses_simple_kv() {
    let m = parse_args("id=sc_2 tone=darker");
    assert_eq!(m.get("id").unwrap(), "sc_2");
    assert_eq!(m.get("tone").unwrap(), "darker");
}

#[test]
fn parses_quoted_value() {
    let m = parse_args(r#"prompt="weather-beaten pirate" role=avatar"#);
    assert_eq!(m.get("prompt").unwrap(), "weather-beaten pirate");
    assert_eq!(m.get("role").unwrap(), "avatar");
}

#[test]
fn captures_bare_text_as_note() {
    let m = parse_args("flesh out a sarcastic pirate captain");
    assert_eq!(
        m.get("note").unwrap(),
        "flesh out a sarcastic pirate captain"
    );
}
