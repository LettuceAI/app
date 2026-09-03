use std::time::Duration;

use lettuce_companions::{
    CompanionConsolidationRun, CompanionConsolidationRunRepository,
    CompanionConsolidationRunRepositoryError, SoulOwner, SoulRepositoryError,
};
use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{JobId, TimestampMillis};

use crate::{CompanionConsolidationExecutionError, CompanionConsolidationExecutionResult};

#[derive(Debug, Clone)]
pub struct CompanionConsolidationClaimedWork {
    pub run: CompanionConsolidationRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum CompanionConsolidationSettledWork {
    Succeeded {
        result: CompanionConsolidationExecutionResult,
        job: JobSnapshot,
    },
    Failed {
        error: CompanionConsolidationExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: CompanionConsolidationExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: CompanionConsolidationExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionConsolidationDispatchError {
    #[error("companion consolidation job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("companion consolidation run operation failed: {0}")]
    Run(#[from] CompanionConsolidationRunRepositoryError),
    #[error("companion consolidation claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct CompanionConsolidationDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    runs: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> CompanionConsolidationDispatchCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(runs: &'a R, jobs: &'a J) -> Self {
        Self { runs, jobs }
    }
}

impl<R: CompanionConsolidationRunRepository + ?Sized, J: JobStore + ?Sized>
    CompanionConsolidationDispatchCoordinator<'_, R, J>
{
    pub fn claim(
        &self,
        job_id: JobId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<CompanionConsolidationClaimedWork>, CompanionConsolidationDispatchError>
    {
        let run = self.runs.load_companion_consolidation_run(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(CompanionConsolidationDispatchError::InvalidWork)?;
        if job.kind != JobKind::CompanionConsolidation {
            return Err(CompanionConsolidationDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Character(run.character_id) {
            return Err(CompanionConsolidationDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("companion-consolidation", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(CompanionConsolidationClaimedWork {
            run,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: CompanionConsolidationClaimedWork,
        result: Result<CompanionConsolidationExecutionResult, CompanionConsolidationExecutionError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<CompanionConsolidationSettledWork, CompanionConsolidationDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.kind != JobKind::CompanionConsolidation
            || work.job.state != JobState::Running
        {
            return Err(CompanionConsolidationDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        match result {
            Ok(result) => {
                if result.receipt.as_ref().is_some_and(|receipt| {
                    receipt.owner != SoulOwner::Character(work.run.character_id)
                        || receipt.operation_id != work.run.operation_id
                }) {
                    return Err(CompanionConsolidationDispatchError::InvalidWork);
                }
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
                        result_ref: OutcomeRef::Character(work.run.character_id),
                    },
                    at,
                })?;
                Ok(CompanionConsolidationSettledWork::Succeeded { result, job })
            }
            Err(error) if matches!(error, CompanionConsolidationExecutionError::Cancelled) => {
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
                Ok(CompanionConsolidationSettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(CompanionConsolidationSettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(CompanionConsolidationSettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &CompanionConsolidationExecutionError) -> bool {
    match error {
        CompanionConsolidationExecutionError::Inference(PortError::Unavailable) => true,
        CompanionConsolidationExecutionError::Inference(PortError::Provider(failure)) => {
            failure.kind == ProviderFailureKind::Unavailable
        }
        CompanionConsolidationExecutionError::ReplayCleanup
        | CompanionConsolidationExecutionError::Run(
            CompanionConsolidationRunRepositoryError::Failure,
        )
        | CompanionConsolidationExecutionError::Soul(SoulRepositoryError::Failure) => true,
        _ => false,
    }
}

fn job_error(error: &CompanionConsolidationExecutionError) -> JobError {
    let (code, message) = match error {
        CompanionConsolidationExecutionError::InvalidOwnership
        | CompanionConsolidationExecutionError::InvalidPrompt
        | CompanionConsolidationExecutionError::ContextTooLarge => (
            JobErrorCode::InvalidInput,
            "companion-consolidation-invalid-input",
        ),
        CompanionConsolidationExecutionError::Inference(_)
        | CompanionConsolidationExecutionError::InvalidResponse => (
            JobErrorCode::WorkerFailed,
            "companion-consolidation-provider-rejected",
        ),
        CompanionConsolidationExecutionError::Prompt(_)
        | CompanionConsolidationExecutionError::ReplayCleanup
        | CompanionConsolidationExecutionError::Run(_)
        | CompanionConsolidationExecutionError::Soul(_) => (
            JobErrorCode::StorageFailure,
            "companion-consolidation-storage-failed",
        ),
        CompanionConsolidationExecutionError::Policy(_) => (
            JobErrorCode::IntegrityFailure,
            "companion-consolidation-policy-rejected",
        ),
        CompanionConsolidationExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_consolidation_failures_retry() {
        assert!(retryable(&CompanionConsolidationExecutionError::Inference(
            PortError::Unavailable
        )));
        assert!(retryable(&CompanionConsolidationExecutionError::Run(
            CompanionConsolidationRunRepositoryError::Failure
        )));
        assert!(!retryable(
            &CompanionConsolidationExecutionError::InvalidResponse
        ));
        assert!(!retryable(&CompanionConsolidationExecutionError::Soul(
            SoulRepositoryError::Conflict
        )));
    }
}
