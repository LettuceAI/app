use lettuce_types::{
    ConversationBranchId, ConversationId, MemoryId, OperationId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::{
    MemoryCategory, MemoryItem, MemoryRepositoryError, MemorySpaceSnapshot, MemorySummary,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum MemoryFieldChange<T> {
    Keep,
    Set(T),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryManualMutation {
    Add {
        item: MemoryItem,
    },
    Update {
        memory_id: MemoryId,
        text: Option<String>,
        category: MemoryFieldChange<Option<MemoryCategory>>,
        observed_at: MemoryFieldChange<Option<TimestampMillis>>,
        token_count: Option<u32>,
    },
    Delete {
        memory_id: MemoryId,
    },
    Pin {
        memory_id: MemoryId,
        pinned: bool,
    },
    Temperature {
        memory_id: MemoryId,
        cold: bool,
    },
    Summary {
        summary: Option<MemorySummary>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryManualEdit {
    pub id: OperationId,
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub conversation_revision: Revision,
    pub expected_revision: Revision,
    pub space_id: lettuce_types::MemorySpaceId,
    pub context_revisions: Vec<MemoryContextRevision>,
    pub mutation: MemoryManualMutation,
    pub at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryContextRevision {
    Character {
        id: lettuce_types::CharacterId,
        revision: Revision,
    },
    Group {
        id: lettuce_types::GroupId,
        revision: Revision,
    },
    Settings {
        revision: Revision,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryManualHistory {
    pub sequence: u64,
    pub edit: MemoryManualEdit,
    pub space_id: lettuce_types::MemorySpaceId,
    pub anchor_message_id: Option<lettuce_types::MessageId>,
    pub message_position: u64,
    pub before_item: Option<MemoryItem>,
    pub after_item: Option<MemoryItem>,
    pub before_summary: Option<MemorySummary>,
    pub after_summary: Option<MemorySummary>,
    pub resulting_revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryManualEditRecord {
    pub history: MemoryManualHistory,
    pub undone_at: Option<TimestampMillis>,
}

impl MemoryManualHistory {
    pub fn validate(&self) -> Result<(), MemoryRepositoryError> {
        if self.sequence == 0
            || self.space_id != self.edit.space_id
            || self
                .edit
                .expected_revision
                .next()
                .map_err(|_| MemoryRepositoryError::Conflict)?
                != self.resulting_revision
        {
            return Err(MemoryRepositoryError::Conflict);
        }
        for item in self.before_item.iter().chain(self.after_item.iter()) {
            item.validate()?;
        }
        for summary in self.before_summary.iter().chain(self.after_summary.iter()) {
            summary.validate()?;
            if summary.space_id != self.space_id {
                return Err(MemoryRepositoryError::Conflict);
            }
        }
        let valid = match &self.edit.mutation {
            MemoryManualMutation::Add { item } => {
                self.before_item.is_none() && self.after_item.as_ref() == Some(item)
            }
            MemoryManualMutation::Delete { memory_id } => {
                self.before_item
                    .as_ref()
                    .is_some_and(|item| item.id == *memory_id)
                    && self.after_item.is_none()
            }
            MemoryManualMutation::Summary { summary } => {
                self.before_item.is_none()
                    && self.after_item.is_none()
                    && self.after_summary == *summary
            }
            MemoryManualMutation::Update { memory_id, .. }
            | MemoryManualMutation::Pin { memory_id, .. }
            | MemoryManualMutation::Temperature { memory_id, .. } => {
                self.before_item
                    .as_ref()
                    .is_some_and(|item| item.id == *memory_id)
                    && self
                        .after_item
                        .as_ref()
                        .is_some_and(|item| item.id == *memory_id)
            }
        };
        if !valid {
            return Err(MemoryRepositoryError::Conflict);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryManualReduction {
    pub items: Vec<MemoryItem>,
    pub before_item: Option<MemoryItem>,
    pub after_item: Option<MemoryItem>,
    pub summary: Option<Option<MemorySummary>>,
}

pub fn reduce_manual_memory(
    snapshot: &MemorySpaceSnapshot,
    mutation: &MemoryManualMutation,
    at: TimestampMillis,
) -> Result<MemoryManualReduction, MemoryRepositoryError> {
    snapshot.validate()?;
    let mut items = snapshot.items.clone();
    let target = match mutation {
        MemoryManualMutation::Add { .. } | MemoryManualMutation::Summary { .. } => None,
        MemoryManualMutation::Update { memory_id, .. }
        | MemoryManualMutation::Delete { memory_id }
        | MemoryManualMutation::Pin { memory_id, .. }
        | MemoryManualMutation::Temperature { memory_id, .. } => Some(*memory_id),
    };
    let index = target
        .map(|id| {
            items
                .iter()
                .position(|item| item.id == id)
                .ok_or(MemoryRepositoryError::NotFound)
        })
        .transpose()?;
    let before_item = index.map(|index| items[index].clone());
    let mut after_item = None;
    let mut summary = None;
    match mutation {
        MemoryManualMutation::Add { item } => {
            item.validate()?;
            if items.iter().any(|existing| existing.id == item.id) {
                return Err(MemoryRepositoryError::AlreadyExists);
            }
            items.push(item.clone());
            after_item = Some(item.clone());
        }
        MemoryManualMutation::Delete { .. } => {
            items.remove(index.ok_or(MemoryRepositoryError::NotFound)?);
        }
        MemoryManualMutation::Summary { summary: value } => {
            if let Some(value) = value {
                value.validate()?;
            }
            summary = Some(value.clone());
        }
        change => {
            let item = &mut items[index.ok_or(MemoryRepositoryError::NotFound)?];
            match change {
                MemoryManualMutation::Update {
                    text,
                    category,
                    observed_at,
                    token_count,
                    ..
                } => {
                    if let Some(text) = text {
                        item.text.clone_from(text);
                        item.token_count = *token_count;
                    }
                    if let MemoryFieldChange::Set(category) = category {
                        item.category = *category;
                    }
                    if let MemoryFieldChange::Set(observed_at) = observed_at {
                        item.observed_at = *observed_at;
                        item.observed_time_precision = observed_at.map(|_| "user".into());
                    }
                }
                MemoryManualMutation::Pin { pinned, .. } => {
                    item.is_pinned = *pinned;
                    if *pinned {
                        item.is_cold = false;
                        item.importance = crate::Score::FULL;
                        item.last_accessed_at = at;
                    }
                }
                MemoryManualMutation::Temperature { cold, .. } => {
                    if *cold && item.is_pinned {
                        return Err(crate::MemoryValidationError::PinnedCold.into());
                    }
                    item.is_cold = *cold;
                    item.importance = if *cold {
                        crate::Score::ZERO
                    } else {
                        crate::Score::FULL
                    };
                    if !*cold {
                        item.last_accessed_at = at;
                    }
                }
                _ => unreachable!(),
            }
            item.validate()?;
            after_item = Some(item.clone());
        }
    }
    MemorySpaceSnapshot {
        items: items.clone(),
        ..snapshot.clone()
    }
    .validate()?;
    Ok(MemoryManualReduction {
        items,
        before_item,
        after_item,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryShortId, Score};

    #[test]
    fn manual_pin_and_temperature_preserve_setter_rules_and_unknown_counts() {
        let id = MemoryId::new();
        let mut item = MemoryItem::written(
            id,
            MemoryShortId::derived(id),
            "A manual memory".into(),
            TimestampMillis::new(1),
        );
        item.is_cold = true;
        item.importance = Score::ZERO;
        let snapshot = MemorySpaceSnapshot {
            id: lettuce_types::MemorySpaceId::new(),
            revision: Revision::INITIAL,
            items: vec![item],
        };
        let pinned = reduce_manual_memory(
            &snapshot,
            &MemoryManualMutation::Pin {
                memory_id: id,
                pinned: true,
            },
            TimestampMillis::new(5),
        )
        .expect("pin");
        assert!(pinned.items[0].is_pinned);
        assert!(!pinned.items[0].is_cold);
        assert_eq!(pinned.items[0].importance, Score::FULL);
        assert_eq!(pinned.items[0].last_accessed_at, TimestampMillis::new(5));
        assert_eq!(pinned.items[0].token_count, None);
        let pinned = MemorySpaceSnapshot {
            items: pinned.items,
            ..snapshot
        };
        assert!(
            reduce_manual_memory(
                &pinned,
                &MemoryManualMutation::Temperature {
                    memory_id: id,
                    cold: true
                },
                TimestampMillis::new(6)
            )
            .is_err()
        );
    }

    #[test]
    fn manual_update_keeps_origin_and_source_and_clears_user_date() {
        let id = MemoryId::new();
        let mut item = MemoryItem::written(
            id,
            MemoryShortId::derived(id),
            "Original memory".into(),
            TimestampMillis::new(1),
        );
        item.origin = crate::MemoryOrigin::Model;
        item.source_message_id = Some(lettuce_types::MessageId::new());
        item.source_role = Some(lettuce_conversations::MessageRole::User);
        item.observed_at = Some(TimestampMillis::new(2));
        item.observed_time_precision = Some("turn".into());
        let snapshot = MemorySpaceSnapshot {
            id: lettuce_types::MemorySpaceId::new(),
            revision: Revision::INITIAL,
            items: vec![item.clone()],
        };
        let changed = reduce_manual_memory(
            &snapshot,
            &MemoryManualMutation::Update {
                memory_id: id,
                text: Some("Updated memory".into()),
                category: MemoryFieldChange::Set(Some(MemoryCategory::Boundary)),
                observed_at: MemoryFieldChange::Set(None),
                token_count: None,
            },
            TimestampMillis::new(3),
        )
        .expect("update");
        assert_eq!(changed.items[0].origin, item.origin);
        assert_eq!(changed.items[0].source_message_id, item.source_message_id);
        assert_eq!(changed.items[0].observed_at, None);
        assert_eq!(changed.items[0].observed_time_precision, None);
        assert_eq!(changed.items[0].category, Some(MemoryCategory::Boundary));
        assert_eq!(changed.before_item, Some(item));
    }
}
