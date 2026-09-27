//! Deleting a conversation, stopping what still runs for it first.

use std::time::Duration;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ConversationOverviewReader, ConversationReader, ConversationRepositoryError,
};
use lettuce_database::PurgeError;
use lettuce_jobs::{
    CancellationReason, JobKind, JobMutation, JobQuery, JobState, JobStore, SubjectId,
};
use lettuce_types::{ConversationId, PageLimit, PageRequest};

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};
use crate::{ConversationGenerationCancellationOutcome, HardDeleteError, MediaGarbageScope};

/// How long a delete waits for the work it cancelled to settle. The runners
/// settle a cancelled turn or memory run as soon as they see its token, so
/// this only ends a wait on a runner that is stuck.
pub(crate) const DELETE_SETTLE_LIMIT: Duration = Duration::from_secs(30);

/// Deletes a conversation and everything recorded for it, then the media
/// it alone used. A turn or memory run still going is cancelled first and
/// the delete waits for it to settle, woken by committed job and
/// conversation changes, so the caller never sees `Busy` for work it can
/// stop. Deleting a conversation that is already gone succeeds.
pub async fn conversation_delete(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<(), ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let mut changes = context.committed_changes();
    let deadline = tokio::time::Instant::now() + DELETE_SETTLE_LIMIT;
    loop {
        changes.borrow_and_update();
        let deleted = context
            .blocking(move |context| delete_step(context, conversation_id))
            .await?;
        if deleted {
            context.wake_workers();
            context.jobs().wake();
            return Ok(());
        }
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Err(api_error(ApiErrorCode::Internal, "the change signal closed"));
                }
            }
            () = tokio::time::sleep_until(deadline) => {
                return Err(api_error(
                    ApiErrorCode::Busy,
                    "the conversation's running work did not stop",
                ));
            }
        }
    }
}

/// Cancels what still runs for the conversation and tries the purge once.
/// Answers whether the conversation is gone.
fn delete_step(context: &ApiContext, conversation_id: ConversationId) -> Result<bool, ApiError> {
    match ConversationReader::get(context.backend().database(), conversation_id) {
        Ok(_) => {}
        Err(ConversationRepositoryError::NotFound) => return Ok(true),
        Err(error) => return Err(error.into_api_error()),
    }
    cancel_conversation_work(context, conversation_id)?;
    match purge(context, conversation_id) {
        Ok(()) | Err(PurgeError::NotFound) => Ok(true),
        Err(PurgeError::Busy) => Ok(false),
        Err(error) => Err(api_error(ApiErrorCode::Internal, error.to_string())),
    }
}

fn purge(context: &ApiContext, conversation_id: ConversationId) -> Result<(), PurgeError> {
    let database = context.backend().database();
    match (context.media(), context.database_files()) {
        (Some(store), Some(files)) => crate::delete_conversation(
            database,
            &MediaGarbageScope {
                store,
                location: &files.location,
                open_database: &files.active,
            },
            conversation_id,
            context.now(),
        )
        .map(|_| ())
        .map_err(|error| match error {
            HardDeleteError::Purge(error) => error,
            HardDeleteError::Media(_) => PurgeError::Storage,
        }),
        _ => database
            .purge_conversation(conversation_id, context.now())
            .map(|_| ()),
    }
}

/// Requests cancellation of the conversation's unsettled turn and memory
/// runs. A queued job is cancelled at once; a running one is signalled and
/// settles through its runner.
pub(crate) fn cancel_conversation_work(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    if let Some(turn_id) = ConversationOverviewReader::live_turn(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
    {
        let turn = ConversationReader::get_turn(database, turn_id)
            .map_err(IntoApiError::into_api_error)?;
        if let Some(job_id) = turn
            .attempts
            .iter()
            .filter(|attempt| attempt.job_id.is_some())
            .max_by_key(|attempt| attempt.ordinal)
            .and_then(|attempt| attempt.job_id)
            && let ConversationGenerationCancellationOutcome::QueuedCancelled(_) = context
                .backend()
                .conversation_generation_cancellation()
                .cancel(job_id, CancellationReason::User, context.now())
                .map_err(IntoApiError::into_api_error)?
        {
            context.settle_turn(
                conversation_id,
                turn_id,
                dto::GenerationEvent::Cancelled {
                    turn_id: turn_id.to_string(),
                },
            );
        }
    }
    let subject = SubjectId::new(conversation_id.to_string()).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "a conversation id is not a job subject",
        )
    })?;
    let mut cursor = None;
    loop {
        let page = JobStore::list(
            database,
            JobQuery {
                state: None,
                kind: Some(JobKind::MemoryExtraction),
                subject: Some(subject.clone()),
                page: PageRequest {
                    cursor: cursor.take(),
                    limit: PageLimit::new(200),
                },
            },
        )
        .map_err(IntoApiError::into_api_error)?;
        for job in page.items {
            if job.state.is_terminal() {
                continue;
            }
            let at = context.now().max(job.updated_at);
            let requested = if job.state == JobState::CancellationRequested {
                job
            } else {
                database
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: job.id,
                        reason: CancellationReason::User,
                        at,
                    })
                    .map_err(IntoApiError::into_api_error)?
            };
            let signalled = context
                .backend()
                .inference_runtime()
                .request_cancel(requested.id)
                .unwrap_or(false);
            if !context.jobs().cancel_running(requested.id)
                && !signalled
                && requested.state == JobState::CancellationRequested
                && requested.claim.is_none()
            {
                database
                    .append_and_transition(JobMutation::FinishQueuedCancellation {
                        id: requested.id,
                        at: at.max(requested.updated_at),
                    })
                    .map_err(IntoApiError::into_api_error)?;
            }
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(()),
        }
    }
}
