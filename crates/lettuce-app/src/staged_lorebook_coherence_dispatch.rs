use std::time::Duration;

use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_creation::{
    StagedLorebookCoherenceRun, StagedLorebookRepository, StagedLorebookRepositoryError,
};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

use crate::{StagedLorebookCoherenceExecutionError, StagedLorebookCoherenceExecutionResult};

#[derive(Debug, Clone)]
pub struct StagedLorebookCoherenceClaimedWork {
    pub project_request_id: RequestId,
    pub run: StagedLorebookCoherenceRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum StagedLorebookCoherenceSettledWork {
    Succeeded {
        result: Box<StagedLorebookCoherenceExecutionResult>,
        job: JobSnapshot,
    },
    Failed {
        error: StagedLorebookCoherenceExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: StagedLorebookCoherenceExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: StagedLorebookCoherenceExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookCoherenceDispatchError {
    #[error("staged lorebook coherence job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("staged lorebook coherence repository operation failed: {0}")]
    Repository(#[from] StagedLorebookRepositoryError),
    #[error("staged lorebook coherence claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct StagedLorebookCoherenceDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> StagedLorebookCoherenceDispatchCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(repository: &'a R, jobs: &'a J) -> Self {
        Self { repository, jobs }
    }
}

impl<R: StagedLorebookRepository + ?Sized, J: JobStore + ?Sized>
    StagedLorebookCoherenceDispatchCoordinator<'_, R, J>
{
    pub fn claim(
        &self,
        project_request_id: RequestId,
        coherence_request_id: RequestId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<StagedLorebookCoherenceClaimedWork>, StagedLorebookCoherenceDispatchError>
    {
        let project = self.repository.load_staged_lorebook(project_request_id)?;
        let run = project
            .coherence_runs
            .iter()
            .find(|run| run.request_id == coherence_request_id)
            .cloned()
            .ok_or(StagedLorebookCoherenceDispatchError::InvalidWork)?;
        let job = self
            .jobs
            .get(run.job_id)?
            .ok_or(StagedLorebookCoherenceDispatchError::InvalidWork)?;
        if job.kind != JobKind::CreationRun {
            return Err(StagedLorebookCoherenceDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .jobs
            .claim(run.job_id, worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(coherence_request_id) {
            return Err(StagedLorebookCoherenceDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(run.job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("staged-lorebook-coherence", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(StagedLorebookCoherenceClaimedWork {
            project_request_id,
            run,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: StagedLorebookCoherenceClaimedWork,
        result: Result<
            StagedLorebookCoherenceExecutionResult,
            StagedLorebookCoherenceExecutionError,
        >,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<StagedLorebookCoherenceSettledWork, StagedLorebookCoherenceDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.job.id
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::Request(work.run.request_id)
        {
            return Err(StagedLorebookCoherenceDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        let result = if self
            .repository
            .load_staged_lorebook(work.project_request_id)?
            .project
            .stage
            == lettuce_creation::StagedLorebookStage::Cancelled
        {
            Err(StagedLorebookCoherenceExecutionError::Cancelled)
        } else {
            result
        };
        match result {
            Ok(result) => {
                self.jobs.append_and_transition(JobMutation::Progress {
                    claim: work.claim.claim.clone(),
                    progress: ProgressSnapshot {
                        fraction: Some(
                            FiniteFraction::new(1.0).expect("constant progress is valid"),
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
                Ok(StagedLorebookCoherenceSettledWork::Succeeded {
                    result: Box::new(result),
                    job,
                })
            }
            Err(error) if matches!(error, StagedLorebookCoherenceExecutionError::Cancelled) => {
                self.jobs
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: work.job.id,
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
                Ok(StagedLorebookCoherenceSettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(StagedLorebookCoherenceSettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(StagedLorebookCoherenceSettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &StagedLorebookCoherenceExecutionError) -> bool {
    matches!(
        error,
        StagedLorebookCoherenceExecutionError::Inference(PortError::Unavailable)
            | StagedLorebookCoherenceExecutionError::ReplayCleanup
            | StagedLorebookCoherenceExecutionError::Repository(
                StagedLorebookRepositoryError::Failure
            )
    ) || matches!(error, StagedLorebookCoherenceExecutionError::Inference(PortError::Provider(failure)) if failure.kind == ProviderFailureKind::Unavailable)
}

fn job_error(error: &StagedLorebookCoherenceExecutionError) -> JobError {
    let (code, message) = match error {
        StagedLorebookCoherenceExecutionError::InvalidOwnership
        | StagedLorebookCoherenceExecutionError::InvalidPrompt
        | StagedLorebookCoherenceExecutionError::ContextTooLarge => (
            JobErrorCode::InvalidInput,
            "staged-lorebook-coherence-invalid-input",
        ),
        StagedLorebookCoherenceExecutionError::Inference(_)
        | StagedLorebookCoherenceExecutionError::InvalidResponse => (
            JobErrorCode::WorkerFailed,
            "staged-lorebook-coherence-provider-rejected",
        ),
        StagedLorebookCoherenceExecutionError::Prompt(_)
        | StagedLorebookCoherenceExecutionError::ReplayCleanup
        | StagedLorebookCoherenceExecutionError::Repository(_) => (
            JobErrorCode::StorageFailure,
            "staged-lorebook-coherence-storage-failed",
        ),
        StagedLorebookCoherenceExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_coherence_failures_retry() {
        assert!(retryable(
            &StagedLorebookCoherenceExecutionError::Inference(PortError::Unavailable)
        ));
        assert!(!retryable(
            &StagedLorebookCoherenceExecutionError::InvalidResponse
        ));
    }
}
