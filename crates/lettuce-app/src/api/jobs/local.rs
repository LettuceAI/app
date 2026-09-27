//! Local model jobs: GGUF downloads (admitted here, run as artifact
//! installs), Ollama pulls and models folder moves. What each job works on
//! and produced is kept next to the job, so the download center can show it
//! after a restart.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{
    BytesProgress, CancellationPolicy, CancellationReason, ClaimRef, IdempotencyKey, JobCatalog,
    JobError, JobErrorCode, JobKind, JobListFilter, JobMutation, JobOutcome, JobPriority,
    JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, ProgressSnapshot,
    RecoveryPolicy, ResourceClass, StageSnapshot, SubjectKind, WorkerId, handle::CancellationToken,
};
use lettuce_types::{JobId, PageLimit, PageRequest, RequestId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::install::{DOWNLOAD_LANE, InstallFinish, InstallWork};
use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, hf_error, invalid_field};
use crate::api::local_models::{busy, folder_busy, models_root};
use crate::{ArtifactInstallCoordinator, GgufDownload, GgufModelSetup};

const LOCAL_JOB_LEASE: Duration = Duration::from_secs(30 * 60);
const PULL_PROGRESS_COALESCE: Duration = Duration::from_millis(150);
const ACTIVE_PAGE: u16 = 200;
const MODELS_FOLDER_SUBJECT: &str = "llm-models-folder";

/// A GGUF download's setup as stored with its job; two requests for one
/// install join only when theirs are equal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredGgufSetup {
    pub setup: dto::HfDownloadSetup,
    pub mtp_bundled: bool,
}

/// One pinned file of a stored GGUF install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredArtifact {
    pub repository: String,
    pub revision: String,
    pub path: String,
    pub local_segments: Vec<String>,
    pub byte_size: u64,
    pub sha256: Option<String>,
}

/// What a local model job works on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum LocalModelJobDetail {
    ModelDownload {
        repo: String,
        revision: String,
        file: String,
        mmproj_file: Option<String>,
        mtp_file: Option<String>,
        display_name: String,
        root: String,
        setup: Box<StoredGgufSetup>,
        install_id: String,
        artifacts: Vec<StoredArtifact>,
    },
    ModelPull {
        provider_account_id: String,
        model: String,
    },
    ModelsFolderMove {
        from: String,
        to: String,
        move_existing: bool,
    },
}

/// What a local model job produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum LocalModelJobResult {
    ModelInstalled {
        model_path: String,
        model_profile_id: Option<String>,
    },
    ModelPulled {
        model: String,
    },
    ModelsFolderMoved {
        path: String,
        moved_entries: u32,
        rewired_models: u32,
    },
}

fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}

fn encode<T: Serialize>(value: &T) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value).map_err(internal)
}

/// Records what a job produced; a job without a stored detail has nowhere
/// to keep it.
pub(crate) fn record_result(
    context: &ApiContext,
    job_id: JobId,
    result: &LocalModelJobResult,
) -> Result<(), ApiError> {
    context
        .backend()
        .database()
        .record_local_model_job_result(job_id, &encode(result)?)
        .map_err(internal)?;
    Ok(())
}

/// Why a local model job failed, beyond its error label: the words an
/// Ollama server used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum LocalModelJobFailure {
    OllamaServer { message: String },
}

const OLLAMA_OFFLINE: &str = "ollama-offline";
const OLLAMA_CREDENTIALS_UNAVAILABLE: &str = "ollama-credentials-unavailable";
const OLLAMA_CREDENTIALS_REFUSED: &str = "ollama-credentials-refused";
const OLLAMA_SERVER_ERROR: &str = "ollama-server-error";
const OLLAMA_INCOMPLETE: &str = "ollama-pull-incomplete";

/// The typed failure an Ollama pull's error label (and the stored server
/// words) name.
pub(crate) fn ollama_failure(
    label: &str,
    stored: Option<&LocalModelJobFailure>,
) -> Option<dto::OllamaFailure> {
    Some(match label {
        OLLAMA_OFFLINE => dto::OllamaFailure::Offline,
        OLLAMA_CREDENTIALS_UNAVAILABLE => dto::OllamaFailure::CredentialsUnavailable,
        OLLAMA_CREDENTIALS_REFUSED => dto::OllamaFailure::CredentialsRefused,
        OLLAMA_INCOMPLETE => dto::OllamaFailure::Incomplete,
        OLLAMA_SERVER_ERROR => dto::OllamaFailure::ServerError {
            message: match stored {
                Some(LocalModelJobFailure::OllamaServer { message }) => message.clone(),
                None => String::new(),
            },
        },
        _ => return None,
    })
}

/// The download center's view of a local model job: its detail, what it
/// produced and why it failed.
pub(crate) struct LocalJobView {
    pub detail: Option<dto::JobSubjectDetail>,
    pub result: Option<dto::JobResultDto>,
    pub failure: Option<LocalModelJobFailure>,
}

pub(crate) fn local_job_view(context: &ApiContext, job: &JobSnapshot) -> LocalJobView {
    let empty = LocalJobView {
        detail: None,
        result: None,
        failure: None,
    };
    if !matches!(
        job.kind,
        JobKind::ArtifactInstall | JobKind::ModelPull | JobKind::ModelsFolderMove
    ) {
        return empty;
    }
    let record = match context.backend().database().local_model_job(job.id) {
        Ok(Some(record)) => record,
        Ok(None) => return empty,
        Err(error) => {
            tracing::warn!(job_id = %job.id, %error, "a local model job's detail could not be read");
            return empty;
        }
    };
    let Ok(detail) = serde_json::from_value::<LocalModelJobDetail>(record.detail) else {
        return empty;
    };
    let result = record
        .result
        .and_then(|result| serde_json::from_value::<LocalModelJobResult>(result).ok())
        .map(|result| match result {
            LocalModelJobResult::ModelInstalled {
                model_path,
                model_profile_id,
            } => dto::JobResultDto::ModelInstalled {
                model_path,
                model_profile_id,
            },
            LocalModelJobResult::ModelPulled { model } => dto::JobResultDto::ModelPulled { model },
            LocalModelJobResult::ModelsFolderMoved {
                path,
                moved_entries,
                rewired_models,
            } => dto::JobResultDto::ModelsFolderMoved {
                path,
                moved_entries,
                rewired_models,
            },
        });
    let detail = match detail {
        LocalModelJobDetail::ModelDownload {
            repo,
            file,
            display_name,
            ..
        } => dto::JobSubjectDetail::ModelDownload {
            repo,
            file,
            display_name,
        },
        LocalModelJobDetail::ModelPull {
            provider_account_id,
            model,
        } => dto::JobSubjectDetail::ModelPull {
            provider_account_id,
            model,
        },
        LocalModelJobDetail::ModelsFolderMove { from, to, .. } => {
            dto::JobSubjectDetail::ModelsFolderMove { from, to }
        }
    };
    LocalJobView {
        detail: Some(detail),
        result,
        failure: record
            .failure
            .and_then(|failure| serde_json::from_value(failure).ok()),
    }
}

fn digest<T: Serialize>(request: &T) -> Result<String, ApiError> {
    let encoded = serde_json::to_vec(request).map_err(internal)?;
    Ok(blake3::hash(&encoded).to_hex().to_string())
}

fn operation_key(command: &str, client_operation_id: &str) -> Result<String, ApiError> {
    let id = client_operation_id.trim();
    if id.is_empty() {
        return Err(invalid_field(
            "client_operation_id",
            "client_operation_id is empty",
        ));
    }
    Ok(format!("{command}:{id}"))
}

fn reused_key() -> ApiError {
    api_error(
        ApiErrorCode::Conflict,
        "client_operation_id was already used for a different request",
    )
}

/// The job an earlier request with this key started; a different request
/// under the same key is a conflict.
fn replay(context: &ApiContext, key: &str, digest: &str) -> Result<Option<JobId>, ApiError> {
    match context
        .backend()
        .database()
        .local_model_operation(key)
        .map_err(internal)?
    {
        None => Ok(None),
        Some(operation) if operation.request_digest == digest => Ok(Some(operation.job_id)),
        Some(_) => Err(reused_key()),
    }
}

fn record_operation(
    context: &ApiContext,
    key: &str,
    digest: &str,
    job_id: JobId,
) -> Result<(), ApiError> {
    let operation = context
        .backend()
        .database()
        .record_local_model_operation(key, digest, job_id)
        .map_err(internal)?;
    if operation.request_digest != digest || operation.job_id != job_id {
        return Err(reused_key());
    }
    Ok(())
}

const ACTIVE_STATES: [JobState; 5] = [
    JobState::Queued,
    JobState::Claimed,
    JobState::Running,
    JobState::CancellationRequested,
    JobState::CleaningUp,
];

/// Every job of `kind` that has not ended, optionally of one subject.
fn active_jobs(
    context: &ApiContext,
    kind: JobKind,
    subject: Option<JobSubject>,
) -> Result<Vec<JobSnapshot>, ApiError> {
    let database = context.backend().database();
    let mut jobs = Vec::new();
    let mut cursor = None;
    loop {
        let page = database
            .list_jobs(&JobListFilter {
                kinds: vec![kind],
                states: ACTIVE_STATES.to_vec(),
                subject: subject
                    .as_ref()
                    .map(|subject| (subject.kind, subject.id.clone())),
                page: PageRequest {
                    cursor: cursor.take(),
                    limit: PageLimit::new(ACTIVE_PAGE),
                },
            })
            .map_err(IntoApiError::into_api_error)?;
        jobs.extend(page.items);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(jobs),
        }
    }
}

/// How many jobs of `subject` exist, so a new one gets a fresh key.
fn earlier_jobs(
    context: &ApiContext,
    kind: JobKind,
    subject: &JobSubject,
) -> Result<usize, ApiError> {
    let database = context.backend().database();
    let mut count = 0;
    let mut cursor = None;
    loop {
        let page = database
            .list_jobs(&JobListFilter {
                kinds: vec![kind],
                states: Vec::new(),
                subject: Some((subject.kind, subject.id.clone())),
                page: PageRequest {
                    cursor: cursor.take(),
                    limit: PageLimit::new(ACTIVE_PAGE),
                },
            })
            .map_err(IntoApiError::into_api_error)?;
        count += page.items.len();
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(count),
        }
    }
}

fn folder_move_active(context: &ApiContext) -> Result<(), ApiError> {
    match active_jobs(context, JobKind::ModelsFolderMove, None)?.first() {
        Some(job) => Err(busy(dto::LocalModelsBusyReason::FolderMoveActive {
            job_id: job.id.to_string(),
        })),
        None => Ok(()),
    }
}

fn gguf_model_setup(setup: &dto::HfDownloadSetup, mtp_bundled: bool) -> GgufModelSetup {
    GgufModelSetup {
        display_name: setup.display_name.clone(),
        context_length: setup.context_length,
        kv_type: setup.kv_type.clone(),
        offload_kqv: setup.offload_kqv,
        gpu_layers: setup.gpu_layers,
        model_offload: model_offload(setup.model_offload),
        mtp_bundled,
    }
}

/// Hands a queued GGUF install an earlier process admitted back to the
/// runner from its stored detail; `false` when the job is no GGUF install
/// with one.
pub(crate) fn resume_gguf_install(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<bool, ApiError> {
    let record = context
        .backend()
        .database()
        .local_model_job(job.id)
        .map_err(internal)?;
    let Some(LocalModelJobDetail::ModelDownload {
        repo,
        file,
        mmproj_file,
        mtp_file,
        root,
        setup,
        install_id,
        artifacts,
        ..
    }) = record.and_then(|record| serde_json::from_value(record.detail).ok())
    else {
        return Ok(false);
    };
    if artifacts.is_empty() {
        return Ok(false);
    }
    let root = PathBuf::from(root);
    let plan = crate::ArtifactInstallPlan {
        install_id,
        root: root.clone(),
        artifacts: artifacts
            .into_iter()
            .map(|stored| {
                let source = crate::ArtifactSource::HuggingFace {
                    repository: stored.repository,
                    revision: stored.revision,
                    path: stored.path,
                };
                crate::PlannedArtifact {
                    artifact: lettuce_model_hub::PinnedArtifact {
                        source_identity: source.identity(),
                        local_segments: stored.local_segments,
                        byte_size: stored.byte_size,
                        sha256: stored.sha256,
                    },
                    source,
                }
            })
            .collect(),
    };
    let finish = InstallFinish::Gguf {
        root,
        download: GgufDownload {
            model_id: repo,
            model_file: file,
            mmproj_file,
            mtp_file,
        },
        create_model: setup
            .setup
            .create_model
            .then(|| gguf_model_setup(&setup.setup, setup.mtp_bundled)),
    };
    context.jobs().put_install(
        job.id,
        InstallWork::Artifact {
            plan,
            finish: Box::new(finish),
        },
    );
    context.jobs().wake();
    Ok(true)
}

/// Settles what an earlier process left of local model jobs: a folder move
/// never runs unattended, so a queued one is cancelled (a running one was
/// interrupted by restart recovery), and every interrupted move's copies
/// are removed or, when it had committed, its originals. Returns the moves
/// it cancelled.
pub(crate) fn recover_local_model_jobs(context: &ApiContext) -> Result<Vec<JobId>, ApiError> {
    let database = context.backend().database();
    let mut cancelled = Vec::new();
    for job in active_jobs(context, JobKind::ModelsFolderMove, None)? {
        if job.claim.is_some() {
            continue;
        }
        let at = context.now().max(job.updated_at);
        let requested = if job.state == JobState::Queued {
            database
                .append_and_transition(JobMutation::RequestCancellation {
                    id: job.id,
                    reason: CancellationReason::Recovery,
                    at,
                })
                .map_err(IntoApiError::into_api_error)?
        } else {
            job.clone()
        };
        if requested.state == JobState::CancellationRequested {
            database
                .append_and_transition(JobMutation::FinishQueuedCancellation {
                    id: job.id,
                    at: at.max(requested.updated_at),
                })
                .map_err(IntoApiError::into_api_error)?;
            cancelled.push(job.id);
        }
    }
    let Some(app_folder) = context.app_folder() else {
        return Ok(cancelled);
    };
    for job in all_jobs(context, JobKind::ModelsFolderMove)? {
        let detail = database
            .local_model_job(job.id)
            .map_err(internal)?
            .and_then(|record| serde_json::from_value::<LocalModelJobDetail>(record.detail).ok());
        if let Some(LocalModelJobDetail::ModelsFolderMove {
            to,
            move_existing: true,
            ..
        }) = detail
            && let Err(error) =
                crate::recover_models_folder_move(database, app_folder, Path::new(&to))
        {
            tracing::warn!(job_id = %job.id, %error, "an interrupted models folder move could not be cleaned up");
        }
    }
    Ok(cancelled)
}

/// Every job of `kind`, whatever its state.
fn all_jobs(context: &ApiContext, kind: JobKind) -> Result<Vec<JobSnapshot>, ApiError> {
    let database = context.backend().database();
    let mut jobs = Vec::new();
    let mut cursor = None;
    loop {
        let page = database
            .list_jobs(&JobListFilter {
                kinds: vec![kind],
                states: Vec::new(),
                subject: None,
                page: PageRequest {
                    cursor: cursor.take(),
                    limit: PageLimit::new(ACTIVE_PAGE),
                },
            })
            .map_err(IntoApiError::into_api_error)?;
        jobs.extend(page.items);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(jobs),
        }
    }
}

const fn model_offload(offload: Option<dto::HfModelOffload>) -> lettuce_model_hub::ModelOffload {
    match offload {
        Some(dto::HfModelOffload::Cpu) => lettuce_model_hub::ModelOffload::Cpu,
        Some(dto::HfModelOffload::Gpu) => lettuce_model_hub::ModelOffload::Gpu,
        Some(dto::HfModelOffload::Mixed) => lettuce_model_hub::ModelOffload::Mixed,
        Some(dto::HfModelOffload::Auto) | None => lettuce_model_hub::ModelOffload::Auto,
    }
}

fn optional_file(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Serialize)]
struct DownloadDigest<'a> {
    repo: &'a str,
    revision: Option<&'a str>,
    file: &'a str,
    mmproj_file: Option<&'a str>,
    mtp_file: Option<&'a str>,
    mtp_bundled: bool,
    setup: &'a dto::HfDownloadSetup,
}

/// Admits a GGUF download (see `hf_download`).
pub(crate) async fn admit_gguf_download(
    context: &ApiContext,
    request: dto::HfDownloadRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let repo = request.repo.trim().to_owned();
    if repo.is_empty() {
        return Err(invalid_field("repo", "repo is empty"));
    }
    let file = request.file.trim().to_owned();
    if file.is_empty() {
        return Err(invalid_field("file", "file is empty"));
    }
    if let Some(kv_type) = request.setup.kv_type.as_deref()
        && serde_json::from_value::<lettuce_models::LlamaKvType>(serde_json::Value::String(
            kv_type.to_owned(),
        ))
        .is_err()
    {
        return Err(invalid_field(
            "setup.kv_type",
            "setup.kv_type is not a llama.cpp KV type",
        ));
    }
    let key = operation_key("hf_download", &request.client_operation_id)?;
    let revision = optional_file(request.revision.as_ref());
    let mmproj_file = optional_file(request.mmproj_file.as_ref());
    let mtp_file = optional_file(request.mtp_file.as_ref());
    let digest = digest(&DownloadDigest {
        repo: &repo,
        revision: revision.as_deref(),
        file: &file,
        mmproj_file: mmproj_file.as_deref(),
        mtp_file: mtp_file.as_deref(),
        mtp_bundled: request.mtp_bundled,
        setup: &request.setup,
    })?;
    let (replayed_key, replayed_digest) = (key.clone(), digest.clone());
    let early = context
        .blocking(move |context| {
            if let Some(job_id) = replay(context, &replayed_key, &replayed_digest)? {
                return Ok(Err(job_id));
            }
            folder_move_active(context)?;
            models_root(context).map(Ok)
        })
        .await?;
    let root = match early {
        Ok(root) => root,
        Err(job_id) => {
            return Ok(dto::JobAccepted {
                job_id: job_id.to_string(),
            });
        }
    };
    let download = GgufDownload {
        model_id: repo.clone(),
        model_file: file.clone(),
        mmproj_file: mmproj_file.clone(),
        mtp_file: mtp_file.clone(),
    };
    let plan = context
        .local_models()
        .browser(context)?
        .gguf_install_plan(
            context.secret_store().as_ref(),
            &root,
            &download,
            revision.as_deref(),
        )
        .await
        .map_err(hf_error)?;
    let pinned = plan
        .artifacts
        .iter()
        .find_map(|planned| match &planned.source {
            crate::ArtifactSource::HuggingFace { revision, .. } => Some(revision.clone()),
            crate::ArtifactSource::Https { .. } => None,
        })
        .unwrap_or_default();
    let setup = request.setup;
    let model_setup = gguf_model_setup(&setup, request.mtp_bundled);
    let artifacts = plan
        .artifacts
        .iter()
        .filter_map(|planned| match &planned.source {
            crate::ArtifactSource::HuggingFace {
                repository,
                revision,
                path,
            } => Some(StoredArtifact {
                repository: repository.clone(),
                revision: revision.clone(),
                path: path.clone(),
                local_segments: planned.artifact.local_segments.clone(),
                byte_size: planned.artifact.byte_size,
                sha256: planned.artifact.sha256.clone(),
            }),
            crate::ArtifactSource::Https { .. } => None,
        })
        .collect();
    let detail = encode(&LocalModelJobDetail::ModelDownload {
        install_id: plan.install_id.clone(),
        artifacts,
        repo,
        revision: pinned,
        file,
        mmproj_file,
        mtp_file,
        display_name: setup
            .display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| download.default_display_name()),
        root: root.to_string_lossy().into_owned(),
        setup: Box::new(StoredGgufSetup {
            setup: setup.clone(),
            mtp_bundled: request.mtp_bundled,
        }),
    })?;
    let finish = InstallFinish::Gguf {
        root,
        download,
        create_model: setup.create_model.then_some(model_setup),
    };
    let job_id = context
        .blocking(move |context| {
            if let Some(job_id) = replay(context, &key, &digest)? {
                return Ok(job_id);
            }
            folder_move_active(context)?;
            let job = ArtifactInstallCoordinator::new(context.backend().database())
                .admit(&plan)
                .map_err(internal)?
                .job;
            let stored = context
                .backend()
                .database()
                .record_local_model_job(job.id, &detail)
                .map_err(internal)?;
            if stored != detail {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "this file is already being installed with another setup",
                ));
            }
            if !job.state.is_terminal() {
                context.jobs().put_install(
                    job.id,
                    InstallWork::Artifact {
                        plan,
                        finish: Box::new(finish),
                    },
                );
            }
            record_operation(context, &key, &digest, job.id)?;
            Ok(job.id)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

fn stable_uuid(parts: &[&str]) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, parts.join("\0").as_bytes())
}

/// Creates a job of `subject` unless one has not ended yet, which is
/// returned instead.
fn admit_job(
    context: &ApiContext,
    kind: JobKind,
    subject: JobSubject,
    input: Uuid,
    resources: Vec<ResourceClass>,
) -> Result<JobSnapshot, ApiError> {
    if let Some(active) = active_jobs(context, kind, Some(subject.clone()))?
        .into_iter()
        .next()
    {
        return Ok(active);
    }
    let earlier = earlier_jobs(context, kind, &subject)?;
    let key = IdempotencyKey::new(format!("local-model-{}-{earlier}", subject.id.as_str()))
        .map_err(internal)?;
    context
        .backend()
        .database()
        .create_or_get(
            JobSpec::new(
                kind,
                subject,
                OutcomeRef::Request(RequestId::from_uuid(input)),
            )
            .with_idempotency_key(key)
            .with_priority(JobPriority::Interactive)
            .with_resources(resources)
            .with_policies(
                RecoveryPolicy::MarkInterrupted,
                CancellationPolicy::Cooperative,
            ),
        )
        .map(|created| created.job)
        .map_err(IntoApiError::into_api_error)
}

/// Admits an Ollama pull (see `ollama_pull`).
pub(crate) async fn admit_model_pull(
    context: &ApiContext,
    request: dto::OllamaPullRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let model = request.model.trim().to_owned();
    if model.is_empty() {
        return Err(invalid_field("model", "model is empty"));
    }
    let key = operation_key("ollama_pull", &request.client_operation_id)?;
    let account = crate::api::ollama::ollama_account(context, &request.provider_account_id).await?;
    let account_id = account.id.to_string();
    let digest = digest(&(&account_id, &model))?;
    let job_id = context
        .blocking(move |context| {
            if let Some(job_id) = replay(context, &key, &digest)? {
                return Ok(job_id);
            }
            let uuid = stable_uuid(&["ollama-pull", &account_id, &model]);
            let subject = JobSubject::from_uuid(SubjectKind::ProviderModel, uuid);
            let job = admit_job(
                context,
                JobKind::ModelPull,
                subject,
                uuid,
                vec![ResourceClass::Network],
            )?;
            context
                .backend()
                .database()
                .record_local_model_job(
                    job.id,
                    &encode(&LocalModelJobDetail::ModelPull {
                        provider_account_id: account_id,
                        model,
                    })?,
                )
                .map_err(internal)?;
            record_operation(context, &key, &digest, job.id)?;
            Ok(job.id)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

/// Admits a models folder switch (see `local_models_dir_set`).
pub(crate) async fn admit_models_folder_move(
    context: &ApiContext,
    request: dto::LocalModelsDirSetRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let path = request.path.trim().to_owned();
    if path.is_empty() {
        return Err(invalid_field("path", "path is empty"));
    }
    let key = operation_key("local_models_dir_set", &request.client_operation_id)?;
    let move_existing = request.move_existing;
    let digest = digest(&(&path, move_existing))?;
    let job_id = context
        .blocking(move |context| {
            if let Some(job_id) = replay(context, &key, &digest)? {
                return Ok(job_id);
            }
            folder_move_active(context)?;
            let from = models_root(context)?;
            let to = PathBuf::from(&path);
            if move_existing && !crate::models::gguf_library::paths_equal(&from, &to) {
                if let Some(reason) = folder_busy(context, &from) {
                    return Err(busy(reason));
                }
                crate::check_folder_move(&from, &to).map_err(|error| match error {
                    crate::FolderMoveError::DestinationNotEmpty(_) => {
                        api_error(ApiErrorCode::Conflict, error.to_string())
                    }
                    error => invalid_field("path", error.to_string()),
                })?;
            }
            let uuid = stable_uuid(&["models-folder-move", &key]);
            let subject = JobSubject::new(SubjectKind::Maintenance, MODELS_FOLDER_SUBJECT)
                .map_err(internal)?;
            let job = admit_job(
                context,
                JobKind::ModelsFolderMove,
                subject,
                uuid,
                vec![ResourceClass::DiskRead, ResourceClass::DiskWrite],
            )?;
            context
                .backend()
                .database()
                .record_local_model_job(
                    job.id,
                    &encode(&LocalModelJobDetail::ModelsFolderMove {
                        from: from.to_string_lossy().into_owned(),
                        to: path,
                        move_existing,
                    })?,
                )
                .map_err(internal)?;
            record_operation(context, &key, &digest, job.id)?;
            Ok(job.id)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

fn job_error(code: JobErrorCode, retryable: bool, label: &'static str) -> JobError {
    JobError::new(code, retryable, label).expect("constant error label")
}

/// A job claimed and started, with its stored detail.
struct StartedJob {
    job_id: JobId,
    claim: ClaimRef,
    detail: LocalModelJobDetail,
    cancellation: CancellationToken,
}

enum Claimed {
    Started(Box<StartedJob>),
    Settled,
}

/// Claims and starts a local model job; a job whose detail is missing or
/// unreadable can never run and fails.
fn claim_local(
    context: &ApiContext,
    job: &JobSnapshot,
    worker_id: WorkerId,
) -> Result<Claimed, ApiError> {
    let database = context.backend().database();
    let detail = database
        .local_model_job(job.id)
        .map_err(internal)?
        .and_then(|record| serde_json::from_value::<LocalModelJobDetail>(record.detail).ok());
    let at = context.now().max(job.updated_at);
    let Some(claim) = database
        .claim(
            job.id,
            worker_id,
            at,
            LOCAL_JOB_LEASE,
            &lettuce_jobs::ResourceAvailability::all(),
        )
        .map_err(IntoApiError::into_api_error)?
    else {
        return Ok(Claimed::Settled);
    };
    let started = database.append_and_transition(JobMutation::Start {
        claim: claim.claim.clone(),
        at,
    });
    if started.is_err() {
        let current = database.get(job.id).map_err(IntoApiError::into_api_error)?;
        if current.is_some_and(|job| job.state == JobState::CancellationRequested) {
            crate::models::artifact_install::finish_claimed_cancellation(
                database,
                &claim.claim,
                at,
            )
            .map_err(IntoApiError::into_api_error)?;
            return Ok(Claimed::Settled);
        }
        started.map_err(IntoApiError::into_api_error)?;
    }
    let Some(detail) = detail else {
        database
            .append_and_transition(JobMutation::Fail {
                claim: claim.claim,
                error: job_error(JobErrorCode::InvalidInput, false, "local-model-job-unknown"),
                at,
            })
            .map_err(IntoApiError::into_api_error)?;
        return Ok(Claimed::Settled);
    };
    database
        .append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new(
                match detail {
                    LocalModelJobDetail::ModelsFolderMove { .. } => "move",
                    _ => "download",
                },
                false,
            )
            .map_err(internal)?,
            at,
        })
        .map_err(IntoApiError::into_api_error)?;
    Ok(Claimed::Started(Box::new(StartedJob {
        job_id: job.id,
        claim: claim.claim,
        detail,
        cancellation: CancellationToken::new(),
    })))
}

enum Settlement {
    Succeeded(Option<LocalModelJobResult>),
    Failed(JobError),
    FailedWith(JobError, LocalModelJobFailure),
    Cancelled,
}

/// Ends a started job: success records what it produced, a cancellation
/// (by the user or by shutdown) cleans up first.
fn settle(
    context: &ApiContext,
    started: &StartedJob,
    settlement: Settlement,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let job = database
        .get(started.job_id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "job was not found"))?;
    if job.state.is_terminal() {
        return Ok(());
    }
    let at = context.now().max(job.updated_at);
    let claim = started.claim.clone();
    match settlement {
        Settlement::Succeeded(result) => {
            if let Some(result) = &result
                && let Err(error) = record_result(context, started.job_id, result)
            {
                tracing::warn!(job_id = %started.job_id, message = %error.message, "a local model job's result could not be recorded");
                database
                    .append_and_transition(JobMutation::Fail {
                        claim,
                        error: job_error(
                            JobErrorCode::StorageFailure,
                            true,
                            "local-model-result-unrecorded",
                        ),
                        at,
                    })
                    .map_err(IntoApiError::into_api_error)?;
                return Ok(());
            }
            database
                .append_and_transition(JobMutation::Succeed {
                    claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::Request(RequestId::from_uuid(stable_uuid(&[
                            "local-model-job",
                            &started.job_id.to_string(),
                        ]))),
                    },
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
        }
        Settlement::Failed(error) => {
            database
                .append_and_transition(JobMutation::Fail { claim, error, at })
                .map_err(IntoApiError::into_api_error)?;
        }
        Settlement::FailedWith(error, failure) => {
            if let Err(error) =
                database.record_local_model_job_failure(started.job_id, &encode(&failure)?)
            {
                tracing::warn!(job_id = %started.job_id, %error, "a local model job's failure could not be recorded");
            }
            database
                .append_and_transition(JobMutation::Fail { claim, error, at })
                .map_err(IntoApiError::into_api_error)?;
        }
        Settlement::Cancelled => {
            if job.state != JobState::CancellationRequested {
                database
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: started.job_id,
                        reason: CancellationReason::Shutdown,
                        at,
                    })
                    .map_err(IntoApiError::into_api_error)?;
            }
            crate::models::artifact_install::finish_claimed_cancellation(database, &claim, at)
                .map_err(IntoApiError::into_api_error)?;
        }
    }
    Ok(())
}

/// Runs Ollama pulls, each in its own lane so pulls run next to each other
/// and next to downloads, as legacy ran them.
#[derive(Debug, Default)]
pub struct ModelPullHandler;

#[async_trait]
impl JobHandler for ModelPullHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::ModelPull]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(format!("ollama-pull:{}", job.id)))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        let claimed = context
            .blocking(move |context| claim_local(context, &job, worker_id))
            .await?;
        Ok(match claimed {
            Claimed::Started(started) => Some(Box::new(ClaimedPull(*started))),
            Claimed::Settled => None,
        })
    }
}

struct ClaimedPull(StartedJob);

#[async_trait]
impl ClaimedJob for ClaimedPull {
    fn cancellation(&self) -> CancellationToken {
        self.0.cancellation.clone()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let started = Arc::new(self.0);
        let settlement = pull(&context, &started).await;
        context
            .blocking(move |context| settle(context, &started, settlement))
            .await
    }
}

fn pull_progress(
    context: &ApiContext,
    started: &StartedJob,
    progress: &lettuce_providers::OllamaPullProgress,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let at = context.now();
    database
        .heartbeat(&started.claim, at, LOCAL_JOB_LEASE)
        .map_err(IntoApiError::into_api_error)?;
    let total = (progress.total > 0).then_some(progress.total);
    database
        .append_and_transition(JobMutation::Progress {
            claim: started.claim.clone(),
            progress: ProgressSnapshot {
                bytes: Some(
                    BytesProgress::new(
                        total.map_or(progress.completed, |total| progress.completed.min(total)),
                        total,
                    )
                    .map_err(internal)?,
                ),
                ..ProgressSnapshot::default()
            },
            at,
        })
        .map_err(IntoApiError::into_api_error)?;
    Ok(())
}

/// Pulls the model, writing its progress at most every 150 ms; cancelling
/// the job drops the stream, which stops the pull.
async fn pull(context: &ApiContext, started: &Arc<StartedJob>) -> Settlement {
    let LocalModelJobDetail::ModelPull {
        provider_account_id,
        model,
    } = &started.detail
    else {
        return Settlement::Failed(job_error(
            JobErrorCode::InvalidInput,
            false,
            "local-model-job-unknown",
        ));
    };
    if started.cancellation.is_cancelled() {
        return Settlement::Cancelled;
    }
    let account = match crate::api::ollama::ollama_account(context, provider_account_id).await {
        Ok(account) => account,
        Err(error) => {
            tracing::warn!(job_id = %started.job_id, message = %error.message, "the Ollama account of a pull is unavailable");
            return Settlement::Failed(job_error(
                JobErrorCode::InvalidInput,
                false,
                "ollama-account-unavailable",
            ));
        }
    };
    let providers = match crate::api::ollama::remote_providers(context) {
        Ok(providers) => providers,
        Err(_) => {
            return Settlement::Failed(job_error(
                JobErrorCode::ResourceUnavailable,
                true,
                "ollama-client-unavailable",
            ));
        }
    };
    let latest = Arc::new(Mutex::new(None));
    let changed = Arc::new(tokio::sync::Notify::new());
    let (reported, signal) = (Arc::clone(&latest), Arc::clone(&changed));
    let mut on_progress = move |progress: lettuce_providers::OllamaPullProgress| {
        *reported
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(progress);
        signal.notify_one();
    };
    let pulling = providers.ollama_pull(&account, model, &mut on_progress);
    tokio::pin!(pulling);
    let result = loop {
        tokio::select! {
            result = &mut pulling => break result,
            () = started.cancellation.cancelled() => return Settlement::Cancelled,
            () = changed.notified() => {
                let progress = latest
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                if let Some(progress) = progress {
                    let job = Arc::clone(started);
                    if let Err(error) = context
                        .blocking(move |context| pull_progress(context, &job, &progress))
                        .await
                    {
                        tracing::warn!(job_id = %started.job_id, message = %error.message, "pull progress could not be recorded");
                    }
                }
                tokio::select! {
                    () = tokio::time::sleep(PULL_PROGRESS_COALESCE) => {}
                    () = started.cancellation.cancelled() => return Settlement::Cancelled,
                }
            }
        }
    };
    match result {
        Ok(()) => Settlement::Succeeded(Some(LocalModelJobResult::ModelPulled {
            model: model.clone(),
        })),
        Err(error) => {
            tracing::warn!(job_id = %started.job_id, %error, "an Ollama pull failed");
            pull_failure(error)
        }
    }
}

fn pull_failure(error: lettuce_providers::OllamaHubError) -> Settlement {
    use lettuce_providers::OllamaHubError;
    match error {
        OllamaHubError::Unreachable(_) => Settlement::Failed(job_error(
            JobErrorCode::ResourceUnavailable,
            true,
            OLLAMA_OFFLINE,
        )),
        OllamaHubError::Credentials => Settlement::Failed(job_error(
            JobErrorCode::Authentication,
            false,
            OLLAMA_CREDENTIALS_UNAVAILABLE,
        )),
        OllamaHubError::CredentialsRefused { .. } => Settlement::Failed(job_error(
            JobErrorCode::Authentication,
            false,
            OLLAMA_CREDENTIALS_REFUSED,
        )),
        OllamaHubError::Server(message) => Settlement::FailedWith(
            job_error(JobErrorCode::WorkerFailed, false, OLLAMA_SERVER_ERROR),
            LocalModelJobFailure::OllamaServer { message },
        ),
        OllamaHubError::Incomplete => Settlement::Failed(job_error(
            JobErrorCode::WorkerFailed,
            true,
            OLLAMA_INCOMPLETE,
        )),
        OllamaHubError::NotOllama | OllamaHubError::EmptyReference => Settlement::Failed(
            job_error(JobErrorCode::InvalidInput, false, "ollama-pull-invalid"),
        ),
        OllamaHubError::Message(_) => Settlement::Failed(job_error(
            JobErrorCode::ResourceUnavailable,
            false,
            "ollama-pull-failed",
        )),
    }
}

fn folder_move_failure(error: &crate::FolderMoveError) -> Settlement {
    use crate::FolderMoveError;
    let (code, retryable, label) = match error {
        FolderMoveError::Cancelled => return Settlement::Cancelled,
        FolderMoveError::EmptyPath => (
            JobErrorCode::InvalidInput,
            false,
            "models-folder-path-empty",
        ),
        FolderMoveError::DestinationInsideSource => (
            JobErrorCode::InvalidInput,
            false,
            "models-folder-destination-inside-source",
        ),
        FolderMoveError::DestinationNotEmpty(_) => (
            JobErrorCode::InvalidInput,
            false,
            "models-folder-destination-not-empty",
        ),
        FolderMoveError::Copy(_) => (
            JobErrorCode::StorageFailure,
            true,
            "models-folder-copy-failed",
        ),
        FolderMoveError::Storage(_) => (
            JobErrorCode::StorageFailure,
            true,
            "models-folder-paths-unsaved",
        ),
    };
    tracing::warn!(%error, "the models folder could not be moved");
    Settlement::Failed(job_error(code, retryable, label))
}

/// Runs models folder moves in the download lane, so no download writes
/// into the folder while it moves.
#[derive(Debug, Default)]
pub struct ModelsFolderMoveHandler;

#[async_trait]
impl JobHandler for ModelsFolderMoveHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::ModelsFolderMove]
    }

    fn lane(&self, _context: &ApiContext, _job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(DOWNLOAD_LANE.to_owned()))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        let claimed = context
            .blocking(move |context| claim_local(context, &job, worker_id))
            .await?;
        Ok(match claimed {
            Claimed::Started(started) => Some(Box::new(ClaimedFolderMove(*started))),
            Claimed::Settled => None,
        })
    }
}

struct ClaimedFolderMove(StartedJob);

#[async_trait]
impl ClaimedJob for ClaimedFolderMove {
    fn cancellation(&self) -> CancellationToken {
        self.0.cancellation.clone()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let started = Arc::new(self.0);
        let settlement = move_folder(&context, &started).await;
        context
            .blocking(move |context| settle(context, &started, settlement))
            .await
    }
}

/// Moves the folder once nothing uses it. A cancellation (the user's or
/// shutdown's) stops the copy between files and chunks and removes the
/// copies; one that arrives after the paths were saved lets the move end.
async fn move_folder(context: &ApiContext, started: &Arc<StartedJob>) -> Settlement {
    let LocalModelJobDetail::ModelsFolderMove {
        from,
        to,
        move_existing,
    } = started.detail.clone()
    else {
        return Settlement::Failed(job_error(
            JobErrorCode::InvalidInput,
            false,
            "local-model-job-unknown",
        ));
    };
    if started.cancellation.is_cancelled() {
        return Settlement::Cancelled;
    }
    let cancellation = started.cancellation.clone();
    let moving = context.blocking(move |context| {
        if move_existing && folder_busy(context, Path::new(&from)).is_some() {
            return Ok(Settlement::Failed(job_error(
                JobErrorCode::ResourceUnavailable,
                true,
                "models-folder-busy",
            )));
        }
        let app_folder = crate::api::local_models::app_folder(context)?.to_path_buf();
        Ok(
            match crate::set_llm_models_dir(
                context.backend().database(),
                &app_folder,
                &to,
                move_existing,
                context.now(),
                &|| cancellation.is_cancelled(),
            ) {
                Ok(change) => Settlement::Succeeded(Some(LocalModelJobResult::ModelsFolderMoved {
                    path: change.path.to_string_lossy().into_owned(),
                    moved_entries: change.moved_entries,
                    rewired_models: change.rewired_models,
                })),
                Err(error) => folder_move_failure(&error),
            },
        )
    });
    tokio::pin!(moving);
    let renew_every = LOCAL_JOB_LEASE / 3;
    let moved = loop {
        tokio::select! {
            moved = &mut moving => break moved,
            () = tokio::time::sleep(renew_every) => {
                let job = Arc::clone(started);
                let renewed = context
                    .blocking(move |context| {
                        context
                            .backend()
                            .database()
                            .heartbeat(&job.claim, context.now(), LOCAL_JOB_LEASE)
                            .map_err(IntoApiError::into_api_error)
                    })
                    .await;
                if let Err(error) = renewed {
                    tracing::warn!(job_id = %started.job_id, message = %error.message, "could not renew the folder move lease");
                }
            }
        }
    };
    match moved {
        Ok(settlement) => settlement,
        Err(error) => {
            tracing::warn!(job_id = %started.job_id, message = %error.message, "the models folder move could not run");
            Settlement::Failed(job_error(
                JobErrorCode::StorageFailure,
                true,
                "models-folder-move-failed",
            ))
        }
    }
}
