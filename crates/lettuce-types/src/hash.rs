use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn parse(value: impl Into<String>) -> Result<Self, ParseContentHashError> {
        let value = value.into();
        if value.len() != BLAKE3_HEX_LENGTH {
            return Err(ParseContentHashError::Length(value.len()));
        }
        if !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ParseContentHashError::Characters);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ContentHash {
    type Err = ParseContentHashError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseContentHashError {
    #[error("expected a 64-character BLAKE3 hash, got {0} characters")]
    Length(usize),
    #[error("content hash contains non-hexadecimal characters")]
    Characters,
}

#[cfg(test)]
mod tests {
    use super::{ContentHash, ParseContentHashError};

    #[test]
    fn validates_and_normalizes_hashes() {
        let value = "AB".repeat(32);
        let hash = ContentHash::parse(value).expect("valid hash");
        assert_eq!(hash.as_str(), "ab".repeat(32));
        assert_eq!(
            ContentHash::parse("nope"),
            Err(ParseContentHashError::Length(4))
        );
    }
}
