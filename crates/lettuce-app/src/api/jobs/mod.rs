//! The jobs API: listing, reading, cancelling and watching jobs, the change
//! feed behind `ApiEvent::JobUpdated`, and the runner for every job kind the
//! conversation generation worker does not run.

mod feed;
mod install;
mod local;
mod runner;
mod state;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{
    CancellationReason, JobCatalog, JobErrorCode, JobKind, JobListFilter, JobMutation, JobOutcome,
    JobSnapshot, JobState, JobStore, OutcomeRef, SubjectId, SubjectKind,
};
use lettuce_types::{JobId, PageRequest};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::events::JobEventSink;
use super::mapping;

pub(crate) use feed::JobFeed;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use install::CatalogVariant;
pub use install::admit_install;
pub(crate) use install::recover_queued_installs;
pub use install::{
    ArtifactInstallHandler, InstallFinish, InstallSources, InstallWork, NetworkInstallSources,
};
pub use local::{ModelPullHandler, ModelsFolderMoveHandler};
pub(crate) use local::{admit_gguf_download, admit_model_pull, admit_models_folder_move};
pub use runner::{ClaimedJob, JobHandler, JobHandlers, JobLane, JobProgressSink, JobRunner};
pub(crate) use state::JobHostState;

pub async fn jobs_list(
    context: &ApiContext,
    request: dto::JobsListRequest,
) -> Result<dto::JobPage, ApiError> {
    let subject = request
        .subject
        .map(|subject| {
            SubjectId::new(subject.id)
                .map(|id| (subject_kind(subject.kind), id))
                .map_err(|_| invalid_field("subject", "subject id is not valid"))
        })
        .transpose()?;
    let filter = JobListFilter {
        kinds: request
            .kinds
            .unwrap_or_default()
            .into_iter()
            .map(job_kind)
            .collect(),
        states: request
            .states
            .unwrap_or_default()
            .into_iter()
            .map(job_state)
            .collect(),
        subject,
        page: PageRequest {
            cursor: request.cursor,
            limit: mapping::page_limit(request.limit),
        },
    };
    context
        .blocking(move |context| {
            let page =
                context
                    .backend()
                    .database()
                    .list_jobs(&filter)
                    .map_err(|error| match error {
                        lettuce_jobs::StoreError::InvalidCursor => {
                            invalid_field("cursor", "cursor is not valid")
                        }
                        error => error.into_api_error(),
                    })?;
            Ok(dto::JobPage {
                items: page
                    .items
                    .iter()
                    .map(|job| job_view(context, job))
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
        .await
}

pub async fn job_get(
    context: &ApiContext,
    request: dto::JobGetRequest,
) -> Result<dto::JobView, ApiError> {
    let job_id: JobId = parse_id(&request.job_id, "job_id")?;
    context
        .blocking(move |context| {
            let job = load(context, job_id)?;
            Ok(job_view(context, &job))
        })
        .await
}

/// Asks a job to stop. A queued job is cancelled at once; a running one is
/// signalled and settles through its runner. Conversation turns are
/// cancelled with `generation_cancel`, and an ended job is left alone.
pub async fn job_cancel(
    context: &ApiContext,
    request: dto::JobCancelRequest,
) -> Result<(), ApiError> {
    let job_id: JobId = parse_id(&request.job_id, "job_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let job = load(context, job_id)?;
            if job.kind == JobKind::ConversationGeneration {
                return Err(api_error(
                    ApiErrorCode::Unsupported,
                    "conversation turns are cancelled with generation_cancel",
                ));
            }
            if job.state.is_terminal() {
                return Ok(());
            }
            let at = context.now().max(job.updated_at);
            let requested = database
                .append_and_transition(JobMutation::RequestCancellation {
                    id: job_id,
                    reason: CancellationReason::User,
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
            context.jobs().forget_install(job_id);
            if !context.jobs().cancel_running(job_id)
                && requested.state == JobState::CancellationRequested
                && requested.claim.is_none()
            {
                database
                    .append_and_transition(JobMutation::FinishQueuedCancellation {
                        id: job_id,
                        at: at.max(requested.updated_at),
                    })
                    .map_err(IntoApiError::into_api_error)?;
            }
            Ok(())
        })
        .await?;
    context.jobs().wake();
    Ok(())
}

/// Attaches `sink` to a job: it gets the job's current state first, then
/// every change, and the stream ends with the job's terminal event. An
/// ended job gets only that event.
pub async fn job_watch(
    context: &ApiContext,
    request: dto::JobWatchRequest,
    sink: Arc<dyn JobEventSink>,
) -> Result<dto::JobView, ApiError> {
    let job_id: JobId = parse_id(&request.job_id, "job_id")?;
    context
        .blocking(move |context| {
            context.jobs().watch(job_id, sink, || {
                let job = load(context, job_id)?;
                let view = job_view(context, &job);
                let (event, terminal) = job_event(&job, view.clone());
                Ok((view, event, terminal))
            })
        })
        .await
}

fn load(context: &ApiContext, job_id: JobId) -> Result<JobSnapshot, ApiError> {
    context
        .backend()
        .database()
        .get(job_id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "job was not found"))
}

/// The watch event a job's state is, and whether it ends the stream.
pub(crate) fn job_event(job: &JobSnapshot, view: dto::JobView) -> (dto::JobEvent, bool) {
    match job.state {
        JobState::Succeeded => (dto::JobEvent::Completed { job: view }, true),
        JobState::Cancelled => (dto::JobEvent::Cancelled { job: view }, true),
        JobState::Failed | JobState::Interrupted => (dto::JobEvent::Failed { job: view }, true),
        JobState::Queued
        | JobState::Claimed
        | JobState::Running
        | JobState::CancellationRequested
        | JobState::CleaningUp => (dto::JobEvent::Progress { job: view }, false),
    }
}

pub(crate) fn job_view(context: &ApiContext, job: &JobSnapshot) -> dto::JobView {
    let local = local::local_job_view(context, job);
    dto::JobView {
        id: job.id.to_string(),
        kind: job_kind_dto(job.kind),
        subject: dto::JobSubjectDto {
            kind: subject_kind_dto(job.subject.kind),
            id: job.subject.id.to_string(),
        },
        subject_detail: local.detail,
        state: job_state_dto(job.state),
        progress: progress(job),
        created_at: job.created_at.get(),
        updated_at: job.updated_at.get(),
        failure: job.error.as_ref().map(|error| dto::JobFailureDto {
            code: failure_code(error.code),
            retryable: error.retryable,
            model: (error.code == JobErrorCode::CapabilityUnavailable
                && error.message.as_str() == crate::EMBEDDING_UNAVAILABLE_JOB_ERROR)
                .then_some(dto::RequiredModel::Embedding),
            hugging_face: (job.kind == JobKind::ArtifactInstall)
                .then(|| {
                    crate::hf_failure_of_job_error(
                        error.message.as_str(),
                        local.repo.as_deref().unwrap_or_default(),
                    )
                })
                .flatten()
                .map(|failure| super::error::hf_failure(&failure)),
        }),
        result: local.result.or_else(|| {
            job.outcome.as_ref().and_then(|outcome| {
                let (JobOutcome::Success { result_ref } | JobOutcome::Partial { result_ref, .. }) =
                    outcome;
                job_result(context, result_ref)
            })
        }),
    }
}

/// A download whose bytes have all arrived is verifying them before its
/// stage moves on.
const VERIFY_LABEL: &str = "verify";
const DOWNLOAD_STAGE: &str = "download";

fn progress(job: &JobSnapshot) -> dto::JobProgressDto {
    let progress = &job.progress;
    let stage = job.stage.name.as_str();
    let (current, total, unit) = if let Some(bytes) = &progress.bytes {
        (
            bytes.completed,
            bytes.total,
            Some(dto::JobProgressUnit::Bytes),
        )
    } else if let Some(units) = &progress.units {
        (
            units.completed,
            units.total,
            Some(dto::JobProgressUnit::Items),
        )
    } else if let Some(fraction) = &progress.fraction {
        let permille = (fraction.value() * 1000.0).round().clamp(0.0, 1000.0);
        (
            u64::try_from(permille as i64).unwrap_or_default(),
            Some(1000),
            Some(dto::JobProgressUnit::Permille),
        )
    } else {
        (0, None, None)
    };
    let verifying = stage == DOWNLOAD_STAGE
        && unit == Some(dto::JobProgressUnit::Bytes)
        && !job.state.is_terminal()
        && total.is_some_and(|total| total > 0 && current >= total);
    dto::JobProgressDto {
        current,
        total,
        unit,
        label_code: Some(if verifying { VERIFY_LABEL } else { stage }.to_owned()),
        bytes_per_second: None,
    }
}

fn job_result(context: &ApiContext, result: &OutcomeRef) -> Option<dto::JobResultDto> {
    Some(match result {
        OutcomeRef::ArtifactInstallation(_) => dto::JobResultDto::ArtifactInstalled,
        OutcomeRef::GeneratedAssetSet(asset_id) => dto::JobResultDto::Asset {
            asset: context.asset_ref(*asset_id),
        },
        OutcomeRef::GenerationTurn(turn_id) => dto::JobResultDto::GenerationTurn {
            turn_id: turn_id.to_string(),
        },
        OutcomeRef::Conversation(conversation_id) => dto::JobResultDto::Conversation {
            conversation_id: conversation_id.to_string(),
        },
        OutcomeRef::Group(group_id) => dto::JobResultDto::Group {
            group_id: group_id.to_string(),
        },
        OutcomeRef::Character(character_id) => dto::JobResultDto::Character {
            character_id: character_id.to_string(),
        },
        OutcomeRef::ModelProfile(model_profile_id) => dto::JobResultDto::ModelProfile {
            model_profile_id: model_profile_id.to_string(),
        },
        OutcomeRef::CreatedProposal(_)
        | OutcomeRef::TransferReport(_)
        | OutcomeRef::SyncReport(_)
        | OutcomeRef::SpeechAsset(_)
        | OutcomeRef::MemoryRun(_)
        | OutcomeRef::Checkpoint(_)
        | OutcomeRef::Request(_) => return None,
    })
}

const fn failure_code(code: JobErrorCode) -> dto::JobFailureCode {
    match code {
        JobErrorCode::Cancelled => dto::JobFailureCode::Cancelled,
        JobErrorCode::InvalidInput => dto::JobFailureCode::InvalidInput,
        JobErrorCode::Authentication => dto::JobFailureCode::Authentication,
        JobErrorCode::CapabilityUnavailable => dto::JobFailureCode::CapabilityUnavailable,
        JobErrorCode::IntegrityFailure => dto::JobFailureCode::IntegrityFailure,
        JobErrorCode::ResourceUnavailable => dto::JobFailureCode::ResourceUnavailable,
        JobErrorCode::LeaseLost => dto::JobFailureCode::LeaseLost,
        JobErrorCode::WorkerFailed => dto::JobFailureCode::WorkerFailed,
        JobErrorCode::StorageFailure => dto::JobFailureCode::StorageFailure,
        JobErrorCode::SafetyRefusal => dto::JobFailureCode::SafetyRefusal,
        JobErrorCode::TimedOut => dto::JobFailureCode::TimedOut,
        JobErrorCode::Unknown => dto::JobFailureCode::Unknown,
    }
}

macro_rules! mirror {
    ($to_dto:ident, $from_dto:ident, $domain:ty, $dto:ty, [$($variant:ident),+ $(,)?]) => {
        const fn $to_dto(value: $domain) -> $dto {
            match value {
                $(<$domain>::$variant => <$dto>::$variant,)+
            }
        }

        const fn $from_dto(value: $dto) -> $domain {
            match value {
                $(<$dto>::$variant => <$domain>::$variant,)+
            }
        }
    };
}

mirror!(
    job_kind_dto,
    job_kind,
    JobKind,
    dto::JobKindDto,
    [
        ArtifactInstall,
        ArtifactVerify,
        RuntimePrepare,
        ModelLoad,
        MemoryExtraction,
        MemoryConsolidation,
        CompanionGrowth,
        CompanionConsolidation,
        CompanionSoulWriter,
        ConversationGeneration,
        VectorIndexBuild,
        CreationRun,
        ImageGenerate,
        MediaTransform,
        TransferImport,
        TransferExport,
        BackupExport,
        BackupRestore,
        SyncSession,
        SpeechTranscribe,
        SpeechSynthesize,
        EmbeddingBenchmark,
        Maintenance,
        ModelPull,
        ModelsFolderMove,
    ]
);

mirror!(
    job_state_dto,
    job_state,
    JobState,
    dto::JobStateDto,
    [
        Queued,
        Claimed,
        Running,
        CancellationRequested,
        CleaningUp,
        Succeeded,
        Failed,
        Cancelled,
        Interrupted,
    ]
);

mirror!(
    subject_kind_dto,
    subject_kind,
    SubjectKind,
    dto::JobSubjectKindDto,
    [
        Conversation,
        Group,
        MemorySpace,
        CreationProject,
        ArtifactInstall,
        ImageRequest,
        TransferPlan,
        Backup,
        Peer,
        SpeechRequest,
        Runtime,
        ModelProfile,
        Maintenance,
        ProviderModel,
    ]
);
