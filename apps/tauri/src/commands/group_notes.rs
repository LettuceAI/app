use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn group_notes_list(
    context: State<'_, ApiContextSlot>,
    request: dto::GroupNotesRequest,
) -> Result<Vec<dto::GroupNoteView>, ApiError> {
    let context = context.snapshot()?;
    api::group_notes_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_notes_upsert(
    context: State<'_, ApiContextSlot>,
    request: dto::GroupNoteUpsertRequest,
) -> Result<dto::GroupNoteView, ApiError> {
    let context = context.snapshot()?;
    api::group_notes_upsert(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_notes_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::GroupNoteDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::group_notes_delete(&context, request).await
}
