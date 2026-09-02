use std::collections::BTreeMap;

use lettuce_companions::{
    CompanionTurnEffect, CompanionTurnEffectRepository, CompanionTurnEffectRepositoryError,
    CompanionTurnEffectStatus,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobQuery, JobSnapshot, JobSpec,
    JobStore, JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_types::{ConversationId, PageLimit, PageRequest};

use crate::CompanionPostTurnEffect;

pub const MAX_COMPANION_POST_TURN_EFFECTS: u16 = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionPostTurnMemoryBatch {
    pub conversation_id: ConversationId,
    pub idempotency_key: IdempotencyKey,
    pub effects: Vec<CompanionTurnEffect>,
}

impl CompanionPostTurnMemoryBatch {
    #[must_use]
    pub fn terminal_effects(&self) -> Vec<CompanionPostTurnEffect<'_>> {
        self.effects
            .iter()
            .map(|effect| CompanionPostTurnEffect {
                effect,
                enqueued_at: effect.created_at,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionPostTurnMemoryAdmission {
    pub batch: CompanionPostTurnMemoryBatch,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionPostTurnMemoryAdmissionError {
    #[error("companion effect discovery failed: {0:?}")]
    Effects(CompanionTurnEffectRepositoryError),
    #[error("companion effect batch is invalid")]
    InvalidBatch,
    #[error("post-turn memory job admission failed: {0}")]
    Jobs(StoreError),
}

#[derive(Debug)]
pub struct CompanionPostTurnMemoryAdmissionCoordinator<'a, R: ?Sized, J: ?Sized> {
    effects: &'a R,
    jobs: &'a J,
}

impl<'a, R: CompanionTurnEffectRepository + ?Sized, J: JobStore + ?Sized>
    CompanionPostTurnMemoryAdmissionCoordinator<'a, R, J>
{
    #[must_use]
    pub const fn new(effects: &'a R, jobs: &'a J) -> Self {
        Self { effects, jobs }
    }

    /// Rebuilds runtime memory-extraction jobs from the durable processing
    /// effects. Calling this after restart is safe: an unchanged effect batch
    /// produces the same idempotency key, while effects finalized meanwhile
    /// disappear from the next batch.
    pub fn discover_and_admit(
        &self,
        limit: u16,
    ) -> Result<Vec<CompanionPostTurnMemoryAdmission>, CompanionPostTurnMemoryAdmissionError> {
        if limit == 0 || limit > MAX_COMPANION_POST_TURN_EFFECTS {
            return Err(CompanionPostTurnMemoryAdmissionError::InvalidBatch);
        }
        let effects = self
            .effects
            .list_processing(limit)
            .map_err(CompanionPostTurnMemoryAdmissionError::Effects)?;
        let mut by_conversation = BTreeMap::<ConversationId, Vec<CompanionTurnEffect>>::new();
        for effect in effects {
            if effect.status != CompanionTurnEffectStatus::Processing
                || effect.source_window.is_some()
                || effect.summary.is_some()
            {
                return Err(CompanionPostTurnMemoryAdmissionError::InvalidBatch);
            }
            by_conversation
                .entry(effect.conversation_id)
                .or_default()
                .push(effect);
        }

        let mut admissions = Vec::with_capacity(by_conversation.len());
        for (conversation_id, mut effects) in by_conversation {
            effects.sort_by_key(|effect| (effect.created_at, effect.id));
            let idempotency_key = batch_idempotency_key(conversation_id, &effects)?;
            let spec = job_spec(conversation_id, idempotency_key.clone())?;
            let active = self
                .jobs
                .list(JobQuery {
                    state: None,
                    kind: Some(JobKind::MemoryExtraction),
                    subject: Some(spec.subject.id.clone()),
                    page: PageRequest {
                        cursor: None,
                        limit: PageLimit::new(200),
                    },
                })
                .map_err(CompanionPostTurnMemoryAdmissionError::Jobs)?
                .items
                .into_iter()
                .find(|job| !job.is_terminal());
            let admitted = match active {
                Some(job) if job.idempotency_key.as_ref() == Some(&idempotency_key) => {
                    lettuce_jobs::CreateJobResult {
                        job,
                        created: false,
                    }
                }
                Some(_) => continue,
                None => self
                    .jobs
                    .create_or_get(spec)
                    .map_err(CompanionPostTurnMemoryAdmissionError::Jobs)?,
            };
            admissions.push(CompanionPostTurnMemoryAdmission {
                batch: CompanionPostTurnMemoryBatch {
                    conversation_id,
                    idempotency_key,
                    effects,
                },
                job: admitted.job,
                created: admitted.created,
            });
        }
        Ok(admissions)
    }
}

fn batch_idempotency_key(
    conversation_id: ConversationId,
    effects: &[CompanionTurnEffect],
) -> Result<IdempotencyKey, CompanionPostTurnMemoryAdmissionError> {
    if effects.is_empty()
        || effects
            .iter()
            .any(|effect| effect.conversation_id != conversation_id)
    {
        return Err(CompanionPostTurnMemoryAdmissionError::InvalidBatch);
    }
    let mut digest = blake3::Hasher::new();
    digest.update(conversation_id.to_string().as_bytes());
    for effect in effects {
        digest.update(effect.id.to_string().as_bytes());
    }
    IdempotencyKey::new(format!("companion-memory-{}", digest.finalize().to_hex()))
        .map_err(|_| CompanionPostTurnMemoryAdmissionError::InvalidBatch)
}

fn job_spec(
    conversation_id: ConversationId,
    idempotency_key: IdempotencyKey,
) -> Result<JobSpec, CompanionPostTurnMemoryAdmissionError> {
    let subject = JobSubject::new(SubjectKind::Conversation, conversation_id.to_string())
        .map_err(|_| CompanionPostTurnMemoryAdmissionError::InvalidBatch)?;
    Ok(JobSpec::new(
        JobKind::MemoryExtraction,
        subject,
        OutcomeRef::Conversation(conversation_id),
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
    .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative))
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use lettuce_companions::{
        CompanionMemoryChanges, CompanionTurnEffectOutcome, CompanionTurnEffectSeed,
    };
    use lettuce_jobs::{
        CancellationReason, InMemoryJobStore, JobErrorCode, JobState, ResourceAvailability,
        WorkerId,
    };
    use lettuce_types::{CompanionEffectId, GenerationTurnId, MessageId, TimestampMillis};

    use super::*;

    #[derive(Debug, Default)]
    struct Effects(Mutex<Vec<CompanionTurnEffect>>);

    impl Effects {
        fn replace(&self, effects: Vec<CompanionTurnEffect>) {
            *self.0.lock().expect("effects") = effects;
        }
    }

    impl CompanionTurnEffectRepository for Effects {
        fn get_for_message(
            &self,
            conversation_id: ConversationId,
            assistant_message_id: MessageId,
        ) -> Result<Option<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
            Ok(self
                .0
                .lock()
                .expect("effects")
                .iter()
                .find(|effect| {
                    effect.conversation_id == conversation_id
                        && effect.assistant_message_id == assistant_message_id
                })
                .cloned())
        }

        fn list_processing(
            &self,
            limit: u16,
        ) -> Result<Vec<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
            Ok(self
                .0
                .lock()
                .expect("effects")
                .iter()
                .filter(|effect| effect.status == CompanionTurnEffectStatus::Processing)
                .take(usize::from(limit))
                .cloned()
                .collect())
        }

        fn settle(
            &self,
            effect_id: CompanionEffectId,
            outcome: CompanionTurnEffectOutcome,
            now: TimestampMillis,
        ) -> Result<CompanionTurnEffect, CompanionTurnEffectRepositoryError> {
            let mut effects = self.0.lock().expect("effects");
            let effect = effects
                .iter_mut()
                .find(|effect| effect.id == effect_id)
                .ok_or(CompanionTurnEffectRepositoryError::NotFound)?;
            match outcome {
                CompanionTurnEffectOutcome::Ready {
                    summary,
                    memory_changes,
                    source_window,
                } => {
                    effect.status = CompanionTurnEffectStatus::Ready;
                    effect.summary = summary;
                    effect.memory_changes = memory_changes;
                    effect.source_window = Some(source_window);
                }
                CompanionTurnEffectOutcome::Failed { summary } => {
                    effect.status = CompanionTurnEffectStatus::Failed;
                    effect.summary = Some(summary);
                }
            }
            effect.updated_at = now;
            Ok(effect.clone())
        }
    }

    fn effect(conversation_id: ConversationId, created_at: i64) -> CompanionTurnEffect {
        CompanionTurnEffect {
            id: CompanionEffectId::new(),
            conversation_id,
            turn_id: GenerationTurnId::new(),
            user_message_id: Some(MessageId::new()),
            assistant_message_id: MessageId::new(),
            status: CompanionTurnEffectStatus::Processing,
            summary: None,
            seed: CompanionTurnEffectSeed::default(),
            memory_changes: CompanionMemoryChanges::default(),
            source_window: None,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(created_at),
        }
    }

    #[test]
    fn discovery_coalesces_per_conversation_and_admits_exactly_once() {
        let effects = Effects::default();
        let jobs = InMemoryJobStore::new();
        let first_conversation = ConversationId::new();
        let second_conversation = ConversationId::new();
        let later = effect(first_conversation, 20);
        let earlier = effect(first_conversation, 10);
        let other = effect(second_conversation, 15);
        effects.replace(vec![later.clone(), other, earlier.clone()]);
        let coordinator = CompanionPostTurnMemoryAdmissionCoordinator::new(&effects, &jobs);

        let admitted = coordinator
            .discover_and_admit(MAX_COMPANION_POST_TURN_EFFECTS)
            .expect("admit batches");
        assert_eq!(admitted.len(), 2);
        let first = admitted
            .iter()
            .find(|admission| admission.batch.conversation_id == first_conversation)
            .expect("first conversation");
        assert!(first.created);
        assert_eq!(first.job.kind, JobKind::MemoryExtraction);
        assert_eq!(first.job.state, JobState::Queued);
        assert_eq!(first.batch.effects[0].id, earlier.id);
        assert_eq!(first.batch.effects[1].id, later.id);
        assert_eq!(
            first.batch.terminal_effects()[0].enqueued_at,
            earlier.created_at
        );

        let replay = coordinator
            .discover_and_admit(MAX_COMPANION_POST_TURN_EFFECTS)
            .expect("replay admission");
        assert!(replay.iter().all(|admission| !admission.created));
        assert_eq!(
            replay
                .iter()
                .find(|admission| admission.batch.conversation_id == first_conversation)
                .map(|admission| admission.job.id),
            Some(first.job.id)
        );

        effects.replace(vec![earlier, later, effect(first_conversation, 30)]);
        assert!(
            coordinator
                .discover_and_admit(MAX_COMPANION_POST_TURN_EFFECTS)
                .expect("active conversation remains coalesced")
                .is_empty()
        );
    }

    #[test]
    fn empty_durable_queue_admits_nothing_and_restart_rebuilds_same_logical_batch() {
        let effects = Effects::default();
        let first_store = InMemoryJobStore::new();
        let conversation_id = ConversationId::new();
        assert!(
            CompanionPostTurnMemoryAdmissionCoordinator::new(&effects, &first_store)
                .discover_and_admit(MAX_COMPANION_POST_TURN_EFFECTS)
                .expect("empty discovery")
                .is_empty()
        );

        let pending = effect(conversation_id, 10);
        effects.replace(vec![pending]);
        let first = CompanionPostTurnMemoryAdmissionCoordinator::new(&effects, &first_store)
            .discover_and_admit(MAX_COMPANION_POST_TURN_EFFECTS)
            .expect("first process")
            .remove(0);
        let restarted_store = InMemoryJobStore::new();
        let restarted =
            CompanionPostTurnMemoryAdmissionCoordinator::new(&effects, &restarted_store)
                .discover_and_admit(MAX_COMPANION_POST_TURN_EFFECTS)
                .expect("restart discovery")
                .remove(0);
        assert!(first.created && restarted.created);
        assert_eq!(first.job.kind, restarted.job.kind);
        assert_eq!(first.job.subject, restarted.job.subject);
        assert_eq!(first.batch.idempotency_key, restarted.batch.idempotency_key);
        assert_eq!(first.batch, restarted.batch);
    }

    #[test]
    fn startup_and_post_finalization_bridge_claims_each_conversation_once() {
        let effects = Effects::default();
        let jobs = InMemoryJobStore::new();
        let first_conversation = ConversationId::new();
        let second_conversation = ConversationId::new();
        effects.replace(vec![
            effect(first_conversation, 10),
            effect(first_conversation, 20),
            effect(second_conversation, 15),
        ]);
        let worker_id = WorkerId::new();
        let coordinator = crate::CompanionMemoryDispatchCoordinator::new(&effects, &jobs);

        let work = coordinator
            .discover_and_claim(
                MAX_COMPANION_POST_TURN_EFFECTS,
                worker_id,
                TimestampMillis::new(30),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("claim discovered work");
        assert_eq!(work.len(), 2);
        assert!(work.iter().all(|item| item.job.state == JobState::Running));
        assert!(work.iter().all(|item| {
            item.handle.id() == item.admission.job.id
                && item.claim.claim.job_id == item.admission.job.id
                && item.claim.claim.worker_id == worker_id
        }));
        assert_eq!(
            work.iter()
                .find(|item| item.admission.batch.conversation_id == first_conversation)
                .expect("first conversation")
                .admission
                .batch
                .effects
                .len(),
            2
        );

        assert!(
            coordinator
                .discover_and_claim(
                    MAX_COMPANION_POST_TURN_EFFECTS,
                    worker_id,
                    TimestampMillis::new(31),
                    Duration::from_secs(60),
                    &ResourceAvailability::all(),
                )
                .expect("post-finalization replay")
                .is_empty()
        );

        let restarted_jobs = InMemoryJobStore::new();
        let restarted = crate::CompanionMemoryDispatchCoordinator::new(&effects, &restarted_jobs)
            .discover_and_claim(
                MAX_COMPANION_POST_TURN_EFFECTS,
                WorkerId::new(),
                TimestampMillis::new(40),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("startup rediscovery");
        assert_eq!(restarted.len(), 2);
        for original in &work {
            let recovered = restarted
                .iter()
                .find(|item| {
                    item.admission.batch.conversation_id == original.admission.batch.conversation_id
                })
                .expect("same conversation after restart");
            assert_eq!(
                recovered.admission.batch.idempotency_key,
                original.admission.batch.idempotency_key
            );
        }
    }

    #[test]
    fn runner_errors_settle_cancel_fail_or_retry_on_the_same_claim() {
        let cancellation_effects = Effects::default();
        cancellation_effects.replace(vec![effect(ConversationId::new(), 10)]);
        let cancellation_jobs = InMemoryJobStore::new();
        let cancellation_coordinator = crate::CompanionMemoryDispatchCoordinator::new(
            &cancellation_effects,
            &cancellation_jobs,
        );
        let cancellation_work = cancellation_coordinator
            .discover_and_claim(
                MAX_COMPANION_POST_TURN_EFFECTS,
                WorkerId::new(),
                TimestampMillis::new(20),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("claim cancellation")
            .remove(0);
        cancellation_work.handle.request_cancel();
        cancellation_jobs
            .append_and_transition(lettuce_jobs::JobMutation::RequestCancellation {
                id: cancellation_work.claim.claim.job_id,
                reason: CancellationReason::Shutdown,
                at: cancellation_work.job.updated_at,
            })
            .expect("request durable cancellation");
        let cancellation = cancellation_coordinator
            .settle_run(
                cancellation_work,
                Err(crate::CompanionMemoryJobRunError::Inference(
                    crate::CompanionMemoryInferenceError::Cancelled,
                )),
                CancellationReason::Shutdown,
                TimestampMillis::new(21),
            )
            .expect("settle cancellation");
        let crate::CompanionMemorySettledWork::Cancelled { job, .. } = cancellation else {
            panic!("expected cancelled job");
        };
        assert_eq!(job.state, JobState::Cancelled);
        assert_eq!(job.cancellation.reason, Some(CancellationReason::Shutdown));

        let failure_effects = Effects::default();
        failure_effects.replace(vec![effect(ConversationId::new(), 30)]);
        let failure_jobs = InMemoryJobStore::new();
        let failure_coordinator =
            crate::CompanionMemoryDispatchCoordinator::new(&failure_effects, &failure_jobs);
        let failure_work = failure_coordinator
            .discover_and_claim(
                MAX_COMPANION_POST_TURN_EFFECTS,
                WorkerId::new(),
                TimestampMillis::new(40),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("claim failure")
            .remove(0);
        let failure = failure_coordinator
            .settle_run(
                failure_work,
                Err(crate::CompanionMemoryJobRunError::Inference(
                    crate::CompanionMemoryInferenceError::Inference(
                        lettuce_conversations::PortError::Unavailable,
                    ),
                )),
                CancellationReason::User,
                TimestampMillis::new(41),
            )
            .expect("settle failure");
        let crate::CompanionMemorySettledWork::Failed { job, .. } = failure else {
            panic!("expected failed job");
        };
        assert_eq!(job.state, JobState::Failed);
        assert_eq!(
            job.error.as_ref().map(|error| error.code),
            Some(JobErrorCode::ResourceUnavailable)
        );

        let retry_effects = Effects::default();
        retry_effects.replace(vec![effect(ConversationId::new(), 50)]);
        let retry_jobs = InMemoryJobStore::new();
        let retry_coordinator =
            crate::CompanionMemoryDispatchCoordinator::new(&retry_effects, &retry_jobs);
        let retry_work = retry_coordinator
            .discover_and_claim(
                MAX_COMPANION_POST_TURN_EFFECTS,
                WorkerId::new(),
                TimestampMillis::new(60),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("claim retry")
            .remove(0);
        let job_id = retry_work.claim.claim.job_id;
        let retry = retry_coordinator
            .settle_run(
                retry_work,
                Err(crate::CompanionMemoryJobRunError::Admission(
                    crate::CompanionPostTurnMemoryRunError::InvalidAdmission,
                )),
                CancellationReason::Recovery,
                TimestampMillis::new(61),
            )
            .expect("schedule retry");
        let crate::CompanionMemorySettledWork::RetryScheduled { job, .. } = retry else {
            panic!("expected retry job");
        };
        assert_eq!(job.state, JobState::Queued);
        let reclaimed = retry_coordinator
            .discover_and_claim(
                MAX_COMPANION_POST_TURN_EFFECTS,
                WorkerId::new(),
                TimestampMillis::new(62),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("reclaim retry")
            .remove(0);
        assert_eq!(reclaimed.claim.claim.job_id, job_id);
        assert_eq!(reclaimed.claim.claim.attempt.get(), 2);
    }
}
