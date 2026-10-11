use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn settings_get(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::SettingsView, ApiError> {
    let context = context.snapshot()?;
    api::settings_get(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    context: State<'_, ApiContextSlot>,
    request: dto::SettingsCommandInput<dto::SettingsUpdateRequest>,
) -> Result<dto::SettingsView, ApiError> {
    let context = context.snapshot()?;
    api::settings_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn settings_sampler_defaults_update(
    context: State<'_, ApiContextSlot>,
    request: dto::SettingsCommandInput<dto::SettingsSamplerDefaultsUpdateRequest>,
) -> Result<dto::SettingsView, ApiError> {
    let context = context.snapshot()?;
    api::settings_sampler_defaults_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn content_filter_log(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::ContentFilterLogView, ApiError> {
    let context = context.snapshot()?;
    api::content_filter_log(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn content_filter_clear(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::content_filter_clear(&context).await
}
