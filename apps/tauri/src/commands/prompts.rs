use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn prompts_list(
    context: State<'_, ApiContext>,
    request: dto::PromptsListRequest,
) -> Result<dto::PromptPage, ApiError> {
    api::prompts_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_get(
    context: State<'_, ApiContext>,
    request: dto::PromptGetRequest,
) -> Result<dto::PromptView, ApiError> {
    api::prompt_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_create(
    context: State<'_, ApiContext>,
    request: dto::PromptCreateRequest,
) -> Result<dto::PromptView, ApiError> {
    api::prompt_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_update(
    context: State<'_, ApiContext>,
    request: dto::PromptUpdateRequest,
) -> Result<dto::PromptView, ApiError> {
    api::prompt_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_delete(
    context: State<'_, ApiContext>,
    request: dto::PromptDeleteRequest,
) -> Result<dto::SourceDeleteResult, ApiError> {
    api::prompt_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_builtin_reset(
    context: State<'_, ApiContext>,
    request: dto::PromptBuiltinResetRequest,
) -> Result<dto::PromptBuiltinResetResult, ApiError> {
    api::prompt_builtin_reset(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_app_default_set(
    context: State<'_, ApiContext>,
    request: dto::PromptAppDefaultSetRequest,
) -> Result<dto::PromptAppDefault, ApiError> {
    api::prompt_app_default_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_preview(
    context: State<'_, ApiContext>,
    request: dto::PromptPreviewRequest,
) -> Result<dto::PromptPreview, ApiError> {
    api::prompt_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn default_character_rules(
    context: State<'_, ApiContext>,
    request: dto::DefaultCharacterRulesRequest,
) -> Result<dto::DefaultCharacterRules, ApiError> {
    api::default_character_rules(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_placeholders(
    context: State<'_, ApiContext>,
    request: dto::PromptPlaceholdersRequest,
) -> Result<dto::PromptPlaceholders, ApiError> {
    api::prompt_placeholders(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_validate(
    context: State<'_, ApiContext>,
    request: dto::PromptValidateRequest,
) -> Result<dto::PromptValidation, ApiError> {
    api::prompt_validate(&context, request).await
}
