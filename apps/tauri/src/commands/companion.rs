use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn companion_state_get(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionStateGetRequest,
) -> Result<Option<dto::CompanionStateView>, ApiError> {
    let context = context.snapshot()?;
    api::companion_state_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_get(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGetRequest,
) -> Result<dto::CompanionSoulView, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_clear(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGrowthClearRequest,
) -> Result<u32, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_growth_clear(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_remove(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGrowthRemoveRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_growth_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_lock(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGrowthLockRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_growth_lock(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_writer_run(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulWriterRunRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_writer_run(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_list(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionNotesRequest,
) -> Result<Vec<dto::CompanionNoteView>, ApiError> {
    let context = context.snapshot()?;
    api::companion_notes_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_upsert(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionNoteUpsertRequest,
) -> Result<dto::CompanionNoteView, ApiError> {
    let context = context.snapshot()?;
    api::companion_notes_upsert(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionNoteDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::companion_notes_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_active_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionNotesActivePreviewRequest,
) -> Result<Vec<dto::CompanionNoteView>, ApiError> {
    let context = context.snapshot()?;
    api::companion_notes_active_preview(&context, request).await
}
