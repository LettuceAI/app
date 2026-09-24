//! Makes ONNX Runtime available before embeddings, the companion emotion
//! classifier or Kokoro load: the `ORT_DYLIB_PATH` override, then a bundled
//! library, then an earlier download, then a fresh download of the official
//! release archive as a cancellable `ArtifactInstall` job whose byte progress
//! is recorded in the job store. Android and iOS ship the runtime inside the
//! app and never download it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use lettuce_embeddings::{
    CommittedOnnxRuntime, ONNX_RUNTIME_VERSION, OnnxRuntimeArchive, OnnxRuntimeBinding,
    OnnxRuntimeHostOs, OnnxRuntimeInitError, OnnxRuntimeLink, OnnxRuntimeLocations,
    OnnxRuntimeProvisionError, OnnxRuntimeSource, ResolvedOnnxRuntime, committed_onnx_runtime,
    initialize_process_onnx_runtime, install_onnx_runtime_archive, onnx_runtime_archives,
    resolve_installed_onnx_runtime,
};
use lettuce_jobs::handle::{CancellationToken, JobHandle};
use lettuce_jobs::{CancellationReason, JobStore, ResourceAvailability, WorkerId};
use lettuce_model_hub::PinnedArtifact;
use lettuce_network::{ArtifactDownloadClient, ArtifactProbeError};
use lettuce_speech::OnnxRuntimeCommitted;
use lettuce_types::{JobId, TimestampMillis};

use crate::{
    ArtifactInstallCoordinator, ArtifactInstallError, ArtifactInstallPlan,
    ArtifactInstallRunResult, ArtifactSource, ArtifactSourceClient, ArtifactSourceError,
    PlannedArtifact,
};

const ORT_DYLIB_PATH: &str = "ORT_DYLIB_PATH";
const INSTALL_LEASE: Duration = Duration::from_secs(60);
const ENVIRONMENT_NAME: &str = "lettuce";

static ENSURE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Where a desktop runtime is unpacked, downloaded and bundled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxRuntimePaths {
    /// `<app>/onnxruntime`
    pub install_dir: PathBuf,
    /// `<app>/downloads/onnxruntime`
    pub downloads: PathBuf,
    /// The app bundle's resource folder, when the host has one.
    pub resource_dir: Option<PathBuf>,
}

impl OnnxRuntimePaths {
    /// Runtimes in `<app_dir>/onnxruntime`, archives in
    /// `<app_dir>/downloads/onnxruntime`.
    #[must_use]
    pub fn legacy_layout(app_dir: &Path, resource_dir: Option<PathBuf>) -> Self {
        Self {
            install_dir: app_dir.join("onnxruntime"),
            downloads: app_dir.join("downloads").join("onnxruntime"),
            resource_dir,
        }
    }

    fn locations(&self) -> OnnxRuntimeLocations {
        OnnxRuntimeLocations {
            install_dir: self.install_dir.clone(),
            resource_dir: self.resource_dir.clone(),
        }
    }
}

/// A runtime the ONNX consumers can load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxRuntimeReady {
    Library(ResolvedOnnxRuntime),
    Linked,
}

impl From<CommittedOnnxRuntime> for OnnxRuntimeReady {
    fn from(committed: CommittedOnnxRuntime) -> Self {
        match committed {
            CommittedOnnxRuntime::Library(library) => Self::Library(ResolvedOnnxRuntime {
                library,
                source: OnnxRuntimeSource::Loaded,
            }),
            CommittedOnnxRuntime::Linked => Self::Linked,
        }
    }
}

impl OnnxRuntimeReady {
    #[must_use]
    pub fn embeddings_link(&self) -> OnnxRuntimeLink {
        match self {
            Self::Library(resolved) => OnnxRuntimeLink::Dynamic(resolved.library.clone()),
            Self::Linked => OnnxRuntimeLink::Linked,
        }
    }

    /// Commits the process's ONNX Runtime environment from this runtime,
    /// before embeddings, the emotion classifier or Kokoro load. It returns
    /// the evidence Kokoro sessions require; embeddings and the emotion
    /// classifier find the committed environment on their own.
    pub fn initialize(&self) -> Result<OnnxRuntimeCommitted, OnnxRuntimeInitError> {
        let binding = match self {
            Self::Library(resolved) => OnnxRuntimeBinding::Library(&resolved.library),
            Self::Linked => OnnxRuntimeBinding::Linked,
        };
        initialize_process_onnx_runtime(binding, ENVIRONMENT_NAME)?;
        Ok(unsafe { OnnxRuntimeCommitted::after_process_commit() })
    }
}

/// The evidence Kokoro sessions require, once the process committed its
/// ONNX Runtime environment.
pub(crate) fn committed_kokoro_runtime() -> Option<OnnxRuntimeCommitted> {
    committed_onnx_runtime().map(|_| unsafe { OnnxRuntimeCommitted::after_process_commit() })
}

/// The ONNX Runtime a Kokoro session is built on, committed on first use:
/// the committed process runtime, else the runtime [`OnnxRuntimeInstaller`]
/// resolves or downloads (the linked runtime on Android and iOS), committed
/// through [`OnnxRuntimeReady::initialize`].
#[derive(Debug)]
pub struct ProcessOnnxRuntime<J: ?Sized> {
    jobs: std::sync::Arc<J>,
    paths: OnnxRuntimePaths,
}

impl<J: ?Sized> ProcessOnnxRuntime<J> {
    #[must_use]
    pub const fn new(jobs: std::sync::Arc<J>, paths: OnnxRuntimePaths) -> Self {
        Self { jobs, paths }
    }
}

impl<J: JobStore + ?Sized> ProcessOnnxRuntime<J> {
    async fn initialize(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<OnnxRuntimeCommitted, lettuce_speech::TtsRuntimeError> {
        let client = ArtifactDownloadClient::new()
            .map_err(|_| lettuce_speech::TtsRuntimeError::Unavailable)?;
        let ready = OnnxRuntimeInstaller::new(self.jobs.as_ref(), self.paths.clone())
            .ensure(&client, cancellation, &|_| {})
            .await
            .map_err(|error| match error {
                OnnxRuntimeInstallError::Cancelled => lettuce_speech::TtsRuntimeError::Cancelled,
                error => {
                    tracing::warn!(%error, "ONNX Runtime is unavailable for Kokoro");
                    lettuce_speech::TtsRuntimeError::Unavailable
                }
            })?;
        tokio::task::spawn_blocking(move || ready.initialize())
            .await
            .map_err(|_| lettuce_speech::TtsRuntimeError::Unavailable)?
            .map_err(|error| {
                tracing::warn!(%error, "ONNX Runtime could not be initialized for Kokoro");
                lettuce_speech::TtsRuntimeError::Unavailable
            })
    }
}

#[async_trait]
impl<J: JobStore + ?Sized + 'static> crate::KokoroOnnxRuntimeProvider for ProcessOnnxRuntime<J> {
    async fn committed(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<OnnxRuntimeCommitted, lettuce_speech::TtsRuntimeError> {
        committed_or_initialize(committed_kokoro_runtime(), || self.initialize(cancellation)).await
    }
}

/// `committed` when the process already committed, else the result of
/// `initialize`.
async fn committed_or_initialize<F, Fut>(
    committed: Option<OnnxRuntimeCommitted>,
    initialize: F,
) -> Result<OnnxRuntimeCommitted, lettuce_speech::TtsRuntimeError>
where
    F: FnOnce() -> Fut,
    Fut:
        std::future::Future<Output = Result<OnnxRuntimeCommitted, lettuce_speech::TtsRuntimeError>>,
{
    match committed {
        Some(committed) => Ok(committed),
        None => initialize().await,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxRuntimeInstallEvent {
    /// A release archive is being downloaded by `job_id`; its byte progress
    /// is recorded on that job.
    Downloading { job_id: JobId, bytes: u64 },
    /// The downloaded archive is being unpacked.
    Unpacking { job_id: JobId },
}

#[derive(Debug, thiserror::Error)]
pub enum OnnxRuntimeInstallError {
    #[error(transparent)]
    Unsupported(OnnxRuntimeProvisionError),
    #[error("Failed to download ONNX Runtime archive {url}: {error}")]
    Probe {
        url: String,
        error: ArtifactSourceError,
    },
    #[error("Failed to download ONNX Runtime archive: {0}")]
    Download(ArtifactInstallError),
    #[error("Failed to extract ONNX Runtime archive: {0}")]
    Unpack(OnnxRuntimeProvisionError),
    #[error("ONNX Runtime is already being downloaded")]
    Busy,
    #[error("the ONNX Runtime download was cancelled")]
    Cancelled,
    #[error("the ONNX Runtime extraction task failed")]
    Task,
}

/// Looks up the size of a release archive before it is downloaded.
#[async_trait]
pub trait OnnxRuntimeDownloadSource: ArtifactSourceClient {
    async fn archive_size(&self, url: &str) -> Result<u64, ArtifactSourceError>;
}

#[async_trait]
impl OnnxRuntimeDownloadSource for ArtifactDownloadClient {
    async fn archive_size(&self, url: &str) -> Result<u64, ArtifactSourceError> {
        self.probe_https_size(url)
            .await
            .map_err(|error| match error {
                ArtifactProbeError::Refused(_) => ArtifactSourceError::InvalidResponse,
                ArtifactProbeError::Download(lettuce_network::ArtifactDownloadError::Transport) => {
                    ArtifactSourceError::Transport
                }
                ArtifactProbeError::Download(
                    lettuce_network::ArtifactDownloadError::InvalidRequest,
                ) => ArtifactSourceError::InvalidRequest,
                ArtifactProbeError::Download(
                    lettuce_network::ArtifactDownloadError::InvalidResponse,
                ) => ArtifactSourceError::InvalidResponse,
            })
    }
}

/// Resolves or downloads the runtime. Calls in one process run one at a
/// time, so concurrent consumers share one download.
#[derive(Debug)]
pub struct OnnxRuntimeInstaller<'a, J: ?Sized> {
    jobs: &'a J,
    paths: OnnxRuntimePaths,
}

impl<'a, J: ?Sized> OnnxRuntimeInstaller<'a, J> {
    #[must_use]
    pub const fn new(jobs: &'a J, paths: OnnxRuntimePaths) -> Self {
        Self { jobs, paths }
    }

    /// The runtime already on this device, without downloading, honoring
    /// the process's `ORT_DYLIB_PATH`. Once the process committed its ONNX
    /// Runtime, that runtime is returned without looking at the disk. This
    /// may run `lipo`, `otool` and `codesign` on macOS; async callers should
    /// use [`OnnxRuntimeInstaller::ensure`].
    #[must_use]
    pub fn installed(&self) -> Option<OnnxRuntimeReady> {
        self.installed_with_override(env_override().as_deref())
    }

    #[must_use]
    pub fn installed_with_override(
        &self,
        override_path: Option<&OsStr>,
    ) -> Option<OnnxRuntimeReady> {
        resolve_ready(&self.paths, committed_onnx_runtime(), override_path)
    }
}

/// The committed process runtime, else the mobile linked runtime, else a
/// runtime found on disk.
fn resolve_ready(
    paths: &OnnxRuntimePaths,
    committed: Option<CommittedOnnxRuntime>,
    override_path: Option<&OsStr>,
) -> Option<OnnxRuntimeReady> {
    if let Some(committed) = committed {
        return Some(OnnxRuntimeReady::from(committed));
    }
    if mobile() {
        return Some(OnnxRuntimeReady::Linked);
    }
    let os = OnnxRuntimeHostOs::current()?;
    resolve_installed_onnx_runtime(os, override_path, &paths.locations())
        .map(OnnxRuntimeReady::Library)
}

impl<J: JobStore + ?Sized> OnnxRuntimeInstaller<'_, J> {
    /// The runtime on this device, downloading it first when there is none.
    pub async fn ensure<S: OnnxRuntimeDownloadSource + ?Sized>(
        &self,
        source: &S,
        cancellation: &CancellationToken,
        events: &(dyn Fn(OnnxRuntimeInstallEvent) + Send + Sync),
    ) -> Result<OnnxRuntimeReady, OnnxRuntimeInstallError> {
        self.ensure_with_override(env_override(), source, cancellation, events)
            .await
    }

    pub async fn ensure_with_override<S: OnnxRuntimeDownloadSource + ?Sized>(
        &self,
        override_path: Option<OsString>,
        source: &S,
        cancellation: &CancellationToken,
        events: &(dyn Fn(OnnxRuntimeInstallEvent) + Send + Sync),
    ) -> Result<OnnxRuntimeReady, OnnxRuntimeInstallError> {
        let _serialized = ENSURE_LOCK.lock().await;
        if let Some(committed) = committed_onnx_runtime() {
            return Ok(OnnxRuntimeReady::from(committed));
        }
        let paths = self.paths.clone();
        let found = tokio::task::spawn_blocking(move || {
            resolve_ready(&paths, None, override_path.as_deref())
        })
        .await
        .map_err(|_| OnnxRuntimeInstallError::Task)?;
        if let Some(ready) = found {
            return Ok(ready);
        }
        let unsupported = || {
            OnnxRuntimeInstallError::Unsupported(OnnxRuntimeProvisionError::UnsupportedPlatform {
                os: std::env::consts::OS.to_owned(),
                arch: std::env::consts::ARCH.to_owned(),
            })
        };
        let os = OnnxRuntimeHostOs::current().ok_or_else(unsupported)?;
        let archives = onnx_runtime_archives(os, std::env::consts::ARCH)
            .map_err(OnnxRuntimeInstallError::Unsupported)?;
        self.install_first(os, archives, source, cancellation, events)
            .await
            .map(OnnxRuntimeReady::Library)
    }

    /// Tries each archive in order until one installs; a cancellation stops
    /// at once.
    async fn install_first<S: OnnxRuntimeDownloadSource + ?Sized>(
        &self,
        os: OnnxRuntimeHostOs,
        archives: Vec<OnnxRuntimeArchive>,
        source: &S,
        cancellation: &CancellationToken,
        events: &(dyn Fn(OnnxRuntimeInstallEvent) + Send + Sync),
    ) -> Result<ResolvedOnnxRuntime, OnnxRuntimeInstallError> {
        let mut last_error = None;
        for archive in archives {
            if cancellation.is_cancelled() {
                return Err(OnnxRuntimeInstallError::Cancelled);
            }
            match self
                .download_and_install(os, archive, source, cancellation, events)
                .await
            {
                Ok(resolved) => return Ok(resolved),
                Err(OnnxRuntimeInstallError::Cancelled) => {
                    return Err(OnnxRuntimeInstallError::Cancelled);
                }
                Err(error) => {
                    tracing::warn!(%error, "ONNX Runtime archive failed; trying the next one");
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or(OnnxRuntimeInstallError::Unsupported(
            OnnxRuntimeProvisionError::UnsupportedPlatform {
                os: std::env::consts::OS.to_owned(),
                arch: std::env::consts::ARCH.to_owned(),
            },
        )))
    }

    /// Downloads one archive and unpacks it as the job's `install` stage, so
    /// the job only succeeds with a usable runtime. The archive is deleted
    /// once unpacked and after any unpack failure, so a corrupt or unusable
    /// archive is downloaded afresh next time.
    async fn download_and_install<S: OnnxRuntimeDownloadSource + ?Sized>(
        &self,
        os: OnnxRuntimeHostOs,
        archive: OnnxRuntimeArchive,
        source: &S,
        cancellation: &CancellationToken,
        events: &(dyn Fn(OnnxRuntimeInstallEvent) + Send + Sync),
    ) -> Result<ResolvedOnnxRuntime, OnnxRuntimeInstallError> {
        let bytes = source.archive_size(&archive.url).await.map_err(|error| {
            OnnxRuntimeInstallError::Probe {
                url: archive.url.clone(),
                error,
            }
        })?;
        let plan = archive_plan(&self.paths, &archive, bytes);
        let installer = ArtifactInstallCoordinator::new(self.jobs);
        let admitted = installer
            .admit(&plan)
            .map_err(OnnxRuntimeInstallError::Download)?;
        let job_id = admitted.job.id;
        let now = TimestampMillis::now().unwrap_or(admitted.job.updated_at);
        let closing_plan = plan.clone();
        let mut work = installer
            .claim(
                plan,
                job_id,
                WorkerId::new(),
                now,
                INSTALL_LEASE,
                &ResourceAvailability::all(),
            )
            .map_err(OnnxRuntimeInstallError::Download)?
            .ok_or(OnnxRuntimeInstallError::Busy)?;
        work.handle = JobHandle::with_cancellation(job_id, cancellation.clone());
        let claim = work.claim.claim.clone();
        events(OnnxRuntimeInstallEvent::Downloading { job_id, bytes });
        let unpacked = std::sync::Mutex::new(None);
        let install_dir = self.paths.install_dir.clone();
        let result = installer
            .run_then(
                work,
                source,
                CancellationReason::User,
                now,
                |paths, token| {
                    let unpacked = &unpacked;
                    async move {
                        let archive_file = paths
                            .into_iter()
                            .next()
                            .ok_or(ArtifactInstallError::InvalidWork)?;
                        events(OnnxRuntimeInstallEvent::Unpacking { job_id });
                        let outcome = unpack_archive(os, archive, archive_file, install_dir, token)
                            .await
                            .map_err(|_| {
                                ArtifactInstallError::Finish("unpack task failed".to_owned())
                            })?;
                        let settled = match &outcome {
                            Ok(_) => Ok(()),
                            Err(OnnxRuntimeProvisionError::Cancelled) => {
                                Err(ArtifactInstallError::Cancelled)
                            }
                            Err(error) => Err(ArtifactInstallError::Finish(error.to_string())),
                        };
                        *unpacked
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(outcome);
                        settled
                    }
                },
            )
            .await;
        let unpacked = unpacked
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = match (result, unpacked) {
            (Ok(result), unpacked) => (result, unpacked),
            (Err(error), Some(Ok(resolved))) => {
                tracing::warn!(
                    %error,
                    library = %resolved.library.display(),
                    "ONNX Runtime is installed but its job could not be recorded"
                );
                installer.close_installed(&claim, &closing_plan);
                return Ok(resolved);
            }
            (Err(_), Some(Err(error))) => return Err(OnnxRuntimeInstallError::Unpack(error)),
            (Err(error), None) => return Err(OnnxRuntimeInstallError::Download(error)),
        };
        match result {
            (ArtifactInstallRunResult::Succeeded { .. }, Some(Ok(resolved))) => {
                tracing::info!(
                    library = %resolved.library.display(),
                    version = ONNX_RUNTIME_VERSION,
                    "installed ONNX Runtime"
                );
                Ok(resolved)
            }
            (ArtifactInstallRunResult::Cancelled { .. }, _) => {
                Err(OnnxRuntimeInstallError::Cancelled)
            }
            (_, Some(Err(error))) => Err(OnnxRuntimeInstallError::Unpack(error)),
            (ArtifactInstallRunResult::Failed { error, .. }, _) => {
                Err(OnnxRuntimeInstallError::Download(error))
            }
            (ArtifactInstallRunResult::Succeeded { .. }, _) => Err(
                OnnxRuntimeInstallError::Download(ArtifactInstallError::InvalidWork),
            ),
        }
    }
}

/// Unpacks off the async executor, polling `token` between archive
/// entries; the archive is deleted unless the unpack was cancelled.
async fn unpack_archive(
    os: OnnxRuntimeHostOs,
    archive: OnnxRuntimeArchive,
    archive_file: PathBuf,
    install_dir: PathBuf,
    token: CancellationToken,
) -> Result<Result<ResolvedOnnxRuntime, OnnxRuntimeProvisionError>, tokio::task::JoinError> {
    tokio::task::spawn_blocking(move || {
        let outcome =
            install_onnx_runtime_archive(os, &archive, &archive_file, &install_dir, &|| {
                token.is_cancelled()
            });
        if !matches!(outcome, Err(OnnxRuntimeProvisionError::Cancelled))
            && let Err(error) = std::fs::remove_file(&archive_file)
        {
            tracing::warn!(%error, "could not delete the ONNX Runtime archive");
        }
        outcome
    })
    .await
}

/// The archive's download. The release publishes no checksum, so the
/// download is verified by the size the server reports, and the unpacked
/// library is checked before it is used.
fn archive_plan(
    paths: &OnnxRuntimePaths,
    archive: &OnnxRuntimeArchive,
    bytes: u64,
) -> ArtifactInstallPlan {
    let source = ArtifactSource::Https {
        url: archive.url.clone(),
    };
    ArtifactInstallPlan {
        install_id: format!("onnxruntime:{ONNX_RUNTIME_VERSION}:{}", archive.file_name),
        root: paths.downloads.clone(),
        artifacts: vec![PlannedArtifact {
            artifact: PinnedArtifact {
                source_identity: source.identity(),
                local_segments: vec![ONNX_RUNTIME_VERSION.to_owned(), archive.file_name.clone()],
                byte_size: bytes,
                sha256: None,
            },
            source,
        }],
    }
}

fn env_override() -> Option<OsString> {
    std::env::var_os(ORT_DYLIB_PATH)
}

const fn mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "onnx_runtime_install_tests.rs"]
mod tests;
