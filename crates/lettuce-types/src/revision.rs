use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Revision(u64);

impl Revision {
    pub const INITIAL: Self = Self(1);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, RevisionOverflow> {
        self.0.checked_add(1).map(Self).ok_or(RevisionOverflow)
    }
}

impl Default for Revision {
    fn default() -> Self {
        Self::INITIAL
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("revision overflow")]
pub struct RevisionOverflow;

#[cfg(test)]
mod tests {
    use super::{Revision, RevisionOverflow};

    #[test]
    fn increments_checked() {
        assert_eq!(Revision::INITIAL.next(), Ok(Revision::new(2)));
        assert_eq!(Revision::new(u64::MAX).next(), Err(RevisionOverflow));
    }
}
