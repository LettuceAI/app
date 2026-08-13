use std::fmt;

/// The representation used for values that must never be rendered.
pub const REDACTED: &str = "[REDACTED]";

/// Wraps a secret or other protected value with unconditional redaction.
///
/// The wrapped value is available only through an explicit accessor. Both
/// [`Debug`](fmt::Debug) and [`Display`](fmt::Display) intentionally discard
/// it, including when the wrapper is used inside another formatted error.
#[derive(Clone, PartialEq, Eq)]
pub struct Sensitive<T>(T);

impl<T> Sensitive<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }

    #[must_use]
    pub const fn as_inner(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Sensitive<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

impl<T> fmt::Display for Sensitive<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

/// Wraps prompts, messages and other user content with unconditional
/// redaction.
///
/// This wrapper does not grant permission to log the content. It exists to
/// make accidental debug, display or error formatting safe by construction.
#[derive(Clone, PartialEq, Eq)]
pub struct UserContent<T>(T);

impl<T> UserContent<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }

    #[must_use]
    pub const fn as_inner(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for UserContent<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

impl<T> fmt::Display for UserContent<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}
