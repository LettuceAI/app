use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{ApiError, GroupDeleteRequest};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn group_delete(
    context: State<'_, ApiContext>,
    request: GroupDeleteRequest,
) -> Result<(), ApiError> {
    api::group_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_delete(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::PersonaDeleteRequest,
) -> Result<(), ApiError> {
    api::persona_delete(&context, request).await
}
