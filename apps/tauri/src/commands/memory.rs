use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn memory_get(
    context: State<'_, ApiContextSlot>,
    request: dto::ConversationRequest,
) -> Result<dto::MemoryView, ApiError> {
    let context = context.snapshot()?;
    api::memory_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_add(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryAddRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_add(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_update(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryUpdateRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryDeleteRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_pin(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryPinRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_pin(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_set_temperature(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryTemperatureRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_set_temperature(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_summary_update(
    context: State<'_, ApiContextSlot>,
    request: dto::MemorySummaryUpdateRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_summary_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_trigger(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryTriggerRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::memory_trigger(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_retry(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryRetryRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::memory_retry(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_skip(
    context: State<'_, ApiContextSlot>,
    request: dto::MemorySkipRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::memory_skip(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_cycles(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryCyclesRequest,
) -> Result<dto::MemoryCyclePage, ApiError> {
    let context = context.snapshot()?;
    api::memory_cycles(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_cycle_revert(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryCycleRevertRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    let context = context.snapshot()?;
    api::memory_cycle_revert(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_error_dismiss(
    context: State<'_, ApiContextSlot>,
    request: dto::MemoryErrorDismissRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::memory_error_dismiss(&context, request).await
}
