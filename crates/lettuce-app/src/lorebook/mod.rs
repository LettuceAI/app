pub(crate) mod lorebook_entry_dispatch;
pub(crate) mod lorebook_entry_execution;
pub(crate) mod lorebook_entry_job;
pub(crate) mod lorebook_entry_preparation;
pub(crate) mod lorebook_keyword_dispatch;
pub(crate) mod lorebook_keyword_execution;
pub(crate) mod lorebook_keyword_job;
pub(crate) mod staged_lorebook_coherence_dispatch;
pub(crate) mod staged_lorebook_coherence_execution;
pub(crate) mod staged_lorebook_dispatch;
pub(crate) mod staged_lorebook_execution;
pub(crate) mod staged_lorebook_job;
pub(crate) mod staged_lorebook_sources;
pub(crate) mod staged_lorebook_writer_dispatch;
pub(crate) mod staged_lorebook_writer_execution;
pub(crate) mod staged_lorebook_writer_job;

pub use lorebook_entry_dispatch::*;
pub use lorebook_entry_execution::*;
pub use lorebook_entry_job::*;
pub use lorebook_entry_preparation::*;
pub use lorebook_keyword_dispatch::*;
pub use lorebook_keyword_execution::*;
pub use lorebook_keyword_job::*;
pub use staged_lorebook_coherence_dispatch::*;
pub use staged_lorebook_coherence_execution::*;
pub use staged_lorebook_dispatch::*;
pub use staged_lorebook_execution::*;
pub use staged_lorebook_job::*;
pub use staged_lorebook_sources::*;
pub use staged_lorebook_writer_dispatch::*;
pub use staged_lorebook_writer_execution::*;
pub use staged_lorebook_writer_job::*;

pub trait LorebookJobAdmission: Send + Sync {
    fn admit_lorebook_job(
        &self,
        spec: lettuce_jobs::NewJob,
        input: lettuce_database::LorebookJobInput,
        operation: Option<(&str, &str)>,
    ) -> Result<
        (
            lettuce_jobs::JobSnapshot,
            bool,
            lettuce_database::LorebookJobInput,
        ),
        lettuce_jobs::StoreError,
    >;
}

impl LorebookJobAdmission for lettuce_database::Database {
    fn admit_lorebook_job(
        &self,
        spec: lettuce_jobs::NewJob,
        input: lettuce_database::LorebookJobInput,
        operation: Option<(&str, &str)>,
    ) -> Result<
        (
            lettuce_jobs::JobSnapshot,
            bool,
            lettuce_database::LorebookJobInput,
        ),
        lettuce_jobs::StoreError,
    > {
        self.admit_lorebook_job(spec, input, operation)
    }
}
