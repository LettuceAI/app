use std::time::Duration;

use lettuce_companions::{
    CompanionConsolidationRunRepository, CompanionGrowthRunRepository, SoulRepository,
};
use lettuce_context::PromptRepository;
use lettuce_conversations::{ConversationReader, InferencePort, ProviderReplayArtifactPort};
use lettuce_jobs::{CancellationReason, Clock, JobKind, JobStore, ResourceAvailability, WorkerId};
use lettuce_types::JobId;

use crate::{
    BuiltInPromptId, CompanionConsolidationDispatchCoordinator,
    CompanionConsolidationExecutionCoordinator, CompanionConsolidationJobAdmissionCoordinator,
    CompanionConsolidationSettledWork, CompanionGrowthDispatchCoordinator,
    CompanionGrowthExecutionCoordinator, CompanionGrowthJobAdmissionCoordinator,
    CompanionGrowthSettledWork, CompanionMemorySettledWork,
};

/// What runs after a post-turn memory cycle settles.
#[async_trait::async_trait]
pub trait CompanionFollowUps: Send + Sync {
    async fn after_memory(
        &self,
        settled: &CompanionMemorySettledWork,
        worker_id: WorkerId,
        lease_for: Duration,
        clock: &dyn Clock,
    );
}

pub trait CompanionFollowUpSources:
    ConversationReader
    + lettuce_characters::CharacterRepository
    + SoulRepository
    + CompanionGrowthRunRepository
    + CompanionConsolidationRunRepository
    + ProviderReplayArtifactPort
    + lettuce_usage::JobUsageLedger
    + crate::generation::runtime_text::RuntimeTextSource
    + PromptRepository
    + lettuce_models::ModelProfileRepository
    + lettuce_models::ProviderAccountRepository
    + lettuce_models::GlobalModelSettingsRepository
    + JobStore
{
}

impl<T> CompanionFollowUpSources for T where
    T: ConversationReader
        + lettuce_characters::CharacterRepository
        + SoulRepository
        + CompanionGrowthRunRepository
        + CompanionConsolidationRunRepository
        + ProviderReplayArtifactPort
        + lettuce_usage::JobUsageLedger
        + crate::generation::runtime_text::RuntimeTextSource
        + PromptRepository
        + lettuce_models::ModelProfileRepository
        + lettuce_models::ProviderAccountRepository
        + lettuce_models::GlobalModelSettingsRepository
        + JobStore
{
}

/// Legacy's growth cycle after a successful companion memory cycle, and the
/// consolidation it triggered when growth applied changes.
#[derive(Debug)]
pub struct CompanionFollowUpHost<'a, R: ?Sized, I: ?Sized> {
    sources: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> CompanionFollowUpHost<'a, R, I> {
    #[must_use]
    pub const fn new(sources: &'a R, inference: &'a I) -> Self {
        Self { sources, inference }
    }
}

impl<R, I> CompanionFollowUpHost<'_, R, I>
where
    R: CompanionFollowUpSources + Sync + ?Sized,
    I: InferencePort + Sync + ?Sized,
{
    /// Must run before any pass: runs each queued growth and consolidation
    /// job, failing one the app stopped during too often.
    pub async fn resume_after_restart(
        &self,
        worker_id: WorkerId,
        lease_for: Duration,
        clock: &dyn Clock,
    ) {
        let (Some(growth), Some(consolidation)) = (
            self.prompt(BuiltInPromptId::CompanionGrowthcycle),
            self.prompt(BuiltInPromptId::CompanionConsolidation),
        ) else {
            return;
        };
        for kind in [JobKind::CompanionGrowth, JobKind::CompanionConsolidation] {
            for job in self.queued(kind) {
                match crate::jobs::job_recovery::interrupted_runs(self.sources, job.id) {
                    Ok(interrupted)
                        if interrupted >= crate::jobs::job_recovery::MAX_INTERRUPTED_RUNS =>
                    {
                        tracing::warn!(job_id = %job.id, interrupted, "companion job stopped the app too often; not running it again");
                        if let Err(error) = crate::jobs::job_recovery::fail_interrupted_job(
                            self.sources,
                            &job,
                            clock.now(),
                        ) {
                            tracing::warn!(job_id = %job.id, %error, "could not fail an interrupted companion job");
                        }
                    }
                    Ok(_) if kind == JobKind::CompanionGrowth => {
                        self.run_growth(job.id, &growth, worker_id, lease_for, clock)
                            .await;
                    }
                    Ok(_) => {
                        self.run_consolidation(job.id, &consolidation, worker_id, lease_for, clock)
                            .await;
                    }
                    Err(error) => {
                        tracing::warn!(job_id = %job.id, %error, "could not read a companion job's history");
                    }
                }
            }
        }
    }

    async fn run_growth(
        &self,
        job_id: JobId,
        prompt: &lettuce_context::PromptDocument,
        worker_id: WorkerId,
        lease_for: Duration,
        clock: &dyn Clock,
    ) {
        let dispatcher = CompanionGrowthDispatchCoordinator::new(self.sources, self.sources);
        let work = match dispatcher.claim(
            job_id,
            worker_id,
            clock.now(),
            lease_for,
            &ResourceAvailability::all(),
        ) {
            Ok(Some(work)) => work,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%job_id, %error, "growth job cannot run; cancelling it");
                self.cancel(job_id, clock);
                return;
            }
        };
        let result = CompanionGrowthExecutionCoordinator::new(self.sources, self.inference)
            .run(job_id, prompt, &work.handle, None, clock.now())
            .await;
        let settled = match dispatcher.settle(work, result, CancellationReason::User, clock.now()) {
            Ok(settled) => settled,
            Err(error) => {
                tracing::warn!(%job_id, %error, "could not settle a growth job");
                return;
            }
        };
        let (result, job) = match settled {
            CompanionGrowthSettledWork::Succeeded { result, job } => (result, job),
            CompanionGrowthSettledWork::RetryScheduled { error, .. } => {
                tracing::warn!(%job_id, %error, "growth failed; not retrying it");
                self.cancel(job_id, clock);
                return;
            }
            _ => return,
        };
        let Some(prompt) = self.prompt(BuiltInPromptId::CompanionConsolidation) else {
            return;
        };
        match CompanionConsolidationJobAdmissionCoordinator::new(self.sources, self.sources)
            .admit_after_growth(job.id, &result)
        {
            Ok(Some(admission)) => {
                self.run_consolidation(admission.job.id, &prompt, worker_id, lease_for, clock)
                    .await;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%job_id, %error, "could not admit consolidation after growth");
            }
        }
    }

    async fn run_consolidation(
        &self,
        job_id: JobId,
        prompt: &lettuce_context::PromptDocument,
        worker_id: WorkerId,
        lease_for: Duration,
        clock: &dyn Clock,
    ) {
        let dispatcher = CompanionConsolidationDispatchCoordinator::new(self.sources, self.sources);
        let work = match dispatcher.claim(
            job_id,
            worker_id,
            clock.now(),
            lease_for,
            &ResourceAvailability::all(),
        ) {
            Ok(Some(work)) => work,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%job_id, %error, "consolidation job cannot run; cancelling it");
                self.cancel(job_id, clock);
                return;
            }
        };
        let result = CompanionConsolidationExecutionCoordinator::new(self.sources, self.inference)
            .run(job_id, prompt, &work.handle, None, clock.now())
            .await;
        match dispatcher.settle(work, result, CancellationReason::User, clock.now()) {
            Ok(CompanionConsolidationSettledWork::Succeeded { .. }) => {}
            Ok(CompanionConsolidationSettledWork::RetryScheduled { error, .. }) => {
                tracing::warn!(%job_id, %error, "consolidation failed; not retrying it");
                self.cancel(job_id, clock);
            }
            Ok(_) => tracing::info!(%job_id, "consolidation did not succeed"),
            Err(error) => tracing::warn!(%job_id, %error, "could not settle a consolidation job"),
        }
    }

    fn cancel(&self, job_id: JobId, clock: &dyn Clock) {
        let cancelled = JobStore::get(self.sources, job_id).and_then(|job| {
            let Some(job) = job.filter(|job| job.state == lettuce_jobs::JobState::Queued) else {
                return Ok(());
            };
            let at = clock.now().max(job.updated_at);
            let requested = JobStore::append_and_transition(
                self.sources,
                lettuce_jobs::JobMutation::RequestCancellation {
                    id: job_id,
                    reason: CancellationReason::Recovery,
                    at,
                },
            )?;
            JobStore::append_and_transition(
                self.sources,
                lettuce_jobs::JobMutation::FinishQueuedCancellation {
                    id: job_id,
                    at: at.max(requested.updated_at),
                },
            )
            .map(|_| ())
        });
        if let Err(error) = cancelled {
            tracing::warn!(%job_id, %error, "could not cancel a companion follow-up job");
        }
    }

    fn prompt(&self, id: BuiltInPromptId) -> Option<lettuce_context::PromptDocument> {
        match crate::generation::built_in_prompts::active_built_in_prompt(self.sources, id) {
            Ok(Some(prompt)) => Some(prompt),
            Ok(None) => {
                tracing::warn!(?id, "companion follow-up prompt is missing");
                None
            }
            Err(error) => {
                tracing::warn!(?id, %error, "companion follow-up prompt could not be read");
                None
            }
        }
    }

    fn queued(&self, kind: JobKind) -> Vec<lettuce_jobs::JobSnapshot> {
        let mut jobs = Vec::new();
        let mut cursor = None;
        loop {
            match JobStore::list(
                self.sources,
                lettuce_jobs::JobQuery {
                    state: Some(lettuce_jobs::JobState::Queued),
                    kind: Some(kind),
                    subject: None,
                    page: lettuce_types::PageRequest {
                        cursor: cursor.take(),
                        limit: lettuce_types::PageLimit::new(200),
                    },
                },
            ) {
                Ok(page) => {
                    jobs.extend(page.items);
                    match page.next_cursor {
                        Some(next) => cursor = Some(next),
                        None => return jobs,
                    }
                }
                Err(error) => {
                    tracing::warn!(?kind, %error, "could not list queued companion jobs");
                    return jobs;
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl<R, I> CompanionFollowUps for CompanionFollowUpHost<'_, R, I>
where
    R: CompanionFollowUpSources + Sync + ?Sized,
    I: InferencePort + Sync + ?Sized,
{
    async fn after_memory(
        &self,
        settled: &CompanionMemorySettledWork,
        worker_id: WorkerId,
        lease_for: Duration,
        clock: &dyn Clock,
    ) {
        let CompanionMemorySettledWork::Succeeded { result, .. } = settled else {
            return;
        };
        if result.fresh_memories.is_empty() {
            return;
        }
        let Some(prompt) = self.prompt(BuiltInPromptId::CompanionGrowthcycle) else {
            return;
        };
        match CompanionGrowthJobAdmissionCoordinator::new(self.sources, self.sources)
            .admit_after_memory(result)
        {
            Ok(Some(admission)) => {
                self.run_growth(admission.job.id, &prompt, worker_id, lease_for, clock)
                    .await;
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not admit growth after memory"),
        }
    }
}
