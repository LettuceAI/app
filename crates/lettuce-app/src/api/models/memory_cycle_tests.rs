use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use async_trait::async_trait;
use lettuce_characters::CharacterRepository;
use lettuce_contracts::{self as dto, RequiredModel};
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::{JobStore, SystemClock, WorkerId, handle::CancellationToken};
use lettuce_settings::{DeviceSettingsStore, GlobalSettingsStore};

use crate::{CompanionEmotionEngine, EmbeddingGenerationError, MemoryEmbeddingEngine};
use crate::api::{ApiContext, JobHandler, JobProgressSink, MemoryExtractionHandler, ModelLoad, ModelLoader};
use crate::api::tests::{Reply, harness_in};

#[derive(Default)]
struct SessionCounts {
    loads: AtomicUsize,
    drops: AtomicUsize,
    embeddings: AtomicUsize,
}

struct CycleModels(Arc<SessionCounts>);
struct CycleEmbedding(Arc<SessionCounts>);

impl Drop for CycleEmbedding {
    fn drop(&mut self) { self.0.drops.fetch_add(1, Ordering::SeqCst); }
}

impl MemoryEmbeddingEngine for CycleEmbedding {
    fn source_revision(&self) -> &str { "cycle-retention-test" }
    fn dimensions(&self) -> EmbeddingDimensions { EmbeddingDimensions::D64 }
    fn count_tokens(&self, _: &str) -> Result<u32, EmbeddingGenerationError> { Ok(1) }
    fn embed_memory(&self, request: &EmbeddingRequest, cancellation: &CancellationToken) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        if cancellation.is_cancelled() { return Err(EmbeddingGenerationError::Cancelled); }
        assert_eq!(self.0.loads.load(Ordering::SeqCst), 1);
        assert_eq!(self.0.drops.load(Ordering::SeqCst), 0);
        let index = self.0.embeddings.fetch_add(1, Ordering::SeqCst);
        let mut values = vec![0.0; request.dimensions.get()];
        values[index] = 1.0;
        Ok(EmbeddingVector { source_revision: self.source_revision().into(), values })
    }
}

#[async_trait]
impl ModelLoader for CycleModels {
    fn installed(&self, _: &ApiContext, model: RequiredModel) -> bool { model == RequiredModel::Embedding }
    async fn prepare(&self, _: &ApiContext) -> bool { true }
    fn embedding(&self, _: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        self.0.loads.fetch_add(1, Ordering::SeqCst);
        ModelLoad::Loaded(Arc::new(CycleEmbedding(self.0.clone())))
    }
    fn emotion(&self, _: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> { ModelLoad::NotInstalled }
}

struct IgnoreProgress;
impl JobProgressSink for IgnoreProgress {
    fn text_delta(&self, _: Option<String>, _: Option<String>) {}
    fn image_progress(&self, _: dto::ImageProgress) {}
}

async fn run_cycle(keep: bool, cancel: bool) {
    let counts = Arc::new(SessionCounts::default());
    let harness = harness_in(
        Reply::Text("The conversation establishes lasting facts."), Arc::new(SystemClock), None, None,
        Arc::new(CycleModels(counts.clone())),
    );
    let (chat, _) = crate::api::conversations::turns_tests::replied_chat(&harness, "cycle-retention").await;
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id).expect("character").expect("present").character;
    let mut defaults = character.defaults;
    defaults.memory_policy = lettuce_characters::MemoryPolicy::Dynamic;
    CharacterRepository::update_defaults(database, harness.character_id, character.revision, defaults, harness.context.now()).expect("dynamic character");
    let stored = database.load().expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    settings.dynamic_memory.recursive_memory_loops = true;
    database.save(settings, stored.default_model_profile_id, stored.revision).expect("settings");
    database.update_device_settings(&|device| device.embedding.keep_model_loaded = keep).expect("retention");
    let accepted = crate::api::memory_trigger(&harness.context, dto::MemoryTriggerRequest {
        conversation_id: chat.clone(), client_operation_id: "cycle-retention-trigger".into(),
    }).await.expect("trigger");
    assert_eq!(counts.loads.load(Ordering::SeqCst), 0);
    let job = JobStore::get(database, accepted.job_id.parse().expect("job id")).expect("job").expect("present");
    let work = MemoryExtractionHandler.claim(&harness.context, &job, WorkerId::new()).await.expect("claim").expect("work");
    let cancellation = work.cancellation();
    let manager_rounds = Arc::new(AtomicUsize::new(0));
    let round_count = manager_rounds.clone();
    let observed = counts.clone();
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(move |request, outcome| {
        if !request.tools.as_ref().is_some_and(|tools| tools.definitions.iter().any(|tool| tool.name == "create_memory")) { return; }
        let round = round_count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(observed.loads.load(Ordering::SeqCst), 1);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 0);
        if round > 0 { assert_eq!(observed.embeddings.load(Ordering::SeqCst), round * 2); }
        if cancel && round == 1 { cancellation.cancel(); }
        let candidate = outcome.candidates.first_mut().expect("candidate");
        // OpenRouter-style replies can include reasoning and visible text
        // alongside every recursive tool round, including the final `done`.
        candidate.parts = vec![
            lettuce_conversations::MessagePart::ReasoningSummary { text: "Reviewing lasting facts.".into() },
            lettuce_conversations::MessagePart::Text { text: format!("Memory round {round}.") },
        ];
        if round < 2 {
            candidate.tool_calls = (0..2).map(|item| lettuce_conversations::ProposedToolCall {
                provider_call_id: Some(format!("create-{round}-{item}")),
                name: "create_memory".into(),
                arguments: serde_json::json!({ "text": format!("Durable fact from round {round}, item {item}."), "category": "other" }),
                raw_arguments: None, provider_replay: None,
            }).collect();
        } else {
            candidate.tool_calls = vec![lettuce_conversations::ProposedToolCall {
                provider_call_id: Some("done".into()), name: "done".into(),
                arguments: serde_json::json!({}), raw_arguments: None, provider_replay: None,
            }];
        }
    }));
    work.run(harness.context.clone(), Arc::new(IgnoreProgress)).await.expect("settle cycle");
    let job = JobStore::get(database, job.id).expect("job").expect("present");
    assert_eq!(job.state, if cancel { lettuce_jobs::JobState::Cancelled } else { lettuce_jobs::JobState::Succeeded }, "cycle failure: {:?}", job.error);
    assert_eq!(manager_rounds.load(Ordering::SeqCst), if cancel { 2 } else { 3 });
    assert_eq!(counts.embeddings.load(Ordering::SeqCst), if cancel { 2 } else { 4 });
    assert_eq!(counts.loads.load(Ordering::SeqCst), 1);
    assert_eq!(counts.drops.load(Ordering::SeqCst), usize::from(!keep));
    assert_eq!(harness.context.models().is_loaded(RequiredModel::Embedding), keep);
    *harness.provider.response_hook.lock().expect("clear hook") = None;
}

#[tokio::test]
async fn a_memory_cycle_reuses_eidos_across_tool_rounds_and_obeys_retention_at_completion() {
    for keep in [false, true] { run_cycle(keep, false).await; }
}

#[tokio::test]
async fn a_cancelled_memory_cycle_releases_its_eidos_session_when_retention_is_disabled() {
    run_cycle(false, true).await;
}

async fn setup_five_message_dynamic_chat(
    prefix: &str,
    run_mode: lettuce_settings::MemoryRunMode,
) -> (
    crate::api::tests::Harness,
    String,
    lettuce_types::ConversationId,
) {
    let counts = Arc::new(SessionCounts::default());
    let harness = harness_in(
        Reply::Text("The conversation establishes lasting facts."),
        Arc::new(SystemClock),
        None,
        None,
        Arc::new(CycleModels(counts)),
    );
    let (chat, _) = crate::api::conversations::turns_tests::replied_chat(&harness, prefix).await;
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("conversation id");
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id)
        .expect("character")
        .expect("present")
        .character;
    let mut defaults = character.defaults;
    defaults.memory_policy = lettuce_characters::MemoryPolicy::Dynamic;
    CharacterRepository::update_defaults(
        database,
        harness.character_id,
        character.revision,
        defaults,
        harness.context.now(),
    )
    .expect("dynamic character");
    let stored = database.load().expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    settings.dynamic_memory.run_mode = run_mode;
    database
        .save(settings, stored.default_model_profile_id, stored.revision)
        .expect("settings");

    crate::api::tests::send(
        &harness,
        &chat,
        &format!("{prefix}-turn-2"),
        "Second user turn",
        Arc::new(crate::api::tests::RecordingStream::default()),
    )
    .await
    .expect("second turn send");
    crate::api::conversations::turns_tests::run_generation(&harness).await;
    let aggregate = lettuce_conversations::ConversationReader::get(database, conversation_id)
        .expect("conversation");
    let user_participant = aggregate
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User)
        .expect("user participant")
        .id;
    let _ = lettuce_conversations::ConversationRepository::begin_send(
        database,
        &lettuce_conversations::SendConversation {
            conversation_id,
            branch_id: aggregate.conversation.active_branch_id,
            expected_revision: aggregate.conversation.revision,
            operation: lettuce_conversations::OperationToken {
                key: lettuce_jobs::IdempotencyKey::new(format!("{prefix}-turn-3")).expect("key"),
                request_digest: lettuce_types::ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            message: lettuce_conversations::MessageDraft {
                role: lettuce_conversations::MessageRole::User,
                author_participant_id: Some(user_participant),
                parts: vec![lettuce_conversations::MessagePart::Text {
                    text: "Fifth message past the chunk cap".into(),
                }],
                visibility: lettuce_conversations::MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
            swap_roles: false,
        },
        harness.context.now(),
    )
    .expect("fifth message");

    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(|request, outcome| {
        if !request
            .tools
            .as_ref()
            .is_some_and(|tools| tools.definitions.iter().any(|tool| tool.name == "create_memory"))
        {
            return;
        }
        let candidate = outcome.candidates.first_mut().expect("candidate");
        candidate.parts.clear();
        candidate.tool_calls = vec![lettuce_conversations::ProposedToolCall {
            provider_call_id: Some("done".into()),
            name: "done".into(),
            arguments: serde_json::json!({}),
            raw_arguments: None,
            provider_replay: None,
        }];
    }));

    (harness, chat, conversation_id)
}

#[tokio::test]
async fn forced_memory_trigger_chains_remaining_chunk_via_job_handler_without_overlap() {
    let (harness, chat, conversation_id) =
        setup_five_message_dynamic_chat("chain-forced", lettuce_settings::MemoryRunMode::Manual)
            .await;
    let database = harness.context.backend().database();
    let accepted = crate::api::memory_trigger(
        &harness.context,
        dto::MemoryTriggerRequest {
            conversation_id: chat,
            client_operation_id: "chain-forced-trigger".into(),
        },
    )
    .await
    .expect("trigger");
    let first_id: lettuce_types::JobId = accepted.job_id.parse().expect("first job id");
    let first_job = JobStore::get(database, first_id).expect("job").expect("present");
    let first_work = MemoryExtractionHandler
        .claim(&harness.context, &first_job, WorkerId::new())
        .await
        .expect("claim first")
        .expect("first work");
    first_work
        .run(harness.context.clone(), Arc::new(IgnoreProgress))
        .await
        .expect("run first chunk");

    let first_done = JobStore::get(database, first_id).expect("job").expect("present");
    assert_eq!(first_done.state, lettuce_jobs::JobState::Succeeded);
    let space = lettuce_memory::MemoryRepository::get_for_branch(
        database,
        conversation_id,
        lettuce_conversations::ConversationReader::get(database, conversation_id)
            .expect("conv")
            .conversation
            .active_branch_id,
    )
    .expect("space")
    .expect("present");
    let first_summary = lettuce_memory::MemorySummaryRepository::get_summary(database, space.id)
        .expect("summary")
        .expect("stored");
    assert_eq!((first_summary.window_start, first_summary.window_end), (0, 4));

    let queued = JobStore::list(
        database,
        lettuce_jobs::JobQuery {
            state: Some(lettuce_jobs::JobState::Queued),
            kind: Some(lettuce_jobs::JobKind::MemoryExtraction),
            subject: Some(
                lettuce_jobs::SubjectId::new(conversation_id.to_string()).expect("subject"),
            ),
            page: lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(10),
            },
        },
    )
    .expect("queued follow-up")
    .items;
    assert_eq!(queued.len(), 1);
    let second_job = queued.into_iter().next().expect("second job");
    let second_work = MemoryExtractionHandler
        .claim(&harness.context, &second_job, WorkerId::new())
        .await
        .expect("claim second")
        .expect("second work");
    second_work
        .run(harness.context.clone(), Arc::new(IgnoreProgress))
        .await
        .expect("run second chunk");

    let second_done = JobStore::get(database, second_job.id)
        .expect("job")
        .expect("present");
    assert_eq!(second_done.state, lettuce_jobs::JobState::Succeeded);
    let second_summary = lettuce_memory::MemorySummaryRepository::get_summary(database, space.id)
        .expect("summary")
        .expect("stored");
    assert_eq!((second_summary.window_start, second_summary.window_end), (4, 5));

    let remaining = JobStore::list(
        database,
        lettuce_jobs::JobQuery {
            state: Some(lettuce_jobs::JobState::Queued),
            kind: Some(lettuce_jobs::JobKind::MemoryExtraction),
            subject: Some(
                lettuce_jobs::SubjectId::new(conversation_id.to_string()).expect("subject"),
            ),
            page: lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(10),
            },
        },
    )
    .expect("remaining jobs")
    .items;
    assert!(remaining.is_empty());
    *harness.provider.response_hook.lock().expect("clear hook") = None;
}

#[tokio::test]
async fn automatic_memory_cycle_does_not_chain_a_forced_chunk_when_remainder_is_below_interval() {
    let (harness, _chat, conversation_id) =
        setup_five_message_dynamic_chat("chain-auto", lettuce_settings::MemoryRunMode::Auto).await;
    let database = harness.context.backend().database();
    let admitted = crate::CompanionPostTurnMemoryAdmissionCoordinator::new(database, database)
        .admit_plain_after_turn(
            conversation_id,
            2,
            lettuce_memory::DynamicMemoryRunMode::Auto,
            harness.context.now(),
        )
        .expect("auto admission")
        .expect("auto batch");
    assert!(!admitted.batch.chain_follow_up);
    assert_eq!(admitted.batch.unsummarized_message_count, 5);
    assert_eq!(
        admitted
            .batch
            .source_messages()
            .expect("auto sources")
            .len(),
        4
    );

    let work = MemoryExtractionHandler
        .claim(&harness.context, &admitted.job, WorkerId::new())
        .await
        .expect("claim auto")
        .expect("auto work");
    work.run(harness.context.clone(), Arc::new(IgnoreProgress))
        .await
        .expect("run auto chunk");

    let done = JobStore::get(database, admitted.job.id)
        .expect("job")
        .expect("present");
    assert_eq!(done.state, lettuce_jobs::JobState::Succeeded);
    let queued = JobStore::list(
        database,
        lettuce_jobs::JobQuery {
            state: Some(lettuce_jobs::JobState::Queued),
            kind: Some(lettuce_jobs::JobKind::MemoryExtraction),
            subject: Some(
                lettuce_jobs::SubjectId::new(conversation_id.to_string()).expect("subject"),
            ),
            page: lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(10),
            },
        },
    )
    .expect("queued jobs")
    .items;
    assert!(queued.is_empty());
    *harness.provider.response_hook.lock().expect("clear hook") = None;
}
