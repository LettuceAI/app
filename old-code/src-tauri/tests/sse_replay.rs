//! SSE decoder stream replay: feed the same byte stream at varying chunk
//! boundaries and assert the reconstructed output is identical to the
//! single-chunk parse. This catches regressions in the partial-line buffer.

use lettuceai_lib::chat_manager::sse::SseDecoder;
use lettuceai_lib::chat_manager::types::NormalizedEvent;

fn reconstruct(stream: &str, chunk_size: usize, provider: &str) -> (String, String, bool) {
    let mut dec = SseDecoder::new();
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut saw_done = false;

    let bytes = stream.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let next = (cursor + chunk_size).min(bytes.len());
        // Snap to char boundary so we never split a UTF-8 code point.
        let mut end = next;
        while end < bytes.len() && (bytes[end] & 0b1100_0000) == 0b1000_0000 {
            end += 1;
        }
        let chunk = std::str::from_utf8(&bytes[cursor..end]).expect("valid utf8 boundary");
        for ev in dec.feed(chunk, Some(provider)) {
            match ev {
                NormalizedEvent::Delta { text: t } => text.push_str(&t),
                NormalizedEvent::Reasoning { text: t } => reasoning.push_str(&t),
                NormalizedEvent::Done => saw_done = true,
                _ => {}
            }
        }
        cursor = end;
    }
    (text, reasoning, saw_done)
}

fn openai_stream(parts: &[&str]) -> String {
    let mut out = String::new();
    for p in parts {
        let escaped = p.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{escaped}\"}}}}]}}\n\n"
        ));
    }
    out.push_str("data: [DONE]\n\n");
    out
}

#[test]
fn openai_stream_reassembles_identically_across_chunk_sizes() {
    let stream = openai_stream(&["Hello", ", ", "world", "!", " How are you?"]);
    let expected = "Hello, world! How are you?";

    let (single_chunk, _, single_done) = reconstruct(&stream, stream.len(), "openai");
    assert_eq!(single_chunk, expected, "baseline parse must match expected");
    assert!(single_done, "baseline must emit Done");

    for chunk_size in [1, 2, 3, 5, 7, 11, 16, 32, 64, 128] {
        let (got, _, done) = reconstruct(&stream, chunk_size, "openai");
        assert_eq!(
            got, expected,
            "chunk_size={chunk_size}: stream output should be invariant"
        );
        assert!(done, "chunk_size={chunk_size}: must emit Done");
    }
}

#[test]
fn openai_stream_with_unicode_split_safely() {
    let stream = openai_stream(&["héllo", " 你好", " 🚀"]);
    let expected = "héllo 你好 🚀";

    for chunk_size in [1, 3, 5, 7, 13, 64] {
        let (got, _, _) = reconstruct(&stream, chunk_size, "openai");
        assert_eq!(got, expected, "chunk_size={chunk_size}: unicode preserved");
    }
}

#[test]
fn openai_stream_split_inside_data_prefix() {
    // Deliberately tricky: the "data: " prefix is multiple bytes, parser must
    // wait for the rest of the line before parsing JSON.
    let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\ndata: [DONE]\n\n";
    for chunk_size in [1, 2, 4, 6] {
        let (got, _, done) = reconstruct(stream, chunk_size, "openai");
        assert_eq!(got, "x", "chunk_size={chunk_size}");
        assert!(done);
    }
}

#[test]
fn openai_stream_split_inside_json_value() {
    let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"abcdef\"}}]}\n\n";
    for chunk_size in [1, 3, 5, 9, 17] {
        let (got, _, _) = reconstruct(stream, chunk_size, "openai");
        assert_eq!(got, "abcdef", "chunk_size={chunk_size}");
    }
}

#[test]
fn openai_reasoning_field_accumulates() {
    let stream = "data: {\"choices\":[{\"delta\":{\"reasoning\":\"step1\"}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"reasoning\":\"step2\"}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"answer\"}}]}\n\n\
         data: [DONE]\n\n";

    for chunk_size in [1, 4, 16, 256] {
        let (text, reasoning, done) = reconstruct(stream, chunk_size, "openai");
        assert_eq!(text, "answer", "chunk_size={chunk_size}");
        assert!(reasoning.contains("step1"), "chunk_size={chunk_size}");
        assert!(reasoning.contains("step2"), "chunk_size={chunk_size}");
        assert!(done);
    }
}

#[test]
fn empty_stream_emits_nothing() {
    let mut dec = SseDecoder::new();
    let evts = dec.feed("", Some("openai"));
    assert!(evts.is_empty());
}

#[test]
fn keepalive_comment_lines_ignored() {
    // SSE spec: lines starting with ':' are comments / keepalives.
    let stream = ": keepalive ping\n\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n\
                  : another\n\n\
                  data: [DONE]\n\n";
    let (got, _, done) = reconstruct(stream, 16, "openai");
    assert_eq!(got, "x");
    assert!(done);
}

#[test]
fn gemini_stream_extracts_text_across_chunks() {
    let stream = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"part1\"}]}}]}\n\n\
         data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" part2\"}]}}]}\n\n";

    for chunk_size in [1, 5, 13, 64, 256] {
        let (got, _, _) = reconstruct(stream, chunk_size, "gemini");
        assert_eq!(got, "part1 part2", "chunk_size={chunk_size}");
    }
}

#[test]
fn malformed_json_in_stream_does_not_break_subsequent_parsing() {
    let stream = "data: not_valid_json\n\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n\
                  data: [DONE]\n\n";
    let (got, _, done) = reconstruct(stream, 16, "openai");
    assert_eq!(got, "ok", "decoder must recover after a malformed line");
    assert!(done);
}

#[test]
fn stream_with_crlf_line_endings_works() {
    let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\r\n\r\n\
                  data: [DONE]\r\n\r\n";
    let (got, _, _done) = reconstruct(stream, 32, "openai");
    assert_eq!(got, "a");
}
