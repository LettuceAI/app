use std::time::Duration;

use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_creation::{
    StagedLorebookPlanningRun, StagedLorebookRepository, StagedLorebookRepositoryError,
    StagedLorebookStage,
};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

use crate::{StagedLorebookPlannerExecutionError, StagedLorebookPlannerExecutionResult};

#[derive(Debug, Clone)]
pub struct StagedLorebookPlannerClaimedWork {
    pub run: StagedLorebookPlanningRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum StagedLorebookPlannerSettledWork {
    Succeeded {
        result: Box<StagedLorebookPlannerExecutionResult>,
        job: JobSnapshot,
    },
    Failed {
        error: StagedLorebookPlannerExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: StagedLorebookPlannerExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: StagedLorebookPlannerExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookPlannerDispatchError {
    #[error("staged lorebook planner job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("staged lorebook planner repository operation failed: {0}")]
    Repository(#[from] StagedLorebookRepositoryError),
    #[error("staged lorebook planner claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct StagedLorebookPlannerDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    runs: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> StagedLorebookPlannerDispatchCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(runs: &'a R, jobs: &'a J) -> Self {
        Self { runs, jobs }
    }
}

impl<R: StagedLorebookRepository + ?Sized, J: JobStore + ?Sized>
    StagedLorebookPlannerDispatchCoordinator<'_, R, J>
{
    pub fn claim(
        &self,
        request_id: RequestId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<StagedLorebookPlannerClaimedWork>, StagedLorebookPlannerDispatchError> {
        let run = self.runs.load_staged_lorebook(request_id)?;
        if run.project.stage != StagedLorebookStage::Planning {
            return Err(StagedLorebookPlannerDispatchError::InvalidWork);
        }
        let job = self
            .jobs
            .get(run.job_id)?
            .ok_or(StagedLorebookPlannerDispatchError::InvalidWork)?;
        if job.kind != JobKind::CreationRun {
            return Err(StagedLorebookPlannerDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .jobs
            .claim(run.job_id, worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(request_id) {
            return Err(StagedLorebookPlannerDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(run.job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("staged-lorebook-planner", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(StagedLorebookPlannerClaimedWork {
            run,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: StagedLorebookPlannerClaimedWork,
        result: Result<StagedLorebookPlannerExecutionResult, StagedLorebookPlannerExecutionError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlannerSettledWork, StagedLorebookPlannerDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.kind != JobKind::CreationRun
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::Request(work.run.request_id)
        {
            return Err(StagedLorebookPlannerDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        let result = if self
            .runs
            .load_staged_lorebook(work.run.request_id)?
            .project
            .stage
            == lettuce_creation::StagedLorebookStage::Cancelled
        {
            Err(StagedLorebookPlannerExecutionError::Cancelled)
        } else {
            result
        };
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
                Ok(StagedLorebookPlannerSettledWork::Succeeded {
                    result: Box::new(result),
                    job,
                })
            }
            Err(error) if matches!(error, StagedLorebookPlannerExecutionError::Cancelled) => {
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
                Ok(StagedLorebookPlannerSettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(StagedLorebookPlannerSettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(StagedLorebookPlannerSettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &StagedLorebookPlannerExecutionError) -> bool {
    match error {
        StagedLorebookPlannerExecutionError::Inference(PortError::Unavailable) => true,
        StagedLorebookPlannerExecutionError::Inference(PortError::Provider(failure)) => {
            failure.kind == ProviderFailureKind::Unavailable
        }
        StagedLorebookPlannerExecutionError::ReplayCleanup
        | StagedLorebookPlannerExecutionError::Repository(StagedLorebookRepositoryError::Failure) => {
            true
        }
        _ => false,
    }
}

fn job_error(error: &StagedLorebookPlannerExecutionError) -> JobError {
    let (code, message) = match error {
        StagedLorebookPlannerExecutionError::InvalidOwnership
        | StagedLorebookPlannerExecutionError::InvalidPrompt
        | StagedLorebookPlannerExecutionError::ContextTooLarge => (
            JobErrorCode::InvalidInput,
            "staged-lorebook-planner-invalid-input",
        ),
        StagedLorebookPlannerExecutionError::Inference(_)
        | StagedLorebookPlannerExecutionError::InvalidResponse => (
            JobErrorCode::WorkerFailed,
            "staged-lorebook-planner-provider-rejected",
        ),
        StagedLorebookPlannerExecutionError::Prompt(_)
        | StagedLorebookPlannerExecutionError::ReplayCleanup
        | StagedLorebookPlannerExecutionError::Repository(_) => (
            JobErrorCode::StorageFailure,
            "staged-lorebook-planner-storage-failed",
        ),
        StagedLorebookPlannerExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_staged_planner_failures_retry() {
        assert!(retryable(&StagedLorebookPlannerExecutionError::Inference(
            PortError::Unavailable
        )));
        assert!(retryable(&StagedLorebookPlannerExecutionError::Repository(
            StagedLorebookRepositoryError::Failure
        )));
        assert!(!retryable(
            &StagedLorebookPlannerExecutionError::InvalidResponse
        ));
    }
}
