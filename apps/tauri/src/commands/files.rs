use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, AssetRef, AssetsIngestRequest, FileInspection, FilesInspectRequest,
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
