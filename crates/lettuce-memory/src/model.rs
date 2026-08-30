use std::collections::HashSet;

use lettuce_types::{MemoryId, MemorySpaceId, MessageId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const MAX_MEMORY_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_MEMORY_ITEMS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Score(u16);

impl Score {
    pub const ZERO: Self = Self(0);
    pub const FULL: Self = Self(10_000);
    pub const LEGACY_VOLATILITY: Self = Self(4_000);
    pub const HARD_DELETE_THRESHOLD: Self = Self(7_000);

    pub fn from_ratio(value: f64) -> Result<Self, MemoryValidationError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(MemoryValidationError::InvalidScore);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(Self((value * 10_000.0).round() as u16))
    }

    #[must_use]
    pub const fn from_basis_points(value: u16) -> Option<Self> {
        if value <= 10_000 {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }

    #[must_use]
    pub fn ratio(self) -> f64 {
        f64::from(self.0) / 10_000.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    CharacterTrait,
    Relationship,
    PlotEvent,
    WorldDetail,
    Preference,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryItem {
    pub id: MemoryId,
    pub text: String,
    pub category: MemoryCategory,
    pub source_message_id: Option<MessageId>,
    pub token_count: u32,
    pub is_cold: bool,
    pub is_pinned: bool,
    pub importance: Score,
    pub persistence_importance: Score,
    pub prompt_importance: Score,
    pub volatility: Score,
    pub access_count: u32,
    pub created_at: TimestampMillis,
    pub last_accessed_at: TimestampMillis,
}

impl MemoryItem {
    pub(crate) fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_memory_text(&self.text)?;
        if self.is_pinned && self.is_cold {
            return Err(MemoryValidationError::PinnedCold);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemorySpaceSnapshot {
    pub id: MemorySpaceId,
    pub revision: Revision,
    pub items: Vec<MemoryItem>,
}

impl MemorySpaceSnapshot {
    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        if self.revision.get() == 0 {
            return Err(MemoryValidationError::InvalidRevision);
        }
        if self.items.len() > MAX_MEMORY_ITEMS {
            return Err(MemoryValidationError::TooManyItems);
        }
        let mut ids = HashSet::with_capacity(self.items.len());
        for item in &self.items {
            item.validate()?;
            if !ids.insert(item.id) {
                return Err(MemoryValidationError::DuplicateItemId);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryPolicy {
    pub max_entries: usize,
    pub hot_token_budget: u32,
    pub cold_threshold: Score,
    pub delete_confidence_default: Score,
    pub max_hard_delete_ratio_per_cycle: Score,
}

impl MemoryPolicy {
    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        if self.max_entries == 0 || self.max_entries > MAX_MEMORY_ITEMS {
            return Err(MemoryValidationError::InvalidMaxEntries);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryValidationError {
    #[error("memory text is empty")]
    EmptyText,
    #[error("memory text is too large")]
    TextTooLarge,
    #[error("memory score is outside zero through one")]
    InvalidScore,
    #[error("pinned memory cannot be cold")]
    PinnedCold,
    #[error("memory space contains duplicate item ids")]
    DuplicateItemId,
    #[error("memory space contains too many items")]
    TooManyItems,
    #[error("memory space revision must be positive")]
    InvalidRevision,
    #[error("memory space identity does not match")]
    InvalidSpaceId,
    #[error("memory policy max entries is invalid")]
    InvalidMaxEntries,
    #[error("new memory space must start at revision one")]
    InvalidInitialRevision,
}

pub(crate) fn validate_memory_text(value: &str) -> Result<&str, MemoryValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MemoryValidationError::EmptyText);
    }
    if trimmed.len() > MAX_MEMORY_TEXT_BYTES {
        return Err(MemoryValidationError::TextTooLarge);
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use lettuce_types::{MemorySpaceId, Revision};

    use super::{MemorySpaceSnapshot, MemoryValidationError, Score};

    #[test]
    fn score_conversion_is_bounded() {
        let score = match Score::from_ratio(0.7) {
            Ok(score) => score,
            Err(error) => panic!("score conversion failed: {error}"),
        };
        assert_eq!(score.basis_points(), 7_000);
        assert_eq!(
            Score::from_ratio(1.1),
            Err(MemoryValidationError::InvalidScore)
        );
    }

    #[test]
    fn memory_space_revision_must_be_positive() {
        let snapshot = MemorySpaceSnapshot {
            id: MemorySpaceId::new(),
            revision: Revision::new(0),
            items: vec![],
        };
        assert_eq!(
            snapshot.validate(),
            Err(MemoryValidationError::InvalidRevision)
        );
    }
}
