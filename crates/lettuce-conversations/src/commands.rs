use lettuce_types::{
    ContentHash, ConversationBranchId, ConversationId, ConversationParticipantId,
    GenerationAttemptId, GenerationTurnId, MessageCandidateId, MessageId, Revision,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::content::{MessagePart, MessageRole, MessageVisibility};
use crate::error::ValidationError;
use crate::generation::{GenerationOperation, IdempotencyKey};
use crate::model::{ConversationKind, ParticipantRole, ParticipantSource};
use crate::snapshot::{ModelSelectionSnapshot, ValidateSnapshot};
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
    pub operation: OperationToken,
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
                        .participant_policy
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
                    if details.participant_policy.members.iter().any(|policy| {
                        self.participants
                            .iter()
                            .find(|participant| participant.id == policy.participant_id)
                            .is_none_or(|participant| {
                                participant.enabled != policy.enabled
                                    || participant.muted != policy.muted
                                    || participant.model_selection != policy.model_override
                            })
                    }) {
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
        match (self.role, self.author_participant_id) {
            (MessageRole::System, Some(_)) | (MessageRole::User | MessageRole::Assistant, None) => {
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

impl SettleCancellation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_revision.get() == 0 || self.expected_turn_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        Ok(())
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
    pub expected_revision: Revision,
    pub operation: OperationToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMessage {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
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
    pub scene_edited: bool,
}

impl MessageEditDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_collection("message_edit.parts", &self.parts, MAX_PARTS)?;
        for part in &self.parts {
            part.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkBranch {
    pub conversation_id: ConversationId,
    pub source_branch_id: ConversationBranchId,
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
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub descendants: DescendantPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescendantPolicy {
    Preserve,
    Tombstone,
    Fork,
}

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
pub struct UpdateParticipantPolicy {
    pub conversation_id: ConversationId,
    pub participant_id: ConversationParticipantId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
    pub model_override: Option<crate::snapshot::SnapshotSelection<ModelSelectionSnapshot>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PatchValue<T> {
    Keep,
    Set(T),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentConversationSettingsPatch {
    pub author_note: PatchValue<String>,
    pub memory: PatchValue<crate::snapshot::MemorySettingsSnapshot>,
    pub model_override: PatchValue<ModelSelectionSnapshot>,
    pub voice: PatchValue<crate::snapshot::VoiceSettingsSnapshot>,
    pub author_note_provenance: PatchValue<SettingProvenance>,
    pub memory_provenance: PatchValue<SettingProvenance>,
    pub model_provenance: PatchValue<SettingProvenance>,
    pub voice_provenance: PatchValue<SettingProvenance>,
}

impl CurrentConversationSettingsPatch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let PatchValue::Set(note) = &self.author_note {
            validate_text(
                "conversation_settings.author_note",
                note,
                crate::validation::MAX_AUTHORED_TEXT_BYTES,
                true,
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
    Fork(ForkBranch),
    SelectBranch(SelectBranch),
    Tombstone(TombstoneMessage),
    Archive(ArchiveConversation),
    Restore(RestoreConversation),
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
                        true,
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
            Self::Fork(command) => validate_expected(command.expected_revision),
            Self::SelectBranch(command) => validate_expected(command.expected_revision),
            Self::Tombstone(command) => validate_expected(command.expected_revision),
            Self::Archive(command) => validate_expected(command.expected_revision),
            Self::Restore(command) => validate_expected(command.expected_revision),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandTime {
    pub requested_at: TimestampMillis,
}
