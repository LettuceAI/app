//! Gathered from inline tests in src/tts_manager/kokoro/phonemizer.rs.

use lettuceai_lib::tts_manager::kokoro::phonemizer::{
    apply_stress_delta, normalize_input_text, split_text_parts, TextPart,
};

#[test]
fn parses_inline_phoneme_annotations() {
    let parts = split_text_parts("[Kokoro](/kˈOkəɹO/) is here.");
    assert_eq!(
        parts,
        vec![
            TextPart::Phonemes("kˈOkəɹO".to_string()),
            TextPart::Space,
            TextPart::Text("is".to_string()),
            TextPart::Space,
            TextPart::Text("here".to_string()),
            TextPart::Punct('.'),
        ]
    );
}

#[test]
fn parses_inline_stress_annotations() {
    let parts = split_text_parts("Try [or](+2) now.");
    assert_eq!(
        parts,
        vec![
            TextPart::Text("Try".to_string()),
            TextPart::Space,
            TextPart::StressText {
                text: "or".to_string(),
                delta: 2,
            },
            TextPart::Space,
            TextPart::Text("now".to_string()),
            TextPart::Punct('.'),
        ]
    );
}

#[test]
fn preserves_numeric_connectors() {
    let parts = split_text_parts("82 million parameters.");
    assert_eq!(
        parts,
        vec![
            TextPart::Text("82".to_string()),
            TextPart::Space,
            TextPart::Text("million".to_string()),
            TextPart::Space,
            TextPart::Text("parameters".to_string()),
            TextPart::Punct('.'),
        ]
    );
}

#[test]
fn raises_unstressed_segment() {
    assert_eq!(apply_stress_delta("ɔɹ", 2), "ˈɔɹ");
}

#[test]
fn lowers_primary_stress() {
    assert_eq!(apply_stress_delta("kˈOkəɹO", -1), "kˌOkəɹO");
    assert_eq!(apply_stress_delta("kˈOkəɹO", -2), "kOkəɹO");
}

#[test]
fn strips_markdown_emphasis_markers() {
    assert_eq!(
        normalize_input_text("Oh you *love* these pancakes"),
        "Oh you love these pancakes"
    );
    assert_eq!(
        normalize_input_text("This is **very** good."),
        "This is very good."
    );
}

#[test]
fn preserves_non_markdown_asterisks_and_annotations() {
    assert_eq!(normalize_input_text("2 * 3 = 6"), "2 * 3 = 6");
    assert_eq!(
        normalize_input_text("Try [or](+2) now."),
        "Try [or](+2) now."
    );
}
