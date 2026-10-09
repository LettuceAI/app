use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn logs_diagnostics_report(context: State<'_, ApiContext>) -> Result<String, ApiError> {
    api::logs_diagnostics_report(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn logs_list(context: State<'_, ApiContext>) -> Result<dto::LogsList, ApiError> {
    api::logs_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_read_page(
    context: State<'_, ApiContext>,
    request: dto::LogReadPageRequest,
) -> Result<dto::LogPageView, ApiError> {
    api::log_read_page(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_search(
    context: State<'_, ApiContext>,
    request: dto::LogSearchRequest,
) -> Result<dto::LogSearchView, ApiError> {
    api::log_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_relevant_lines(
    context: State<'_, ApiContext>,
    request: dto::LogRelevantLinesRequest,
) -> Result<dto::LogSearchView, ApiError> {
    api::log_relevant_lines(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_delete(
    context: State<'_, ApiContext>,
    request: dto::LogNameRequest,
) -> Result<(), ApiError> {
    api::log_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn logs_clear(context: State<'_, ApiContext>) -> Result<(), ApiError> {
    api::logs_clear(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_export(
    context: State<'_, ApiContext>,
    request: dto::LogExportRequest,
) -> Result<(), ApiError> {
    api::log_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn log_append(
    context: State<'_, ApiContext>,
    request: dto::LogAppendRequest,
) -> Result<(), ApiError> {
    api::log_append(&context, request).await
}
