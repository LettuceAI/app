//! The jobs API: listing, reading, cancelling and watching jobs, the change
//! feed behind `ApiEvent::JobUpdated`, and the runner for every job kind the
//! conversation generation worker does not run.

mod feed;
mod image;
mod image_tools;
pub(super) mod install;
pub(super) mod local;
mod lorebook;
mod memory;
mod runner;
pub(crate) mod speech;
mod state;
mod text;
pub(crate) mod voice_creation;

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
pub use image::ImageGenerateHandler;
pub use image_tools::ImageToolHandler;
pub(crate) use image_tools::{ImageToolDetail, admit_tool, is_lora_discovery, tool_view};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use install::CatalogVariant;
pub use install::admit_install;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) use install::admit_install_with_detail;
pub(crate) use install::recover_queued_installs;
pub use install::{
    ArtifactInstallHandler, InstallFinish, InstallSources, InstallWork, NetworkInstallSources,
};
pub(crate) use local::folder_move_active;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) use local::image_bundle_detail;
pub use local::{ModelPullHandler, ModelsFolderMoveHandler};
pub(crate) use local::{
    admit_gguf_download, admit_model_pull, admit_models_folder_move, recover_local_model_jobs,
};
pub use lorebook::LorebookHandler;
pub(crate) use memory::MemoryJobOutput;
pub use memory::{MemoryExtractionHandler, SoulWriterHandler};
pub use runner::{ClaimedJob, JobHandler, JobHandlers, JobLane, JobProgressSink, JobRunner};
pub use speech::{SpeechSynthesizeHandler, SpeechTranscribeHandler};
pub(crate) use state::JobHostState;
pub use text::{TextFeatureHandler, conversation_help_me_reply};
pub(crate) use text::{admit_design_reference, admit_scene_prompt};
pub use voice_creation::{VoiceCreationHandler, voice_design_create};

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
                    .collect::<Result<_, _>>()?,
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
            job_view(context, &job)
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
            let reset_database = super::app_reset::database_for_job(context, job_id)?;
            let database = reset_database
                .as_deref()
                .unwrap_or_else(|| context.backend().database());
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
            context.jobs().watch_with_load(
                job_id,
                sink,
                || {
                    let job = load(context, job_id)?;
                    let view = job_view(context, &job)?;
                    let (event, terminal) = job_event(&job, view.clone());
                    Ok((view, event, terminal))
                },
                || {
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    {
                        context.backend().local_runtime_events().job_load(job_id)
                    }
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    {
                        None
                    }
                },
            )
        })
        .await
}

fn load(context: &ApiContext, job_id: JobId) -> Result<JobSnapshot, ApiError> {
    let reset_database = super::app_reset::database_for_job(context, job_id)?;
    reset_database
        .as_deref()
        .unwrap_or_else(|| context.backend().database())
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

pub(crate) fn job_view(context: &ApiContext, job: &JobSnapshot) -> Result<dto::JobView, ApiError> {
    let local = local::local_job_view(context, job);
    let feature = text::feature_view(context, job)?;
    let (image_result, image_failure) = image_view(context, job)?;
    let (speech_result, speech_failure) = speech::speech_view(context, job)?;
    let soul_result = memory::soul_draft_view(context, job)?;
    let lorebook_result = lorebook::result_view(context, job)?;
    Ok(dto::JobView {
        id: job.id.to_string(),
        kind: job_kind_dto(job.kind),
        subject: dto::JobSubjectDto {
            kind: subject_kind_dto(job.subject.kind),
            id: job.subject.id.to_string(),
        },
        subject_detail: local.detail.or(lorebook::subject_view(context, job)?),
        state: job_state_dto(job.state),
        progress: progress(job),
        created_at: job.created_at.get(),
        updated_at: job.updated_at.get(),
        failure: job
            .error
            .as_ref()
            .map(|error| dto::JobFailureDto {
                code: failure_code(error.code),
                retryable: error.retryable,
                reason: failure_reason(error.message.as_str()),
                model: (error.code == JobErrorCode::CapabilityUnavailable
                    && error.message.as_str() == crate::EMBEDDING_UNAVAILABLE_JOB_ERROR)
                    .then_some(dto::RequiredModel::Embedding),
                hugging_face: (job.kind == JobKind::ArtifactInstall
                    && crate::is_hf_job_error(error.message.as_str()))
                .then(|| {
                    let repository = context
                        .backend()
                        .database()
                        .hugging_face_refusal(job.id)
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    crate::hf_failure_of_job_error(error.message.as_str(), &repository)
                })
                .flatten()
                .map(|failure| super::error::hf_failure(&failure)),
                ollama: (job.kind == JobKind::ModelPull)
                    .then(|| local::ollama_failure(error.message.as_str(), local.failure.as_ref()))
                    .flatten(),
                image: image_failure,
                speech: speech_failure,
            })
            .or_else(|| {
                speech_failure.map(|failure| dto::JobFailureDto {
                    code: dto::JobFailureCode::WorkerFailed,
                    retryable: false,
                    reason: None,
                    model: None,
                    hugging_face: None,
                    ollama: None,
                    image: None,
                    speech: Some(failure),
                })
            }),
        result: feature
            .or(super::usage_billing::result_view(context, job)?)
            .or(super::maintenance_jobs::result_view(context, job)?)
            .or(super::app_reset::result_view(context, job)?)
            .or(local.result)
            .or(soul_result)
            .or(lorebook_result)
            .or(image_result)
            .or(speech_result)
            .or_else(|| {
                job.outcome.as_ref().and_then(|outcome| {
                    let (JobOutcome::Success { result_ref }
                    | JobOutcome::Partial { result_ref, .. }) = outcome;
                    job_result(context, result_ref)
                })
            }),
    })
}

/// What the job view shows of an image job: its result and why it failed.
fn image_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<(Option<dto::JobResultDto>, Option<dto::ImageFailure>), ApiError> {
    if job.kind == JobKind::ImageGenerate {
        let view = super::image::generation_view(context, job)?;
        return Ok((view.result, view.failure));
    }
    let (result, failure) = tool_view(context, job)?;
    Ok((result, failure.filter(|_| job.error.is_some())))
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

/// What a chat feature job needs changed, from the label its error carries.
pub(super) fn failure_reason(label: &str) -> Option<dto::JobFailureReason> {
    match label {
        "reset-workers" => return Some(dto::JobFailureReason::ResetWorkers),
        "reset-database" => return Some(dto::JobFailureReason::ResetDatabase),
        "reset-webview-storage" => return Some(dto::JobFailureReason::ResetWebviewStorage),
        "reset-restart" => return Some(dto::JobFailureReason::ResetRestart),
        "usage-billing-unavailable" => return Some(dto::JobFailureReason::UsageBillingUnavailable),
        "usage-billing-malformed" => return Some(dto::JobFailureReason::UsageBillingMalformed),
        "usage-account-missing" => return Some(dto::JobFailureReason::UsageAccountMissing),
        "usage-cost-conflict" => return Some(dto::JobFailureReason::UsageCostConflict),
        "usage-cost-storage" => return Some(dto::JobFailureReason::UsageCostStorage),
        "usage-billing-credentials" => return Some(dto::JobFailureReason::UsageBillingCredentials),
        "usage-billing-rejected" => return Some(dto::JobFailureReason::UsageBillingRejected),
        "usage-billing-unsupported" => return Some(dto::JobFailureReason::UsageBillingUnsupported),
        "usage-cost-invalid" => return Some(dto::JobFailureReason::UsageCostInvalid),
        "storage-checkpoint-busy" => return Some(dto::JobFailureReason::StorageCheckpointBusy),
        "storage-unavailable" => return Some(dto::JobFailureReason::StorageUnavailable),
        "database-kept-read-only" => return Some(dto::JobFailureReason::DatabaseKeptReadOnly),
        _ => {}
    }
    use crate::jobs::failure_labels as labels;
    Some(match label {
        labels::HELP_ME_REPLY_DISABLED => dto::JobFailureReason::HelpMeReplyDisabled,
        labels::HELP_ME_REPLY_NO_HISTORY => dto::JobFailureReason::HelpMeReplyNoHistory,
        labels::HELP_ME_REPLY_NO_MODEL => dto::JobFailureReason::HelpMeReplyNoModel,
        labels::HELP_ME_REPLY_NO_REPLY => dto::JobFailureReason::HelpMeReplyNoReply,
        labels::SCENE_PROMPT_DISABLED => dto::JobFailureReason::ScenePromptDisabled,
        labels::SCENE_PROMPT_NO_MODEL => dto::JobFailureReason::ScenePromptNoModel,
        labels::SCENE_PROMPT_NO_REPLY => dto::JobFailureReason::ScenePromptNoReply,
        labels::SCENE_IMAGE_DISABLED => dto::JobFailureReason::SceneImageDisabled,
        labels::SCENE_IMAGE_NO_MODEL => dto::JobFailureReason::SceneImageNoModel,
        labels::SCENE_IMAGE_NO_IMAGE => dto::JobFailureReason::SceneImageNoImage,
        labels::DESIGN_REFERENCE_NO_MODEL => dto::JobFailureReason::DesignReferenceNoModel,
        labels::DESIGN_REFERENCE_NO_IMAGES => dto::JobFailureReason::DesignReferenceNoImages,
        _ => return None,
    })
}

pub(super) const fn failure_code(code: JobErrorCode) -> dto::JobFailureCode {
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
        SpeechVoiceCreate,
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
