use super::{
    ApiContext,
    error::{invalid_field, parse_id},
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};

fn storage(error: impl std::fmt::Display) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::MetricsUnavailable),
    }
}

fn view(metric: lettuce_database::LlmGenerationMetric) -> dto::LlmMetricView {
    dto::LlmMetricView {
        id: metric.id,
        created_at: metric.created_at,
        summary: metric.summary,
        samples: metric.samples,
    }
}

pub async fn llm_metrics_list(
    context: &ApiContext,
    request: dto::LlmMetricsListRequest,
) -> Result<dto::LlmMetricsPage, ApiError> {
    context
        .blocking(move |context| {
            let cursor = request
                .cursor
                .map(|cursor| {
                    serde_json::from_str::<(i64, String)>(&cursor)
                        .map_err(|_| invalid_field("cursor", "invalid metrics cursor"))
                })
                .transpose()?;
            let limit = request.limit.clamp(1, 2000) as usize;
            let mut items = context
                .backend()
                .database()
                .llm_generation_metrics_page(cursor, Some(limit + 1))
                .map_err(storage)?;
            let more = items.len() > limit;
            items.truncate(limit);
            let next_cursor = if more {
                items
                    .last()
                    .map(|row| serde_json::to_string(&(row.created_at, &row.id)).map_err(storage))
                    .transpose()?
            } else {
                None
            };
            Ok(dto::LlmMetricsPage {
                items: items.into_iter().map(view).collect(),
                next_cursor,
            })
        })
        .await
}

pub async fn llm_metrics_get(
    context: &ApiContext,
    request: dto::LlmMetricGetRequest,
) -> Result<Option<dto::LlmMetricView>, ApiError> {
    context
        .blocking(move |context| {
            if request.id.trim().is_empty() {
                return Err(invalid_field("id", "metric id is empty"));
            }
            context
                .backend()
                .database()
                .llm_generation_metric(&request.id)
                .map(|metric| metric.map(view))
                .map_err(storage)
        })
        .await
}

pub async fn llm_metrics_for_message(
    context: &ApiContext,
    request: dto::LlmMetricForMessageRequest,
) -> Result<Option<dto::LlmMetricView>, ApiError> {
    context
        .blocking(move |context| {
            let conversation: lettuce_types::ConversationId =
                parse_id(&request.conversation_id, "conversation_id")?;
            let message: lettuce_types::MessageId = parse_id(&request.message_id, "message_id")?;
            context
                .backend()
                .database()
                .llm_generation_metric_for_message(&conversation.to_string(), &message.to_string())
                .map(|metric| metric.map(view))
                .map_err(storage)
        })
        .await
}

pub async fn llm_metrics_clear(
    context: &ApiContext,
    request: dto::LlmMetricsClearRequest,
) -> Result<dto::LlmMetricsCleared, ApiError> {
    context
        .blocking(move |context| {
            let key: lettuce_types::RequestId =
                parse_id(&request.client_operation_id, "client_operation_id")?;
            let removed = context
                .backend()
                .database()
                .commit_api_operation::<u64, lettuce_database::ApiOperationError>(
                    "llm_metrics_clear",
                    &key.to_string(),
                    "all-v1",
                    context.now(),
                    |transaction| transaction.clear_llm_metrics(),
                )
                .map_err(|error| match error {
                    lettuce_database::ApiOperationError::Conflict => ApiError {
                        code: ApiErrorCode::Conflict,
                        message: error.to_string(),
                        details: None,
                    },
                    _ => storage(error),
                })?;
            Ok(dto::LlmMetricsCleared { removed })
        })
        .await
}
