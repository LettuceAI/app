use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn models_list(context: State<'_, ApiContextSlot>) -> Result<dto::ModelsView, ApiError> {
    let context = context.snapshot()?;
    api::models_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_get(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelGetRequest,
) -> Result<dto::ModelView, ApiError> {
    let context = context.snapshot()?;
    api::model_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_save(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelSaveRequest,
) -> Result<dto::ModelView, ApiError> {
    let context = context.snapshot()?;
    api::model_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::model_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_usage(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelUsageRequest,
) -> Result<dto::ModelUsage, ApiError> {
    let context = context.snapshot()?;
    api::model_usage(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_duplicate(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelDuplicateRequest,
) -> Result<dto::ModelView, ApiError> {
    let context = context.snapshot()?;
    api::model_duplicate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_default_set(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelDefaultSetRequest,
) -> Result<dto::ModelDefaultView, ApiError> {
    let context = context.snapshot()?;
    api::model_default_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_nanogpt_usage(
    context: State<'_, ApiContextSlot>,
    request: dto::ProviderNanoGptUsageRequest,
) -> Result<dto::NanoGptUsageView, ApiError> {
    let context = context.snapshot()?;
    api::provider_nanogpt_usage(&context, request).await
}
