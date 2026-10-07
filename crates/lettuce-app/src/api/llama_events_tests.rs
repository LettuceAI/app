use std::sync::Arc;

use super::tests::{RecordingStream, Reply, harness, launch, send};
use super::ConversationGenerationWorker;

fn event_types(stream: &RecordingStream) -> Vec<String> {
    stream.events().iter().map(|event| {
        serde_json::to_value(event).expect("event")["type"].as_str().expect("type").to_owned()
    }).collect()
}

#[tokio::test]
async fn group_speaker_events_precede_the_first_delta() {
    let harness = harness(Reply::Text("Hello."));
    let cast = super::turns_tests::group_cast(&harness, "live-speaker").await;
    let stream = Arc::new(RecordingStream::default());
    send(&harness, &cast.chat, "live-speaker-send", "Hi all", stream.clone()).await.expect("send");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation");
    let types = event_types(&stream);
    let selecting = types.iter().position(|value| value == "speaker_selecting").expect("selecting event");
    let selected = types.iter().position(|value| value == "speaker_selected").expect("selected event");
    let delta = types.iter().position(|value| value == "delta").expect("delta");
    assert!(selecting < selected && selected < delta, "{types:?}");
}

#[tokio::test]
async fn direct_speaker_event_precedes_the_first_delta_without_selection() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "live-direct").await;
    let stream = Arc::new(RecordingStream::default());
    send(&harness, &chat, "live-direct-send", "Hi", stream.clone()).await.expect("send");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation");
    let types = event_types(&stream);
    assert!(!types.iter().any(|value| value == "speaker_selecting"));
    let selected = types.iter().position(|value| value == "speaker_selected").expect("selected event");
    let delta = types.iter().position(|value| value == "delta").expect("delta");
    assert!(selected < delta, "{types:?}");
}

#[tokio::test]
async fn a_late_attach_receives_the_resolved_speaker() {
    let harness = harness(Reply::UntilCancelled);
    let chat = launch(&harness, "late-speaker").await;
    let first = Arc::new(RecordingStream::default());
    let accepted = send(&harness, &chat, "late-speaker-send", "Hi", first).await.expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let running = tokio::spawn(async move { worker.run_once().await });
    harness.provider.entered.notified().await;
    let late = Arc::new(RecordingStream::default());
    harness.context.attach_stream(accepted.turn_id.parse().expect("turn"), late.clone());
    let events = late.events();
    assert!(matches!(&events[0], lettuce_contracts::GenerationEvent::SpeakerSelected { turn_id, character_id } if turn_id == &accepted.turn_id && character_id == &harness.character_id.to_string()));
    super::generation_cancel(&harness.context, lettuce_contracts::GenerationCancelRequest { turn_id: accepted.turn_id }).await.expect("cancel");
    running.await.expect("worker task").expect("worker");
}

#[tokio::test]
async fn a_director_turn_emits_selected_without_selecting() {
    use lettuce_characters::GroupRepository;
    let harness = harness(Reply::Text("Hello."));
    let cast = super::turns_tests::group_cast(&harness, "live-director").await;
    let first = Arc::new(RecordingStream::default());
    send(&harness, &cast.chat, "live-director-send", "Hi", first).await.expect("send");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation");
    let database = harness.context.backend().database();
    let group = GroupRepository::get(database, cast.group_id).expect("group").expect("present");
    GroupRepository::set_speaker_selection(database, cast.group_id, group.group.revision, lettuce_characters::SpeakerSelection::Director, lettuce_types::TimestampMillis::now().expect("now")).expect("director");
    let mut request = super::turns_tests::continue_request(&harness, &cast.chat, "live-director-continue");
    request.forced_speaker_participant_id = Some(cast.ada.to_string());
    let stream = Arc::new(RecordingStream::default());
    super::conversation_continue(&harness.context, request, stream.clone()).await.expect("continue");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation");
    let types = event_types(&stream);
    assert!(!types.iter().any(|value| value == "speaker_selecting"));
    let selected = types.iter().position(|value| value == "speaker_selected").expect("selected event");
    let delta = types.iter().position(|value| value == "delta").expect("delta");
    assert!(selected < delta, "{types:?}");
}

#[tokio::test]
async fn a_mentioned_character_skips_selection_status() {
    let harness = harness(Reply::Text("Hello."));
    let cast = super::turns_tests::group_cast(&harness, "live-mention").await;
    let stream = Arc::new(RecordingStream::default());
    send(&harness, &cast.chat, "live-mention-send", "@Cleo hello", stream.clone()).await.expect("send");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation");
    let types = event_types(&stream);
    assert!(!types.iter().any(|value| value == "speaker_selecting"));
    assert!(stream.events().iter().any(|event| matches!(event, lettuce_contracts::GenerationEvent::SpeakerSelected { character_id, .. } if character_id == &cast.cleo_character.to_string())));
}
