use lettuce_conversations::{
    GroupChatModeSnapshot, InitialMessageOrigin, InitialTimelineDraft, MemoryModeSnapshot,
    MessageRole, SnapshotArtifactDraft, SnapshotSource,
};
use lettuce_types::{
    CharacterId, ContentHash, ConversationStarterId, GroupId, LorebookId, ModelProfileId,
    PersonaId, PromptDocumentId, Revision, SceneId, StarterMessageId,
};
use serde::Serialize;

use super::policy::Selected;
use super::request::{
    DirectConversationLaunchRequest, GroupConversationLaunchRequest, LaunchSelection,
};

pub(crate) const LAUNCH_INTENT_FORMAT_V1: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum IntentSelectionV1<T> {
    Inherit,
    Explicit(T),
    Disabled,
}

impl<T: Copy> From<LaunchSelection<T>> for IntentSelectionV1<T> {
    fn from(value: LaunchSelection<T>) -> Self {
        match value {
            LaunchSelection::Inherit => Self::Inherit,
            LaunchSelection::Explicit(inner) => Self::Explicit(inner),
            LaunchSelection::Disabled => Self::Disabled,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum IntentResolvedV1<T> {
    Inherited(T),
    Explicit(T),
    Disabled,
}

impl<T> From<Selected<T>> for IntentResolvedV1<T> {
    fn from(value: Selected<T>) -> Self {
        match value {
            Selected::Inherited(inner) => Self::Inherited(inner),
            Selected::Explicit(inner) => Self::Explicit(inner),
            Selected::Disabled => Self::Disabled,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct IntentSourceV1 {
    source: SnapshotSource,
    source_revision: Revision,
    digest: String,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum IntentOriginV1 {
    SelectedScene,
    StarterMessage(StarterMessageId),
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct IntentTimelineEntryV1 {
    ordinal: u32,
    role: MessageRole,
    origin: IntentOriginV1,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct DirectLaunchIntentV1 {
    format_version: u32,
    conversation_kind: &'static str,
    operation_key: String,
    title: String,
    user_display_name: String,
    user_authored_description: Option<String>,
    character_id: CharacterId,
    scene: IntentSelectionV1<SceneId>,
    starter: IntentSelectionV1<ConversationStarterId>,
    persona: IntentSelectionV1<PersonaId>,
    sources: Vec<IntentSourceV1>,
    timeline: Vec<IntentTimelineEntryV1>,
}

/// Digests the caller's intent plus every resolved source revision and payload
/// digest, so reusing one operation key after an edit conflicts instead of
/// silently replaying the older launch.
pub(crate) fn direct_request_digest(
    request: &DirectConversationLaunchRequest,
    ordered_sources: &[&SnapshotArtifactDraft],
    timeline: &InitialTimelineDraft,
) -> Option<ContentHash> {
    let intent = DirectLaunchIntentV1 {
        format_version: LAUNCH_INTENT_FORMAT_V1,
        conversation_kind: "direct",
        operation_key: request.operation_key.as_str().to_owned(),
        title: request.title.clone(),
        user_display_name: request.user.display_name.clone(),
        user_authored_description: request.user.authored_description.clone(),
        character_id: request.character_id,
        scene: request.scene.into(),
        starter: request.starter.into(),
        persona: request.persona.into(),
        sources: ordered_sources
            .iter()
            .map(|draft| IntentSourceV1 {
                source: draft.source,
                source_revision: draft.source_revision,
                digest: draft.digest.as_str().to_owned(),
            })
            .collect(),
        timeline: timeline
            .entries
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| IntentTimelineEntryV1 {
                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                role: entry.role,
                origin: match &entry.origin {
                    InitialMessageOrigin::SelectedScene { .. } => IntentOriginV1::SelectedScene,
                    InitialMessageOrigin::StarterMessage {
                        starter_message_id, ..
                    } => IntentOriginV1::StarterMessage(*starter_message_id),
                },
            })
            .collect(),
    };
    let encoded = serde_json::to_vec(&intent).ok()?;
    ContentHash::parse(blake3::hash(&encoded).to_hex().to_string()).ok()
}

/// One member exactly as the launch resolved it, before it is split across the
/// group document, the participant list, and the participant policy.
#[derive(Debug, Clone)]
pub(crate) struct GroupMemberIntent<'a> {
    pub(crate) ordinal: u32,
    pub(crate) character_id: CharacterId,
    pub(crate) enabled: bool,
    pub(crate) muted: bool,
    pub(crate) model: Selected<ModelProfileId>,
    pub(crate) lorebooks: Selected<&'a [LorebookId]>,
}

#[derive(Debug, Clone)]
pub(crate) struct GroupLaunchIntent<'a> {
    pub(crate) title: &'a str,
    pub(crate) chat_mode: GroupChatModeSnapshot,
    pub(crate) memory_mode: MemoryModeSnapshot,
    pub(crate) members: &'a [GroupMemberIntent<'a>],
    pub(crate) scene: Selected<SceneId>,
    pub(crate) prompt: Selected<PromptDocumentId>,
    pub(crate) lorebooks: Selected<&'a [LorebookId]>,
    pub(crate) persona_lorebooks: Selected<&'a [LorebookId]>,
    pub(crate) model: Selected<ModelProfileId>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct GroupMemberIntentV1<'a> {
    ordinal: u32,
    character_id: CharacterId,
    enabled: bool,
    muted: bool,
    model: IntentResolvedV1<ModelProfileId>,
    lorebooks: IntentResolvedV1<&'a [LorebookId]>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct GroupLaunchIntentV1<'a> {
    format_version: u32,
    conversation_kind: &'static str,
    operation_key: String,
    title: &'a str,
    user_display_name: String,
    user_authored_description: Option<String>,
    group_id: GroupId,
    chat_mode: GroupChatModeSnapshot,
    memory_mode: MemoryModeSnapshot,
    persona: IntentSelectionV1<PersonaId>,
    members: Vec<GroupMemberIntentV1<'a>>,
    scene: IntentResolvedV1<SceneId>,
    prompt: IntentResolvedV1<PromptDocumentId>,
    lorebooks: IntentResolvedV1<&'a [LorebookId]>,
    persona_lorebooks: IntentResolvedV1<&'a [LorebookId]>,
    model: IntentResolvedV1<ModelProfileId>,
    sources: Vec<IntentSourceV1>,
    timeline: Vec<IntentTimelineEntryV1>,
}

/// The group counterpart of [`direct_request_digest`]. It covers the derived
/// title and every per-member resolution as well, so reusing one operation key
/// after a member, binding, or model change conflicts instead of replaying.
pub(crate) fn group_request_digest(
    request: &GroupConversationLaunchRequest,
    intent: &GroupLaunchIntent<'_>,
    ordered_sources: &[&SnapshotArtifactDraft],
    timeline: &InitialTimelineDraft,
) -> Option<ContentHash> {
    let intent = GroupLaunchIntentV1 {
        format_version: LAUNCH_INTENT_FORMAT_V1,
        conversation_kind: "group",
        operation_key: request.operation_key.as_str().to_owned(),
        title: intent.title,
        user_display_name: request.user.display_name.clone(),
        user_authored_description: request.user.authored_description.clone(),
        group_id: request.group_id,
        chat_mode: intent.chat_mode,
        memory_mode: intent.memory_mode,
        persona: request.persona.into(),
        members: intent
            .members
            .iter()
            .map(|member| GroupMemberIntentV1 {
                ordinal: member.ordinal,
                character_id: member.character_id,
                enabled: member.enabled,
                muted: member.muted,
                model: member.model.into(),
                lorebooks: member.lorebooks.into(),
            })
            .collect(),
        scene: intent.scene.into(),
        prompt: intent.prompt.into(),
        lorebooks: intent.lorebooks.into(),
        persona_lorebooks: intent.persona_lorebooks.into(),
        model: intent.model.into(),
        sources: ordered_sources
            .iter()
            .map(|draft| IntentSourceV1 {
                source: draft.source,
                source_revision: draft.source_revision,
                digest: draft.digest.as_str().to_owned(),
            })
            .collect(),
        timeline: timeline
            .entries
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| IntentTimelineEntryV1 {
                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                role: entry.role,
                origin: match &entry.origin {
                    InitialMessageOrigin::SelectedScene { .. } => IntentOriginV1::SelectedScene,
                    InitialMessageOrigin::StarterMessage {
                        starter_message_id, ..
                    } => IntentOriginV1::StarterMessage(*starter_message_id),
                },
            })
            .collect(),
    };
    let encoded = serde_json::to_vec(&intent).ok()?;
    ContentHash::parse(blake3::hash(&encoded).to_hex().to_string()).ok()
}
