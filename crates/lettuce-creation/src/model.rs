use lettuce_types::{
    CharacterId, CreationProposalId, CreationTurnId, CreationWorkflowId, LorebookEntryId,
    LorebookId, PersonaId, Revision, SceneId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

pub const MAX_CREATION_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_CREATION_USER_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_CREATION_SCENES: usize = 128;
pub const MAX_CREATION_LOREBOOK_ENTRIES: usize = 2_048;
pub const MAX_CREATION_DRAFT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationTargetKind {
    Character,
    Persona,
    Lorebook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreationTarget {
    NewCharacter,
    ExistingCharacter { id: CharacterId, revision: Revision },
    NewPersona,
    ExistingPersona { id: PersonaId, revision: Revision },
    NewLorebook,
    ExistingLorebook { id: LorebookId, revision: Revision },
}

impl CreationTarget {
    #[must_use]
    pub const fn kind(&self) -> CreationTargetKind {
        match self {
            Self::NewCharacter | Self::ExistingCharacter { .. } => CreationTargetKind::Character,
            Self::NewPersona | Self::ExistingPersona { .. } => CreationTargetKind::Persona,
            Self::NewLorebook | Self::ExistingLorebook { .. } => CreationTargetKind::Lorebook,
        }
    }

    pub fn validate(&self) -> Result<(), super::CreationProposalError> {
        let revision = match self {
            Self::ExistingCharacter { revision, .. }
            | Self::ExistingPersona { revision, .. }
            | Self::ExistingLorebook { revision, .. } => Some(*revision),
            Self::NewCharacter | Self::NewPersona | Self::NewLorebook => None,
        };
        if revision.is_some_and(|revision| revision.get() == 0) {
            return Err(super::CreationProposalError::InvalidTarget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationStage {
    Drafting,
    AwaitingReview,
    AwaitingConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationScene {
    pub id: SceneId,
    pub content: String,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationLorebookEntry {
    pub id: LorebookEntryId,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreationDraft {
    Character {
        name: Option<String>,
        definition: Option<String>,
        scenes: Vec<CreationScene>,
    },
    Persona {
        name: Option<String>,
        description: Option<String>,
    },
    Lorebook {
        name: Option<String>,
        description: Option<String>,
        entries: Vec<CreationLorebookEntry>,
    },
}

impl CreationDraft {
    #[must_use]
    pub const fn kind(&self) -> CreationTargetKind {
        match self {
            Self::Character { .. } => CreationTargetKind::Character,
            Self::Persona { .. } => CreationTargetKind::Persona,
            Self::Lorebook { .. } => CreationTargetKind::Lorebook,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), super::CreationProposalError> {
        match self {
            Self::Character {
                name,
                definition,
                scenes,
            } => {
                validate_optional(name)?;
                validate_optional(definition)?;
                if scenes.len() > MAX_CREATION_SCENES {
                    return Err(super::CreationProposalError::DraftTooLarge);
                }
                for scene in scenes {
                    validate_required(&scene.content)?;
                    validate_optional(&scene.direction)?;
                }
                ensure_unique(scenes.iter().map(|scene| scene.id))?;
            }
            Self::Persona { name, description } => {
                validate_optional(name)?;
                validate_optional(description)?;
            }
            Self::Lorebook {
                name,
                description,
                entries,
            } => {
                validate_optional(name)?;
                validate_optional(description)?;
                if entries.len() > MAX_CREATION_LOREBOOK_ENTRIES {
                    return Err(super::CreationProposalError::DraftTooLarge);
                }
                for entry in entries {
                    validate_required(&entry.title)?;
                    validate_required(&entry.content)?;
                }
                ensure_unique(entries.iter().map(|entry| entry.id))?;
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| super::CreationProposalError::DraftTooLarge)?
            .len()
            > MAX_CREATION_DRAFT_BYTES
        {
            return Err(super::CreationProposalError::DraftTooLarge);
        }
        Ok(())
    }
}

fn ensure_unique<T: Ord>(
    values: impl Iterator<Item = T>,
) -> Result<(), super::CreationProposalError> {
    let mut seen = std::collections::BTreeSet::new();
    if values.into_iter().all(|value| seen.insert(value)) {
        Ok(())
    } else {
        Err(super::CreationProposalError::DuplicateDraftIdentity)
    }
}

pub(crate) fn validate_required(value: &str) -> Result<(), super::CreationProposalError> {
    if value.trim().is_empty() {
        return Err(super::CreationProposalError::BlankText);
    }
    if value.len() > MAX_CREATION_TEXT_BYTES {
        return Err(super::CreationProposalError::DraftTooLarge);
    }
    Ok(())
}

pub(crate) fn validate_optional(
    value: &Option<String>,
) -> Result<(), super::CreationProposalError> {
    if let Some(value) = value {
        validate_required(value)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCreationWorkflow {
    pub id: CreationWorkflowId,
    pub initial_proposal_id: CreationProposalId,
    pub target: CreationTarget,
    pub initial_draft: CreationDraft,
    pub now: TimestampMillis,
}

impl NewCreationWorkflow {
    pub fn validate(&self) -> Result<(), super::CreationProposalError> {
        self.target.validate()?;
        self.initial_draft.validate()?;
        if self.target.kind() != self.initial_draft.kind() {
            return Err(super::CreationProposalError::InvalidTarget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationWorkflow {
    pub id: CreationWorkflowId,
    pub target: CreationTarget,
    pub stage: CreationStage,
    pub current_proposal_id: CreationProposalId,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCreationTurn {
    pub id: CreationTurnId,
    pub workflow_id: CreationWorkflowId,
    pub base_proposal_id: CreationProposalId,
    pub user_message: String,
    pub now: TimestampMillis,
}

impl NewCreationTurn {
    pub fn validate(&self) -> Result<(), super::CreationProposalError> {
        if self.user_message.trim().is_empty() {
            return Err(super::CreationProposalError::BlankText);
        }
        if self.user_message.len() > MAX_CREATION_USER_MESSAGE_BYTES {
            return Err(super::CreationProposalError::DraftTooLarge);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationTurn {
    pub id: CreationTurnId,
    pub workflow_id: CreationWorkflowId,
    pub ordinal: u32,
    pub base_proposal_id: CreationProposalId,
    pub user_message: String,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedPersonaApply {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub destination_persona_id: PersonaId,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedPersonaRevisionApply {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub persona_id: PersonaId,
    pub expected_persona_revision: Revision,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedCharacterApply {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub destination_character_id: CharacterId,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedLorebookApply {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub destination_lorebook_id: LorebookId,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedLorebookRevisionApply {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub lorebook_id: LorebookId,
    pub expected_lorebook_revision: Revision,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationApplyReceipt {
    pub workflow_id: CreationWorkflowId,
    pub workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub persona_id: PersonaId,
    pub persona_revision: Revision,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationCharacterApplyReceipt {
    pub workflow_id: CreationWorkflowId,
    pub workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub character_id: CharacterId,
    pub character_revision: Revision,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationLorebookApplyReceipt {
    pub workflow_id: CreationWorkflowId,
    pub workflow_revision: Revision,
    pub proposal_id: CreationProposalId,
    pub lorebook_id: LorebookId,
    pub lorebook_revision: Revision,
    pub applied_at: TimestampMillis,
}
