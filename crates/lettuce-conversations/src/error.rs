use lettuce_types::Revision;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("revision must be at least one")]
    ZeroRevision,
    #[error("{field} must not be blank")]
    Blank { field: &'static str },
    #[error("{field} exceeds its authored-content bound")]
    TooLarge { field: &'static str },
    #[error("{field} has more than {max} items")]
    TooMany { field: &'static str, max: usize },
    #[error("{field} contains a duplicate")]
    Duplicate { field: &'static str },
    #[error("{field} contains an invalid value")]
    InvalidValue { field: &'static str },
    #[error("{field} has an invalid reference")]
    InvalidReference { field: &'static str },
    #[error("{field} has an unsupported version {version}")]
    UnsupportedVersion { field: &'static str, version: u32 },
    #[error("{field} has an invalid timestamp order")]
    InvalidTimestampOrder { field: &'static str },
    #[error("{field} violates an aggregate invariant")]
    Invariant { field: &'static str },
    #[error("{field} has an illegal lifecycle transition")]
    IllegalTransition { field: &'static str },
    #[error("{field} is outside the supported bound")]
    OutOfBounds { field: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversationRepositoryError {
    #[error("conversation record was not found")]
    NotFound,
    #[error("the expected conversation revision is stale")]
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    #[error("the conversation mutation conflicts with current state")]
    Conflict,
    #[error("the conversation has a dependent record")]
    Dependency,
    #[error("the conversation operation is invalid")]
    Invalid(#[from] ValidationError),
    #[error("the conversation operation is unsupported")]
    Unsupported,
    #[error("conversation storage is unavailable")]
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversationServiceError {
    #[error("conversation operation is invalid")]
    Invalid(#[from] ValidationError),
    #[error("conversation repository failed")]
    Repository(#[from] ConversationRepositoryError),
    #[error("conversation dependency failed")]
    Dependency,
    #[error("conversation operation is unsupported")]
    Unsupported,
}
