use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_image_generation::CivitaiLoraDownload;
use lettuce_jobs::{
    CancellationReason, JobError, JobErrorCode, JobKind, JobMutation, JobQuery, JobSnapshot,
    JobState, JobStore, StoreError, WorkerId, handle::CancellationToken,
};
use lettuce_model_hub::{
    CompanionEmotionInstallStore, EmbeddingPin, KokoroInstallStore, KokoroVoiceInstallStore,
    RemoteCompanionEmotionModel, RemoteKokoroModel, RemoteWhisperModel,
};
use lettuce_types::{JobId, PageLimit, PageRequest};

use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error};
use crate::{
    ArtifactInstallClaimedWork, ArtifactInstallCoordinator, ArtifactInstallError,
    ArtifactInstallPlan, ArtifactInstallRunResult, ArtifactSourceClient, CivitaiBrowser,
    CompanionEmotionDownloadError, EmbeddingModelCoordinator, GgufDownload, GgufModelSetup,
    HuggingFaceBrowser, KokoroDownloadClaimedWork, KokoroDownloadSource, KokoroVoiceBundle,
    KokoroVoiceDownloadClaimedWork, KokoroVoiceDownloadSource, WhisperDownloadClaimedWork,
    WhisperDownloadSource,
};

/// A claim outlives a stalled download long enough to recover on its own;
/// progress and the install stage renew it.
const INSTALL_LEASE: Duration = Duration::from_secs(30 * 60);
const RECOVERY_PAGE: u16 = 200;
pub(super) const DOWNLOAD_LANE: &str = "install:downloads";

/// A stable-diffusion.cpp catalog variant and the engine build it runs on.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogVariant {
    pub profile_id: String,
    pub variant_id: String,
    pub runtime_release: String,
    pub runtime_asset: String,
}

/// What completes an artifact install once its files are verified.
#[derive(Debug, Clone)]
pub enum InstallFinish {
    /// The verified files are the whole install (the upscaler).
    Files,
    /// A downloaded GGUF model joins the library; with `create_model` it
    /// also becomes a llama.cpp model set up that way.
    Gguf {
        root: PathBuf,
        download: GgufDownload,
        create_model: Option<GgufModelSetup>,
    },
    /// Extracts an engine build, then registers `variant` when every file
    /// of it is on disk.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    StableDiffusionRuntime {
        paths: lettuce_image_generation::sd_runtime::layout::DiffusionPaths,
        release: String,
        asset: lettuce_image_generation::sd_runtime::releases::RuntimeAsset,
        variant: Option<CatalogVariant>,
    },
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    StableDiffusionVariant {
        paths: lettuce_image_generation::sd_runtime::layout::DiffusionPaths,
        variant: CatalogVariant,
    },
    HuggingFaceBundle {
        paths: lettuce_image_generation::sd_runtime::layout::DiffusionPaths,
        bundle_id: String,
    },
    CivitaiLora {
        lora_root: PathBuf,
        download: CivitaiLoraDownload,
    },
    Embedding {
        root: PathBuf,
        pin: EmbeddingPin,
        enable_dynamic_memory: bool,
    },
    CompanionEmotion {
        root: PathBuf,
        remote: RemoteCompanionEmotionModel,
    },
}

/// An install the runner holds until it claims the job. The job store keeps
/// only the job, so work admitted by an earlier process is recovered at
/// startup.
#[derive(Debug, Clone)]
pub enum InstallWork {
    Artifact {
        plan: ArtifactInstallPlan,
        finish: Box<InstallFinish>,
    },
    Whisper {
        model: RemoteWhisperModel,
        install_root: PathBuf,
    },
    KokoroModel {
        model: RemoteKokoroModel,
        install_root: PathBuf,
    },
    KokoroVoices {
        bundle: KokoroVoiceBundle,
        install_root: PathBuf,
    },
}

impl InstallWork {
    /// The folder the install writes below.
    pub(crate) fn root(&self) -> &std::path::Path {
        match self {
            Self::Artifact { plan, .. } => &plan.root,
            Self::Whisper { install_root, .. }
            | Self::KokoroModel { install_root, .. }
            | Self::KokoroVoices { install_root, .. } => install_root,
        }
    }

    /// The Hugging Face repository the install downloads from, if any.
    pub(crate) fn hugging_face_repository(&self) -> Option<&str> {
        match self {
            Self::Artifact { plan, .. } => plan.artifacts.iter().find_map(|planned| match &planned
                .source
            {
                crate::ArtifactSource::HuggingFace { repository, .. } => Some(repository.as_str()),
                crate::ArtifactSource::Https { .. } => None,
            }),
            Self::Whisper { .. } => Some(crate::WHISPER_REPOSITORY),
            Self::KokoroModel { .. } | Self::KokoroVoices { .. } => {
                Some(lettuce_model_hub::KOKORO_REPOSITORY)
            }
        }
    }

    /// GGUF, stable-diffusion.cpp, CivitAI, Hugging Face bundle, Whisper
    /// and Kokoro downloads share one sequential queue, as legacy's download
    /// queue did; the embedding and emotion models share a separate lane.
    pub(super) fn lane(&self) -> JobLane {
        let name = match self {
            Self::Artifact { finish, .. } => match finish.as_ref() {
                InstallFinish::Embedding { .. } | InstallFinish::CompanionEmotion { .. } => {
                    "install:memory-models"
                }
                _ => DOWNLOAD_LANE,
            },
            Self::Whisper { .. } | Self::KokoroModel { .. } | Self::KokoroVoices { .. } => {
                DOWNLOAD_LANE
            }
        };
        JobLane(name.to_owned())
    }
}

/// Where install downloads come from; tests replace the network.
#[async_trait]
pub trait InstallSources: Send + Sync {
    async fn artifacts(
        &self,
        context: &ApiContext,
        finish: &InstallFinish,
    ) -> Result<Box<dyn ArtifactSourceClient>, ApiError>;

    fn whisper(&self) -> Result<Box<dyn WhisperDownloadSource>, ApiError>;

    fn kokoro_model(&self) -> Result<Box<dyn KokoroDownloadSource>, ApiError>;

    fn kokoro_voices(&self) -> Result<Box<dyn KokoroVoiceDownloadSource>, ApiError>;
}

/// Downloads over the network, signed in with the saved Hugging Face token,
/// or the CivitAI token for a CivitAI LoRA.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkInstallSources;

fn unavailable(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Unavailable, error.to_string())
}

fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}

#[async_trait]
impl InstallSources for NetworkInstallSources {
    async fn artifacts(
        &self,
        context: &ApiContext,
        finish: &InstallFinish,
    ) -> Result<Box<dyn ArtifactSourceClient>, ApiError> {
        let secrets = context.secret_store().as_ref();
        let client = match finish {
            InstallFinish::CivitaiLora { .. } => CivitaiBrowser::download_client(secrets)
                .await
                .map_err(unavailable)?,
            _ => HuggingFaceBrowser::download_client(secrets)
                .await
                .map_err(unavailable)?,
        };
        Ok(Box::new(client))
    }

    fn whisper(&self) -> Result<Box<dyn WhisperDownloadSource>, ApiError> {
        crate::HuggingFaceWhisperDownloadSource::new()
            .map(|source| Box::new(source) as Box<dyn WhisperDownloadSource>)
            .map_err(unavailable)
    }

    fn kokoro_model(&self) -> Result<Box<dyn KokoroDownloadSource>, ApiError> {
        crate::HuggingFaceKokoroDownloadSource::new()
            .map(|source| Box::new(source) as Box<dyn KokoroDownloadSource>)
            .map_err(unavailable)
    }

    fn kokoro_voices(&self) -> Result<Box<dyn KokoroVoiceDownloadSource>, ApiError> {
        crate::HuggingFaceKokoroVoiceDownloadSource::new()
            .map(|source| Box::new(source) as Box<dyn KokoroVoiceDownloadSource>)
            .map_err(unavailable)
    }
}

/// Admits an install and hands its work to the runner. A request for an
/// install already queued or running joins that job; one that already
/// finished (a replayed Whisper or Kokoro download) is returned as is.
pub async fn admit_install(
    context: &ApiContext,
    work: InstallWork,
) -> Result<dto::JobAccepted, ApiError> {
    admit_install_with_detail(context, work, None).await
}

/// Admits an install and stores `detail` with its job before the runner can
/// reach it, for the download center to show what it is.
pub(crate) async fn admit_install_with_detail(
    context: &ApiContext,
    work: InstallWork,
    detail: Option<serde_json::Value>,
) -> Result<dto::JobAccepted, ApiError> {
    let job_id = context
        .blocking(move |context| {
            let _folder_access = context.local_models().folder_access();
            if is_image_install(&work) || work.root().starts_with(crate::api::local_models::models_root(context)?) {
                super::local::folder_move_active(context)?;
            }
            let (job, work) = admit(context, work)?;
            if let Some(detail) = &detail {
                context
                    .backend()
                    .database()
                    .record_local_model_job(job.id, detail)
                    .map_err(internal)?;
            }
            if !job.state.is_terminal() {
                context.jobs().put_install(job.id, work);
            }
            Ok(job.id)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

/// Whether the install writes image models, engine builds or LoRAs.
fn is_image_install(work: &InstallWork) -> bool {
    let InstallWork::Artifact { finish, .. } = work else {
        return false;
    };
    match finish.as_ref() {
        InstallFinish::Files
        | InstallFinish::HuggingFaceBundle { .. }
        | InstallFinish::CivitaiLora { .. } => true,
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        InstallFinish::StableDiffusionRuntime { .. }
        | InstallFinish::StableDiffusionVariant { .. } => true,
        _ => false,
    }
}

fn admit(context: &ApiContext, work: InstallWork) -> Result<(JobSnapshot, InstallWork), ApiError> {
    let backend = context.backend();
    let database = backend.database();
    Ok(match work {
        InstallWork::Artifact { plan, finish } => match *finish {
            InstallFinish::CompanionEmotion { root, remote } => {
                let admitted = crate::admit_companion_emotion_install(database, &root, &remote)
                    .map_err(|error| match error {
                        CompanionEmotionDownloadError::InstallInProgress(_) => {
                            api_error(ApiErrorCode::Busy, error.to_string())
                        }
                        error => internal(error),
                    })?;
                (
                    admitted.job,
                    InstallWork::Artifact {
                        plan: admitted.plan,
                        finish: Box::new(InstallFinish::CompanionEmotion {
                            root,
                            remote: admitted.remote,
                        }),
                    },
                )
            }
            finish => {
                let admitted = ArtifactInstallCoordinator::new(database)
                    .admit(&plan)
                    .map_err(internal)?;
                (
                    admitted.job,
                    InstallWork::Artifact {
                        plan,
                        finish: Box::new(finish),
                    },
                )
            }
        },
        InstallWork::Whisper {
            model,
            install_root,
        } => {
            let admitted = backend
                .whisper_downloads(&install_root)
                .map_err(internal)?
                .admit(model)
                .map_err(internal)?;
            (
                admitted.job,
                InstallWork::Whisper {
                    model: admitted.model,
                    install_root,
                },
            )
        }
        InstallWork::KokoroModel {
            model,
            install_root,
        } => {
            let installs = KokoroInstallStore::open(&install_root).map_err(internal)?;
            let admitted = backend
                .kokoro_downloads(installs)
                .admit(model)
                .map_err(internal)?;
            (
                admitted.job,
                InstallWork::KokoroModel {
                    model: admitted.model,
                    install_root,
                },
            )
        }
        InstallWork::KokoroVoices {
            bundle,
            install_root,
        } => {
            let installs = KokoroVoiceInstallStore::open(&install_root).map_err(internal)?;
            let admitted = backend
                .kokoro_voice_downloads(installs)
                .admit(bundle)
                .map_err(internal)?;
            (
                admitted.job,
                InstallWork::KokoroVoices {
                    bundle: admitted.bundle,
                    install_root,
                },
            )
        }
    })
}

/// Queued installs whose work this process does not hold: a GGUF download
/// resumes from the detail stored with it and the Thymos install from its
/// hint; every other one is cancelled so a new request admits a fresh job.
/// Returns the cancelled jobs.
pub(crate) fn recover_queued_installs(context: &ApiContext) -> Result<Vec<JobId>, ApiError> {
    let database = context.backend().database();
    let mut waiting = Vec::new();
    for state in [JobState::Queued, JobState::CancellationRequested] {
        let mut cursor = None;
        loop {
            let page = database
                .list(JobQuery {
                    state: Some(state),
                    kind: Some(JobKind::ArtifactInstall),
                    subject: None,
                    page: PageRequest {
                        cursor: cursor.take(),
                        limit: PageLimit::new(RECOVERY_PAGE),
                    },
                })
                .map_err(IntoApiError::into_api_error)?;
            waiting.extend(page.items.into_iter().filter(|job| job.claim.is_none()));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
    }
    if let Some(root) = context.retained_model_roots()?.thymos {
        resume_companion_emotion(context, std::path::Path::new(&root), &waiting);
    }
    let mut cancelled = Vec::new();
    for job in waiting {
        if context.jobs().has_install(job.id) {
            continue;
        }
        if job.state == JobState::Queued {
            if let Some(record) = context.backend().database().local_model_job(job.id).map_err(internal)?
                && let Ok(super::local::LocalModelJobDetail::EmbeddingInstall { root, pin, enable_dynamic_memory }) = serde_json::from_value(record.detail) {
                let plan = crate::embedding_install_plan(&root, &pin);
                context.jobs().put_install(job.id, InstallWork::Artifact { plan, finish: Box::new(InstallFinish::Embedding { root, pin, enable_dynamic_memory }) });
                continue;
            }
            match super::local::resume_gguf_install(context, &job) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(job_id = %job.id, message = %error.message, "a queued GGUF install could not be resumed");
                }
            }
        }
        match cancel_waiting(context, &job) {
            Ok(()) => cancelled.push(job.id),
            Err(error) => {
                tracing::warn!(job_id = %job.id, code = ?error.code, message = %error.message, "a queued install could not be cancelled");
            }
        }
    }
    Ok(cancelled)
}

fn resume_companion_emotion(context: &ApiContext, root: &std::path::Path, waiting: &[JobSnapshot]) {
    let hint = CompanionEmotionInstallStore::open(root)
        .ok()
        .and_then(|store| store.lock().active().ok().flatten());
    let Some(hint) = hint else {
        return;
    };
    if !waiting
        .iter()
        .any(|job| job.state == JobState::Queued && job.id.to_string() == hint.job_id)
    {
        return;
    }
    match admit(
        context,
        InstallWork::Artifact {
            plan: ArtifactInstallPlan {
                install_id: String::new(),
                root: root.to_path_buf(),
                artifacts: Vec::new(),
            },
            finish: Box::new(InstallFinish::CompanionEmotion {
                root: root.to_path_buf(),
                remote: hint.remote,
            }),
        },
    ) {
        Ok((job, work)) if !job.state.is_terminal() => context.jobs().put_install(job.id, work),
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(code = ?error.code, message = %error.message, "the queued Thymos install could not be resumed");
        }
    }
}

fn cancel_waiting(context: &ApiContext, job: &JobSnapshot) -> Result<(), ApiError> {
    let database = context.backend().database();
    let at = context.now().max(job.updated_at);
    let requested = match job.state {
        JobState::Queued => database
            .append_and_transition(JobMutation::RequestCancellation {
                id: job.id,
                reason: CancellationReason::Recovery,
                at,
            })
            .map_err(IntoApiError::into_api_error)?,
        _ => job.clone(),
    };
    database
        .append_and_transition(JobMutation::FinishQueuedCancellation {
            id: job.id,
            at: at.max(requested.updated_at),
        })
        .map_err(IntoApiError::into_api_error)?;
    Ok(())
}

/// Runs `ArtifactInstall` jobs: artifact installs through
/// `ArtifactInstallCoordinator` and their finisher, Whisper and Kokoro
/// downloads through their coordinators, which record the install.
pub struct ArtifactInstallHandler {
    sources: Arc<dyn InstallSources>,
}

impl std::fmt::Debug for ArtifactInstallHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArtifactInstallHandler")
            .finish_non_exhaustive()
    }
}

impl ArtifactInstallHandler {
    #[must_use]
    pub fn new(sources: Arc<dyn InstallSources>) -> Self {
        Self { sources }
    }
}

enum InstallSource {
    Artifact(Box<dyn ArtifactSourceClient>),
    Whisper(Box<dyn WhisperDownloadSource>),
    KokoroModel(Box<dyn KokoroDownloadSource>),
    KokoroVoices(Box<dyn KokoroVoiceDownloadSource>),
}

enum ClaimedWork {
    Artifact(ArtifactInstallClaimedWork),
    Whisper(WhisperDownloadClaimedWork),
    KokoroModel(KokoroDownloadClaimedWork),
    KokoroVoices(KokoroVoiceDownloadClaimedWork),
}

#[async_trait]
impl JobHandler for ArtifactInstallHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::ArtifactInstall]
    }

    fn lane(&self, context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        context.jobs().install(job.id).map(|work| work.lane())
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let Some(work) = context.jobs().install(job.id) else {
            return Ok(None);
        };
        let job_id = job.id;
        let source = match &work {
            InstallWork::Artifact { finish, .. } => self
                .sources
                .artifacts(context, finish)
                .await
                .map(InstallSource::Artifact),
            InstallWork::Whisper { .. } => self.sources.whisper().map(InstallSource::Whisper),
            InstallWork::KokoroModel { .. } => {
                self.sources.kokoro_model().map(InstallSource::KokoroModel)
            }
            InstallWork::KokoroVoices { .. } => self
                .sources
                .kokoro_voices()
                .map(InstallSource::KokoroVoices),
        };
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                tracing::warn!(%job_id, message = %error.message, "the install's download source is unavailable");
                context
                    .blocking(move |context| {
                        context.jobs().forget_install(job_id);
                        settle_unclaimable(context, job_id, SOURCE_UNAVAILABLE)
                    })
                    .await?;
                return Ok(None);
            }
        };
        let resources = self.resources();
        let claimed = context
            .blocking(move |context| {
                match claim_work(context, job_id, work.clone(), worker_id, &resources) {
                    Ok(Some(claimed)) => Ok(Some((claimed, work))),
                    Ok(None) => {
                        let ended = context
                            .backend()
                            .database()
                            .get(job_id)
                            .map_err(IntoApiError::into_api_error)?
                            .is_none_or(|job| job.state.is_terminal());
                        if ended {
                            context.jobs().forget_install(job_id);
                        }
                        Ok(None)
                    }
                    Err(ClaimFailure::Transient(message)) => {
                        Err(api_error(ApiErrorCode::Unavailable, message))
                    }
                    Err(ClaimFailure::Invalid(message)) => {
                        tracing::warn!(%job_id, %message, "install work cannot run");
                        context.jobs().forget_install(job_id);
                        settle_unclaimable(context, job_id, WORK_INVALID)?;
                        Ok(None)
                    }
                }
            })
            .await?;
        let Some((claimed, work)) = claimed else {
            return Ok(None);
        };
        Ok(Some(Box::new(ClaimedInstall {
            work,
            claimed,
            source,
        })))
    }
}

/// A download source could not be built (no client, the saved token could
/// not be read).
const SOURCE_UNAVAILABLE: (JobErrorCode, bool, &str) = (
    JobErrorCode::ResourceUnavailable,
    true,
    "install-source-unavailable",
);
/// The install work does not match its job.
const WORK_INVALID: (JobErrorCode, bool, &str) =
    (JobErrorCode::InvalidInput, false, "install-work-invalid");

/// Ends a job the runner could not run: one whose cancellation was requested
/// (after a claim or before it) is cancelled, any other fails with
/// `failure`. An ended job is left alone.
fn settle_unclaimable(
    context: &ApiContext,
    job_id: JobId,
    failure: (JobErrorCode, bool, &'static str),
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let Some(job) = database.get(job_id).map_err(IntoApiError::into_api_error)? else {
        return Ok(());
    };
    let at = context.now().max(job.updated_at);
    let (code, retryable, message) = failure;
    let error =
        JobError::new(code, retryable, message).map_err(|_| internal("invalid job error"))?;
    match (job.state, job.claim.clone()) {
        (JobState::CancellationRequested, Some(claim)) => {
            crate::models::artifact_install::finish_claimed_cancellation(database, &claim, at)
                .map_err(IntoApiError::into_api_error)?;
        }
        (JobState::CancellationRequested, None) => {
            database
                .append_and_transition(JobMutation::FinishQueuedCancellation { id: job_id, at })
                .map_err(IntoApiError::into_api_error)?;
        }
        (JobState::Queued, _) => {
            let claim = database
                .claim(
                    job_id,
                    WorkerId::new(),
                    at,
                    INSTALL_LEASE,
                    &lettuce_jobs::ResourceAvailability::all(),
                )
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| internal("the install job could not be claimed to end it"))?;
            database
                .append_and_transition(JobMutation::Start {
                    claim: claim.claim.clone(),
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
            database
                .append_and_transition(JobMutation::Fail {
                    claim: claim.claim,
                    error,
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
        }
        (JobState::Claimed, Some(claim)) => {
            database
                .append_and_transition(JobMutation::Start {
                    claim: claim.clone(),
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
            database
                .append_and_transition(JobMutation::Fail { claim, error, at })
                .map_err(IntoApiError::into_api_error)?;
        }
        (JobState::Running, Some(claim)) => {
            database
                .append_and_transition(JobMutation::Fail { claim, error, at })
                .map_err(IntoApiError::into_api_error)?;
        }
        _ => {}
    }
    Ok(())
}

/// Why a claim failed: a transient storage failure leaves the job queued
/// for the runner to retry; anything else means the work can never run.
#[derive(Debug)]
enum ClaimFailure {
    Transient(String),
    Invalid(String),
}

impl ClaimFailure {
    fn of_store(error: &StoreError) -> Self {
        match error {
            StoreError::Storage => Self::Transient(error.to_string()),
            _ => Self::Invalid(error.to_string()),
        }
    }

    fn artifact(error: ArtifactInstallError) -> Self {
        match &error {
            ArtifactInstallError::Jobs(store) => Self::of_store(store),
            _ => Self::Invalid(error.to_string()),
        }
    }

    fn whisper(error: crate::WhisperDownloadError) -> Self {
        match &error {
            crate::WhisperDownloadError::Jobs(store) => Self::of_store(store),
            crate::WhisperDownloadError::Repository(_) => Self::Transient(error.to_string()),
            _ => Self::Invalid(error.to_string()),
        }
    }

    fn kokoro_model(error: crate::KokoroDownloadError) -> Self {
        match &error {
            crate::KokoroDownloadError::Jobs(store) => Self::of_store(store),
            _ => Self::Invalid(error.to_string()),
        }
    }

    fn kokoro_voices(error: crate::KokoroVoiceDownloadError) -> Self {
        match &error {
            crate::KokoroVoiceDownloadError::Jobs(store) => Self::of_store(store),
            _ => Self::Invalid(error.to_string()),
        }
    }
}

fn claim_work(
    context: &ApiContext,
    job_id: JobId,
    work: InstallWork,
    worker_id: WorkerId,
    resources: &lettuce_jobs::ResourceAvailability,
) -> Result<Option<ClaimedWork>, ClaimFailure> {
    let backend = context.backend();
    let database = backend.database();
    let now = context.now();
    let invalid = |error: &dyn std::fmt::Display| ClaimFailure::Invalid(error.to_string());
    Ok(match work {
        InstallWork::Artifact { plan, .. } => ArtifactInstallCoordinator::new(database)
            .claim(plan, job_id, worker_id, now, INSTALL_LEASE, resources)
            .map_err(ClaimFailure::artifact)?
            .map(ClaimedWork::Artifact),
        InstallWork::Whisper {
            model,
            install_root,
        } => backend
            .whisper_downloads(&install_root)
            .map_err(ClaimFailure::whisper)?
            .claim(model, job_id, worker_id, now, INSTALL_LEASE, resources)
            .map_err(ClaimFailure::whisper)?
            .map(ClaimedWork::Whisper),
        InstallWork::KokoroModel {
            model,
            install_root,
        } => backend
            .kokoro_downloads(
                KokoroInstallStore::open(&install_root).map_err(|error| invalid(&error))?,
            )
            .claim(model, job_id, worker_id, now, INSTALL_LEASE, resources)
            .map_err(ClaimFailure::kokoro_model)?
            .map(ClaimedWork::KokoroModel),
        InstallWork::KokoroVoices {
            bundle,
            install_root,
        } => backend
            .kokoro_voice_downloads(
                KokoroVoiceInstallStore::open(&install_root).map_err(|error| invalid(&error))?,
            )
            .claim(bundle, job_id, worker_id, now, INSTALL_LEASE, resources)
            .map_err(ClaimFailure::kokoro_voices)?
            .map(ClaimedWork::KokoroVoices),
    })
}

struct ClaimedInstall {
    work: InstallWork,
    claimed: ClaimedWork,
    source: InstallSource,
}

#[async_trait]
impl ClaimedJob for ClaimedInstall {
    fn cancellation(&self) -> CancellationToken {
        match &self.claimed {
            ClaimedWork::Artifact(work) => work.handle.cancellation_token(),
            ClaimedWork::Whisper(work) => work.handle.cancellation_token(),
            ClaimedWork::KokoroModel(work) => work.handle.cancellation_token(),
            ClaimedWork::KokoroVoices(work) => work.handle.cancellation_token(),
        }
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let Self {
            work,
            claimed,
            source,
        } = *self;
        let job_id = match &claimed {
            ClaimedWork::Artifact(work) => work.job.id,
            ClaimedWork::Whisper(work) => work.job.id,
            ClaimedWork::KokoroModel(work) => work.job.id,
            ClaimedWork::KokoroVoices(work) => work.job.id,
        };
        let repository = work.hugging_face_repository().map(str::to_owned);
        let result = run_claimed(&context, work, claimed, source).await;
        context.jobs().forget_install(job_id);
        if let Some(repository) = repository {
            let recorded = context
                .blocking(move |context| record_refusal(context, job_id, &repository))
                .await;
            if let Err(error) = recorded {
                tracing::warn!(%job_id, message = %error.message, "a refused download's repository could not be recorded");
            }
        }
        result
    }
}

async fn run_claimed(
    context: &ApiContext,
    work: InstallWork,
    claimed: ClaimedWork,
    source: InstallSource,
) -> Result<(), ApiError> {
    let backend = context.backend();
    let database = backend.database();
    let now = context.now();
    let reason = CancellationReason::Shutdown;
    match (work, claimed, source) {
        (
            InstallWork::Artifact { plan, finish },
            ClaimedWork::Artifact(claimed),
            InstallSource::Artifact(source),
        ) => match *finish {
            InstallFinish::CompanionEmotion { root, remote } => {
                context.models_changed();
                let result = ArtifactInstallCoordinator::new(database)
                    .run(claimed, source.as_ref(), reason, now)
                    .await
                    .map_err(internal)?;
                if let ArtifactInstallRunResult::Succeeded { .. } = result {
                    crate::finish_companion_emotion_install(database, &root, &remote)
                        .map_err(internal)?;
                    context.models_changed();
                }
            }
            finish => {
                let reload = matches!(finish, InstallFinish::Embedding { .. });
                if reload {
                    context.models_changed();
                }
                let finisher = context.clone();
                let job_id = claimed.job.id;
                let result = ArtifactInstallCoordinator::new(database)
                    .run_then(
                        claimed,
                        source.as_ref(),
                        reason,
                        now,
                        move |paths, cancellation| async move {
                            finish_artifact(&finisher, job_id, &plan, finish, paths, cancellation)
                                .await
                                .map_err(ArtifactInstallError::Finish)
                        },
                    )
                    .await
                    .map_err(internal)?;
                if reload && matches!(result, ArtifactInstallRunResult::Succeeded { .. }) {
                    context.models_changed();
                }
            }
        },
        (
            InstallWork::Whisper { install_root, .. },
            ClaimedWork::Whisper(claimed),
            InstallSource::Whisper(source),
        ) => {
            backend
                .whisper_downloads(&install_root)
                .map_err(internal)?
                .run(claimed, source.as_ref(), reason, now)
                .await
                .map_err(internal)?;
        }
        (
            InstallWork::KokoroModel { install_root, .. },
            ClaimedWork::KokoroModel(claimed),
            InstallSource::KokoroModel(source),
        ) => {
            backend
                .kokoro_downloads(KokoroInstallStore::open(&install_root).map_err(internal)?)
                .run(claimed, source.as_ref(), reason, now)
                .await
                .map_err(internal)?;
        }
        (
            InstallWork::KokoroVoices { install_root, .. },
            ClaimedWork::KokoroVoices(claimed),
            InstallSource::KokoroVoices(source),
        ) => {
            backend
                .kokoro_voice_downloads(
                    KokoroVoiceInstallStore::open(&install_root).map_err(internal)?,
                )
                .run(claimed, source.as_ref(), reason, now)
                .await
                .map_err(internal)?;
        }
        _ => return Err(internal("install work does not match its claim")),
    }
    Ok(())
}

/// Keeps the repository of a download Hugging Face refused or never
/// answered, so its failure names the gated repository.
fn record_refusal(context: &ApiContext, job_id: JobId, repository: &str) -> Result<(), ApiError> {
    let database = context.backend().database();
    let Some(job) = database.get(job_id).map_err(IntoApiError::into_api_error)? else {
        return Ok(());
    };
    if job.state == JobState::Failed
        && job
            .error
            .as_ref()
            .is_some_and(|error| crate::is_hf_job_error(error.message.as_str()))
    {
        database
            .record_hugging_face_refusal(job_id, repository)
            .map_err(internal)?;
    }
    Ok(())
}

/// Completes an install whose files are verified, as the job's `install`
/// stage; an error fails the job.
async fn finish_artifact(
    context: &ApiContext,
    job_id: JobId,
    plan: &ArtifactInstallPlan,
    finish: InstallFinish,
    paths: Vec<PathBuf>,
    cancellation: CancellationToken,
) -> Result<(), String> {
    let database = context.backend().database();
    let now = context.now();
    match finish {
        InstallFinish::Files | InstallFinish::CompanionEmotion { .. } => Ok(()),
        InstallFinish::Gguf {
            root,
            download,
            create_model,
        } => {
            let model_profile_id = match create_model {
                Some(setup) => Some(
                    crate::register_downloaded_gguf(database, &root, &download, &setup, now)
                        .map_err(|error| error.to_string())?
                        .id
                        .to_string(),
                ),
                None => None,
            };
            super::local::record_result(
                context,
                job_id,
                &super::local::LocalModelJobResult::ModelInstalled {
                    model_path: download.installed(&root).model_path,
                    model_profile_id,
                },
            )
            .map_err(|error| error.message)
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        InstallFinish::StableDiffusionRuntime {
            paths: layout,
            release,
            asset,
            variant,
        } => {
            crate::finish_runtime_install(&layout, &release, &asset, paths)
                .await
                .map_err(|error| error.to_string())?;
            let Some(variant) = variant else {
                return Ok(());
            };
            let (profile, catalog_variant) = lettuce_image_generation::diffusion_catalog()
                .find_variant(&variant.profile_id, &variant.variant_id)
                .map_err(|error| error.to_string())?;
            if crate::is_variant_installed(
                &layout,
                profile,
                catalog_variant,
                Some((&variant.runtime_release, &variant.runtime_asset)),
                false,
            ) {
                register_variant(database, &layout, &variant, now)?;
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        InstallFinish::StableDiffusionVariant {
            paths: layout,
            variant,
        } => register_variant(database, &layout, &variant, now),
        InstallFinish::HuggingFaceBundle {
            paths: layout,
            bundle_id,
        } => {
            let manifest =
                crate::finish_hf_bundle(database, &layout, &bundle_id, plan, now).await?;
            let state = match manifest.registration_state {
                lettuce_image_generation::BundleRegistrationState::Downloading => {
                    dto::ImageBundleState::Downloading
                }
                lettuce_image_generation::BundleRegistrationState::Registered => {
                    dto::ImageBundleState::Registered
                }
                lettuce_image_generation::BundleRegistrationState::SetupFailed => {
                    dto::ImageBundleState::SetupFailed
                }
            };
            super::local::record_result(
                context,
                job_id,
                &super::local::LocalModelJobResult::ImageBundle {
                    bundle_id,
                    state,
                    model_id: manifest.model_id,
                    setup_error: manifest.setup_error,
                },
            )
            .map_err(|error| error.message)
        }
        InstallFinish::CivitaiLora {
            lora_root,
            download,
        } => crate::record_civitai_lora(database, &lora_root, &download, now),
        InstallFinish::Embedding { root, pin, enable_dynamic_memory } => {
            EmbeddingModelCoordinator::new(&root, database).complete_install(&pin).map_err(|error| error.to_string())?;
            context.models_changed();
            context.models().prepare_embedding(context).await.map_err(|error| error.message)?;
            let engine = match context.models().resolve_embedding(context) {
                crate::api::ModelLoad::Loaded(engine) => engine,
                _ => return Err("the installed embedding model could not be loaded".into()),
            };
            crate::api::embedding_health::run(engine.as_ref(), &cancellation)?;
            if enable_dynamic_memory {
                use lettuce_settings::GlobalSettingsStore;
                let mut stored = GlobalSettingsStore::load(database).map_err(|error| error.to_string())?;
                stored.settings.dynamic_memory = lettuce_settings::DynamicMemorySettings {
                    enabled: true, min_similarity_basis_points: Some(3200), ..Default::default()
                };
                GlobalSettingsStore::save(database, stored.settings, stored.default_model_profile_id, stored.revision)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        },
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn register_variant(
    database: &lettuce_database::Database,
    layout: &lettuce_image_generation::sd_runtime::layout::DiffusionPaths,
    variant: &CatalogVariant,
    now: lettuce_types::TimestampMillis,
) -> Result<(), String> {
    crate::register_catalog_model(
        database,
        layout,
        &variant.profile_id,
        &variant.variant_id,
        &variant.runtime_release,
        &variant.runtime_asset,
        now,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod claim_failure_tests {
    use super::*;

    #[test]
    fn only_storage_failures_are_retried() {
        assert!(matches!(
            ClaimFailure::artifact(ArtifactInstallError::Jobs(StoreError::Storage)),
            ClaimFailure::Transient(_)
        ));
        assert!(matches!(
            ClaimFailure::artifact(ArtifactInstallError::InvalidWork),
            ClaimFailure::Invalid(_)
        ));
        assert!(matches!(
            ClaimFailure::kokoro_model(crate::KokoroDownloadError::Jobs(StoreError::Storage)),
            ClaimFailure::Transient(_)
        ));
        assert!(matches!(
            ClaimFailure::kokoro_voices(crate::KokoroVoiceDownloadError::Jobs(
                StoreError::IllegalTransition
            )),
            ClaimFailure::Invalid(_)
        ));
    }
}
