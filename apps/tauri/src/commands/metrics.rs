use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_list(
    context: State<'_, ApiContext>,
    request: dto::LlmMetricsListRequest,
) -> Result<dto::LlmMetricsPage, ApiError> {
    api::llm_metrics_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_get(
    context: State<'_, ApiContext>,
    request: dto::LlmMetricGetRequest,
) -> Result<Option<dto::LlmMetricView>, ApiError> {
    api::llm_metrics_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_for_message(
    context: State<'_, ApiContext>,
    request: dto::LlmMetricForMessageRequest,
) -> Result<Option<dto::LlmMetricView>, ApiError> {
    api::llm_metrics_for_message(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llm_metrics_clear(
    context: State<'_, ApiContext>,
    request: dto::LlmMetricsClearRequest,
) -> Result<dto::LlmMetricsCleared, ApiError> {
    api::llm_metrics_clear(&context, request).await
}
