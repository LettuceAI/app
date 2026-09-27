use std::sync::Arc;

use lettuce_app::api::{self, ApiContext, GenerationEventSink};
use lettuce_contracts::{
    ApiError, ConversationMessagesRequest, ConversationOpenRequest, ConversationPage,
    ConversationSendRequest, ConversationView, ConversationsListRequest, GenerationCancelRequest,
    GenerationEvent, LatestConversationPage, LatestConversationsRequest, LaunchDirectRequest,
    LaunchDirectResponse, LaunchGroupRequest, LaunchGroupResponse, MessagePage, SendAccepted,
};
use tauri::{State, ipc::Channel};

#[tauri::command]
#[specta::specta]
pub async fn conversations_list(
    context: State<'_, ApiContext>,
    request: ConversationsListRequest,
) -> Result<ConversationPage, ApiError> {
    api::conversations_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversations_latest_by_character(
    context: State<'_, ApiContext>,
    request: LatestConversationsRequest,
) -> Result<LatestConversationPage, ApiError> {
    api::conversations_latest_by_character(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversations_latest_by_group(
    context: State<'_, ApiContext>,
    request: LatestConversationsRequest,
) -> Result<LatestConversationPage, ApiError> {
    api::conversations_latest_by_group(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_open(
    context: State<'_, ApiContext>,
    request: ConversationOpenRequest,
) -> Result<ConversationView, ApiError> {
    api::conversation_open(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_messages(
    context: State<'_, ApiContext>,
    request: ConversationMessagesRequest,
) -> Result<MessagePage, ApiError> {
    api::conversation_messages(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_send(
    context: State<'_, ApiContext>,
    request: ConversationSendRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<SendAccepted, ApiError> {
    api::conversation_send(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn generation_cancel(
    context: State<'_, ApiContext>,
    request: GenerationCancelRequest,
) -> Result<(), ApiError> {
    api::generation_cancel(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_launch_direct(
    context: State<'_, ApiContext>,
    request: LaunchDirectRequest,
) -> Result<LaunchDirectResponse, ApiError> {
    api::conversation_launch_direct(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_launch_group(
    context: State<'_, ApiContext>,
    request: LaunchGroupRequest,
) -> Result<LaunchGroupResponse, ApiError> {
    api::conversation_launch_group(&context, request).await
}

/// A send's generation stream, delivered over the command's IPC channel.
struct ChannelSink(Channel<GenerationEvent>);

impl GenerationEventSink for ChannelSink {
    fn emit(&self, event: GenerationEvent) {
        if let Err(error) = self.0.send(event) {
            tracing::debug!(%error, "generation channel is closed");
        }
    }
}
