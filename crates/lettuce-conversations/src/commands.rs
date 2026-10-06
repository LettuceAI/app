use lettuce_types::{
    ContentHash, ConversationBranchId, ConversationId, ConversationParticipantId,
    GenerationAttemptId, GenerationTurnId, MessageCandidateId, MessageId, MessageRevisionId,
    Revision, StarterMessageId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::content::{Message, MessagePart, MessageRenderSource, MessageRole, MessageVisibility};
use crate::error::ValidationError;
use crate::generation::{GenerationOperation, IdempotencyKey};
use crate::model::{ConversationKind, ParticipantRole, ParticipantSource};
use crate::ports::ContextAttributions;
use crate::snapshot::{
    LorebookLaunchSnapshot, ModelSelectionSnapshot, PersonaLaunchSnapshot, PromptLaunchSnapshot,
    ProtectedSnapshotRef, SceneLaunchSnapshot, ValidateSnapshot,
};
use crate::validation::{validate_text, validate_unique};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationParticipantDraft {
    pub id: ConversationParticipantId,
    pub role: ParticipantRole,
    pub ordinal: u32,
    pub source: ParticipantSource,
    pub enabled: bool,
    pub muted: bool,
    pub display_name: String,
    pub authored_description: Option<String>,
    pub model_selection: crate::snapshot::SnapshotSelection<ModelSelectionSnapshot>,
}

/// Canonical idempotency token: the key alone is never sufficient to replay a
/// mutation because a reused key with a different request must conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationToken {
    pub key: IdempotencyKey,
    pub request_digest: ContentHash,
}

impl ConversationParticipantDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_text(
            "participant_draft.display_name",
            &self.display_name,
            crate::validation::MAX_DISPLAY_CHARS * 4,
            false,
        )?;
        if let Some(description) = &self.authored_description {
            validate_text(
                "participant_draft.description",
                description,
                crate::validation::MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
        }
        self.model_selection.validate("participant_draft.model")?;
        let source_matches_role = matches!(
            (self.role, self.source),
            (ParticipantRole::User, ParticipantSource::User)
                | (ParticipantRole::Character, ParticipantSource::Character(_))
                | (ParticipantRole::System, ParticipantSource::System)
        );
        if !source_matches_role {
            return Err(ValidationError::InvalidReference {
                field: "participant_draft.source",
            });
        }
        if self.role != ParticipantRole::Character
            && !matches!(
                self.model_selection,
                crate::snapshot::SnapshotSelection::Disabled
            )
        {
            return Err(ValidationError::InvalidReference {
                field: "participant_draft.non_character_model",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateConversationPlan {
    pub conversation_id: ConversationId,
    pub title: String,
    pub kind: ConversationKind,
    pub participants: Vec<ConversationParticipantDraft>,
    pub initial_timeline: InitialTimelineDraft,
    pub operation: OperationToken,
    /// The conversation's own settings from its first moment, created with
    /// it; a launch that turns the persona off records that here.
    #[serde(default)]
    pub current_settings: Option<crate::model::CurrentConversationSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// The ordered, authored launch messages. The future application-level
/// `ConversationLaunchPlanner` materializes these from selected scene/starter
/// snapshots and creates their artifact bytes before calling the creator; the
/// database never needs to read snapshot payload bytes.
pub struct InitialTimelineDraft {
    pub format_version: u32,
    pub entries: Vec<InitialMessageDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialMessageDraft {
    pub message_id: MessageId,
    pub revision_id: MessageRevisionId,
    pub origin: InitialMessageOrigin,
    pub role: MessageRole,
    pub author_participant_id: Option<ConversationParticipantId>,
    pub parts: Vec<MessagePart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum InitialMessageOrigin {
    SelectedScene {
        snapshot_ref: ProtectedSnapshotRef,
    },
    StarterMessage {
        snapshot_ref: ProtectedSnapshotRef,
        starter_message_id: StarterMessageId,
    },
}

impl InitialTimelineDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "initial_timeline",
                version: self.format_version,
            });
        }
        let mut messages = std::collections::HashSet::new();
        let mut revisions = std::collections::HashSet::new();
        let mut starters = std::collections::HashSet::new();
        let mut scenes = 0usize;
        for (index, entry) in self.entries.iter().enumerate() {
            if !messages.insert(entry.message_id) || !revisions.insert(entry.revision_id) {
                return Err(ValidationError::Invariant {
                    field: "initial_timeline.identity",
                });
            }
            match &entry.origin {
                InitialMessageOrigin::SelectedScene { snapshot_ref } => {
                    if !matches!(
                        snapshot_ref.source,
                        crate::snapshot::SnapshotSource::Scene(_)
                    ) {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_timeline.scene_source",
                        });
                    }
                    scenes += 1;
                    if index != 0 {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_timeline.scene_first",
                        });
                    }
                    if scenes > 1
                        || entry.role != MessageRole::Scene
                        || entry.author_participant_id.is_some()
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_timeline.scene",
                        });
                    }
                    if !entry.parts.iter().any(|part| matches!(part, MessagePart::Text { text } if !text.trim().is_empty())) { return Err(ValidationError::InvalidValue { field: "initial_timeline.scene_text" }); }
                    snapshot_ref.validate()?;
                }
                InitialMessageOrigin::StarterMessage {
                    snapshot_ref,
                    starter_message_id,
                } => {
                    if !matches!(
                        snapshot_ref.source,
                        crate::snapshot::SnapshotSource::Starter(_)
                    ) {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_timeline.starter_source",
                        });
                    }
                    if !starters.insert(*starter_message_id) {
                        return Err(ValidationError::Invariant {
                            field: "initial_timeline.starter_identity",
                        });
                    }
                    if !matches!(entry.role, MessageRole::User | MessageRole::Assistant)
                        || entry.author_participant_id.is_none()
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_timeline.starter",
                        });
                    }
                    snapshot_ref.validate()?;
                }
            }
            for part in &entry.parts {
                part.validate()?;
            }
        }
        Ok(())
    }
}

impl CreateConversationPlan {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_text(
            "conversation_plan.title",
            &self.title,
            crate::validation::MAX_DISPLAY_CHARS * 4,
            false,
        )?;
        self.kind.validate()?;
        self.initial_timeline.validate()?;
        if let Some(settings) = &self.current_settings {
            settings.validate_against_kind(&self.kind)?;
        }
        for participant in &self.participants {
            participant.validate()?;
        }
        let mut participant_ids = std::collections::HashSet::new();
        for (ordinal, participant) in self.participants.iter().enumerate() {
            if participant.ordinal as usize != ordinal || !participant_ids.insert(participant.id) {
                return Err(ValidationError::Invariant {
                    field: "conversation_plan.participant_identity",
                });
            }
        }
        match &self.kind {
            ConversationKind::Direct(details) => {
                let selected_scene = match &details.scene {
                    crate::snapshot::SnapshotSelection::Inherited(value)
                    | crate::snapshot::SnapshotSelection::Explicit(value) => {
                        Some(&value.snapshot_ref)
                    }
                    crate::snapshot::SnapshotSelection::Disabled => None,
                };
                let selected_starter = match &details.starter {
                    crate::snapshot::SnapshotSelection::Inherited(value)
                    | crate::snapshot::SnapshotSelection::Explicit(value) => {
                        Some(&value.snapshot_ref)
                    }
                    crate::snapshot::SnapshotSelection::Disabled => None,
                };
                for entry in &self.initial_timeline.entries {
                    match &entry.origin {
                        InitialMessageOrigin::SelectedScene { snapshot_ref }
                            if selected_scene != Some(snapshot_ref) =>
                        {
                            return Err(ValidationError::InvalidReference {
                                field: "conversation_plan.direct.scene_origin",
                            });
                        }
                        InitialMessageOrigin::StarterMessage { snapshot_ref, .. }
                            if selected_starter != Some(snapshot_ref) =>
                        {
                            return Err(ValidationError::InvalidReference {
                                field: "conversation_plan.direct.starter_origin",
                            });
                        }
                        InitialMessageOrigin::StarterMessage { .. } => {
                            let participant = entry
                                .author_participant_id
                                .and_then(|id| self.participants.iter().find(|p| p.id == id));
                            if !matches!(
                                (entry.role, participant.map(|p| p.role)),
                                (
                                    crate::content::MessageRole::User,
                                    Some(ParticipantRole::User)
                                ) | (
                                    crate::content::MessageRole::Assistant,
                                    Some(ParticipantRole::Character)
                                )
                            ) {
                                return Err(ValidationError::InvalidReference {
                                    field: "conversation_plan.direct.starter_author",
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            ConversationKind::Group(details) => {
                if matches!(
                    details.group.chat_mode,
                    crate::snapshot::GroupChatModeSnapshot::Conversation
                ) && !self.initial_timeline.entries.is_empty()
                {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation_plan.group.conversation_initial_timeline",
                    });
                }
                if self.initial_timeline.entries.iter().any(|entry| {
                    matches!(entry.origin, InitialMessageOrigin::StarterMessage { .. })
                }) {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation_plan.group.starter_origin",
                    });
                }
                if let Some(entry) = self.initial_timeline.entries.first() {
                    if let InitialMessageOrigin::SelectedScene { snapshot_ref } = &entry.origin {
                        let selected = match &details.group.scene {
                            crate::snapshot::SnapshotSelection::Inherited(value)
                            | crate::snapshot::SnapshotSelection::Explicit(value) => {
                                Some(&value.snapshot_ref)
                            }
                            crate::snapshot::SnapshotSelection::Disabled => None,
                        };
                        if selected != Some(snapshot_ref)
                            || !matches!(
                                details.group.chat_mode,
                                crate::snapshot::GroupChatModeSnapshot::Roleplay
                            )
                        {
                            return Err(ValidationError::InvalidReference {
                                field: "conversation_plan.group.scene_origin",
                            });
                        }
                    }
                }
            }
        }
        match self.kind {
            ConversationKind::Direct(_) => {
                let users = self
                    .participants
                    .iter()
                    .filter(|participant| participant.role == ParticipantRole::User)
                    .count();
                let characters = self
                    .participants
                    .iter()
                    .filter(|participant| participant.role == ParticipantRole::Character)
                    .count();
                if self.participants.len() != 2 || users != 1 || characters != 1 {
                    return Err(ValidationError::Invariant {
                        field: "conversation_plan.direct.participants",
                    });
                }
                if let ConversationKind::Direct(details) = &self.kind {
                    let character_source =
                        self.participants
                            .iter()
                            .find_map(|participant| match participant.source {
                                ParticipantSource::Character(id) => Some(id),
                                _ => None,
                            });
                    if character_source != Some(details.character.source_id) {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_plan.direct.character_source",
                        });
                    }
                    if self
                        .participants
                        .iter()
                        .find(|participant| participant.role == ParticipantRole::Character)
                        .is_none_or(|participant| participant.model_selection != details.model)
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_plan.direct.character_model",
                        });
                    }
                }
                Ok(())
            }
            ConversationKind::Group(_) => {
                let users = self
                    .participants
                    .iter()
                    .filter(|participant| participant.role == ParticipantRole::User)
                    .count();
                let characters = self
                    .participants
                    .iter()
                    .filter(|participant| participant.role == ParticipantRole::Character)
                    .count();
                if self.participants.len() < 2 || users != 1 || characters < 1 {
                    return Err(ValidationError::Invariant {
                        field: "conversation_plan.group.participants",
                    });
                }
                if let ConversationKind::Group(details) = &self.kind {
                    let members = &details.group.members;
                    if members.len() != characters
                        || members.iter().enumerate().any(|(ordinal, member)| {
                            member.ordinal as usize != ordinal
                                || !self.participants.iter().any(|participant| {
                                    participant.role == ParticipantRole::Character
                                        && participant.source
                                            == ParticipantSource::Character(
                                                member.character.source_id,
                                            )
                                        // Conversation ordinals include the
                                        // user at zero; member ordinals are
                                        // local to the character list.
                                        && participant.ordinal == member.ordinal + 1
                                        && participant.enabled == member.enabled
                                        && participant.muted == member.muted
                                        && participant.model_selection == member.model_override
                                })
                        })
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_plan.group.member_bijection",
                        });
                    }
                    let policy_ids: std::collections::HashSet<_> = details
                        .initial_participant_policy
                        .members
                        .iter()
                        .map(|member| member.participant_id)
                        .collect();
                    let participant_ids: std::collections::HashSet<_> = self
                        .participants
                        .iter()
                        .filter(|participant| participant.role == ParticipantRole::Character)
                        .map(|participant| participant.id)
                        .collect();
                    if policy_ids != participant_ids {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_plan.group.policy_ids",
                        });
                    }
                    if details
                        .initial_participant_policy
                        .members
                        .iter()
                        .any(|policy| {
                            self.participants
                                .iter()
                                .find(|participant| participant.id == policy.participant_id)
                                .is_none_or(|participant| {
                                    participant.enabled != policy.enabled
                                        || participant.muted != policy.muted
                                        || participant.model_selection != policy.model_override
                                })
                        })
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_plan.group.policy_values",
                        });
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageDraft {
    pub role: MessageRole,
    pub author_participant_id: Option<ConversationParticipantId>,
    pub parts: Vec<MessagePart>,
    pub visibility: MessageVisibility,
    pub pinned: bool,
    pub scene_edited: bool,
}

impl MessageDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for part in &self.parts {
            part.validate()?;
        }
        if self.visibility == MessageVisibility::Tombstoned {
            return Err(ValidationError::InvalidValue {
                field: "message_draft.visibility",
            });
        }
        if self.scene_edited && self.role != MessageRole::Scene {
            return Err(ValidationError::InvalidValue {
                field: "message_draft.scene_edited",
            });
        }
        match (self.role, self.author_participant_id) {
            (MessageRole::System | MessageRole::Scene, Some(_))
            | (MessageRole::User | MessageRole::Assistant, None) => {
                Err(ValidationError::InvalidReference {
                    field: "message_draft.author",
                })
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationInputSource {
    UserMessage,
    ExistingHead,
    ExistingCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendConversation {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub message: MessageDraft,
    pub swap_roles: bool,
}

impl SendConversation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.message.role != MessageRole::User {
            return Err(ValidationError::InvalidValue {
                field: "send.message.role",
            });
        }
        self.message.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinueConversation {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub expected_revision: Revision,
    pub forced_speaker: Option<ConversationParticipantId>,
    pub swap_roles: bool,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegenerateCandidate {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub message_id: MessageId,
    pub turn_id: GenerationTurnId,
    pub expected_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
    pub active_candidate_id: MessageCandidateId,
    pub guidance: Option<String>,
    pub model_override: Option<ModelSelectionSnapshot>,
    pub forced_speaker: Option<ConversationParticipantId>,
    pub swap_roles: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryGeneration {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub turn_id: GenerationTurnId,
    pub expected_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelGeneration {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub expected_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
}

/// The first half of cancellation.  This mutation only records the user's
/// intent.  A reply that already streamed visible text may still finalize as
/// the stopped reply; otherwise the job is cancelled outside the repository
/// before [`SettleCancellation`] is committed.
pub type RequestCancellation = CancelGeneration;

/// The second half of cancellation, committed after the runtime job has been
/// asked to stop and usage has been recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettleCancellation {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub expected_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
    pub usage_event_id: lettuce_types::UsageEventId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveGroupSpeaker {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
    pub selected_speaker: crate::generation::SelectedSpeakerDecision,
}

impl SettleCancellation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_revision.get() == 0 || self.expected_turn_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        Ok(())
    }
}

impl ResolveGroupSpeaker {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_turn_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        self.selected_speaker.validate_for_persistence()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachAttemptJob {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub expected_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
    pub job_id: lettuce_types::JobId,
}

impl CancelGeneration {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_revision.get() == 0 || self.expected_turn_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        Ok(())
    }
}

impl AttachAttemptJob {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_revision.get() == 0 || self.expected_turn_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareGeneration {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub job_id: lettuce_types::JobId,
    pub expected_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
    pub model: ModelSelectionSnapshot,
    pub attributions: ContextAttributions,
}

impl PrepareGeneration {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_revision.get() == 0 || self.expected_turn_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        self.model
            .validate_snapshot("generation_preparation.model")?;
        self.attributions.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChooseCandidate {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub candidate_id: MessageCandidateId,
    /// The conversation revision, not the message revision.
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMessage {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    /// The conversation revision, not the message revision.
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub draft: MessageEditDraft,
}

/// Appends one media asset to a variant of a reply, whichever variant the
/// message shows. When the message shows that variant the asset becomes part
/// of what it shows; otherwise it waits in the variant's own latest edit, which
/// selecting the variant renders. Carries no conversation revision: the asset
/// belongs to the variant, not to what the conversation looked like when the
/// image was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachSceneMedia {
    pub complete_deferred: bool,
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub target: crate::SceneFollowUpTarget,
    pub operation: OperationToken,
    pub asset_id: lettuce_types::AssetId,
    pub role: crate::content::MediaAssetRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageEditDraft {
    pub parts: Vec<MessagePart>,
    pub visibility: MessageVisibility,
    pub pinned: bool,
    /// Applies to scene-role messages; the edit adapter rejects it elsewhere.
    pub scene_edited: bool,
}

impl MessageEditDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for part in &self.parts {
            part.validate()?;
        }
        if self.visibility == MessageVisibility::Tombstoned {
            return Err(ValidationError::InvalidValue {
                field: "message_edit.visibility",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateMessageFlags {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    /// The conversation revision, not the message revision.
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub pinned: Option<bool>,
    pub visibility: Option<MessageVisibility>,
}

impl UpdateMessageFlags {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.pinned.is_none() && self.visibility.is_none() {
            return Err(ValidationError::Invariant {
                field: "message_flags.empty_patch",
            });
        }
        if self.visibility == Some(MessageVisibility::Tombstoned) {
            return Err(ValidationError::InvalidValue {
                field: "message_flags.visibility",
            });
        }
        Ok(())
    }

    /// The committed message must carry exactly the requested patch.
    pub fn validate_result(&self, message: &Message) -> Result<(), ValidationError> {
        if message.id != self.message_id || message.conversation_id != self.conversation_id {
            return Err(ValidationError::InvalidReference {
                field: "message_flags.result_identity",
            });
        }
        if message.visibility == MessageVisibility::Tombstoned {
            return Err(ValidationError::InvalidValue {
                field: "message_flags.result_visibility",
            });
        }
        if self.pinned.is_some_and(|pinned| pinned != message.pinned)
            || self
                .visibility
                .is_some_and(|visibility| visibility != message.visibility)
        {
            return Err(ValidationError::Invariant {
                field: "message_flags.result_patch",
            });
        }
        message.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkBranch {
    pub conversation_id: ConversationId,
    pub source_branch_id: ConversationBranchId,
    /// `None` forks at the source branch head.  A headless source branch is a
    /// conflict, never an empty fork.
    pub at_message_id: Option<MessageId>,
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameBranch {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub label: String,
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

impl RenameBranch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_expected(self.expected_revision)?;
        validate_text("branch.label", &self.label, 1_048_576, true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteBranch {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

impl DeleteBranch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_expected(self.expected_revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectBranch {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TombstoneMessage {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    /// The conversation revision, not the message revision.
    pub expected_revision: Revision,
    pub operation: OperationToken,
    /// [`DescendantPolicy::Tombstone`] leaves the branch head unchanged, since
    /// a tombstone is a flag and the timeline still renders tombstoned
    /// entries.  The policy is branch-local: cross-branch descendants belong
    /// to `Fork` or to branch archival.
    pub descendants: DescendantPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescendantPolicy {
    Preserve,
    Tombstone,
    Fork,
}

/// Metadata only: an in-flight generation keeps running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveConversation {
    pub conversation_id: ConversationId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreConversation {
    pub conversation_id: ConversationId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameConversation {
    pub conversation_id: ConversationId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub title: String,
}

impl RenameConversation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_expected(self.expected_revision)?;
        validate_text(
            "conversation.title",
            &self.title,
            crate::validation::MAX_DISPLAY_CHARS * 4,
            false,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateParticipantPolicy {
    pub conversation_id: ConversationId,
    pub participant_id: ConversationParticipantId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
    pub model_override: Option<crate::snapshot::SnapshotSelection<ModelSelectionSnapshot>>,
    /// Values written to other participants in the same change: what they
    /// followed from the group before the conversation took over that
    /// aspect.
    #[serde(default)]
    pub materialize: Vec<ParticipantPolicyChange>,
    /// The aspects the conversation owns from this change on.
    #[serde(default)]
    pub overrides: ParticipantOverrides,
}

/// New values for one participant; `None` keeps a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantPolicyChange {
    pub participant_id: ConversationParticipantId,
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
    pub model_override: Option<crate::snapshot::SnapshotSelection<ModelSelectionSnapshot>>,
}

/// Which participant aspects a group conversation owns instead of following
/// its group: the member list, the muted flags, the members' models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantOverrides {
    pub members: bool,
    pub muted: bool,
    pub member_models: bool,
}

impl ParticipantOverrides {
    #[must_use]
    pub const fn any(self) -> bool {
        self.members || self.muted || self.member_models
    }
}

impl UpdateParticipantPolicy {
    /// Every participant change the command writes, the named participant
    /// last.
    #[must_use]
    pub fn changes(&self) -> Vec<ParticipantPolicyChange> {
        let mut changes = self.materialize.clone();
        changes.push(ParticipantPolicyChange {
            participant_id: self.participant_id,
            enabled: self.enabled,
            muted: self.muted,
            model_override: self.model_override.clone(),
        });
        changes
    }

    pub fn validate_against_participants(
        &self,
        participants: &[crate::model::ConversationParticipant],
    ) -> Result<(), ValidationError> {
        validate_unique(
            "participant_policy.participant_ids",
            self.changes().iter().map(|change| change.participant_id),
        )?;
        for change in self.changes() {
            let participant = participants
                .iter()
                .find(|participant| participant.id == change.participant_id)
                .ok_or(ValidationError::InvalidReference {
                    field: "participant_policy.participant_id",
                })?;
            if participant.role != ParticipantRole::Character {
                return Err(ValidationError::InvalidReference {
                    field: "participant_policy.character",
                });
            }
            if let Some(model) = &change.model_override {
                model.validate("participant_policy.model_override")?;
            }
        }
        Ok(())
    }
}

/// Adds a character to a group conversation, or enables the row it kept from
/// an earlier membership. A new row carries `member`, the member snapshot
/// the application built like a launch does; re-enabling an existing row
/// carries none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddConversationParticipant {
    pub conversation_id: ConversationId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub participant_id: ConversationParticipantId,
    pub character_id: lettuce_types::CharacterId,
    pub display_name: String,
    pub muted: bool,
    pub member: Option<crate::snapshot::GroupMemberLaunchSnapshot>,
    /// A membership edit by the user: the conversation stops following the
    /// group's member list, and every other participant keeps in its row
    /// the enabled flag it followed from the group when the add commits. A
    /// member the group gained after launch is added without it.
    pub override_members: bool,
}

impl AddConversationParticipant {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_expected(self.expected_revision)?;
        validate_text(
            "participant_add.display_name",
            &self.display_name,
            crate::validation::MAX_DISPLAY_CHARS * 4,
            false,
        )?;
        if let Some(member) = &self.member {
            member.validate()?;
            if member.character.source_id != self.character_id {
                return Err(ValidationError::InvalidReference {
                    field: "participant_add.member",
                });
            }
        }
        Ok(())
    }
}

/// The rule every group conversation keeps: at least one enabled, unmuted
/// character. `participants` are the effective values after a change.
pub fn require_active_member(
    participants: &[crate::model::ConversationParticipant],
) -> Result<(), ValidationError> {
    if participants.iter().any(|participant| {
        participant.role == ParticipantRole::Character && participant.enabled && !participant.muted
    }) {
        Ok(())
    } else {
        Err(ValidationError::Invariant {
            field: "conversation.group.active_member",
        })
    }
}

impl RegenerateCandidate {
    /// An edited reply renders from its revision and stays regenerable
    /// through any of its candidates; a deleted reply is not a target.
    pub fn validate_target_context(
        &self,
        message: &Message,
        active_branch_id: ConversationBranchId,
        active_head_message_id: Option<MessageId>,
        participants: &[crate::model::ConversationParticipant],
        is_group: bool,
    ) -> Result<(), ValidationError> {
        if message.id != self.message_id
            || message.conversation_id != self.conversation_id
            || message.branch_id != self.branch_id
            || message.branch_id != active_branch_id
            || message.role != MessageRole::Assistant
            || message.visibility == MessageVisibility::Tombstoned
            || matches!(
                message.active_render_source,
                MessageRenderSource::Candidate(active) if active != self.active_candidate_id
            )
            || (!is_group && active_head_message_id != Some(message.id))
        {
            return Err(ValidationError::InvalidReference {
                field: "regenerate.target_message",
            });
        }
        if let Some(forced) = self.forced_speaker {
            let participant = participants
                .iter()
                .find(|participant| participant.id == forced)
                .ok_or(ValidationError::InvalidReference {
                    field: "regenerate.forced_speaker",
                })?;
            if participant.role != ParticipantRole::Character {
                return Err(ValidationError::InvalidReference {
                    field: "regenerate.forced_speaker",
                });
            }
        } else if !participants.iter().any(|participant| {
            Some(participant.id) == message.author_participant_id
                && participant.role == ParticipantRole::Character
        }) {
            return Err(ValidationError::InvalidReference {
                field: "regenerate.target_author",
            });
        }
        Ok(())
    }

    pub fn validate_selected_speaker(
        &self,
        target_author: Option<ConversationParticipantId>,
        selected_speaker: Option<ConversationParticipantId>,
        is_group: bool,
    ) -> Result<(), ValidationError> {
        if !is_group {
            return Ok(());
        }
        let expected = self.forced_speaker.or(target_author);
        if selected_speaker != expected {
            return Err(ValidationError::InvalidReference {
                field: "regenerate.selected_speaker",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub enum PatchValue<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
    UseLaunchDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CurrentConversationSettingsPatch {
    #[serde(default)]
    pub companion_clock: PatchValue<crate::CompanionClockSettings>,
    #[serde(default)]
    pub model_settings: PatchValue<lettuce_models::ModelSettingsLayer>,
    #[serde(default)]
    pub background: PatchValue<crate::model::ConversationBackground>,
    pub author_note: PatchValue<String>,
    pub memory: PatchValue<crate::snapshot::MemorySettingsSnapshot>,
    pub model_override: PatchValue<ModelSelectionSnapshot>,
    pub voice: PatchValue<crate::snapshot::VoiceSettingsSnapshot>,
    pub prompt: PatchValue<PromptLaunchSnapshot>,
    pub lorebooks: PatchValue<Vec<LorebookLaunchSnapshot>>,
    pub persona: PatchValue<PersonaLaunchSnapshot>,
    pub scene: PatchValue<SceneLaunchSnapshot>,
    /// Group conversations only; it cannot be cleared, only set or returned
    /// to the group's method.
    #[serde(default)]
    pub speaker_selection: PatchValue<crate::snapshot::GroupSpeakerSelectionSnapshot>,
    /// Group conversations only: the roleplay-mode prompt; `prompt` is the
    /// conversation-mode one.
    #[serde(default)]
    pub roleplay_prompt: PatchValue<PromptLaunchSnapshot>,
    /// Group conversations only; it cannot be cleared, only set or returned
    /// to the group's.
    #[serde(default)]
    pub chat_mode: PatchValue<crate::snapshot::GroupChatModeSnapshot>,
    /// Group conversations only; it cannot be cleared, only set or returned
    /// to the group's.
    #[serde(default)]
    pub disable_character_lorebooks: PatchValue<bool>,
    /// Group conversations only: the members and their muted flags follow
    /// the group again.
    #[serde(default)]
    pub follow_group_members: bool,
    /// Group conversations only: the members' models follow the group again.
    #[serde(default)]
    pub follow_group_member_models: bool,
}

impl CurrentConversationSettingsPatch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let PatchValue::Set(clock) = self.companion_clock {
            clock.validate()?;
        }
        if let PatchValue::Set(settings) = &self.model_settings {
            settings
                .validate()
                .map_err(|_| ValidationError::InvalidValue {
                    field: "conversation_settings.model_settings",
                })?;
        }
        if matches!(self.speaker_selection, PatchValue::Clear) {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.speaker_selection",
            });
        }
        if matches!(self.chat_mode, PatchValue::Clear) {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.chat_mode",
            });
        }
        if matches!(self.disable_character_lorebooks, PatchValue::Clear) {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.disable_character_lorebooks",
            });
        }
        if let PatchValue::Set(prompt) = &self.roleplay_prompt {
            prompt.validate()?;
        }
        if let PatchValue::Set(note) = &self.author_note {
            validate_text(
                "conversation_settings.author_note",
                note,
                crate::validation::MAX_AUTHORED_TEXT_BYTES,
                false,
            )?;
        }
        if let PatchValue::Set(memory) = &self.memory {
            memory.validate()?;
        }
        if let PatchValue::Set(model) = &self.model_override {
            model.validate_snapshot("conversation_settings.model_override")?;
        }
        if let PatchValue::Set(voice) = &self.voice {
            voice.validate()?;
        }
        if let PatchValue::Set(prompt) = &self.prompt {
            prompt.validate()?;
        }
        if let PatchValue::Set(lorebooks) = &self.lorebooks {
            if lorebooks.is_empty() {
                return Err(ValidationError::InvalidValue {
                    field: "conversation_settings.lorebooks",
                });
            }
            validate_unique(
                "conversation_settings.lorebook_ids",
                lorebooks.iter().map(|book| book.source_id),
            )?;
            for book in lorebooks {
                book.validate()?;
            }
        }
        if let PatchValue::Set(persona) = &self.persona {
            persona.validate()?;
        }
        if let PatchValue::Set(scene) = &self.scene {
            scene.validate()?;
        }
        Ok(())
    }

    /// Materializes the repository-owned settings state and applies the
    /// command's optimistic-concurrency requirement.  Callers can only
    /// choose Keep/Set/Clear/UseLaunchDefault; provenance is derived here and
    /// cannot be forged.
    pub fn apply(
        &self,
        current: Option<&crate::model::CurrentConversationSettings>,
        expected_revision: Option<Revision>,
    ) -> Result<crate::model::CurrentConversationSettings, ValidationError> {
        self.validate()?;
        if let Some(expected) = expected_revision {
            if expected.get() == 0 {
                return Err(ValidationError::ZeroRevision);
            }
            if current.is_none_or(|settings| settings.revision != expected) {
                return Err(ValidationError::InvalidReference {
                    field: "conversation_settings.expected_revision",
                });
            }
        } else if current.is_some() {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.create_only",
            });
        }
        let revision = match current {
            Some(settings) => {
                settings
                    .revision
                    .next()
                    .map_err(|_| ValidationError::OutOfBounds {
                        field: "conversation_settings.revision",
                    })?
            }
            None => Revision::INITIAL,
        };
        fn apply_value<T: Clone>(
            patch: &PatchValue<T>,
            current_value: Option<&T>,
            current_provenance: SettingProvenance,
            has_current: bool,
        ) -> (Option<T>, SettingProvenance) {
            match patch {
                PatchValue::Keep => (
                    current_value.cloned(),
                    if has_current {
                        current_provenance
                    } else {
                        SettingProvenance::LaunchInherited
                    },
                ),
                PatchValue::Set(value) => (Some(value.clone()), SettingProvenance::CurrentOverride),
                PatchValue::Clear => (None, SettingProvenance::Disabled),
                PatchValue::UseLaunchDefault => (None, SettingProvenance::LaunchInherited),
            }
        }
        let empty = crate::model::CurrentConversationSettings::inherited(revision);
        let base = current.unwrap_or(&empty);
        let (author_note, author_note_provenance) = apply_value(
            &self.author_note,
            base.author_note.as_ref(),
            base.author_note_provenance,
            current.is_some(),
        );
        let (memory, memory_provenance) = apply_value(
            &self.memory,
            base.memory.as_ref(),
            base.memory_provenance,
            current.is_some(),
        );
        let (model_override, model_provenance) = apply_value(
            &self.model_override,
            base.model_override.as_ref(),
            base.model_provenance,
            current.is_some(),
        );
        let (voice, voice_provenance) = apply_value(
            &self.voice,
            base.voice.as_ref(),
            base.voice_provenance,
            current.is_some(),
        );
        let (prompt, prompt_provenance) = apply_value(
            &self.prompt,
            base.prompt.as_ref(),
            base.prompt_provenance,
            current.is_some(),
        );
        let (lorebooks, lorebooks_provenance) = apply_value(
            &self.lorebooks,
            base.lorebooks.as_ref(),
            base.lorebooks_provenance,
            current.is_some(),
        );
        let (persona, persona_provenance) = apply_value(
            &self.persona,
            base.persona.as_ref(),
            base.persona_provenance,
            current.is_some(),
        );
        let (scene, scene_provenance) = apply_value(
            &self.scene,
            base.scene.as_ref(),
            base.scene_provenance,
            current.is_some(),
        );
        let (speaker_selection, speaker_selection_provenance) = apply_value(
            &self.speaker_selection,
            base.speaker_selection.as_ref(),
            base.speaker_selection_provenance,
            current.is_some(),
        );
        let (roleplay_prompt, roleplay_prompt_provenance) = apply_value(
            &self.roleplay_prompt,
            base.roleplay_prompt.as_ref(),
            base.roleplay_prompt_provenance,
            current.is_some(),
        );
        let result = crate::model::CurrentConversationSettings {
            companion_clock: match self.companion_clock {
                PatchValue::Keep => base.companion_clock,
                PatchValue::Set(clock) => Some(clock),
                PatchValue::Clear | PatchValue::UseLaunchDefault => None,
            },
            model_settings: match &self.model_settings {
                PatchValue::Keep => base.model_settings.clone(),
                PatchValue::Set(settings) => settings.clone(),
                PatchValue::Clear | PatchValue::UseLaunchDefault => Default::default(),
            },
            background: match self.background {
                PatchValue::Keep => base.background,
                PatchValue::Set(background) => Some(background),
                PatchValue::Clear | PatchValue::UseLaunchDefault => None,
            },
            revision,
            author_note,
            author_note_provenance,
            memory,
            memory_provenance,
            model_override,
            model_provenance,
            voice,
            voice_provenance,
            prompt,
            prompt_provenance,
            lorebooks,
            lorebooks_provenance,
            persona,
            persona_provenance,
            scene,
            scene_provenance,
            speaker_selection,
            speaker_selection_provenance,
            chat_mode: match self.chat_mode {
                PatchValue::Keep => base.chat_mode,
                PatchValue::Set(mode) => Some(mode),
                PatchValue::Clear | PatchValue::UseLaunchDefault => None,
            },
            disable_character_lorebooks: match self.disable_character_lorebooks {
                PatchValue::Keep => base.disable_character_lorebooks,
                PatchValue::Set(disabled) => Some(disabled),
                PatchValue::Clear | PatchValue::UseLaunchDefault => None,
            },
            roleplay_prompt,
            roleplay_prompt_provenance,
            members_overridden: base.members_overridden && !self.follow_group_members,
            muted_overridden: base.muted_overridden && !self.follow_group_members,
            member_models_overridden: base.member_models_overridden
                && !self.follow_group_member_models,
        };
        result.validate()?;
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingProvenance {
    #[default]
    LaunchInherited,
    CurrentOverride,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateConversationSettings {
    pub conversation_id: ConversationId,
    /// `None` is create-only: the repository must reject the mutation when a
    /// current settings record already exists. `Some(revision)` is an exact
    /// CAS against an existing record; the repository owns INITIAL/next
    /// revision assignment.
    pub expected_settings_revision: Option<Revision>,
    pub operation: OperationToken,
    pub patch: CurrentConversationSettingsPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCasRequirement {
    CreateOnly,
    Exact(Revision),
}

impl UpdateConversationSettings {
    #[must_use]
    pub const fn cas_requirement(&self) -> SettingsCasRequirement {
        match self.expected_settings_revision {
            Some(revision) => SettingsCasRequirement::Exact(revision),
            None => SettingsCasRequirement::CreateOnly,
        }
    }
}

impl UpdateConversationSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(revision) = self.expected_settings_revision {
            validate_expected(revision)?;
        }
        self.patch.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ConversationMutation {
    Send(SendConversation),
    Continue(ContinueConversation),
    Regenerate(RegenerateCandidate),
    Retry(RetryGeneration),
    Cancel(CancelGeneration),
    Choose(ChooseCandidate),
    Edit(EditMessage),
    Flags(UpdateMessageFlags),
    Fork(ForkBranch),
    SelectBranch(SelectBranch),
    RenameBranch(RenameBranch),
    DeleteBranch(DeleteBranch),
    Tombstone(TombstoneMessage),
    Archive(ArchiveConversation),
    Restore(RestoreConversation),
    Rename(RenameConversation),
    ParticipantPolicy(UpdateParticipantPolicy),
    ParticipantAdd(AddConversationParticipant),
    Settings(UpdateConversationSettings),
}

impl ConversationMutation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Send(command) => {
                validate_expected(command.expected_revision)?;
                command.validate()
            }
            Self::Continue(command) => validate_expected(command.expected_revision),
            Self::Regenerate(command) => {
                validate_expected(command.expected_revision)?;
                validate_expected(command.expected_turn_revision)?;
                if let Some(guidance) = &command.guidance {
                    validate_text(
                        "regenerate.guidance",
                        guidance,
                        crate::validation::MAX_GUIDANCE_BYTES,
                        false,
                    )?;
                }
                if let Some(model) = &command.model_override {
                    model.validate()?;
                }
                Ok(())
            }
            Self::Retry(command) => {
                validate_expected(command.expected_revision)?;
                validate_expected(command.expected_turn_revision)
            }
            Self::Cancel(command) => {
                validate_expected(command.expected_revision)?;
                validate_expected(command.expected_turn_revision)
            }
            Self::Choose(command) => validate_expected(command.expected_revision),
            Self::Edit(command) => {
                validate_expected(command.expected_revision)?;
                command.draft.validate()
            }
            Self::Flags(command) => {
                validate_expected(command.expected_revision)?;
                command.validate()
            }
            Self::Fork(command) => validate_expected(command.expected_revision),
            Self::SelectBranch(command) => validate_expected(command.expected_revision),
            Self::RenameBranch(command) => command.validate(),
            Self::DeleteBranch(command) => command.validate(),
            Self::Tombstone(command) => validate_expected(command.expected_revision),
            Self::Archive(command) => validate_expected(command.expected_revision),
            Self::Restore(command) => validate_expected(command.expected_revision),
            Self::Rename(command) => command.validate(),
            Self::ParticipantPolicy(command) => validate_expected(command.expected_revision),
            Self::ParticipantAdd(command) => command.validate(),
            Self::Settings(command) => command.validate(),
        }
    }

    #[must_use]
    pub const fn operation(&self) -> Option<GenerationOperation> {
        match self {
            Self::Send(_) => Some(GenerationOperation::Send),
            Self::Continue(_) => Some(GenerationOperation::Continue),
            Self::Regenerate(_) => Some(GenerationOperation::Regenerate),
            Self::Retry(_) => None,
            _ => None,
        }
    }
}

fn validate_expected(revision: Revision) -> Result<(), ValidationError> {
    if revision.get() == 0 {
        Err(ValidationError::ZeroRevision)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_title_uses_conversation_bounds() {
        let mut command = RenameConversation {
            conversation_id: ConversationId::new(),
            expected_revision: Revision::INITIAL,
            operation: OperationToken {
                key: IdempotencyKey::new("rename").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            title: "   ".into(),
        };
        assert_eq!(
            command.validate(),
            Err(ValidationError::Blank {
                field: "conversation.title"
            })
        );
        command.title = "a".repeat(crate::validation::MAX_DISPLAY_CHARS * 4 + 1);
        assert_eq!(
            command.validate(),
            Err(ValidationError::TooLarge {
                field: "conversation.title"
            })
        );
        command.title = "Renamed".into();
        command.validate().expect("valid title");
    }

    #[test]
    fn group_switches_cannot_be_cleared_and_resets_follow_the_group() {
        for patch in [
            CurrentConversationSettingsPatch {
                chat_mode: PatchValue::Clear,
                ..CurrentConversationSettingsPatch::default()
            },
            CurrentConversationSettingsPatch {
                disable_character_lorebooks: PatchValue::Clear,
                ..CurrentConversationSettingsPatch::default()
            },
        ] {
            assert!(patch.apply(None, None).is_err());
        }
        let set = CurrentConversationSettingsPatch {
            chat_mode: PatchValue::Set(crate::snapshot::GroupChatModeSnapshot::Roleplay),
            disable_character_lorebooks: PatchValue::Set(true),
            ..CurrentConversationSettingsPatch::default()
        }
        .apply(None, None)
        .expect("set");
        assert_eq!(
            set.chat_mode,
            Some(crate::snapshot::GroupChatModeSnapshot::Roleplay)
        );
        assert_eq!(set.disable_character_lorebooks, Some(true));
        let mut owned = set.clone();
        owned.members_overridden = true;
        owned.muted_overridden = true;
        owned.member_models_overridden = true;
        let reset = CurrentConversationSettingsPatch {
            chat_mode: PatchValue::UseLaunchDefault,
            disable_character_lorebooks: PatchValue::UseLaunchDefault,
            follow_group_members: true,
            ..CurrentConversationSettingsPatch::default()
        }
        .apply(Some(&owned), Some(owned.revision))
        .expect("reset");
        assert_eq!(reset.chat_mode, None);
        assert_eq!(reset.disable_character_lorebooks, None);
        assert!(!reset.members_overridden && !reset.muted_overridden);
        assert!(reset.member_models_overridden);
        let models = CurrentConversationSettingsPatch {
            follow_group_member_models: true,
            ..CurrentConversationSettingsPatch::default()
        }
        .apply(Some(&reset), Some(reset.revision))
        .expect("reset models");
        assert!(!models.member_models_overridden);
    }

    #[test]
    fn a_group_keeps_one_enabled_unmuted_character() {
        let participant = |role, enabled, muted| crate::model::ConversationParticipant {
            id: ConversationParticipantId::new(),
            role,
            ordinal: 0,
            enabled,
            muted,
            source: match role {
                ParticipantRole::Character => {
                    ParticipantSource::Character(lettuce_types::CharacterId::new())
                }
                _ => ParticipantSource::User,
            },
            display_name: "Name".into(),
            authored_description: None,
            model_selection: crate::snapshot::SnapshotSelection::Disabled,
            member_snapshot: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let user = participant(ParticipantRole::User, true, false);
        let muted = participant(ParticipantRole::Character, true, true);
        let disabled = participant(ParticipantRole::Character, false, false);
        let active = participant(ParticipantRole::Character, true, false);
        assert_eq!(
            require_active_member(&[user.clone(), muted.clone(), disabled.clone()]),
            Err(ValidationError::Invariant {
                field: "conversation.group.active_member"
            })
        );
        assert!(require_active_member(&[user, muted, disabled, active]).is_ok());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandTime {
    pub requested_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedConversationCopyKind {
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationContentCopy {
    pub source_conversation_id: ConversationId,
    pub source_branch_id: ConversationBranchId,
    pub target_conversation_id: ConversationId,
    pub target_branch_id: ConversationBranchId,
    pub through_message_id: Option<MessageId>,
    pub kind: SelectedConversationCopyKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateConversation {
    pub source_conversation_id: ConversationId,
    pub conversation_id: ConversationId,
    pub title: Option<String>,
    pub with_messages: bool,
    pub operation: OperationToken,
}
