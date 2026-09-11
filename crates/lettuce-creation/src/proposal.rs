use lettuce_types::{
    CreationProposalId, CreationTurnId, LorebookEntryId, SceneId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::{CreationDraft, CreationLorebookEntry, CreationScene, CreationStage};

pub const MAX_CREATION_OPERATIONS: usize = 64;
pub const MAX_CREATION_PROPOSAL_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreationOperation {
    SetName {
        value: String,
    },
    SetDescription {
        value: String,
    },
    AddScene {
        id: SceneId,
        content: String,
        direction: Option<String>,
    },
    UpdateScene {
        id: SceneId,
        content: String,
        direction: Option<String>,
    },
    DeleteScene {
        id: SceneId,
    },
    UpsertLorebookEntry {
        id: LorebookEntryId,
        title: String,
        content: String,
    },
    UpdateLorebookEntry {
        id: LorebookEntryId,
        title: String,
        content: String,
    },
    DeleteLorebookEntry {
        id: LorebookEntryId,
    },
    ReorderLorebookEntries {
        order: Vec<LorebookEntryId>,
    },
    ShowPreview,
    RequestConfirmation,
    /// A call to a tool the attempt did not declare; it changes nothing and
    /// reports an error back to the model, as legacy did.
    UndeclaredTool {
        name: String,
    },
    /// A declared tool called without usable arguments; like legacy, the call
    /// is answered with an error and later calls still run.
    Rejected {
        tool: String,
        reason: CreationRejection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreationRejection {
    MissingArgument { argument: String },
    UnknownId { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationOperationError {
    UnknownTool,
    InvalidArguments,
    WrongTarget,
    InvalidText,
    DuplicateIdentity,
    NotFound,
    LimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationOperationOutcome {
    pub ordinal: u32,
    pub operation: CreationOperation,
    pub error: Option<CreationOperationError>,
}

impl CreationOperationOutcome {
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.error.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationProposal {
    pub id: CreationProposalId,
    pub turn_id: Option<CreationTurnId>,
    pub parent_id: Option<CreationProposalId>,
    pub ordinal: u32,
    pub stage: CreationStage,
    pub draft: CreationDraft,
    pub outcomes: Vec<CreationOperationOutcome>,
    pub created_at: TimestampMillis,
}

impl CreationProposal {
    pub fn initial(
        id: CreationProposalId,
        draft: CreationDraft,
        now: TimestampMillis,
    ) -> Result<Self, CreationProposalError> {
        draft.validate()?;
        let proposal = Self {
            id,
            turn_id: None,
            parent_id: None,
            ordinal: 0,
            stage: CreationStage::Drafting,
            draft,
            outcomes: Vec::new(),
            created_at: now,
        };
        proposal.validate()?;
        Ok(proposal)
    }

    pub fn apply(
        &self,
        id: CreationProposalId,
        turn_id: CreationTurnId,
        operations: Vec<CreationOperation>,
        now: TimestampMillis,
    ) -> Result<Self, CreationProposalError> {
        self.validate()?;
        if operations.is_empty() || operations.len() > MAX_CREATION_OPERATIONS {
            return Err(CreationProposalError::InvalidOperationCount);
        }
        let mut draft = self.draft.clone();
        let mut stage = self.stage;
        let mut outcomes = Vec::with_capacity(operations.len());
        for (index, operation) in operations.into_iter().enumerate() {
            let error = apply_one(&mut draft, &mut stage, &operation).err();
            outcomes.push(CreationOperationOutcome {
                ordinal: u32::try_from(index)
                    .map_err(|_| CreationProposalError::InvalidOperationCount)?,
                operation,
                error,
            });
        }
        draft.validate()?;
        let proposal = Self {
            id,
            turn_id: Some(turn_id),
            parent_id: Some(self.id),
            ordinal: self
                .ordinal
                .checked_add(1)
                .ok_or(CreationProposalError::RevisionOverflow)?,
            stage,
            draft,
            outcomes,
            created_at: now,
        };
        proposal.validate()?;
        Ok(proposal)
    }

    pub fn validate(&self) -> Result<(), CreationProposalError> {
        self.draft.validate()?;
        if self.ordinal == 0 {
            if self.turn_id.is_some() || self.parent_id.is_some() || !self.outcomes.is_empty() {
                return Err(CreationProposalError::InvalidLineage);
            }
        } else if self.turn_id.is_none() || self.parent_id.is_none() || self.outcomes.is_empty() {
            return Err(CreationProposalError::InvalidLineage);
        }
        for (index, outcome) in self.outcomes.iter().enumerate() {
            if outcome.ordinal
                != u32::try_from(index).map_err(|_| CreationProposalError::InvalidOperationCount)?
            {
                return Err(CreationProposalError::InvalidLineage);
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| CreationProposalError::DraftTooLarge)?
            .len()
            > MAX_CREATION_PROPOSAL_BYTES
        {
            return Err(CreationProposalError::DraftTooLarge);
        }
        Ok(())
    }
}

/// Legacy kept every tool available after a preview; here a successful change
/// also returns the proposal to drafting, so confirmation must be requested
/// again before the user can apply it.
fn apply_one(
    draft: &mut CreationDraft,
    stage: &mut CreationStage,
    operation: &CreationOperation,
) -> Result<(), CreationOperationError> {
    let is_mutation = !matches!(
        operation,
        CreationOperation::ShowPreview
            | CreationOperation::RequestConfirmation
            | CreationOperation::UndeclaredTool { .. }
            | CreationOperation::Rejected { .. }
    );
    apply_change(draft, stage, operation)?;
    if is_mutation {
        *stage = CreationStage::Drafting;
    }
    Ok(())
}

fn apply_change(
    draft: &mut CreationDraft,
    stage: &mut CreationStage,
    operation: &CreationOperation,
) -> Result<(), CreationOperationError> {
    match operation {
        CreationOperation::SetName { value } => {
            validate_text(value)?;
            match draft {
                CreationDraft::Character { name, .. }
                | CreationDraft::Persona { name, .. }
                | CreationDraft::Lorebook { name, .. } => *name = Some(value.clone()),
            }
        }
        CreationOperation::SetDescription { value } => {
            validate_text(value)?;
            match draft {
                CreationDraft::Character { definition, .. } => *definition = Some(value.clone()),
                CreationDraft::Persona { description, .. }
                | CreationDraft::Lorebook { description, .. } => *description = Some(value.clone()),
            }
        }
        CreationOperation::AddScene {
            id,
            content,
            direction,
        } => {
            validate_text(content)?;
            validate_optional_text(direction)?;
            let CreationDraft::Character { scenes, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            if scenes.len() >= crate::model::MAX_CREATION_SCENES {
                return Err(CreationOperationError::LimitExceeded);
            }
            if scenes.iter().any(|scene| scene.id == *id) {
                return Err(CreationOperationError::DuplicateIdentity);
            }
            scenes.push(CreationScene {
                id: *id,
                content: content.clone(),
                direction: direction.clone(),
            });
        }
        CreationOperation::UpdateScene {
            id,
            content,
            direction,
        } => {
            validate_text(content)?;
            validate_optional_text(direction)?;
            let CreationDraft::Character { scenes, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            let scene = scenes
                .iter_mut()
                .find(|scene| scene.id == *id)
                .ok_or(CreationOperationError::NotFound)?;
            scene.content.clone_from(content);
            if direction.is_some() {
                scene.direction.clone_from(direction);
            }
        }
        CreationOperation::UpsertLorebookEntry { id, title, content } => {
            validate_text(title)?;
            validate_text(content)?;
            let CreationDraft::Lorebook { entries, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            if let Some(entry) = entries.iter_mut().find(|entry| entry.id == *id) {
                entry.title.clone_from(title);
                entry.content.clone_from(content);
            } else {
                if entries.len() >= crate::model::MAX_CREATION_LOREBOOK_ENTRIES {
                    return Err(CreationOperationError::LimitExceeded);
                }
                entries.push(CreationLorebookEntry {
                    id: *id,
                    title: title.clone(),
                    content: content.clone(),
                });
            }
        }
        CreationOperation::DeleteScene { id } => {
            let CreationDraft::Character { scenes, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            let index = scenes
                .iter()
                .position(|scene| scene.id == *id)
                .ok_or(CreationOperationError::NotFound)?;
            scenes.remove(index);
        }
        CreationOperation::UpdateLorebookEntry { id, title, content } => {
            validate_text(title)?;
            validate_text(content)?;
            let CreationDraft::Lorebook { entries, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            let entry = entries
                .iter_mut()
                .find(|entry| entry.id == *id)
                .ok_or(CreationOperationError::NotFound)?;
            entry.title.clone_from(title);
            entry.content.clone_from(content);
        }
        CreationOperation::ReorderLorebookEntries { order } => {
            let CreationDraft::Lorebook { entries, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            let mut seen = std::collections::HashSet::new();
            if order.iter().any(|id| !seen.insert(*id)) {
                return Err(CreationOperationError::DuplicateIdentity);
            }
            if order
                .iter()
                .any(|id| !entries.iter().any(|entry| entry.id == *id))
            {
                return Err(CreationOperationError::NotFound);
            }
            let mut reordered = order
                .iter()
                .filter_map(|id| entries.iter().find(|entry| entry.id == *id).cloned())
                .collect::<Vec<_>>();
            reordered.extend(
                entries
                    .iter()
                    .filter(|entry| !seen.contains(&entry.id))
                    .cloned(),
            );
            *entries = reordered;
        }
        CreationOperation::DeleteLorebookEntry { id } => {
            let CreationDraft::Lorebook { entries, .. } = draft else {
                return Err(CreationOperationError::WrongTarget);
            };
            let index = entries
                .iter()
                .position(|entry| entry.id == *id)
                .ok_or(CreationOperationError::NotFound)?;
            entries.remove(index);
        }
        CreationOperation::ShowPreview => *stage = CreationStage::AwaitingReview,
        CreationOperation::RequestConfirmation => *stage = CreationStage::AwaitingConfirmation,
        CreationOperation::UndeclaredTool { .. } => {
            return Err(CreationOperationError::UnknownTool);
        }
        CreationOperation::Rejected { reason, .. } => {
            return Err(match reason {
                CreationRejection::MissingArgument { .. } => {
                    CreationOperationError::InvalidArguments
                }
                CreationRejection::UnknownId { .. } => CreationOperationError::NotFound,
            });
        }
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), CreationOperationError> {
    crate::model::validate_required(value).map_err(|_| CreationOperationError::InvalidText)
}

fn validate_optional_text(value: &Option<String>) -> Result<(), CreationOperationError> {
    crate::model::validate_optional(value).map_err(|_| CreationOperationError::InvalidText)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationProposalError {
    #[error("creation target is invalid")]
    InvalidTarget,
    #[error("creation draft is too large")]
    DraftTooLarge,
    #[error("creation text is blank")]
    BlankText,
    #[error("creation draft contains duplicate identity")]
    DuplicateDraftIdentity,
    #[error("creation proposal lineage is invalid")]
    InvalidLineage,
    #[error("creation operation count is invalid")]
    InvalidOperationCount,
    #[error("creation revision overflow")]
    RevisionOverflow,
}

#[cfg(test)]
mod tests {
    use lettuce_types::{
        CreationProposalId, CreationTurnId, LorebookEntryId, SceneId, TimestampMillis,
    };

    use crate::{
        CreationDraft, CreationOperation, CreationOperationError, CreationProposal, CreationStage,
    };

    #[test]
    fn operations_are_ordered_and_one_failure_does_not_stop_later_calls() {
        let base = CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Character {
                name: None,
                definition: None,
                scenes: Vec::new(),
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let missing = SceneId::new();
        let added = SceneId::new();
        let result = base
            .apply(
                CreationProposalId::new(),
                CreationTurnId::new(),
                vec![
                    CreationOperation::UpdateScene {
                        id: missing,
                        content: "missing".to_owned(),
                        direction: None,
                    },
                    CreationOperation::AddScene {
                        id: added,
                        content: "hello".to_owned(),
                        direction: Some("softly".to_owned()),
                    },
                    CreationOperation::SetName {
                        value: "Aster".to_owned(),
                    },
                ],
                TimestampMillis::new(2),
            )
            .expect("proposal");

        assert_eq!(
            result.outcomes[0].error,
            Some(CreationOperationError::NotFound)
        );
        assert!(result.outcomes[1].succeeded());
        assert!(result.outcomes[2].succeeded());
        assert_eq!(
            result.draft,
            CreationDraft::Character {
                name: Some("Aster".to_owned()),
                definition: None,
                scenes: vec![crate::CreationScene {
                    id: added,
                    content: "hello".to_owned(),
                    direction: Some("softly".to_owned()),
                }],
            }
        );
    }

    #[test]
    fn a_change_after_preview_or_confirmation_returns_to_drafting() {
        let base = CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Persona {
                name: None,
                description: None,
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let confirmation = base
            .apply(
                CreationProposalId::new(),
                CreationTurnId::new(),
                vec![
                    CreationOperation::ShowPreview,
                    CreationOperation::RequestConfirmation,
                ],
                TimestampMillis::new(2),
            )
            .expect("preview then confirmation");
        assert_eq!(confirmation.stage, CreationStage::AwaitingConfirmation);
        assert!(
            confirmation
                .outcomes
                .iter()
                .all(|outcome| outcome.succeeded())
        );

        let edited = confirmation
            .apply(
                CreationProposalId::new(),
                CreationTurnId::new(),
                vec![
                    CreationOperation::SetName {
                        value: "Navigator".to_owned(),
                    },
                    CreationOperation::UndeclaredTool {
                        name: "generate_image".to_owned(),
                    },
                ],
                TimestampMillis::new(3),
            )
            .expect("edit after confirmation");
        assert_eq!(edited.stage, CreationStage::Drafting);
        assert_eq!(
            edited.outcomes[1].error,
            Some(CreationOperationError::UnknownTool)
        );

        let failed = confirmation
            .apply(
                CreationProposalId::new(),
                CreationTurnId::new(),
                vec![CreationOperation::SetName {
                    value: "  ".to_owned(),
                }],
                TimestampMillis::new(3),
            )
            .expect("failed edit");
        assert_eq!(failed.stage, CreationStage::AwaitingConfirmation);
    }

    #[test]
    fn target_specific_operations_fail_without_mutating_other_drafts() {
        let base = CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Lorebook {
                name: None,
                description: None,
                entries: Vec::new(),
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let entry_id = LorebookEntryId::new();
        let result = base
            .apply(
                CreationProposalId::new(),
                CreationTurnId::new(),
                vec![
                    CreationOperation::AddScene {
                        id: SceneId::new(),
                        content: "wrong".to_owned(),
                        direction: None,
                    },
                    CreationOperation::UpsertLorebookEntry {
                        id: entry_id,
                        title: "Gate".to_owned(),
                        content: "The northern gate.".to_owned(),
                    },
                ],
                TimestampMillis::new(2),
            )
            .expect("proposal");
        assert_eq!(
            result.outcomes[0].error,
            Some(CreationOperationError::WrongTarget)
        );
        let CreationDraft::Lorebook { entries, .. } = result.draft else {
            panic!("lorebook draft");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, entry_id);
    }
}
