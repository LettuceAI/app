use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn models_list(context: State<'_, ApiContext>) -> Result<dto::ModelsView, ApiError> {
    api::models_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_get(
    context: State<'_, ApiContext>,
    request: dto::ModelGetRequest,
) -> Result<dto::ModelView, ApiError> {
    api::model_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_save(
    context: State<'_, ApiContext>,
    request: dto::ModelSaveRequest,
) -> Result<dto::ModelView, ApiError> {
    api::model_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_delete(
    context: State<'_, ApiContext>,
    request: dto::ModelDeleteRequest,
) -> Result<(), ApiError> {
    api::model_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_duplicate(
    context: State<'_, ApiContext>,
    request: dto::ModelDuplicateRequest,
) -> Result<dto::ModelView, ApiError> {
    api::model_duplicate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_default_set(
    context: State<'_, ApiContext>,
    request: dto::ModelDefaultSetRequest,
) -> Result<dto::ModelDefaultView, ApiError> {
    api::model_default_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_nanogpt_usage(
    context: State<'_, ApiContext>,
    request: dto::ProviderNanoGptUsageRequest,
) -> Result<dto::NanoGptUsageView, ApiError> {
    api::provider_nanogpt_usage(&context, request).await
}
