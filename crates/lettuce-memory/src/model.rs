use std::collections::HashSet;

use lettuce_types::{
    MemoryId, MemoryRevisionId, MemorySpaceId, MessageId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

pub const MAX_MEMORY_TEXT_BYTES: usize = 8 * 1024 * 1024;
/// The largest `max_entries` a dynamic memory policy may keep; a memory space
/// itself holds any number of items.
pub const MAX_MEMORY_ITEMS: usize = 4096;
pub const MAX_MEMORY_SUMMARY_BYTES: usize = 8 * 1024 * 1024;

#[must_use]
pub fn memory_revision_id(space_id: MemorySpaceId, revision: Revision) -> MemoryRevisionId {
    let name = format!("lettuce-memory:{space_id}:{}", revision.get());
    MemoryRevisionId::from_uuid(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        name.as_bytes(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicMemoryRunMode {
    Auto,
    AskFirst,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryPendingApproval {
    pub conversation_id: lettuce_types::ConversationId,
    pub branch_id: lettuce_types::ConversationBranchId,
    pub prompted_message_count: u64,
    pub pending: bool,
    pub skipped: bool,
    pub updated_at: TimestampMillis,
}

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

/// The six-digit memory id a model sees and quotes back. It is unique within
/// a memory space and never changes after creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct MemoryShortId(u32);

impl MemoryShortId {
    pub const SPACE: u32 = 1_000_000;

    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value < Self::SPACE {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The preferred short id of a memory, before collision probing.
    #[must_use]
    pub fn derived(id: MemoryId) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        Self((id.as_uuid().as_u128() % u128::from(Self::SPACE)) as u32)
    }

    /// The derived short id, moved upward past ids already in use.
    #[must_use]
    pub fn allocate(id: MemoryId, in_use: impl Fn(Self) -> bool) -> Self {
        let mut candidate = Self::derived(id);
        while in_use(candidate) {
            candidate = Self((candidate.0 + 1) % Self::SPACE);
        }
        candidate
    }

    /// Exactly six ASCII digits.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| value.parse().ok())
            .flatten()
            .and_then(Self::new)
    }
}

impl TryFrom<u32> for MemoryShortId {
    type Error = MemoryValidationError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(MemoryValidationError::InvalidShortId)
    }
}

impl From<MemoryShortId> for u32 {
    fn from(value: MemoryShortId) -> Self {
        value.0
    }
}

impl std::fmt::Display for MemoryShortId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:06}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    CharacterTrait,
    Relationship,
    PlotEvent,
    WorldDetail,
    Preference,
    Other,
    Milestone,
    Boundary,
    Profile,
    Routine,
    Episodic,
    EmotionalSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOrigin {
    User,
    Model,
    Import,
}

impl MemoryOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Model => "model",
            Self::Import => "import",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "model" => Some(Self::Model),
            "import" => Some(Self::Import),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryItem {
    pub id: MemoryId,
    pub short_id: MemoryShortId,
    pub text: String,
    pub category: Option<MemoryCategory>,
    pub origin: MemoryOrigin,
    pub source_message_id: Option<MessageId>,
    pub source_role: Option<lettuce_conversations::MessageRole>,
    pub observed_at: Option<TimestampMillis>,
    pub observed_time_precision: Option<String>,
    pub superseded_by: Option<MemoryId>,
    pub superseded_at: Option<TimestampMillis>,
    pub supersedes: Vec<MemoryId>,
    pub token_count: Option<u32>,
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
    /// A memory a person wrote: full importance, default volatility, no
    /// source turn, and no token count until one is measured.
    #[must_use]
    pub fn written(
        id: MemoryId,
        short_id: MemoryShortId,
        text: String,
        at: TimestampMillis,
    ) -> Self {
        Self {
            id,
            short_id,
            text,
            category: None,
            origin: MemoryOrigin::User,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: None,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: at,
            last_accessed_at: at,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_memory_text(&self.text)?;
        if self.is_pinned && self.is_cold {
            return Err(MemoryValidationError::PinnedCold);
        }
        let valid_observation = match self.observed_time_precision.as_deref() {
            None => self.observed_at.is_none(),
            Some("user") => self.observed_at.is_some(),
            Some("turn") => {
                self.observed_at.is_some()
                    && self.source_message_id.is_some()
                    && self.source_role.is_some()
            }
            Some(_) => false,
        };
        if !valid_observation
            || (self.source_role.is_some() && self.source_message_id.is_none())
            || self.source_role.is_some_and(|role| {
                !matches!(
                    role,
                    lettuce_conversations::MessageRole::User
                        | lettuce_conversations::MessageRole::Assistant
                )
            })
            || self.superseded_by.is_some() != self.superseded_at.is_some()
            || self.superseded_by == Some(self.id)
            || self.supersedes.contains(&self.id)
        {
            return Err(MemoryValidationError::InvalidTemporalAttribution);
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
        let mut ids = HashSet::with_capacity(self.items.len());
        let mut short_ids = HashSet::with_capacity(self.items.len());
        for item in &self.items {
            item.validate()?;
            if !ids.insert(item.id) {
                return Err(MemoryValidationError::DuplicateItemId);
            }
            if !short_ids.insert(item.short_id) {
                return Err(MemoryValidationError::DuplicateShortId);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemorySummary {
    pub origin: MemoryOrigin,
    pub space_id: MemorySpaceId,
    pub branch_id: lettuce_types::ConversationBranchId,
    pub text: String,
    pub token_count: Option<u32>,
    pub window_start: u64,
    pub window_end: u64,
    pub source_message_ids: Vec<MessageId>,
    pub updated_at: TimestampMillis,
}

impl MemorySummary {
    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        let text = self.text.trim();
        if text.is_empty() {
            return Err(MemoryValidationError::EmptySummary);
        }
        if text.len() > MAX_MEMORY_SUMMARY_BYTES {
            return Err(MemoryValidationError::SummaryTooLarge);
        }
        if self.origin == MemoryOrigin::User {
            if self.window_start != 0 || self.window_end != 0 || !self.source_message_ids.is_empty()
            {
                return Err(MemoryValidationError::InvalidSummaryWindow);
            }
            return Ok(());
        }
        if self.source_message_ids.is_empty()
            || self.window_end <= self.window_start
            || self.window_end - self.window_start
                != u64::try_from(self.source_message_ids.len()).unwrap_or(u64::MAX)
        {
            return Err(MemoryValidationError::InvalidSummaryWindow);
        }
        let mut ids = HashSet::with_capacity(self.source_message_ids.len());
        if self.source_message_ids.iter().any(|id| !ids.insert(*id)) {
            return Err(MemoryValidationError::DuplicateSummarySourceMessage);
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
    pub decay_rate: Score,
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
    #[error("memory temporal attribution is inconsistent")]
    InvalidTemporalAttribution,
    #[error("memory space contains duplicate item ids")]
    DuplicateItemId,
    #[error("memory space contains duplicate short ids")]
    DuplicateShortId,
    #[error("memory short id is outside six digits")]
    InvalidShortId,
    #[error("memory space revision must be positive")]
    InvalidRevision,
    #[error("memory summary is empty")]
    EmptySummary,
    #[error("memory summary is too large")]
    SummaryTooLarge,
    #[error("memory summary window is invalid")]
    InvalidSummaryWindow,
    #[error("memory summary contains a duplicate source message")]
    DuplicateSummarySourceMessage,
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
    use lettuce_types::{MemorySpaceId, MessageId, Revision, TimestampMillis};

    use super::{
        MemorySpaceSnapshot, MemorySummary, MemoryValidationError, Score, memory_revision_id,
    };

    #[test]
    fn memory_revision_identity_is_stable_and_revision_specific() {
        let space_id = MemorySpaceId::new();
        let first = memory_revision_id(space_id, Revision::INITIAL);
        assert_eq!(first, memory_revision_id(space_id, Revision::INITIAL));
        assert_ne!(first, memory_revision_id(space_id, Revision::new(2)));
        assert_ne!(
            first,
            memory_revision_id(MemorySpaceId::new(), Revision::INITIAL)
        );
    }

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

    #[test]
    fn user_observed_time_without_source_message_round_trips() {
        let id = lettuce_types::MemoryId::new();
        let mut item = super::MemoryItem::written(
            id,
            super::MemoryShortId::derived(id),
            "An anniversary".into(),
            TimestampMillis::new(10),
        );
        item.observed_at = Some(TimestampMillis::new(5));
        item.observed_time_precision = Some("user".into());
        assert_eq!(item.validate(), Ok(()));
        let encoded = serde_json::to_vec(&item).expect("encode memory");
        let decoded: super::MemoryItem = serde_json::from_slice(&encoded).expect("decode memory");
        assert_eq!(decoded, item);
        assert_eq!(decoded.validate(), Ok(()));
    }

    #[test]
    fn memory_origins_round_trip_and_user_summaries_need_no_source_window() {
        use super::{MemoryItem, MemoryShortId};
        use lettuce_types::MemoryId;
        let id = MemoryId::new();
        let item = MemoryItem::written(
            id,
            MemoryShortId::derived(id),
            "A written memory".into(),
            TimestampMillis::new(10),
        );
        for origin in ["user", "model", "import"] {
            let mut encoded = serde_json::to_value(&item).expect("encode memory");
            encoded["origin"] = serde_json::json!(origin);
            let decoded: MemoryItem = serde_json::from_value(encoded).expect("decode origin");
            assert_eq!(
                serde_json::to_value(decoded).expect("encode origin")["origin"],
                origin
            );
        }
        let summary: MemorySummary = serde_json::from_value(serde_json::json!({
            "space_id": MemorySpaceId::new(),
            "branch_id": lettuce_types::ConversationBranchId::new(),
            "origin": "user",
            "text": "An authored summary",
            "token_count": 4,
            "window_start": 0,
            "window_end": 0,
            "source_message_ids": [],
            "updated_at": 10
        }))
        .expect("decode user summary");
        assert_eq!(summary.validate(), Ok(()));
    }

    #[test]
    fn unknown_item_and_user_summary_counts_round_trip_as_null() {
        let id = lettuce_types::MemoryId::new();
        let item = super::MemoryItem::written(
            id,
            super::MemoryShortId::derived(id),
            "A manual memory".into(),
            TimestampMillis::new(10),
        );
        let mut encoded = serde_json::to_value(item).expect("encode item");
        encoded["token_count"] = serde_json::Value::Null;
        let item: super::MemoryItem = serde_json::from_value(encoded).expect("unknown item count");
        assert_eq!(item.validate(), Ok(()));
        assert!(serde_json::to_value(item).expect("item")["token_count"].is_null());
        let summary: MemorySummary = serde_json::from_value(serde_json::json!({
            "space_id": MemorySpaceId::new(),
            "branch_id": lettuce_types::ConversationBranchId::new(),
            "origin": "user",
            "text": "An authored summary",
            "token_count": null,
            "window_start": 0,
            "window_end": 0,
            "source_message_ids": [],
            "updated_at": 10
        }))
        .expect("unknown user summary count");
        assert_eq!(summary.validate(), Ok(()));
        assert!(serde_json::to_value(summary).expect("summary")["token_count"].is_null());
    }

    #[test]
    fn companion_and_null_categories_round_trip_without_mapping_to_other() {
        let id = lettuce_types::MemoryId::new();
        let item = super::MemoryItem::written(
            id,
            super::MemoryShortId::derived(id),
            "A companion memory".into(),
            TimestampMillis::new(10),
        );
        for category in [
            serde_json::json!("milestone"),
            serde_json::json!("boundary"),
            serde_json::json!("profile"),
            serde_json::json!("routine"),
            serde_json::json!("episodic"),
            serde_json::json!("emotional_snapshot"),
            serde_json::Value::Null,
        ] {
            let mut encoded = serde_json::to_value(&item).expect("encode memory");
            encoded["category"] = category.clone();
            let decoded: super::MemoryItem =
                serde_json::from_value(encoded).expect("decode category");
            assert_eq!(decoded.validate(), Ok(()));
            assert_eq!(
                serde_json::to_value(decoded).expect("reencode memory")["category"],
                category
            );
        }
    }

    #[test]
    fn user_observed_time_preserves_existing_source_attribution() {
        let id = lettuce_types::MemoryId::new();
        let mut item = super::MemoryItem::written(
            id,
            super::MemoryShortId::derived(id),
            "An anniversary".into(),
            TimestampMillis::new(10),
        );
        item.source_message_id = Some(MessageId::new());
        item.source_role = Some(lettuce_conversations::MessageRole::Assistant);
        item.observed_at = Some(TimestampMillis::new(5));
        item.observed_time_precision = Some("user".into());
        assert_eq!(item.validate(), Ok(()));
        item.observed_at = None;
        item.observed_time_precision = None;
        assert_eq!(item.validate(), Ok(()));
    }

    #[test]
    fn summary_cursor_must_exactly_cover_its_window() {
        let summary = MemorySummary {
            origin: crate::MemoryOrigin::Model,
            branch_id: lettuce_types::ConversationBranchId::new(),
            space_id: MemorySpaceId::new(),
            text: "summary".to_owned(),
            token_count: Some(1),
            window_start: 4,
            window_end: 7,
            source_message_ids: vec![MessageId::new(), MessageId::new()],
            updated_at: TimestampMillis::new(1),
        };
        assert_eq!(
            summary.validate(),
            Err(MemoryValidationError::InvalidSummaryWindow)
        );
    }
}
