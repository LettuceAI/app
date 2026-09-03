use lettuce_companions::{
    CompanionConsolidationRun, CompanionConsolidationRunRepository,
    CompanionConsolidationRunRepositoryError, CompanionGrowthRunRepository,
    CompanionGrowthRunRepositoryError, SoulOwner, SoulRepository, SoulRepositoryError,
    consolidation_ready,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobState,
    JobStore, JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_types::{JobId, OperationRecordId};
use uuid::Uuid;

use crate::CompanionGrowthExecutionResult;

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionConsolidationJobAdmission {
    pub run: CompanionConsolidationRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionConsolidationJobAdmissionError {
    #[error("companion consolidation source is invalid")]
    InvalidSource,
    #[error("companion consolidation Soul read failed: {0:?}")]
    Soul(SoulRepositoryError),
    #[error("companion consolidation growth read failed: {0}")]
    Growth(CompanionGrowthRunRepositoryError),
    #[error("companion consolidation job admission failed: {0}")]
    Job(StoreError),
    #[error("companion consolidation run persistence failed: {0}")]
    Run(CompanionConsolidationRunRepositoryError),
}

#[derive(Debug)]
pub struct CompanionConsolidationJobAdmissionCoordinator<'a, R: ?Sized, J: ?Sized> {
    sources: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> CompanionConsolidationJobAdmissionCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(sources: &'a R, jobs: &'a J) -> Self {
        Self { sources, jobs }
    }
}

impl<
    R: CompanionGrowthRunRepository + CompanionConsolidationRunRepository + SoulRepository + ?Sized,
    J: JobStore + ?Sized,
> CompanionConsolidationJobAdmissionCoordinator<'_, R, J>
{
    pub fn admit_after_growth(
        &self,
        growth_job_id: JobId,
        result: &CompanionGrowthExecutionResult,
    ) -> Result<Option<CompanionConsolidationJobAdmission>, CompanionConsolidationJobAdmissionError>
    {
        if result.applied_facts == 0 {
            return Ok(None);
        }
        let receipt = result
            .receipt
            .as_ref()
            .ok_or(CompanionConsolidationJobAdmissionError::InvalidSource)?;
        let growth_run = self
            .sources
            .load_companion_growth_run(growth_job_id)
            .map_err(CompanionConsolidationJobAdmissionError::Growth)?;
        if growth_run.job_id != growth_job_id
            || receipt.owner != SoulOwner::Character(growth_run.character_id)
            || receipt.operation_id != growth_run.operation_id
            || receipt.expected_revision != growth_run.soul.revision
        {
            return Err(CompanionConsolidationJobAdmissionError::InvalidSource);
        }
        let growth_job = self
            .jobs
            .get(growth_job_id)
            .map_err(CompanionConsolidationJobAdmissionError::Job)?
            .ok_or(CompanionConsolidationJobAdmissionError::InvalidSource)?;
        if growth_job.kind != JobKind::CompanionGrowth
            || growth_job.state != JobState::Succeeded
            || growth_job.outcome
                != Some(lettuce_jobs::JobOutcome::Success {
                    result_ref: OutcomeRef::Character(growth_run.character_id),
                })
        {
            return Err(CompanionConsolidationJobAdmissionError::InvalidSource);
        }
        match self
            .sources
            .load_companion_consolidation_run_for_growth(growth_job_id)
        {
            Ok(run) => {
                let job = self
                    .jobs
                    .get(run.job_id)
                    .map_err(CompanionConsolidationJobAdmissionError::Job)?
                    .ok_or(CompanionConsolidationJobAdmissionError::InvalidSource)?;
                return Ok(Some(CompanionConsolidationJobAdmission {
                    run,
                    job,
                    created: false,
                }));
            }
            Err(CompanionConsolidationRunRepositoryError::NotFound) => {}
            Err(error) => return Err(CompanionConsolidationJobAdmissionError::Run(error)),
        }
        let soul = SoulRepository::get(self.sources, SoulOwner::Character(growth_run.character_id))
            .map_err(CompanionConsolidationJobAdmissionError::Soul)?
            .ok_or(CompanionConsolidationJobAdmissionError::InvalidSource)?;
        if soul.revision != receipt.resulting_revision
            || !consolidation_ready(&soul, receipt.applied_at)
        {
            return Ok(None);
        }
        let idempotency_key =
            IdempotencyKey::new(format!("companion-consolidation-{growth_job_id}"))
                .map_err(|_| CompanionConsolidationJobAdmissionError::InvalidSource)?;
        let subject = JobSubject::new(
            SubjectKind::Conversation,
            growth_run.conversation_id.to_string(),
        )
        .map_err(|_| CompanionConsolidationJobAdmissionError::InvalidSource)?;
        let admitted = self
            .jobs
            .create_or_get(
                JobSpec::new(
                    JobKind::CompanionConsolidation,
                    subject,
                    OutcomeRef::Character(growth_run.character_id),
                )
                .with_idempotency_key(idempotency_key)
                .with_resources(vec![
                    ResourceClass::Network,
                    ResourceClass::ModelLoad,
                    ResourceClass::DiskRead,
                    ResourceClass::DiskWrite,
                    ResourceClass::Cpu,
                ])
                .with_priority(JobPriority::Background)
                .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative),
            )
            .map_err(CompanionConsolidationJobAdmissionError::Job)?;
        if !admitted.created {
            match self
                .sources
                .load_companion_consolidation_run(admitted.job.id)
            {
                Ok(run) => {
                    return Ok(Some(CompanionConsolidationJobAdmission {
                        run,
                        job: admitted.job,
                        created: false,
                    }));
                }
                Err(CompanionConsolidationRunRepositoryError::NotFound) => {}
                Err(error) => return Err(CompanionConsolidationJobAdmissionError::Run(error)),
            }
        }
        let run = self
            .sources
            .admit_companion_consolidation_run(CompanionConsolidationRun {
                job_id: admitted.job.id,
                growth_job_id,
                conversation_id: growth_run.conversation_id,
                character_id: growth_run.character_id,
                profile: growth_run.profile,
                companion_name: growth_run.companion_name,
                authored_soul: growth_run.authored_soul,
                soul,
                operation_id: OperationRecordId::from_uuid(Uuid::new_v5(
                    &admitted.job.id.as_uuid(),
                    b"companion-consolidation-soul-apply",
                )),
                created_at: receipt.applied_at,
                proposal_checkpoint: None,
            })
            .map_err(CompanionConsolidationJobAdmissionError::Run)?;
        Ok(Some(CompanionConsolidationJobAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        }))
    }
}
