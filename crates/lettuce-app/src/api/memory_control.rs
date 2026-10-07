use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_types::{ConversationId, ModelProfileId};
use serde::{Serialize, de::DeserializeOwned};

use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::memory::{EditFailure, live_conversation};
use super::{ApiContext, messages};
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
