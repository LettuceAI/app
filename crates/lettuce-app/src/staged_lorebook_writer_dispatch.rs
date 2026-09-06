use std::time::Duration;

use crate::{StagedLorebookWriterExecutionError, StagedLorebookWriterExecutionResult};
use lettuce_conversations::{PortError, ProviderFailureKind};
use lettuce_creation::{
    StagedLorebookRepository, StagedLorebookRepositoryError, StagedLorebookWriterRun,
    StagedLorebookWriterRunRepository, StagedLorebookWriterRunRepositoryError,
};
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot,
    ResourceAvailability, StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

#[derive(Debug, Clone)]
pub struct StagedLorebookWriterClaimedWork {
    pub run: StagedLorebookWriterRun,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum StagedLorebookWriterSettledWork {
    Succeeded {
        result: Box<StagedLorebookWriterExecutionResult>,
        job: JobSnapshot,
    },
    Failed {
        error: StagedLorebookWriterExecutionError,
        job: JobSnapshot,
    },
    Cancelled {
        error: StagedLorebookWriterExecutionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: StagedLorebookWriterExecutionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookWriterDispatchError {
    #[error("staged lorebook project operation failed: {0}")]
    Project(#[from] StagedLorebookRepositoryError),
    #[error("staged lorebook writer job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("staged lorebook writer run operation failed: {0}")]
    Run(#[from] StagedLorebookWriterRunRepositoryError),
    #[error("staged lorebook writer claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct StagedLorebookWriterDispatchCoordinator<'a, R: ?Sized, P: ?Sized, J: ?Sized> {
    runs: &'a R,
    projects: &'a P,
    jobs: &'a J,
}
impl<'a, R: ?Sized, P: ?Sized, J: ?Sized> StagedLorebookWriterDispatchCoordinator<'a, R, P, J> {
    #[must_use]
    pub const fn new(runs: &'a R, projects: &'a P, jobs: &'a J) -> Self {
        Self {
            runs,
            projects,
            jobs,
        }
    }
}

impl<
    R: StagedLorebookWriterRunRepository + ?Sized,
    P: StagedLorebookRepository + ?Sized,
    J: JobStore + ?Sized,
> StagedLorebookWriterDispatchCoordinator<'_, R, P, J>
{
    pub fn claim(
        &self,
        request_id: RequestId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<StagedLorebookWriterClaimedWork>, StagedLorebookWriterDispatchError> {
        let run = self.runs.load_staged_lorebook_writer_run(request_id)?;
        let job = self
            .jobs
            .get(run.job_id)?
            .ok_or(StagedLorebookWriterDispatchError::InvalidWork)?;
        if job.kind != JobKind::CreationRun {
            return Err(StagedLorebookWriterDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .jobs
            .claim(run.job_id, worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(request_id) {
            return Err(StagedLorebookWriterDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(run.job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("staged-lorebook-writer", false)
                .expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(StagedLorebookWriterClaimedWork {
            run,
            claim,
            handle,
            job,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_batch<I: lettuce_conversations::InferencePort + ?Sized>(
        &self,
        batch: &crate::StagedLorebookWriterBatchAdmission,
        inference: &I,
        prompt: &lettuce_context::PromptDocument,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Vec<Result<Option<StagedLorebookWriterSettledWork>, StagedLorebookWriterDispatchError>>
    where
        R: lettuce_conversations::ProviderReplayArtifactPort + lettuce_usage::JobUsageLedger,
    {
        use futures_util::{StreamExt, stream::FuturesUnordered};

        let mut requests = std::collections::HashSet::new();
        if batch.writers.len() > lettuce_creation::STAGED_LOREBOOK_DRAFT_BATCH_SIZE
            || batch
                .writers
                .iter()
                .any(|writer| !requests.insert(writer.run.request_id))
        {
            return vec![Err(StagedLorebookWriterDispatchError::InvalidWork)];
        }
        let executor = crate::StagedLorebookWriterExecutionCoordinator::new(
            self.runs,
            self.projects,
            inference,
        );
        let mut pending = FuturesUnordered::new();
        for writer in &batch.writers {
            let executor = &executor;
            pending.push(async move {
                let run = self
                    .runs
                    .load_staged_lorebook_writer_run(writer.run.request_id)?;
                if run.project_request_id != batch.project.request_id
                    || run.project_id != batch.project.project.id
                    || run.refinement.is_some()
                {
                    return Err(StagedLorebookWriterDispatchError::InvalidWork);
                }
                let Some(work) =
                    self.claim(writer.run.request_id, worker_id, now, lease_for, allowed)?
                else {
                    return Ok(None);
                };
                let result = executor
                    .run(work.run.request_id, prompt, &work.handle, None, now)
                    .await;
                self.settle(work, result, CancellationReason::User, now)
                    .map(Some)
            });
        }
        let mut outcomes = Vec::with_capacity(batch.writers.len());
        while let Some(outcome) = pending.next().await {
            outcomes.push(outcome);
        }
        outcomes
    }

    pub fn settle(
        &self,
        work: StagedLorebookWriterClaimedWork,
        result: Result<StagedLorebookWriterExecutionResult, StagedLorebookWriterExecutionError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<StagedLorebookWriterSettledWork, StagedLorebookWriterDispatchError> {
        if work.run.job_id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.kind != JobKind::CreationRun
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::Request(work.run.request_id)
        {
            return Err(StagedLorebookWriterDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        let result = if self
            .projects
            .load_staged_lorebook(work.run.project_request_id)?
            .project
            .stage
            == lettuce_creation::StagedLorebookStage::Cancelled
        {
            Err(StagedLorebookWriterExecutionError::Cancelled)
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
                Ok(StagedLorebookWriterSettledWork::Succeeded {
                    result: Box::new(result),
                    job,
                })
            }
            Err(error) if matches!(error, StagedLorebookWriterExecutionError::Cancelled) => {
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
                Ok(StagedLorebookWriterSettledWork::Cancelled { error, job })
            }
            Err(error) if retryable(&error) => {
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::RetryScheduled {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(StagedLorebookWriterSettledWork::RetryScheduled { error, job })
            }
            Err(error) => {
                if work.run.refinement.is_none() {
                    match self.projects.fail_staged_lorebook_draft(
                        work.run.project_request_id,
                        work.run.plan_id,
                        work.run.project_revision,
                        at,
                    ) {
                        Ok(_) | Err(StagedLorebookRepositoryError::Conflict) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(&error),
                    at,
                })?;
                Ok(StagedLorebookWriterSettledWork::Failed { error, job })
            }
        }
    }
}

fn retryable(error: &StagedLorebookWriterExecutionError) -> bool {
    match error {
        StagedLorebookWriterExecutionError::Inference(PortError::Unavailable) => true,
        StagedLorebookWriterExecutionError::Inference(PortError::Provider(failure)) => {
            failure.kind == ProviderFailureKind::Unavailable
        }
        StagedLorebookWriterExecutionError::ReplayCleanup
        | StagedLorebookWriterExecutionError::Run(
            StagedLorebookWriterRunRepositoryError::Failure,
        )
        | StagedLorebookWriterExecutionError::Project(
            lettuce_creation::StagedLorebookRepositoryError::Failure,
        ) => true,
        _ => false,
    }
}

fn job_error(error: &StagedLorebookWriterExecutionError) -> JobError {
    let (code, message) = match error {
        StagedLorebookWriterExecutionError::InvalidOwnership
        | StagedLorebookWriterExecutionError::InvalidPrompt
        | StagedLorebookWriterExecutionError::ContextTooLarge => (
            JobErrorCode::InvalidInput,
            "staged-lorebook-writer-invalid-input",
        ),
        StagedLorebookWriterExecutionError::Inference(_)
        | StagedLorebookWriterExecutionError::InvalidResponse => (
            JobErrorCode::WorkerFailed,
            "staged-lorebook-writer-provider-rejected",
        ),
        StagedLorebookWriterExecutionError::Prompt(_)
        | StagedLorebookWriterExecutionError::ReplayCleanup
        | StagedLorebookWriterExecutionError::Run(_)
        | StagedLorebookWriterExecutionError::Project(_) => (
            JobErrorCode::StorageFailure,
            "staged-lorebook-writer-storage-failed",
        ),
        StagedLorebookWriterExecutionError::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, false, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_transient_writer_failures_retry() {
        assert!(retryable(&StagedLorebookWriterExecutionError::Inference(
            PortError::Unavailable
        )));
        assert!(!retryable(
            &StagedLorebookWriterExecutionError::InvalidResponse
        ));
    }
}
