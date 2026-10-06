use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_types::ConversationId;

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};
use super::messages::operation;

struct CopyFailure(ApiError);

impl From<ApiOperationError> for CopyFailure {
    fn from(error: ApiOperationError) -> Self {
        Self(api_error(
            match error {
                ApiOperationError::Conflict => ApiErrorCode::Conflict,
                ApiOperationError::InvalidData | ApiOperationError::Storage => {
                    ApiErrorCode::Internal
                }
            },
            error.to_string(),
        ))
    }
}

pub async fn conversation_duplicate(
    context: &ApiContext,
    request: dto::ConversationDuplicateRequest,
) -> Result<dto::ConversationCopyResult, ApiError> {
    let source_conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    let request_bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "the duplicate request could not be encoded",
        )
    })?);
    let operation = operation(
        request.client_operation_id.clone(),
        &[b"duplicate", &request_bytes],
    )?;
    context
        .blocking(move |context| {
            let now = context.now();
            context
                .backend()
                .database()
                .commit_api_operation(
                    "conversation_duplicate",
                    &request.client_operation_id,
                    operation.request_digest.as_str(),
                    now,
                    |scope| {
                        let commit = scope
                            .duplicate_conversation(
                                &lettuce_conversations::DuplicateConversation {
                                    source_conversation_id,
                                    conversation_id: ConversationId::new(),
                                    title: request.title,
                                    with_messages: request.with_messages,
                                    operation: operation.clone(),
                                },
                                now,
                            )
                            .map_err(|error| CopyFailure(error.into_api_error()))?;
                        Ok(dto::ConversationCopyResult {
                            conversation_id: commit.value.conversation.id.to_string(),
                            revision: commit.value.conversation.revision.get(),
                        })
                    },
                )
                .map_err(|failure: CopyFailure| failure.0)
        })
        .await
}
