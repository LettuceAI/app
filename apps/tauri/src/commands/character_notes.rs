use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn character_notes_list(
    context: State<'_, ApiContextSlot>,
    request: dto::CharacterNotesRequest,
) -> Result<Vec<dto::CharacterNoteView>, ApiError> {
    let context = context.snapshot()?;
    api::character_notes_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_notes_upsert(
    context: State<'_, ApiContextSlot>,
    request: dto::CharacterNoteUpsertRequest,
) -> Result<dto::CharacterNoteView, ApiError> {
    let context = context.snapshot()?;
    api::character_notes_upsert(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_notes_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::CharacterNoteDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::character_notes_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_notes_active_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::CharacterNotesActivePreviewRequest,
) -> Result<Vec<dto::CharacterNoteView>, ApiError> {
    let context = context.snapshot()?;
    api::character_notes_active_preview(&context, request).await
}
