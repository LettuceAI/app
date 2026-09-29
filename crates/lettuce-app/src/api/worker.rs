use std::time::Duration;

use futures_util::FutureExt;

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
    ConversationGenerationExecutionOutcome, ConversationGenerationExecutionRequest,
    ConversationGenerationRuntimeInput, QueuedConversationGeneration, ReplyImageOrigin,
    ReplyMediaStore,
};

/// A claim outlives the longest provider request, so a slow reply is never
/// settled after its lease ran out.
const GENERATION_LEASE: Duration = Duration::from_secs(60 * 60);

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

    /// Runs until `shutdown` completes. A job already running finishes
    /// first; `ApiContext::begin_shutdown` cancels it and any job started
    /// after it. An idle worker, or one whose job could not be claimed,
    /// sleeps until a send, a cancellation or another scheduling path wakes
    /// it; no queued generation job waits for a later time. A failed step is
    /// retried after a delay growing from 250 ms to 30 s.
    pub async fn run(&self, shutdown: impl Future<Output = ()>) {
        drive(self, shutdown).await;
    }

    /// Runs the next queued job, if any; returns whether one ran. A job that
    /// could not be claimed, or had already ended, counts as not run.
    pub async fn run_once(&self) -> Result<bool, ApiError> {
        let context = &self.context;
        let next = {
            let backend = context.backend();
            let embedding = context.embedding();
            let runner = backend.prepared_conversation_generation_runner(
                embedding.as_ref(),
                context.inference(),
                &UnstoredReplyMedia,
            );
            runner
                .next_queued(&self.resources, context.now())
                .map_err(IntoApiError::into_api_error)?
        };
        context.finish_settled_streams()?;
        match next {
            Some(next) => self.run_queued(next).await,
            None => Ok(false),
        }
    }

    pub(super) async fn run_queued(
        &self,
        next: QueuedConversationGeneration,
    ) -> Result<bool, ApiError> {
        let context = &self.context;
        let backend = context.backend();
        let database = backend.database();
        let reply_media = context.media().map(|store| backend.reply_media(store));
        let reply_media: &dyn ReplyMediaStore = match &reply_media {
            Some(store) => store,
            None => &UnstoredReplyMedia,
        };
        let embedding = context.embedding();
        let runner = backend.prepared_conversation_generation_runner(
            embedding.as_ref(),
            context.inference(),
            reply_media,
        );
        let turn_label = next.turn_id.to_string();
        let (cancellation, _shutdown_link) = shutdown_child(context.shutdown_token());
        let mut stream_sink = None;
        let mut forwarder = None;
        let outcome = runner
            .execute_with(
                ConversationGenerationExecutionRequest {
                    conversation_id: next.conversation_id,
                    turn_id: next.turn_id,
                    attempt_id: next.attempt_id,
                    worker_id: self.worker_id,
                    lease_for: GENERATION_LEASE,
                    resources: self.resources,
                    runtime: ConversationGenerationRuntimeInput::default(),
                    cancellation,
                    cancellation_reason: CancellationReason::Shutdown,
                },
                context.clock(),
                || {
                    let sink = context.stream(next.turn_id)?;
                    sink.emit(dto::GenerationEvent::Started {
                        turn_id: turn_label.clone(),
                    });
                    let sink_id = RequestId::new();
                    match backend.inference_runtime().register_stream(sink_id) {
                        Ok(receiver) => {
                            forwarder = Some(tokio::spawn(forward_deltas(
                                receiver,
                                sink,
                                turn_label.clone(),
                            )));
                            stream_sink = Some(sink_id);
                            stream_sink
                        }
                        Err(error) => {
                            tracing::warn!(%error, turn_id = %next.turn_id, "generation stream could not be registered");
                            None
                        }
                    }
                },
            )
            .await;
        if let Some(sink_id) = stream_sink
            && let Err(error) = backend.inference_runtime().unregister_stream(sink_id)
        {
            tracing::warn!(%error, "generation stream could not be unregistered");
        }
        if let Some(forwarder) = forwarder
            && let Err(error) = forwarder.await
        {
            tracing::warn!(%error, "generation stream forwarder stopped");
        }
        let ran = match &outcome {
            Ok(
                ConversationGenerationExecutionOutcome::Settled(_)
                | ConversationGenerationExecutionOutcome::Replayed { .. },
            ) => true,
            Ok(
                ConversationGenerationExecutionOutcome::Terminal(_)
                | ConversationGenerationExecutionOutcome::NotClaimed(_),
            ) => false,
            Err(error) => {
                tracing::error!(%error, turn_id = %next.turn_id, "conversation generation failed to run");
                true
            }
        };
        if let Some(event) = settled_event(database, next.turn_id)? {
            if matches!(&event, dto::GenerationEvent::Completed { .. }) {
                let conversation_id = next.conversation_id;
                let turn_id = next.turn_id;
                context
                    .blocking(move |context| {
                        let turn =
                            ConversationReader::get_turn(context.backend().database(), turn_id)
                                .map_err(IntoApiError::into_api_error)?;
                        for candidate_id in turn.candidate_ids {
                            super::scenes::start_auto(
                                context,
                                conversation_id,
                                lettuce_conversations::SceneFollowUpTarget::Candidate(candidate_id),
                            )?;
                        }
                        Ok(())
                    })
                    .await?;
            }
            if ran {
                context.settle_turn(next.conversation_id, next.turn_id, event);
            } else if context.finish_stream(next.turn_id, event) {
                context.emit(dto::ApiEvent::GenerationSettled {
                    conversation_id: next.conversation_id.to_string(),
                    turn_id: turn_label,
                });
            }
        }
        outcome.map_err(|error| {
            api_error(
                lettuce_contracts::ApiErrorCode::Internal,
                format!("conversation generation failed to run: {error}"),
            )
        })?;
        Ok(ran)
    }
}

pub(super) const RETRY_MIN: Duration = Duration::from_millis(250);
pub(super) const RETRY_MAX: Duration = Duration::from_secs(30);

/// A worker loop's step and what it waits on between steps.
pub(super) trait WorkerStep {
    const LABEL: &'static str;

    /// Runs what is due; `true` when something ran and the next step should
    /// follow at once.
    async fn step(&self) -> Result<bool, ApiError>;

    /// Completes when new work may be there.
    async fn woken(&self);

    /// Completes when queued work becomes due; never by default.
    async fn due(&self) {
        std::future::pending::<()>().await;
    }
}

/// Runs `worker` until `shutdown`. After a step that ran something the next
/// follows at once; an idle worker waits for its wake-up or due time only;
/// a failed step, such as a storage error, is retried after a delay that
/// doubles from `RETRY_MIN` to `RETRY_MAX` and resets after a step succeeds.
pub(super) async fn drive<W: WorkerStep>(worker: &W, shutdown: impl Future<Output = ()>) {
    let shutdown = shutdown.fuse();
    futures_util::pin_mut!(shutdown);
    let mut retry: Option<Duration> = None;
    while (&mut shutdown).now_or_never().is_none() {
        match worker.step().await {
            Ok(true) => {
                retry = None;
                continue;
            }
            Ok(false) => retry = None,
            Err(error) => {
                let delay = retry.map_or(RETRY_MIN, |delay| (delay * 2).min(RETRY_MAX));
                retry = Some(delay);
                tracing::warn!(worker = W::LABEL, code = ?error.code, message = %error.message, ?delay, "worker step failed; retrying");
            }
        }
        let retry_after = async {
            match retry {
                Some(delay) => tokio::time::sleep(delay).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            () = &mut shutdown => break,
            () = worker.woken() => {}
            () = worker.due() => {}
            () = retry_after => {}
        }
    }
}

impl WorkerStep for ConversationGenerationWorker {
    const LABEL: &'static str = "conversation-generation";

    async fn step(&self) -> Result<bool, ApiError> {
        self.run_once().await
    }

    async fn woken(&self) {
        self.context.woken().await;
    }
}

/// A token cancelled with `parent`, including when `parent` already is;
/// the link ends when the returned guard drops.
fn shutdown_child(parent: &CancellationToken) -> (CancellationToken, ShutdownLink) {
    let child = CancellationToken::new();
    let link = link_to_shutdown(parent, child.clone());
    (child, link)
}

/// Cancels `child` with `parent`, including when `parent` already is; the
/// link ends when the returned guard drops.
pub(super) fn link_to_shutdown(
    parent: &CancellationToken,
    child: CancellationToken,
) -> ShutdownLink {
    if parent.is_cancelled() {
        child.cancel();
    }
    let parent = parent.clone();
    let link = tokio::spawn(async move {
        parent.cancelled().await;
        child.cancel();
    });
    ShutdownLink(link)
}

pub(super) struct ShutdownLink(tokio::task::JoinHandle<()>);

impl Drop for ShutdownLink {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn forward_deltas(
    mut receiver: lettuce_inference::InferenceStreamReceiver,
    sink: std::sync::Arc<dyn super::GenerationEventSink>,
    turn_id: String,
) {
    while let Some(envelope) = receiver.recv().await {
        let (text, reasoning) = match envelope.event {
            GenerationStreamEvent::TextDelta { text } => (Some(text), None),
            GenerationStreamEvent::ReasoningDelta { text } => (None, Some(text)),
        };
        sink.emit(dto::GenerationEvent::Delta {
            turn_id: turn_id.clone(),
            text,
            reasoning,
        });
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
        Some(GenerationFailureCode::EmbeddingUnavailable) => {
            dto::GenerationFailureCode::EmbeddingUnavailable
        }
        Some(GenerationFailureCode::Cancelled | GenerationFailureCode::Internal) | None => {
            dto::GenerationFailureCode::Internal
        }
    }
}

/// Used without a media store: a reply carrying images fails instead of
/// losing them.
pub(super) struct UnstoredReplyMedia;

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
