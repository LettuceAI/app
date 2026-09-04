use std::time::Duration;

use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_creation::{
    LorebookEntryGenerationRun, LorebookEntryRunRepository, LorebookEntryRunRepositoryError,
};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

use crate::{LorebookEntryExecutionError, LorebookEntryExecutionResult};

#[derive(Debug, Clone)]
pub struct LorebookEntryClaimedWork {
    pub run: LorebookEntryGenerationRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum LorebookEntrySettledWork {
    Succeeded {
        result: LorebookEntryExecutionResult,
        job: JobSnapshot,
    },
    Failed {
        error: LorebookEntryExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: LorebookEntryExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: LorebookEntryExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookEntryDispatchError {
    #[error("lorebook entry generation job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("lorebook entry generation run operation failed: {0}")]
    Run(#[from] LorebookEntryRunRepositoryError),
    #[error("lorebook entry generation claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct LorebookEntryDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    runs: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> LorebookEntryDispatchCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(runs: &'a R, jobs: &'a J) -> Self {
        Self { runs, jobs }
    }
}

impl<R: LorebookEntryRunRepository + ?Sized, J: JobStore + ?Sized>
    LorebookEntryDispatchCoordinator<'_, R, J>
{
    pub fn claim(
        &self,
        request_id: RequestId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<LorebookEntryClaimedWork>, LorebookEntryDispatchError> {
        let run = self.runs.load_lorebook_entry_run(request_id)?;
        let job = self
            .jobs
            .get(run.job_id)?
            .ok_or(LorebookEntryDispatchError::InvalidWork)?;
        if job.kind != JobKind::CreationRun {
            return Err(LorebookEntryDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .jobs
            .claim(run.job_id, worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(request_id) {
            return Err(LorebookEntryDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(run.job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("lorebook-entry", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(LorebookEntryClaimedWork {
            run,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: LorebookEntryClaimedWork,
        result: Result<LorebookEntryExecutionResult, LorebookEntryExecutionError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<LorebookEntrySettledWork, LorebookEntryDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.kind != JobKind::CreationRun
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::Request(work.run.request_id)
        {
            return Err(LorebookEntryDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        match result {
            Ok(result) => {
                self.jobs.append_and_transition(JobMutation::Progress {
                    claim: work.claim.claim.clone(),
                    progress: ProgressSnapshot {
                        fraction: Some(
                            FiniteFraction::new(1.0).expect("constant job progress is valid"),
                        ),
                        ..ProgressSnapshot::default()
                    },
                    at,
                })?;
                let job = self.jobs.append_and_transition(JobMutation::Succeed {
                    claim: work.claim.claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::Request(work.run.request_id),
                    },
                    at,
                })?;
                Ok(LorebookEntrySettledWork::Succeeded { result, job })
            }
            Err(error) if matches!(error, LorebookEntryExecutionError::Cancelled) => {
                self.jobs
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: work.claim.claim.job_id,
                        reason: cancellation_reason,
                        at,
                    })?;
                self.jobs
                    .append_and_transition(JobMutation::RequestCleanup {
                        claim: work.claim.claim.clone(),
                        at,
                    })?;
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::FinishCancellation {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(LorebookEntrySettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(LorebookEntrySettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(LorebookEntrySettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &LorebookEntryExecutionError) -> bool {
    match error {
        LorebookEntryExecutionError::Inference(PortError::Unavailable) => true,
        LorebookEntryExecutionError::Inference(PortError::Provider(failure)) => {
            failure.kind == ProviderFailureKind::Unavailable
        }
        LorebookEntryExecutionError::ReplayCleanup
        | LorebookEntryExecutionError::Run(LorebookEntryRunRepositoryError::Failure) => true,
        _ => false,
    }
}

fn job_error(error: &LorebookEntryExecutionError) -> JobError {
    let (code, message) = match error {
        LorebookEntryExecutionError::InvalidOwnership
        | LorebookEntryExecutionError::InvalidPrompt
        | LorebookEntryExecutionError::ContextTooLarge => {
            (JobErrorCode::InvalidInput, "lorebook-entry-invalid-input")
        }
        LorebookEntryExecutionError::Inference(_)
        | LorebookEntryExecutionError::InvalidResponse => (
            JobErrorCode::WorkerFailed,
            "lorebook-entry-provider-rejected",
        ),
        LorebookEntryExecutionError::Prompt(_)
        | LorebookEntryExecutionError::ReplayCleanup
        | LorebookEntryExecutionError::Run(_) => (
            JobErrorCode::StorageFailure,
            "lorebook-entry-storage-failed",
        ),
        LorebookEntryExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_lorebook_entry_failures_retry() {
        assert!(retryable(&LorebookEntryExecutionError::Inference(
            PortError::Unavailable
        )));
        assert!(retryable(&LorebookEntryExecutionError::Run(
            LorebookEntryRunRepositoryError::Failure
        )));
        assert!(!retryable(&LorebookEntryExecutionError::InvalidResponse));
    }
}
