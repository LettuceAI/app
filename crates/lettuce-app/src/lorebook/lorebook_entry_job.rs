use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_creation::{
    LorebookEntryFallbackFormat, LorebookEntryGenerationRun, LorebookEntryPromptValues,
    LorebookEntryRunRepository, LorebookEntryRunRepositoryError, LorebookEntrySource,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_types::{
    CharacterId, ConversationId, LorebookId, MemoryId, MessageId, PersonaId, RequestId,
    TimestampMillis,
};

#[derive(Debug, Clone)]
pub struct LorebookEntryAdmissionRequest<'a> {
    pub request_id: RequestId,
    pub conversation_id: ConversationId,
    pub lorebook_id: LorebookId,
    pub character_id: CharacterId,
    pub persona_id: Option<PersonaId>,
    pub selected_message_ids: Vec<MessageId>,
    pub selected_memory_ids: Vec<MemoryId>,
    pub source: LorebookEntrySource,
    pub include_memory_summary: bool,
    pub time_awareness_enabled: bool,
    pub force: bool,
    pub profile: ResolvedInferenceProfile,
    pub prompt: &'a PromptDocument,
    pub prompt_values: LorebookEntryPromptValues,
    pub fallback_format: LorebookEntryFallbackFormat,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LorebookEntryAdmission {
    pub run: LorebookEntryGenerationRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookEntryAdmissionError {
    #[error("lorebook entry generation admission input is invalid")]
    InvalidInput,
    #[error("lorebook entry generation run persistence failed: {0}")]
    Run(#[from] LorebookEntryRunRepositoryError),
    #[error("lorebook entry generation job persistence failed: {0}")]
    Job(#[from] StoreError),
}

#[derive(Debug)]
pub struct LorebookEntryAdmissionCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> LorebookEntryAdmissionCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(repository: &'a R, jobs: &'a J) -> Self {
        Self { repository, jobs }
    }
}

impl<R: LorebookEntryRunRepository + ?Sized, J: JobStore + ?Sized>
    LorebookEntryAdmissionCoordinator<'_, R, J>
{
    pub fn admit(
        &self,
        request: LorebookEntryAdmissionRequest<'_>,
    ) -> Result<LorebookEntryAdmission, LorebookEntryAdmissionError> {
        if request.prompt.status != LifecycleStatus::Active
            || request.prompt.purpose != PromptPurpose::LorebookEntryWriter
            || request.prompt.revision.get() == 0
        {
            return Err(LorebookEntryAdmissionError::InvalidInput);
        }
        match self.repository.load_lorebook_entry_run(request.request_id) {
            Ok(run) => {
                let expected = run_from_request(&request, run.job_id);
                if run != expected {
                    return Err(LorebookEntryAdmissionError::Run(
                        LorebookEntryRunRepositoryError::Conflict,
                    ));
                }
                let job = self
                    .jobs
                    .get(run.job_id)?
                    .ok_or(LorebookEntryAdmissionError::InvalidInput)?;
                validate_job(&run, &job)?;
                return Ok(LorebookEntryAdmission {
                    run,
                    job,
                    created: false,
                });
            }
            Err(LorebookEntryRunRepositoryError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let idempotency_key =
            IdempotencyKey::new(format!("lorebook-entry-generator-{}", request.request_id))
                .map_err(|_| LorebookEntryAdmissionError::InvalidInput)?;
        let subject = JobSubject::new(
            SubjectKind::Conversation,
            request.conversation_id.to_string(),
        )
        .map_err(|_| LorebookEntryAdmissionError::InvalidInput)?;
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::CreationRun,
                subject,
                OutcomeRef::Request(request.request_id),
            )
            .with_idempotency_key(idempotency_key)
            .with_resources(vec![
                ResourceClass::Network,
                ResourceClass::ModelLoad,
                ResourceClass::DiskRead,
                ResourceClass::DiskWrite,
                ResourceClass::Cpu,
            ])
            .with_priority(JobPriority::Interactive)
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative),
        )?;
        let run = run_from_request(&request, admitted.job.id);
        run.validate()?;
        let run = self.repository.admit_lorebook_entry_run(run)?;
        validate_job(&run, &admitted.job)?;
        Ok(LorebookEntryAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }
}

fn run_from_request(
    request: &LorebookEntryAdmissionRequest<'_>,
    job_id: lettuce_types::JobId,
) -> LorebookEntryGenerationRun {
    LorebookEntryGenerationRun {
        request_id: request.request_id,
        job_id,
        conversation_id: request.conversation_id,
        lorebook_id: request.lorebook_id,
        character_id: request.character_id,
        persona_id: request.persona_id,
        selected_message_ids: request.selected_message_ids.clone(),
        selected_memory_ids: request.selected_memory_ids.clone(),
        source: request.source,
        include_memory_summary: request.include_memory_summary,
        time_awareness_enabled: request.time_awareness_enabled,
        force: request.force,
        profile: request.profile.clone(),
        prompt_id: request.prompt.id,
        prompt_revision: request.prompt.revision,
        prompt_values: request.prompt_values.clone(),
        fallback_format: request.fallback_format,
        created_at: request.now,
    }
}

fn validate_job(
    run: &LorebookEntryGenerationRun,
    job: &JobSnapshot,
) -> Result<(), LorebookEntryAdmissionError> {
    if job.id != run.job_id
        || job.kind != JobKind::CreationRun
        || job.subject.kind != SubjectKind::Conversation
        || job.subject.id.as_str() != run.conversation_id.to_string()
    {
        return Err(LorebookEntryAdmissionError::InvalidInput);
    }
    Ok(())
}
