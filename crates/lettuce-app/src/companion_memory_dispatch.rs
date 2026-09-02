use std::time::Duration;

use lettuce_companions::CompanionTurnEffectRepository;
use lettuce_jobs::{
    CancellationReason, Claim, FiniteFraction, JobError, JobErrorCode, JobMutation, JobOutcome,
    JobSnapshot, JobState, JobStore, OutcomeRef, ProgressSnapshot, ResourceAvailability,
    StageSnapshot, StoreError, WorkerId, handle::JobHandle,
};
use lettuce_memory::{DynamicMemoryApprovalRepository, DynamicMemoryRunMode};
use lettuce_types::TimestampMillis;

use crate::{
    CompanionMemoryJobRunError, CompanionMemoryJobRunResult, CompanionMemoryTerminalFailure,
    CompanionPostTurnMemoryAdmission, CompanionPostTurnMemoryAdmissionCoordinator,
    CompanionPostTurnMemoryAdmissionError,
};

#[derive(Debug, Clone)]
pub struct CompanionMemoryClaimedWork {
    pub admission: CompanionPostTurnMemoryAdmission,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum CompanionMemorySettledWork {
    Succeeded {
        result: Box<CompanionMemoryJobRunResult>,
        job: JobSnapshot,
    },
    Failed {
        error: CompanionMemoryJobRunError,
        job: JobSnapshot,
    },
    Cancelled {
        error: CompanionMemoryJobRunError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: CompanionMemoryJobRunError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryDispatchError {
    #[error("post-turn memory discovery failed: {0}")]
    Admission(#[from] CompanionPostTurnMemoryAdmissionError),
    #[error("post-turn memory job claim failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("post-turn memory claimed work is inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct CompanionMemoryDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    effects: &'a R,
    jobs: &'a J,
}

impl<'a, R, J> CompanionMemoryDispatchCoordinator<'a, R, J>
where
    R: CompanionTurnEffectRepository + DynamicMemoryApprovalRepository + ?Sized,
    J: JobStore + ?Sized,
{
    #[must_use]
    pub const fn new(effects: &'a R, jobs: &'a J) -> Self {
        Self { effects, jobs }
    }

    /// Shared startup and post-finalization entry point. The host supplies the
    /// worker identity and immediately dispatches each returned item through
    /// `CompanionMemoryJobRunner` with its resolved runtime inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn discover_and_claim(
        &self,
        limit: u16,
        summary_message_interval: u32,
        run_mode: DynamicMemoryRunMode,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Vec<CompanionMemoryClaimedWork>, CompanionMemoryDispatchError> {
        let admissions = CompanionPostTurnMemoryAdmissionCoordinator::new(self.effects, self.jobs)
            .discover_and_admit(limit, summary_message_interval, run_mode, now)?;
        self.claim_admissions(admissions, worker_id, now, lease_for, allowed)
    }

    pub fn skip_pending_approval(
        &self,
        conversation_id: lettuce_types::ConversationId,
        now: TimestampMillis,
    ) -> Result<Option<lettuce_memory::DynamicMemoryPendingApproval>, CompanionMemoryDispatchError>
    {
        Ok(
            CompanionPostTurnMemoryAdmissionCoordinator::new(self.effects, self.jobs)
                .skip_pending_approval(conversation_id, now)?,
        )
    }

    pub fn pending_approval_count(
        &self,
        conversation_id: lettuce_types::ConversationId,
    ) -> Result<Option<u64>, CompanionMemoryDispatchError> {
        Ok(
            CompanionPostTurnMemoryAdmissionCoordinator::new(self.effects, self.jobs)
                .pending_approval_count(conversation_id)?,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn approve_and_claim(
        &self,
        conversation_id: lettuce_types::ConversationId,
        limit: u16,
        summary_message_interval: u32,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Vec<CompanionMemoryClaimedWork>, CompanionMemoryDispatchError> {
        let admission = CompanionPostTurnMemoryAdmissionCoordinator::new(self.effects, self.jobs)
            .approve_and_admit(conversation_id, limit, summary_message_interval)?;
        self.claim_admissions(
            admission.into_iter().collect(),
            worker_id,
            now,
            lease_for,
            allowed,
        )
    }

    fn claim_admissions(
        &self,
        admissions: Vec<CompanionPostTurnMemoryAdmission>,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Vec<CompanionMemoryClaimedWork>, CompanionMemoryDispatchError> {
        let mut work = Vec::with_capacity(admissions.len());
        for admission in admissions {
            let claim_at = now.max(admission.job.updated_at);
            let Some(claim) =
                self.jobs
                    .claim(admission.job.id, worker_id, claim_at, lease_for, allowed)?
            else {
                continue;
            };
            let handle = JobHandle::new(claim.claim.job_id);
            self.jobs.append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: claim_at,
            })?;
            let job = self.jobs.append_and_transition(JobMutation::StageChanged {
                claim: claim.claim.clone(),
                stage: StageSnapshot::new("memory-extraction", false)
                    .expect("constant job stage is valid"),
                at: claim_at,
            })?;
            work.push(CompanionMemoryClaimedWork {
                admission,
                claim,
                handle,
                job,
            });
        }
        Ok(work)
    }

    pub fn settle_run(
        &self,
        work: CompanionMemoryClaimedWork,
        result: Result<CompanionMemoryJobRunResult, CompanionMemoryJobRunError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<CompanionMemorySettledWork, CompanionMemoryDispatchError> {
        if work.admission.job.id != work.claim.claim.job_id
            || work.handle.id() != work.claim.claim.job_id
            || work.job.id != work.claim.claim.job_id
            || work.job.state != JobState::Running
        {
            return Err(CompanionMemoryDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        match result {
            Ok(result) => {
                if result.dispatch.run.conversation_id != work.admission.batch.conversation_id
                    || result.dispatch.attempt.job_id != work.claim.claim.job_id
                {
                    return Err(CompanionMemoryDispatchError::InvalidWork);
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
                        result_ref: OutcomeRef::Conversation(work.admission.batch.conversation_id),
                    },
                    at,
                })?;
                Ok(CompanionMemorySettledWork::Succeeded {
                    result: Box::new(result),
                    job,
                })
            }
            Err(error) => match error.terminal_failure() {
                Some(CompanionMemoryTerminalFailure::Cancelled) => {
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
                    Ok(CompanionMemorySettledWork::Cancelled { error, job })
                }
                Some(failure) => {
                    let job = self.jobs.append_and_transition(JobMutation::Fail {
                        claim: work.claim.claim,
                        error: job_error(failure),
                        at,
                    })?;
                    Ok(CompanionMemorySettledWork::Failed { error, job })
                }
                None => {
                    let job = self
                        .jobs
                        .append_and_transition(JobMutation::RetryScheduled {
                            claim: work.claim.claim,
                            at,
                        })?;
                    Ok(CompanionMemorySettledWork::RetryScheduled { error, job })
                }
            },
        }
    }
}

fn job_error(failure: CompanionMemoryTerminalFailure) -> JobError {
    let (code, retryable, message) = match failure {
        CompanionMemoryTerminalFailure::ProviderUnavailable => (
            JobErrorCode::ResourceUnavailable,
            true,
            "companion-memory-provider-unavailable",
        ),
        CompanionMemoryTerminalFailure::ProviderRejected => (
            JobErrorCode::WorkerFailed,
            false,
            "companion-memory-provider-rejected",
        ),
        CompanionMemoryTerminalFailure::EmptyResponse => (
            JobErrorCode::WorkerFailed,
            false,
            "companion-memory-empty-response",
        ),
        CompanionMemoryTerminalFailure::RoundLimit => (
            JobErrorCode::WorkerFailed,
            false,
            "companion-memory-round-limit",
        ),
        CompanionMemoryTerminalFailure::Tool => (
            JobErrorCode::WorkerFailed,
            false,
            "companion-memory-tool-failed",
        ),
        CompanionMemoryTerminalFailure::Recovery => (
            JobErrorCode::StorageFailure,
            true,
            "companion-memory-recovery-failed",
        ),
        CompanionMemoryTerminalFailure::Cancelled => {
            unreachable!("cancellation settles separately")
        }
    };
    JobError::new(code, retryable, message).expect("constant job error is valid")
}
