use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, AssetRef, AssetsIngestRequest, FileInspection, FileSavePicked, FilesInspectRequest,
    FilesPickOpenRequest, FilesPickSaveRequest, FilesPicked,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn files_inspect(
    context: State<'_, ApiContext>,
    request: FilesInspectRequest,
) -> Result<FileInspection, ApiError> {
    api::files_inspect(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn assets_ingest(
    context: State<'_, ApiContext>,
    request: AssetsIngestRequest,
) -> Result<AssetRef, ApiError> {
    api::assets_ingest(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn files_pick_open(
    context: State<'_, ApiContext>,
    request: FilesPickOpenRequest,
) -> Result<FilesPicked, ApiError> {
    api::files_pick_open(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn files_pick_save(
    context: State<'_, ApiContext>,
    request: FilesPickSaveRequest,
) -> Result<FileSavePicked, ApiError> {
    api::files_pick_save(&context, request).await
}
