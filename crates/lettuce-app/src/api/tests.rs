use std::sync::{Arc, Mutex};

use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
    CharacterProvenance, CharacterRepository, CreateCharacterPlan,
};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails, ApiEvent, GenerationEvent};
use lettuce_conversations::{
    ConversationReader, ConversationRepositoryError, GenerationStreamEvent,
    GenerationStreamEventEnvelope, GenerationTurnStatus, InferenceCandidate, InferenceOutcome,
    InferencePort, InferenceRequest, MessagePart, PortError, ValidationError,
};
use lettuce_inference::{InferenceRuntime, InferenceRuntimePort};
use lettuce_jobs::{StoreError, SystemClock};
use lettuce_media::MediaStoreError;
use lettuce_models::{ModelProfileRepository, ProviderProtocol};
use lettuce_types::{CharacterId, TimestampMillis};

use super::context::UnavailableEmbedding;
use super::error::IntoApiError;
use super::*;
use crate::AppBackend;

#[derive(Default)]
struct RecordingStream(Mutex<Vec<GenerationEvent>>);

impl GenerationEventSink for RecordingStream {
    fn emit(&self, event: GenerationEvent) {
        self.0.lock().expect("stream events").push(event);
    }
}

impl RecordingStream {
    fn events(&self) -> Vec<GenerationEvent> {
        self.0.lock().expect("stream events").clone()
    }
}

#[derive(Default)]
struct RecordingEvents(Mutex<Vec<ApiEvent>>);

impl ApiEventSink for RecordingEvents {
    fn emit(&self, event: ApiEvent) {
        self.0.lock().expect("api events").push(event);
    }
}

enum Reply {
    Text(&'static str),
    UntilCancelled,
}

/// Streams "Hel" and "lo." when the request has a sink, then answers or
/// waits for its job to be cancelled.
struct FakeProvider {
    runtime: Arc<InferenceRuntime>,
    reply: Reply,
    entered: tokio::sync::Notify,
    requests: Mutex<Vec<InferenceRequest>>,
}

#[async_trait::async_trait]
impl InferencePort for FakeProvider {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        self.requests
            .lock()
            .expect("requests")
            .push(request.clone());
        if let Some(sink) = request.stream_sink {
            for (sequence, text) in (1..).zip(["Hel", "lo."]) {
                self.runtime
                    .emit(
                        sink,
                        GenerationStreamEventEnvelope {
                            operation: request.operation,
                            turn_id: request.turn_id,
                            attempt_id: request.attempt_id,
                            sequence,
                            event: GenerationStreamEvent::TextDelta { text: text.into() },
                        },
                    )
                    .await
                    .map_err(|_| PortError::Unavailable)?;
            }
        }
        self.entered.notify_one();
        match self.reply {
            Reply::Text(text) => Ok(InferenceOutcome {
                provider_response_id: Some("fake-response".into()),
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text { text: text.into() }],
                    tool_calls: vec![],
                    provider_replay: None,
                    media: Vec::new(),
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: Some("stop".into()),
                provider_request_id: Some("fake-request".into()),
                warning_codes: vec![],
            }),
            Reply::UntilCancelled => {
                let job = request.cancellation.ok_or(PortError::Unavailable)?;
                self.runtime
                    .cancelled(job)
                    .await
                    .map_err(|_| PortError::Unavailable)?;
                Err(PortError::Cancelled)
            }
        }
    }
}

struct Harness {
    context: ApiContext,
    provider: Arc<FakeProvider>,
    events: Arc<RecordingEvents>,
    character_id: CharacterId,
}

fn harness(reply: Reply) -> Harness {
    let backend = Arc::new(AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend"));
    let database = backend.database();
    let model_id = crate::launch::tests::seed_model(database, ProviderProtocol::Ollama, "ollama");
    let mut model = ModelProfileRepository::get(database, model_id)
        .expect("model")
        .expect("model exists");
    let revision = model.revision;
    model.config.chat_parameters.temperature = None;
    model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("streaming model");
    crate::launch::tests::set_application_default_model(database, model_id);
    let character_id = CharacterId::new();
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character: Character::new(
                character_id,
                CharacterProfile {
                    name: "Ada".into(),
                    nickname: None,
                    description: Some("A meticulous engineer".into()),
                    definition: None,
                    design_description: None,
                    scenario: None,
                    rules: Vec::new(),
                },
                CharacterProvenance::default(),
                CharacterDefaults::default(),
                CharacterPresentationV1::default(),
                None,
                CharacterMedia::default(),
                TimestampMillis::new(1),
            )
            .expect("character"),
            scenes: Vec::new(),
            variants: Vec::new(),
            starters: Vec::new(),
        },
    )
    .expect("create character");
    let provider = Arc::new(FakeProvider {
        runtime: Arc::clone(backend.inference_runtime()),
        reply,
        entered: tokio::sync::Notify::new(),
        requests: Mutex::new(Vec::new()),
    });
    let events = Arc::new(RecordingEvents::default());
    let context = ApiContext::new(ApiContextParts {
        backend,
        secret_store: Arc::new(lettuce_settings::InMemorySecretStore::new()),
        inference: provider.clone(),
        embedding: Arc::new(UnavailableEmbedding),
        emotion: None,
        media: None,
        events: events.clone(),
        clock: Arc::new(SystemClock),
    });
    Harness {
        context,
        provider,
        events,
        character_id,
    }
}

async fn launch(harness: &Harness, key: &str) -> String {
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: harness.character_id.to_string(),
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id
}

async fn send(
    harness: &Harness,
    conversation_id: &str,
    key: &str,
    text: &str,
    stream: Arc<RecordingStream>,
) -> Result<dto::SendAccepted, dto::ApiError> {
    conversation_send(
        &harness.context,
        dto::ConversationSendRequest {
            conversation_id: conversation_id.into(),
            text: text.into(),
            client_operation_id: key.into(),
        },
        stream,
    )
    .await
}

fn text_of(message: &dto::TimelineMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            dto::MessagePartView::Text { text } => Some(text.as_str()),
            dto::MessagePartView::Media { .. } => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn launch_lists_characters_and_conversations_and_opens_an_empty_chat() {
    let harness = harness(Reply::Text("Hello."));
    let characters = characters_list(&harness.context, dto::CharactersListRequest::default())
        .await
        .expect("characters");
    assert_eq!(characters.items.len(), 1);
    assert_eq!(characters.items[0].id, harness.character_id.to_string());
    assert_eq!(characters.items[0].name, "Ada");
    assert_eq!(characters.items[0].avatar, None);

    let conversation_id = launch(&harness, "launch-1").await;
    assert_eq!(launch(&harness, "launch-1").await, conversation_id);

    let page = conversations_list(&harness.context, dto::ConversationsListRequest::default())
        .await
        .expect("conversations");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, conversation_id);
    assert_eq!(page.items[0].kind, dto::ConversationKind::Direct);
    assert_eq!(page.items[0].title, "Ada");
    assert_eq!(page.items[0].last_message_preview, None);
    assert_eq!(page.next_cursor, None);

    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("open");
    assert_eq!(view.id, conversation_id);
    assert!(view.can_send);
    assert_eq!(view.pending_turn_id, None);
    assert_eq!(view.participants.len(), 2);
    assert!(view.participants.iter().any(|participant| {
        participant.role == dto::ParticipantRole::Character
            && participant.character_id.as_deref() == Some(&*harness.character_id.to_string())
    }));
    assert!(view.messages.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn send_streams_deltas_completes_and_persists_the_reply() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-send").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &conversation_id,
        "send-1",
        "Hi there",
        stream.clone(),
    )
    .await
    .expect("send");
    let pending = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("open while pending");
    assert!(!pending.can_send);
    assert_eq!(pending.pending_turn_id.as_deref(), Some(&*accepted.turn_id));

    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("run generation"));
    assert!(!worker.run_once().await.expect("idle worker"));

    let events = stream.events();
    let turn_id = accepted.turn_id.clone();
    let GenerationEvent::Completed { message_id, .. } = events.last().expect("last event").clone()
    else {
        panic!("generation did not complete: {events:?}");
    };
    assert_eq!(
        events,
        vec![
            GenerationEvent::Started {
                turn_id: turn_id.clone()
            },
            GenerationEvent::Delta {
                turn_id: turn_id.clone(),
                text: Some("Hel".into()),
                reasoning: None,
            },
            GenerationEvent::Delta {
                turn_id: turn_id.clone(),
                text: Some("lo.".into()),
                reasoning: None,
            },
            GenerationEvent::Completed {
                turn_id: turn_id.clone(),
                message_id: message_id.clone(),
            },
        ]
    );
    assert!(
        harness.provider.requests.lock().expect("requests")[0]
            .stream_sink
            .is_some()
    );
    assert_eq!(
        harness.events.0.lock().expect("api events").clone(),
        vec![ApiEvent::GenerationSettled {
            conversation_id: conversation_id.clone(),
            turn_id: turn_id.clone(),
        }]
    );

    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("open");
    assert!(view.can_send);
    let messages = &view.messages.items;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].id, accepted.user_message_id);
    assert_eq!(messages[0].role, dto::MessageRole::User);
    assert_eq!(text_of(&messages[0]), "Hi there");
    assert_eq!(messages[1].id, message_id);
    assert_eq!(messages[1].role, dto::MessageRole::Assistant);
    assert_eq!(text_of(&messages[1]), "Hello.");
    assert_eq!(messages[1].candidate_index, Some(0));
    assert_eq!(messages[1].candidate_count, 1);
    assert_eq!(view.branch.head_message_id.as_deref(), Some(&*message_id));

    let newest = conversation_messages(
        &harness.context,
        dto::ConversationMessagesRequest {
            conversation_id: conversation_id.clone(),
            before_cursor: None,
            limit: Some(1),
        },
    )
    .await
    .expect("newest page");
    assert_eq!(newest.items.len(), 1);
    assert_eq!(newest.items[0].id, message_id);
    let older = conversation_messages(
        &harness.context,
        dto::ConversationMessagesRequest {
            conversation_id: conversation_id.clone(),
            before_cursor: newest.next_cursor.clone(),
            limit: Some(1),
        },
    )
    .await
    .expect("older page");
    assert_eq!(older.items[0].id, accepted.user_message_id);

    let list = conversations_list(&harness.context, dto::ConversationsListRequest::default())
        .await
        .expect("list");
    assert_eq!(
        list.items[0].last_message_preview.as_deref(),
        Some("Hello.")
    );

    let replayed = send(
        &harness,
        &conversation_id,
        "send-1",
        "Hi there",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("replayed send");
    assert_eq!(replayed, accepted);
    assert!(!worker.run_once().await.expect("no second generation"));
    assert_eq!(harness.provider.requests.lock().expect("requests").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_send_while_a_reply_is_pending_is_busy() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-busy").await;
    send(
        &harness,
        &conversation_id,
        "busy-1",
        "First",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("first send");
    let error = send(
        &harness,
        &conversation_id,
        "busy-2",
        "Second",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("second send");
    assert_eq!(error.code, ApiErrorCode::Busy);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_queued_turn_settles_it_without_running() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-cancel-queued").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &conversation_id,
        "queued-1",
        "Stop",
        stream.clone(),
    )
    .await
    .expect("send");
    generation_cancel(
        &harness.context,
        dto::GenerationCancelRequest {
            turn_id: accepted.turn_id.clone(),
        },
    )
    .await
    .expect("cancel");
    assert_eq!(
        stream.events(),
        vec![GenerationEvent::Cancelled {
            turn_id: accepted.turn_id.clone()
        }]
    );
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(!worker.run_once().await.expect("nothing queued"));
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
    generation_cancel(
        &harness.context,
        dto::GenerationCancelRequest {
            turn_id: accepted.turn_id.clone(),
        },
    )
    .await
    .expect("cancelling again is a no-op");
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest { conversation_id },
    )
    .await
    .expect("open");
    assert!(view.can_send);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_running_turn_stops_the_provider_and_settles_cancelled() {
    let harness = harness(Reply::UntilCancelled);
    let conversation_id = launch(&harness, "launch-cancel-running").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &conversation_id,
        "running-1",
        "Wait",
        stream.clone(),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, cancelled) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        generation_cancel(
            &harness.context,
            dto::GenerationCancelRequest {
                turn_id: accepted.turn_id.clone(),
            },
        )
        .await
    });
    assert!(ran.expect("worker ran"));
    cancelled.expect("cancel");
    let events = stream.events();
    assert_eq!(
        events.first(),
        Some(&GenerationEvent::Started {
            turn_id: accepted.turn_id.clone()
        })
    );
    assert_eq!(
        events.last(),
        Some(&GenerationEvent::Cancelled {
            turn_id: accepted.turn_id.clone()
        })
    );
    let turn = ConversationReader::get_turn(
        harness.context.backend().database(),
        accepted.turn_id.parse().expect("turn id"),
    )
    .expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Cancelled);
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_requests_map_to_stable_codes() {
    let harness = harness(Reply::Text("Hello."));
    let error = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: "not-a-uuid".into(),
        },
    )
    .await
    .expect_err("invalid id");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "conversation_id".into()
        })
    );
    let error = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: lettuce_types::ConversationId::new().to_string(),
        },
    )
    .await
    .expect_err("unknown conversation");
    assert_eq!(error.code, ApiErrorCode::NotFound);
    let conversation_id = launch(&harness, "launch-errors").await;
    let error = send(
        &harness,
        &conversation_id,
        "blank",
        "   ",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("blank text");
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "text".into()
        })
    );
    let error = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: CharacterId::new().to_string(),
            client_operation_id: "missing-character".into(),
        },
    )
    .await
    .expect_err("unknown character");
    assert_eq!(error.code, ApiErrorCode::NotFound);
    let error = read_asset(&harness.context, &lettuce_types::AssetId::new().to_string())
        .await
        .expect_err("no media store");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
}

#[test]
fn domain_errors_map_to_api_codes() {
    for (error, code) in [
        (
            ConversationRepositoryError::NotFound,
            ApiErrorCode::NotFound,
        ),
        (
            ConversationRepositoryError::Conflict,
            ApiErrorCode::Conflict,
        ),
        (
            ConversationRepositoryError::StaleRevision {
                expected: lettuce_types::Revision::INITIAL,
                actual: lettuce_types::Revision::INITIAL,
            },
            ApiErrorCode::Conflict,
        ),
        (
            ConversationRepositoryError::Unsupported,
            ApiErrorCode::Unsupported,
        ),
        (ConversationRepositoryError::Storage, ApiErrorCode::Internal),
    ] {
        assert_eq!(error.into_api_error().code, code);
    }
    let invalid = ConversationRepositoryError::Invalid(ValidationError::Blank {
        field: "message.parts",
    })
    .into_api_error();
    assert_eq!(invalid.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        invalid.details,
        Some(ApiErrorDetails::InvalidField {
            field: "message.parts".into()
        })
    );
    assert_eq!(
        StoreError::ResourceUnavailable.into_api_error().code,
        ApiErrorCode::Busy
    );
    assert_eq!(
        StoreError::AlreadyTerminal.into_api_error().code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        MediaStoreError::AssetNotFound.into_api_error().code,
        ApiErrorCode::NotFound
    );
    assert_eq!(
        MediaStoreError::NotReady.into_api_error().code,
        ApiErrorCode::Unavailable
    );
    assert_eq!(
        lettuce_characters::RepositoryError::Archived
            .into_api_error()
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        crate::CompanionTurnError::Cancelled.into_api_error().code,
        ApiErrorCode::Cancelled
    );
    assert_eq!(
        crate::ConversationLaunchError::CharacterNotFound {
            character_id: CharacterId::new()
        }
        .into_api_error()
        .code,
        ApiErrorCode::NotFound
    );
    let invalid =
        crate::ConversationLaunchError::InvalidRequest { field: "title" }.into_api_error();
    assert_eq!(invalid.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        invalid.details,
        Some(ApiErrorDetails::InvalidField {
            field: "title".into()
        })
    );
}

#[test]
fn contract_events_serialize_as_tagged_snake_case() {
    let event = serde_json::to_value(GenerationEvent::Delta {
        turn_id: "t".into(),
        text: Some("a".into()),
        reasoning: None,
    })
    .expect("event json");
    assert_eq!(
        event,
        serde_json::json!({"type": "delta", "turn_id": "t", "text": "a", "reasoning": null})
    );
    let error = serde_json::to_value(dto::ApiError {
        code: ApiErrorCode::InvalidInput,
        message: "m".into(),
        details: Some(ApiErrorDetails::InvalidField { field: "f".into() }),
    })
    .expect("error json");
    assert_eq!(
        error,
        serde_json::json!({"code": "invalid_input", "message": "m", "details": {"type": "invalid_field", "field": "f"}})
    );
}
