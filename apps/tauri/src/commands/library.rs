use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{ApiError, GroupDeleteRequest};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn group_delete(
    context: State<'_, ApiContextSlot>,
    request: GroupDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::group_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_delete(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::persona_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_list(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLorebooksListRequest,
) -> Result<Vec<lettuce_contracts::LibraryLorebookBinding>, ApiError> {
    let context = context.snapshot()?;
    api::character_lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_bind(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLorebookBindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::character_lorebooks_bind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_unbind(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLorebookUnbindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::character_lorebooks_unbind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_enable(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLorebookEnableRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::character_lorebooks_enable(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_lorebooks_reorder(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLorebookReorderRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::character_lorebooks_reorder(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_list(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaLorebooksListRequest,
) -> Result<Vec<lettuce_contracts::LibraryLorebookBinding>, ApiError> {
    let context = context.snapshot()?;
    api::persona_lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_bind(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaLorebookBindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::persona_lorebooks_bind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_unbind(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaLorebookUnbindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::persona_lorebooks_unbind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_enable(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaLorebookEnableRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::persona_lorebooks_enable(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_lorebooks_reorder(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaLorebookReorderRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::persona_lorebooks_reorder(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_list(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupLorebooksListRequest,
) -> Result<Vec<lettuce_contracts::LibraryLorebookBinding>, ApiError> {
    let context = context.snapshot()?;
    api::group_lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_bind(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupLorebookBindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::group_lorebooks_bind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_unbind(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupLorebookUnbindRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::group_lorebooks_unbind(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_enable(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupLorebookEnableRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::group_lorebooks_enable(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_lorebooks_reorder(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupLorebookReorderRequest,
) -> Result<lettuce_contracts::LibraryLorebookMutationResult, ApiError> {
    let context = context.snapshot()?;
    api::group_lorebooks_reorder(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_duplicate(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterDuplicateRequest,
) -> Result<lettuce_contracts::CharacterSummary, ApiError> {
    let context = context.snapshot()?;
    api::character_duplicate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_get(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaGetRequest,
) -> Result<lettuce_contracts::PersonaView, ApiError> {
    let context = context.snapshot()?;
    api::persona_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_save(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaSaveRequest,
) -> Result<lettuce_contracts::PersonaView, ApiError> {
    let context = context.snapshot()?;
    api::persona_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_default_set(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaDefaultSetRequest,
) -> Result<lettuce_contracts::PersonaDefaultView, ApiError> {
    let context = context.snapshot()?;
    api::persona_default_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_default_clear(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaDefaultClearRequest,
) -> Result<lettuce_contracts::PersonaDefaultView, ApiError> {
    let context = context.snapshot()?;
    api::persona_default_clear(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn personas_list(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonasListRequest,
) -> Result<lettuce_contracts::PersonaPage, ApiError> {
    let context = context.snapshot()?;
    api::personas_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_get(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterGetRequest,
) -> Result<lettuce_contracts::CharacterView, ApiError> {
    let context = context.snapshot()?;
    api::character_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_save(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterSaveRequest,
) -> Result<lettuce_contracts::CharacterView, ApiError> {
    let context = context.snapshot()?;
    api::character_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_get(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupGetRequest,
) -> Result<lettuce_contracts::GroupView, ApiError> {
    let context = context.snapshot()?;
    api::group_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_save(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupSaveRequest,
) -> Result<lettuce_contracts::GroupView, ApiError> {
    let context = context.snapshot()?;
    api::group_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_archive(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLifecycleRequest,
) -> Result<lettuce_contracts::CharacterView, ApiError> {
    let context = context.snapshot()?;
    api::character_archive(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_restore(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterLifecycleRequest,
) -> Result<lettuce_contracts::CharacterView, ApiError> {
    let context = context.snapshot()?;
    api::character_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_delete(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::character_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn groups_list(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::GroupsListRequest,
) -> Result<lettuce_contracts::GroupPage, ApiError> {
    let context = context.snapshot()?;
    api::groups_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_media_set(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CharacterMediaSetRequest,
) -> Result<lettuce_contracts::CharacterView, ApiError> {
    let context = context.snapshot()?;
    api::character_media_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_media_set(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::PersonaMediaSetRequest,
) -> Result<lettuce_contracts::PersonaView, ApiError> {
    let context = context.snapshot()?;
    api::persona_media_set(&context, request).await
}
