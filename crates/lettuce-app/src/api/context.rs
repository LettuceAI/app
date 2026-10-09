use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use lettuce_contracts::{ApiError, ApiErrorCode, GenerationEvent};
use lettuce_conversations::InferencePort;
use lettuce_database::Database;
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
use lettuce_image_generation::ImageProviderPort;
use lettuce_jobs::{Clock, SystemClock, handle::CancellationToken};
use lettuce_media::LocalMediaBlobStore;
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_settings::SecretStore;
use lettuce_types::GenerationTurnId;

use super::error::{IntoApiError, api_error};
use super::events::{ApiEventSink, GenerationEventSink};
use super::files::FileAccess;
use super::jobs::JobHostState;
use super::local_models::LocalModelsState;
use super::models::{ApiEmbedding, ApiEmotion, InstalledModels, ModelLoader, ModelSlots};
use crate::{
    AppActiveUsageTracker, AppBackend, AppDatabaseLocation, CompanionEmotionEngine,
    EmbeddingGenerationError, MemoryEmbeddingEngine,
};

const PRIVATE_PERSISTENT_DIRECTORY: &str = "private-persistent-v2";

/// The media store the API reads assets from and writes reply images to.
pub type ApiMediaStore = LocalMediaBlobStore<Database, Database>;

/// The database files the process uses, for media collection.
#[derive(Debug)]
pub struct ApiDatabaseFiles {
    pub location: AppDatabaseLocation,
    pub active: PathBuf,
}

/// Everything an `ApiContext` is built from.
pub struct ApiContextParts {
    pub backend: Arc<AppBackend>,
    pub secret_store: Arc<dyn SecretStore>,
    pub inference: Arc<dyn InferencePort>,
    /// Generates images: the local engine or the remote provider that serves
    /// an image model's account.
    pub image_provider: Arc<dyn ImageProviderPort>,
    /// Loads the optional embedding and emotion models on first use.
    pub models: Arc<dyn ModelLoader>,
    /// The synthesis runtime and the microphone.
    pub speech: Arc<dyn super::speech::SpeechHost>,
    pub media: Option<Arc<ApiMediaStore>>,
    pub events: Arc<dyn ApiEventSink>,
    pub clock: Arc<dyn Clock>,
    pub files: Arc<dyn FileAccess>,
    /// The app data folder models, runtimes and legacy data live below.
    pub app_folder: Option<PathBuf>,
    /// The app bundle's resource folder, when the host has one.
    pub resource_dir: Option<PathBuf>,
    pub database_files: Option<ApiDatabaseFiles>,
    /// What an asset id is appended to for its `AssetRef::url`; the host
    /// picks it for its transport.
    pub asset_url_base: String,
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
    models: ModelSlots,
    streams: Mutex<HashMap<GenerationTurnId, Arc<dyn GenerationEventSink>>>,
    speakers: Mutex<HashMap<GenerationTurnId, GenerationEvent>>,
    wake: tokio::sync::Notify,
    shutdown: CancellationToken,
    jobs: JobHostState,
    memory_work: super::memory_worker::MemoryWorkState,
    conversations_changed: Arc<tokio::sync::Notify>,
    settings_changed: Arc<tokio::sync::Notify>,
    settings_sections: Arc<std::sync::Mutex<Vec<&'static str>>>,
    committed: tokio::sync::watch::Sender<u64>,
    app_usage: AppActiveUsageTracker,
    logs: Mutex<Option<super::logs::LogHost>>,
    legacy_database_detected: AtomicBool,
    local_models: LocalModelsState,
    provider_writes: tokio::sync::Mutex<()>,
    quota: super::nanogpt::QuotaState,
    quota_inference: Arc<dyn InferencePort>,
    content_filter: Arc<lettuce_inference::content_filter::ContentFilter>,
    quota_images: Arc<dyn ImageProviderPort>,
    image: super::image::ImageApiState,
    speech: super::speech::SpeechApiState,
}

impl std::fmt::Debug for ApiContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ApiContext").finish_non_exhaustive()
    }
}

impl ApiContext {
    #[must_use]
    pub fn new(parts: ApiContextParts) -> Self {
        Self::new_with_filter(
            parts,
            Arc::new(lettuce_inference::content_filter::ContentFilter::new(
                lettuce_inference::content_filter::PureModeLevel::Standard,
            )),
        )
    }

    pub(super) fn new_with_filter(
        mut parts: ApiContextParts,
        content_filter: Arc<lettuce_inference::content_filter::ContentFilter>,
    ) -> Self {
        if !parts.asset_url_base.ends_with('/') {
            parts.asset_url_base.push('/');
        }
        let now = parts.clock.now();
        let jobs = JobHostState::default();
        let changed = jobs.change_signal();
        parts
            .backend
            .database()
            .on_job_change(move || changed.notify_one());
        let conversations_changed = Arc::new(tokio::sync::Notify::new());
        let signal = Arc::clone(&conversations_changed);
        parts
            .backend
            .database()
            .on_conversation_change(move || signal.notify_one());
        let signal = Arc::clone(&conversations_changed);
        parts
            .backend
            .database()
            .on_model_change(move || signal.notify_one());
        let settings_changed = Arc::new(tokio::sync::Notify::new());
        let signal = settings_changed.clone();
        let settings_sections = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sections = settings_sections.clone();
        parts
            .backend
            .database()
            .on_settings_section_change(move |section| {
                sections.lock().expect("settings sections").push(section);
                signal.notify_one();
            });
        let (committed, _) = tokio::sync::watch::channel(0_u64);
        for listen in [
            lettuce_database::Database::on_job_change,
            lettuce_database::Database::on_conversation_change,
        ] {
            let committed = committed.clone();
            listen(parts.backend.database(), move || {
                committed.send_modify(|count| *count = count.wrapping_add(1));
            });
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            parts.inference = Arc::new(super::local_runtime_events::RoutedInference {
                inference: parts.inference,
                router: parts.backend.local_runtime_events().clone(),
            });
        }
        let quota_signal = Arc::new(std::sync::OnceLock::new());
        let quota_inference = Arc::new(super::nanogpt::QuotaInference {
            inference: parts.inference.clone(),
            signal: quota_signal.clone(),
        });
        let quota_images = Arc::new(super::nanogpt::QuotaImages {
            provider: parts.image_provider.clone(),
            signal: quota_signal.clone(),
        });
        let context = Self {
            inner: Arc::new(ApiContextInner {
                models: ModelSlots::new(Arc::clone(&parts.models)),
                parts,
                streams: Mutex::new(HashMap::new()),
                speakers: Mutex::new(HashMap::new()),
                wake: tokio::sync::Notify::new(),
                shutdown: CancellationToken::new(),
                jobs,
                memory_work: super::memory_worker::MemoryWorkState::default(),
                conversations_changed,
                settings_changed,
                settings_sections,
                committed,
                app_usage: AppActiveUsageTracker::new(now),
                logs: Mutex::new(None),
                legacy_database_detected: AtomicBool::new(false),
                local_models: LocalModelsState::default(),
                provider_writes: tokio::sync::Mutex::new(()),
                quota: super::nanogpt::QuotaState::default(),
                quota_inference,
                content_filter,
                quota_images,
                image: super::image::ImageApiState::default(),
                speech: super::speech::SpeechApiState::default(),
            }),
        };
        let _ = quota_signal.set(context.downgrade());
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let weak = context.downgrade();
            context
                .backend()
                .local_runtime_events()
                .set_global(move |event| {
                    if let Some(context) = weak.upgrade() {
                        context.runtime_report_changed(event);
                    }
                });
        }
        context
    }

    /// Opens the production backend under the app data directory: the active
    /// database, the media store and the remote provider runtime over the
    /// host's native secret store. The optional embedding and emotion models
    /// load when a chat first needs them.
    pub fn open_desktop(
        app_data_dir: &Path,
        resource_dir: Option<PathBuf>,
        secret_store: Arc<dyn SecretStore>,
        events: Arc<dyn ApiEventSink>,
        files: Arc<dyn FileAccess>,
        asset_url_base: String,
        microphone: Option<Arc<dyn crate::MicrophoneCapture>>,
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
        let file_lifecycle = location
            .acquire_file_lifecycle(true)
            .map_err(|error| super::storage::file_error(error, None))?;
        let path = file_lifecycle
            .prepare_open(clock.now())
            .map_err(|error| super::storage::file_error(error, None))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| storage_error("database directory", error))?;
        }
        let backend = AppBackend::open(&path, clock.now())
            .map_err(|error| storage_error("application database", error))?;
        file_lifecycle
            .complete_open()
            .map_err(|error| super::storage::file_error(error, None))?;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let backend = {
            use lettuce_settings::DeviceSettingsStore;
            let device = backend
                .database()
                .load_device_settings()
                .map_err(|error| storage_error("device settings", error))?;
            backend
                .with_local_diffusion(crate::image::local_diffusion::diffusion_paths(
                    &device,
                    app_data_dir,
                ))
                .map_err(|error| {
                    api_error(
                        ApiErrorCode::Unavailable,
                        format!("the image engine could not start: {error}"),
                    )
                })?
        };
        let backend = Arc::new(backend);
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
        let provider = backend
            .provider_runtime(Arc::clone(&secret_store), &tls)
            .map_err(|error| {
                api_error(
                    ApiErrorCode::Unavailable,
                    format!("provider runtime could not start: {error}"),
                )
            })?;
        let content_filter = provider.content_filter();
        let inference: Arc<dyn InferencePort> = Arc::new(provider);
        let image_provider: Arc<dyn ImageProviderPort> = Arc::new(
            backend
                .image_providers(Arc::clone(&secret_store), &tls)
                .map_err(|error| {
                    api_error(
                        ApiErrorCode::Unavailable,
                        format!("image provider runtime could not start: {error}"),
                    )
                })?,
        );
        Ok(Self::new_with_filter(
            ApiContextParts {
                backend,
                secret_store,
                inference,
                image_provider,
                models: Arc::new(InstalledModels),
                speech: Arc::new(super::speech::InstalledSpeech::new(microphone)),
                media: Some(Arc::new(media)),
                events,
                clock,
                files,
                app_folder: Some(app_data_dir.to_path_buf()),
                resource_dir,
                database_files: Some(ApiDatabaseFiles {
                    location,
                    active: path,
                }),
                asset_url_base,
            },
            content_filter,
        ))
    }

    /// A new context over the same backend and host services, as a
    /// restarted process would open them: no job, watch or install work of
    /// this one carries over.
    #[cfg(test)]
    pub(crate) fn restarted(&self) -> Self {
        let parts = &self.inner.parts;
        Self::new(ApiContextParts {
            backend: Arc::clone(&parts.backend),
            secret_store: Arc::clone(&parts.secret_store),
            inference: Arc::clone(&parts.inference),
            image_provider: Arc::clone(&parts.image_provider),
            models: Arc::clone(&parts.models),
            speech: Arc::clone(&parts.speech),
            media: parts.media.clone(),
            events: Arc::clone(&parts.events),
            clock: Arc::clone(&parts.clock),
            files: Arc::clone(&parts.files),
            app_folder: parts.app_folder.clone(),
            resource_dir: parts.resource_dir.clone(),
            database_files: None,
            asset_url_base: parts.asset_url_base.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_secret_store(&self, store: Arc<dyn SecretStore>) -> Self {
        let parts = &self.inner.parts;
        Self::new(ApiContextParts {
            backend: Arc::clone(&parts.backend),
            secret_store: store,
            inference: Arc::clone(&parts.inference),
            image_provider: Arc::clone(&parts.image_provider),
            models: Arc::clone(&parts.models),
            speech: Arc::clone(&parts.speech),
            media: parts.media.clone(),
            events: Arc::clone(&parts.events),
            clock: Arc::clone(&parts.clock),
            files: Arc::clone(&parts.files),
            app_folder: parts.app_folder.clone(),
            resource_dir: parts.resource_dir.clone(),
            database_files: None,
            asset_url_base: parts.asset_url_base.clone(),
        })
    }

    /// A new context over the same backend and host services with another
    /// speech host.
    #[cfg(test)]
    pub(crate) fn with_speech(&self, speech: Arc<dyn super::speech::SpeechHost>) -> Self {
        let parts = &self.inner.parts;
        Self::new(ApiContextParts {
            backend: Arc::clone(&parts.backend),
            secret_store: Arc::clone(&parts.secret_store),
            inference: Arc::clone(&parts.inference),
            image_provider: Arc::clone(&parts.image_provider),
            models: Arc::clone(&parts.models),
            speech,
            media: parts.media.clone(),
            events: Arc::clone(&parts.events),
            clock: Arc::clone(&parts.clock),
            files: Arc::clone(&parts.files),
            app_folder: parts.app_folder.clone(),
            resource_dir: parts.resource_dir.clone(),
            database_files: None,
            asset_url_base: parts.asset_url_base.clone(),
        })
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn shared_backend(&self) -> Arc<AppBackend> {
        Arc::clone(&self.inner.parts.backend)
    }

    pub fn backend(&self) -> &AppBackend {
        &self.inner.parts.backend
    }

    /// No app folder means there can be no models-folder move to guard.
    /// Device-settings failures remain errors, including on a headless host.
    pub(crate) fn retained_model_roots_for_guard(
        &self,
    ) -> Result<Option<lettuce_settings::RetainedModelRoots>, lettuce_contracts::ApiError> {
        use lettuce_settings::DeviceSettingsStore;
        let device = self
            .backend()
            .database()
            .load_device_settings()
            .map_err(|error| {
                super::error::api_error(
                    lettuce_contracts::ApiErrorCode::Internal,
                    error.to_string(),
                )
            })?;
        Ok(self
            .app_folder()
            .map(|folder| crate::speech::speech_roots::retained_model_roots(&device, folder)))
    }

    pub(crate) fn retained_model_roots(
        &self,
    ) -> Result<lettuce_settings::RetainedModelRoots, lettuce_contracts::ApiError> {
        self.retained_model_roots_for_guard()?.ok_or_else(|| {
            super::error::api_error(
                lettuce_contracts::ApiErrorCode::Unavailable,
                "the app folder is unavailable",
            )
        })
    }

    #[must_use]
    pub fn secret_store(&self) -> &Arc<dyn SecretStore> {
        &self.inner.parts.secret_store
    }

    /// Settles what the previous process left running; call once before any
    /// worker starts.
    pub(super) async fn provider_write_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.inner.provider_writes.lock().await
    }

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
        for (turn_id, _) in &report.turns {
            self.forget_stream(*turn_id);
        }
        super::jobs::voice_creation::recover_queued(self)?;
        super::scenes::recover(self)?;
        super::speech::sweep_scratch(self);
        super::speech::collect_recordings(self)?;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Some(engine) = self.backend().local_diffusion() {
            engine.clear_upscale_scratch();
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn with_test_secrets(&self, secret_store: Arc<dyn SecretStore>) -> Self {
        let parts = &self.inner.parts;
        Self::new(ApiContextParts {
            backend: parts.backend.clone(),
            secret_store,
            inference: parts.inference.clone(),
            image_provider: parts.image_provider.clone(),
            models: parts.models.clone(),
            speech: parts.speech.clone(),
            media: parts.media.clone(),
            events: parts.events.clone(),
            clock: parts.clock.clone(),
            files: parts.files.clone(),
            app_folder: parts.app_folder.clone(),
            resource_dir: parts.resource_dir.clone(),
            database_files: None,
            asset_url_base: parts.asset_url_base.clone(),
        })
    }

    /// Cancels every running generation and any job a worker starts from
    /// now on; workers stop through their own shutdown signal.
    pub fn begin_shutdown(&self) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        self.backend().local_runtime_events().shutdown();
        self.inner.shutdown.cancel();
        self.backend().begin_shutdown();
        self.inner.jobs.wake();
    }

    pub(crate) fn shutdown_token(&self) -> &CancellationToken {
        &self.inner.shutdown
    }

    pub(crate) fn asset_id_from_url(
        &self,
        uri: &str,
    ) -> Result<Option<lettuce_types::AssetId>, ApiError> {
        uri.strip_prefix(&self.inner.parts.asset_url_base)
            .map(|id| super::error::parse_id(id, "source"))
            .transpose()
    }

    pub(crate) fn asset_ref(
        &self,
        asset_id: lettuce_types::AssetId,
    ) -> lettuce_contracts::AssetRef {
        let asset_id = asset_id.to_string();
        lettuce_contracts::AssetRef {
            url: format!("{}{asset_id}", self.inner.parts.asset_url_base),
            asset_id,
        }
    }

    pub(crate) fn downgrade(&self) -> WeakApiContext {
        WeakApiContext(Arc::downgrade(&self.inner))
    }

    pub(crate) fn inference(&self) -> &dyn InferencePort {
        self.inner.quota_inference.as_ref()
    }

    pub(crate) fn image_provider(&self) -> &dyn ImageProviderPort {
        self.inner.quota_images.as_ref()
    }

    /// The embedding engine for one use; the installed model loads at its
    /// first call.
    pub(crate) fn embedding(&self) -> Arc<dyn MemoryEmbeddingEngine> {
        Arc::new(ApiEmbedding::new(self.clone()))
    }

    /// The emotion engine for one use; the installed model loads at its
    /// first classification.
    pub(crate) fn emotion(&self) -> Arc<dyn CompanionEmotionEngine> {
        Arc::new(ApiEmotion::new(self.clone()))
    }

    pub(crate) fn models(&self) -> &ModelSlots {
        &self.inner.models
    }

    /// Forgets the loaded optional models after an install, switch or
    /// removal, so the next use loads what is installed then, and tells
    /// every window to re-read the models its chats miss.
    pub fn models_changed(&self) {
        self.inner.models.forget();
        self.emit(lettuce_contracts::ApiEvent::RequiredModelsChanged);
    }

    pub fn attach_logs(&self, directory: PathBuf, sink: lettuce_observability::LogSink) {
        let weak = self.downgrade();
        sink.set_observer(move |line| {
            if let Some(context) = weak.upgrade()
                && context.content_filter().logging_enabled() == Ok(true)
            {
                context.emit(lettuce_contracts::ApiEvent::DeveloperLogLine {
                    line: line.to_owned(),
                });
            }
        });
        *self.inner.logs.lock().expect("log host") = Some(super::logs::LogHost {
            directory: lettuce_observability::LogDirectory::new(directory),
            sink,
        });
    }

    pub(super) fn logs(&self) -> Result<super::logs::LogHost, ApiError> {
        self.inner
            .logs
            .lock()
            .map_err(|_| {
                super::logs::unavailable(
                    lettuce_contracts::LogFailureReason::HostUnavailable,
                    "log host unavailable",
                )
            })?
            .clone()
            .ok_or_else(|| {
                super::logs::unavailable(
                    lettuce_contracts::LogFailureReason::HostUnavailable,
                    "log host unavailable",
                )
            })
    }

    pub(crate) fn files(&self) -> &dyn FileAccess {
        self.inner.parts.files.as_ref()
    }

    pub(crate) fn app_folder(&self) -> Option<&Path> {
        self.inner.parts.app_folder.as_deref()
    }

    pub(crate) fn resource_dir(&self) -> Option<&Path> {
        self.inner.parts.resource_dir.as_deref()
    }

    pub(crate) fn database_files(&self) -> Option<&ApiDatabaseFiles> {
        self.inner.parts.database_files.as_ref()
    }

    pub(super) fn memory_work(&self) -> &super::memory_worker::MemoryWorkState {
        &self.inner.memory_work
    }

    pub(crate) fn jobs(&self) -> &JobHostState {
        &self.inner.jobs
    }

    pub(crate) fn local_models(&self) -> &LocalModelsState {
        &self.inner.local_models
    }

    pub(crate) fn image_state(&self) -> &super::image::ImageApiState {
        &self.inner.image
    }

    pub(crate) fn speech(&self) -> &dyn super::speech::SpeechHost {
        self.inner.parts.speech.as_ref()
    }

    pub(crate) fn speech_state(&self) -> &super::speech::SpeechApiState {
        &self.inner.speech
    }

    /// Resolves after a committed conversation change, including one made
    /// before the call that no caller has waited for yet.
    pub(crate) async fn conversations_changed(&self) {
        self.inner.conversations_changed.notified().await;
    }

    /// Changes after each committed transaction that changed a job or a
    /// conversation; a caller waits on it for work it cancelled to settle.
    pub(crate) fn committed_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.inner.committed.subscribe()
    }

    pub(crate) fn legacy_database_detected(&self) -> bool {
        self.inner.legacy_database_detected.load(Ordering::Acquire)
    }

    pub(crate) fn set_legacy_database_detected(&self, detected: bool) {
        self.inner
            .legacy_database_detected
            .store(detected, Ordering::Release);
    }

    /// Records that the window gained or lost focus (or, on mobile, that the
    /// app resumed or went to the background) for the active-time counter;
    /// losing focus writes the counted time.
    pub fn app_focus_changed(&self, focused: bool) {
        self.inner.app_usage.on_focus_changed(focused, self.now());
        if !focused {
            self.flush_app_usage();
        }
    }

    /// Adds the counted active time to each day's usage; the host calls it on
    /// exit.
    pub fn flush_app_usage(&self) {
        match self
            .inner
            .app_usage
            .flush(self.backend().database(), self.now())
        {
            Ok(0) => {}
            Ok(_) => self.emit(lettuce_contracts::ApiEvent::AppUsageChanged),
            Err(error) => {
                self.emit(lettuce_contracts::ApiEvent::AppUsageChanged);
                self.emit(lettuce_contracts::ApiEvent::AppUsageWriteFailed {
                    error: super::app::app_usage_error(error),
                });
            }
        }
    }

    pub(super) fn app_usage_days(&self) -> Result<Vec<lettuce_usage::AppUsageDay>, ApiError> {
        self.inner
            .app_usage
            .days(self.backend().database(), self.now())
            .map_err(super::app::app_usage_error)
    }

    pub(crate) fn media(&self) -> Option<&ApiMediaStore> {
        self.inner.parts.media.as_deref()
    }

    pub(crate) fn take_settings_sections(&self) -> Vec<&'static str> {
        std::mem::take(
            &mut *self
                .inner
                .settings_sections
                .lock()
                .expect("settings sections"),
        )
    }

    pub(crate) async fn settings_changed(&self) {
        self.inner.settings_changed.notified().await;
    }

    pub(crate) fn content_filter(&self) -> &Arc<lettuce_inference::content_filter::ContentFilter> {
        &self.inner.content_filter
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
        let delivery = Arc::new(super::serial_events::SerialEvents::new(move |event| {
            sink.emit(event);
            true
        }));
        struct Stream(Arc<super::serial_events::SerialEvents<GenerationEvent>>);
        impl GenerationEventSink for Stream {
            fn emit_if(
                &self,
                event: GenerationEvent,
                valid: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
            ) {
                self.0.send_if(event, valid);
            }
            fn emit(&self, event: GenerationEvent) {
                if matches!(
                    event,
                    GenerationEvent::Completed { .. }
                        | GenerationEvent::Failed { .. }
                        | GenerationEvent::Cancelled { .. }
                ) {
                    self.0.finish(event);
                } else {
                    self.0.send(event);
                }
            }
        }
        let replay = if let Ok(mut streams) = self.inner.streams.lock() {
            let mut replay = Vec::new();
            if let Ok(speakers) = self.inner.speakers.lock()
                && let Some(event) = speakers.get(&turn_id)
            {
                replay.push(event.clone());
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if let Some((load, valid)) = self
                .backend()
                .local_runtime_events()
                .generation_load(turn_id)
            {
                delivery.send_if(load, valid);
            }
            streams.insert(turn_id, Arc::new(Stream(delivery.clone())));
            replay
        } else {
            Vec::new()
        };
        delivery.initialize(replay);
    }

    pub(crate) fn live_generation_event(&self, turn_id: GenerationTurnId, event: GenerationEvent) {
        let sink = self.inner.streams.lock().ok().and_then(|streams| {
            if matches!(event, GenerationEvent::SpeakerSelected { .. })
                && let Ok(mut speakers) = self.inner.speakers.lock()
            {
                speakers.insert(turn_id, event.clone());
            }
            streams.get(&turn_id).cloned()
        });
        if let Some(sink) = sink {
            sink.emit(event);
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(crate) fn live_runtime_generation_event(
        &self,
        turn_id: GenerationTurnId,
        event: GenerationEvent,
        valid: super::serial_events::Validity,
    ) {
        let sink = self.inner.streams.lock().ok().and_then(|streams| {
            if let GenerationEvent::ModelLoading { .. } = &event {
                let current = self
                    .backend()
                    .local_runtime_events()
                    .generation_load(turn_id);
                if current
                    .as_ref()
                    .is_none_or(|(current, _)| current != &event)
                {
                    return None;
                }
            }
            streams.get(&turn_id).cloned()
        });
        if let Some(sink) = sink {
            sink.emit_if(event, valid);
        }
    }

    pub(crate) fn stream(&self, turn_id: GenerationTurnId) -> Option<Arc<dyn GenerationEventSink>> {
        self.inner
            .streams
            .lock()
            .ok()
            .and_then(|streams| streams.get(&turn_id).cloned())
    }

    /// Sends a turn's last event and forgets its stream; returns whether the
    /// turn still had one.
    pub(crate) fn finish_stream(&self, turn_id: GenerationTurnId, event: GenerationEvent) -> bool {
        if let Ok(mut speakers) = self.inner.speakers.lock() {
            speakers.remove(&turn_id);
        }
        match self.forget_stream(turn_id) {
            Some(sink) => {
                sink.emit(event);
                true
            }
            None => false,
        }
    }

    fn forget_stream(&self, turn_id: GenerationTurnId) -> Option<Arc<dyn GenerationEventSink>> {
        self.inner
            .streams
            .lock()
            .ok()
            .and_then(|mut streams| streams.remove(&turn_id))
    }

    /// Ends the stream of a turn that reached a terminal state and tells
    /// every window.
    pub(crate) fn settle_turn(
        &self,
        conversation_id: lettuce_types::ConversationId,
        turn_id: GenerationTurnId,
        event: GenerationEvent,
    ) {
        self.finish_stream(turn_id, event);
        self.emit(lettuce_contracts::ApiEvent::GenerationSettled {
            conversation_id: conversation_id.to_string(),
            turn_id: turn_id.to_string(),
        });
    }

    /// Ends the streams of turns settled outside a worker run, such as a
    /// job failed because its work could not be resolved.
    pub(crate) fn finish_settled_streams(&self) -> Result<(), ApiError> {
        let turns = self
            .inner
            .streams
            .lock()
            .map(|streams| streams.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for turn_id in turns {
            let turn = match lettuce_conversations::ConversationReader::get_turn(
                self.backend().database(),
                turn_id,
            ) {
                Ok(turn) => turn,
                Err(lettuce_conversations::ConversationRepositoryError::NotFound) => {
                    self.forget_stream(turn_id);
                    continue;
                }
                Err(error) => return Err(error.into_api_error()),
            };
            if let Some(event) = super::worker::settled_event(self.backend().database(), turn_id)?
                && self.finish_stream(turn_id, event)
            {
                self.emit(lettuce_contracts::ApiEvent::GenerationSettled {
                    conversation_id: turn.conversation_id.to_string(),
                    turn_id: turn_id.to_string(),
                });
            }
        }
        Ok(())
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

pub(crate) struct WeakApiContext(std::sync::Weak<ApiContextInner>);

impl WeakApiContext {
    pub(crate) fn upgrade(&self) -> Option<ApiContext> {
        self.0.upgrade().map(|inner| ApiContext { inner })
    }
}

impl ApiContext {
    pub(crate) fn quota(&self) -> &super::nanogpt::QuotaState {
        &self.inner.quota
    }
}
