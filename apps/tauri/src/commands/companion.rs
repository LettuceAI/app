use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_get(
    context: State<'_, ApiContext>,
    request: dto::CompanionSoulGetRequest,
) -> Result<dto::CompanionSoulView, ApiError> {
    api::companion_soul_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_clear(
    context: State<'_, ApiContext>,
    request: dto::CompanionSoulGrowthClearRequest,
) -> Result<u32, ApiError> {
    api::companion_soul_growth_clear(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_remove(
    context: State<'_, ApiContext>,
    request: dto::CompanionSoulGrowthRemoveRequest,
) -> Result<bool, ApiError> {
    api::companion_soul_growth_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_lock(
    context: State<'_, ApiContext>,
    request: dto::CompanionSoulGrowthLockRequest,
) -> Result<bool, ApiError> {
    api::companion_soul_growth_lock(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_writer_run(
    context: State<'_, ApiContext>,
    request: dto::CompanionSoulWriterRunRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::companion_soul_writer_run(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_list(
    context: State<'_, ApiContext>,
    request: dto::CompanionNotesRequest,
) -> Result<Vec<dto::CompanionNoteView>, ApiError> {
    api::companion_notes_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_upsert(
    context: State<'_, ApiContext>,
    request: dto::CompanionNoteUpsertRequest,
) -> Result<dto::CompanionNoteView, ApiError> {
    api::companion_notes_upsert(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_delete(
    context: State<'_, ApiContext>,
    request: dto::CompanionNoteDeleteRequest,
) -> Result<(), ApiError> {
    api::companion_notes_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_notes_active_preview(
    context: State<'_, ApiContext>,
    request: dto::CompanionNotesActivePreviewRequest,
) -> Result<Vec<dto::CompanionNoteView>, ApiError> {
    api::companion_notes_active_preview(&context, request).await
}
