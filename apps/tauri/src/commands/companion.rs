use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn companion_state_get(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionStateGetRequest,
) -> Result<Option<dto::CompanionStateView>, ApiError> {
    let context = context.snapshot()?;
    api::companion_state_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_companion_state_get(
    context: State<'_, ApiContextSlot>,
    request: dto::GroupCompanionStateGetRequest,
) -> Result<Option<dto::GroupCompanionStateView>, ApiError> {
    let context = context.snapshot()?;
    api::group_companion_state_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_companion_message_effects(
    context: State<'_, ApiContextSlot>,
    request: dto::GroupCompanionMessageEffectsRequest,
) -> Result<Vec<dto::GroupCompanionMemberEffectView>, ApiError> {
    let context = context.snapshot()?;
    api::group_companion_message_effects(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_get(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGetRequest,
) -> Result<dto::CompanionSoulView, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_clear(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGrowthClearRequest,
) -> Result<u32, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_growth_clear(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_remove(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGrowthRemoveRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_growth_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_growth_lock(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulGrowthLockRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_growth_lock(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_soul_writer_run(
    context: State<'_, ApiContextSlot>,
    request: dto::CompanionSoulWriterRunRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::companion_soul_writer_run(&context, request).await
}
