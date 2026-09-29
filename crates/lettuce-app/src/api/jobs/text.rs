//! The jobs that write one text for a chat: help me reply and the scene
//! prompt writer. Admitting one records what it works on next to the job; the
//! runner claims it, streams the model's text to the job's watches and
//! stores the cleaned text before the job settles, so the job's result
//! carries it.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{ConversationReader, GenerationStreamEvent};
use lettuce_jobs::{
    JobKind, JobSnapshot, ResourceAvailability, WorkerId, handle::CancellationToken,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{ConversationId, JobId, MessageId, RequestId};

use super::local::{digest, encode, internal, operation_key, stable_uuid};
use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, parse_id};
use crate::{ReplyHelperRequest, ResultRecorder, ScenePromptRequest, ScenePromptWriter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TextFeatureDetail {
    HelpMeReply {
        request_id: String,
        conversation_id: String,
        draft: Option<String>,
        swap_places: bool,
    },
    ScenePrompt {
        request_id: String,
        conversation_id: String,
        message_id: String,
        target: lettuce_conversations::SceneFollowUpTarget,
        recent_context: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TextFeatureResult {
    pub text: String,
}

fn replay(context: &ApiContext, key: &str, digest: &str) -> Result<Option<JobId>, ApiError> {
    match context
        .backend()
        .database()
        .job_operation(key)
        .map_err(internal)?
    {
        Some(prior) if prior.request_digest == digest => Ok(Some(prior.job_id)),
        Some(_) => Err(api_error(
            ApiErrorCode::Conflict,
            "the operation key names another request",
        )),
        None => Ok(None),
    }
}

fn admission_error(error: lettuce_jobs::StoreError) -> ApiError {
    if error == lettuce_jobs::StoreError::IdempotencyConflict {
        api_error(
            ApiErrorCode::Conflict,
            "the operation key names another request",
        )
    } else {
        internal(error)
    }
}

pub(super) fn feature_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<dto::JobResultDto>, ApiError> {
    if !is_text_feature(job) {
        return Ok(None);
    }
    let record = context
        .backend()
        .database()
        .job_detail(job.id)
        .map_err(internal)?
        .ok_or_else(|| internal("the text job detail is missing"))?;
    serde_json::from_value::<TextFeatureDetail>(record.detail).map_err(internal)?;
    record
        .result
        .map(|result| serde_json::from_value::<TextFeatureResult>(result).map_err(internal))
        .transpose()
        .map(|result| result.map(|result| dto::JobResultDto::GeneratedText { text: result.text }))
}

const TEXT_LEASE: Duration = Duration::from_secs(60 * 60);
const REPLY_HELPER_PREFIX: &str = "reply-helper-";
const SCENE_PROMPT_PREFIX: &str = "scene-prompt-";

/// Queues help me reply for the conversation and returns its job; the
/// request's key names it, so repeating the request returns the same job and
/// another request under the key is `Conflict`. A disabled feature is
/// `Unsupported`.
pub async fn conversation_help_me_reply(
    context: &ApiContext,
    request: dto::ConversationHelpMeReplyRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let key = operation_key("help_me_reply", &request.client_operation_id)?;
    let mode = request.mode;
    let draft = match request.mode {
        dto::HelpMeReplyMode::New => None,
        dto::HelpMeReplyMode::Enrich => request
            .current_draft
            .filter(|draft| !draft.trim().is_empty()),
    };
    let swap_places = request.swap_places;
    let request_digest = digest(&(conversation_id.to_string(), mode, &draft, swap_places))?;
    let job_id = context
        .blocking(move |context| {
            if let Some(job_id) = replay(context, &key, &request_digest)? {
                return Ok(job_id);
            }
            let database = context.backend().database();
            ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let settings = GlobalSettingsStore::load(database)
                .map_err(|_| api_error(ApiErrorCode::Internal, "the settings could not be read"))?
                .settings;
            if !settings.help_me_reply.enabled {
                return Err(api_error(
                    ApiErrorCode::Unsupported,
                    "Help Me Reply is disabled in settings",
                ));
            }
            let request_id = RequestId::from_uuid(stable_uuid(&["help-me-reply", &key]));
            let job = database
                .admit_job_with_detail(
                    crate::jobs::one_shot_job::one_shot_spec(
                        crate::jobs::one_shot_job::OneShotJob {
                            name: "reply-helper",
                            stage: "help-me-reply",
                            subject_kind: lettuce_jobs::SubjectKind::Conversation,
                            subject: &conversation_id.to_string(),
                            request_id,
                        },
                    ),
                    &key,
                    &request_digest,
                    &encode(&TextFeatureDetail::HelpMeReply {
                        request_id: request_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        draft,
                        swap_places,
                    })?,
                )
                .map_err(admission_error)?;
            Ok(job.id)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

/// Queues the scene prompt writer for a message of a one-to-one chat and
/// returns its job; repeating the request under its key returns the same
/// job. A disabled scene feature is `Unsupported`.
pub(crate) async fn admit_scene_prompt(
    context: &ApiContext,
    conversation_id: ConversationId,
    message_id: MessageId,
    key: String,
) -> Result<dto::JobAccepted, ApiError> {
    let key = operation_key("scene_prompt", &key)?;
    let request_digest = digest(&(conversation_id.to_string(), message_id.to_string()))?;
    let job_id = context
        .blocking(move |context| {
            if let Some(job_id) = replay(context, &key, &request_digest)? {
                return Ok(job_id);
            }
            let database = context.backend().database();
            let settings = GlobalSettingsStore::load(database)
                .map_err(|_| api_error(ApiErrorCode::Internal, "the settings could not be read"))?
                .settings;
            if !settings.image_generation.scene_enabled {
                return Err(api_error(
                    ApiErrorCode::Unsupported,
                    "Scene generation is disabled in settings",
                ));
            }
            let Some(media) = context.media() else {
                return Err(api_error(
                    ApiErrorCode::Unavailable,
                    "the media store is unavailable",
                ));
            };
            let (_, target) = crate::api::scenes::scene_target(context, message_id)?;
            let recent_context = ScenePromptWriter::new(database, media, context.inference())
                .capture_recent(conversation_id, message_id)
                .map_err(internal)?;
            let request_id = RequestId::from_uuid(stable_uuid(&["scene-prompt", &key]));
            let job = database
                .admit_job_with_detail(
                    crate::jobs::one_shot_job::one_shot_spec(
                        crate::jobs::one_shot_job::OneShotJob {
                            name: "scene-prompt",
                            stage: "scene-prompt",
                            subject_kind: lettuce_jobs::SubjectKind::Conversation,
                            subject: &conversation_id.to_string(),
                            request_id,
                        },
                    ),
                    &key,
                    &request_digest,
                    &encode(&TextFeatureDetail::ScenePrompt {
                        request_id: request_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        message_id: message_id.to_string(),
                        target,
                        recent_context,
                    })?,
                )
                .map_err(admission_error)?;
            Ok(job.id)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

/// Runs the queued text feature jobs the API admitted. A queued job of the
/// same kind without a recorded detail belongs to a caller that runs it
/// itself and is left alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextFeatureHandler;

fn is_text_feature(job: &JobSnapshot) -> bool {
    job.idempotency_key.as_ref().is_some_and(|key| {
        key.as_str().starts_with(REPLY_HELPER_PREFIX)
            || key.as_str().starts_with(SCENE_PROMPT_PREFIX)
    })
}

#[async_trait]
impl JobHandler for TextFeatureHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::CreationRun]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        is_text_feature(job).then(|| JobLane(format!("text-feature:{}", job.id)))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let record = context
            .blocking(move |context| {
                context
                    .backend()
                    .database()
                    .job_detail(job_id)
                    .map_err(internal)
            })
            .await?;
        let detail = record
            .ok_or_else(|| internal("the text job detail is missing"))
            .and_then(|record| {
                serde_json::from_value::<TextFeatureDetail>(record.detail).map_err(internal)
            });
        let detail = match detail {
            Ok(detail) => detail,
            Err(error) => {
                fail_unclaimed(context, job_id).await?;
                return Err(error);
            }
        };
        let claim = context
            .blocking(move |context| {
                use lettuce_jobs::JobStore;
                context
                    .backend()
                    .database()
                    .claim(
                        job_id,
                        worker_id,
                        context.now(),
                        TEXT_LEASE,
                        &ResourceAvailability::all(),
                    )
                    .map_err(internal)
            })
            .await?;
        let Some(claim) = claim else {
            return Ok(None);
        };
        let owned = claim.claim.clone();
        context
            .blocking(move |context| {
                use lettuce_jobs::{JobMutation, JobStore};
                context
                    .backend()
                    .database()
                    .append_and_transition(JobMutation::Start {
                        claim: owned,
                        at: context.now(),
                    })
                    .map_err(internal)?;
                Ok(())
            })
            .await?;
        Ok(Some(Box::new(ClaimedText {
            claim: claim.claim,
            detail,
            cancellation: CancellationToken::new(),
        })))
    }
}

struct ClaimedText {
    claim: lettuce_jobs::ClaimRef,
    detail: TextFeatureDetail,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct TextRecorder(ApiContext);

impl ResultRecorder for TextRecorder {
    fn record(&self, job_id: JobId, text: &str) -> bool {
        let Ok(result) = encode(&TextFeatureResult {
            text: text.to_owned(),
        }) else {
            return false;
        };
        self.0
            .backend()
            .database()
            .record_job_detail_result(job_id, &result)
            .unwrap_or(false)
    }
}

/// Forwards what the model streams for `sink_id` to the job's watches until
/// the stream ends.
async fn forward(
    mut receiver: lettuce_inference::InferenceStreamReceiver,
    progress: Arc<dyn JobProgressSink>,
) {
    while let Some(envelope) = receiver.recv().await {
        match envelope.event {
            GenerationStreamEvent::TextDelta { text } => progress.text_delta(Some(text), None),
            GenerationStreamEvent::ReasoningDelta { text } => {
                progress.text_delta(None, Some(text));
            }
        }
    }
}

#[async_trait]
impl ClaimedJob for ClaimedText {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let result = self.execute(&context, progress).await;
        if result.is_err() {
            fail_owned(&context, self.claim.clone()).await?;
        }
        result
    }
}

impl ClaimedText {
    async fn execute(
        &self,
        context: &ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let backend = context.backend();
        let runtime = backend.inference_runtime();
        let recorder = TextRecorder(context.clone());
        let cancellation = self.cancellation.clone();
        let allowed = ResourceAvailability::all();
        match self.detail.clone() {
            TextFeatureDetail::HelpMeReply {
                request_id,
                conversation_id,
                draft,
                swap_places,
            } => {
                let request = ReplyHelperRequest {
                    conversation_id: parse_id(&conversation_id, "conversation_id")?,
                    request_id: parse_id(&request_id, "request_id")?,
                    current_draft: draft,
                    swap_places,
                };
                let sink_id = request.request_id;
                let receiver = runtime.register_stream(sink_id).map_err(internal)?;
                let forwarder = tokio::spawn(forward(receiver, progress));
                let outcome = backend
                    .reply_helper(context.inference())
                    .with_claim(&self.claim)
                    .with_cancellation(&cancellation)
                    .with_result_recorder(&recorder)
                    .generate(
                        &request,
                        WorkerId::new(),
                        context.now(),
                        TEXT_LEASE,
                        &allowed,
                    )
                    .await;
                if let Err(error) = runtime.unregister_stream(sink_id) {
                    tracing::warn!(%error, "help me reply stream could not be unregistered");
                }
                if let Err(error) = forwarder.await {
                    tracing::warn!(%error, "help me reply stream forwarder stopped");
                }
                if let Err(error) = &outcome {
                    fail_owned_with_error(
                        context,
                        self.claim.clone(),
                        crate::jobs::one_shot_job::OneShotFailure::job_error(error),
                    )
                    .await?;
                }
                outcome.map_err(internal)?;
            }
            TextFeatureDetail::ScenePrompt {
                request_id,
                conversation_id,
                message_id,
                target: _,
                recent_context,
            } => {
                let request = ScenePromptRequest {
                    conversation_id: parse_id(&conversation_id, "conversation_id")?,
                    message_id: parse_id(&message_id, "message_id")?,
                    recent_context: Some(recent_context.clone()),
                    request_id: parse_id(&request_id, "request_id")?,
                };
                let Some(media) = context.media() else {
                    return Err(api_error(
                        ApiErrorCode::Unavailable,
                        "the media store is unavailable",
                    ));
                };
                let outcome =
                    ScenePromptWriter::new(backend.database(), media, context.inference())
                        .with_inference_runtime(runtime)
                        .with_claim(&self.claim)
                        .with_cancellation(&cancellation)
                        .with_result_recorder(&recorder)
                        .generate(
                            &request,
                            WorkerId::new(),
                            context.now(),
                            TEXT_LEASE,
                            &allowed,
                        )
                        .await;
                if let Err(error) = &outcome {
                    fail_owned_with_error(
                        context,
                        self.claim.clone(),
                        crate::jobs::one_shot_job::OneShotFailure::job_error(error),
                    )
                    .await?;
                }
                outcome.map_err(internal)?;
            }
        }
        Ok(())
    }
}

async fn fail_unclaimed(context: &ApiContext, job_id: JobId) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            use lettuce_jobs::{JobError, JobErrorCode, JobMutation, JobStore};
            let database = context.backend().database();
            let Some(job) = JobStore::get(database, job_id).map_err(internal)? else {
                return Err(internal("the text job is missing"));
            };
            if job.state != lettuce_jobs::JobState::Queued {
                return Ok(());
            }
            if let Some(claim) = database
                .claim(
                    job_id,
                    WorkerId::new(),
                    context.now(),
                    TEXT_LEASE,
                    &ResourceAvailability::all(),
                )
                .map_err(internal)?
            {
                database
                    .append_and_transition(JobMutation::Start {
                        claim: claim.claim.clone(),
                        at: context.now(),
                    })
                    .map_err(internal)?;
                database
                    .append_and_transition(JobMutation::Fail {
                        claim: claim.claim,
                        error: JobError::new(
                            JobErrorCode::StorageFailure,
                            false,
                            crate::jobs::failure_labels::RESULT_STORAGE_FAILED,
                        )
                        .expect("constant job error"),
                        at: context.now(),
                    })
                    .map_err(internal)?;
            }
            Ok(())
        })
        .await
}

async fn fail_owned(context: &ApiContext, claim: lettuce_jobs::ClaimRef) -> Result<(), ApiError> {
    let error = lettuce_jobs::JobError::new(
        lettuce_jobs::JobErrorCode::StorageFailure,
        false,
        crate::jobs::failure_labels::RESULT_STORAGE_FAILED,
    )
    .expect("constant job error");
    fail_owned_with_error(context, claim, error).await
}

async fn fail_owned_with_error(
    context: &ApiContext,
    claim: lettuce_jobs::ClaimRef,
    error: lettuce_jobs::JobError,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            use lettuce_jobs::{JobMutation, JobStore};
            let database = context.backend().database();
            let job = JobStore::get(database, claim.job_id)
                .map_err(internal)?
                .ok_or_else(|| internal("the text job is missing"))?;
            if job.state.is_terminal() || job.claim.as_ref() != Some(&claim) {
                return Ok(());
            }
            if job.state == lettuce_jobs::JobState::CancellationRequested {
                let at = context.now().max(job.updated_at);
                database
                    .append_and_transition(JobMutation::RequestCleanup {
                        claim: claim.clone(),
                        at,
                    })
                    .map_err(internal)?;
                database
                    .append_and_transition(JobMutation::FinishCancellation { claim, at })
                    .map_err(internal)?;
                return Ok(());
            }

            database
                .append_and_transition(JobMutation::Fail {
                    claim,
                    error,
                    at: context.now().max(job.updated_at),
                })
                .map_err(internal)?;
            Ok(())
        })
        .await
}
