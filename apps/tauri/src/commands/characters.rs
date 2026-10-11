use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{ApiError, CharacterPage, CharactersListRequest};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn characters_list(
    context: State<'_, ApiContextSlot>,
    request: CharactersListRequest,
) -> Result<CharacterPage, ApiError> {
    let context = context.snapshot()?;
    api::characters_list(&context, request).await
}
