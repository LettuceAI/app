use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{
    ApiError, JobAccepted, LlamaChatTemplate, LlamaChatTemplateRequest, LlamaContextInfo,
    LlamaContextInfoRequest, LlamaDeviceList, LocalModelAdoptRequest, LocalModelAdopted,
    LocalModelDeleteRequest, LocalModelDeleted, LocalModelList, LocalModelsDir,
    LocalModelsDirSetRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn llama_devices(
    context: State<'_, ApiContextSlot>,
) -> Result<LlamaDeviceList, ApiError> {
    let context = context.snapshot()?;
    api::llama_devices(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn llama_context_info(
    context: State<'_, ApiContextSlot>,
    request: LlamaContextInfoRequest,
) -> Result<LlamaContextInfo, ApiError> {
    let context = context.snapshot()?;
    api::llama_context_info(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llama_chat_template(
    context: State<'_, ApiContextSlot>,
    request: LlamaChatTemplateRequest,
) -> Result<LlamaChatTemplate, ApiError> {
    let context = context.snapshot()?;
    api::llama_chat_template(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn llama_unload(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::llama_unload(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_models_list(
    context: State<'_, ApiContextSlot>,
) -> Result<LocalModelList, ApiError> {
    let context = context.snapshot()?;
    api::local_models_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_model_delete(
    context: State<'_, ApiContextSlot>,
    request: LocalModelDeleteRequest,
) -> Result<LocalModelDeleted, ApiError> {
    let context = context.snapshot()?;
    api::local_model_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_model_adopt(
    context: State<'_, ApiContextSlot>,
    request: LocalModelAdoptRequest,
) -> Result<LocalModelAdopted, ApiError> {
    let context = context.snapshot()?;
    api::local_model_adopt(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_models_dir_get(
    context: State<'_, ApiContextSlot>,
) -> Result<LocalModelsDir, ApiError> {
    let context = context.snapshot()?;
    api::local_models_dir_get(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn local_models_dir_set(
    context: State<'_, ApiContextSlot>,
    request: LocalModelsDirSetRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::local_models_dir_set(&context, request).await
}
