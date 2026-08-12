//! Gathered from inline tests in src/chat_manager/thinking.rs.

use lettuceai_lib::chat_manager::thinking::{
    normalize_thinking_content, split_thinking_tags, ThinkingTagStreamParser,
};

#[test]
fn splits_complete_think_block() {
    let split = split_thinking_tags("Hello<think>hidden</think>world");
    assert_eq!(split.content, "Helloworld");
    assert_eq!(split.reasoning, "hidden");
}

#[test]
fn splits_streamed_think_block_with_fragmented_tags() {
    let mut parser = ThinkingTagStreamParser::default();

    let a = parser.feed("Hello<th");
    let b = parser.feed("ink>hid");
    let c = parser.feed("den</th");
    let d = parser.feed("ink>world");
    let tail = parser.finish();

    assert_eq!(a.content, "Hello");
    assert_eq!(a.reasoning, "");
    assert_eq!(b.content, "");
    assert_eq!(b.reasoning, "hid");
    assert_eq!(c.reasoning, "den");
    assert_eq!(d.content, "world");
    assert_eq!(tail.content, "");
    assert_eq!(tail.reasoning, "");
}

#[test]
fn merges_explicit_reasoning_with_tag_reasoning_without_duplication() {
    let split = normalize_thinking_content(Some("<think>alpha</think>done"), Some("alpha"));
    assert_eq!(split.content, "done");
    assert_eq!(split.reasoning, "alpha");
}

#[test]
fn supports_reasoning_tag_aliases() {
    let split =
        split_thinking_tags("visible<reasoning>hidden</reasoning><thinking>more</thinking>end");
    assert_eq!(split.content, "visibleend");
    assert_eq!(split.reasoning, "hiddenmore");
}

#[test]
fn supports_fragmented_reason_alias_streaming() {
    let mut parser = ThinkingTagStreamParser::default();

    let a = parser.feed("Hello<rea");
    let b = parser.feed("son>hidden");
    let c = parser.feed("</reason>world");
    let tail = parser.finish();

    assert_eq!(a.content, "Hello");
    assert_eq!(a.reasoning, "");
    assert_eq!(b.content, "");
    assert_eq!(b.reasoning, "hidden");
    assert_eq!(c.content, "world");
    assert_eq!(c.reasoning, "");
    assert_eq!(tail.content, "");
    assert_eq!(tail.reasoning, "");
}

#[test]
fn supports_mixed_case_tags() {
    let split = split_thinking_tags("visible<THINKING>hidden</THINKING><ReAsOn>more</ReAsOn>end");
    assert_eq!(split.content, "visibleend");
    assert_eq!(split.reasoning, "hiddenmore");
}

#[test]
fn supports_unicode_before_partial_tag_boundaries() {
    let mut parser = ThinkingTagStreamParser::default();

    let a = parser.feed("“<th");
    let b = parser.feed("ink>hidden</think>");
    let tail = parser.finish();

    assert_eq!(a.content, "“");
    assert_eq!(a.reasoning, "");
    assert_eq!(b.content, "");
    assert_eq!(b.reasoning, "hidden");
    assert_eq!(tail.content, "");
    assert_eq!(tail.reasoning, "");
}
