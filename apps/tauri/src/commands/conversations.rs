use std::sync::Arc;

use lettuce_app::api::{self, ApiContextSlot, GenerationEventSink};
use lettuce_contracts::{
    ApiError, ConversationAddUserMessageRequest, ConversationContinueRequest,
    ConversationHelpMeReplyRequest, ConversationMessagesAroundRequest, ConversationMessagesRequest,
    ConversationOpenRequest, ConversationPage, ConversationParticipantAddRequest,
    ConversationParticipantUpdateRequest, ConversationPinnedMessagesRequest,
    ConversationRegenerateRequest, ConversationRenameRequest, ConversationRequest,
    ConversationRetryRequest, ConversationRevisions, ConversationSearchRequest,
    ConversationSendRequest, ConversationSettingsGetRequest, ConversationSettingsUpdateRequest,
    ConversationSettingsView, ConversationView, ConversationsListRequest, GenerationAccepted,
    GenerationCancelRequest, GenerationEvent, JobAccepted, LatestConversationPage,
    LatestConversationsRequest, LaunchDirectRequest, LaunchDirectResponse, LaunchGroupRequest,
    LaunchGroupResponse, MemoryRewindRetryOutcome, MemoryRewindRetryRequest, MessageCandidatePage,
    MessageCandidateSelectRequest, MessageChanged, MessageCompanionEffect,
    MessageCompanionEffectRequest, MessageCount, MessageDeleteRequest, MessageEditRequest,
    MessageHistoryRequest, MessagePage, MessagePinRequest, MessagePromptSnapshotRequest,
    MessageRevisionPage, MessageSceneImageApproveRequest, MessageSceneImageGenerateRequest,
    MessageScenePromptGenerateRequest, MessageSceneRequest, MessageSceneSelectRequest,
    MessageWindow, MessagesDeleteResult, ParticipationStats, PromptSnapshot, SearchHitPage,
    SendAccepted, SpeakerSelectionPreview, SpeakerSelectionPreviewRequest,
};
use tauri::{State, ipc::Channel};

#[tauri::command]
#[specta::specta]
pub async fn conversations_list(
    context: State<'_, ApiContextSlot>,
    request: ConversationsListRequest,
) -> Result<ConversationPage, ApiError> {
    let context = context.snapshot()?;
    api::conversations_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversations_latest_by_character(
    context: State<'_, ApiContextSlot>,
    request: LatestConversationsRequest,
) -> Result<LatestConversationPage, ApiError> {
    let context = context.snapshot()?;
    api::conversations_latest_by_character(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversations_latest_by_group(
    context: State<'_, ApiContextSlot>,
    request: LatestConversationsRequest,
) -> Result<LatestConversationPage, ApiError> {
    let context = context.snapshot()?;
    api::conversations_latest_by_group(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_open(
    context: State<'_, ApiContextSlot>,
    request: ConversationOpenRequest,
) -> Result<ConversationView, ApiError> {
    let context = context.snapshot()?;
    api::conversation_open(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_messages(
    context: State<'_, ApiContextSlot>,
    request: ConversationMessagesRequest,
) -> Result<MessagePage, ApiError> {
    let context = context.snapshot()?;
    api::conversation_messages(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_edit(
    context: State<'_, ApiContextSlot>,
    request: MessageEditRequest,
) -> Result<MessageChanged, ApiError> {
    let context = context.snapshot()?;
    api::message_edit(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_delete(
    context: State<'_, ApiContextSlot>,
    request: MessageDeleteRequest,
) -> Result<MessagesDeleteResult, ApiError> {
    let context = context.snapshot()?;
    api::message_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_delete_after(
    context: State<'_, ApiContextSlot>,
    request: MessageDeleteRequest,
) -> Result<MessagesDeleteResult, ApiError> {
    let context = context.snapshot()?;
    api::messages_delete_after(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn memory_rewind_retry(
    context: State<'_, ApiContextSlot>,
    request: MemoryRewindRetryRequest,
) -> Result<MemoryRewindRetryOutcome, ApiError> {
    let context = context.snapshot()?;
    api::memory_rewind_retry(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_pin(
    context: State<'_, ApiContextSlot>,
    request: MessagePinRequest,
) -> Result<MessageChanged, ApiError> {
    let context = context.snapshot()?;
    api::message_pin(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_candidate_select(
    context: State<'_, ApiContextSlot>,
    request: MessageCandidateSelectRequest,
) -> Result<MessageChanged, ApiError> {
    let context = context.snapshot()?;
    api::message_candidate_select(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_scene_select(
    context: State<'_, ApiContextSlot>,
    request: MessageSceneSelectRequest,
) -> Result<MessageChanged, ApiError> {
    let context = context.snapshot()?;
    api::message_scene_select(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_revisions(
    context: State<'_, ApiContextSlot>,
    request: MessageHistoryRequest,
) -> Result<MessageRevisionPage, ApiError> {
    let context = context.snapshot()?;
    api::message_revisions(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_candidates(
    context: State<'_, ApiContextSlot>,
    request: MessageHistoryRequest,
) -> Result<MessageCandidatePage, ApiError> {
    let context = context.snapshot()?;
    api::message_candidates(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_search(
    context: State<'_, ApiContextSlot>,
    request: ConversationSearchRequest,
) -> Result<SearchHitPage, ApiError> {
    let context = context.snapshot()?;
    api::conversation_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_pinned_messages(
    context: State<'_, ApiContextSlot>,
    request: ConversationPinnedMessagesRequest,
) -> Result<MessagePage, ApiError> {
    let context = context.snapshot()?;
    api::conversation_pinned_messages(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_message_count(
    context: State<'_, ApiContextSlot>,
    request: ConversationRequest,
) -> Result<MessageCount, ApiError> {
    let context = context.snapshot()?;
    api::conversation_message_count(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_messages_around(
    context: State<'_, ApiContextSlot>,
    request: ConversationMessagesAroundRequest,
) -> Result<MessageWindow, ApiError> {
    let context = context.snapshot()?;
    api::conversation_messages_around(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_send(
    context: State<'_, ApiContextSlot>,
    request: ConversationSendRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<SendAccepted, ApiError> {
    let context = context.snapshot()?;
    api::conversation_send(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_regenerate(
    context: State<'_, ApiContextSlot>,
    request: ConversationRegenerateRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<GenerationAccepted, ApiError> {
    let context = context.snapshot()?;
    api::conversation_regenerate(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_continue(
    context: State<'_, ApiContextSlot>,
    request: ConversationContinueRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<GenerationAccepted, ApiError> {
    let context = context.snapshot()?;
    api::conversation_continue(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_retry(
    context: State<'_, ApiContextSlot>,
    request: ConversationRetryRequest,
    on_event: Channel<GenerationEvent>,
) -> Result<GenerationAccepted, ApiError> {
    let context = context.snapshot()?;
    api::conversation_retry(&context, request, Arc::new(ChannelSink(on_event))).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_add_user_message(
    context: State<'_, ApiContextSlot>,
    request: ConversationAddUserMessageRequest,
) -> Result<MessageChanged, ApiError> {
    let context = context.snapshot()?;
    api::conversation_add_user_message(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_help_me_reply(
    context: State<'_, ApiContextSlot>,
    request: ConversationHelpMeReplyRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::conversation_help_me_reply(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_scene_image_approve(
    context: State<'_, ApiContextSlot>,
    request: MessageSceneImageApproveRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::message_scene_image_approve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_scene_image_dismiss(
    context: State<'_, ApiContextSlot>,
    request: MessageSceneRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::message_scene_image_dismiss(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_scene_image_generate(
    context: State<'_, ApiContextSlot>,
    request: MessageSceneImageGenerateRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::message_scene_image_generate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_scene_prompt_generate(
    context: State<'_, ApiContextSlot>,
    request: MessageScenePromptGenerateRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::message_scene_prompt_generate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_prompt_snapshot(
    context: State<'_, ApiContextSlot>,
    request: MessagePromptSnapshotRequest,
) -> Result<PromptSnapshot, ApiError> {
    let context = context.snapshot()?;
    api::message_prompt_snapshot(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_speaker_selection_preview(
    context: State<'_, ApiContextSlot>,
    request: SpeakerSelectionPreviewRequest,
) -> Result<SpeakerSelectionPreview, ApiError> {
    let context = context.snapshot()?;
    api::conversation_speaker_selection_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_participation_stats(
    context: State<'_, ApiContextSlot>,
    request: ConversationRequest,
) -> Result<ParticipationStats, ApiError> {
    let context = context.snapshot()?;
    api::conversation_participation_stats(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn message_companion_effect(
    context: State<'_, ApiContextSlot>,
    request: MessageCompanionEffectRequest,
) -> Result<Option<MessageCompanionEffect>, ApiError> {
    let context = context.snapshot()?;
    api::message_companion_effect(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn generation_cancel(
    context: State<'_, ApiContextSlot>,
    request: GenerationCancelRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::generation_cancel(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_launch_direct(
    context: State<'_, ApiContextSlot>,
    request: LaunchDirectRequest,
) -> Result<LaunchDirectResponse, ApiError> {
    let context = context.snapshot()?;
    api::conversation_launch_direct(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_launch_group(
    context: State<'_, ApiContextSlot>,
    request: LaunchGroupRequest,
) -> Result<LaunchGroupResponse, ApiError> {
    let context = context.snapshot()?;
    api::conversation_launch_group(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_settings_get(
    context: State<'_, ApiContextSlot>,
    request: ConversationSettingsGetRequest,
) -> Result<ConversationSettingsView, ApiError> {
    let context = context.snapshot()?;
    api::conversation_settings_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_settings_update(
    context: State<'_, ApiContextSlot>,
    request: ConversationSettingsUpdateRequest,
) -> Result<ConversationSettingsView, ApiError> {
    let context = context.snapshot()?;
    api::conversation_settings_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_rename(
    context: State<'_, ApiContextSlot>,
    request: ConversationRenameRequest,
) -> Result<ConversationRevisions, ApiError> {
    let context = context.snapshot()?;
    api::conversation_rename(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_archive(
    context: State<'_, ApiContextSlot>,
    request: ConversationRequest,
) -> Result<ConversationRevisions, ApiError> {
    let context = context.snapshot()?;
    api::conversation_archive(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_restore(
    context: State<'_, ApiContextSlot>,
    request: ConversationRequest,
) -> Result<ConversationRevisions, ApiError> {
    let context = context.snapshot()?;
    api::conversation_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_delete(
    context: State<'_, ApiContextSlot>,
    request: ConversationRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::conversation_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_participant_add(
    context: State<'_, ApiContextSlot>,
    request: ConversationParticipantAddRequest,
) -> Result<ConversationRevisions, ApiError> {
    let context = context.snapshot()?;
    api::conversation_participant_add(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_participant_update(
    context: State<'_, ApiContextSlot>,
    request: ConversationParticipantUpdateRequest,
) -> Result<ConversationRevisions, ApiError> {
    let context = context.snapshot()?;
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

#[tauri::command]
#[specta::specta]
pub async fn conversation_branches(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationRequest,
) -> Result<lettuce_contracts::ConversationBranchList, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branches(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_fork(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationBranchForkRequest,
) -> Result<lettuce_contracts::ConversationBranchChanged, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_fork(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_rename(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationBranchRenameRequest,
) -> Result<lettuce_contracts::ConversationBranchChanged, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_rename(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_select(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationBranchMutationRequest,
) -> Result<lettuce_contracts::ConversationBranchChanged, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_select(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_duplicate(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationDuplicateRequest,
) -> Result<lettuce_contracts::ConversationCopyResult, ApiError> {
    let context = context.snapshot()?;
    api::conversation_duplicate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_delete(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationBranchMutationRequest,
) -> Result<lettuce_contracts::ConversationBranchChanged, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_to_character_from_message(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationCharacterCopyRequest,
) -> Result<lettuce_contracts::ConversationCopyResult, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_to_character_from_message(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_to_character(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationGroupCharacterCopyRequest,
) -> Result<lettuce_contracts::ConversationCopyResult, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_to_character(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_direct_to_character(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationCharacterCopyRequest,
) -> Result<lettuce_contracts::ConversationCopyResult, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_direct_to_character(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_branch_direct_to_group(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ConversationGroupCopyRequest,
) -> Result<lettuce_contracts::ConversationCopyResult, ApiError> {
    let context = context.snapshot()?;
    api::conversation_branch_direct_to_group(&context, request).await
}
