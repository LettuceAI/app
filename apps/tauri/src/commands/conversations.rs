use std::sync::Arc;

use lettuce_app::api::{self, ApiContext, GenerationEventSink};
use lettuce_contracts::{
    ApiError, ConversationAddUserMessageRequest, ConversationContinueRequest,
    ConversationMessagesAroundRequest, ConversationMessagesRequest, ConversationOpenRequest,
    ConversationPage, ConversationParticipantAddRequest, ConversationParticipantUpdateRequest,
    ConversationPinnedMessagesRequest, ConversationRegenerateRequest, ConversationRenameRequest,
    ConversationRequest, ConversationRetryRequest, ConversationRevisions,
    ConversationSearchRequest, ConversationSendRequest, ConversationSettingsGetRequest,
    ConversationSettingsUpdateRequest, ConversationSettingsView, ConversationView,
    ConversationsListRequest, GenerationAccepted, GenerationCancelRequest, GenerationEvent,
    LatestConversationPage, LatestConversationsRequest, LaunchDirectRequest, LaunchDirectResponse,
    LaunchGroupRequest, LaunchGroupResponse, MemoryRewindRetryOutcome, MemoryRewindRetryRequest,
    MessageCandidatePage, MessageCandidateSelectRequest, MessageChanged, MessageCompanionEffect,
    MessageCompanionEffectRequest, MessageCount, MessageDeleteRequest, MessageEditRequest,
    MessageHistoryRequest, MessagePage, MessagePinRequest, MessagePromptSnapshotRequest,
    MessageRevisionPage, MessageSceneSelectRequest, MessageWindow, MessagesDeleteResult,
    ParticipationStats, PromptSnapshot, SearchHitPage, SendAccepted, SpeakerSelectionPreview,
    SpeakerSelectionPreviewRequest,
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
pub async fn message_edit(
    context: State<'_, ApiContext>,
    request: MessageEditRequest,
) -> Result<MessageChanged, ApiError> {
    api::message_edit(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_delete(
    context: State<'_, ApiContext>,
    request: MessageDeleteRequest,
) -> Result<MessagesDeleteResult, ApiError> {
    api::message_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_delete_after(
    context: State<'_, ApiContext>,
    request: MessageDeleteRequest,
) -> Result<MessagesDeleteResult, ApiError> {
    api::messages_delete_after(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_rewind_retry(
    context: State<'_, ApiContext>,
    request: MemoryRewindRetryRequest,
) -> Result<MemoryRewindRetryOutcome, ApiError> {
    api::memory_rewind_retry(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_pin(
    context: State<'_, ApiContext>,
    request: MessagePinRequest,
) -> Result<MessageChanged, ApiError> {
    api::message_pin(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_candidate_select(
    context: State<'_, ApiContext>,
    request: MessageCandidateSelectRequest,
) -> Result<MessageChanged, ApiError> {
    api::message_candidate_select(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_scene_select(
    context: State<'_, ApiContext>,
    request: MessageSceneSelectRequest,
) -> Result<MessageChanged, ApiError> {
    api::message_scene_select(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_revisions(
    context: State<'_, ApiContext>,
    request: MessageHistoryRequest,
) -> Result<MessageRevisionPage, ApiError> {
    api::message_revisions(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_candidates(
    context: State<'_, ApiContext>,
    request: MessageHistoryRequest,
) -> Result<MessageCandidatePage, ApiError> {
    api::message_candidates(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_search(
    context: State<'_, ApiContext>,
    request: ConversationSearchRequest,
) -> Result<SearchHitPage, ApiError> {
    api::conversation_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_pinned_messages(
    context: State<'_, ApiContext>,
    request: ConversationPinnedMessagesRequest,
) -> Result<MessagePage, ApiError> {
    api::conversation_pinned_messages(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_message_count(
    context: State<'_, ApiContext>,
    request: ConversationRequest,
) -> Result<MessageCount, ApiError> {
    api::conversation_message_count(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_messages_around(
    context: State<'_, ApiContext>,
    request: ConversationMessagesAroundRequest,
) -> Result<MessageWindow, ApiError> {
    api::conversation_messages_around(&context, request).await
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
pub async fn conversation_regenerate(
    context: State<'_, ApiContext>,
    request: ConversationRegenerateRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<GenerationAccepted, ApiError> {
    api::conversation_regenerate(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_continue(
    context: State<'_, ApiContext>,
    request: ConversationContinueRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<GenerationAccepted, ApiError> {
    api::conversation_continue(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_retry(
    context: State<'_, ApiContext>,
    request: ConversationRetryRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<GenerationAccepted, ApiError> {
    api::conversation_retry(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_add_user_message(
    context: State<'_, ApiContext>,
    request: ConversationAddUserMessageRequest,
) -> Result<MessageChanged, ApiError> {
    api::conversation_add_user_message(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_prompt_snapshot(
    context: State<'_, ApiContext>,
    request: MessagePromptSnapshotRequest,
) -> Result<PromptSnapshot, ApiError> {
    api::message_prompt_snapshot(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_speaker_selection_preview(
    context: State<'_, ApiContext>,
    request: SpeakerSelectionPreviewRequest,
) -> Result<SpeakerSelectionPreview, ApiError> {
    api::conversation_speaker_selection_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_participation_stats(
    context: State<'_, ApiContext>,
    request: ConversationRequest,
) -> Result<ParticipationStats, ApiError> {
    api::conversation_participation_stats(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_companion_effect(
    context: State<'_, ApiContext>,
    request: MessageCompanionEffectRequest,
) -> Result<Option<MessageCompanionEffect>, ApiError> {
    api::message_companion_effect(&context, request).await
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
