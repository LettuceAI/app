use std::time::Duration;

use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_creation::{
    LorebookKeywordGenerationRun, LorebookKeywordRunRepository, LorebookKeywordRunRepositoryError,
};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

use crate::{LorebookKeywordExecutionError, LorebookKeywordExecutionResult};

#[derive(Debug, Clone)]
pub struct LorebookKeywordClaimedWork {
    pub run: LorebookKeywordGenerationRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum LorebookKeywordSettledWork {
    Succeeded {
        result: LorebookKeywordExecutionResult,
        job: JobSnapshot,
    },
    Failed {
        error: LorebookKeywordExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: LorebookKeywordExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: LorebookKeywordExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookKeywordDispatchError {
    #[error("lorebook keyword generation job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("lorebook keyword generation run operation failed: {0}")]
    Run(#[from] LorebookKeywordRunRepositoryError),
    #[error("lorebook keyword generation claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct LorebookKeywordDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    runs: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> LorebookKeywordDispatchCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(runs: &'a R, jobs: &'a J) -> Self {
        Self { runs, jobs }
    }
}

impl<R: LorebookKeywordRunRepository + ?Sized, J: JobStore + ?Sized>
    LorebookKeywordDispatchCoordinator<'_, R, J>
{
    pub fn claim(
        &self,
        request_id: RequestId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<LorebookKeywordClaimedWork>, LorebookKeywordDispatchError> {
        let run = self.runs.load_lorebook_keyword_run(request_id)?;
        let job = self
            .jobs
            .get(run.job_id)?
            .ok_or(LorebookKeywordDispatchError::InvalidWork)?;
        if job.kind != JobKind::CreationRun {
            return Err(LorebookKeywordDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .jobs
            .claim(run.job_id, worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(request_id) {
            return Err(LorebookKeywordDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(run.job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("lorebook-keyword", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(LorebookKeywordClaimedWork {
            run,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: LorebookKeywordClaimedWork,
        result: Result<LorebookKeywordExecutionResult, LorebookKeywordExecutionError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<LorebookKeywordSettledWork, LorebookKeywordDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.kind != JobKind::CreationRun
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::Request(work.run.request_id)
        {
            return Err(LorebookKeywordDispatchError::InvalidWork);
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
                Ok(LorebookKeywordSettledWork::Succeeded { result, job })
            }
            Err(error) if matches!(error, LorebookKeywordExecutionError::Cancelled) => {
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
                Ok(LorebookKeywordSettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(LorebookKeywordSettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(LorebookKeywordSettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &LorebookKeywordExecutionError) -> bool {
    match error {
        LorebookKeywordExecutionError::Inference(PortError::Unavailable) => true,
        LorebookKeywordExecutionError::Inference(PortError::Provider(failure)) => {
            failure.kind == ProviderFailureKind::Unavailable
        }
        LorebookKeywordExecutionError::ReplayCleanup
        | LorebookKeywordExecutionError::Run(LorebookKeywordRunRepositoryError::Failure) => true,
        _ => false,
    }
}

fn job_error(error: &LorebookKeywordExecutionError) -> JobError {
    let (code, message) = match error {
        LorebookKeywordExecutionError::InvalidOwnership
        | LorebookKeywordExecutionError::InvalidPrompt
        | LorebookKeywordExecutionError::ContextTooLarge => {
            (JobErrorCode::InvalidInput, "lorebook-keyword-invalid-input")
        }
        LorebookKeywordExecutionError::Inference(_)
        | LorebookKeywordExecutionError::InvalidResponse => (
            JobErrorCode::WorkerFailed,
            "lorebook-keyword-provider-rejected",
        ),
        LorebookKeywordExecutionError::Prompt(_)
        | LorebookKeywordExecutionError::ReplayCleanup
        | LorebookKeywordExecutionError::Run(_) => (
            JobErrorCode::StorageFailure,
            "lorebook-keyword-storage-failed",
        ),
        LorebookKeywordExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_lorebook_keyword_failures_retry() {
        assert!(retryable(&LorebookKeywordExecutionError::Inference(
            PortError::Unavailable
        )));
        assert!(retryable(&LorebookKeywordExecutionError::Run(
            LorebookKeywordRunRepositoryError::Failure
        )));
        assert!(!retryable(&LorebookKeywordExecutionError::InvalidResponse));
    }
}
