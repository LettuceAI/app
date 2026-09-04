use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_creation::{
    LorebookEntryFallbackFormat, LorebookKeywordGenerationRun, LorebookKeywordPromptValues,
    LorebookKeywordRunRepository, LorebookKeywordRunRepositoryError,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_models::{CapabilityStatus, Modality};
use lettuce_types::{RequestId, TimestampMillis};

#[derive(Debug, Clone)]
pub struct LorebookKeywordRequest<'a> {
    pub request_id: RequestId,
    pub title: Option<String>,
    pub content: String,
    pub direction_prompt: Option<String>,
    pub existing_keywords: Vec<String>,
    pub profile: ResolvedInferenceProfile,
    pub prompt: &'a PromptDocument,
    pub fallback_format: LorebookEntryFallbackFormat,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LorebookKeywordAdmission {
    pub run: LorebookKeywordGenerationRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookKeywordAdmissionError {
    #[error("lorebook keyword generation input is invalid")]
    InvalidInput,
    #[error("lorebook keyword generation run persistence failed: {0}")]
    Run(#[from] LorebookKeywordRunRepositoryError),
    #[error("lorebook keyword generation job persistence failed: {0}")]
    Job(#[from] StoreError),
}

#[derive(Debug)]
pub struct LorebookKeywordCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> LorebookKeywordCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(repository: &'a R, jobs: &'a J) -> Self {
        Self { repository, jobs }
    }
}

impl<R: LorebookKeywordRunRepository + ?Sized, J: JobStore + ?Sized>
    LorebookKeywordCoordinator<'_, R, J>
{
    pub fn prepare_and_admit(
        &self,
        request: LorebookKeywordRequest<'_>,
    ) -> Result<LorebookKeywordAdmission, LorebookKeywordAdmissionError> {
        validate_request(&request)?;
        let prompt_values = LorebookKeywordPromptValues {
            entry_title: normalized_or(&request.title, "(untitled)"),
            entry_content: request.content.trim().to_owned(),
            existing_keywords: format_existing_keywords(&request.existing_keywords),
            direction_prompt: normalized_or(&request.direction_prompt, "(none)"),
        };
        match self
            .repository
            .load_lorebook_keyword_run(request.request_id)
        {
            Ok(run) => {
                let expected = run_from_request(&request, run.job_id, prompt_values);
                if run != expected {
                    return Err(LorebookKeywordRunRepositoryError::Conflict.into());
                }
                let job = self
                    .jobs
                    .get(run.job_id)?
                    .ok_or(LorebookKeywordAdmissionError::InvalidInput)?;
                validate_job(&run, &job)?;
                return Ok(LorebookKeywordAdmission {
                    run,
                    job,
                    created: false,
                });
            }
            Err(LorebookKeywordRunRepositoryError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let model_profile_id = request.profile.chat_profile.model_profile_id;
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::CreationRun,
                JobSubject::new(SubjectKind::ModelProfile, model_profile_id.to_string())
                    .map_err(|_| LorebookKeywordAdmissionError::InvalidInput)?,
                OutcomeRef::Request(request.request_id),
            )
            .with_idempotency_key(
                IdempotencyKey::new(format!("lorebook-keyword-generator-{}", request.request_id))
                    .map_err(|_| LorebookKeywordAdmissionError::InvalidInput)?,
            )
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
        let run = run_from_request(&request, admitted.job.id, prompt_values);
        let run = self.repository.admit_lorebook_keyword_run(run)?;
        validate_job(&run, &admitted.job)?;
        Ok(LorebookKeywordAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }
}

fn validate_request(
    request: &LorebookKeywordRequest<'_>,
) -> Result<(), LorebookKeywordAdmissionError> {
    if request.content.trim().is_empty()
        || request.prompt.status != LifecycleStatus::Active
        || request.prompt.purpose != PromptPurpose::LorebookKeywordGenerator
        || request.prompt.revision.get() == 0
        || request
            .profile
            .chat_profile
            .capabilities
            .input_modalities
            .get(Modality::Text)
            != CapabilityStatus::Supported
        || request
            .profile
            .chat_profile
            .capabilities
            .output_modalities
            .get(Modality::Text)
            != CapabilityStatus::Supported
    {
        return Err(LorebookKeywordAdmissionError::InvalidInput);
    }
    Ok(())
}

fn normalized_or(value: &Option<String>, fallback: &str) -> String {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

fn format_existing_keywords(keywords: &[String]) -> String {
    let keywords = keywords
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keywords.is_empty() {
        "(none)".to_owned()
    } else {
        keywords.join(", ")
    }
}

fn run_from_request(
    request: &LorebookKeywordRequest<'_>,
    job_id: lettuce_types::JobId,
    prompt_values: LorebookKeywordPromptValues,
) -> LorebookKeywordGenerationRun {
    LorebookKeywordGenerationRun {
        request_id: request.request_id,
        job_id,
        profile: request.profile.clone(),
        prompt_id: request.prompt.id,
        prompt_revision: request.prompt.revision,
        prompt_values,
        fallback_format: request.fallback_format,
        created_at: request.now,
    }
}

fn validate_job(
    run: &LorebookKeywordGenerationRun,
    job: &JobSnapshot,
) -> Result<(), LorebookKeywordAdmissionError> {
    if job.id != run.job_id
        || job.kind != JobKind::CreationRun
        || job.subject.kind != SubjectKind::ModelProfile
        || job.subject.id.as_str() != run.profile.chat_profile.model_profile_id.to_string()
    {
        return Err(LorebookKeywordAdmissionError::InvalidInput);
    }
    Ok(())
}
