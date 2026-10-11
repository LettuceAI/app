use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{
    ApiError, AppStatus, AppUiStateUpdateRequest, AppUiStateView, PurgeNoticeDismissRequest,
    PurgeNoticeList,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn app_status(context: State<'_, ApiContextSlot>) -> Result<AppStatus, ApiError> {
    let context = context.snapshot()?;
    api::app_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn app_ui_state_update(
    context: State<'_, ApiContextSlot>,
    request: AppUiStateUpdateRequest,
) -> Result<AppUiStateView, ApiError> {
    let context = context.snapshot()?;
    api::app_ui_state_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn purge_notices_list(
    context: State<'_, ApiContextSlot>,
) -> Result<PurgeNoticeList, ApiError> {
    let context = context.snapshot()?;
    api::purge_notices_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn purge_notice_dismiss(
    context: State<'_, ApiContextSlot>,
    request: PurgeNoticeDismissRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::purge_notice_dismiss(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn app_usage_days(
    context: State<'_, ApiContextSlot>,
) -> Result<lettuce_contracts::AppUsageDaysView, ApiError> {
    let context = context.snapshot()?;
    api::app_usage_days(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn legacy_state(
    context: State<'_, ApiContextSlot>,
) -> Result<lettuce_contracts::LegacyState, ApiError> {
    let context = context.snapshot()?;
    api::legacy_state(&context).await
}
