//! Read models over the job store for hosts: a filtered listing, newest
//! first, and a feed of changed jobs.

use lettuce_types::{JobId, Page, PageRequest};

use crate::{JobKind, JobSnapshot, JobState, StoreError, SubjectId, SubjectKind};

/// An empty `kinds` or `states` matches every value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobListFilter {
    pub kinds: Vec<JobKind>,
    pub states: Vec<JobState>,
    pub subject: Option<(SubjectKind, SubjectId)>,
    pub page: PageRequest,
}

/// A job whose snapshot changed, at the feed position of its latest change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobChange {
    pub job_id: JobId,
    pub position: u64,
}

pub trait JobCatalog: Send + Sync {
    /// Jobs matching `filter`, most recently created first; the cursor is
    /// opaque and stable while jobs are created.
    fn list_jobs(&self, filter: &JobListFilter) -> Result<Page<JobSnapshot>, StoreError>;

    /// The feed position of the latest recorded change.
    fn job_change_position(&self) -> Result<u64, StoreError>;

    /// Jobs changed after `after`, in feed order, each once with the
    /// position of its latest change, at most `limit` of them.
    fn job_changes_since(&self, after: u64, limit: u32) -> Result<Vec<JobChange>, StoreError>;
}
