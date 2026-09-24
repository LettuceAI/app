use std::time::Duration;

use lettuce_companions::{
    CompanionSoulWriterRun, CompanionSoulWriterRunRepository, CompanionSoulWriterRunRepositoryError,
};
use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

use crate::{CompanionSoulWriterExecutionError, CompanionSoulWriterExecutionResult};

#[derive(Debug, Clone)]
pub struct CompanionSoulWriterClaimedWork {
    pub run: CompanionSoulWriterRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum CompanionSoulWriterSettledWork {
    Succeeded {
        result: CompanionSoulWriterExecutionResult,
        job: JobSnapshot,
    },
    Failed {
        error: CompanionSoulWriterExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: CompanionSoulWriterExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: CompanionSoulWriterExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionSoulWriterDispatchError {
    #[error("companion Soul-writer job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("companion Soul-writer run operation failed: {0}")]
    Run(#[from] CompanionSoulWriterRunRepositoryError),
    #[error("companion Soul-writer claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct CompanionSoulWriterDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    runs: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> CompanionSoulWriterDispatchCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(runs: &'a R, jobs: &'a J) -> Self {
        Self { runs, jobs }
    }
}

impl<R: CompanionSoulWriterRunRepository + ?Sized, J: JobStore + ?Sized>
    CompanionSoulWriterDispatchCoordinator<'_, R, J>
{
    pub fn claim(
        &self,
        request_id: RequestId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<CompanionSoulWriterClaimedWork>, CompanionSoulWriterDispatchError> {
        let run = self.runs.load_companion_soul_writer_run(request_id)?;
        let job = self
            .jobs
            .get(run.job_id)?
            .ok_or(CompanionSoulWriterDispatchError::InvalidWork)?;
        if job.kind != JobKind::CompanionSoulWriter {
            return Err(CompanionSoulWriterDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .jobs
            .claim(run.job_id, worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(request_id) {
            return Err(CompanionSoulWriterDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(run.job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("companion-soul-writer", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(CompanionSoulWriterClaimedWork {
            run,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: CompanionSoulWriterClaimedWork,
        result: Result<CompanionSoulWriterExecutionResult, CompanionSoulWriterExecutionError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<CompanionSoulWriterSettledWork, CompanionSoulWriterDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.kind != JobKind::CompanionSoulWriter
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::Request(work.run.request_id)
        {
            return Err(CompanionSoulWriterDispatchError::InvalidWork);
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
                Ok(CompanionSoulWriterSettledWork::Succeeded { result, job })
            }
            Err(error) if matches!(error, CompanionSoulWriterExecutionError::Cancelled) => {
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
                Ok(CompanionSoulWriterSettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(CompanionSoulWriterSettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(CompanionSoulWriterSettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &CompanionSoulWriterExecutionError) -> bool {
    match error {
        CompanionSoulWriterExecutionError::Inference(PortError::Unavailable) => true,
        CompanionSoulWriterExecutionError::Inference(PortError::Provider(failure)) => {
            failure.kind == ProviderFailureKind::Unavailable
        }
        CompanionSoulWriterExecutionError::ReplayCleanup
        | CompanionSoulWriterExecutionError::Run(CompanionSoulWriterRunRepositoryError::Failure) => {
            true
        }
        _ => false,
    }
}

fn job_error(error: &CompanionSoulWriterExecutionError) -> JobError {
    let (code, message) = match error {
        CompanionSoulWriterExecutionError::InvalidOwnership
        | CompanionSoulWriterExecutionError::InvalidPrompt
        | CompanionSoulWriterExecutionError::ContextTooLarge => (
            JobErrorCode::InvalidInput,
            "companion-soul-writer-invalid-input",
        ),
        CompanionSoulWriterExecutionError::Inference(_)
        | CompanionSoulWriterExecutionError::InvalidResponse
        | CompanionSoulWriterExecutionError::RoundLimit => (
            JobErrorCode::WorkerFailed,
            "companion-soul-writer-provider-rejected",
        ),
        CompanionSoulWriterExecutionError::Prompt(_)
        | CompanionSoulWriterExecutionError::ReplayCleanup
        | CompanionSoulWriterExecutionError::Run(_) => (
            JobErrorCode::StorageFailure,
            "companion-soul-writer-storage-failed",
        ),
        CompanionSoulWriterExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_soul_writer_failures_retry() {
        assert!(retryable(&CompanionSoulWriterExecutionError::Inference(
            PortError::Unavailable
        )));
        assert!(retryable(&CompanionSoulWriterExecutionError::Run(
            CompanionSoulWriterRunRepositoryError::Failure
        )));
        assert!(!retryable(&CompanionSoulWriterExecutionError::RoundLimit));
        assert!(!retryable(
            &CompanionSoulWriterExecutionError::InvalidResponse
        ));
    }
}
