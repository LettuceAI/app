use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{ApiError, DevConversationSeedRequest, DevConversationSeeded};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn dev_conversation_seed(
    context: State<'_, ApiContextSlot>,
    request: DevConversationSeedRequest,
) -> Result<DevConversationSeeded, ApiError> {
    let context = context.snapshot()?;
    api::dev_conversation_seed(&context, request).await
}
