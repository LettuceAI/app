use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{ApiError, CharacterPage, CharactersListRequest};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn characters_list(
    context: State<'_, ApiContext>,
    request: CharactersListRequest,
) -> Result<CharacterPage, ApiError> {
    api::characters_list(&context, request).await
}
