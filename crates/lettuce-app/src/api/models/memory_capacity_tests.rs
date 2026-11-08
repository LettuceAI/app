use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use async_trait::async_trait;
use lettuce_characters::{CharacterRepository, InteractionMode, MemoryPolicy};
use lettuce_contracts::{self as dto, RequiredModel};
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::{SystemClock, handle::CancellationToken};
use lettuce_settings::{DeviceSettingsStore, GlobalSettingsStore};

use crate::{CompanionEmotionEngine, EmbeddingGenerationError, MemoryEmbeddingEngine};
use crate::api::{ApiContext, ModelLoad, ModelLoader};
use crate::api::memory::worker::MemoryWorker;
use crate::api::tests::{Harness, NoImages, RecordingEvents, RecordingStream, Reply, StdFiles, harness_in, harness_over, launch, send};
use crate::api::worker::{ConversationGenerationWorker, WorkerStep};

const FACTS: [&str; 24] = [
    "Amber dragons nest beneath volcanic ridges.",
    "Silver bells summon harbor guardians.",
    "Mira inherited a sapphire crown.",
    "Desert caravans trade enchanted silk.",
    "Ancient oaks conceal sleeping giants.",
    "Winter festivals celebrate fallen heroes.",
    "Captain Rowan commands seven ships.",
    "Moonlit mushrooms cure poisoned wounds.",
    "Stone golems patrol abandoned mines.",
    "Northern monks practice silent meditation.",
    "Crimson wolves hunt during eclipses.",
    "Elena studies forbidden weather magic.",
    "Golden bridges connect floating islands.",
    "Baker Tomas secretly shelters fugitives.",
    "Crystal lanterns repel wandering spirits.",
    "Emerald rivers flow toward sunrise.",
    "Queen Lyra outlawed necromancy yesterday.",
    "Copper coins bear owl engravings.",
    "Jasper lost his compass gambling.",
    "Velvet curtains hide secret doorways.",
    "Lighthouse keepers raise messenger falcons.",
    "Obsidian mirrors reveal distant cities.",
    "Autumn rains awaken buried seeds.",
    "Pearl necklaces signify royal protection.",
];

struct CapacityModels;
struct CapacityEmbedding;

impl MemoryEmbeddingEngine for CapacityEmbedding {
    fn source_revision(&self) -> &str { "capacity-test" }
    fn dimensions(&self) -> EmbeddingDimensions { EmbeddingDimensions::D64 }
    fn count_tokens(&self, _: &str) -> Result<u32, EmbeddingGenerationError> { Ok(1) }
    fn embed_memory(&self, request: &EmbeddingRequest, cancellation: &CancellationToken) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        if cancellation.is_cancelled() { return Err(EmbeddingGenerationError::Cancelled); }
        // Distinct facts must not be removed by semantic deduplication. Queries
        // use a separate dimension so retrieval cannot manufacture duplicates.
        let index = FACTS.iter().position(|fact| *fact == request.text).unwrap_or(63);
        let mut values = vec![0.0; request.dimensions.get()];
        values[index] = 1.0;
        Ok(EmbeddingVector { source_revision: self.source_revision().into(), values })
    }
}

#[async_trait]
impl ModelLoader for CapacityModels {
    fn installed(&self, _: &ApiContext, model: RequiredModel) -> bool { model == RequiredModel::Embedding }
    async fn prepare(&self, _: &ApiContext) -> bool { true }
    fn embedding(&self, _: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        ModelLoad::Loaded(Arc::new(CapacityEmbedding))
    }
    fn emotion(&self, _: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> { ModelLoad::NotInstalled }
}

async fn roleplay_capacity(batch: bool) {
    let harness = harness_in(
        Reply::Text("The adventure continues."), Arc::new(SystemClock), None, None,
        Arc::new(CapacityModels),
    );
    run_capacity(&harness, batch, if batch { 1 } else { FACTS.len() }).await;
}

async fn run_capacity(harness: &Harness, batch: bool, cycles: usize) -> (String, Vec<serde_json::Value>) {
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id).expect("character").expect("present").character;
    let mut defaults = character.defaults;
    assert_eq!(defaults.interaction_mode, InteractionMode::Roleplay);
    defaults.memory_policy = MemoryPolicy::Dynamic;
    CharacterRepository::update_defaults(database, harness.character_id, character.revision, defaults, harness.context.now()).expect("dynamic roleplay");
    let stored = database.load().expect("settings");
    let mut settings = stored.settings;
    assert_eq!(settings.dynamic_memory.max_entries, 200);
    assert_eq!(settings.dynamic_memory.summary_message_interval, 20);
    settings.dynamic_memory.enabled = true;
    // Single-round tool replies exercise the default non-recursive workflow.
    assert!(!settings.dynamic_memory.recursive_memory_loops);
    database.save(settings, stored.default_model_profile_id, stored.revision).expect("settings");
    database.update_device_settings(&|device| device.embedding.keep_model_loaded = true).expect("retention");

    let rounds = Arc::new(AtomicUsize::new(0));
    let observed = rounds.clone();
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(move |request, outcome| {
        if !request.tools.as_ref().is_some_and(|tools| tools.definitions.iter().any(|tool| tool.name == "create_memory")) { return; }
        let round = observed.fetch_add(1, Ordering::SeqCst);
        let range = if batch { 0..FACTS.len() } else { round..round + 1 };
        let candidate = outcome.candidates.first_mut().expect("candidate");
        candidate.parts.clear();
        candidate.tool_calls = range.map(|index| lettuce_conversations::ProposedToolCall {
            provider_call_id: Some(format!("fact-{index}")),
            name: "create_memory".into(),
            arguments: serde_json::json!({ "text": FACTS[index], "category": "world_detail" }),
            raw_arguments: None,
            provider_replay: None,
        }).collect();
    }));

    let chat = launch(harness, "capacity-launch").await;
    let generation = ConversationGenerationWorker::new(harness.context.clone());
    let memory = MemoryWorker::new(harness.context.clone());
    let mut report = Vec::new();
    for cycle in 0..cycles {
        let facts = if batch { FACTS.join(" ") } else { FACTS[cycle].into() };
        for turn in 0..10 {
            send(harness, &chat, &format!("capacity-{cycle}-{turn}"),
                if turn == 0 { &facts } else { "Continue the adventure." }, Arc::new(RecordingStream::default()))
                .await.expect("send");
            assert!(generation.run_once().await.expect("generate"));
            assert!(memory.step().await.expect("automatic memory worker"));
        }
        let view = crate::api::memory_get(&harness.context, dto::ConversationRequest { conversation_id: chat.clone() })
            .await.expect("memory page");
        assert_eq!(view.status.failure, None, "cycle {cycle}: {:?}", view.status);
        assert_eq!(view.status.latest_cycle_status, Some(dto::MemoryCycleStatus::Complete));
        assert_eq!(view.items.len(), if batch { FACTS.len() } else { cycle + 1 }, "cycle {cycle}");
        assert_eq!(view.status.total_conversation_messages, (cycle as u64 + 1) * 20);
        assert_eq!(view.status.messages_since_last_cycle, 0, "summary cursor must advance");
        for fact in &FACTS[..view.items.len()] {
            assert!(view.items.iter().any(|item| item.text == *fact), "missing {fact}");
        }
        report.push(serde_json::json!({ "cycle": cycle + 1, "saved_memories": view.items.len(), "status": view.status }));
    }
    assert_eq!(rounds.load(Ordering::SeqCst), cycles);
    (chat, report)
}

#[tokio::test]
async fn roleplay_saves_more_than_sixteen_memories_in_one_automatic_cycle() {
    roleplay_capacity(true).await;
}

#[tokio::test]
async fn roleplay_keeps_saving_past_sixteen_cycles_and_history_page_boundaries() {
    roleplay_capacity(false).await;
}

#[tokio::test]
#[ignore = "needs installed Eidos and ONNX Runtime in LETTUCE_MEMORY_LAB_MODELS_DIR; no provider calls"]
async fn automatic_roleplay_keeps_saving_past_sixteen_with_installed_eidos() {
    let root = std::path::PathBuf::from(std::env::var_os("LETTUCE_MEMORY_LAB_MODELS_DIR").expect("set the app folder containing installed Eidos and ONNX Runtime"));
    assert!(root.join("onnxruntime").is_dir());
    let harness = harness_in(
        Reply::Text("The adventure continues."), Arc::new(SystemClock), None, Some(root),
        Arc::new(crate::api::InstalledModels),
    );
    assert!(crate::api::InstalledModels.installed(&harness.context, RequiredModel::Embedding));
    run_capacity(&harness, false, FACTS.len()).await;
    assert!(harness.context.models().is_loaded(RequiredModel::Embedding));
    assert_eq!(harness.context.embedding().source_revision(), "v5");
}

#[tokio::test]
async fn automatic_roleplay_drains_coalesced_windows_and_recovers_after_a_failed_cycle() {
    let harness = harness_in(
        Reply::Text("The adventure continues."), Arc::new(SystemClock), None, None,
        Arc::new(CapacityModels),
    );
    let (chat, _) = run_capacity(&harness, false, 16).await;
    let generation = ConversationGenerationWorker::new(harness.context.clone());
    let memory = MemoryWorker::new(harness.context.clone());
    // Several ordinary turns can complete before the shared memory worker
    // gets to this conversation. Their wake-ups coalesce into one queue entry.
    for cycle in 16..19 {
        for turn in 0..10 {
            send(&harness, &chat, &format!("burst-{cycle}-{turn}"),
                if turn == 0 { FACTS[cycle] } else { "Continue the adventure." }, Arc::new(RecordingStream::default()))
                .await.expect("burst send");
            assert!(generation.run_once().await.expect("burst reply"));
        }
    }
    assert!(memory.step().await.expect("drain automatic work"));
    let request = dto::ConversationRequest { conversation_id: chat.clone() };
    let view = crate::api::memory_get(&harness.context, request.clone()).await.expect("memory page");
    assert_eq!(view.items.len(), 19, "all three queued windows must finish without another send or a manual trigger");
    assert_eq!(view.status.messages_since_last_cycle, 0);
    assert!(!memory.step().await.expect("queue is idle"));

    let original = harness.provider.response_hook.lock().expect("hook").take().expect("original hook");
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(|request, outcome| {
        if request.tools.as_ref().is_some_and(|tools| tools.definitions.iter().any(|tool| tool.name == "create_memory")) {
            outcome.candidates.clear();
        }
    }));
    for turn in 0..10 {
        send(&harness, &chat, &format!("failed-cycle-{turn}"),
            if turn == 0 { FACTS[19] } else { "Continue the adventure." }, Arc::new(RecordingStream::default()))
            .await.expect("send before failure");
        assert!(generation.run_once().await.expect("reply before failure"));
    }
    assert!(memory.step().await.expect("failed automatic cycle must settle"));
    let failed = crate::api::memory_get(&harness.context, request.clone()).await.expect("failed status");
    assert_eq!(failed.items.len(), 19);
    assert_eq!(failed.status.latest_cycle_status, Some(dto::MemoryCycleStatus::Failed));
    assert_eq!(failed.status.messages_since_last_cycle, 20);
    assert!(!memory.step().await.expect("failed cycle must not spin"));

    *harness.provider.response_hook.lock().expect("restore hook") = Some(original);
    send(&harness, &chat, "recovery-send", "Continue the adventure.", Arc::new(RecordingStream::default()))
        .await.expect("recovery send");
    assert!(generation.run_once().await.expect("recovery reply"));
    assert!(memory.step().await.expect("ordinary send retries the failed window"));
    let recovered = crate::api::memory_get(&harness.context, request).await.expect("recovered status");
    assert_eq!(recovered.items.len(), 20);
    assert_eq!(recovered.status.latest_cycle_status, Some(dto::MemoryCycleStatus::Complete));
    assert_eq!(recovered.status.messages_since_last_cycle, 2);
    assert!(!memory.step().await.expect("recovered queue is idle"));
}

/// Produces a disposable desktop profile for taking over with real models.
/// The ordinary capacity tests stay isolated in memory and require no files.
#[tokio::test]
#[ignore = "creates a fresh desktop profile at LETTUCE_MEMORY_LAB_DIR"]
async fn prepare_roleplay_memory_lab() {
    let root = std::path::PathBuf::from(std::env::var_os("LETTUCE_MEMORY_LAB_DIR").expect("set LETTUCE_MEMORY_LAB_DIR to a fresh absolute directory"));
    assert!(root.is_absolute() && !root.exists(), "the lab directory must be absolute and must not exist");
    let bootstrap = ApiContext::open_desktop(
        &root, None, Arc::new(lettuce_settings::InMemorySecretStore::new()),
        Arc::new(RecordingEvents::default()), Arc::new(StdFiles), "test-asset://host".into(), None,
    ).expect("initialize desktop profile");
    let path = bootstrap.database_files().expect("desktop database").active.clone();
    drop(bootstrap);
    let backend = Arc::new(crate::AppBackend::open(&path, lettuce_types::TimestampMillis::new(1)).expect("lab backend"));
    let harness = harness_over(
        backend, Reply::Text("The adventure continues."), Arc::new(SystemClock), None,
        Some(root.clone()), Arc::new(CapacityModels), Arc::new(NoImages),
    );
    let (chat, cycles) = run_capacity(&harness, false, 16).await;
    let generation = ConversationGenerationWorker::new(harness.context.clone());
    for turn in 0..10 {
        send(&harness, &chat, &format!("pending-{turn}"),
            if turn == 0 { FACTS[16] } else { "Continue the adventure." }, Arc::new(RecordingStream::default()))
            .await.expect("pending source message");
        assert!(generation.run_once().await.expect("pending reply"));
        // Leave this window unprocessed for the real model in the desktop app.
    }
    let pending = crate::api::memory_get(&harness.context, dto::ConversationRequest { conversation_id: chat.clone() })
        .await.expect("lab baseline");
    assert_eq!(pending.items.len(), 16);
    assert_eq!(pending.status.messages_since_last_cycle, 20);
    let report = serde_json::json!({
        "mode": "controlled fixture; real model extraction has not run",
        "conversation_id": chat,
        "database": path,
        "cycles": cycles,
        "baseline": pending,
        "next_fact": FACTS[16],
    });
    std::fs::write(root.join("report.json"), serde_json::to_vec_pretty(&report).expect("report JSON")).expect("report");
    let config = serde_json::json!({
        "identifier": "dev.lettuceai.memorylab",
        "productName": "LettuceAI Memory Lab",
        "app": { "appDirectoriesOverride": root },
    });
    std::fs::write(root.join("tauri.memory-lab.conf.json"), serde_json::to_vec_pretty(&config).expect("config JSON")).expect("config");
    println!("Memory lab prepared at {}", root.display());
}
