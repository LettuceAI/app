use std::sync::Arc;

use lettuce_app::api::{self, ApiContext, GenerationEventSink};
use lettuce_contracts::{
    ApiError, ConversationMessagesRequest, ConversationOpenRequest, ConversationPage,
    ConversationParticipantAddRequest, ConversationParticipantUpdateRequest,
    ConversationRenameRequest, ConversationRequest, ConversationRevisions, ConversationSendRequest,
    ConversationSettingsGetRequest, ConversationSettingsUpdateRequest, ConversationSettingsView,
    ConversationView, ConversationsListRequest, GenerationCancelRequest, GenerationEvent,
    LatestConversationPage, LatestConversationsRequest, LaunchDirectRequest, LaunchDirectResponse,
    LaunchGroupRequest, LaunchGroupResponse, MessagePage, SendAccepted,
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

#[tauri::command]
#[specta::specta]
pub async fn conversation_settings_get(
    context: State<'_, ApiContext>,
    request: ConversationSettingsGetRequest,
) -> Result<ConversationSettingsView, ApiError> {
    api::conversation_settings_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_settings_update(
    context: State<'_, ApiContext>,
    request: ConversationSettingsUpdateRequest,
) -> Result<ConversationSettingsView, ApiError> {
    api::conversation_settings_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_rename(
    context: State<'_, ApiContext>,
    request: ConversationRenameRequest,
) -> Result<ConversationRevisions, ApiError> {
    api::conversation_rename(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_archive(
    context: State<'_, ApiContext>,
    request: ConversationRequest,
) -> Result<ConversationRevisions, ApiError> {
    api::conversation_archive(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_restore(
    context: State<'_, ApiContext>,
    request: ConversationRequest,
) -> Result<ConversationRevisions, ApiError> {
    api::conversation_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_delete(
    context: State<'_, ApiContext>,
    request: ConversationRequest,
) -> Result<(), ApiError> {
    api::conversation_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_participant_add(
    context: State<'_, ApiContext>,
    request: ConversationParticipantAddRequest,
) -> Result<ConversationRevisions, ApiError> {
    api::conversation_participant_add(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_participant_update(
    context: State<'_, ApiContext>,
    request: ConversationParticipantUpdateRequest,
) -> Result<ConversationRevisions, ApiError> {
    api::conversation_participant_update(&context, request).await
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
