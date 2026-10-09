use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

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
