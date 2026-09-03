use lettuce_companions::{
    CompanionSoulWriterRun, CompanionSoulWriterRunRepository,
    CompanionSoulWriterRunRepositoryError, SoulWriterFallbackFormat, normalize_soul_writer_draft,
    soul_writer_prompt_values,
};
use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
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
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionSoulWriterAdmissionError {
    #[error("companion Soul-writer admission input is invalid")]
    InvalidInput,
    #[error("companion Soul-writer run persistence failed: {0}")]
    Run(CompanionSoulWriterRunRepositoryError),
}

#[derive(Debug)]
pub struct CompanionSoulWriterAdmissionCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> CompanionSoulWriterAdmissionCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R: CompanionSoulWriterRunRepository + ?Sized> CompanionSoulWriterAdmissionCoordinator<'_, R> {
    pub fn admit(
        &self,
        request: CompanionSoulWriterAdmissionRequest<'_>,
    ) -> Result<CompanionSoulWriterAdmission, CompanionSoulWriterAdmissionError> {
        match self
            .repository
            .load_companion_soul_writer_run(request.request_id)
        {
            Ok(run) => {
                return Ok(CompanionSoulWriterAdmission {
                    run,
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
        let run = CompanionSoulWriterRun {
            request_id: request.request_id,
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
        Ok(CompanionSoulWriterAdmission { run, created: true })
    }
}
