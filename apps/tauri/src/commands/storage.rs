use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn app_data_reset(
    context: State<'_, ApiContextSlot>,
    request: dto::AppDataResetRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::app_data_reset(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_optimize(
    context: State<'_, ApiContextSlot>,
    request: dto::StorageOptimizeRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::storage_optimize(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_library_list(
    context: State<'_, ApiContextSlot>,
    request: dto::MediaLibraryListRequest,
) -> Result<dto::MediaLibraryPage, ApiError> {
    let context = context.snapshot()?;
    api::media_library_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_references(
    context: State<'_, ApiContextSlot>,
    request: dto::MediaReferencesRequest,
) -> Result<Vec<dto::MediaReferenceView>, ApiError> {
    let context = context.snapshot()?;
    api::media_references(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_library_remove(
    context: State<'_, ApiContextSlot>,
    request: dto::MediaLibraryRemoveRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::media_library_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn media_save_to(
    context: State<'_, ApiContextSlot>,
    request: dto::MediaSaveToRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::media_save_to(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_database_files_list(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::DatabaseFileView>, ApiError> {
    let context = context.snapshot()?;
    api::storage_database_files_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_database_file_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::DatabaseFileDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::storage_database_file_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn storage_summary(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::StorageSummary, ApiError> {
    let context = context.snapshot()?;
    api::storage_summary(&context).await
}
