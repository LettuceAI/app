use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, AppStatus, AppUiStateUpdateRequest, AppUiStateView, PurgeNoticeDismissRequest,
    PurgeNoticeList,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn app_status(context: State<'_, ApiContext>) -> Result<AppStatus, ApiError> {
    api::app_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn app_ui_state_update(
    context: State<'_, ApiContext>,
    request: AppUiStateUpdateRequest,
) -> Result<AppUiStateView, ApiError> {
    api::app_ui_state_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn purge_notices_list(
    context: State<'_, ApiContext>,
) -> Result<PurgeNoticeList, ApiError> {
    api::purge_notices_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn purge_notice_dismiss(
    context: State<'_, ApiContext>,
    request: PurgeNoticeDismissRequest,
) -> Result<(), ApiError> {
    api::purge_notice_dismiss(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn app_usage_days(
    context: State<'_, ApiContext>,
) -> Result<lettuce_contracts::AppUsageDaysView, ApiError> {
    api::app_usage_days(&context).await
}
