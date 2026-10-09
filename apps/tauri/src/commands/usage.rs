use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn usage_query(
    context: State<'_, ApiContext>,
    request: dto::UsageQueryRequest,
) -> Result<dto::UsagePage, ApiError> {
    api::usage_query(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_stats(
    context: State<'_, ApiContext>,
    request: dto::UsageStatsRequest,
) -> Result<dto::UsageStats, ApiError> {
    api::usage_stats(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_export_csv(
    context: State<'_, ApiContext>,
    request: dto::UsageExportCsvRequest,
) -> Result<(), ApiError> {
    api::usage_export_csv(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_clear_before(
    context: State<'_, ApiContext>,
    request: dto::UsageClearBeforeRequest,
) -> Result<dto::UsageCleared, ApiError> {
    api::usage_clear_before(&context, request).await
}
