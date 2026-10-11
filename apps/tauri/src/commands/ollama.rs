use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{
    ApiError, JobAccepted, OllamaModelDeleteRequest, OllamaModelList, OllamaModelsRequest,
    OllamaPullRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn ollama_models_list(
    context: State<'_, ApiContextSlot>,
    request: OllamaModelsRequest,
) -> Result<OllamaModelList, ApiError> {
    let context = context.snapshot()?;
    api::ollama_models_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn ollama_model_delete(
    context: State<'_, ApiContextSlot>,
    request: OllamaModelDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::ollama_model_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn ollama_pull(
    context: State<'_, ApiContextSlot>,
    request: OllamaPullRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::ollama_pull(&context, request).await
}
