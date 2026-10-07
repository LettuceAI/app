use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_jobs::{JobState, JobStore};
use lettuce_memory::{
    MemoryCycleRevert, MemoryCycleRevertError, MemoryReadRepository, MemoryToolOutcome,
};
use lettuce_types::{ConversationId, DynamicMemoryRunId, ModelProfileId};
use serde::{Serialize, de::DeserializeOwned};

use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::memory::{EditFailure, live_conversation};
use super::memory_read::{failure, latest_memory_job};
use super::{ApiContext, mapping, messages};
use crate::companion::companion_memory_host::{CompanionMemoryHostError, MemoryGate};

fn gate_error(gate: MemoryGate) -> ApiError {
    let (code, reason) = match gate {
        MemoryGate::NotDynamic => (ApiErrorCode::Unsupported, dto::MemoryGateReason::NotDynamic),
        MemoryGate::Disabled => (ApiErrorCode::Unsupported, dto::MemoryGateReason::Disabled),
        MemoryGate::NothingToSummarise => (
            ApiErrorCode::Unavailable,
            dto::MemoryGateReason::NothingToSummarise,
        ),
        MemoryGate::CycleRunning => (ApiErrorCode::Busy, dto::MemoryGateReason::CycleRunning),
    };
    ApiError {
        code,
        message: gate.to_string(),
        details: Some(ApiErrorDetails::MemoryGate { gate: reason }),
    }
}

fn host_error(
    context: &ApiContext,
    conversation_id: ConversationId,
    error: CompanionMemoryHostError,
) -> ApiError {
    match error {
        CompanionMemoryHostError::Gated(gate) => gate_error(gate),
        CompanionMemoryHostError::Conversation(error) => error.into_api_error(),
        CompanionMemoryHostError::PendingRewind(error) => {
            messages::delete_after_error(context, conversation_id, error)
        }
        error => api_error(ApiErrorCode::Internal, error.to_string()),
    }
}

/// Runs `action` once per operation key: the same request replays the
/// recorded result, another request under the key is `Conflict`. A crash
/// between the action and its receipt repeats the action, which every caller
/// makes idempotent.
async fn controlled<T, R>(
    context: &ApiContext,
    command: &'static str,
    key: &str,
    request: &R,
    action: impl FnOnce(&ApiContext) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError>
where
    T: Serialize + DeserializeOwned + Send + 'static,
    R: Serialize,
{
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "memory request could not be encoded",
        )
    })?);
    let operation = messages::operation(key.to_owned(), &[command.as_bytes(), &bytes])?;
    let key = key.to_owned();
    context
        .blocking(move |context| {
            let database = context.backend().database();
            if let Some(receipt) = database
                .lookup_api_operation(command, &key)
                .map_err(|error| EditFailure::from(error).0)?
            {
                if receipt.request_digest != operation.request_digest.as_str() {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the operation key was used for another request",
                    ));
                }
                return serde_json::from_value(receipt.result)
                    .map_err(|_| api_error(ApiErrorCode::Internal, "memory receipt is invalid"));
            }
            let result = action(context)?;
            database
                .commit_api_operation(
                    command,
                    &key,
                    operation.request_digest.as_str(),
                    context.now(),
                    |_| Ok::<_, EditFailure>(result),
                )
                .map_err(|error| error.0)
        })
        .await
}

async fn start_cycle(
    context: &ApiContext,
    command: &'static str,
    conversation_id: &str,
    key: &str,
    request: &impl Serialize,
    model_profile_id: Option<ModelProfileId>,
) -> Result<dto::JobAccepted, ApiError> {
    let conversation_id: ConversationId = parse_id(conversation_id, "conversation_id")?;
    let accepted = controlled(context, command, key, request, move |context| {
        let database = context.backend().database();
        live_conversation(database, conversation_id)?;
        let embedding = context.embedding();
        let admission = context
            .backend()
            .companion_memory_host(embedding.as_ref(), context.inference())
            .trigger_admit(
                conversation_id,
                model_profile_id,
                model_profile_id.is_some(),
                context.now(),
            )
            .map_err(|error| host_error(context, conversation_id, error))?;
        Ok(dto::JobAccepted {
            job_id: admission.job.id.to_string(),
        })
    })
    .await?;
    context.jobs().wake();
    Ok(accepted)
}

/// Starts a forced memory cycle over the conversation's most recent window
/// and answers a pending approval. A conversation that does not run dynamic
/// memory, an empty dialogue and a cycle already running are refused with the
/// reason named.
pub async fn memory_trigger(
    context: &ApiContext,
    request: dto::MemoryTriggerRequest,
) -> Result<dto::JobAccepted, ApiError> {
    start_cycle(
        context,
        "memory_trigger",
        &request.conversation_id,
        &request.client_operation_id,
        &request,
        None,
    )
    .await
}

/// A forced cycle with a different summarisation model. A chosen model also
/// becomes the default after the cycle succeeds, as the legacy retry did.
pub async fn memory_retry(
    context: &ApiContext,
    request: dto::MemoryRetryRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let model = request
        .model_profile_id
        .as_deref()
        .map(|value| parse_id::<ModelProfileId>(value, "model_profile_id"))
        .transpose()?;
    if let Some(model) = model {
        let conversation: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let aggregate =
                    lettuce_conversations::ConversationReader::get(database, conversation)
                        .map_err(IntoApiError::into_api_error)?;
                if matches!(
                    aggregate.conversation.kind,
                    lettuce_conversations::ConversationKind::Group(_)
                ) {
                    return Err(invalid_field(
                        "model_profile_id",
                        "group chats retry with their configured model",
                    ));
                }
                lettuce_models::ModelProfileRepository::get(database, model)
                    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the model was not found"))?;
                Ok(())
            })
            .await?;
    }
    start_cycle(
        context,
        "memory_retry",
        &request.conversation_id,
        &request.client_operation_id,
        &request,
        model,
    )
    .await
}

/// Declines the pending approval of an ask-first cycle; the next prompt waits
/// a full interval. Nothing pending is not an error.
pub async fn memory_skip(
    context: &ApiContext,
    request: dto::MemorySkipRequest,
) -> Result<(), ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    controlled(
        context,
        "memory_skip",
        &request.client_operation_id,
        &request,
        move |context| {
            live_conversation(context.backend().database(), conversation_id)?;
            let embedding = context.embedding();
            context
                .backend()
                .companion_memory_host(embedding.as_ref(), context.inference())
                .skip(conversation_id, context.now())
                .map(|_| ())
                .map_err(|error| host_error(context, conversation_id, error))
        },
    )
    .await
}

struct RevertFailure(ApiError);

impl From<lettuce_database::ApiOperationError> for RevertFailure {
    fn from(error: lettuce_database::ApiOperationError) -> Self {
        Self(EditFailure::from(error).0)
    }
}

impl From<MemoryCycleRevertError> for RevertFailure {
    fn from(error: MemoryCycleRevertError) -> Self {
        Self(match error {
            MemoryCycleRevertError::NotFound => {
                api_error(ApiErrorCode::NotFound, error.to_string())
            }
            MemoryCycleRevertError::Conflict | MemoryCycleRevertError::AlreadyReverted => {
                api_error(ApiErrorCode::Conflict, error.to_string())
            }
            MemoryCycleRevertError::Running => api_error(ApiErrorCode::Busy, error.to_string()),
            MemoryCycleRevertError::NothingToRevert | MemoryCycleRevertError::Invalid => {
                api_error(ApiErrorCode::InvalidInput, error.to_string())
            }
            MemoryCycleRevertError::Dependent { later_run_id } => ApiError {
                code: ApiErrorCode::Conflict,
                message: error.to_string(),
                details: Some(ApiErrorDetails::MemoryCycleDependent {
                    later_run_id: later_run_id.to_string(),
                }),
            },
            MemoryCycleRevertError::Storage => api_error(ApiErrorCode::Internal, error.to_string()),
        })
    }
}

impl From<lettuce_memory::MemoryRepositoryError> for RevertFailure {
    fn from(error: lettuce_memory::MemoryRepositoryError) -> Self {
        Self(super::memory::memory_error(error))
    }
}

/// Hides the failure the status shows until a newer cycle fails. Nothing to
/// dismiss is not an error.
pub async fn memory_error_dismiss(
    context: &ApiContext,
    request: dto::MemoryErrorDismissRequest,
) -> Result<(), ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "memory request could not be encoded",
        )
    })?);
    let operation = messages::operation(
        request.client_operation_id.clone(),
        &[b"memory_error_dismiss", &bytes],
    )?;
    let key = request.client_operation_id;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let conversation = live_conversation(database, conversation_id)?;
            let scope = database
                .read_memory_scope(conversation_id, conversation.revision)
                .map_err(super::memory::memory_error)?;
            let failed = latest_memory_job(context, &scope)?.filter(|job| {
                scope.dismissed_job != Some(job.id)
                    && matches!(job.state, JobState::Failed | JobState::Interrupted)
            });
            database
                .commit_api_operation(
                    "memory_error_dismiss",
                    &key,
                    operation.request_digest.as_str(),
                    context.now(),
                    |scope| {
                        if let Some(job) = &failed {
                            scope
                                .dismiss_memory_error(conversation_id, job.id, context.now())
                                .map_err(EditFailure::from)?;
                        }
                        Ok::<_, EditFailure>(())
                    },
                )
                .map_err(|error| error.0)
        })
        .await
}

fn action(outcome: &MemoryToolOutcome) -> dto::MemoryCycleAction {
    use dto::MemoryCycleActionKind as K;
    let (kind, memory_id, text) = match outcome {
        MemoryToolOutcome::Created {
            id,
            short_id,
            memories,
        } => (
            K::Created,
            Some(id.to_string()),
            memories
                .iter()
                .find(|memory| memory.short_id == *short_id)
                .map(|memory| memory.text.clone()),
        ),
        MemoryToolOutcome::DuplicateSkipped { existing_id, .. } => {
            (K::DuplicateSkipped, Some(existing_id.to_string()), None)
        }
        MemoryToolOutcome::Deleted { id, text, .. } => {
            (K::Deleted, Some(id.to_string()), Some(text.clone()))
        }
        MemoryToolOutcome::SoftDeleted { id, text, .. } => {
            (K::SoftDeleted, Some(id.to_string()), Some(text.clone()))
        }
        MemoryToolOutcome::Pinned { id, .. } => (K::Pinned, Some(id.to_string()), None),
        MemoryToolOutcome::Unpinned { id, .. } => (K::Unpinned, Some(id.to_string()), None),
        MemoryToolOutcome::TargetNotFound { .. } => (K::TargetNotFound, None, None),
        MemoryToolOutcome::Done { .. } => (K::Done, None, None),
        MemoryToolOutcome::Rejected { .. } => (K::Rejected, None, None),
        MemoryToolOutcome::Skipped { .. } => (K::Skipped, None, None),
        MemoryToolOutcome::StoppedAfterDone => (K::StoppedAfterDone, None, None),
    };
    dto::MemoryCycleAction {
        kind,
        memory_id,
        text,
    }
}

fn attempt_failure(
    code: lettuce_memory::DynamicMemoryAttemptFailureCode,
) -> dto::MemoryFailureCode {
    use dto::MemoryFailureCode as F;
    use lettuce_memory::DynamicMemoryAttemptFailureCode as C;
    match code {
        C::ProviderUnavailable => F::ProviderUnavailable,
        C::ProviderRejected => F::ProviderRejected,
        C::EmptyResponse => F::EmptyResponse,
        C::TimedOut => F::TimedOut,
        C::RoundLimit => F::RoundLimit,
        C::Internal => F::Internal,
    }
}

/// The conversation's cycles, newest first, with what each did, why it
/// failed and whether it can be reverted.
pub async fn memory_cycles(
    context: &ApiContext,
    request: dto::MemoryCyclesRequest,
) -> Result<dto::MemoryCyclePage, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let after = request
        .cursor
        .as_deref()
        .map(|cursor| parse_id::<DynamicMemoryRunId>(cursor, "cursor"))
        .transpose()?;
    let limit = usize::from(mapping::page_limit(request.limit).get());
    context
        .blocking(move |context| {
            use lettuce_memory::{DynamicMemoryAttemptStatus as S, cycle_revert_blocker};
            let database = context.backend().database();
            let conversation = live_conversation(database, conversation_id)?;
            let scope = database
                .read_memory_scope(conversation_id, conversation.revision)
                .map_err(super::memory::memory_error)?;
            let mut newest_first = scope.cycles.iter().rev().collect::<Vec<_>>();
            if let Some(after) = after {
                let position = newest_first
                    .iter()
                    .position(|cycle| cycle.run.id == after)
                    .ok_or_else(|| invalid_field("cursor", "the cursor is not valid here"))?;
                newest_first.drain(..=position);
            }
            let next_cursor =
                (newest_first.len() > limit).then(|| newest_first[limit - 1].run.id.to_string());
            newest_first.truncate(limit);
            let items = newest_first
                .into_iter()
                .map(|cycle| {
                    let attempt = &cycle.latest_attempt;
                    let job = JobStore::get(database, attempt.job_id)
                        .map_err(IntoApiError::into_api_error)?;
                    let status = match attempt.status {
                        S::Created => dto::MemoryCycleStatus::Queued,
                        S::Processing => dto::MemoryCycleStatus::Processing,
                        S::Succeeded => dto::MemoryCycleStatus::Complete,
                        S::Failed => dto::MemoryCycleStatus::Failed,
                        S::Cancelled => dto::MemoryCycleStatus::Cancelled,
                        S::Interrupted => dto::MemoryCycleStatus::Interrupted,
                    };
                    let failure = job
                        .as_ref()
                        .and_then(failure)
                        .or_else(|| attempt.failure.map(attempt_failure));
                    let blocked_by = (!cycle.reverted && !cycle.in_flight() && cycle.recorded())
                        .then(|| cycle_revert_blocker(&scope.cycles, cycle.run.id))
                        .flatten();
                    Ok(dto::MemoryCycleView {
                        run_id: cycle.run.id.to_string(),
                        job_id: attempt.job_id.to_string(),
                        started_at: cycle.run.created_at.get(),
                        label: format!(
                            "{}-{}",
                            cycle.run.summary_window.start, cycle.run.summary_window.end
                        ),
                        window_start: cycle.run.summary_window.start,
                        window_end: cycle.run.summary_window.end,
                        status,
                        failure: (status == dto::MemoryCycleStatus::Failed
                            || status == dto::MemoryCycleStatus::Interrupted)
                            .then_some(failure)
                            .flatten(),
                        summary: cycle
                            .checkpoint
                            .as_ref()
                            .map(|summary| summary.text.clone()),
                        actions: cycle
                            .results
                            .iter()
                            .map(|result| action(&result.outcome))
                            .collect(),
                        reverted: cycle.reverted,
                        revertable: !cycle.reverted
                            && !cycle.in_flight()
                            && cycle.recorded()
                            && blocked_by.is_none(),
                        blocked_by: blocked_by.map(|id| id.to_string()),
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            Ok(dto::MemoryCyclePage { items, next_cursor })
        })
        .await
}

/// Undoes one finished cycle's recorded outcomes and the summary it
/// published. A cycle that a later cycle started from is refused with
/// `Conflict` naming that cycle.
pub async fn memory_cycle_revert(
    context: &ApiContext,
    request: dto::MemoryCycleRevertRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let run_id: DynamicMemoryRunId = parse_id(&request.run_id, "run_id")?;
    let expected_revision = messages::expected_revision(request.expected_revision)?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "memory request could not be encoded",
        )
    })?);
    let operation = messages::operation(
        request.client_operation_id.clone(),
        &[b"memory_cycle_revert", &bytes],
    )?;
    let key = request.client_operation_id;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            if database
                .lookup_api_operation("memory_cycle_revert", &key)
                .map_err(|error| EditFailure::from(error).0)?
                .is_none()
            {
                live_conversation(database, conversation_id)?;
            }
            database
                .commit_api_operation(
                    "memory_cycle_revert",
                    &key,
                    operation.request_digest.as_str(),
                    context.now(),
                    |scope| {
                        let record = scope.revert_memory_cycle(&MemoryCycleRevert {
                            conversation_id,
                            run_id,
                            expected_revision,
                            at: context.now(),
                        })?;
                        Ok::<_, RevertFailure>(dto::MemoryEditResult {
                            revision: record.resulting_revision.get(),
                            memory_id: None,
                        })
                    },
                )
                .map_err(|error: RevertFailure| error.0)
        })
        .await
}
