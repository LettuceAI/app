use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn logs_diagnostics_report(
    context: State<'_, ApiContextSlot>,
) -> Result<String, ApiError> {
    let context = context.snapshot()?;
    api::logs_diagnostics_report(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn logs_list(context: State<'_, ApiContextSlot>) -> Result<dto::LogsList, ApiError> {
    let context = context.snapshot()?;
    api::logs_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_read_page(
    context: State<'_, ApiContextSlot>,
    request: dto::LogReadPageRequest,
) -> Result<dto::LogPageView, ApiError> {
    let context = context.snapshot()?;
    api::log_read_page(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_search(
    context: State<'_, ApiContextSlot>,
    request: dto::LogSearchRequest,
) -> Result<dto::LogSearchView, ApiError> {
    let context = context.snapshot()?;
    api::log_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_relevant_lines(
    context: State<'_, ApiContextSlot>,
    request: dto::LogRelevantLinesRequest,
) -> Result<dto::LogSearchView, ApiError> {
    let context = context.snapshot()?;
    api::log_relevant_lines(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::LogNameRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::log_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn logs_clear(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::logs_clear(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_export(
    context: State<'_, ApiContextSlot>,
    request: dto::LogExportRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::log_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_append(
    context: State<'_, ApiContextSlot>,
    request: dto::LogAppendRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::log_append(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_query(
    context: State<'_, ApiContextSlot>,
    request: dto::LogQueryRequest,
) -> Result<dto::LogQueryView, ApiError> {
    let context = context.snapshot()?;
    api::log_query(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_export_selection(
    context: State<'_, ApiContextSlot>,
    request: dto::LogExportSelectionRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::log_export_selection(&context, request).await
}
