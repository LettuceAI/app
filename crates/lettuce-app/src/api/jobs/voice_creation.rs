//! Billable voice creation: one provider call, with no replay after a claim.
use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::{
    ApiContext,
    error::{IntoApiError, api_error, parse_id},
};
use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, SpeechFailure};
use lettuce_jobs::{
    CancellationReason, ClaimRef, JobCatalog, JobError, JobErrorCode, JobKind, JobListFilter,
    JobMutation, JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef,
    RecoveryPolicy, ResourceAvailability, ResourceClass, SubjectKind, WorkerId,
    handle::CancellationToken,
};
use lettuce_speech::{CreatedVoice, VoiceCreationRequest, VoiceDesignRuntimeError};
use lettuce_types::{PageLimit, PageRequest, RequestId};
use std::{sync::Arc, time::Duration};

const LEASE: Duration = Duration::from_secs(10 * 60);
const UNKNOWN: &str = "voice-creation-outcome-unknown";

pub async fn voice_design_create(
    context: &ApiContext,
    request: dto::VoiceDesignCreateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    crate::api::speech::validate_operation_key(&request.client_operation_id)?;
    let digest = crate::api::speech::operation_digest(&request)?;
    context
        .blocking(move |context| {
            let key = format!("voice_design_create:{}", request.client_operation_id);
            let database = context.backend().database();
            if let Some(prior) = database
                .job_operation(&key)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            {
                if prior.request_digest != digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the voice creation key was used for another request",
                    ));
                }
                return Ok(dto::JobAccepted {
                    job_id: prior.job_id.to_string(),
                });
            }
            let creation = context
                .backend()
                .tts_voice_design(context.secret_store().as_ref())
                .admit_creation(crate::VoiceCreationDraft {
                    provider_id: parse_id(&request.provider_id, "provider_id")?,
                    voice_name: request.name,
                    generated_voice_id: request.generated_voice_id,
                    voice_description: request.description,
                })
                .map_err(IntoApiError::into_api_error)?;
            let id = RequestId::from(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                key.as_bytes(),
            ));
            let mut spec = JobSpec::new(
                JobKind::SpeechVoiceCreate,
                JobSubject::new(SubjectKind::SpeechRequest, id.to_string()).expect("UUID subject"),
                OutcomeRef::Request(id),
            )
            .with_resources(vec![ResourceClass::Network]);
            spec.recovery_policy = RecoveryPolicy::MarkInterrupted;
            let detail = serde_json::json!({"type": "speech_voice_create", "request": creation});
            let job = database
                .admit_job_with_detail(spec, &key, &digest, &detail)
                .map_err(IntoApiError::into_api_error)?;
            context.jobs().wake();
            Ok(dto::JobAccepted {
                job_id: job.id.to_string(),
            })
        })
        .await
}

/// Never send work that was merely queued in the previous process.
pub(crate) fn recover_queued(context: &ApiContext) -> Result<(), ApiError> {
    let mut cursor = None;
    loop {
        let page = context
            .backend()
            .database()
            .list_jobs(&JobListFilter {
                kinds: vec![JobKind::SpeechVoiceCreate],
                states: vec![JobState::Queued],
                page: PageRequest {
                    cursor: cursor.take(),
                    limit: PageLimit::new(200),
                },
                ..JobListFilter::default()
            })
            .map_err(IntoApiError::into_api_error)?;
        for job in page.items {
            let at = context.now().max(job.updated_at);
            context
                .backend()
                .database()
                .append_and_transition(JobMutation::RequestCancellation {
                    id: job.id,
                    reason: CancellationReason::Shutdown,
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
            context
                .backend()
                .database()
                .append_and_transition(JobMutation::FinishQueuedCancellation { id: job.id, at })
                .map_err(IntoApiError::into_api_error)?;
        }
        if let Some(next) = page.next_cursor {
            cursor = Some(next);
        } else {
            break;
        }
    }
    Ok(())
}

pub(crate) fn view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<(Option<dto::JobResultDto>, Option<SpeechFailure>), ApiError> {
    if job.state == JobState::Interrupted
        || (job.state == JobState::Cancelled && job.attempt.get() > 0)
    {
        return Ok((None, Some(SpeechFailure::VoiceCreationOutcomeUnknown)));
    }
    let detail = context
        .backend()
        .database()
        .job_detail(job.id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .ok_or_else(|| api_error(ApiErrorCode::Internal, "voice creation detail is missing"))?;
    let result = detail
        .result
        .map(serde_json::from_value::<CreatedVoice>)
        .transpose()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .map(|voice| dto::JobResultDto::VoiceCreated {
            voice_id: voice.voice_id,
        });
    let failure = detail
        .failure
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    Ok((result, failure))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceCreationHandler;
#[async_trait]
impl JobHandler for VoiceCreationHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::SpeechVoiceCreate]
    }
    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(format!("voice-create-{}", job.id)))
    }
    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let id = job.id;
        let at = context.now().max(job.updated_at);
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let detail = database
                    .job_detail(id)
                    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                    .ok_or_else(|| {
                        api_error(ApiErrorCode::Internal, "voice creation detail is missing")
                    })?;
                let request: VoiceCreationRequest =
                    serde_json::from_value(detail.detail["request"].clone())
                        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
                let Some(claim) = database
                    .claim(id, worker_id, at, LEASE, &ResourceAvailability::all())
                    .map_err(IntoApiError::into_api_error)?
                else {
                    return Ok(None);
                };
                database
                    .append_and_transition(JobMutation::Start {
                        claim: claim.claim.clone(),
                        at,
                    })
                    .map_err(IntoApiError::into_api_error)?;
                Ok(Some(Box::new(Work {
                    claim: claim.claim,
                    request,
                    cancellation: CancellationToken::new(),
                }) as Box<dyn ClaimedJob>))
            })
            .await
    }
}
struct Work {
    claim: ClaimRef,
    request: VoiceCreationRequest,
    cancellation: CancellationToken,
}
#[async_trait]
impl ClaimedJob for Work {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let runtime = context
            .blocking(|context| context.speech().voice_creation_runtime(context))
            .await;
        let outcome = match runtime {
            Ok(runtime) => {
                super::image_tools::renewing(
                    &context,
                    &self.claim,
                    LEASE,
                    context
                        .backend()
                        .tts_voice_design(context.secret_store().as_ref())
                        .create_voice(&self.request, runtime.as_ref(), &self.cancellation),
                )
                .await
            }
            Err(_) => Err(crate::TtsVoiceDesignError::Runtime(
                VoiceDesignRuntimeError::Unavailable,
            )),
        };
        let id = self.claim.job_id;
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let job = database
                    .get(id)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| {
                        api_error(ApiErrorCode::NotFound, "voice creation job missing")
                    })?;
                if job.state.is_terminal() {
                    return Ok(());
                }
                let at = context.now().max(job.updated_at);
                let (mutation, result, failure) = match outcome {
                    Ok(created) => (
                        JobMutation::Succeed {
                            claim: self.claim,
                            outcome: JobOutcome::Success {
                                result_ref: OutcomeRef::Request(parse_id(
                                    job.subject.id.as_str(),
                                    "request_id",
                                )?),
                            },
                            at,
                        },
                        Some(serde_json::to_value(created).expect("voice serialization")),
                        None,
                    ),
                    Err(error) => {
                        let (code, label, failure) = match error {
                            crate::TtsVoiceDesignError::Runtime(
                                VoiceDesignRuntimeError::ProviderRejected { status },
                            ) => (
                                JobErrorCode::WorkerFailed,
                                "voice-provider-rejected",
                                Some(SpeechFailure::VoiceCreationProviderRejected { status }),
                            ),
                            crate::TtsVoiceDesignError::Runtime(
                                VoiceDesignRuntimeError::Cancelled
                                | VoiceDesignRuntimeError::OutcomeUnknown,
                            ) => (
                                JobErrorCode::WorkerFailed,
                                UNKNOWN,
                                Some(SpeechFailure::VoiceCreationOutcomeUnknown),
                            ),
                            crate::TtsVoiceDesignError::SecretStore(_) => (
                                JobErrorCode::Authentication,
                                "speech-secret-missing",
                                Some(SpeechFailure::SecretMissing),
                            ),
                            _ => (
                                JobErrorCode::CapabilityUnavailable,
                                "voice-creation-unavailable",
                                None,
                            ),
                        };
                        (
                            JobMutation::Fail {
                                claim: self.claim,
                                error: JobError::new(code, false, label)
                                    .expect("constant error label"),
                                at,
                            },
                            None,
                            failure.map(|failure| {
                                serde_json::to_value(failure).expect("failure serialization")
                            }),
                        )
                    }
                };
                database
                    .settle_job_with_detail(mutation, result.as_ref(), failure.as_ref())
                    .map_err(IntoApiError::into_api_error)?;
                Ok(())
            })
            .await
    }
}
