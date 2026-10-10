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

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_list(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CharacterLorebooksListRequest,
) -> Result<Vec<lettuce_contracts::LibraryLorebookBinding>, ApiError> {
    api::character_lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_bind(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CharacterLorebookBindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::character_lorebooks_bind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_unbind(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CharacterLorebookUnbindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::character_lorebooks_unbind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_enable(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CharacterLorebookEnableRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::character_lorebooks_enable(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_reorder(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CharacterLorebookReorderRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::character_lorebooks_reorder(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_list(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::PersonaLorebooksListRequest,
) -> Result<Vec<lettuce_contracts::LibraryLorebookBinding>, ApiError> {
    api::persona_lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_bind(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::PersonaLorebookBindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::persona_lorebooks_bind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_unbind(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::PersonaLorebookUnbindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::persona_lorebooks_unbind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_enable(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::PersonaLorebookEnableRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::persona_lorebooks_enable(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_reorder(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::PersonaLorebookReorderRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::persona_lorebooks_reorder(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_list(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::GroupLorebooksListRequest,
) -> Result<Vec<lettuce_contracts::LibraryLorebookBinding>, ApiError> {
    api::group_lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_bind(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::GroupLorebookBindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::group_lorebooks_bind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_unbind(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::GroupLorebookUnbindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::group_lorebooks_unbind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_enable(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::GroupLorebookEnableRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::group_lorebooks_enable(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_reorder(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::GroupLorebookReorderRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    api::group_lorebooks_reorder(&context, request).await
}
