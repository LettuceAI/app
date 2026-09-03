use lettuce_companions::{
    CompanionSoulWriterRun, CompanionSoulWriterRunRepository,
    CompanionSoulWriterRunRepositoryError, SoulWriterFallbackFormat, normalize_soul_writer_draft,
    soul_writer_prompt_values,
};
use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_types::{RequestId, TimestampMillis};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CompanionSoulWriterAdmissionRequest<'a> {
    pub request_id: RequestId,
    pub primary_profile: ResolvedInferenceProfile,
    pub fallback_profile: Option<ResolvedInferenceProfile>,
    pub prompt: &'a PromptDocument,
    pub character_name: &'a str,
    pub character_definition: Option<&'a str>,
    pub character_description: Option<&'a str>,
    pub opening_context: Option<&'a str>,
    pub current_soul: Option<&'a Value>,
    pub user_notes: Option<&'a str>,
    pub fallback_format: SoulWriterFallbackFormat,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionSoulWriterAdmission {
    pub run: CompanionSoulWriterRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionSoulWriterAdmissionError {
    #[error("companion Soul-writer admission input is invalid")]
    InvalidInput,
    #[error("companion Soul-writer run persistence failed: {0}")]
    Run(CompanionSoulWriterRunRepositoryError),
    #[error("companion Soul-writer job persistence failed: {0}")]
    Job(StoreError),
}

#[derive(Debug)]
pub struct CompanionSoulWriterAdmissionCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> CompanionSoulWriterAdmissionCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(repository: &'a R, jobs: &'a J) -> Self {
        Self { repository, jobs }
    }
}

impl<R: CompanionSoulWriterRunRepository + ?Sized, J: JobStore + ?Sized>
    CompanionSoulWriterAdmissionCoordinator<'_, R, J>
{
    pub fn admit(
        &self,
        request: CompanionSoulWriterAdmissionRequest<'_>,
    ) -> Result<CompanionSoulWriterAdmission, CompanionSoulWriterAdmissionError> {
        match self
            .repository
            .load_companion_soul_writer_run(request.request_id)
        {
            Ok(run) => {
                let job = self
                    .jobs
                    .get(run.job_id)
                    .map_err(CompanionSoulWriterAdmissionError::Job)?
                    .ok_or(CompanionSoulWriterAdmissionError::InvalidInput)?;
                return Ok(CompanionSoulWriterAdmission {
                    run,
                    job,
                    created: false,
                });
            }
            Err(CompanionSoulWriterRunRepositoryError::NotFound) => {}
            Err(error) => return Err(CompanionSoulWriterAdmissionError::Run(error)),
        }
        if request.prompt.status != LifecycleStatus::Active
            || request.prompt.purpose != PromptPurpose::CompanionSoulWriter
            || request.prompt.revision.get() == 0
            || request.character_name.trim().is_empty()
            || request.now.get() < 0
        {
            return Err(CompanionSoulWriterAdmissionError::InvalidInput);
        }
        let idempotency_key =
            IdempotencyKey::new(format!("companion-soul-writer-{}", request.request_id))
                .map_err(|_| CompanionSoulWriterAdmissionError::InvalidInput)?;
        let subject = JobSubject::new(
            SubjectKind::ModelProfile,
            request
                .primary_profile
                .chat_profile
                .model_profile_id
                .to_string(),
        )
        .map_err(|_| CompanionSoulWriterAdmissionError::InvalidInput)?;
        let admitted = self
            .jobs
            .create_or_get(
                JobSpec::new(
                    JobKind::CompanionSoulWriter,
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
            )
            .map_err(CompanionSoulWriterAdmissionError::Job)?;
        let run = CompanionSoulWriterRun {
            request_id: request.request_id,
            job_id: admitted.job.id,
            primary_profile: request.primary_profile,
            fallback_profile: request.fallback_profile,
            prompt_id: request.prompt.id,
            prompt_revision: request.prompt.revision,
            prompt_values: soul_writer_prompt_values(
                request.character_name,
                request.character_definition,
                request.character_description,
                request.opening_context,
                request.current_soul,
                request.user_notes,
            ),
            starting_draft: normalize_soul_writer_draft(request.current_soul),
            fallback_format: request.fallback_format,
            created_at: request.now,
            rounds: Vec::new(),
        };
        let run = self
            .repository
            .admit_companion_soul_writer_run(run)
            .map_err(CompanionSoulWriterAdmissionError::Run)?;
        Ok(CompanionSoulWriterAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }
}
