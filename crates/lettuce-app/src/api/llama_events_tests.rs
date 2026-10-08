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

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_local_turn_routes_notices_without_job_throughput_or_cross_turn_delivery() {
    use lettuce_contracts::GenerationEvent;
    let harness = harness(Reply::Text("Hello."));
    let model = crate::launch::tests::seed_model(harness.context.backend().database(), lettuce_models::ProviderProtocol::LlamaCpp, "llamacpp");
    crate::launch::tests::set_application_default_model(harness.context.backend().database(), model);
    harness.provider.runtime_events.store(true, std::sync::atomic::Ordering::Release);
    let chat = launch(&harness, "local-notice").await;
    let unrelated = lettuce_types::GenerationTurnId::new();
    let other = Arc::new(RecordingStream::default());
    harness.context.attach_stream(unrelated, other.clone());
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(&harness, &chat, "local-notice-send", "Hi", stream.clone()).await.expect("send");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation");
    assert!(stream.events().iter().any(|event| matches!(event, GenerationEvent::Notice { turn_id, code: lettuce_contracts::RuntimeNoticeCode::MtpDisabledForVision } if turn_id == &accepted.turn_id)));
    let types = event_types(&stream);
    let loading = types.iter().position(|kind| kind == "model_loading").expect("model loading");
    let notice = types.iter().position(|kind| kind == "notice").expect("notice");
    assert!(loading < notice);
    assert!(other.events().is_empty());
    let request = harness.provider.requests.lock().expect("requests")[0].clone();
    assert_eq!(request.profile.chat_profile.provider_protocol, lettuce_models::ProviderProtocol::LlamaCpp);
    let before = stream.events();
    harness.context.backend().local_runtime_events().emit(lettuce_local_llm::generation::LlamaHostEvent::Notice {
        request_id: Some(request.attempt_id.to_string()), notice: lettuce_local_llm::generation::LlamaNotice::KvCacheMovedToRam,
    });
    assert_eq!(stream.events(), before);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_cancelled_local_turn_stops_receiving_notices_while_inference_is_live() {
    use lettuce_contracts::GenerationEvent;
    let harness = harness(Reply::UntilCancelled);
    harness.provider.runtime_events.store(true, std::sync::atomic::Ordering::Release);
    let chat = launch(&harness, "cancel-local-notice").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(&harness, &chat, "cancel-local-notice-send", "Hi", stream.clone()).await.expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let running = tokio::spawn(async move { worker.run_once().await });
    harness.provider.entered.notified().await;
    let request = harness.provider.requests.lock().expect("requests")[0].clone();
    let count = stream.events().iter().filter(|event| matches!(event, GenerationEvent::Notice { .. } | GenerationEvent::ModelLoading { .. })).count();
    super::generation_cancel(&harness.context, lettuce_contracts::GenerationCancelRequest { turn_id: accepted.turn_id }).await.expect("cancel");
    harness.context.backend().local_runtime_events().emit(lettuce_local_llm::generation::LlamaHostEvent::Notice {
        request_id: Some(request.attempt_id.to_string()), notice: lettuce_local_llm::generation::LlamaNotice::KvCacheMovedToRam,
    });
    harness.context.backend().local_runtime_events().emit(lettuce_local_llm::generation::LlamaHostEvent::ModelLoadProgress(
        lettuce_local_llm::engine::ModelLoadProgress {
            request_id: Some(request.attempt_id.to_string()),
            model_path: "local-events.gguf".into(),
            model_name: "Local events".into(),
            backend_path: "cpu".into(),
            stage: lettuce_local_llm::engine::ModelLoadStage::Finalizing,
            status: lettuce_local_llm::engine::ModelLoadStatus::Loaded,
            progress: 1.0,
            percent: 100,
            gpus: None,
        },
    ));
    assert_eq!(stream.events().iter().filter(|event| matches!(event, GenerationEvent::Notice { .. } | GenerationEvent::ModelLoading { .. })).count(), count);
    running.await.expect("task").expect("generation");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn runtime_reports_emit_only_matching_local_model_ids_after_storage() {
    use lettuce_models::{ModelCatalog, ModelProfileRepository};
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let first = crate::launch::tests::seed_model(database, lettuce_models::ProviderProtocol::LlamaCpp, "llamacpp");
    let second = crate::launch::tests::seed_model(database, lettuce_models::ProviderProtocol::LlamaCpp, "llamacpp");
    let remote = crate::launch::tests::seed_model(database, lettuce_models::ProviderProtocol::Ollama, "ollama");
    let path = "local-events.gguf";
    for id in [first, second, remote] {
        let mut model = ModelProfileRepository::get(database, id).expect("model").expect("present");
        let revision = model.revision;
        model.external_model_id = path.into();
        ModelProfileRepository::upsert(database, model, Some(revision)).expect("path");
    }
    let routes = harness.context.backend().local_runtime_events().clone();
    let report = serde_json::json!({"context":4096});
    assert!(database.store_llama_runtime_report(path, &report, 1).expect("store"));
    use lettuce_local_llm::generation::LlamaHostEvent;
    routes.emit(LlamaHostEvent::RuntimeReportUpdated { model_path: path.into() });
    let events = super::tests::api_events(&harness);
    let ids = events.iter().find_map(|event| match event {
        lettuce_contracts::ApiEvent::LocalModelRuntimeReportChanged { model_ids } => Some(model_ids.clone()),
        _ => None,
    }).expect("report changed");
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&first.to_string()) && ids.contains(&second.to_string()));
    assert_eq!(database.llama_runtime_report(path).expect("read"), Some(report));
    routes.emit(LlamaHostEvent::RuntimeReportUpdated { model_path: "unknown.gguf".into() });
    routes.emit(LlamaHostEvent::Heartbeat { request_id: None, heartbeat: lettuce_local_llm::generation::GenerationHeartbeat { tokens: 4, elapsed_ms: 10, tokens_per_second: 400.0, recent_text: String::new() } });
    assert_eq!(super::tests::api_events(&harness), events);
    assert!(database.model_profiles().expect("models").iter().any(|model| model.id == remote));
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_local_turn_completes_when_the_ui_discards_every_event() {
    struct DiscardingStream;
    impl super::GenerationEventSink for DiscardingStream {
        fn emit(&self, _event: lettuce_contracts::GenerationEvent) {}
    }
    let harness = harness(Reply::Text("Hello."));
    harness.provider.runtime_events.store(true, std::sync::atomic::Ordering::Release);
    let model = crate::launch::tests::seed_model(harness.context.backend().database(), lettuce_models::ProviderProtocol::LlamaCpp, "llamacpp");
    crate::launch::tests::set_application_default_model(harness.context.backend().database(), model);
    let chat = launch(&harness, "unwatched-local").await;
    let accepted = super::conversation_send(
        &harness.context,
        lettuce_contracts::ConversationSendRequest {
            conversation_id: chat,
            text: "Hi".into(),
            client_operation_id: "unwatched-local-send".into(),
        },
        Arc::new(DiscardingStream),
    ).await.expect("send");
    assert!(ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("generation"));
    assert!(matches!(super::worker::settled_event(harness.context.backend().database(), accepted.turn_id.parse().expect("turn")).expect("settled"), Some(lettuce_contracts::GenerationEvent::Completed { .. })));
}
