use std::time::Duration;

use lettuce_contracts::{self as dto, ApiError};
use lettuce_conversations::{
    ConversationReader, GenerationFailureCode, GenerationStreamEvent, GenerationTurnStatus,
};
use lettuce_database::Database;
use lettuce_jobs::{CancellationReason, ResourceAvailability, WorkerId, handle::CancellationToken};
use lettuce_types::{GenerationTurnId, RequestId};

use super::ApiContext;
use super::error::{IntoApiError, api_error};
use crate::{
    ConversationGenerationExecutionRequest, ConversationGenerationRuntimeInput, ReplyImageOrigin,
    ReplyMediaStore,
};

/// A claim outlives the longest provider request, so a slow reply is never
/// settled after its lease ran out.
const GENERATION_LEASE: Duration = Duration::from_secs(60 * 60);
const IDLE_BACKOFF_MIN: Duration = Duration::from_millis(250);
const IDLE_BACKOFF_MAX: Duration = Duration::from_secs(5);

/// Runs queued conversation generation jobs one at a time and streams each
/// turn into the sink its send attached. Repository calls are synchronous,
/// so the host runs the worker on its own thread.
#[derive(Debug, Clone)]
pub struct ConversationGenerationWorker {
    context: ApiContext,
    worker_id: WorkerId,
    resources: ResourceAvailability,
}

impl ConversationGenerationWorker {
    #[must_use]
    pub fn new(context: ApiContext) -> Self {
        Self {
            context,
            worker_id: WorkerId::new(),
            resources: ResourceAvailability::all(),
        }
    }

    /// Polls until `shutdown` fires. An idle worker waits with a doubling
    /// backoff, and a send wakes it at once.
    pub async fn run(&self, shutdown: CancellationToken) {
        let mut idle = IDLE_BACKOFF_MIN;
        while !shutdown.is_cancelled() {
            match self.run_once().await {
                Ok(true) => {
                    idle = IDLE_BACKOFF_MIN;
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(code = ?error.code, message = %error.message, "conversation generation worker step failed");
                }
            }
            tokio::select! {
                () = shutdown.cancelled() => break,
                () = self.context.woken() => idle = IDLE_BACKOFF_MIN,
                () = tokio::time::sleep(idle) => idle = (idle * 2).min(IDLE_BACKOFF_MAX),
            }
        }
    }

    /// Runs the next queued job, if any; returns whether one ran.
    pub async fn run_once(&self) -> Result<bool, ApiError> {
        let context = &self.context;
        let backend = context.backend();
        let database = backend.database();
        let reply_media = context.media().map(|store| backend.reply_media(store));
        let reply_media: &dyn ReplyMediaStore = match &reply_media {
            Some(store) => store,
            None => &UnstoredReplyMedia,
        };
        let runner = backend.prepared_conversation_generation_runner(
            context.embedding(),
            context.inference(),
            reply_media,
        );
        let Some(next) = runner
            .next_queued(&self.resources)
            .map_err(IntoApiError::into_api_error)?
        else {
            return Ok(false);
        };
        let turn_label = next.turn_id.to_string();
        let sink = context.stream(next.turn_id);
        let mut stream_sink = None;
        let mut forwarder = None;
        if let Some(sink) = sink {
            sink.emit(dto::GenerationEvent::Started {
                turn_id: turn_label.clone(),
            });
            let sink_id = RequestId::new();
            match backend.inference_runtime().register_stream(sink_id) {
                Ok(mut receiver) => {
                    let turn_id = turn_label.clone();
                    forwarder = Some(tokio::spawn(async move {
                        while let Some(envelope) = receiver.recv().await {
                            let (text, reasoning) = match envelope.event {
                                GenerationStreamEvent::TextDelta { text } => (Some(text), None),
                                GenerationStreamEvent::ReasoningDelta { text } => {
                                    (None, Some(text))
                                }
                            };
                            sink.emit(dto::GenerationEvent::Delta {
                                turn_id: turn_id.clone(),
                                text,
                                reasoning,
                            });
                        }
                    }));
                    stream_sink = Some(sink_id);
                }
                Err(error) => {
                    tracing::warn!(%error, turn_id = %next.turn_id, "generation stream could not be registered");
                }
            }
        }
        let outcome = runner
            .execute(
                ConversationGenerationExecutionRequest {
                    conversation_id: next.conversation_id,
                    turn_id: next.turn_id,
                    attempt_id: next.attempt_id,
                    worker_id: self.worker_id,
                    lease_for: GENERATION_LEASE,
                    resources: self.resources,
                    runtime: ConversationGenerationRuntimeInput {
                        stream_sink,
                        prompt_values: Default::default(),
                    },
                    cancellation: CancellationToken::new(),
                    cancellation_reason: CancellationReason::Shutdown,
                },
                context.clock(),
            )
            .await;
        if let Some(sink_id) = stream_sink {
            if let Err(error) = backend.inference_runtime().unregister_stream(sink_id) {
                tracing::warn!(%error, "generation stream could not be unregistered");
            }
        }
        if let Some(forwarder) = forwarder
            && let Err(error) = forwarder.await
        {
            tracing::warn!(%error, "generation stream forwarder stopped");
        }
        if let Err(error) = &outcome {
            tracing::error!(%error, turn_id = %next.turn_id, "conversation generation failed to run");
        }
        if let Some(event) = settled_event(database, next.turn_id)? {
            context.finish_stream(next.turn_id, event);
            context.emit(dto::ApiEvent::GenerationSettled {
                conversation_id: next.conversation_id.to_string(),
                turn_id: turn_label,
            });
        }
        outcome.map_err(|error| {
            api_error(
                lettuce_contracts::ApiErrorCode::Internal,
                format!("conversation generation failed to run: {error}"),
            )
        })?;
        Ok(true)
    }
}

/// The last event of a turn that reached a terminal state; `None` while it
/// can still run.
pub(crate) fn settled_event(
    database: &Database,
    turn_id: GenerationTurnId,
) -> Result<Option<dto::GenerationEvent>, ApiError> {
    let turn =
        ConversationReader::get_turn(database, turn_id).map_err(IntoApiError::into_api_error)?;
    let label = turn_id.to_string();
    let event = match turn.status {
        GenerationTurnStatus::Succeeded => {
            let Some(candidate_id) = turn
                .selected_candidate_id
                .or(turn.candidate_ids.last().copied())
            else {
                return Err(api_error(
                    lettuce_contracts::ApiErrorCode::Internal,
                    "a succeeded turn has no candidate",
                ));
            };
            let candidate = ConversationReader::get_candidate(database, candidate_id)
                .map_err(IntoApiError::into_api_error)?;
            dto::GenerationEvent::Completed {
                turn_id: label,
                message_id: candidate.message_id.to_string(),
            }
        }
        GenerationTurnStatus::Cancelled => dto::GenerationEvent::Cancelled { turn_id: label },
        GenerationTurnStatus::Failed | GenerationTurnStatus::Interrupted => match turn.failure {
            Some(GenerationFailureCode::Cancelled) => {
                dto::GenerationEvent::Cancelled { turn_id: label }
            }
            failure => dto::GenerationEvent::Failed {
                turn_id: label,
                code: failure_code(failure),
            },
        },
        GenerationTurnStatus::Created
        | GenerationTurnStatus::Preparing
        | GenerationTurnStatus::SelectingSpeaker
        | GenerationTurnStatus::ContextPrepared
        | GenerationTurnStatus::Running
        | GenerationTurnStatus::CancellationRequested
        | GenerationTurnStatus::Finalizing
        | GenerationTurnStatus::Recovering => return Ok(None),
    };
    Ok(Some(event))
}

const fn failure_code(failure: Option<GenerationFailureCode>) -> dto::GenerationFailureCode {
    match failure {
        Some(GenerationFailureCode::InvalidConversation) => {
            dto::GenerationFailureCode::InvalidConversation
        }
        Some(GenerationFailureCode::MissingModel) => dto::GenerationFailureCode::MissingModel,
        Some(GenerationFailureCode::ContextUnavailable) => {
            dto::GenerationFailureCode::ContextUnavailable
        }
        Some(GenerationFailureCode::SpeakerUnavailable) => {
            dto::GenerationFailureCode::SpeakerUnavailable
        }
        Some(GenerationFailureCode::ProviderUnavailable) => {
            dto::GenerationFailureCode::ProviderUnavailable
        }
        Some(GenerationFailureCode::ProviderRejected) => {
            dto::GenerationFailureCode::ProviderRejected
        }
        Some(GenerationFailureCode::EmptyOutput) => dto::GenerationFailureCode::EmptyOutput,
        Some(GenerationFailureCode::TimedOut) => dto::GenerationFailureCode::TimedOut,
        Some(GenerationFailureCode::RecoveryUnavailable) => {
            dto::GenerationFailureCode::RecoveryUnavailable
        }
        Some(GenerationFailureCode::Cancelled | GenerationFailureCode::Internal) | None => {
            dto::GenerationFailureCode::Internal
        }
    }
}

/// Used without a media store: a reply carrying images fails instead of
/// losing them.
struct UnstoredReplyMedia;

impl ReplyMediaStore for UnstoredReplyMedia {
    fn store_reply_image(
        &self,
        _asset_id: lettuce_types::AssetId,
        _origin: ReplyImageOrigin,
        _bytes: &[u8],
    ) -> Result<lettuce_types::AssetId, lettuce_media::MediaStoreError> {
        Err(lettuce_media::MediaStoreError::CatalogFailure)
    }

    fn discard_reply_image(
        &self,
        _asset_id: lettuce_types::AssetId,
        _now: lettuce_types::TimestampMillis,
    ) {
    }
}
