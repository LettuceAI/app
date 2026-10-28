use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_list(
    context: State<'_, ApiContextSlot>,
    request: dto::LlmMetricsListRequest,
) -> Result<dto::LlmMetricsPage, ApiError> {
    let context = context.snapshot()?;
    api::llm_metrics_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_get(
    context: State<'_, ApiContextSlot>,
    request: dto::LlmMetricGetRequest,
) -> Result<Option<dto::LlmMetricView>, ApiError> {
    let context = context.snapshot()?;
    api::llm_metrics_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_for_message(
    context: State<'_, ApiContextSlot>,
    request: dto::LlmMetricForMessageRequest,
) -> Result<Option<dto::LlmMetricView>, ApiError> {
    let context = context.snapshot()?;
    api::llm_metrics_for_message(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_usage(
    context: State<'_, ApiContextSlot>,
    request: dto::LlmMetricForMessageRequest,
) -> Result<Option<dto::MessageUsageView>, ApiError> {
    let context = context.snapshot()?;
    api::message_usage(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_clear(
    context: State<'_, ApiContextSlot>,
    request: dto::LlmMetricsClearRequest,
) -> Result<dto::LlmMetricsCleared, ApiError> {
    let context = context.snapshot()?;
    api::llm_metrics_clear(&context, request).await
}
