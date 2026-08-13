use std::{
    fmt,
    path::{Component, Path},
};

use crate::error::PlatformError;

pub(crate) const STAGE_PREFIX: &str = ".lettuce-stage-";
pub(crate) const TRASH_PREFIX: &str = ".lettuce-trash-";
pub(crate) const RECOVERY_PREFIX: &str = ".lettuce-recovery-";
pub(crate) const JOURNAL_PREFIX: &str = ".lettuce-journal-";
pub(crate) const MAX_KEY_SEGMENTS: usize = 64;
pub(crate) const MAX_SEGMENT_SCALARS: usize = 255;
pub(crate) const MAX_SEGMENT_BYTES: usize = 255;
pub(crate) const MAX_KEY_BYTES: usize = 4096;

/// Typed, relative object identifier. Segments are always explicit normal
/// Unicode strings; no `FromStr`/path parser exists by design.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    pub(crate) segments: Vec<String>,
}

impl ObjectKey {
    /// Build a key from explicit normal segments.
    ///
    /// Unicode is accepted as-is (normalization is not silently applied), but
    /// controls, separators, dot segments, drive-like prefixes and empty
    /// segments are rejected. This makes the key portable without changing a
    /// caller's display text.
    pub fn from_segments<I, S>(segments: I) -> Result<Self, PlatformError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut checked = Vec::new();
        let mut total_bytes = 0usize;
        for segment in segments {
            let value = segment.as_ref();
            if checked.len() >= MAX_KEY_SEGMENTS {
                return Err(PlatformError::InvalidKey);
            }
            validate_segment(value)?;
            total_bytes = total_bytes
                .checked_add(value.len())
                .ok_or(PlatformError::InvalidKey)?;
            if total_bytes > MAX_KEY_BYTES {
                return Err(PlatformError::InvalidKey);
            }
            checked.push(value.to_owned());
        }
        if checked.is_empty() {
            return Err(PlatformError::InvalidKey);
        }
        Ok(Self { segments: checked })
    }

    pub fn new<I, S>(segments: I) -> Result<Self, PlatformError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::from_segments(segments)
    }

    pub fn single(segment: impl AsRef<str>) -> Result<Self, PlatformError> {
        Self::from_segments([segment.as_ref()])
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Debug for ObjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectKey")
            .field("segments", &self.segments.len())
            .finish()
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "object key ({} segments)", self.segments.len())
    }
}

pub(crate) fn validate_segment(value: &str) -> Result<(), PlatformError> {
    if value.is_empty()
        || value.len() > MAX_SEGMENT_BYTES
        || value.chars().count() > MAX_SEGMENT_SCALARS
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || looks_like_drive_prefix(value)
        || is_reserved_segment(value)
    {
        return Err(PlatformError::InvalidKey);
    }
    if Path::new(value).components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::CurDir | Component::ParentDir
        )
    }) {
        return Err(PlatformError::InvalidKey);
    }
    Ok(())
}

fn looks_like_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

pub(crate) fn is_reserved_segment(value: &str) -> bool {
    [STAGE_PREFIX, TRASH_PREFIX, RECOVERY_PREFIX, JOURNAL_PREFIX]
        .into_iter()
        .any(|prefix| value.starts_with(prefix))
}

pub(crate) fn is_owned_stage_name(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix(STAGE_PREFIX) else {
        return false;
    };
    suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn is_owned_trash_name(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix(TRASH_PREFIX) else {
        return false;
    };
    suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
}
