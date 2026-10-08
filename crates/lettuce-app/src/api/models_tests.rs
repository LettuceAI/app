use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use lettuce_characters::CharacterDefaults;
use lettuce_companions::EmotionClassification;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails, RequiredModel};
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::{SystemClock, handle::CancellationToken};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{CharacterId, ConversationId};

use super::models::{TurnOperation, require_conversation_models};
use super::tests::{Harness, Reply, create_character, harness_in};
use super::{
    ApiContext, ModelLoad, ModelLoader, conversation_launch_direct, conversation_open,
    conversation_send,
};
use crate::{
    CompanionEmotionEngine, CompanionEmotionGenerationError, EmbeddingGenerationError,
    MemoryEmbeddingEngine,
};

struct FixedEmbedding;

impl MemoryEmbeddingEngine for FixedEmbedding {
    fn source_revision(&self) -> &str {
        "fixed"
    }

    fn dimensions(&self) -> EmbeddingDimensions {
        EmbeddingDimensions::from_preference(None)
    }

    fn count_tokens(&self, _text: &str) -> Result<u32, EmbeddingGenerationError> {
        Ok(1)
    }

    fn embed_memory(
        &self,
        _request: &EmbeddingRequest,
        _cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        Err(EmbeddingGenerationError::Unavailable)
    }
}

struct NeutralEmotion;

impl CompanionEmotionEngine for NeutralEmotion {
    fn classify_emotion(
        &self,
        _text: &str,
        _cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        Ok(None)
    }
}

struct FailingEmotion;

impl CompanionEmotionEngine for FailingEmotion {
    fn classify_emotion(
        &self,
        _text: &str,
        _cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        Err(CompanionEmotionGenerationError::Unavailable)
    }
}

/// Counts every question and load; models are installed when `installed`
/// and load when `loadable`.
#[derive(Default)]
struct CountingModels {
    installed: AtomicBool,
    loadable: AtomicBool,
    failing_emotion: AtomicBool,
    absent: std::sync::Mutex<Vec<RequiredModel>>,
    checks: AtomicUsize,
    prepares: AtomicUsize,
    loads: AtomicUsize,
}

impl CountingModels {
    fn new(installed: bool, loadable: bool) -> Arc<Self> {
        let models = Self::default();
        models.installed.store(installed, Ordering::SeqCst);
        models.loadable.store(loadable, Ordering::SeqCst);
        Arc::new(models)
    }

    fn has(&self, model: RequiredModel) -> bool {
        self.installed.load(Ordering::SeqCst)
            && !self.absent.lock().expect("absent models").contains(&model)
    }

    fn calls(&self) -> (usize, usize, usize) {
        (
            self.checks.load(Ordering::SeqCst),
            self.prepares.load(Ordering::SeqCst),
            self.loads.load(Ordering::SeqCst),
        )
    }
}

#[async_trait]
impl ModelLoader for CountingModels {
    fn installed(&self, _context: &ApiContext, model: RequiredModel) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.has(model)
    }

    async fn prepare(&self, _context: &ApiContext) -> bool {
        self.prepares.fetch_add(1, Ordering::SeqCst);
        true
    }

    fn embedding(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if !self.has(RequiredModel::Embedding) {
            ModelLoad::NotInstalled
        } else if self.loadable.load(Ordering::SeqCst) {
            ModelLoad::Loaded(Arc::new(FixedEmbedding))
        } else {
            ModelLoad::Unavailable
        }
    }

    fn emotion(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if !self.has(RequiredModel::Emotion) {
            ModelLoad::NotInstalled
        } else if self.failing_emotion.load(Ordering::SeqCst) {
            ModelLoad::Loaded(Arc::new(FailingEmotion))
        } else if self.loadable.load(Ordering::SeqCst) {
            ModelLoad::Loaded(Arc::new(NeutralEmotion))
        } else {
            ModelLoad::Unavailable
        }
    }
}

fn harness_with_models(models: Arc<CountingModels>) -> Harness {
    harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        None,
        models,
    )
}

fn companion_defaults() -> CharacterDefaults {
    CharacterDefaults {
        interaction_mode: lettuce_characters::InteractionMode::Companion,
        companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
        ..CharacterDefaults::default()
    }
}

fn dynamic_defaults() -> CharacterDefaults {
    CharacterDefaults {
        memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
        ..CharacterDefaults::default()
    }
}

fn enable_dynamic_memory(harness: &Harness) {
    let database = harness.context.backend().database();
    let stored = database.load().expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    database
        .save(settings, stored.default_model_profile_id, stored.revision)
        .expect("enable dynamic memory");
}

async fn launch(harness: &Harness, character_id: CharacterId, key: &str) -> String {
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
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

async fn send(harness: &Harness, conversation_id: &str, key: &str) -> Result<(), dto::ApiError> {
    conversation_send(
        &harness.context,
        dto::ConversationSendRequest {
            conversation_id: conversation_id.into(),
            text: "Hello there".into(),
            client_operation_id: key.into(),
        },
        Arc::new(NoStream),
    )
    .await
    .map(|_| ())
}

struct NoStream;

impl super::GenerationEventSink for NoStream {
    fn emit(&self, _event: dto::GenerationEvent) {}
}

async fn assert_untouched(harness: &Harness, conversation_id: &str) {
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    .expect("open");
    assert!(view.messages.items.is_empty());
    assert!(view.pending_turn_id.is_none());
    assert!(view.can_send);
}

fn model_of(error: &dto::ApiError) -> Option<RequiredModel> {
    match error.details {
        Some(ApiErrorDetails::Model { model }) => Some(model),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_chat_without_the_emotion_model_is_refused_and_untouched() {
    let models = CountingModels::new(false, false);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let conversation_id = launch(&harness, companion, "companion-launch").await;
    let error = send(&harness, &conversation_id, "companion-send")
        .await
        .expect_err("refused");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(model_of(&error), Some(RequiredModel::Emotion));
    assert_untouched(&harness, &conversation_id).await;
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dynamic_memory_chat_without_the_embedding_model_is_refused() {
    let models = CountingModels::new(false, false);
    let harness = harness_with_models(Arc::clone(&models));
    enable_dynamic_memory(&harness);
    let character = create_character(
        harness.context.backend().database(),
        "Rin",
        dynamic_defaults(),
    );
    let conversation_id = launch(&harness, character, "dynamic-launch").await;
    let error = send(&harness, &conversation_id, "dynamic-send")
        .await
        .expect_err("refused");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(model_of(&error), Some(RequiredModel::Embedding));
    assert_untouched(&harness, &conversation_id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_installed_model_that_cannot_load_refuses_the_send() {
    let models = CountingModels::new(true, false);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let conversation_id = launch(&harness, companion, "broken-launch").await;
    let error = send(&harness, &conversation_id, "broken-send")
        .await
        .expect_err("refused");
    assert_eq!(error.code, ApiErrorCode::ModelUnavailable);
    assert_eq!(model_of(&error), Some(RequiredModel::Emotion));
    assert_untouched(&harness, &conversation_id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_memory_roleplay_chat_needs_no_model() {
    let models = CountingModels::new(false, false);
    let harness = harness_with_models(Arc::clone(&models));
    let conversation_id = launch(&harness, harness.character_id, "plain-launch").await;
    send(&harness, &conversation_id, "plain-send")
        .await
        .expect("sends");
    assert_eq!(models.calls(), (0, 0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn installed_models_load_once_until_they_change() {
    let models = CountingModels::new(true, true);
    let harness = harness_with_models(Arc::clone(&models));
    use lettuce_settings::DeviceSettingsStore;
    let database = harness.context.backend().database();
    let mut device = database.load_device_settings().expect("device settings");
    device.embedding.keep_model_loaded = true;
    database.update_device_settings(&|current| current.clone_from(&device)).expect("save setting");
    enable_dynamic_memory(&harness);
    let defaults = CharacterDefaults {
        memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
        ..companion_defaults()
    };
    let companion = create_character(harness.context.backend().database(), "Mira", defaults);
    let conversation_id = launch(&harness, companion, "lazy-launch").await;
    let conversation: ConversationId = conversation_id.parse().expect("id");
    assert_eq!(models.calls(), (0, 0, 0));
    require_conversation_models(&harness.context, conversation, TurnOperation::Send)
        .await
        .expect("models load");
    assert_eq!(models.calls(), (2, 2, 2));
    require_conversation_models(&harness.context, conversation, TurnOperation::Send)
        .await
        .expect("models stay loaded");
    assert_eq!(models.calls(), (2, 2, 2));
    let embedding = harness.context.embedding();
    assert_eq!(embedding.source_revision(), "fixed");
    assert!(embedding.requires_model());
    assert_eq!(models.calls(), (2, 2, 2));
    send(&harness, &conversation_id, "lazy-send")
        .await
        .expect("sends");
    assert_eq!(models.calls(), (2, 2, 2));

    harness.context.models_changed();
    require_conversation_models(&harness.context, conversation, TurnOperation::Send)
        .await
        .expect("models reload");
    assert_eq!(models.calls(), (4, 4, 4));
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_on_a_fresh_install_fetches_and_loads_nothing() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-api-fresh-{}",
        lettuce_types::OperationId::new()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let models = CountingModels::new(false, false);
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::clone(&models) as Arc<dyn ModelLoader>,
    );
    let workers = super::startup(&harness.context).await.expect("startup");
    tokio::time::timeout(Duration::from_secs(30), workers.started())
        .await
        .expect("workers started");
    workers.stop().await;
    assert_eq!(models.calls(), (0, 0, 0));
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
    assert!(!root.join("onnxruntime").exists());
    assert!(!root.join("downloads").exists());
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_send_whose_classification_fails_is_refused() {
    let models = CountingModels::new(true, true);
    models.failing_emotion.store(true, Ordering::SeqCst);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let conversation_id = launch(&harness, companion, "failing-launch").await;
    let error = send(&harness, &conversation_id, "failing-send")
        .await
        .expect_err("refused");
    assert_eq!(error.code, ApiErrorCode::ModelUnavailable);
    assert_eq!(model_of(&error), Some(RequiredModel::Emotion));
    assert_untouched(&harness, &conversation_id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_accepted_send_replays_after_its_model_is_removed() {
    let models = CountingModels::new(true, true);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let conversation_id = launch(&harness, companion, "replay-launch").await;
    send(&harness, &conversation_id, "replay-send")
        .await
        .expect("first send");
    models.installed.store(false, Ordering::SeqCst);
    harness.context.models_changed();
    send(&harness, &conversation_id, "replay-send")
        .await
        .expect("the same send replays");
    let conflict = conversation_send(
        &harness.context,
        dto::ConversationSendRequest {
            conversation_id: conversation_id.clone(),
            text: "Something else".into(),
            client_operation_id: "replay-send".into(),
        },
        Arc::new(NoStream),
    )
    .await
    .expect_err("a different send under the same key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    let refused = send(&harness, &conversation_id, "new-send")
        .await
        .expect_err("a new send needs the model");
    assert_eq!(refused.code, ApiErrorCode::ModelRequired);
}

#[tokio::test(flavor = "multi_thread")]
async fn adopting_legacy_embedding_files_forgets_a_missing_model() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-api-adopt-{}",
        lettuce_types::OperationId::new()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let models = CountingModels::new(false, false);
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::clone(&models) as Arc<dyn ModelLoader>,
    );
    enable_dynamic_memory(&harness);
    let character = create_character(
        harness.context.backend().database(),
        "Rin",
        dynamic_defaults(),
    );
    let conversation_id = launch(&harness, character, "adopt-launch").await;
    let conversation: ConversationId = conversation_id.parse().expect("id");
    let missing = require_conversation_models(&harness.context, conversation, TurnOperation::Send)
        .await
        .expect_err("no model yet");
    assert_eq!(missing.code, ApiErrorCode::ModelRequired);

    let legacy = root.join("lettuce").join("models").join("embedding");
    std::fs::create_dir_all(&legacy).expect("legacy folder");
    std::fs::write(legacy.join("v4-model.int8.onnx"), b"v4 model").expect("model");
    std::fs::write(legacy.join("v4-tokenizer.json"), b"v4 tokenizer").expect("tokenizer");
    models.installed.store(true, Ordering::SeqCst);
    models.loadable.store(true, Ordering::SeqCst);
    let workers = super::startup(&harness.context).await.expect("startup");
    require_conversation_models(&harness.context, conversation, TurnOperation::Send)
        .await
        .expect("the adopted model loads");
    tokio::time::timeout(Duration::from_secs(30), workers.started())
        .await
        .expect("workers started");
    workers.stop().await;
    std::fs::remove_dir_all(root).ok();
}

fn without(models: &CountingModels, model: RequiredModel) {
    models.absent.lock().expect("absent models").push(model);
}

async fn missing_in_list_and_view(harness: &Harness, conversation_id: &str) -> Vec<RequiredModel> {
    let listed = super::conversations_list(
        &harness.context,
        dto::ConversationsListRequest {
            lifecycle: Some(dto::LifecycleFilter::All),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await
    .expect("list")
    .items
    .into_iter()
    .find(|item| item.id == conversation_id)
    .expect("listed")
    .missing_models;
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    .expect("open");
    assert_eq!(view.missing_models, listed);
    listed
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_chat_misses_the_emotion_model_without_loading_anything() {
    let models = CountingModels::new(true, true);
    without(&models, RequiredModel::Emotion);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let conversation_id = launch(&harness, companion, "missing-emotion").await;
    assert_eq!(
        missing_in_list_and_view(&harness, &conversation_id).await,
        vec![RequiredModel::Emotion]
    );
    let (_, prepares, loads) = models.calls();
    assert_eq!((prepares, loads), (0, 0), "listing never loads a model");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_chat_needs_the_embedding_model_too() {
    let models = CountingModels::new(true, true);
    without(&models, RequiredModel::Embedding);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let conversation_id = launch(&harness, companion, "missing-embedding").await;
    assert_eq!(
        missing_in_list_and_view(&harness, &conversation_id).await,
        vec![RequiredModel::Embedding]
    );
    let (_, prepares, loads) = models.calls();
    assert_eq!((prepares, loads), (0, 0));
    let error = send(&harness, &conversation_id, "companion-no-embedding")
        .await
        .expect_err("a companion send needs the embedding model");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(model_of(&error), Some(RequiredModel::Embedding));
    assert_untouched(&harness, &conversation_id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dynamic_memory_chat_misses_the_embedding_model_and_a_manual_one_misses_nothing() {
    let models = CountingModels::new(false, false);
    let harness = harness_with_models(Arc::clone(&models));
    enable_dynamic_memory(&harness);
    let dynamic = create_character(
        harness.context.backend().database(),
        "Rin",
        dynamic_defaults(),
    );
    let dynamic_chat = launch(&harness, dynamic, "missing-dynamic").await;
    let manual_chat = launch(&harness, harness.character_id, "missing-manual").await;
    assert_eq!(
        missing_in_list_and_view(&harness, &dynamic_chat).await,
        vec![RequiredModel::Embedding]
    );
    assert_eq!(
        missing_in_list_and_view(&harness, &manual_chat).await,
        Vec::new()
    );
    let (_, prepares, loads) = models.calls();
    assert_eq!((prepares, loads), (0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_model_change_tells_every_window_to_re_read_missing_models() {
    let models = CountingModels::new(false, false);
    let harness = harness_with_models(Arc::clone(&models));
    harness.context.models_changed();
    assert_eq!(
        super::tests::api_events(&harness),
        vec![dto::ApiEvent::RequiredModelsChanged]
    );
}

async fn settle_generation(harness: &Harness) {
    let worker = super::ConversationGenerationWorker::new(harness.context.clone());
    while worker.run_once().await.expect("worker") {}
}

async fn regenerate(
    harness: &Harness,
    conversation_id: &str,
    message_id: &str,
    key: &str,
) -> Result<dto::GenerationAccepted, dto::ApiError> {
    let revision = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    .expect("open")
    .revision;
    super::conversation_regenerate(
        &harness.context,
        dto::ConversationRegenerateRequest {
            conversation_id: conversation_id.into(),
            message_id: message_id.into(),
            expected_revision: revision,
            client_operation_id: key.into(),
            guidance: None,
            model_profile_id: None,
            forced_speaker_participant_id: None,
            swap_places: false,
        },
        Arc::new(NoStream),
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_regenerate_needs_the_embedding_model_only_and_only_in_a_dynamic_chat() {
    let models = CountingModels::new(true, true);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let chat = launch(&harness, companion, "regen-models-launch").await;
    send(&harness, &chat, "regen-models-send")
        .await
        .expect("a companion send with both models");
    settle_generation(&harness).await;
    let reply = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("open")
    .messages
    .items
    .iter()
    .find(|message| message.role == dto::MessageRole::Assistant)
    .expect("reply")
    .id
    .clone();

    without(&models, RequiredModel::Emotion);
    harness.context.models_changed();
    regenerate(&harness, &chat, &reply, "regen-models-1")
        .await
        .expect("no classification, so no emotion model");
    settle_generation(&harness).await;

    enable_dynamic_memory(&harness);
    super::conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                memory: Some(dto::MemoryModeChange::Set {
                    mode: dto::MemoryMode::Dynamic,
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("dynamic memory");
    models.absent.lock().expect("absent models").clear();
    without(&models, RequiredModel::Embedding);
    harness.context.models_changed();
    let before = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("open");
    let error = regenerate(&harness, &chat, &reply, "regen-models-2")
        .await
        .expect_err("retrieval needs the embedding model");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(model_of(&error), Some(RequiredModel::Embedding));
    let after = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("open");
    assert_eq!(after.revision, before.revision);
    assert!(after.pending_turn_id.is_none());
    let continued = super::conversation_continue(
        &harness.context,
        dto::ConversationContinueRequest {
            conversation_id: chat.clone(),
            expected_revision: after.revision,
            client_operation_id: "regen-models-continue".into(),
            forced_speaker_participant_id: None,
            swap_places: false,
        },
        Arc::new(NoStream),
    )
    .await
    .expect_err("a continue retrieves memory too");
    assert_eq!(continued.code, ApiErrorCode::ModelRequired);
    assert_eq!(model_of(&continued), Some(RequiredModel::Embedding));
    let retried = super::conversation_retry(
        &harness.context,
        dto::ConversationRetryRequest {
            conversation_id: chat.clone(),
            turn_id: uuid::Uuid::new_v4().to_string(),
            client_operation_id: "regen-models-retry".into(),
        },
        Arc::new(NoStream),
    )
    .await
    .expect_err("a retry retrieves memory too");
    assert_eq!(retried.code, ApiErrorCode::ModelRequired);

    models.absent.lock().expect("absent models").clear();
    without(&models, RequiredModel::Emotion);
    harness.context.models_changed();
    regenerate(&harness, &chat, &reply, "regen-models-3")
        .await
        .expect("the embedding model is enough");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_send_without_an_emotion_engine_is_model_required() {
    let models = CountingModels::new(true, true);
    let harness = harness_with_models(Arc::clone(&models));
    let companion = create_character(
        harness.context.backend().database(),
        "Mira",
        companion_defaults(),
    );
    let chat = launch(&harness, companion, "no-engine-launch").await;
    let database = harness.context.backend().database();
    let stored = lettuce_conversations::ConversationReader::get(
        database,
        chat.parse().expect("conversation id"),
    )
    .expect("conversation")
    .conversation;
    let user = stored
        .participants
        .iter()
        .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User)
        .expect("user")
        .id;
    let command = lettuce_conversations::SendConversation {
        conversation_id: stored.id,
        branch_id: stored.active_branch_id,
        expected_revision: stored.revision,
        operation: crate::conversation::edit_operation("no-engine".into(), &[b"send"])
            .expect("token"),
        message: lettuce_conversations::MessageDraft {
            role: lettuce_conversations::MessageRole::User,
            author_participant_id: Some(user),
            parts: vec![lettuce_conversations::MessagePart::Text { text: "Hi".into() }],
            visibility: lettuce_conversations::MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
        },
        swap_roles: false,
    };
    let error =
        crate::CompanionTurnCoordinator::<_, dyn CompanionEmotionEngine>::new(database, None)
            .begin_send(&command, harness.context.now(), &CancellationToken::new())
            .expect_err("no emotion engine");
    assert!(matches!(error, crate::CompanionTurnError::EmotionRequired));
    let api = super::error::IntoApiError::into_api_error(error);
    assert_eq!(api.code, ApiErrorCode::ModelRequired);
    assert_eq!(
        api.details,
        Some(ApiErrorDetails::Model {
            model: RequiredModel::Emotion
        })
    );
    assert_untouched(&harness, &chat).await;
}

#[test]
fn keep_loaded_off_releases_the_embedding_between_uses() {
    use lettuce_settings::DeviceSettingsStore;
    let models = CountingModels::new(true, true);
    let harness = harness_with_models(Arc::clone(&models));
    let engine = harness.context.embedding();
    assert_eq!(engine.count_tokens("First use."), Ok(1));
    assert_eq!(engine.count_tokens("Second use."), Ok(1));
    assert_eq!(models.loads.load(Ordering::SeqCst), 2);
    let database = harness.context.backend().database();
    let mut device = database.load_device_settings().expect("device settings");
    device.embedding.keep_model_loaded = true;
    database.update_device_settings(&|current| current.clone_from(&device)).expect("save setting");
    assert_eq!(engine.count_tokens("Third use."), Ok(1));
    assert_eq!(engine.count_tokens("Fourth use."), Ok(1));
    assert_eq!(models.loads.load(Ordering::SeqCst), 3);
}

struct ComparisonModels {
    loads: AtomicUsize,
    invalid: bool,
}
struct ComparisonEngine {
    invalid: bool,
}
impl MemoryEmbeddingEngine for ComparisonEngine {
    fn source_revision(&self) -> &str {
        "comparison-test"
    }
    fn dimensions(&self) -> EmbeddingDimensions {
        EmbeddingDimensions::D64
    }
    fn count_tokens(&self, _text: &str) -> Result<u32, EmbeddingGenerationError> {
        Ok(1)
    }
    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        _cancel: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        let mut values = vec![0.0; 64];
        values[0] = if request.text == "opposite" {
            -1.0
        } else {
            1.0
        };
        if self.invalid {
            values[1] = f32::NAN;
        }
        Ok(EmbeddingVector {
            source_revision: "comparison-test".into(),
            values,
        })
    }
}
#[async_trait]
impl ModelLoader for ComparisonModels {
    fn installed(&self, _context: &ApiContext, model: RequiredModel) -> bool {
        model == RequiredModel::Embedding
    }
    async fn prepare(&self, _context: &ApiContext) -> bool {
        true
    }
    fn embedding(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        ModelLoad::Loaded(Arc::new(ComparisonEngine {
            invalid: self.invalid,
        }))
    }
    fn emotion(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> {
        ModelLoad::NotInstalled
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn embedding_comparison_uses_one_engine_per_call_and_preserves_raw_cosine() {
    let models = Arc::new(ComparisonModels {
        loads: AtomicUsize::new(0),
        invalid: false,
    });
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        None,
        models.clone(),
    );
    let request = || dto::EmbeddingCompareRequest {
        left: "same".into(),
        right: "opposite".into(),
    };
    let result = super::embedding_compare(&harness.context, request())
        .await
        .expect("compare");
    assert_eq!(result.cosine_similarity, -1.0);
    assert_eq!(result.dimensions, 64);
    assert_eq!(models.loads.load(Ordering::SeqCst), 1);
    super::embedding_compare(&harness.context, request())
        .await
        .expect("compare again");
    assert_eq!(
        models.loads.load(Ordering::SeqCst),
        2,
        "keep_model_loaded=false releases each call's engine"
    );
    use lettuce_settings::DeviceSettingsStore;
    let database = harness.context.backend().database();
    let mut settings = database.load_device_settings().expect("settings");
    settings.embedding.keep_model_loaded = true;
    database
        .update_device_settings(&|current| current.clone_from(&settings))
        .expect("keep loaded");
    super::embedding_compare(&harness.context, request())
        .await
        .expect("cache engine");
    super::embedding_compare(&harness.context, request())
        .await
        .expect("reuse engine");
    assert_eq!(models.loads.load(Ordering::SeqCst), 3);
    super::embedding_unload(&harness.context)
        .await
        .expect("unload");
    super::embedding_compare(&harness.context, request())
        .await
        .expect("load after unload");
    assert_eq!(models.loads.load(Ordering::SeqCst), 4);
}
#[tokio::test(flavor = "multi_thread")]
async fn embedding_comparison_rejects_invalid_vectors_and_missing_models() {
    let models = Arc::new(ComparisonModels {
        loads: AtomicUsize::new(0),
        invalid: true,
    });
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        None,
        models,
    );
    let request = || dto::EmbeddingCompareRequest {
        left: "same".into(),
        right: "same".into(),
    };
    assert_eq!(
        super::embedding_compare(&harness.context, request())
            .await
            .expect_err("NaN rejected")
            .code,
        ApiErrorCode::ModelUnavailable
    );
    let absent = harness_with_models(CountingModels::new(false, false));
    let error = super::embedding_compare(&absent.context, request())
        .await
        .expect_err("missing model");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(model_of(&error), Some(RequiredModel::Embedding));
}

#[tokio::test(flavor = "multi_thread")]
async fn embedding_inventory_choose_remove_and_busy_install_use_the_api() {
    let folder = std::env::temp_dir().join(format!(
        "embedding-api-{}",
        lettuce_types::OperationId::new()
    ));
    let root = crate::embedding_models_root(&folder);
    std::fs::create_dir_all(&root).expect("root");
    std::fs::write(root.join("v4-model.int8.onnx"), b"legacy model").expect("model");
    std::fs::write(root.join("v4-tokenizer.json"), b"legacy tokenizer").expect("tokenizer");
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        Some(folder.clone()),
        Arc::new(super::NoModels),
    );
    let database = harness.context.backend().database();
    crate::EmbeddingModelCoordinator::new(&root, database)
        .adopt_legacy_install(&root)
        .expect("adopt");
    let inventory = super::embedding_status(&harness.context)
        .await
        .expect("status");
    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].family, dto::EmbeddingFamily::LettuceEmbV4);
    assert!(inventory[0].active);
    let v4 = || dto::EmbeddingModelRequest {
        family: dto::EmbeddingFamily::LettuceEmbV4,
    };
    super::embedding_choose(&harness.context, v4())
        .await
        .expect("choose installed");
    assert_eq!(
        super::embedding_choose(
            &harness.context,
            dto::EmbeddingModelRequest {
                family: dto::EmbeddingFamily::LettuceEidosV5
            }
        )
        .await
        .expect_err("missing family")
        .code,
        ApiErrorCode::ModelRequired
    );
    let source = crate::ArtifactSource::HuggingFace {
        repository: "test/model".into(),
        revision: "ab".repeat(20),
        path: "model.onnx".into(),
    };
    let plan = crate::ArtifactInstallPlan {
        install_id: "embedding-busy-test".into(),
        root: root.clone(),
        artifacts: vec![crate::PlannedArtifact {
            artifact: lettuce_model_hub::PinnedArtifact {
                source_identity: source.identity(),
                local_segments: vec!["download-test.onnx".into()],
                byte_size: 1,
                sha256: Some("ab".repeat(32)),
            },
            source,
        }],
    };
    let admitted = crate::ArtifactInstallCoordinator::new(database)
        .admit(&plan)
        .expect("admit");
    harness.context.jobs().put_install(
        admitted.job.id,
        super::jobs::InstallWork::Artifact {
            plan,
            finish: Box::new(super::jobs::InstallFinish::Files),
        },
    );
    let error = super::embedding_remove(&harness.context, v4())
        .await
        .expect_err("install busy");
    assert_eq!(error.code, ApiErrorCode::Busy);
    assert!(matches!(
        error.details,
        Some(ApiErrorDetails::LocalModelsBusy {
            reason: dto::LocalModelsBusyReason::InstallActive { .. }
        })
    ));
    assert_eq!(
        super::embedding_status(&harness.context)
            .await
            .expect("kept")
            .len(),
        1
    );
    harness.context.jobs().forget_install(admitted.job.id);
    assert!(
        super::embedding_remove(&harness.context, v4())
            .await
            .expect("remove")
    );
    assert!(
        super::embedding_status(&harness.context)
            .await
            .expect("empty")
            .is_empty()
    );
    assert!(
        !super::embedding_remove(&harness.context, v4())
            .await
            .expect("repeat removal")
    );
    std::fs::remove_dir_all(folder).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn headless_model_guards_reach_missing_model_errors_and_install_recovery() {
    let harness = harness_with_models(CountingModels::new(false, false));
    let context = &harness.context;
    assert!(context.app_folder().is_none());
    assert!(
        context
            .retained_model_roots_for_guard()
            .expect("guard")
            .is_none()
    );
    let prepare = context
        .models()
        .prepare_embedding(context)
        .await
        .expect_err("missing embedding");
    assert_eq!(prepare.code, ApiErrorCode::ModelRequired);
    for model in [RequiredModel::Embedding, RequiredModel::Emotion] {
        let error = context
            .models()
            .require(context, model)
            .await
            .expect_err("missing model");
        assert_eq!(error.code, ApiErrorCode::ModelRequired);
        assert_eq!(error.details, Some(ApiErrorDetails::Model { model }));
    }
    let status = super::companion_emotion_status(context)
        .await
        .expect_err("missing emotion root");
    assert_eq!(status.code, ApiErrorCode::ModelRequired);
    let status = super::embedding_status(context)
        .await
        .expect_err("missing embedding root");
    assert_eq!(status.code, ApiErrorCode::ModelRequired);
    super::jobs::install::recover_queued_installs(context)
        .expect("no roots do not abort install recovery");
}
