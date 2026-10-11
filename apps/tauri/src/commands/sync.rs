use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn sync_share_start(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::SyncSharing, ApiError> {
    let context = context.snapshot()?;
    api::sync_share_start(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_share_approve(
    context: State<'_, ApiContextSlot>,
    request: dto::SyncShareApproveRequest,
) -> Result<dto::SyncStatus, ApiError> {
    let context = context.snapshot()?;
    api::sync_share_approve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_share_begin(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::SyncStatus, ApiError> {
    let context = context.snapshot()?;
    api::sync_share_begin(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_connect(
    context: State<'_, ApiContextSlot>,
    request: dto::SyncConnectRequest,
) -> Result<dto::SyncStatus, ApiError> {
    let context = context.snapshot()?;
    api::sync_connect(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_stop(context: State<'_, ApiContextSlot>) -> Result<dto::SyncStatus, ApiError> {
    let context = context.snapshot()?;
    api::sync_stop(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_status(context: State<'_, ApiContextSlot>) -> Result<dto::SyncStatus, ApiError> {
    let context = context.snapshot()?;
    api::sync_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_conflicts_list(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::SyncConflictsList, ApiError> {
    let context = context.snapshot()?;
    api::sync_conflicts_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_conflict_resolve(
    context: State<'_, ApiContextSlot>,
    request: dto::SyncConflictResolveRequest,
) -> Result<dto::SyncConflictResolution, ApiError> {
    let context = context.snapshot()?;
    api::sync_conflict_resolve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_fork_resolve(
    context: State<'_, ApiContextSlot>,
    request: dto::SyncForkResolveRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::sync_fork_resolve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn legacy_conflict_dismiss(
    context: State<'_, ApiContextSlot>,
    request: dto::LegacyConflictDismissRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::legacy_conflict_dismiss(&context, request).await
}
