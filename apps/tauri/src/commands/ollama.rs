use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, JobAccepted, OllamaModelDeleteRequest, OllamaModelList, OllamaModelsRequest,
    OllamaPullRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn ollama_models_list(
    context: State<'_, ApiContext>,
    request: OllamaModelsRequest,
) -> Result<OllamaModelList, ApiError> {
    api::ollama_models_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn ollama_model_delete(
    context: State<'_, ApiContext>,
    request: OllamaModelDeleteRequest,
) -> Result<(), ApiError> {
    api::ollama_model_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn ollama_pull(
    context: State<'_, ApiContext>,
    request: OllamaPullRequest,
) -> Result<JobAccepted, ApiError> {
    api::ollama_pull(&context, request).await
}
