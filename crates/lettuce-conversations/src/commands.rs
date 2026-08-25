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
use crate::snapshot::{ModelSelectionSnapshot, ProtectedSnapshotRef, ValidateSnapshot};
use crate::validation::{MAX_PARTS, validate_collection, validate_text};

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
        validate_collection("initial_timeline.entries", &self.entries, 512)?;
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
            validate_collection("initial_timeline.parts", &entry.parts, MAX_PARTS)?;
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
        validate_collection(
            "conversation_plan.participants",
            &self.participants,
            crate::validation::MAX_PARTICIPANTS,
        )?;
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
        validate_collection("message_draft.parts", &self.parts, MAX_PARTS)?;
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
/// intent and makes success finalization impossible.  The job is cancelled
/// outside the repository before [`SettleCancellation`] is committed.
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
        validate_collection("message_edit.parts", &self.parts, MAX_PARTS)?;
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

/// Metadata only: an in-flight generation keeps running, matching legacy.
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
}

impl UpdateParticipantPolicy {
    pub fn validate_against_participants(
        &self,
        participants: &[crate::model::ConversationParticipant],
    ) -> Result<(), ValidationError> {
        let participant = participants
            .iter()
            .find(|participant| participant.id == self.participant_id)
            .ok_or(ValidationError::InvalidReference {
                field: "participant_policy.participant_id",
            })?;
        if participant.role != ParticipantRole::Character {
            return Err(ValidationError::InvalidReference {
                field: "participant_policy.character",
            });
        }
        Ok(())
    }
}

impl RegenerateCandidate {
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
            || message.active_render_source
                != MessageRenderSource::Candidate(self.active_candidate_id)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PatchValue<T> {
    Keep,
    Set(T),
    Clear,
    UseLaunchDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentConversationSettingsPatch {
    pub author_note: PatchValue<String>,
    pub memory: PatchValue<crate::snapshot::MemorySettingsSnapshot>,
    pub model_override: PatchValue<ModelSelectionSnapshot>,
    pub voice: PatchValue<crate::snapshot::VoiceSettingsSnapshot>,
}

impl CurrentConversationSettingsPatch {
    pub fn validate(&self) -> Result<(), ValidationError> {
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
        let empty = crate::model::CurrentConversationSettings {
            revision,
            author_note: None,
            author_note_provenance: SettingProvenance::LaunchInherited,
            memory: None,
            memory_provenance: SettingProvenance::LaunchInherited,
            model_override: None,
            model_provenance: SettingProvenance::LaunchInherited,
            voice: None,
            voice_provenance: SettingProvenance::LaunchInherited,
        };
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
        let result = crate::model::CurrentConversationSettings {
            revision,
            author_note,
            author_note_provenance,
            memory,
            memory_provenance,
            model_override,
            model_provenance,
            voice,
            voice_provenance,
        };
        result.validate()?;
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingProvenance {
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
    Tombstone(TombstoneMessage),
    Archive(ArchiveConversation),
    Restore(RestoreConversation),
    Rename(RenameConversation),
    ParticipantPolicy(UpdateParticipantPolicy),
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
                        crate::validation::MAX_REASONING_BYTES,
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
            Self::Tombstone(command) => validate_expected(command.expected_revision),
            Self::Archive(command) => validate_expected(command.expected_revision),
            Self::Restore(command) => validate_expected(command.expected_revision),
            Self::Rename(command) => command.validate(),
            Self::ParticipantPolicy(command) => validate_expected(command.expected_revision),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandTime {
    pub requested_at: TimestampMillis,
}
