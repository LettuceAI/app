use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn usage_get(context: State<'_, ApiContextSlot>, request: dto::UsageGetRequest) -> Result<Option<dto::UsageRow>, ApiError> {
    let context = context.snapshot()?;
    api::usage_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_query(
    context: State<'_, ApiContextSlot>,
    request: dto::UsageQueryRequest,
) -> Result<dto::UsagePage, ApiError> {
    let context = context.snapshot()?;
    api::usage_query(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_stats(
    context: State<'_, ApiContextSlot>,
    request: dto::UsageStatsRequest,
) -> Result<dto::UsageStats, ApiError> {
    let context = context.snapshot()?;
    api::usage_stats(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_export_csv(
    context: State<'_, ApiContextSlot>,
    request: dto::UsageExportCsvRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::usage_export_csv(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_recalculate_costs(
    context: State<'_, ApiContextSlot>,
    request: dto::UsageRecalculateCostsRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::usage_recalculate_costs(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn usage_clear_before(
    context: State<'_, ApiContextSlot>,
    request: dto::UsageClearBeforeRequest,
) -> Result<dto::UsageCleared, ApiError> {
    let context = context.snapshot()?;
    api::usage_clear_before(&context, request).await
}
