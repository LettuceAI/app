use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use lettuce_contracts::{ApiError, ApiErrorCode, GenerationEvent};
use lettuce_conversations::InferencePort;
use lettuce_database::Database;
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::{Clock, SystemClock, handle::CancellationToken};
use lettuce_media::LocalMediaBlobStore;
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_settings::SecretStore;
use lettuce_types::GenerationTurnId;

use super::error::{IntoApiError, api_error};
use super::events::{ApiEventSink, GenerationEventSink};
use crate::{
    AppBackend, AppDatabaseLocation, CompanionEmotionEngine, EmbeddingGenerationError,
    MemoryEmbeddingEngine,
};

const PRIVATE_PERSISTENT_DIRECTORY: &str = "private-persistent-v2";

/// The media store the API reads assets from and writes reply images to.
pub type ApiMediaStore = LocalMediaBlobStore<Database, Database>;

/// Everything an `ApiContext` is built from.
pub struct ApiContextParts {
    pub backend: Arc<AppBackend>,
    pub secret_store: Arc<dyn SecretStore>,
    pub inference: Arc<dyn InferencePort>,
    pub embedding: Arc<dyn MemoryEmbeddingEngine>,
    pub emotion: Option<Arc<dyn CompanionEmotionEngine>>,
    pub media: Option<Arc<ApiMediaStore>>,
    pub events: Arc<dyn ApiEventSink>,
    pub clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for ApiContextParts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApiContextParts")
            .finish_non_exhaustive()
    }
}

/// The application API's state: the backend, the host services it runs
/// with, and the per-turn stream sinks. Cloning shares the same state.
#[derive(Clone)]
pub struct ApiContext {
    inner: Arc<ApiContextInner>,
}

struct ApiContextInner {
    parts: ApiContextParts,
    streams: Mutex<HashMap<GenerationTurnId, Arc<dyn GenerationEventSink>>>,
    wake: tokio::sync::Notify,
}

impl std::fmt::Debug for ApiContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ApiContext").finish_non_exhaustive()
    }
}

impl ApiContext {
    #[must_use]
    pub fn new(parts: ApiContextParts) -> Self {
        Self {
            inner: Arc::new(ApiContextInner {
                parts,
                streams: Mutex::new(HashMap::new()),
                wake: tokio::sync::Notify::new(),
            }),
        }
    }

    /// Opens the production backend under the app data directory: the active
    /// database, the media store and the remote provider runtime over the
    /// host's native secret store. Memory embedding and emotion models are
    /// not loaded, so dynamic memory retrieval runs without vectors and
    /// companion sends use the neutral update.
    pub fn open_desktop(
        app_data_dir: &Path,
        secret_store: Arc<dyn SecretStore>,
        events: Arc<dyn ApiEventSink>,
    ) -> Result<Self, ApiError> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let snapshot = DirectorySnapshot::with_private_persistent(
            app_data_dir,
            app_data_dir.join(PRIVATE_PERSISTENT_DIRECTORY),
        )
        .map_err(|error| storage_error("app data directory", error))?;
        let authority = FilesystemAuthority::new(&snapshot)
            .map_err(|error| storage_error("app data directory", error))?;
        let location =
            AppDatabaseLocation::new(app_data_dir.join(PRIVATE_PERSISTENT_DIRECTORY), &authority)
                .map_err(|error| storage_error("database location", error))?;
        let path = location
            .active_path()
            .map_err(|error| storage_error("database location", error))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| storage_error("database directory", error))?;
        }
        let backend = Arc::new(
            AppBackend::open(&path, clock.now())
                .map_err(|error| storage_error("application database", error))?,
        );
        let media = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .map_err(|error| storage_error("media store", error))?,
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .map_err(|error| storage_error("media store", error))?,
            Database::open(&path).map_err(|error| storage_error("media catalog", error))?,
            Database::open(&path).map_err(|error| storage_error("media catalog", error))?,
        );
        let tls = backend
            .tls_policy()
            .map_err(|error| storage_error("device settings", error))?;
        let inference: Arc<dyn InferencePort> = Arc::new(
            backend
                .provider_runtime(Arc::clone(&secret_store), &tls)
                .map_err(|error| {
                    api_error(
                        ApiErrorCode::Unavailable,
                        format!("provider runtime could not start: {error}"),
                    )
                })?,
        );
        Ok(Self::new(ApiContextParts {
            backend,
            secret_store,
            inference,
            embedding: Arc::new(UnavailableEmbedding),
            emotion: None,
            media: Some(Arc::new(media)),
            events,
            clock,
        }))
    }

    #[must_use]
    pub fn backend(&self) -> &AppBackend {
        &self.inner.parts.backend
    }

    #[must_use]
    pub fn secret_store(&self) -> &Arc<dyn SecretStore> {
        &self.inner.parts.secret_store
    }

    /// Settles what the previous process left running; call once before any
    /// worker starts.
    pub fn recover_after_restart(&self) -> Result<(), ApiError> {
        let report = self
            .backend()
            .recover_after_restart(self.now())
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        tracing::info!(
            jobs = report.jobs.len(),
            cancelled_generation_jobs = report.cancelled_generation_jobs.len(),
            turns = report.turns.len(),
            "recovered work left by the previous process"
        );
        Ok(())
    }

    /// Cancels every running generation; workers stop through their own
    /// shutdown token.
    pub fn begin_shutdown(&self) {
        self.backend().begin_shutdown();
    }

    pub(crate) fn inference(&self) -> &dyn InferencePort {
        self.inner.parts.inference.as_ref()
    }

    pub(crate) fn embedding(&self) -> &dyn MemoryEmbeddingEngine {
        self.inner.parts.embedding.as_ref()
    }

    pub(crate) fn emotion(&self) -> Option<&dyn CompanionEmotionEngine> {
        self.inner.parts.emotion.as_deref()
    }

    pub(crate) fn media(&self) -> Option<&ApiMediaStore> {
        self.inner.parts.media.as_deref()
    }

    pub(crate) fn clock(&self) -> &dyn Clock {
        self.inner.parts.clock.as_ref()
    }

    pub(crate) fn now(&self) -> lettuce_types::TimestampMillis {
        self.inner.parts.clock.now()
    }

    pub(crate) fn emit(&self, event: lettuce_contracts::ApiEvent) {
        self.inner.parts.events.emit(event);
    }

    pub(crate) fn attach_stream(
        &self,
        turn_id: GenerationTurnId,
        sink: Arc<dyn GenerationEventSink>,
    ) {
        if let Ok(mut streams) = self.inner.streams.lock() {
            streams.insert(turn_id, sink);
        }
    }

    pub(crate) fn stream(&self, turn_id: GenerationTurnId) -> Option<Arc<dyn GenerationEventSink>> {
        self.inner
            .streams
            .lock()
            .ok()
            .and_then(|streams| streams.get(&turn_id).cloned())
    }

    /// Sends a turn's last event and forgets its stream.
    pub(crate) fn finish_stream(&self, turn_id: GenerationTurnId, event: GenerationEvent) {
        let sink = self
            .inner
            .streams
            .lock()
            .ok()
            .and_then(|mut streams| streams.remove(&turn_id));
        if let Some(sink) = sink {
            sink.emit(event);
        }
    }

    pub(crate) fn wake_workers(&self) {
        self.inner.wake.notify_one();
    }

    pub(crate) async fn woken(&self) {
        self.inner.wake.notified().await;
    }

    /// Runs synchronous repository work on the blocking pool.
    pub(crate) async fn blocking<T, F>(&self, work: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&Self) -> Result<T, ApiError> + Send + 'static,
    {
        let context = self.clone();
        tokio::task::spawn_blocking(move || work(&context))
            .await
            .map_err(IntoApiError::into_api_error)?
    }
}

fn storage_error(what: &str, error: impl std::fmt::Display) -> ApiError {
    api_error(
        ApiErrorCode::Unavailable,
        format!("{what} is unavailable: {error}"),
    )
}

/// Stands in until an embedding model is loaded; every embedding is
/// unavailable, which retrieval treats as having no vectors.
#[derive(Debug)]
pub(super) struct UnavailableEmbedding;

impl MemoryEmbeddingEngine for UnavailableEmbedding {
    fn source_revision(&self) -> &str {
        ""
    }

    fn dimensions(&self) -> EmbeddingDimensions {
        EmbeddingDimensions::from_preference(None)
    }

    fn count_tokens(&self, _text: &str) -> Result<u32, EmbeddingGenerationError> {
        Err(EmbeddingGenerationError::Unavailable)
    }

    fn embed_memory(
        &self,
        _request: &EmbeddingRequest,
        _cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        Err(EmbeddingGenerationError::Unavailable)
    }
}
