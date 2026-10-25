//! The optional embedding and emotion models. Nothing loads them at startup:
//! a chat that needs one checks it before its turn starts, which loads it
//! once (fetching ONNX Runtime first when the device has none), and the
//! operation holds one session throughout its work. Independent device
//! preferences retain idle Eidos, Thymos and Kokoro sessions between tasks;
//! memory pressure can release idle sessions without changing those choices.

pub(super) mod hugging_face;
#[cfg(test)]
mod llama_events_tests;
pub(super) mod local;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) mod local_runtime_events;
#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
mod local_runtime_review_tests;
#[cfg(test)]
mod local_tests;
pub(super) mod nanogpt;
#[cfg(test)]
mod nanogpt_tests;
pub(super) mod ollama;
pub(super) mod profiles;
#[cfg(test)]
mod profiles_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod memory_cycle_tests;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

use async_trait::async_trait;
use lettuce_companions::EmotionClassification;
use lettuce_contracts::{ApiError, ApiErrorCode, RequiredModel};
use lettuce_conversations::{Conversation, ConversationReader};
use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingRequest, EmbeddingVector, SimilarityCalibration,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_settings::{DeviceSettingsStore, GlobalSettingsStore};
use lettuce_types::ConversationId;

use super::ApiContext;
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
        if context.app_folder().is_none() {
            return false;
        }
        let Ok(roots) = context.retained_model_roots() else {
            return false;
        };
        match model {
            RequiredModel::Embedding => EmbeddingModelCoordinator::new(
                Path::new(roots.embedding.as_deref().expect("resolved root")),
                context.backend().database(),
            )
            .active()
            .ok()
            .flatten()
            .is_some(),
            RequiredModel::Emotion => lettuce_model_hub::CompanionEmotionInstallStore::open(
                Path::new(roots.thymos.as_deref().expect("resolved root")),
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
        let Ok(roots) = context.retained_model_roots() else {
            return ModelLoad::Unavailable;
        };
        let root = PathBuf::from(roots.embedding.expect("resolved root"));
        let models = EmbeddingModelCoordinator::new(&root, database);
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
        let Ok(roots) = context.retained_model_roots() else {
            return ModelLoad::Unavailable;
        };
        match crate::try_load_companion_emotion(
            Path::new(roots.thymos.as_deref().expect("resolved root")),
            &link,
        ) {
            Ok(Some(service)) => ModelLoad::Loaded(Arc::new(service)),
            Ok(None) => ModelLoad::NotInstalled,
            Err(error) => {
                tracing::warn!(%error, "the companion emotion model could not be loaded");
                ModelLoad::Unavailable
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EmbeddingCacheKey {
    version: Option<lettuce_settings::EmbeddingModelVersion>,
    max_tokens: Option<u16>,
    dimensions: Option<u16>,
    root: Option<String>,
}

pub(crate) struct ModelSlots {
    loader: Arc<dyn ModelLoader>,
    manager: Arc<crate::onnx_models::OnnxModelManager>,
    embedding: Arc<crate::onnx_models::OnnxSessionCache<EmbeddingCacheKey, dyn MemoryEmbeddingEngine>>,
    emotion: Arc<crate::onnx_models::OnnxSessionCache<Option<String>, dyn CompanionEmotionEngine>>,
}

impl ModelSlots {
    pub(crate) fn new(loader: Arc<dyn ModelLoader>, manager: Arc<crate::onnx_models::OnnxModelManager>) -> Self {
        Self {
            loader,
            embedding: crate::onnx_models::OnnxSessionCache::new(lettuce_settings::OnnxModelKind::Eidos, manager.clone()),
            emotion: crate::onnx_models::OnnxSessionCache::new(lettuce_settings::OnnxModelKind::Thymos, manager.clone()),
            manager,
        }
    }

    pub(crate) fn unload_embedding(&self) {
        self.manager.invalidate(lettuce_settings::OnnxModelKind::Eidos);
    }

    pub(crate) async fn prepare_embedding(&self, context: &ApiContext) -> Result<(), ApiError> {
        let loader = Arc::clone(&self.loader);
        if !context
            .blocking(move |context| {
                if let Some(root) = context
                    .retained_model_roots_for_guard()?
                    .and_then(|roots| roots.embedding)
                    && Path::new(&root).starts_with(self::local::models_root(context)?)
                {
                    super::jobs::local::folder_move_active(context)?;
                }
                Ok(loader.installed(context, RequiredModel::Embedding))
            })
            .await?
        {
            return Err(model_error(
                ApiErrorCode::ModelRequired,
                RequiredModel::Embedding,
            ));
        }
        if !self.loader.prepare(context).await {
            return Err(model_error(
                ApiErrorCode::ModelUnavailable,
                RequiredModel::Embedding,
            ));
        }
        Ok(())
    }

    pub(crate) fn forget(&self) {
        for kind in [lettuce_settings::OnnxModelKind::Eidos, lettuce_settings::OnnxModelKind::Thymos, lettuce_settings::OnnxModelKind::Kokoro] {
            self.manager.invalidate(kind);
        }
    }

    pub(crate) fn installed(&self, context: &ApiContext, model: RequiredModel) -> bool {
        let current = match model {
            RequiredModel::Embedding => self.embedding.has_current(),
            RequiredModel::Emotion => self.emotion.has_current(),
        };
        current || self.loader.installed(context, model)
    }

    pub(crate) fn is_loaded(&self, model: RequiredModel) -> bool {
        match model {
            RequiredModel::Embedding => self.embedding.is_loaded(),
            RequiredModel::Emotion => self.emotion.is_loaded(),
        }
    }

    /// Loads `model` unless it already is: `ModelRequired` when it is not
    /// installed, `ModelUnavailable` when it cannot load.
    pub(super) async fn require(
        &self,
        context: &ApiContext,
        model: RequiredModel,
    ) -> Result<(), ApiError> {
        let current = context
            .blocking(move |context| {
                let root =
                    context
                        .retained_model_roots_for_guard()?
                        .and_then(|roots| match model {
                            RequiredModel::Embedding => roots.embedding,
                            RequiredModel::Emotion => roots.thymos,
                        });
                if let Some(root) = root
                    && Path::new(&root).starts_with(self::local::models_root(context)?)
                {
                    super::jobs::local::folder_move_active(context)?;
                }
                let (stored, _, device, _) = context.backend().database().settings_snapshot()
                    .map_err(super::app::settings_error)?;
                Ok(match model {
                    RequiredModel::Embedding => context.models().embedding.contains(&EmbeddingCacheKey {
                        version: device.embedding.model_version,
                        max_tokens: device.embedding.max_tokens,
                        dimensions: stored.settings.embedding.dimensions,
                        root: device.retained_model_roots.embedding,
                    }),
                    RequiredModel::Emotion => context.models().emotion.contains(&device.retained_model_roots.thymos),
                })
            })
            .await?;
        if current { return Ok(()); }
        let loader = Arc::clone(&self.loader);
        let installed = context
            .blocking(move |context| Ok(loader.installed(context, model)))
            .await?;
        if !installed {
            return Err(model_error(ApiErrorCode::ModelRequired, model));
        }
        if !self.loader.prepare(context).await {
            return Err(model_error(ApiErrorCode::ModelUnavailable, model));
        }
        let loaded = context
            .blocking(move |context| {
                let _folder_access = context.local_models().folder_access();
                let root =
                    context
                        .retained_model_roots_for_guard()?
                        .and_then(|roots| match model {
                            RequiredModel::Embedding => roots.embedding,
                            RequiredModel::Emotion => roots.thymos,
                        });
                if let Some(root) = root
                    && Path::new(&root).starts_with(self::local::models_root(context)?)
                {
                    super::jobs::local::folder_move_active(context)?;
                }
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

    pub(crate) fn resolve_embedding(
        &self, context: &ApiContext,
    ) -> ModelLoad<crate::onnx_models::OnnxModelLease<dyn MemoryEmbeddingEngine>> {
        let Ok((stored, _, device, _)) = context.backend().database().settings_snapshot() else {
            return ModelLoad::Unavailable;
        };
        if self.manager.refresh_settings(|| context.backend().database().load_device_settings()).is_err() {
            return ModelLoad::Unavailable;
        }
        let key = EmbeddingCacheKey {
            version: device.embedding.model_version,
            max_tokens: device.embedding.max_tokens,
            dimensions: stored.settings.embedding.dimensions,
            root: device.retained_model_roots.embedding,
        };
        match self.embedding.acquire(key, || {
            match self.loader.embedding(context) {
                ModelLoad::Loaded(engine) => Ok(engine),
                ModelLoad::NotInstalled => Err(false),
                ModelLoad::Unavailable => Err(true),
            }
        }) {
            Ok(engine) => ModelLoad::Loaded(engine),
            Err(false) => ModelLoad::NotInstalled,
            Err(true) => ModelLoad::Unavailable,
        }
    }

    fn resolve_emotion(
        &self, context: &ApiContext,
    ) -> ModelLoad<crate::onnx_models::OnnxModelLease<dyn CompanionEmotionEngine>> {
        let Ok(device) = context.backend().database().load_device_settings() else {
            return ModelLoad::Unavailable;
        };
        if self.manager.refresh_settings(|| context.backend().database().load_device_settings()).is_err() {
            return ModelLoad::Unavailable;
        }
        match self.emotion.acquire(device.retained_model_roots.thymos, || {
            match self.loader.emotion(context) {
                ModelLoad::Loaded(engine) => Ok(engine),
                ModelLoad::NotInstalled => Err(false),
                ModelLoad::Unavailable => Err(true),
            }
        }) {
            Ok(engine) => ModelLoad::Loaded(engine),
            Err(false) => ModelLoad::NotInstalled,
            Err(true) => ModelLoad::Unavailable,
        }
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

/// The generation operations that begin a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnOperation {
    Send,
    Regenerate,
    Continue,
    Retry,
}

/// The optional models `operation` uses on `conversation`: a send classifies
/// in a companion chat and retrieves memory in a dynamic-memory chat; the
/// other operations only retrieve memory.
pub(crate) fn operation_models(
    database: &lettuce_database::Database,
    settings: &lettuce_settings::GlobalSettings,
    conversation: &Conversation,
    operation: TurnOperation,
) -> Result<Vec<RequiredModel>, ApiError> {
    if operation == TurnOperation::Send {
        return required_models(database, settings, conversation);
    }
    let dynamic = crate::companion::companion_memory_host::dynamic_memory_on(
        database,
        conversation,
        settings,
    )
    .map_err(IntoApiError::into_api_error)?;
    Ok(if dynamic {
        vec![RequiredModel::Embedding]
    } else {
        Vec::new()
    })
}

fn needed_models(
    context: &ApiContext,
    conversation_id: ConversationId,
    operation: TurnOperation,
) -> Result<Vec<RequiredModel>, ApiError> {
    let database = context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    operation_models(
        database,
        &global_settings(database)?,
        &conversation,
        operation,
    )
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
/// model `operation` uses on the conversation is installed and loads; used
/// before any turn is created, so a refused request writes nothing.
pub(crate) async fn require_conversation_models(
    context: &ApiContext,
    conversation_id: ConversationId,
    operation: TurnOperation,
) -> Result<(), ApiError> {
    let needed = context
        .blocking(move |context| needed_models(context, conversation_id, operation))
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
    metadata: OnceLock<(String, EmbeddingDimensions, SimilarityCalibration)>,
    resolved: Mutex<Option<crate::onnx_models::OnnxModelLease<dyn MemoryEmbeddingEngine>>>,
}

impl ApiEmbedding {
    pub(crate) fn new(context: ApiContext) -> Self {
        Self { context, metadata: OnceLock::new(), resolved: Mutex::new(None) }
    }

    fn engine(&self) -> Result<crate::onnx_models::OnnxModelLease<dyn MemoryEmbeddingEngine>, EmbeddingGenerationError> {
        let mut resolved = self.resolved.lock().map_err(|_| EmbeddingGenerationError::Unavailable)?;
        if let Some(engine) = resolved.as_ref() { return Ok(engine.clone()); }
        match self.context.models().resolve_embedding(&self.context) {
            ModelLoad::Loaded(engine) => {
                *resolved = Some(engine.clone());
                Ok(engine)
            }
            ModelLoad::NotInstalled | ModelLoad::Unavailable => Err(EmbeddingGenerationError::Unavailable),
        }
    }

    fn metadata(&self) -> &(String, EmbeddingDimensions, SimilarityCalibration) {
        self.metadata.get_or_init(|| match self.engine() {
            Ok(engine) => (engine.source_revision().to_owned(), engine.dimensions(), engine.calibration()),
            Err(_) => (String::new(), EmbeddingDimensions::from_preference(None), SimilarityCalibration::RawCosine),
        })
    }
}

impl MemoryEmbeddingEngine for ApiEmbedding {
    fn source_revision(&self) -> &str {
        &self.metadata().0
    }

    fn dimensions(&self) -> EmbeddingDimensions {
        self.metadata().1
    }

    fn calibration(&self) -> SimilarityCalibration {
        self.metadata().2
    }

    fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingGenerationError> {
        self.engine()?.count_tokens(text)
    }

    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        self.engine()?
            .embed_memory(request, cancellation)
    }

    fn requires_model(&self) -> bool {
        true
    }
}

/// The emotion engine the API hands out, loaded at its first
/// classification. A companion send fails when it is unavailable.
pub(crate) struct ApiEmotion {
    context: ApiContext,
    resolved: Mutex<Option<crate::onnx_models::OnnxModelLease<dyn CompanionEmotionEngine>>>,
}

impl ApiEmotion {
    pub(crate) fn new(context: ApiContext) -> Self {
        Self {
            context,
            resolved: Mutex::new(None),
        }
    }
}

impl CompanionEmotionEngine for ApiEmotion {
    fn classify_emotion(
        &self,
        text: &str,
        cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        let engine = {
            let mut resolved = self.resolved.lock().map_err(|_| CompanionEmotionGenerationError::Unavailable)?;
            if resolved.is_none() {
                match self.context.models().resolve_emotion(&self.context) {
                    ModelLoad::Loaded(engine) => *resolved = Some(engine),
                    ModelLoad::NotInstalled | ModelLoad::Unavailable => return Err(CompanionEmotionGenerationError::Unavailable),
                }
            }
            resolved.as_ref().cloned().ok_or(CompanionEmotionGenerationError::Unavailable)?
        };
        engine.classify_emotion(text, cancellation)
    }

    fn requires_model(&self) -> bool {
        true
    }
}
