use lettuce_conversations::IdempotencyKey;
use lettuce_types::{CharacterId, ConversationStarterId, GroupId, PersonaId, SceneId};
use uuid::Uuid;

pub const DIRECT_LAUNCH_REQUEST_FORMAT_V1: u32 = 1;
pub const GROUP_LAUNCH_REQUEST_FORMAT_V1: u32 = 1;

/// A caller's choice for one launch slot: fall back to the character's own
/// default, name a specific record, or opt the slot out entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchSelection<T> {
    Inherit,
    Explicit(T),
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectUserParticipant {
    pub display_name: String,
    pub authored_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectConversationLaunchRequest {
    pub format_version: u32,
    pub title: String,
    pub user: DirectUserParticipant,
    pub character_id: CharacterId,
    pub scene: LaunchSelection<SceneId>,
    pub starter: LaunchSelection<ConversationStarterId>,
    pub persona: LaunchSelection<PersonaId>,
    /// One key per launch attempt: it fixes the conversation identity forever,
    /// so reusing a key for a different launch conflicts instead of creating.
    pub operation_key: IdempotencyKey,
}

impl DirectConversationLaunchRequest {
    #[must_use]
    pub fn new_operation_key() -> IdempotencyKey {
        IdempotencyKey::new(format!("launch.direct.{}", Uuid::new_v4()))
            .expect("a generated launch key is always safe text")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupConversationLaunchRequest {
    pub format_version: u32,
    /// A blank title is derived from the member display names instead of
    /// being rejected, matching how the legacy create form named a cast.
    pub title: String,
    pub user: DirectUserParticipant,
    pub group_id: GroupId,
    pub persona: LaunchSelection<PersonaId>,
    pub operation_key: IdempotencyKey,
}

impl GroupConversationLaunchRequest {
    #[must_use]
    pub fn new_operation_key() -> IdempotencyKey {
        IdempotencyKey::new(format!("launch.group.{}", Uuid::new_v4()))
            .expect("a generated launch key is always safe text")
    }
}
