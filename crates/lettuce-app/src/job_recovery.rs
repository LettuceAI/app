use lettuce_jobs::{
    ExpiredClaim, JobSnapshot, JobStore, StoreError, Timestamp, recovery::RecoveryAction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredStartupJob {
    pub expired_claim: ExpiredClaim,
    pub requested_action: RecoveryAction,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StartupJobRecoveryReport {
    pub jobs: Vec<RecoveredStartupJob>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartupJobRecoveryError {
    #[error("startup job recovery failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("a recovered job disappeared from the durable store")]
    MissingRecoveredJob,
}

#[derive(Debug)]
pub struct StartupJobRecoveryCoordinator<'a, J: ?Sized> {
    jobs: &'a J,
}

impl<'a, J: JobStore + ?Sized> StartupJobRecoveryCoordinator<'a, J> {
    #[must_use]
    pub const fn new(jobs: &'a J) -> Self {
        Self { jobs }
    }

    /// Performs one bounded startup pass. Feature executors decide how to
    /// resume, restart, compensate, or surface each returned durable job.
    pub fn recover(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> Result<StartupJobRecoveryReport, StartupJobRecoveryError> {
        let expired = self.jobs.expired_claims(now, limit)?;
        let mut jobs = Vec::with_capacity(expired.len());
        for expired_claim in expired {
            let job = self
                .jobs
                .get(expired_claim.job_id)?
                .ok_or(StartupJobRecoveryError::MissingRecoveredJob)?;
            jobs.push(RecoveredStartupJob {
                requested_action: expired_claim.recovery_action(),
                expired_claim,
                job,
            });
        }
        Ok(StartupJobRecoveryReport { jobs })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use lettuce_database::Database;
    use lettuce_jobs::{
        CancellationReason, IdempotencyKey, JobKind, JobMutation, JobSpec, JobState, JobSubject,
        OutcomeRef, RecoveryPolicy, ResourceAvailability, ResourceClass, SubjectKind, WorkerId,
    };
    use lettuce_types::ConversationId;
    use uuid::Uuid;

    use super::*;

    fn memory_job(key: &str, recovery: RecoveryPolicy) -> JobSpec {
        let conversation_id = ConversationId::new();
        JobSpec::new(
            JobKind::MemoryExtraction,
            JobSubject::new(SubjectKind::Conversation, conversation_id.to_string())
                .expect("subject"),
            OutcomeRef::Conversation(conversation_id),
        )
        .with_resources(vec![
            ResourceClass::Network,
            ResourceClass::ModelLoad,
            ResourceClass::Cpu,
            ResourceClass::DiskWrite,
        ])
        .with_policies(recovery, lettuce_jobs::CancellationPolicy::Cooperative)
        .with_idempotency_key(IdempotencyKey::new(key).expect("key"))
    }

    fn start(database: &Database, spec: JobSpec) -> (JobSpec, lettuce_jobs::ClaimRef, Timestamp) {
        let created = database.create_or_get(spec.clone()).expect("create");
        let claimed_at = Timestamp::new(created.job.updated_at.get() + 1);
        let claim = database
            .claim(
                created.job.id,
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(1),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("eligible");
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        (spec, claim.claim, claimed_at)
    }

    #[test]
    fn file_restart_recovers_and_reclaims_the_same_durable_memory_job() {
        let path = std::env::temp_dir().join(format!("lettuce-startup-{}.sqlite", Uuid::new_v4()));
        let database = Database::open(&path).expect("open database");
        let (spec, stale_claim, claimed_at) = start(
            &database,
            memory_job("memory-restart", RecoveryPolicy::Restart),
        );
        let job_id = stale_claim.job_id;
        drop(database);

        let backend = crate::AppBackend::open(&path, Timestamp::new(claimed_at.get() + 10))
            .expect("reopen backend");
        let report = backend
            .startup_job_recovery()
            .recover(Timestamp::new(claimed_at.get() + 1_001), 20)
            .expect("recover startup jobs");
        assert_eq!(report.jobs.len(), 1);
        assert_eq!(report.jobs[0].job.id, job_id);
        assert_eq!(report.jobs[0].job.kind, JobKind::MemoryExtraction);
        assert_eq!(report.jobs[0].job.state, JobState::Queued);
        assert_eq!(report.jobs[0].requested_action, RecoveryAction::Restart);
        assert_eq!(
            backend
                .job_store()
                .append_and_transition(JobMutation::Interrupt {
                    claim: stale_claim,
                    at: Timestamp::new(claimed_at.get() + 1_002),
                }),
            Err(StoreError::StaleLease)
        );
        let replay = backend
            .job_store()
            .create_or_get(spec)
            .expect("replay admission");
        assert!(!replay.created);
        assert_eq!(replay.job.id, job_id);
        let reclaimed = backend
            .job_store()
            .claim(
                job_id,
                WorkerId::new(),
                Timestamp::new(claimed_at.get() + 1_003),
                Duration::from_secs(1),
                &ResourceAvailability::all(),
            )
            .expect("reclaim")
            .expect("eligible");
        assert_eq!(reclaimed.claim.attempt.get(), 2);
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn reports_unrelated_policy_actions_and_never_requeues_cleanup() {
        let database = Database::open_in_memory().expect("open database");
        let (_, cleanup_claim, cleanup_at) =
            start(&database, memory_job("cleanup", RecoveryPolicy::Restart));
        database
            .append_and_transition(JobMutation::RequestCancellation {
                id: cleanup_claim.job_id,
                reason: CancellationReason::Shutdown,
                at: Timestamp::new(cleanup_at.get() + 2),
            })
            .expect("request cancellation");
        database
            .append_and_transition(JobMutation::RequestCleanup {
                claim: cleanup_claim,
                at: Timestamp::new(cleanup_at.get() + 3),
            })
            .expect("start cleanup");

        let maintenance = JobSpec::new(
            JobKind::Maintenance,
            JobSubject::new(SubjectKind::Maintenance, "projection-repair").expect("subject"),
            OutcomeRef::Checkpoint(lettuce_jobs::OutcomeId::new()),
        )
        .with_resources(vec![ResourceClass::DiskWrite])
        .with_policies(
            RecoveryPolicy::Compensate,
            lettuce_jobs::CancellationPolicy::Cooperative,
        )
        .with_idempotency_key(IdempotencyKey::new("maintenance").expect("key"));
        let (_, _, maintenance_at) = start(&database, maintenance);
        let recovered_at = Timestamp::new(cleanup_at.max(maintenance_at).get() + 1_001);
        let report = StartupJobRecoveryCoordinator::new(&database)
            .recover(recovered_at, 20)
            .expect("recover");
        assert_eq!(report.jobs.len(), 2);
        let cleanup = report
            .jobs
            .iter()
            .find(|job| job.job.kind == JobKind::MemoryExtraction)
            .expect("cleanup job");
        assert_eq!(cleanup.job.state, JobState::Interrupted);
        assert_eq!(cleanup.requested_action, RecoveryAction::Restart);
        let unrelated = report
            .jobs
            .iter()
            .find(|job| job.job.kind == JobKind::Maintenance)
            .expect("unrelated job");
        assert_eq!(unrelated.job.state, JobState::Interrupted);
        assert_eq!(unrelated.requested_action, RecoveryAction::Compensate);
    }
}
