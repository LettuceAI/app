use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn prompts_list(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptsListRequest,
) -> Result<dto::PromptPage, ApiError> {
    let context = context.snapshot()?;
    api::prompts_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_get(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptGetRequest,
) -> Result<dto::PromptView, ApiError> {
    let context = context.snapshot()?;
    api::prompt_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_create(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptCreateRequest,
) -> Result<dto::PromptView, ApiError> {
    let context = context.snapshot()?;
    api::prompt_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_update(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptUpdateRequest,
) -> Result<dto::PromptView, ApiError> {
    let context = context.snapshot()?;
    api::prompt_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptDeleteRequest,
) -> Result<dto::SourceDeleteResult, ApiError> {
    let context = context.snapshot()?;
    api::prompt_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_builtin_reset(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptBuiltinResetRequest,
) -> Result<dto::PromptBuiltinResetResult, ApiError> {
    let context = context.snapshot()?;
    api::prompt_builtin_reset(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_app_default_set(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptAppDefaultSetRequest,
) -> Result<dto::PromptAppDefault, ApiError> {
    let context = context.snapshot()?;
    api::prompt_app_default_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptPreviewRequest,
) -> Result<dto::PromptPreview, ApiError> {
    let context = context.snapshot()?;
    api::prompt_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_placeholders(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptPlaceholdersRequest,
) -> Result<dto::PromptPlaceholders, ApiError> {
    let context = context.snapshot()?;
    api::prompt_placeholders(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_validate(
    context: State<'_, ApiContextSlot>,
    request: dto::PromptValidateRequest,
) -> Result<dto::PromptValidation, ApiError> {
    let context = context.snapshot()?;
    api::prompt_validate(&context, request).await
}
