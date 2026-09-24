use lettuce_jobs::{
    ExpiredClaim, JobSnapshot, JobState, JobStore, StoreError, Timestamp, recovery::RecoveryAction,
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
        let expired = self.jobs.orphaned_claims(now, limit)?;
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

/// Runs of one job the app may stop during before it no longer runs on its
/// own.
pub(crate) const MAX_INTERRUPTED_RUNS: usize = 2;

const APP_STOPPED_DURING_JOB: &str = "app-stopped-during-job";

/// How many times a claim on the job ended without its worker settling it.
pub(crate) fn interrupted_runs<J: JobStore + ?Sized>(
    jobs: &J,
    job_id: lettuce_types::JobId,
) -> Result<usize, StoreError> {
    let mut count = 0;
    let mut after = None;
    loop {
        let events = jobs.events_since(job_id, after, 500)?;
        let Some(last) = events.last() else {
            return Ok(count);
        };
        after = Some(last.seq);
        count += events
            .iter()
            .filter(|event| {
                matches!(
                    event.event,
                    lettuce_jobs::events::JobEvent::LeaseExpired { .. }
                )
            })
            .count();
    }
}

/// Fails a queued job the app stopped during too often.
pub(crate) fn fail_interrupted_job<J: JobStore + ?Sized>(
    jobs: &J,
    job: &JobSnapshot,
    now: Timestamp,
) -> Result<JobSnapshot, StoreError> {
    let at = now.max(job.updated_at);
    let claim = jobs
        .claim(
            job.id,
            lettuce_jobs::WorkerId::new(),
            at,
            std::time::Duration::from_secs(60),
            &lettuce_jobs::ResourceAvailability::all(),
        )?
        .ok_or(StoreError::IllegalTransition)?;
    jobs.append_and_transition(lettuce_jobs::JobMutation::Start {
        claim: claim.claim.clone(),
        at,
    })?;
    jobs.append_and_transition(lettuce_jobs::JobMutation::Fail {
        claim: claim.claim,
        error: lettuce_jobs::JobError::new(
            lettuce_jobs::JobErrorCode::LeaseLost,
            false,
            APP_STOPPED_DURING_JOB,
        )
        .expect("constant job error is valid"),
        at,
    })
}

/// Whether the job ended because the app stopped during it too often.
pub(crate) fn failed_as_interrupted(job: &JobSnapshot) -> bool {
    job.state == JobState::Failed
        && job.error.as_ref().is_some_and(|error| {
            error.code == lettuce_jobs::JobErrorCode::LeaseLost && !error.retryable
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StartupRecoveryReport {
    pub jobs: Vec<RecoveredStartupJob>,
    pub cancelled_generation_jobs: Vec<lettuce_types::JobId>,
    pub settled_requested_jobs: Vec<lettuce_types::JobId>,
    pub turns: Vec<(
        lettuce_types::GenerationTurnId,
        crate::ConversationGenerationRestartSettlement,
    )>,
}

const STARTUP_RECOVERY_PAGE: u32 = 200;

const STAGED_LOREBOOK_WRITER_KEYS: [&str; 2] =
    ["staged-lorebook-writer-", "staged-lorebook-refine-"];
const STAGED_LOREBOOK_COHERENCE_KEY: &str = "staged-lorebook-coherence-";
const STAGED_LOREBOOK_PLANNER_KEYS: [&str; 2] = ["staged-lorebook-", "staged-planner-retry-"];

impl crate::AppBackend {
    /// Work a user asked for is not run again after a restart: a staged
    /// lorebook planner or writer without a saved attempt fails through its
    /// own settlement so the project can be retried (one with a saved attempt
    /// stays queued to finish from it), other creation, speech and image jobs
    /// are cancelled, and ended image generations settle their records.
    fn settle_requested_work_after_restart(
        &self,
        now: Timestamp,
        report: &mut StartupRecoveryReport,
    ) {
        use lettuce_jobs::JobKind;
        for kind in [
            JobKind::CreationRun,
            JobKind::SpeechTranscribe,
            JobKind::SpeechSynthesize,
            JobKind::ImageGenerate,
        ] {
            for job in self.pending_jobs(kind) {
                let settled = if kind == JobKind::CreationRun {
                    self.settle_creation_job(&job, now)
                } else {
                    self.cancel_waiting_job(&job, now)
                };
                match settled {
                    Ok(true) => report.settled_requested_jobs.push(job.id),
                    Ok(false) => {}
                    Err(error) => {
                        tracing::warn!(job_id = %job.id, %error, "startup could not settle requested work");
                    }
                }
            }
        }
        let images = self.image_generations();
        let mut interrupted = Vec::new();
        for state in [JobState::Interrupted, JobState::Cancelled] {
            let mut cursor = None;
            loop {
                let Ok(page) = self.database().list(lettuce_jobs::JobQuery {
                    state: Some(state),
                    kind: Some(JobKind::ImageGenerate),
                    subject: None,
                    page: lettuce_types::PageRequest {
                        cursor: cursor.take(),
                        limit: lettuce_types::PageLimit::new(STARTUP_RECOVERY_PAGE as u16),
                    },
                }) else {
                    tracing::warn!("startup could not list ended image generations");
                    break;
                };
                interrupted.extend(page.items.into_iter().map(|job| job.id));
                match page.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
        }
        for id in interrupted {
            if let Err(error) = images.reconcile_after_restart(id) {
                tracing::warn!(job_id = %id, %error, "startup could not settle an image generation");
            }
        }
    }

    fn settle_creation_job(&self, job: &JobSnapshot, now: Timestamp) -> Result<bool, String> {
        use lettuce_creation::{StagedLorebookRepository, StagedLorebookWriterRunRepository};
        let key = job
            .idempotency_key
            .as_ref()
            .map_or("", lettuce_jobs::IdempotencyKey::as_str);
        let writer = STAGED_LOREBOOK_WRITER_KEYS
            .iter()
            .any(|prefix| key.starts_with(prefix));
        let planner = !writer
            && !key.starts_with(STAGED_LOREBOOK_COHERENCE_KEY)
            && STAGED_LOREBOOK_PLANNER_KEYS
                .iter()
                .any(|prefix| key.starts_with(prefix));
        if (!writer && !planner) || job.state != JobState::Queued {
            return self.cancel_waiting_job(job, now);
        }
        let Some(request_id) = self.request_input(job.id) else {
            return self.cancel_waiting_job(job, now);
        };
        let worker_id = lettuce_jobs::WorkerId::new();
        let lease = std::time::Duration::from_secs(60);
        let allowed = lettuce_jobs::ResourceAvailability::all();
        if writer {
            let run = self
                .database()
                .load_staged_lorebook_writer_run(request_id)
                .map_err(|error| error.to_string())?;
            if run.attempt.is_some() {
                return Ok(false);
            }
            if run.refinement.is_some() {
                return self.cancel_waiting_job(job, now);
            }
            let dispatcher = self.staged_lorebook_writer_dispatcher();
            let Some(work) = dispatcher
                .claim(request_id, worker_id, now, lease, &allowed)
                .map_err(|error| error.to_string())?
            else {
                return Ok(false);
            };
            let claim = work.claim.claim.clone();
            if let Err(error) = dispatcher.settle(
                work,
                Err(crate::StagedLorebookWriterExecutionError::AppStopped),
                lettuce_jobs::CancellationReason::Recovery,
                now,
            ) {
                self.fail_claimed_job(claim, now)?;
                return Err(error.to_string());
            }
        } else {
            let run = self
                .database()
                .load_staged_lorebook(request_id)
                .map_err(|error| error.to_string())?;
            if run.project.stage != lettuce_creation::StagedLorebookStage::Planning
                || run.planner_attempt.is_some()
            {
                return Ok(false);
            }
            let dispatcher = self.staged_lorebook_planner_dispatcher();
            let Some(work) = dispatcher
                .claim(request_id, worker_id, now, lease, &allowed)
                .map_err(|error| error.to_string())?
            else {
                return Ok(false);
            };
            let claim = work.claim.claim.clone();
            if let Err(error) = dispatcher.settle(
                work,
                Err(crate::StagedLorebookPlannerExecutionError::AppStopped),
                lettuce_jobs::CancellationReason::Recovery,
                now,
            ) {
                self.fail_claimed_job(claim, now)?;
                return Err(error.to_string());
            }
        }
        Ok(true)
    }

    fn fail_claimed_job(
        &self,
        claim: lettuce_jobs::ClaimRef,
        now: Timestamp,
    ) -> Result<(), String> {
        let at = self
            .database()
            .get(claim.job_id)
            .ok()
            .flatten()
            .map_or(now, |job| now.max(job.updated_at));
        self.database()
            .append_and_transition(lettuce_jobs::JobMutation::Fail {
                claim,
                error: lettuce_jobs::JobError::new(
                    lettuce_jobs::JobErrorCode::LeaseLost,
                    false,
                    APP_STOPPED_DURING_JOB,
                )
                .expect("constant job error is valid"),
                at,
            })
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn cancel_waiting_job(&self, job: &JobSnapshot, now: Timestamp) -> Result<bool, String> {
        let at = now.max(job.updated_at);
        let job = match job.state {
            JobState::Queued => self
                .database()
                .append_and_transition(lettuce_jobs::JobMutation::RequestCancellation {
                    id: job.id,
                    reason: lettuce_jobs::CancellationReason::Recovery,
                    at,
                })
                .map_err(|error| error.to_string())?,
            _ => job.clone(),
        };
        if job.state != JobState::CancellationRequested || job.claim.is_some() {
            return Ok(false);
        }
        self.database()
            .append_and_transition(lettuce_jobs::JobMutation::FinishQueuedCancellation {
                id: job.id,
                at: at.max(job.updated_at),
            })
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    fn request_input(&self, job_id: lettuce_types::JobId) -> Option<lettuce_types::RequestId> {
        match self
            .database()
            .events_since(job_id, None, 1)
            .ok()?
            .first()
            .map(|event| &event.event)
        {
            Some(lettuce_jobs::events::JobEvent::Created {
                input_ref: lettuce_jobs::OutcomeRef::Request(request_id),
                ..
            }) => Some(*request_id),
            _ => None,
        }
    }

    fn pending_jobs(&self, kind: lettuce_jobs::JobKind) -> Vec<JobSnapshot> {
        let mut jobs = Vec::new();
        for state in [JobState::Queued, JobState::CancellationRequested] {
            let mut cursor = None;
            loop {
                let Ok(page) = self.database().list(lettuce_jobs::JobQuery {
                    state: Some(state),
                    kind: Some(kind),
                    subject: None,
                    page: lettuce_types::PageRequest {
                        cursor: cursor.take(),
                        limit: lettuce_types::PageLimit::new(STARTUP_RECOVERY_PAGE as u16),
                    },
                }) else {
                    tracing::warn!(?kind, "startup could not list waiting jobs");
                    break;
                };
                jobs.extend(page.items.into_iter().filter(|job| job.claim.is_none()));
                match page.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
        }
        jobs
    }
}

impl crate::AppBackend {
    /// Must run before any worker starts: every claim is released, every
    /// generation job still waiting is cancelled and every live turn settled.
    /// A job or turn that cannot be settled is reported and skipped.
    pub fn recover_after_restart(
        &self,
        now: Timestamp,
    ) -> Result<StartupRecoveryReport, StartupJobRecoveryError> {
        use crate::ConversationGenerationRestartSettlement as Settlement;
        let mut report = StartupRecoveryReport::default();
        loop {
            let page = self
                .startup_job_recovery()
                .recover(now, STARTUP_RECOVERY_PAGE)?;
            let done = page.jobs.len() < STARTUP_RECOVERY_PAGE as usize;
            report.jobs.extend(page.jobs);
            if done {
                break;
            }
        }
        let dispatcher = self.conversation_generation_dispatcher();
        let mut skipped = std::collections::BTreeSet::new();
        for state in [
            lettuce_jobs::JobState::Queued,
            lettuce_jobs::JobState::CancellationRequested,
        ] {
            let mut cursor = None;
            loop {
                let page = self.database().list(lettuce_jobs::JobQuery {
                    state: Some(state),
                    kind: Some(lettuce_jobs::JobKind::ConversationGeneration),
                    subject: None,
                    page: lettuce_types::PageRequest {
                        cursor: cursor.take(),
                        limit: lettuce_types::PageLimit::new(STARTUP_RECOVERY_PAGE as u16),
                    },
                })?;
                let pending = page
                    .items
                    .into_iter()
                    .filter(|job| !skipped.contains(&job.id))
                    .collect::<Vec<_>>();
                if pending.is_empty() {
                    match page.next_cursor {
                        Some(next) => {
                            cursor = Some(next);
                            continue;
                        }
                        None => break,
                    }
                }
                for job in pending {
                    let id = job.id;
                    match dispatcher.cancel_job_after_restart(job, now) {
                        Ok(job) if job.state.is_terminal() => {
                            report.cancelled_generation_jobs.push(id);
                        }
                        Ok(_) => {
                            skipped.insert(id);
                        }
                        Err(error) => {
                            tracing::warn!(job_id = %id, %error, "startup could not cancel a generation job");
                            skipped.insert(id);
                        }
                    }
                }
            }
        }
        self.settle_requested_work_after_restart(now, &mut report);
        let mut unsettled = std::collections::BTreeSet::new();
        loop {
            let limit = STARTUP_RECOVERY_PAGE
                .saturating_add(u32::try_from(unsettled.len()).unwrap_or(u32::MAX));
            let turns =
                match lettuce_conversations::LiveTurnReader::live_turns(self.database(), limit) {
                    Ok(turns) => turns,
                    Err(error) => {
                        tracing::warn!(%error, "startup could not list live generation turns");
                        break;
                    }
                };
            let pending = turns
                .into_iter()
                .filter(|turn_id| !unsettled.contains(turn_id))
                .collect::<Vec<_>>();
            if pending.is_empty() {
                break;
            }
            for turn_id in pending {
                let settlement = lettuce_conversations::ConversationReader::get_turn(
                    self.database(),
                    turn_id,
                )
                .map_err(crate::ConversationGenerationDispatchError::from)
                .and_then(|turn| dispatcher.settle_after_restart(&turn, now))
                .unwrap_or_else(|error| {
                    tracing::warn!(%turn_id, %error, "startup could not settle a generation turn");
                    Settlement::Unsettled
                });
                if settlement == Settlement::Unsettled {
                    unsettled.insert(turn_id);
                }
                report.turns.push((turn_id, settlement));
            }
        }
        Ok(report)
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

    #[test]
    fn requested_work_left_waiting_is_cancelled_on_restart() {
        let backend = crate::AppBackend::open_in_memory(Timestamp::new(1)).expect("backend");
        let queued = |kind, key: &str| {
            let request = lettuce_types::RequestId::new();
            backend
                .job_store()
                .create_or_get(
                    JobSpec::new(
                        kind,
                        JobSubject::new(SubjectKind::Conversation, request.to_string())
                            .expect("subject"),
                        OutcomeRef::Request(request),
                    )
                    .with_resources(vec![ResourceClass::Network])
                    .with_idempotency_key(IdempotencyKey::new(key).expect("key")),
                )
                .expect("queued job")
                .job
                .id
        };
        let reply = queued(JobKind::CreationRun, "reply-helper-restart");
        let transcription = queued(JobKind::SpeechTranscribe, "speech-transcribe-restart");
        let report = backend
            .recover_after_restart(Timestamp::new(10))
            .expect("recover");
        for id in [reply, transcription] {
            assert!(report.settled_requested_jobs.contains(&id));
            assert_eq!(
                backend
                    .job_store()
                    .get(id)
                    .expect("get")
                    .expect("job")
                    .state,
                JobState::Cancelled
            );
        }
        assert!(
            backend
                .recover_after_restart(Timestamp::new(11))
                .expect("recover again")
                .settled_requested_jobs
                .is_empty()
        );
    }
}
