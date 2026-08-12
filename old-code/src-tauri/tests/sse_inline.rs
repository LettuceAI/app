//! Gathered from inline tests in src/chat_manager/sse.rs.

use lettuceai_lib::chat_manager::sse::SseDecoder;
use lettuceai_lib::chat_manager::types::NormalizedEvent;

#[test]
fn ollama_reasoning_stream_preserves_leading_spaces() {
    let mut decoder = SseDecoder::new();

    let first = decoder.feed(
        "{\"message\":{\"thinking\":\"ThinkingProcess: 1.\"},\"done\":false}\n",
        Some("ollama"),
    );
    let second = decoder.feed(
        "{\"message\":{\"thinking\":\" Analyze the Request\"},\"done\":false}\n",
        Some("ollama"),
    );

    assert_eq!(first.len(), 1);
    match &first[0] {
        NormalizedEvent::Reasoning { text } => assert_eq!(text, "ThinkingProcess: 1."),
        other => panic!("unexpected first event: {other:?}"),
    }

    assert_eq!(second.len(), 1);
    match &second[0] {
        NormalizedEvent::Reasoning { text } => assert_eq!(text, " Analyze the Request"),
        other => panic!("unexpected second event: {other:?}"),
    }
}
