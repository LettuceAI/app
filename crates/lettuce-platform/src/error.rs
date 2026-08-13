use std::{fmt, io};

/// Safe, non-path-bearing platform errors suitable for IPC or diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformError {
    InvalidKey,
    InvalidRoot,
    RootDeletion,
    WrongCapability,
    SymlinkEscape,
    NotFound,
    Conflict,
    Denied,
    Unsupported,
    ReplaceFailed,
    LimitExceeded,
    RecoveryNeeded,
    Io,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidKey => "invalid object key",
            Self::InvalidRoot => "invalid managed root",
            Self::RootDeletion => "managed root deletion is not permitted",
            Self::WrongCapability => "capability does not belong to this filesystem",
            Self::SymlinkEscape => "symbolic-link traversal is not permitted",
            Self::NotFound => "managed object was not found",
            Self::Conflict => "managed object conflicts with an existing object",
            Self::Denied => "managed filesystem operation was denied",
            Self::Unsupported => "managed filesystem operation is unsupported",
            Self::ReplaceFailed => "atomic replacement could not be completed",
            Self::LimitExceeded => "managed filesystem limit exceeded",
            Self::RecoveryNeeded => "managed filesystem recovery is required",
            Self::Io => "managed filesystem operation failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PlatformError {}

impl From<io::Error> for PlatformError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::AlreadyExists => Self::Conflict,
            io::ErrorKind::PermissionDenied => Self::Denied,
            io::ErrorKind::InvalidInput => Self::InvalidKey,
            io::ErrorKind::CrossesDevices => Self::Unsupported,
            _ => Self::Io,
        }
    }
}
