use super::{ApiContext, error::parse_id};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_types::{RequestId, TimestampMillis};

pub async fn usage_clear_before(
    context: &ApiContext,
    request: dto::UsageClearBeforeRequest,
) -> Result<dto::UsageCleared, ApiError> {
    context
        .blocking(move |context| {
            let key: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
            let removed = context
                .backend()
                .database()
                .commit_api_operation::<u64, ApiOperationError>(
                    "usage_clear_before",
                    &key.to_string(),
                    &request.before.to_string(),
                    context.now(),
                    |transaction| {
                        transaction.clear_usage_before(TimestampMillis::new(request.before))
                    },
                )
                .map_err(|error| ApiError {
                    code: match error {
                        ApiOperationError::Conflict => ApiErrorCode::Conflict,
                        ApiOperationError::InvalidData => ApiErrorCode::InvalidInput,
                        ApiOperationError::Storage => ApiErrorCode::Unavailable,
                    },
                    message: error.to_string(),
                    details: Some(dto::ApiErrorDetails::UsageStorage),
                })?;
            Ok(dto::UsageCleared { removed })
        })
        .await
}
