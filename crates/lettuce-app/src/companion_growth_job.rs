use lettuce_characters::{CharacterRepository, InteractionMode, RepositoryError};
use lettuce_companions::{MAX_GROWTH_MEMORIES, SoulOwner, SoulRepository, SoulRepositoryError};
use lettuce_conversations::{
    ConversationKind, ConversationReader, ConversationRepositoryError, ResolvedInferenceProfile,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_memory::{DynamicMemoryAttemptStatus, MemoryItem};
use lettuce_types::{
    CharacterId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId, Revision,
};

use crate::CompanionMemoryJobRunResult;

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionGrowthJobInput {
    pub conversation_id: ConversationId,
    pub character_id: CharacterId,
    pub memory_run_id: DynamicMemoryRunId,
    pub memory_attempt_id: DynamicMemoryAttemptId,
    pub profile: ResolvedInferenceProfile,
    pub expected_soul_revision: Revision,
    pub fresh_memories: Vec<MemoryItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionGrowthJobAdmission {
    pub input: CompanionGrowthJobInput,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionGrowthJobAdmissionError {
    #[error("companion growth source is invalid")]
    InvalidSource,
    #[error("companion growth conversation read failed: {0}")]
    Conversation(#[from] ConversationRepositoryError),
    #[error("companion growth character read failed: {0}")]
    Character(#[from] RepositoryError),
    #[error("companion growth Soul read failed: {0:?}")]
    Soul(SoulRepositoryError),
    #[error("companion growth job admission failed: {0}")]
    Job(StoreError),
}

#[derive(Debug)]
pub struct CompanionGrowthJobAdmissionCoordinator<'a, R: ?Sized, J: ?Sized> {
    sources: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> CompanionGrowthJobAdmissionCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(sources: &'a R, jobs: &'a J) -> Self {
        Self { sources, jobs }
    }
}

impl<R: ConversationReader + CharacterRepository + SoulRepository + ?Sized, J: JobStore + ?Sized>
    CompanionGrowthJobAdmissionCoordinator<'_, R, J>
{
    pub fn admit_after_memory(
        &self,
        result: &CompanionMemoryJobRunResult,
    ) -> Result<Option<CompanionGrowthJobAdmission>, CompanionGrowthJobAdmissionError> {
        if result.fresh_memories.is_empty() {
            return Ok(None);
        }
        if result.fresh_memories.len() > MAX_GROWTH_MEMORIES
            || result
                .fresh_memories
                .iter()
                .any(|memory| memory.text.trim().is_empty())
            || result.dispatch.attempt.status != DynamicMemoryAttemptStatus::Succeeded
            || result.dispatch.attempt.run_id != result.dispatch.run.id
        {
            return Err(CompanionGrowthJobAdmissionError::InvalidSource);
        }
        let conversation =
            ConversationReader::get(self.sources, result.dispatch.run.conversation_id)?;
        let ConversationKind::Direct(details) = conversation.conversation.kind else {
            return Ok(None);
        };
        let character_id = details.character.source_id;
        let character = CharacterRepository::get(self.sources, character_id)?
            .ok_or(CompanionGrowthJobAdmissionError::InvalidSource)?;
        if character.character.defaults.interaction_mode != InteractionMode::Companion {
            return Ok(None);
        }
        let soul = SoulRepository::get(self.sources, SoulOwner::Character(character_id))
            .map_err(CompanionGrowthJobAdmissionError::Soul)?
            .ok_or(CompanionGrowthJobAdmissionError::InvalidSource)?;
        let input = CompanionGrowthJobInput {
            conversation_id: conversation.conversation.id,
            character_id,
            memory_run_id: result.dispatch.run.id,
            memory_attempt_id: result.dispatch.attempt.id,
            profile: result.dispatch.run.profile.clone(),
            expected_soul_revision: soul.revision,
            fresh_memories: result.fresh_memories.clone(),
        };
        let idempotency_key =
            IdempotencyKey::new(format!("companion-growth-{}", input.memory_run_id))
                .map_err(|_| CompanionGrowthJobAdmissionError::InvalidSource)?;
        let subject = JobSubject::new(SubjectKind::Conversation, input.conversation_id.to_string())
            .map_err(|_| CompanionGrowthJobAdmissionError::InvalidSource)?;
        let admitted = self
            .jobs
            .create_or_get(
                JobSpec::new(
                    JobKind::CompanionGrowth,
                    subject,
                    OutcomeRef::Character(character_id),
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
            .map_err(CompanionGrowthJobAdmissionError::Job)?;
        Ok(Some(CompanionGrowthJobAdmission {
            input,
            job: admitted.job,
            created: admitted.created,
        }))
    }
}
