//! The optional embedding and emotion models. Nothing loads them at startup:
//! a chat that needs one checks it before its turn starts, which loads it
//! once (fetching ONNX Runtime first when the device has none), and the
//! loaded model is kept until an install, switch or removal changes it.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use async_trait::async_trait;
use lettuce_companions::EmotionClassification;
use lettuce_contracts::{ApiError, ApiErrorCode, RequiredModel};
use lettuce_conversations::{Conversation, ConversationReader};
use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingRequest, EmbeddingVector, SimilarityCalibration,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::ConversationId;

use super::ApiContext;
use super::context::UnavailableEmbedding;
use super::error::{IntoApiError, api_error, model_error};
use crate::{
    CompanionEmotionEngine, CompanionEmotionGenerationError, EmbeddingGenerationError,
    EmbeddingModelCoordinator, MemoryEmbeddingEngine, OnnxRuntimeInstaller, OnnxRuntimePaths,
};

/// What loading an optional model found.
pub enum ModelLoad<T> {
    Loaded(T),
    NotInstalled,
    /// Installed but not loadable now; the next use tries again.
    Unavailable,
}

impl<T> std::fmt::Debug for ModelLoad<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Loaded(_) => "Loaded",
            Self::NotInstalled => "NotInstalled",
            Self::Unavailable => "Unavailable",
        })
    }
}

/// Finds and loads the installed optional models. `InstalledModels` is the
/// production loader; tests supply their own.
#[async_trait]
pub trait ModelLoader: Send + Sync {
    /// Whether `model` is installed, without loading it.
    fn installed(&self, context: &ApiContext, model: RequiredModel) -> bool;

    /// Makes what loading needs available (ONNX Runtime), fetching it when
    /// the device has none; `false` when it cannot.
    async fn prepare(&self, context: &ApiContext) -> bool;

    fn embedding(&self, context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>>;

    fn emotion(&self, context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>>;
}

/// No optional model is ever installed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoModels;

#[async_trait]
impl ModelLoader for NoModels {
    fn installed(&self, _context: &ApiContext, _model: RequiredModel) -> bool {
        false
    }

    async fn prepare(&self, _context: &ApiContext) -> bool {
        true
    }

    fn embedding(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        ModelLoad::NotInstalled
    }

    fn emotion(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> {
        ModelLoad::NotInstalled
    }
}

/// The models installed below the app data folder, on the ONNX Runtime the
/// device has.
#[derive(Debug, Clone, Copy, Default)]
pub struct InstalledModels;

fn runtime_paths(context: &ApiContext, folder: &std::path::Path) -> OnnxRuntimePaths {
    OnnxRuntimePaths::legacy_layout(folder, context.resource_dir().map(PathBuf::from))
}

fn runtime_link(
    context: &ApiContext,
    folder: &std::path::Path,
) -> Option<lettuce_embeddings::OnnxRuntimeLink> {
    let ready =
        OnnxRuntimeInstaller::new(context.backend().database(), runtime_paths(context, folder))
            .installed()?;
    match ready.initialize() {
        Ok(_) => Some(ready.embeddings_link()),
        Err(error) => {
            tracing::warn!(%error, "ONNX Runtime could not be initialized");
            None
        }
    }
}

#[async_trait]
impl ModelLoader for InstalledModels {
    fn installed(&self, context: &ApiContext, model: RequiredModel) -> bool {
        let Some(folder) = context.app_folder() else {
            return false;
        };
        match model {
            RequiredModel::Embedding => EmbeddingModelCoordinator::new(
                &crate::embedding_models_root(folder),
                context.backend().database(),
            )
            .active()
            .ok()
            .flatten()
            .is_some(),
            RequiredModel::Emotion => lettuce_model_hub::CompanionEmotionInstallStore::open(
                crate::companion_emotion_root(folder),
            )
            .ok()
            .and_then(|store| store.installed().ok().flatten())
            .is_some(),
        }
    }

    async fn prepare(&self, context: &ApiContext) -> bool {
        let Some(folder) = context.app_folder().map(PathBuf::from) else {
            return false;
        };
        let client = match lettuce_network::ArtifactDownloadClient::new() {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(%error, "the ONNX Runtime download client could not be built");
                return false;
            }
        };
        match OnnxRuntimeInstaller::new(
            context.backend().database(),
            runtime_paths(context, &folder),
        )
        .ensure(&client, context.shutdown_token(), &|_| {})
        .await
        {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!(%error, "ONNX Runtime is unavailable");
                false
            }
        }
    }

    fn embedding(&self, context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        let Some(folder) = context.app_folder().map(PathBuf::from) else {
            return ModelLoad::NotInstalled;
        };
        let database = context.backend().database();
        let models =
            EmbeddingModelCoordinator::new(&crate::embedding_models_root(&folder), database);
        match models.active() {
            Ok(Some(_)) => {}
            Ok(None) => return ModelLoad::NotInstalled,
            Err(error) => {
                tracing::warn!(%error, "the installed embedding model could not be read");
                return ModelLoad::Unavailable;
            }
        }
        let Some(link) = runtime_link(context, &folder) else {
            return ModelLoad::Unavailable;
        };
        let dimensions = database
            .load()
            .ok()
            .and_then(|stored| stored.settings.embedding.dimensions);
        match models.load_active(&link, dimensions) {
            Ok(Some(service)) => ModelLoad::Loaded(Arc::new(service)),
            Ok(None) => ModelLoad::NotInstalled,
            Err(error) => {
                tracing::warn!(%error, "the embedding model could not be loaded");
                ModelLoad::Unavailable
            }
        }
    }

    fn emotion(&self, context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> {
        let Some(folder) = context.app_folder().map(PathBuf::from) else {
            return ModelLoad::NotInstalled;
        };
        if !self.installed(context, RequiredModel::Emotion) {
            return ModelLoad::NotInstalled;
        }
        let Some(link) = runtime_link(context, &folder) else {
            return ModelLoad::Unavailable;
        };
        match crate::try_load_companion_emotion(&crate::companion_emotion_root(&folder), &link) {
            Ok(Some(service)) => ModelLoad::Loaded(Arc::new(service)),
            Ok(None) => ModelLoad::NotInstalled,
            Err(error) => {
                tracing::warn!(%error, "the companion emotion model could not be loaded");
                ModelLoad::Unavailable
            }
        }
    }
}

enum Slot<T> {
    Unknown,
    Loaded(T),
    Absent,
}

/// The context's model slots: each model loads once, on first use, and is
/// forgotten by `ApiContext::models_changed`.
pub(crate) struct ModelSlots {
    loader: Arc<dyn ModelLoader>,
    embedding: Mutex<Slot<Arc<dyn MemoryEmbeddingEngine>>>,
    emotion: Mutex<Slot<Arc<dyn CompanionEmotionEngine>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl ModelSlots {
    pub(crate) fn new(loader: Arc<dyn ModelLoader>) -> Self {
        Self {
            loader,
            embedding: Mutex::new(Slot::Unknown),
            emotion: Mutex::new(Slot::Unknown),
        }
    }

    pub(crate) fn forget(&self) {
        *lock(&self.embedding) = Slot::Unknown;
        *lock(&self.emotion) = Slot::Unknown;
    }

    /// Whether `model` is installed, from its install record; a loaded model
    /// is installed.
    pub(crate) fn installed(&self, context: &ApiContext, model: RequiredModel) -> bool {
        self.known(model) == Some(true) || self.loader.installed(context, model)
    }

    fn known(&self, model: RequiredModel) -> Option<bool> {
        match model {
            RequiredModel::Embedding => match &*lock(&self.embedding) {
                Slot::Loaded(_) => Some(true),
                Slot::Absent => Some(false),
                Slot::Unknown => None,
            },
            RequiredModel::Emotion => match &*lock(&self.emotion) {
                Slot::Loaded(_) => Some(true),
                Slot::Absent => Some(false),
                Slot::Unknown => None,
            },
        }
    }

    fn mark_absent(&self, model: RequiredModel) {
        match model {
            RequiredModel::Embedding => *lock(&self.embedding) = Slot::Absent,
            RequiredModel::Emotion => *lock(&self.emotion) = Slot::Absent,
        }
    }

    /// Loads `model` unless it already is: `ModelRequired` when it is not
    /// installed, `ModelUnavailable` when it cannot load.
    async fn require(&self, context: &ApiContext, model: RequiredModel) -> Result<(), ApiError> {
        match self.known(model) {
            Some(true) => return Ok(()),
            Some(false) => return Err(model_error(ApiErrorCode::ModelRequired, model)),
            None => {}
        }
        let loader = Arc::clone(&self.loader);
        let installed = context
            .blocking(move |context| Ok(loader.installed(context, model)))
            .await?;
        if !installed {
            self.mark_absent(model);
            return Err(model_error(ApiErrorCode::ModelRequired, model));
        }
        if !self.loader.prepare(context).await {
            return Err(model_error(ApiErrorCode::ModelUnavailable, model));
        }
        let loaded = context
            .blocking(move |context| {
                let slots = context.models();
                Ok(match model {
                    RequiredModel::Embedding => match slots.resolve_embedding(context) {
                        ModelLoad::Loaded(_) => ModelLoad::Loaded(()),
                        ModelLoad::NotInstalled => ModelLoad::NotInstalled,
                        ModelLoad::Unavailable => ModelLoad::Unavailable,
                    },
                    RequiredModel::Emotion => match slots.resolve_emotion(context) {
                        ModelLoad::Loaded(_) => ModelLoad::Loaded(()),
                        ModelLoad::NotInstalled => ModelLoad::NotInstalled,
                        ModelLoad::Unavailable => ModelLoad::Unavailable,
                    },
                })
            })
            .await?;
        match loaded {
            ModelLoad::Loaded(()) => Ok(()),
            ModelLoad::NotInstalled => Err(model_error(ApiErrorCode::ModelRequired, model)),
            ModelLoad::Unavailable => Err(model_error(ApiErrorCode::ModelUnavailable, model)),
        }
    }

    fn resolve_embedding(&self, context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        let mut slot = lock(&self.embedding);
        match &*slot {
            Slot::Loaded(engine) => return ModelLoad::Loaded(Arc::clone(engine)),
            Slot::Absent => return ModelLoad::NotInstalled,
            Slot::Unknown => {}
        }
        let loaded = self.loader.embedding(context);
        match &loaded {
            ModelLoad::Loaded(engine) => *slot = Slot::Loaded(Arc::clone(engine)),
            ModelLoad::NotInstalled => *slot = Slot::Absent,
            ModelLoad::Unavailable => {}
        }
        loaded
    }

    fn resolve_emotion(&self, context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> {
        let mut slot = lock(&self.emotion);
        match &*slot {
            Slot::Loaded(engine) => return ModelLoad::Loaded(Arc::clone(engine)),
            Slot::Absent => return ModelLoad::NotInstalled,
            Slot::Unknown => {}
        }
        let loaded = self.loader.emotion(context);
        match &loaded {
            ModelLoad::Loaded(engine) => *slot = Slot::Loaded(Arc::clone(engine)),
            ModelLoad::NotInstalled => *slot = Slot::Absent,
            ModelLoad::Unavailable => {}
        }
        loaded
    }
}

/// The optional models a conversation needs, read from its live state
/// without loading anything: a companion chat needs the emotion and the
/// embedding model, a chat with dynamic memory the embedding model.
pub(crate) fn required_models(
    database: &lettuce_database::Database,
    settings: &lettuce_settings::GlobalSettings,
    conversation: &Conversation,
) -> Result<Vec<RequiredModel>, ApiError> {
    let companion =
        match crate::companion::companion_clock::companion_clock_context(database, conversation) {
            Ok(clock) => clock.companion,
            Err(crate::companion::companion_clock::CompanionClockError::MissingCharacter) => false,
            Err(_) => {
                return Err(api_error(
                    ApiErrorCode::Internal,
                    "the companion state could not be read",
                ));
            }
        };
    let mut needed = Vec::new();
    if companion {
        needed.push(RequiredModel::Emotion);
    }
    let dynamic = crate::companion::companion_memory_host::dynamic_memory_on(
        database,
        conversation,
        settings,
    )
    .map_err(IntoApiError::into_api_error)?;
    if companion || dynamic {
        needed.push(RequiredModel::Embedding);
    }
    Ok(needed)
}

/// The optional models installed now, from their install records; nothing
/// is loaded.
pub(crate) fn installed_models(context: &ApiContext) -> Vec<RequiredModel> {
    [RequiredModel::Embedding, RequiredModel::Emotion]
        .into_iter()
        .filter(|model| context.models().installed(context, *model))
        .collect()
}

/// The models `required` names that `installed` lacks.
pub(crate) fn missing_models(
    required: &[RequiredModel],
    installed: &[RequiredModel],
) -> Vec<RequiredModel> {
    required
        .iter()
        .copied()
        .filter(|model| !installed.contains(model))
        .collect()
}

fn global_settings(
    database: &lettuce_database::Database,
) -> Result<lettuce_settings::GlobalSettings, ApiError> {
    GlobalSettingsStore::load(database)
        .map(|stored| stored.settings)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
}

fn needed_models(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<Vec<RequiredModel>, ApiError> {
    let database = context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    required_models(database, &global_settings(database)?, &conversation)
}

/// Missing models of many conversations, reading the settings and the
/// install records once.
pub(crate) struct MissingModels {
    settings: lettuce_settings::GlobalSettings,
    installed: Option<Vec<RequiredModel>>,
}

impl MissingModels {
    pub(crate) fn new(context: &ApiContext) -> Result<Self, ApiError> {
        Ok(Self {
            settings: global_settings(context.backend().database())?,
            installed: None,
        })
    }

    pub(crate) fn of(
        &mut self,
        context: &ApiContext,
        conversation: &Conversation,
    ) -> Result<Vec<RequiredModel>, ApiError> {
        let required = required_models(context.backend().database(), &self.settings, conversation)?;
        if required.is_empty() {
            return Ok(required);
        }
        let installed = self
            .installed
            .get_or_insert_with(|| installed_models(context));
        Ok(missing_models(&required, installed))
    }
}

/// Fails with `ModelRequired` or `ModelUnavailable` unless every optional
/// model the conversation needs is installed and loads; used before any
/// turn is created, so a refused request writes nothing.
pub(crate) async fn require_conversation_models(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<(), ApiError> {
    let needed = context
        .blocking(move |context| needed_models(context, conversation_id))
        .await?;
    for model in needed {
        context.models().require(context, model).await?;
    }
    Ok(())
}

/// The embedding engine the API hands out: the loaded model, loaded at its
/// first call if it is not yet. A memory cycle fails when it is unavailable.
pub(crate) struct ApiEmbedding {
    context: ApiContext,
    resolved: OnceLock<Arc<dyn MemoryEmbeddingEngine>>,
}

impl ApiEmbedding {
    pub(crate) fn new(context: ApiContext) -> Self {
        Self {
            context,
            resolved: OnceLock::new(),
        }
    }

    fn engine(&self) -> &Arc<dyn MemoryEmbeddingEngine> {
        self.resolved.get_or_init(
            || match self.context.models().resolve_embedding(&self.context) {
                ModelLoad::Loaded(engine) => engine,
                ModelLoad::NotInstalled | ModelLoad::Unavailable => Arc::new(UnavailableEmbedding),
            },
        )
    }
}

impl MemoryEmbeddingEngine for ApiEmbedding {
    fn source_revision(&self) -> &str {
        self.engine().source_revision()
    }

    fn dimensions(&self) -> EmbeddingDimensions {
        self.engine().dimensions()
    }

    fn calibration(&self) -> SimilarityCalibration {
        self.engine().calibration()
    }

    fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingGenerationError> {
        self.engine().count_tokens(text)
    }

    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        self.engine().embed_memory(request, cancellation)
    }

    fn requires_model(&self) -> bool {
        true
    }
}

/// The emotion engine the API hands out, loaded at its first
/// classification. A companion send fails when it is unavailable.
pub(crate) struct ApiEmotion {
    context: ApiContext,
    resolved: OnceLock<Option<Arc<dyn CompanionEmotionEngine>>>,
}

impl ApiEmotion {
    pub(crate) fn new(context: ApiContext) -> Self {
        Self {
            context,
            resolved: OnceLock::new(),
        }
    }
}

impl CompanionEmotionEngine for ApiEmotion {
    fn classify_emotion(
        &self,
        text: &str,
        cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        let engine = self.resolved.get_or_init(|| {
            match self.context.models().resolve_emotion(&self.context) {
                ModelLoad::Loaded(engine) => Some(engine),
                ModelLoad::NotInstalled | ModelLoad::Unavailable => None,
            }
        });
        match engine {
            Some(engine) => engine.classify_emotion(text, cancellation),
            None => Err(CompanionEmotionGenerationError::Unavailable),
        }
    }

    fn requires_model(&self) -> bool {
        true
    }
}
