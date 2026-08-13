use lettuce_types::{Revision, TimestampMillis};

use crate::ValidationError;

pub(crate) const MAX_NAME_SCALARS: usize = 256;
pub(crate) const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_TAGS_OR_SOURCES: usize = 256;
pub(crate) const MAX_COLLECTION_ITEMS: usize = 10_000;

pub(crate) fn validate_revision_timestamps(
    field: &'static str,
    revision: Revision,
    created_at: TimestampMillis,
    updated_at: TimestampMillis,
) -> Result<(), ValidationError> {
    if revision.get() == 0 {
        return Err(ValidationError::ZeroRevision);
    }
    if created_at > updated_at {
        return Err(ValidationError::InvalidTimestampOrder { field });
    }
    Ok(())
}

pub(crate) fn validate_non_blank(field: &'static str, value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::Blank { field });
    }
    Ok(())
}

pub(crate) fn validate_name(field: &'static str, value: &str) -> Result<(), ValidationError> {
    validate_non_blank(field, value)?;
    validate_scalar_limit(field, value, MAX_NAME_SCALARS)?;
    if value.len() > 1024 {
        return Err(ValidationError::TooLarge { field });
    }
    Ok(())
}

pub(crate) fn validate_text(field: &'static str, value: &str) -> Result<(), ValidationError> {
    if value.len() > MAX_TEXT_BYTES {
        return Err(ValidationError::TooLarge { field });
    }
    Ok(())
}

pub(crate) fn validate_scalar_limit(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), ValidationError> {
    if value.chars().count() > max {
        return Err(ValidationError::TooLong { field, max });
    }
    Ok(())
}

pub(crate) fn validate_collection<T>(
    field: &'static str,
    values: &[T],
    max: usize,
) -> Result<(), ValidationError> {
    if values.len() > max {
        return Err(ValidationError::TooMany { field, max });
    }
    Ok(())
}

pub(crate) fn validate_unique<T: Eq + std::hash::Hash>(
    field: &'static str,
    values: impl IntoIterator<Item = T>,
) -> Result<(), ValidationError> {
    let mut seen = std::collections::HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(ValidationError::Duplicate { field });
        }
    }
    Ok(())
}

pub(crate) fn validate_contiguous(
    field: &'static str,
    ordinals: impl IntoIterator<Item = u32>,
) -> Result<(), ValidationError> {
    let mut expected = 0;
    for ordinal in ordinals {
        if ordinal != expected {
            return Err(ValidationError::NonContiguous { field });
        }
        expected = expected.checked_add(1).ok_or(ValidationError::TooMany {
            field,
            max: MAX_COLLECTION_ITEMS,
        })?;
    }
    Ok(())
}

pub(crate) fn validate_color(field: &'static str, value: &str) -> Result<(), ValidationError> {
    let valid = value.len() == 4 || value.len() == 5 || value.len() == 7 || value.len() == 9;
    let valid = valid
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !valid {
        return Err(ValidationError::InvalidValue { field });
    }
    Ok(())
}

pub(crate) fn validate_optional_color(
    field: &'static str,
    value: Option<&String>,
) -> Result<(), ValidationError> {
    if let Some(value) = value {
        validate_color(field, value)?;
    }
    Ok(())
}

pub(crate) fn validate_finite(field: &'static str, value: f32) -> Result<(), ValidationError> {
    if !value.is_finite() {
        return Err(ValidationError::NonFinite { field });
    }
    Ok(())
}
