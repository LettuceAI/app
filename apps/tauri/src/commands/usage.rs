use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn usage_clear_before(
    context: State<'_, ApiContext>,
    request: dto::UsageClearBeforeRequest,
) -> Result<dto::UsageCleared, ApiError> {
    api::usage_clear_before(&context, request).await
}
