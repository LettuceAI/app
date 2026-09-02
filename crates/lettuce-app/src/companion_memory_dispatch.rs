use std::time::Duration;

use lettuce_companions::CompanionTurnEffectRepository;
use lettuce_jobs::{
    Claim, JobMutation, JobSnapshot, JobStore, ResourceAvailability, StoreError, WorkerId,
    handle::JobHandle,
};
use lettuce_types::TimestampMillis;

use crate::{
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

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryDispatchError {
    #[error("post-turn memory discovery failed: {0}")]
    Admission(#[from] CompanionPostTurnMemoryAdmissionError),
    #[error("post-turn memory job claim failed: {0}")]
    Jobs(#[from] StoreError),
}

#[derive(Debug)]
pub struct CompanionMemoryDispatchCoordinator<'a, R: ?Sized, J: ?Sized> {
    effects: &'a R,
    jobs: &'a J,
}

impl<'a, R, J> CompanionMemoryDispatchCoordinator<'a, R, J>
where
    R: CompanionTurnEffectRepository + ?Sized,
    J: JobStore + ?Sized,
{
    #[must_use]
    pub const fn new(effects: &'a R, jobs: &'a J) -> Self {
        Self { effects, jobs }
    }

    /// Shared startup and post-finalization entry point. The host supplies the
    /// worker identity and immediately dispatches each returned item through
    /// `CompanionMemoryJobRunner` with its resolved runtime inputs.
    pub fn discover_and_claim(
        &self,
        limit: u16,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Vec<CompanionMemoryClaimedWork>, CompanionMemoryDispatchError> {
        let admissions = CompanionPostTurnMemoryAdmissionCoordinator::new(self.effects, self.jobs)
            .discover_and_admit(limit)?;
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
            let job = self.jobs.append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
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
}
