use lettuce_conversations::{
    InitialMessageOrigin, InitialTimelineDraft, MessageRole, SnapshotArtifactDraft, SnapshotSource,
};
use lettuce_types::{
    CharacterId, ContentHash, ConversationStarterId, PersonaId, Revision, SceneId, StarterMessageId,
};
use serde::Serialize;

use super::request::{DirectConversationLaunchRequest, LaunchSelection};

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
