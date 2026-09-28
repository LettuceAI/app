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

use super::local::{
    LocalModelJobDetail, LocalModelJobResult, digest, encode, internal, operation_key,
    record_operation, replay, stable_uuid,
};
use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use crate::{ReplyHelperRequest, ResultRecorder, ScenePromptRequest, ScenePromptWriter};

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
    let draft = match request.mode {
        dto::HelpMeReplyMode::New => None,
        dto::HelpMeReplyMode::Enrich => request
            .current_draft
            .filter(|draft| !draft.trim().is_empty()),
    };
    let swap_places = request.swap_places;
    let request_digest = digest(&(conversation_id.to_string(), &draft, swap_places))?;
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
            let request = ReplyHelperRequest {
                conversation_id,
                request_id,
                current_draft: draft.clone(),
                swap_places,
            };
            let job = context
                .backend()
                .reply_helper(context.inference())
                .admit(&request)
                .map_err(internal)?;
            database
                .record_local_model_job(
                    job.id,
                    &encode(&LocalModelJobDetail::HelpMeReply {
                        request_id: request_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        draft,
                        swap_places,
                    })?,
                )
                .map_err(internal)?;
            record_operation(context, &key, &request_digest, job.id)?;
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
            let request_id = RequestId::from_uuid(stable_uuid(&["scene-prompt", &key]));
            let request = ScenePromptRequest {
                conversation_id,
                message_id,
                request_id,
            };
            let job = ScenePromptWriter::new(database, media, context.inference())
                .admit(&request)
                .map_err(internal)?;
            database
                .record_local_model_job(
                    job.id,
                    &encode(&LocalModelJobDetail::ScenePrompt {
                        request_id: request_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        message_id: message_id.to_string(),
                    })?,
                )
                .map_err(internal)?;
            record_operation(context, &key, &request_digest, job.id)?;
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

    fn lane(&self, context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        if !is_text_feature(job) {
            return None;
        }
        match context.backend().database().local_model_job(job.id) {
            Ok(Some(_)) => Some(JobLane(format!("text-feature:{}", job.id))),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(job_id = %job.id, %error, "a text feature job's detail could not be read");
                None
            }
        }
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        _worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let record = context
            .blocking(move |context| {
                context
                    .backend()
                    .database()
                    .local_model_job(job_id)
                    .map_err(internal)
            })
            .await?;
        let Some(record) = record else {
            return Ok(None);
        };
        let Ok(detail) = serde_json::from_value::<LocalModelJobDetail>(record.detail) else {
            return Ok(None);
        };
        if !matches!(
            detail,
            LocalModelJobDetail::HelpMeReply { .. } | LocalModelJobDetail::ScenePrompt { .. }
        ) {
            return Ok(None);
        }
        Ok(Some(Box::new(ClaimedText {
            job_id,
            detail,
            cancellation: CancellationToken::new(),
        })))
    }
}

struct ClaimedText {
    job_id: JobId,
    detail: LocalModelJobDetail,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct TextRecorder(ApiContext);

impl ResultRecorder for TextRecorder {
    fn record(&self, job_id: JobId, text: &str) -> bool {
        let Ok(result) = encode(&LocalModelJobResult::GeneratedText {
            text: text.to_owned(),
        }) else {
            return false;
        };
        self.0
            .backend()
            .database()
            .record_local_model_job_result(job_id, &result)
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
        let backend = context.backend();
        let runtime = backend.inference_runtime();
        let recorder = TextRecorder(context.clone());
        let cancellation = self.cancellation.clone();
        let allowed = ResourceAvailability::all();
        match self.detail {
            LocalModelJobDetail::HelpMeReply {
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
                let receiver = runtime.register_stream(sink_id).ok();
                let forwarder = receiver.map(|receiver| tokio::spawn(forward(receiver, progress)));
                let outcome = backend
                    .reply_helper(context.inference())
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
                if let Some(forwarder) = forwarder
                    && let Err(error) = forwarder.await
                {
                    tracing::warn!(%error, "help me reply stream forwarder stopped");
                }
                if let Err(error) = outcome {
                    tracing::info!(job_id = %self.job_id, %error, "help me reply did not produce a reply");
                }
            }
            LocalModelJobDetail::ScenePrompt {
                request_id,
                conversation_id,
                message_id,
            } => {
                let request = ScenePromptRequest {
                    conversation_id: parse_id(&conversation_id, "conversation_id")?,
                    message_id: parse_id(&message_id, "message_id")?,
                    request_id: parse_id(&request_id, "request_id")?,
                };
                let Some(media) = context.media() else {
                    return Err(invalid_field("job", "the media store is unavailable"));
                };
                let outcome =
                    ScenePromptWriter::new(backend.database(), media, context.inference())
                        .with_inference_runtime(runtime)
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
                if let Err(error) = outcome {
                    tracing::info!(job_id = %self.job_id, %error, "the scene prompt writer did not produce a prompt");
                }
            }
            LocalModelJobDetail::ModelDownload { .. }
            | LocalModelJobDetail::ModelPull { .. }
            | LocalModelJobDetail::ModelsFolderMove { .. } => {}
        }
        Ok(())
    }
}
