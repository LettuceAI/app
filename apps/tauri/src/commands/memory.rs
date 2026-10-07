use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn memory_get(
    context: State<'_, ApiContext>,
    request: dto::ConversationRequest,
) -> Result<dto::MemoryView, ApiError> {
    api::memory_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_add(
    context: State<'_, ApiContext>,
    request: dto::MemoryAddRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_add(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_update(
    context: State<'_, ApiContext>,
    request: dto::MemoryUpdateRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_delete(
    context: State<'_, ApiContext>,
    request: dto::MemoryDeleteRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_pin(
    context: State<'_, ApiContext>,
    request: dto::MemoryPinRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_pin(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_set_temperature(
    context: State<'_, ApiContext>,
    request: dto::MemoryTemperatureRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_set_temperature(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_summary_update(
    context: State<'_, ApiContext>,
    request: dto::MemorySummaryUpdateRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_summary_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_trigger(
    context: State<'_, ApiContext>,
    request: dto::MemoryTriggerRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::memory_trigger(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_retry(
    context: State<'_, ApiContext>,
    request: dto::MemoryRetryRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::memory_retry(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_skip(
    context: State<'_, ApiContext>,
    request: dto::MemorySkipRequest,
) -> Result<(), ApiError> {
    api::memory_skip(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_cycles(
    context: State<'_, ApiContext>,
    request: dto::MemoryCyclesRequest,
) -> Result<dto::MemoryCyclePage, ApiError> {
    api::memory_cycles(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_cycle_revert(
    context: State<'_, ApiContext>,
    request: dto::MemoryCycleRevertRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    api::memory_cycle_revert(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_error_dismiss(
    context: State<'_, ApiContext>,
    request: dto::MemoryErrorDismissRequest,
) -> Result<(), ApiError> {
    api::memory_error_dismiss(&context, request).await
}
