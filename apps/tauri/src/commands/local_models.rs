use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, JobAccepted, LlamaChatTemplate, LlamaChatTemplateRequest, LlamaContextInfo,
    LlamaContextInfoRequest, LlamaDeviceList, LocalFileRunnability, LocalFileRunnabilityRequest,
    LocalModelAdoptRequest, LocalModelAdopted, LocalModelDeleteRequest, LocalModelDeleted,
    LocalModelList, LocalModelsDir, LocalModelsDirSetRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn llama_devices(context: State<'_, ApiContext>) -> Result<LlamaDeviceList, ApiError> {
    api::llama_devices(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn llama_context_info(
    context: State<'_, ApiContext>,
    request: LlamaContextInfoRequest,
) -> Result<LlamaContextInfo, ApiError> {
    api::llama_context_info(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llama_chat_template(
    context: State<'_, ApiContext>,
    request: LlamaChatTemplateRequest,
) -> Result<LlamaChatTemplate, ApiError> {
    api::llama_chat_template(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llama_unload(context: State<'_, ApiContext>) -> Result<(), ApiError> {
    api::llama_unload(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_models_list(context: State<'_, ApiContext>) -> Result<LocalModelList, ApiError> {
    api::local_models_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_model_delete(
    context: State<'_, ApiContext>,
    request: LocalModelDeleteRequest,
) -> Result<LocalModelDeleted, ApiError> {
    api::local_model_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_model_adopt(
    context: State<'_, ApiContext>,
    request: LocalModelAdoptRequest,
) -> Result<LocalModelAdopted, ApiError> {
    api::local_model_adopt(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_models_dir_get(
    context: State<'_, ApiContext>,
) -> Result<LocalModelsDir, ApiError> {
    api::local_models_dir_get(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_models_dir_set(
    context: State<'_, ApiContext>,
    request: LocalModelsDirSetRequest,
) -> Result<JobAccepted, ApiError> {
    api::local_models_dir_set(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_file_runnability(
    context: State<'_, ApiContext>,
    request: LocalFileRunnabilityRequest,
) -> Result<LocalFileRunnability, ApiError> {
    api::local_file_runnability(&context, request).await
}
