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

use super::error::IntoApiError;
use super::*;
use crate::AppBackend;

/// Files on the local disk, as the desktop shell reaches them.
pub(crate) struct StdFiles;

impl FileAccess for StdFiles {
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        let path = std::path::Path::new(uri);
        let metadata = std::fs::metadata(path).map_err(|_| FileAccessError::NotFound)?;
        Ok(FileDescription {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size: metadata.len(),
        })
    }

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        std::fs::File::open(uri)
            .map(|file| Box::new(file) as Box<dyn FileReader>)
            .map_err(|_| FileAccessError::NotFound)
    }

    fn create(&self, uri: &str) -> Result<Box<dyn std::io::Write + Send>, FileAccessError> {
        std::fs::File::create(uri)
            .map(|file| Box::new(file) as Box<dyn std::io::Write + Send>)
            .map_err(|_| FileAccessError::Io)
    }
    fn create_export(
        &self,
        uri: &str,
        source: Option<&std::fs::File>,
        protected: ExportProtection<'_>,
    ) -> Result<Box<dyn std::io::Write + Send>, FileAccessError> {
        if protected(uri, None)? {
            return Err(FileAccessError::SourceIsTarget);
        }
        let mut target = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(uri)
            .map_err(|_| FileAccessError::Io)?;
        let target_handle =
            same_file::Handle::from_file(target.try_clone().map_err(|_| FileAccessError::Io)?)
                .map_err(|_| FileAccessError::Io)?;
        if let Some(source) = source {
            let source_handle =
                same_file::Handle::from_file(source.try_clone().map_err(|_| FileAccessError::Io)?)
                    .map_err(|_| FileAccessError::Io)?;
            if source_handle == target_handle {
                return Err(FileAccessError::SourceIsTarget);
            }
        }
        if protected(uri, Some(&target))? {
            return Err(FileAccessError::SourceIsTarget);
        }
        target.set_len(0).map_err(|_| FileAccessError::Io)?;
        std::io::Seek::seek(&mut target, std::io::SeekFrom::Start(0))
            .map_err(|_| FileAccessError::Io)?;
        Ok(Box::new(target))
    }
}

#[derive(Default)]
pub(super) struct RecordingStream(Mutex<Vec<GenerationEvent>>);

impl GenerationEventSink for RecordingStream {
    fn emit(&self, event: GenerationEvent) {
        self.0.lock().expect("stream events").push(event);
    }
}

impl RecordingStream {
    pub(super) fn events(&self) -> Vec<GenerationEvent> {
        self.0.lock().expect("stream events").clone()
    }
}

#[derive(Default)]
pub(crate) struct RecordingEvents(Mutex<Vec<ApiEvent>>, tokio::sync::Notify);

impl ApiEventSink for RecordingEvents {
    fn emit(&self, event: ApiEvent) {
        self.0.lock().expect("api events").push(event);
        self.1.notify_one();
    }
}

impl RecordingEvents {
    pub(super) fn events(&self) -> Vec<ApiEvent> {
        self.0.lock().expect("api events").clone()
    }
    /// Waits until the recorded events satisfy `done`.
    pub(super) async fn until(&self, done: impl Fn(&[ApiEvent]) -> bool) {
        loop {
            let notified = self.1.notified();
            if done(&self.0.lock().expect("api events")) {
                return;
            }
            notified.await;
        }
    }
}

/// An image provider that serves nothing.
pub(super) struct NoImages;

#[async_trait::async_trait]
impl lettuce_image_generation::ImageProviderPort for NoImages {
    async fn generate(
        &self,
        request: lettuce_image_generation::ProviderImageRequest,
    ) -> Result<
        lettuce_image_generation::ProviderImageOutput,
        lettuce_image_generation::ImageProviderError,
    > {
        Err(lettuce_image_generation::ImageProviderError::Unsupported(
            request.account.provider_kind.clone(),
        ))
    }
}

pub(super) enum Reply {
    LorebookTools,
    LorebookToolsUntil(&'static str),
    Text(&'static str),
    UntilCancelled,
    PartialUntilCancelled(&'static str),
}

/// Streams "Hel" and "lo." when the request has a sink, then answers or
/// waits for its job to be cancelled.
type ResponseHook = Arc<dyn Fn(&InferenceRequest, &mut InferenceOutcome) + Send + Sync>;

pub(super) struct FakeProvider {
    pub(super) response_hook: Mutex<Option<ResponseHook>>,
    pub(super) response_release: Mutex<Option<Arc<tokio::sync::Notify>>>,
    runtime: Arc<InferenceRuntime>,
    reply: Reply,
    pub(super) entered: tokio::sync::Notify,
    pub(super) requests: Mutex<Vec<InferenceRequest>>,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) runtime_events: std::sync::atomic::AtomicBool,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    events_router: Arc<super::local_runtime_events::RuntimeEventRouter>,
}

#[async_trait::async_trait]
impl InferencePort for FakeProvider {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        self.requests
            .lock()
            .expect("requests")
            .push(request.clone());
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self
            .runtime_events
            .load(std::sync::atomic::Ordering::Acquire)
        {
            use lettuce_local_llm::generation::{GenerationHeartbeat, LlamaHostEvent, LlamaNotice};
            self.events_router.emit(LlamaHostEvent::ModelLoadProgress(
                lettuce_local_llm::engine::ModelLoadProgress {
                    request_id: Some(request.attempt_id.to_string()),
                    model_path: "local-events.gguf".into(),
                    model_name: "Local events".into(),
                    backend_path: "cpu".into(),
                    stage: lettuce_local_llm::engine::ModelLoadStage::Cpu,
                    status: lettuce_local_llm::engine::ModelLoadStatus::Loading,
                    progress: 0.42,
                    percent: 42,
                    gpus: None,
                },
            ));
            self.events_router.emit(LlamaHostEvent::Notice {
                request_id: Some(request.attempt_id.to_string()),
                notice: LlamaNotice::MtpDisabledForVision,
            });
            self.events_router.emit(LlamaHostEvent::Heartbeat {
                request_id: Some(request.attempt_id.to_string()),
                heartbeat: GenerationHeartbeat {
                    tokens: 4,
                    elapsed_ms: 10,
                    tokens_per_second: 400.0,
                    recent_text: "ignored heartbeat text".into(),
                },
            });
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self
            .runtime_events
            .load(std::sync::atomic::Ordering::Acquire)
        {
            self.events_router.flush();
        }
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
        let mut outcome = match self.reply {
            Reply::LorebookTools | Reply::LorebookToolsUntil(_) => {
                if let Reply::LorebookToolsUntil(blocked) = self.reply {
                    if request.tools.as_ref().is_some_and(|tools| {
                        tools.definitions.iter().any(|tool| tool.name == blocked)
                    }) {
                        let job = request.cancellation.ok_or(PortError::Unavailable)?;
                        self.runtime
                            .cancelled(job)
                            .await
                            .map_err(|_| PortError::Unavailable)?;
                        return Err(PortError::Cancelled);
                    }
                }
                let name = request
                    .tools
                    .as_ref()
                    .and_then(|tools| tools.definitions.first())
                    .ok_or(PortError::Unavailable)?
                    .name
                    .clone();
                let arguments = match name.as_str() {
                    "propose_lorebook_outline" => {
                        serde_json::json!({ "entries": (0..5).map(|index| serde_json::json!({ "title": format!("District {index}"), "category": "location", "rationale": "A district of the coastal city", "proposedKeys": [format!("District {index}")], "sourceRefs": [] })).collect::<Vec<_>>() })
                    }
                    "write_lorebook_entry" => {
                        serde_json::json!({ "title": "District", "content": "A coastal district.", "keywords": ["District"], "alwaysActive": false })
                    }
                    "propose_coherence_changes" => {
                        serde_json::json!({ "changes": [{ "kind": "toggleAlwaysActive", "entryIdx": 0, "newValue": true, "reason": "Central setting" }] })
                    }
                    _ => return Err(PortError::Unavailable),
                };
                Ok(InferenceOutcome {
                    provider_response_id: None,
                    candidates: vec![InferenceCandidate {
                        ordinal: 0,
                        parts: vec![],
                        tool_calls: vec![lettuce_conversations::ProposedToolCall {
                            provider_call_id: None,
                            name,
                            arguments,
                            raw_arguments: None,
                            provider_replay: None,
                        }],
                        provider_replay: None,
                        media: vec![],
                    }],
                    usage: None,
                    finish_reason: lettuce_conversations::FinishReason::Stop,
                    provider_finish_reason: None,
                    provider_request_id: None,
                    warning_codes: vec![],
                })
            }
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
            Reply::PartialUntilCancelled(text) => {
                let job = request.cancellation.ok_or(PortError::Unavailable)?;
                self.runtime
                    .cancelled(job)
                    .await
                    .map_err(|_| PortError::Unavailable)?;
                Ok(InferenceOutcome {
                    provider_response_id: None,
                    candidates: vec![InferenceCandidate {
                        ordinal: 0,
                        parts: vec![MessagePart::Text { text: text.into() }],
                        tool_calls: vec![],
                        provider_replay: None,
                        media: Vec::new(),
                    }],
                    usage: None,
                    finish_reason: lettuce_conversations::FinishReason::Cancelled,
                    provider_finish_reason: None,
                    provider_request_id: None,
                    warning_codes: vec![],
                })
            }
        }?;
        let hook = self
            .response_hook
            .lock()
            .expect("response hook lock")
            .clone();
        if let Some(hook) = hook {
            hook(&request, &mut outcome);
        }
        let release = self
            .response_release
            .lock()
            .expect("response release lock")
            .clone();
        if let Some(release) = release {
            release.notified().await;
        }
        Ok(outcome)
    }
}

pub(super) struct Harness {
    pub(super) context: ApiContext,
    pub(super) provider: Arc<FakeProvider>,
    pub(super) events: Arc<RecordingEvents>,
    pub(super) character_id: CharacterId,
    pub(super) filter_runtime:
        crate::generation::provider_runtime::ProviderRuntime<lettuce_settings::InMemorySecretStore>,
}

pub(super) fn harness(reply: Reply) -> Harness {
    harness_with(reply, Arc::new(SystemClock), None)
}

fn harness_with(
    reply: Reply,
    clock: Arc<dyn lettuce_jobs::Clock>,
    media: Option<Arc<ApiMediaStore>>,
) -> Harness {
    harness_in(reply, clock, media, None, Arc::new(NoModels))
}

/// A harness whose context has `app_folder` as its app data folder.
pub(super) fn harness_in(
    reply: Reply,
    clock: Arc<dyn lettuce_jobs::Clock>,
    media: Option<Arc<ApiMediaStore>>,
    app_folder: Option<std::path::PathBuf>,
    models: Arc<dyn ModelLoader>,
) -> Harness {
    harness_full(reply, clock, media, app_folder, models, Arc::new(NoImages))
}

/// A harness whose context generates images with `images`.
pub(super) fn harness_full(
    reply: Reply,
    clock: Arc<dyn lettuce_jobs::Clock>,
    media: Option<Arc<ApiMediaStore>>,
    app_folder: Option<std::path::PathBuf>,
    models: Arc<dyn ModelLoader>,
    images: Arc<dyn lettuce_image_generation::ImageProviderPort>,
) -> Harness {
    let backend = Arc::new(AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend"));
    harness_over(backend, reply, clock, media, app_folder, models, images)
}

/// A harness over `backend`, such as one opened on a file the media store
/// shares.
pub(super) fn harness_over(
    backend: Arc<AppBackend>,
    reply: Reply,
    clock: Arc<dyn lettuce_jobs::Clock>,
    media: Option<Arc<ApiMediaStore>>,
    app_folder: Option<std::path::PathBuf>,
    models: Arc<dyn ModelLoader>,
    images: Arc<dyn lettuce_image_generation::ImageProviderPort>,
) -> Harness {
    harness_over_files(
        backend, reply, clock, media, app_folder, models, images, None,
    )
}

/// A harness over `backend` whose context knows the database files media
/// collection reads.
#[expect(clippy::too_many_arguments, reason = "one parameter per context part")]
pub(super) fn harness_over_files(
    backend: Arc<AppBackend>,
    reply: Reply,
    clock: Arc<dyn lettuce_jobs::Clock>,
    media: Option<Arc<ApiMediaStore>>,
    app_folder: Option<std::path::PathBuf>,
    models: Arc<dyn ModelLoader>,
    images: Arc<dyn lettuce_image_generation::ImageProviderPort>,
    database_files: Option<ApiDatabaseFiles>,
) -> Harness {
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
    let character_id = create_character(database, "Ada", CharacterDefaults::default());
    let provider = Arc::new(FakeProvider {
        response_hook: Mutex::new(None),
        response_release: Mutex::new(None),
        runtime: Arc::clone(backend.inference_runtime()),
        reply,
        entered: tokio::sync::Notify::new(),
        requests: Mutex::new(Vec::new()),
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        runtime_events: std::sync::atomic::AtomicBool::new(false),
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        events_router: backend.local_runtime_events().clone(),
    });
    let events = Arc::new(RecordingEvents::default());
    let secrets = Arc::new(lettuce_settings::InMemorySecretStore::new());
    let runtime = backend
        .provider_runtime(secrets.clone(), &backend.tls_policy().expect("tls"))
        .expect("provider runtime");
    let runtime_filter = runtime.content_filter();
    let context = ApiContext::new_with_filter(
        ApiContextParts {
            backend,
            secret_store: Arc::new(lettuce_settings::InMemorySecretStore::new()),
            inference: provider.clone(),
            image_provider: images,
            models,
            speech: Arc::new(super::NoSpeech),
            media,
            events: events.clone(),
            clock,
            files: Arc::new(StdFiles),
            app_folder,
            resource_dir: None,
            database_files,
            asset_url_base: "test-asset://host".into(),
        },
        runtime_filter.clone(),
    );
    assert!(Arc::ptr_eq(context.content_filter(), &runtime_filter));
    Harness {
        context,
        provider,
        events,
        character_id,
        filter_runtime: runtime,
    }
}

/// Creates an active character named `name` with `defaults`.
pub(super) fn create_character(
    database: &lettuce_database::Database,
    name: &str,
    defaults: CharacterDefaults,
) -> CharacterId {
    let character_id = CharacterId::new();
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character: Character::new(
                character_id,
                CharacterProfile {
                    name: name.into(),
                    nickname: None,
                    description: Some("A meticulous engineer".into()),
                    definition: None,
                    design_description: None,
                    scenario: None,
                    rules: Vec::new(),
                },
                CharacterProvenance::default(),
                defaults,
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
    character_id
}

pub(super) async fn launch(harness: &Harness, key: &str) -> String {
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: harness.character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id
}

pub(super) async fn send(
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
            GenerationEvent::SpeakerSelected {
                turn_id: turn_id.clone(),
                character_id: harness.character_id.to_string(),
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
            after_cursor: None,
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
            after_cursor: None,
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
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "missing-character".into(),
        },
    )
    .await
    .expect_err("unknown character");
    assert_eq!(error.code, ApiErrorCode::NotFound);
    let error = read_asset(
        &harness.context,
        &lettuce_types::AssetId::new().to_string(),
        AssetRange::Whole,
    )
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

fn queued_generation(harness: &Harness, turn_id: &str) -> crate::QueuedConversationGeneration {
    let turn = ConversationReader::get_turn(
        harness.context.backend().database(),
        turn_id.parse().expect("turn id"),
    )
    .expect("turn");
    let attempt = turn.attempts.last().expect("attempt");
    crate::QueuedConversationGeneration {
        conversation_id: turn.conversation_id,
        turn_id: turn.id,
        attempt_id: attempt.id,
        job_id: attempt.job_id.expect("attached job"),
    }
}

async fn can_send(harness: &Harness, conversation_id: &str) -> bool {
    conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    .expect("open")
    .can_send
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retried_send_while_queued_returns_the_first_send_and_other_text_conflicts() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-retry").await;
    let first = Arc::new(RecordingStream::default());
    let accepted = send(&harness, &conversation_id, "retry-1", "Hi", first.clone())
        .await
        .expect("send");
    let retry = Arc::new(RecordingStream::default());
    let replayed = send(&harness, &conversation_id, "retry-1", "Hi", retry.clone())
        .await
        .expect("retried send while queued");
    assert_eq!(replayed, accepted);
    let error = send(
        &harness,
        &conversation_id,
        "retry-1",
        "Something else",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("same key, different text");
    assert_eq!(error.code, ApiErrorCode::Conflict);

    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("run generation"));
    assert!(matches!(
        retry.events().last(),
        Some(GenerationEvent::Completed { .. })
    ));
    assert!(first.events().is_empty());
    assert_eq!(harness.provider.requests.lock().expect("requests").len(), 1);
    let error = send(
        &harness,
        &conversation_id,
        "retry-1",
        "Something else",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("same key, different text after settling");
    assert_eq!(error.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_that_cannot_be_claimed_or_already_ended_is_not_run() {
    let harness = harness(Reply::Text("Hello."));
    let worker = ConversationGenerationWorker::new(harness.context.clone());

    let claimed_elsewhere = launch(&harness, "launch-not-claimed").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(&harness, &claimed_elsewhere, "nc-1", "Hi", stream.clone())
        .await
        .expect("send");
    let next = queued_generation(&harness, &accepted.turn_id);
    lettuce_jobs::JobStore::claim(
        harness.context.backend().database(),
        next.job_id,
        lettuce_jobs::WorkerId::new(),
        harness.context.now(),
        std::time::Duration::from_secs(60),
        &lettuce_jobs::ResourceAvailability::all(),
    )
    .expect("claim")
    .expect("claimed by another worker");
    assert!(!worker.run_queued(next).await.expect("not claimed"));
    assert!(!worker.run_once().await.expect("nothing queued"));
    assert!(stream.events().is_empty());
    assert!(harness.context.stream(next.turn_id).is_some());

    let cancelled = launch(&harness, "launch-terminal").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(&harness, &cancelled, "terminal-1", "Hi", stream.clone())
        .await
        .expect("send");
    let next = queued_generation(&harness, &accepted.turn_id);
    generation_cancel(
        &harness.context,
        dto::GenerationCancelRequest {
            turn_id: accepted.turn_id.clone(),
        },
    )
    .await
    .expect("cancel");
    assert!(!worker.run_queued(next).await.expect("already ended"));
    assert_eq!(
        stream.events(),
        vec![GenerationEvent::Cancelled {
            turn_id: accepted.turn_id.clone()
        }]
    );
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
}

fn stray_job_spec(key: &str, turn_id: lettuce_types::GenerationTurnId) -> lettuce_jobs::JobSpec {
    lettuce_jobs::JobSpec::new(
        lettuce_jobs::JobKind::ConversationGeneration,
        lettuce_jobs::JobSubject::new(
            lettuce_jobs::SubjectKind::Conversation,
            lettuce_types::ConversationId::new().to_string(),
        )
        .expect("subject"),
        lettuce_jobs::OutcomeRef::GenerationTurn(turn_id),
    )
    .with_idempotency_key(lettuce_jobs::IdempotencyKey::new(key).expect("key"))
    .with_resources(vec![lettuce_jobs::ResourceClass::Network])
}

fn job_state(harness: &Harness, job_id: lettuce_types::JobId) -> lettuce_jobs::JobState {
    lettuce_jobs::JobStore::get(harness.context.backend().database(), job_id)
        .expect("job")
        .expect("job exists")
        .state
}

pub(super) fn api_events(harness: &Harness) -> Vec<ApiEvent> {
    harness.events.0.lock().expect("api events").clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn unresolvable_queued_jobs_are_failed_and_do_not_block_the_queue() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let orphan = lettuce_jobs::JobStore::create_or_get(
        database,
        stray_job_spec("orphan", lettuce_types::GenerationTurnId::new()),
    )
    .expect("orphan job")
    .job;

    let healthy = launch(&harness, "launch-healthy").await;
    let healthy_stream = Arc::new(RecordingStream::default());
    send(
        &harness,
        &healthy,
        "healthy-1",
        "Hi",
        healthy_stream.clone(),
    )
    .await
    .expect("healthy send");

    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let mut runs = 0;
    while worker.run_once().await.expect("worker step") {
        runs += 1;
    }
    assert_eq!(runs, 1);
    assert!(matches!(
        healthy_stream.events().last(),
        Some(GenerationEvent::Completed { .. })
    ));
    assert_eq!(
        job_state(&harness, orphan.id),
        lettuce_jobs::JobState::Failed
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_orphan_job_naming_a_turn_leaves_its_healthy_job_alone() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let conversation_id = launch(&harness, "launch-orphan-sibling").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &conversation_id,
        "sibling-1",
        "Hi",
        stream.clone(),
    )
    .await
    .expect("send");
    let healthy = queued_generation(&harness, &accepted.turn_id);
    let orphan = lettuce_jobs::JobStore::create_or_get(
        database,
        stray_job_spec("orphan-sibling", healthy.turn_id),
    )
    .expect("orphan job")
    .job;
    harness
        .context
        .backend()
        .conversation_generation_dispatcher()
        .fail_unresolvable_job(&orphan, harness.context.now())
        .expect("orphan failed");
    assert_eq!(
        job_state(&harness, orphan.id),
        lettuce_jobs::JobState::Failed
    );
    assert_eq!(
        job_state(&harness, healthy.job_id),
        lettuce_jobs::JobState::Queued
    );
    assert!(!can_send(&harness, &conversation_id).await);

    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("healthy job runs"));
    assert!(matches!(
        stream.events().last(),
        Some(GenerationEvent::Completed { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_created_but_not_yet_attached_is_skipped_not_failed() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-attach-race").await;
    let stream = Arc::new(RecordingStream::default());
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let scanning = worker.clone();
    let accepted = super::conversations::send_with(
        &harness.context,
        dto::ConversationSendRequest {
            conversation_id: conversation_id.clone(),
            text: "Hi".into(),
            client_operation_id: "attach-race-1".into(),
        },
        stream.clone(),
        move |context, begun, now| {
            let database = context.backend().database();
            let job = lettuce_jobs::JobStore::create_or_get(
                database,
                crate::generation::conversation_generation::attempt_job_spec(
                    begun.conversation.id,
                    begun.turn.id,
                    begun.attempt.id,
                )?,
            )?
            .job;
            let ran = tokio::runtime::Handle::current()
                .block_on(scanning.run_once())
                .expect("scan between create and attach");
            assert!(!ran);
            let pending = lettuce_jobs::JobStore::get(database, job.id)?.expect("job exists");
            assert_eq!(pending.state, lettuce_jobs::JobState::Queued);
            context
                .backend()
                .conversation_generation_dispatcher()
                .schedule(begun, now)
        },
    )
    .await
    .expect("send");
    assert!(stream.events().is_empty());
    assert!(worker.run_once().await.expect("run generation"));
    assert!(matches!(
        stream.events().last(),
        Some(GenerationEvent::Completed { .. })
    ));
    assert_eq!(
        stream.events().first(),
        Some(&GenerationEvent::Started {
            turn_id: accepted.turn_id.clone()
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_send_whose_reply_cannot_be_queued_settles_its_turn() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-unschedulable").await;
    let stream = Arc::new(RecordingStream::default());
    let request = dto::ConversationSendRequest {
        conversation_id: conversation_id.clone(),
        text: "Hi".into(),
        client_operation_id: "unschedulable-1".into(),
    };
    let error = super::conversations::send_with(
        &harness.context,
        request.clone(),
        stream.clone(),
        |_, _, _| Err(crate::ConversationGenerationDispatchError::InvalidWork),
    )
    .await
    .expect_err("schedule failed");
    assert_eq!(error.code, ApiErrorCode::Internal);
    let events = stream.events();
    let [GenerationEvent::Cancelled { turn_id }] = events.as_slice() else {
        panic!("unexpected stream: {events:?}");
    };
    assert!(
        harness
            .context
            .stream(turn_id.parse().expect("turn id"))
            .is_none()
    );
    let settled = ApiEvent::GenerationSettled {
        conversation_id: conversation_id.clone(),
        turn_id: turn_id.clone(),
    };
    assert!(api_events(&harness).contains(&settled));
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("open");
    assert!(view.can_send);
    assert_eq!(view.pending_turn_id, None);

    let retry_stream = Arc::new(RecordingStream::default());
    let retried = super::conversations::send_with(
        &harness.context,
        request,
        retry_stream.clone(),
        |_, _, _| panic!("a settled send is not scheduled again"),
    )
    .await
    .expect("a retry of the settled send replays it");
    assert_eq!(&retried.turn_id, turn_id);
    assert_eq!(
        retry_stream.events(),
        vec![GenerationEvent::Cancelled {
            turn_id: turn_id.clone()
        }]
    );
    assert_eq!(
        api_events(&harness)
            .iter()
            .filter(|event| **event == settled)
            .count(),
        1
    );
    assert!(
        harness
            .context
            .stream(turn_id.parse().expect("turn id"))
            .is_none()
    );
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(!worker.run_once().await.expect("nothing queued"));

    send(
        &harness,
        &conversation_id,
        "unschedulable-2",
        "Again",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("the next send is accepted");
}

#[tokio::test(flavor = "multi_thread")]
async fn conversations_page_through_ties_on_updated_at() {
    let harness = harness_with(
        Reply::Text("Hello."),
        Arc::new(lettuce_jobs::FakeClock::new(TimestampMillis::new(5_000))),
        None,
    );
    let mut launched = Vec::new();
    for index in 0..5 {
        launched.push(launch(&harness, &format!("launch-tie-{index}")).await);
    }
    let mut seen = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = conversations_list(
            &harness.context,
            dto::ConversationsListRequest {
                cursor: cursor.take(),
                limit: Some(2),
                ..dto::ConversationsListRequest::default()
            },
        )
        .await
        .expect("page");
        pages += 1;
        assert!(page.items.len() <= 2);
        assert!(page.items.iter().all(|item| item.updated_at == 5_000));
        seen.extend(page.items.into_iter().map(|item| item.id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), seen.len());
    launched.sort();
    assert_eq!(unique, launched);

    let clamped = conversations_list(
        &harness.context,
        dto::ConversationsListRequest {
            cursor: None,
            limit: Some(u32::MAX),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await
    .expect("an oversized limit is clamped");
    assert_eq!(clamped.items.len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn message_cursor_errors_name_the_cursor_only_when_one_was_sent() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "launch-cursor").await;
    let error = conversation_messages(
        &harness.context,
        dto::ConversationMessagesRequest {
            conversation_id,
            before_cursor: Some("not-a-cursor".into()),
            after_cursor: None,
            limit: None,
        },
    )
    .await
    .expect_err("bad cursor");
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "before_cursor".into()
        })
    );
}

pub(super) fn png_bytes() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&13_u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    bytes.extend_from_slice(&3_u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(b"api asset range bytes");
    bytes
}

pub(super) fn media_store(root: &std::path::Path) -> Arc<ApiMediaStore> {
    let path = root.join("media.sqlite3");
    let authority = lettuce_platform::FilesystemAuthority::new(
        &lettuce_platform::DirectorySnapshot::new(root).expect("snapshot"),
    )
    .expect("authority");
    Arc::new(lettuce_media::LocalMediaBlobStore::new(
        authority.managed_files(),
        authority
            .read_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("read capability"),
        authority
            .write_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("write capability"),
        lettuce_database::Database::open(&path).expect("blob database"),
        lettuce_database::Database::open(&path).expect("asset database"),
    ))
}

async fn read_bytes(harness: &Harness, id: &str, range: AssetRange) -> AssetBytes {
    match read_asset(&harness.context, id, range)
        .await
        .expect("asset read")
    {
        AssetRead::Bytes(bytes) => bytes,
        AssetRead::Unsatisfiable { .. } => panic!("{range:?} is unsatisfiable"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn assets_are_read_whole_or_by_range_with_urls_from_the_host_base() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-api-assets-{}",
        lettuce_types::RequestId::new()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let store = media_store(&root);
    let bytes = png_bytes();
    let asset_id = store
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::AvatarOriginal,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Persistent,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("ingest")
        .asset
        .id;
    let harness = harness_with(Reply::Text("Hello."), Arc::new(SystemClock), Some(store));
    let id = asset_id.to_string();
    let total = bytes.len() as u64;

    let whole = read_bytes(&harness, &id, AssetRange::Whole).await;
    assert_eq!(whole.bytes, bytes);
    assert_eq!((whole.start, whole.total_len), (0, total));
    assert_eq!(whole.mime_type, "image/png");
    let middle = read_bytes(&harness, &id, AssetRange::Between { start: 4, end: 9 }).await;
    assert_eq!(middle.bytes, bytes[4..=9]);
    assert_eq!(middle.start, 4);
    let tail = read_bytes(&harness, &id, AssetRange::Last { len: 5 }).await;
    assert_eq!(tail.bytes, bytes[bytes.len() - 5..]);
    let rest = read_bytes(&harness, &id, AssetRange::From { start: 20 }).await;
    assert_eq!(rest.bytes, bytes[20..]);
    assert_eq!(
        read_asset(&harness.context, &id, AssetRange::From { start: total })
            .await
            .expect("past the end"),
        AssetRead::Unsatisfiable { total_len: total }
    );

    let error = read_asset(&harness.context, "not-an-id", AssetRange::Whole)
        .await
        .expect_err("bad id");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    let error = read_asset(
        &harness.context,
        &lettuce_types::AssetId::new().to_string(),
        AssetRange::Whole,
    )
    .await
    .expect_err("unknown id");
    assert_eq!(error.code, ApiErrorCode::NotFound);

    assert_eq!(
        harness.context.asset_ref(asset_id),
        dto::AssetRef {
            asset_id: id.clone(),
            url: format!("test-asset://host/{id}"),
        }
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_started_after_shutdown_began_is_cancelled() {
    let harness = harness(Reply::UntilCancelled);
    let conversation_id = launch(&harness, "launch-shutdown").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &conversation_id,
        "shutdown-1",
        "Wait",
        stream.clone(),
    )
    .await
    .expect("send");
    harness.context.begin_shutdown();
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    tokio::time::timeout(std::time::Duration::from_secs(10), worker.run_once())
        .await
        .expect("the job does not wait for a provider reply")
        .expect("worker ran");
    let turn = ConversationReader::get_turn(
        harness.context.backend().database(),
        accepted.turn_id.parse().expect("turn id"),
    )
    .expect("turn");
    assert!(
        matches!(
            turn.status,
            GenerationTurnStatus::Cancelled
                | GenerationTurnStatus::Failed
                | GenerationTurnStatus::Interrupted
        ),
        "{:?}",
        turn.status
    );
    assert!(harness.context.stream(turn.id).is_none());
}
