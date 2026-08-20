use std::fmt;

use serde::{Deserialize, Serialize};

pub(crate) const MAX_DISPLAY_CHARS: usize = 256;
pub(crate) const MAX_AUTHORED_TEXT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_REASONING_BYTES: usize = 256 * 1024;
pub(crate) const MAX_ANNOTATION_BYTES: usize = 64 * 1024;
pub(crate) const MAX_PARTS: usize = 256;
pub(crate) const MAX_PARTICIPANTS: usize = 256;
pub(crate) const MAX_BRANCHES: usize = 10_000;
pub(crate) const MAX_ATTEMPTS: usize = 128;
pub(crate) const MAX_LOREBOOKS: usize = 128;
pub(crate) const MAX_MEMORY_REVISIONS: usize = 256;
pub(crate) const MAX_DOCUMENT_COLLECTION: usize = 10_000;
pub(crate) const MAX_DOCUMENT_ENTRIES: usize = 512;
pub(crate) const MAX_ENTRY_KEYWORDS: usize = 128;
pub(crate) const MAX_CONDITION_DEPTH: usize = 16;
pub(crate) const MAX_CONDITION_NODES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BoundedText(String);

impl BoundedText {
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, TextError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TextError::Blank);
        }
        if value.len() > max_bytes {
            return Err(TextError::TooLarge);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BoundedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl TryFrom<String> for BoundedText {
    type Error = TextError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value, MAX_AUTHORED_TEXT_BYTES)
    }
}

impl From<BoundedText> for String {
    fn from(value: BoundedText) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    #[error("text must not be blank")]
    Blank,
    #[error("text exceeds its bound")]
    TooLarge,
}

pub(crate) fn validate_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
    allow_blank: bool,
) -> Result<(), crate::ValidationError> {
    if !allow_blank && value.trim().is_empty() {
        return Err(crate::ValidationError::Blank { field });
    }
    if value.len() > max_bytes {
        return Err(crate::ValidationError::TooLarge { field });
    }
    Ok(())
}

pub(crate) fn validate_collection<T>(
    field: &'static str,
    values: &[T],
    max: usize,
) -> Result<(), crate::ValidationError> {
    if values.len() > max {
        return Err(crate::ValidationError::TooMany { field, max });
    }
    Ok(())
}

pub(crate) fn validate_revision_timestamps(
    field: &'static str,
    revision: lettuce_types::Revision,
    created_at: lettuce_types::TimestampMillis,
    updated_at: lettuce_types::TimestampMillis,
) -> Result<(), crate::ValidationError> {
    if revision.get() == 0 {
        return Err(crate::ValidationError::ZeroRevision);
    }
    if created_at > updated_at {
        return Err(crate::ValidationError::InvalidTimestampOrder { field });
    }
    Ok(())
}

pub(crate) fn validate_unique<T: Eq + std::hash::Hash>(
    field: &'static str,
    values: impl IntoIterator<Item = T>,
) -> Result<(), crate::ValidationError> {
    let mut seen = std::collections::HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(crate::ValidationError::Duplicate { field });
        }
    }
    Ok(())
}
