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
        candidate.parts.clear();
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
