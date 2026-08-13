use lettuce_types::Revision;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("revision must be at least one")]
    ZeroRevision,
    #[error("{field} must not be blank")]
    Blank { field: &'static str },
    #[error("{field} exceeds the {max}-scalar limit")]
    TooLong { field: &'static str, max: usize },
    #[error("{field} exceeds the authored-content byte limit")]
    TooLarge { field: &'static str },
    #[error("{field} has more than {max} items")]
    TooMany { field: &'static str, max: usize },
    #[error("{field} contains a duplicate")]
    Duplicate { field: &'static str },
    #[error("{field} must be contiguous and zero-based")]
    NonContiguous { field: &'static str },
    #[error("{field} contains an invalid value")]
    InvalidValue { field: &'static str },
    #[error("{field} contains a non-finite number")]
    NonFinite { field: &'static str },
    #[error("{field} has an invalid reference")]
    InvalidReference { field: &'static str },
    #[error("{field} uses an unsupported format version {version}")]
    UnsupportedVersion { field: &'static str, version: u32 },
    #[error("{field} violates an aggregate invariant")]
    Invariant { field: &'static str },
    #[error("{field} must not be later than updated_at")]
    InvalidTimestampOrder { field: &'static str },
    #[error("revision overflow")]
    RevisionOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepositoryError {
    #[error("record was not found")]
    NotFound,
    #[error("record already exists")]
    AlreadyExists,
    #[error("revision {expected} is stale; current revision is {actual}")]
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    #[error("the record is archived")]
    Archived,
    #[error("the operation has dependent records")]
    HasDependencies,
    #[error("the proposed record is invalid: {0}")]
    Invalid(ValidationError),
    #[error("storage operation failed")]
    Storage,
}

impl From<ValidationError> for RepositoryError {
    fn from(value: ValidationError) -> Self {
        Self::Invalid(value)
    }
}

impl From<lettuce_types::RevisionOverflow> for ValidationError {
    fn from(_: lettuce_types::RevisionOverflow) -> Self {
        Self::RevisionOverflow
    }
}
