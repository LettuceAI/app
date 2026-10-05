//! Runs the queued speech jobs: Whisper transcriptions one at a time, local
//! Kokoro syntheses one at a time, remote syntheses concurrently. Failures
//! retrying cannot fix end the job at once with a typed label; transient
//! ones are retried after a growing delay, up to an attempt cap.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{
    self as dto, ApiError, SpeechFailure, SpeechModelKind, SpeechRuntimeKind,
};
use lettuce_jobs::{
    CancellationReason, JobError, JobErrorCode, JobKind, JobMutation, JobSnapshot, JobState,
    JobStore, ResourceAvailability, ResourceClass, WorkerId, handle::CancellationToken,
};
use lettuce_speech::SynthesisState;
use lettuce_types::JobId;

use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::api::error::IntoApiError;
use crate::{
    SPEECH_ESPEAK_MISSING, SPEECH_MODEL_REQUIRED_KOKORO, SPEECH_MODEL_REQUIRED_WHISPER,
    SPEECH_RETRIES_EXHAUSTED, SPEECH_SECRET_MISSING, SPEECH_VOICE_MISSING,
    SpeechTranscriptionClaimedWork, SpeechTranscriptionError, TtsSynthesisClaimedWork,
    TtsSynthesisError, speech_not_before,
};

/// A running speech job renews its claim at a third of this.
const SPEECH_LEASE: Duration = Duration::from_secs(10 * 60);
const TRANSCRIBE_LANE: &str = "speech:whisper";
const KOKORO_LANE: &str = "speech:kokoro";
const MEDIA_UNAVAILABLE: &str = "speech-media-unavailable";
const WORK_INVALID: &str = "speech-work-invalid";

/// What a speech job's error label tells the user.
pub(crate) fn speech_failure(label: &str) -> Option<SpeechFailure> {
    Some(match label {
        SPEECH_MODEL_REQUIRED_WHISPER => SpeechFailure::ModelRequired {
            model: SpeechModelKind::Whisper,
        },
        SPEECH_MODEL_REQUIRED_KOKORO => SpeechFailure::ModelRequired {
            model: SpeechModelKind::Kokoro,
        },
        SPEECH_SECRET_MISSING => SpeechFailure::SecretMissing,
        SPEECH_VOICE_MISSING => SpeechFailure::VoiceMissing,
        SPEECH_ESPEAK_MISSING => SpeechFailure::RuntimeMissing {
            runtime: SpeechRuntimeKind::Espeak,
        },
        crate::SPEECH_ONNX_MISSING => SpeechFailure::RuntimeMissing { runtime: SpeechRuntimeKind::OnnxRuntime },
        SPEECH_RETRIES_EXHAUSTED => SpeechFailure::RetriesExhausted,
        _ => return None,
    })
}

/// What the job view shows of a speech job: its failure and, once it
/// succeeded, its result.
pub(crate) fn speech_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<(Option<dto::JobResultDto>, Option<SpeechFailure>), ApiError> {
    let failure = job
        .error
        .as_ref()
        .and_then(|error| speech_failure(error.message.as_str()));
    let result = match job.kind {
        JobKind::SpeechTranscribe => crate::api::speech::transcription_view(context, job)?
            .map(|transcription| dto::JobResultDto::Transcription { transcription }),
        JobKind::SpeechSynthesize if job.state == JobState::Succeeded => {
            match lettuce_speech::SynthesisRepository::get(context.backend().database(), job.id)
                .map(|record| record.state)
            {
                Ok(SynthesisState::Succeeded { result }) => Some(dto::JobResultDto::Asset {
                    asset: context.asset_ref(result.audio_asset_id),
                }),
                Ok(_) => return Err(crate::api::error::api_error(
                    dto::ApiErrorCode::Internal, "the completed synthesis has no audio result",
                )),
                Err(error) => return Err(crate::api::error::api_error(
                    dto::ApiErrorCode::Internal, error.to_string(),
                )),
            }
        }
        _ => None,
    };
    Ok((result, failure))
}

/// Ends a queued job that can never run: it is claimed and failed, so it
/// does not wait for work that will not come.
fn fail_unrunnable(
    context: &ApiContext,
    job_id: JobId,
    label: &'static str,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let Some(job) = database.get(job_id).map_err(IntoApiError::into_api_error)? else {
        return Ok(());
    };
    if job.state != JobState::Queued {
        return Ok(());
    }
    let at = context.now().max(job.updated_at);
    let claim = database
        .claim(
            job_id,
            WorkerId::new(),
            at,
            SPEECH_LEASE,
            &ResourceAvailability::all(),
        )
        .map_err(IntoApiError::into_api_error)?;
    let Some(claim) = claim else {
        return Ok(());
    };
    database
        .append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })
        .map_err(IntoApiError::into_api_error)?;
    database
        .append_and_transition(JobMutation::Fail {
            claim: claim.claim,
            error: JobError::new(JobErrorCode::InvalidInput, false, label)
                .expect("constant job error is valid"),
            at,
        })
        .map_err(IntoApiError::into_api_error)?;
    Ok(())
}

fn fail_running(
    context: &ApiContext,
    claim: lettuce_jobs::ClaimRef,
    code: JobErrorCode,
    label: &'static str,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let Some(job) = database
        .get(claim.job_id)
        .map_err(IntoApiError::into_api_error)?
    else {
        return Ok(());
    };
    if job.state.is_terminal() {
        return Ok(());
    }
    database
        .append_and_transition(JobMutation::Fail {
            claim,
            error: JobError::new(code, false, label).expect("constant job error is valid"),
            at: context.now().max(job.updated_at),
        })
        .map_err(IntoApiError::into_api_error)?;
    Ok(())
}

fn cancellation_reason(context: &ApiContext) -> CancellationReason {
    if context.shutdown_token().is_cancelled() {
        CancellationReason::Shutdown
    } else {
        CancellationReason::User
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpeechTranscribeHandler;

#[async_trait]
impl JobHandler for SpeechTranscribeHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::SpeechTranscribe]
    }

    fn lane(&self, _context: &ApiContext, _job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(TRANSCRIBE_LANE.to_owned()))
    }

    fn not_before(
        &self,
        _context: &ApiContext,
        job: &JobSnapshot,
    ) -> Option<lettuce_types::TimestampMillis> {
        speech_not_before(job)
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let claimed = context
            .blocking(move |context| {
                let _folder_access = context.local_models().folder_access();
                let root = crate::api::speech::whisper::whisper_root(context)?;
                if root.starts_with(crate::api::local_models::models_root(context)?)
                    && folder_moving(context)? { return Ok(None); }
                let claimed = context.backend().speech_transcriptions().claim(
                    job_id,
                    worker_id,
                    context.now(),
                    SPEECH_LEASE,
                    &ResourceAvailability::all(),
                );
                match claimed {
                    Ok(work) => Ok(work),
                    Err(SpeechTranscriptionError::Jobs(error)) => {
                        Err(IntoApiError::into_api_error(error))
                    }
                    Err(error) => {
                        tracing::warn!(%job_id, %error, "a queued transcription cannot run");
                        fail_unrunnable(context, job_id, WORK_INVALID)?;
                        Ok(None)
                    }
                }
            })
            .await?;
        Ok(claimed.map(|work| Box::new(ClaimedTranscription { work }) as Box<dyn ClaimedJob>))
    }
}

struct ClaimedTranscription {
    work: SpeechTranscriptionClaimedWork,
}

#[async_trait]
impl ClaimedJob for ClaimedTranscription {
    fn cancellation(&self) -> CancellationToken {
        self.work.handle.cancellation_token()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let work = self.work;
        let claim = work.claim.claim.clone();
        let renew_claim = claim.clone();
        let run_context = context.clone();
        let result = super::image_tools::renewing(
            &context,
            &renew_claim,
            SPEECH_LEASE,
            context.blocking(move |context| {
                let Some(media) = context.media() else {
                    fail_running(
                        context,
                        work.claim.claim.clone(),
                        JobErrorCode::StorageFailure,
                        MEDIA_UNAVAILABLE,
                    )?;
                    return Ok(None);
                };
                let backend = context.backend();
                let runtime = context.speech().asr_runtime(context);
                backend
                    .speech_transcriptions()
                    .run_with_clock(
                        work,
                        media,
                        &backend.asr_learning(),
                        runtime.as_ref(),
                        cancellation_reason(context),
                        context.clock(),
                    )
                    .map(Some)
                    .map_err(IntoApiError::into_api_error)
            }),
        )
        .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                let failed = run_context
                    .blocking(move |context| {
                        fail_running(context, claim, JobErrorCode::WorkerFailed, WORK_INVALID)
                    })
                    .await;
                if let Err(failed) = failed {
                    tracing::warn!(message = %failed.message, "a transcription that failed could not be ended");
                }
                Err(error)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpeechSynthesizeHandler;

#[async_trait]
impl JobHandler for SpeechSynthesizeHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::SpeechSynthesize]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(if job.resources.contains(&ResourceClass::ModelLoad) {
            KOKORO_LANE.to_owned()
        } else {
            format!("speech:remote:{}", job.id)
        }))
    }

    fn not_before(
        &self,
        _context: &ApiContext,
        job: &JobSnapshot,
    ) -> Option<lettuce_types::TimestampMillis> {
        speech_not_before(job)
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let claimed = context
            .blocking(move |context| {
                let _folder_access = context.local_models().folder_access();
                if let Ok(record) = lettuce_speech::SynthesisRepository::get(context.backend().database(), job_id)
                    && record.request.provider.config.provider_kind() == lettuce_speech::AudioProviderKind::Kokoro
                    && let Some(root) = context.retained_model_roots_for_guard()?.and_then(|roots| roots.kokoro)
                    && std::path::Path::new(&root).starts_with(crate::api::local_models::models_root(context)?)
                    && folder_moving(context)? { return Ok(None); }
                let claimed = context.backend().tts_syntheses().claim(
                    job_id,
                    worker_id,
                    context.now(),
                    SPEECH_LEASE,
                    &ResourceAvailability::all(),
                );
                match claimed {
                    Ok(work) => Ok(work),
                    Err(TtsSynthesisError::Jobs(error)) => {
                        Err(IntoApiError::into_api_error(error))
                    }
                    Err(error) => {
                        tracing::warn!(%job_id, %error, "a queued synthesis cannot run");
                        fail_unrunnable(context, job_id, WORK_INVALID)?;
                        Ok(None)
                    }
                }
            })
            .await?;
        Ok(claimed.map(|work| Box::new(ClaimedSynthesis { work }) as Box<dyn ClaimedJob>))
    }
}

struct ClaimedSynthesis {
    work: TtsSynthesisClaimedWork,
}

#[async_trait]
impl ClaimedJob for ClaimedSynthesis {
    fn cancellation(&self) -> CancellationToken {
        self.work.handle.cancellation_token()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let work = self.work;
        let claim = work.claim.claim.clone();
        let renew_claim = claim.clone();
        let failure = |context: ApiContext, claim, code, label| async move {
            context
                .blocking(move |context| fail_running(context, claim, code, label))
                .await
        };
        let reason = cancellation_reason(&context);
        let result = super::image_tools::renewing(
            &context,
            &renew_claim,
            SPEECH_LEASE,
            context.blocking(move |context| {
                let Some(media) = context.media() else {
                    fail_running(context, work.claim.claim.clone(), JobErrorCode::StorageFailure, MEDIA_UNAVAILABLE)?;
                    return Ok(());
                };
                let runtime = match context.speech().tts_runtime(context) {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        fail_running(context, work.claim.claim.clone(), JobErrorCode::CapabilityUnavailable, WORK_INVALID)?;
                        return Err(error);
                    }
                };
                tokio::runtime::Handle::current().block_on(context.backend().tts_syntheses().run_with_clock(
                    work,
                    context.secret_store().as_ref(),
                    runtime.as_ref(),
                    media,
                    reason,
                    context.clock(),
                )).map(|_| ()).map_err(IntoApiError::into_api_error)
            }),
        ).await;
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                failure(context.clone(), claim, JobErrorCode::WorkerFailed, WORK_INVALID).await?;
                Err(error)
            }
        }
    }
}

fn folder_moving(context: &ApiContext) -> Result<bool, ApiError> {
    match super::local::folder_move_active(context) {
        Ok(()) => Ok(false),
        Err(error) if error.code == dto::ApiErrorCode::Busy => Ok(true),
        Err(error) => Err(error),
    }
}

pub(crate) fn active_local_files(context: &ApiContext, root: &std::path::Path) -> Result<Option<(JobId, String)>, ApiError> {
    use lettuce_jobs::JobQuery;
    use lettuce_speech::{SynthesisRepository, TranscriptionRepository};
    use lettuce_model_hub::WhisperModelRepository;
    use lettuce_types::{PageLimit, PageRequest};
    let database = context.backend().database();
    let mut cursor = None;
    loop {
        let page = database.list(JobQuery { page: PageRequest { cursor, limit: PageLimit::new(200) }, ..JobQuery::default() })
            .map_err(IntoApiError::into_api_error)?;
        for job in page.items.into_iter().filter(|job| job.claim.is_some() && !job.is_terminal()) {
            match job.kind {
                JobKind::SpeechTranscribe => {
                    let record = TranscriptionRepository::get(database, job.id)
                        .map_err(|error| crate::api::error::api_error(dto::ApiErrorCode::Internal, error.to_string()))?;
                    if let Some(model) = database.get_whisper_model(record.request.model.id.as_str())
                            .map_err(|error| crate::api::error::api_error(dto::ApiErrorCode::Internal, error.to_string()))?
                        && model.model.path.starts_with(root) {
                        return Ok(Some((job.id, model.model.path.to_string_lossy().into_owned())));
                    }
                }
                JobKind::SpeechSynthesize => {
                    let record = SynthesisRepository::get(database, job.id)
                        .map_err(|error| crate::api::error::api_error(dto::ApiErrorCode::Internal, error.to_string()))?;
                    if record.request.provider.config.provider_kind() == lettuce_speech::AudioProviderKind::Kokoro
                        && let Some(path) = context.retained_model_roots_for_guard()?.and_then(|roots| roots.kokoro)
                        && std::path::Path::new(&path).starts_with(root) {
                        return Ok(Some((job.id, path)));
                    }
                }
                _ => {}
            }
        }
        match page.next_cursor { Some(next) => cursor = Some(next), None => return Ok(None) }
    }
}
