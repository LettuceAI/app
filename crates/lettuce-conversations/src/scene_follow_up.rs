//! The scene image a reply asks for, kept by message: the prompt the reply's
//! scene tag carried, whether the user approves it first, and how far its
//! image got. It is written when the reply is finalized, so an ask-first
//! prompt survives a restart and a replayed turn finds it.

use lettuce_types::{ConversationId, MessageId, RequestId, TimestampMillis};
use serde::{Deserialize, Serialize};

/// What a finalized reply asks for: the prompt and whether the user
/// approves it before an image is generated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneFollowUpDraft {
    pub prompt: String,
    pub ask_first: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneFollowUpMode {
    /// The image is generated as soon as the reply is final.
    Auto,
    /// The user approves, and may edit, the prompt first.
    AskFirst,
    /// The user asked for the image from the message.
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneFollowUpState {
    /// Waiting for the user, or for the auto follow-up to be admitted.
    Pending,
    /// An image job is admitted and has not started.
    Approved,
    /// The image job runs.
    Running,
    /// The image is on the message.
    Done,
    Failed,
    Dismissed,
}

impl SceneFollowUpState {
    /// Whether an image job is admitted or running for it.
    #[must_use]
    pub const fn generating(self) -> bool {
        matches!(self, Self::Approved | Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneFollowUp {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub prompt: String,
    pub mode: SceneFollowUpMode,
    pub state: SceneFollowUpState,
    /// How many times an image was asked for this message; each one has its
    /// own request ids.
    pub generation: u32,
    /// The attempt the current request id belongs to, from 1 while an image
    /// is generated.
    pub attempt: u32,
    /// The image request of the current attempt.
    pub request_id: Option<RequestId>,
    /// Why it failed, as a label the API turns into a reason.
    pub failure: Option<String>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

/// Changes to a follow-up; an absent field is kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneFollowUpChange {
    pub state: Option<SceneFollowUpState>,
    pub prompt: Option<String>,
    pub mode: Option<SceneFollowUpMode>,
    pub request_id: Option<Option<RequestId>>,
    pub attempt: Option<u32>,
    pub next_generation: bool,
    pub failure: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SceneFollowUpError {
    #[error("the message has no such follow-up")]
    NotFound,
    #[error("the scene follow-up storage failed")]
    Storage,
}

pub trait SceneFollowUpRepository: Send + Sync {
    fn get_follow_up(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError>;

    /// The follow-ups of `message_ids`; a message without one is left out.
    fn follow_ups_of(
        &self,
        conversation_id: ConversationId,
        message_ids: &[MessageId],
    ) -> Result<Vec<SceneFollowUp>, SceneFollowUpError>;

    /// The follow-up whose current attempt is image request `request_id`.
    fn follow_up_of_request(
        &self,
        request_id: RequestId,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError>;

    /// Every follow-up in one of `states`, oldest first.
    fn follow_ups_in(
        &self,
        states: &[SceneFollowUpState],
    ) -> Result<Vec<SceneFollowUp>, SceneFollowUpError>;

    /// Creates the follow-up of a message unless it has one; returns the one
    /// it has.
    fn ensure_follow_up(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
        prompt: &str,
        mode: SceneFollowUpMode,
        now: TimestampMillis,
    ) -> Result<SceneFollowUp, SceneFollowUpError>;

    /// Applies `change` when the follow-up is in one of `from`; `None` when
    /// it is not, or has none.
    fn change_follow_up(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
        from: &[SceneFollowUpState],
        change: &SceneFollowUpChange,
        now: TimestampMillis,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError>;
}
