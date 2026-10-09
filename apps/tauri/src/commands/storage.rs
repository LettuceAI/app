use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn storage_optimize(
    context: State<'_, ApiContext>,
    request: dto::StorageOptimizeRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::storage_optimize(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_library_list(
    context: State<'_, ApiContext>,
    request: dto::MediaLibraryListRequest,
) -> Result<dto::MediaLibraryPage, ApiError> {
    api::media_library_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_library_remove(
    context: State<'_, ApiContext>,
    request: dto::MediaLibraryRemoveRequest,
) -> Result<(), ApiError> {
    api::media_library_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_save_to(
    context: State<'_, ApiContext>,
    request: dto::MediaSaveToRequest,
) -> Result<(), ApiError> {
    api::media_save_to(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_database_files_list(
    context: State<'_, ApiContext>,
) -> Result<Vec<dto::DatabaseFileView>, ApiError> {
    api::storage_database_files_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_database_file_delete(
    context: State<'_, ApiContext>,
    request: dto::DatabaseFileDeleteRequest,
) -> Result<(), ApiError> {
    api::storage_database_file_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_summary(
    context: State<'_, ApiContext>,
) -> Result<dto::StorageSummary, ApiError> {
    api::storage_summary(&context).await
}
