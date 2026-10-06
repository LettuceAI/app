use serde::{Deserialize, Serialize};

use crate::AssetRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    User,
    Character,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Scene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaRole {
    Inline,
    Attachment,
    Avatar,
    Scene,
    Reference,
}

/// Which conversations a list shows; archived ones are only hidden from the
/// default list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LifecycleFilter {
    #[default]
    Active,
    Archived,
    All,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationsListRequest {
    pub kind: Option<ConversationKind>,
    /// One-to-one chats with this character.
    pub character_id: Option<String>,
    /// Group chats launched from this group.
    pub source_group_id: Option<String>,
    pub lifecycle: Option<LifecycleFilter>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// What a conversation was started from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConversationSource {
    Direct { character_id: String },
    Group { group_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum GroupChatMode {
    Conversation,
    Roleplay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSummary {
    pub id: String,
    pub kind: ConversationKind,
    pub title: String,
    pub avatars: Vec<AssetRef>,
    /// The newest visible message's text on the selected branch, at most 400
    /// characters.
    pub last_message_preview: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
    pub archived: bool,
    pub source: ConversationSource,
    /// Visible messages on the selected branch, system notes excluded.
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub message_count: u64,
    /// Group chats only.
    pub chat_mode: Option<GroupChatMode>,
    /// Optional models the chat needs that are not installed.
    pub missing_models: Vec<crate::RequiredModel>,
    /// Why the chat's dynamic memory is stopped, when it is.
    pub memory_blocked: Option<MemoryBlockedReason>,
}

/// Conversations, most recently updated first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationPage {
    pub items: Vec<ConversationSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LatestConversationsRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// A character's or group's newest conversation, archived included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LatestConversation {
    /// The character or group id.
    pub source_id: String,
    pub conversation: ConversationSummary,
}

/// One newest conversation per character or group, most recently updated
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LatestConversationPage {
    pub items: Vec<LatestConversation>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationOpenRequest {
    pub conversation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ParticipantView {
    pub id: String,
    pub role: ParticipantRole,
    pub name: String,
    pub character_id: Option<String>,
    pub avatar: Option<AssetRef>,
}

/// The selected branch and the message it currently ends at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct BranchHead {
    pub branch_id: String,
    pub head_message_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationView {
    pub id: String,
    pub origin_conversation_id: Option<String>,
    pub origin_message_id: Option<String>,
    pub kind: ConversationKind,
    pub title: String,
    pub participants: Vec<ParticipantView>,
    pub branch: BranchHead,
    pub messages: MessagePage,
    /// The turn still running or queued, if any; a send waits for it.
    pub pending_turn_id: Option<String>,
    pub can_send: bool,
    /// The conversation revision a later change passes as its expected
    /// revision.
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    /// The revision of the chat's own settings, once it has any.
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub settings_revision: Option<u64>,
    pub archived: bool,
    pub source: ConversationSource,
    /// Group chats only.
    pub chat_mode: Option<GroupChatMode>,
    /// Optional models the chat needs that are not installed.
    pub missing_models: Vec<crate::RequiredModel>,
    /// Why the chat's dynamic memory is stopped, when it is.
    pub memory_blocked: Option<MemoryBlockedReason>,
}

/// Why a chat's dynamic memory does not run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryBlockedReason {
    OwedRewindFailed { code: MemoryRewindFailureCode },
}

/// Why the memory rewind a delete owes could not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryRewindFailureCode {
    Conflict,
    Inconsistent,
    Storage,
    Other,
}

/// Asks for the memory rewind a chat owes to be finished now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryRewindRetryRequest {
    pub conversation_id: String,
}

/// What a retry of an owed memory rewind did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryRewindRetryOutcome {
    NothingOwed,
    Completed,
    StillFailing { code: MemoryRewindFailureCode },
}

/// A page of the selected branch: without cursors the newest messages,
/// with `before_cursor` older ones, with `after_cursor` newer ones (at most
/// one cursor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationMessagesRequest {
    pub conversation_id: String,
    pub before_cursor: Option<String>,
    #[serde(default)]
    pub after_cursor: Option<String>,
    pub limit: Option<u32>,
}

/// Messages around one message of the selected branch, for jumping to it:
/// up to `before` older messages, the message and up to `after` newer ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationMessagesAroundRequest {
    pub conversation_id: String,
    pub message_id: String,
    pub before: u32,
    pub after: u32,
}

/// Visible messages in conversation order, oldest first. `before_cursor`
/// continues with older messages and `after_cursor` with newer ones, as
/// `ConversationMessagesRequest` cursors; none means that side is done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageWindow {
    pub items: Vec<TimelineMessage>,
    pub before_cursor: Option<String>,
    pub after_cursor: Option<String>,
}

/// Edits a message's text. `keep_media` names the attachments it keeps; an
/// edit can remove attachments, never add them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageEditRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    /// Trimmed; blank is refused.
    pub text: String,
    pub keep_media: Vec<String>,
    /// Idempotency key: repeating the call with the same key and request
    /// returns the first result; another request under the key is
    /// `Conflict`.
    pub client_operation_id: String,
}

/// Names one message for a delete or a delete of everything after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageDeleteRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessagePinRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub pinned: bool,
    pub client_operation_id: String,
}

/// Shows another reply variant of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageCandidateSelectRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub candidate_id: String,
    pub client_operation_id: String,
}

/// Switches a one-to-one chat's starting scene from its scene message: the
/// chat's scene setting and the message's text both change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageSceneSelectRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub scene_id: String,
    pub client_operation_id: String,
}

/// A changed message as the chat shows it and the conversation revision the
/// change left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageChanged {
    pub message: TimelineMessage,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MessagesDeleteOutcome {
    Tombstoned { removed: Vec<String> },
    Branched { branch_id: String },
}

/// The outcome of a delete-after and the conversation revision it left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessagesDeleteResult {
    pub outcome: MessagesDeleteOutcome,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageHistoryRequest {
    pub message_id: String,
    pub cursor: Option<String>,
}

/// One saved version of a message's content, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageRevisionView {
    pub id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub sequence: u64,
    pub parts: Vec<MessagePartView>,
    pub reasoning: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub authored_at: i64,
    /// The reply variant this edit rewrote.
    pub supersedes_candidate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageRevisionPage {
    pub items: Vec<MessageRevisionView>,
    pub next_cursor: Option<String>,
}

/// One generated reply variant as it was generated, in variant order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageCandidateView {
    pub id: String,
    pub index: u16,
    pub author_participant_id: Option<String>,
    pub parts: Vec<MessagePartView>,
    pub reasoning: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageCandidatePage {
    pub items: Vec<MessageCandidateView>,
    pub next_cursor: Option<String>,
}

/// Searches the selected branch's visible messages. The query is trimmed;
/// an empty one finds nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSearchRequest {
    pub conversation_id: String,
    pub query: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// A message whose shown text contains the query, ignoring case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SearchHit {
    pub message_id: String,
    pub role: MessageRole,
    /// The speaker in a group chat.
    pub author_participant_id: Option<String>,
    pub text: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
}

/// Search hits oldest first; `next_cursor` continues with later hits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SearchHitPage {
    pub items: Vec<SearchHit>,
    pub next_cursor: Option<String>,
}

/// The pinned visible messages of the selected branch, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationPinnedMessagesRequest {
    pub conversation_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// Visible messages on the selected branch, system notes excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageCount {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MessagePartView {
    Text { text: String },
    Media { asset: AssetRef, role: MediaRole },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TimelineMessage {
    pub id: String,
    pub role: MessageRole,
    pub author_participant_id: Option<String>,
    pub parts: Vec<MessagePartView>,
    pub reasoning: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    /// The shown reply variant's ordinal; set on generated replies only.
    pub candidate_index: Option<u16>,
    pub candidate_count: u32,
    pub pinned: bool,
    /// The scene image the reply asked for, or that was asked of it, until
    /// its image is on the message or the user dismissed it.
    pub scene_image: Option<SceneImageView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SceneImageMode {
    Auto,
    AskFirst,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SceneImageState {
    Pending,
    Approved,
    Running,
    AwaitingTurn,
    Done,
    Failed,
    Dismissed,
}

/// Why a scene image failed, where the user can act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SceneImageFailure {
    MediaUnavailable,
    Disabled,
    NoModel,
    NoImage,
    MessageUnavailable,
    Interrupted,
    Failed,
}

/// The scene image of a reply. `job_id` is the image job, once one exists;
/// watch it for progress. An approved image needs no further input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SceneImageView {
    pub state: SceneImageState,
    pub mode: SceneImageMode,
    pub prompt: String,
    pub job_id: Option<String>,
    pub failure: Option<SceneImageFailure>,
}

/// Names one message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageSceneRequest {
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageScenePromptGenerateRequest {
    pub message_id: String,
    pub client_operation_id: String,
}

/// Approves the scene image a reply asked for, with the prompt as the user
/// edited it. `prompt` is trimmed and a blank one is refused; without it the
/// reply's own prompt is used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageSceneImageApproveRequest {
    pub message_id: String,
    pub prompt: Option<String>,
}

/// Asks for a scene image of a reply from `prompt` (trimmed, not blank).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageSceneImageGenerateRequest {
    pub message_id: String,
    pub prompt: String,
    pub client_operation_id: String,
}

/// One page of visible messages in conversation order, oldest first;
/// `next_cursor` loads the page before it, or after it for a page read with
/// `after_cursor` or a pinned-message page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessagePage {
    pub items: Vec<TimelineMessage>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSendRequest {
    pub conversation_id: String,
    pub text: String,
    /// Idempotency key: repeating a send with the same key and text returns
    /// the first send's result.
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SendAccepted {
    pub user_message_id: String,
    pub turn_id: String,
}

/// Generates another variant of a reply. `guidance` and `model_profile_id`
/// are trimmed and a blank one is none. In a group chat the reply is spoken
/// by `forced_speaker_participant_id`, else by the member who spoke the
/// message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationRegenerateRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub guidance: Option<String>,
    pub model_profile_id: Option<String>,
    pub forced_speaker_participant_id: Option<String>,
    pub swap_places: bool,
}

/// Generates a new reply after the newest message, from `forced_speaker_participant_id`
/// in a group chat or by the chat's speaker selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationContinueRequest {
    pub conversation_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub forced_speaker_participant_id: Option<String>,
    pub swap_places: bool,
}

/// Starts a new turn that repeats a failed or cancelled one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationRetryRequest {
    pub conversation_id: String,
    pub turn_id: String,
    pub client_operation_id: String,
}

/// Adds a user message without a reply, for a director who then continues
/// with a chosen speaker. `text` is trimmed; blank is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationAddUserMessageRequest {
    pub conversation_id: String,
    pub text: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

/// A queued turn; its stream is the channel the request passed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GenerationAccepted {
    pub turn_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HelpMeReplyMode {
    New,
    Enrich,
}

/// Asks the help-me-reply model to write the user's next message. `Enrich`
/// improves `current_draft` when it is not blank; `New` ignores it. The text
/// streams as `JobEvent::TextDelta` on `job_watch` and the cleaned reply is
/// the job's `JobResultDto::GeneratedText`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationHelpMeReplyRequest {
    pub conversation_id: String,
    pub mode: HelpMeReplyMode,
    pub current_draft: Option<String>,
    pub swap_places: bool,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum GenerationFailureCode {
    InvalidConversation,
    MissingModel,
    ContextUnavailable,
    SpeakerUnavailable,
    ProviderUnavailable,
    ProviderRejected,
    EmptyOutput,
    TimedOut,
    RecoveryUnavailable,
    EmbeddingUnavailable,
    Internal,
}

/// The stream of one generation turn, delivered on the channel its send
/// passed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GenerationEvent {
    Started {
        turn_id: String,
    },
    Delta {
        turn_id: String,
        text: Option<String>,
        reasoning: Option<String>,
    },
    Completed {
        turn_id: String,
        message_id: String,
    },
    Failed {
        turn_id: String,
        code: GenerationFailureCode,
    },
    Cancelled {
        turn_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GenerationCancelRequest {
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LaunchDirectRequest {
    pub character_id: String,
    /// Trimmed; blank or absent uses the character's name.
    pub title: Option<String>,
    /// A scene of the character; absent uses its default scene.
    pub scene_id: Option<String>,
    /// A chat template of the character; absent starts without one.
    pub starter_id: Option<String>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LaunchDirectResponse {
    pub conversation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LaunchGroupRequest {
    pub group_id: String,
    /// Idempotency key: repeating a launch with the same key returns the
    /// first launch's conversation.
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LaunchGroupResponse {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharactersListRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterSummary {
    pub id: String,
    pub name: String,
    pub avatar: Option<AssetRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterPage {
    pub items: Vec<CharacterSummary>,
    pub next_cursor: Option<String>,
}

/// Where a setting's current value comes from: the conversation's own
/// choice (which the user can reset), its group, its character, its launch,
/// or the app default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingSource {
    Conversation,
    Group,
    Character,
    Launch,
    AppDefault,
}

/// A setting that names one source; `id` is none when the setting is off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingChoice {
    pub id: Option<String>,
    pub source: SettingSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingChoices {
    pub ids: Vec<String>,
    pub source: SettingSource,
}

/// The background the chat shows: an image, hidden, or (neither) whatever
/// the scene, character or group shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingBackground {
    pub asset: Option<AssetRef>,
    pub hidden: bool,
    pub source: SettingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    Manual,
    Dynamic,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingMemory {
    pub mode: MemoryMode,
    pub source: SettingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingChatMode {
    pub mode: GroupChatMode,
    pub source: SettingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingFlag {
    pub value: bool,
    pub source: SettingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SpeakerSelectionMethod {
    Llm,
    Heuristic,
    RoundRobin,
    Director,
    DirectorAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingSpeakerSelection {
    pub method: SpeakerSelectionMethod,
    pub source: SettingSource,
}

/// A group participant as the chat uses it now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ParticipantSettings {
    pub participant_id: String,
    pub character_id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub muted: bool,
    /// The member's own model; none follows the character's default model.
    pub model: SettingChoice,
}

/// A group chat's members: where the member list, the muted flags and the
/// members' models come from, and each member as the chat uses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupMembersSettings {
    pub members_source: SettingSource,
    pub muted_source: SettingSource,
    pub models_source: SettingSource,
    pub participants: Vec<ParticipantSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSettingsGetRequest {
    pub conversation_id: String,
}

/// Every setting of a chat with its current value and where it comes from.
/// A field whose source is `conversation` is the chat's own and can be
/// reset. Group-only fields are none for a one-to-one chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSettingsView {
    pub conversation_id: String,
    pub kind: ConversationKind,
    pub title: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    /// The revision a settings update passes; none until the chat has
    /// settings of its own.
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub settings_revision: Option<u64>,
    pub author_note: Option<String>,
    pub persona: SettingChoice,
    /// The one-to-one prompt, or a group's conversation-mode prompt.
    pub prompt: SettingChoice,
    /// A group's roleplay-mode prompt.
    pub roleplay_prompt: Option<SettingChoice>,
    pub lorebooks: SettingChoices,
    pub model: SettingChoice,
    pub background: SettingBackground,
    pub scene: SettingChoice,
    pub memory: SettingMemory,
    pub chat_mode: Option<SettingChatMode>,
    pub disable_character_lorebooks: Option<SettingFlag>,
    pub speaker_selection: Option<SettingSpeakerSelection>,
    pub members: Option<GroupMembersSettings>,
}

/// Name a source, turn the setting off, or reset it to what the chat
/// follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChoiceChange {
    Set { id: String },
    None,
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum IdChange {
    Set { id: String },
    Reset,
}

/// An empty list turns the chat's lorebooks off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebooksChange {
    Set { ids: Vec<String> },
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackgroundChange {
    Image { asset_id: String },
    Hidden,
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryModeChange {
    Set { mode: MemoryMode },
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatModeChange {
    Set { mode: GroupChatMode },
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FlagChange {
    Set { value: bool },
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeakerSelectionChange {
    Set { method: SpeakerSelectionMethod },
    Reset,
}

/// The fields a settings update changes; an absent field is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSettingsPatch {
    pub persona: Option<ChoiceChange>,
    pub prompt: Option<ChoiceChange>,
    pub roleplay_prompt: Option<ChoiceChange>,
    pub lorebooks: Option<LorebooksChange>,
    pub model: Option<IdChange>,
    pub background: Option<BackgroundChange>,
    pub scene: Option<IdChange>,
    /// Trimmed; blank removes the note.
    pub author_note: Option<String>,
    pub speaker_selection: Option<SpeakerSelectionChange>,
    pub memory: Option<MemoryModeChange>,
    pub chat_mode: Option<ChatModeChange>,
    pub disable_character_lorebooks: Option<FlagChange>,
    /// The members and their muted flags follow the group again.
    #[serde(default)]
    pub reset_members: bool,
    /// The members' models follow the group again.
    #[serde(default)]
    pub reset_member_models: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationSettingsUpdateRequest {
    pub conversation_id: String,
    /// The settings revision the change was made against; none when the chat
    /// has no settings of its own yet.
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expected_settings_revision: Option<u64>,
    pub patch: ConversationSettingsPatch,
}

/// The revisions a conversation change left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationRevisions {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub settings_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationRenameRequest {
    pub conversation_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    /// Trimmed; blank is refused.
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationRequest {
    pub conversation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationParticipantAddRequest {
    pub conversation_id: String,
    pub character_id: String,
    /// Idempotency key: repeating an add with the same key returns the first
    /// add's result.
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationParticipantUpdateRequest {
    pub conversation_id: String,
    pub participant_id: String,
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
    /// The member's own model, or reset to follow the character's default.
    pub model: Option<IdChange>,
    /// Idempotency key: repeating an update with the same key and request
    /// returns the first update's result; another request under the key is
    /// `Conflict`.
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationBranchView {
    pub id: String,
    pub label: String,
    pub parent_branch_id: Option<String>,
    pub fork_message_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub message_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationBranchList {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    pub branches: Vec<ConversationBranchView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationBranchForkRequest {
    pub conversation_id: String,
    pub message_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationBranchMutationRequest {
    pub branch_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationBranchRenameRequest {
    pub branch_id: String,
    pub label: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationBranchChanged {
    pub branch_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationDuplicateRequest {
    pub conversation_id: String,
    pub with_messages: bool,
    pub title: Option<String>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ConversationCopyResult {
    pub conversation_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}
