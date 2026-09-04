use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_creation::{
    StagedLorebookCoherenceChange, StagedLorebookDraftEdit, StagedLorebookPlanningRun,
    StagedLorebookProject, StagedLorebookRepository, StagedLorebookRepositoryError,
    StagedLorebookSourceExcerpt,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_models::{CapabilityStatus, Modality};
use lettuce_types::{CreationWorkflowId, LorebookEntryId, RequestId, Revision, TimestampMillis};

#[derive(Debug, Clone)]
pub struct StagedLorebookAdmissionRequest<'a> {
    pub request_id: RequestId,
    pub project_id: CreationWorkflowId,
    pub brief: String,
    pub initial_lorebook_name: Option<String>,
    pub target_count: u32,
    pub excerpts: Vec<StagedLorebookSourceExcerpt>,
    pub planner_profile: ResolvedInferenceProfile,
    pub planner_prompt: &'a PromptDocument,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagedLorebookAdmission {
    pub run: StagedLorebookPlanningRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookAdmissionError {
    #[error("staged lorebook admission input is invalid")]
    InvalidInput,
    #[error("staged lorebook persistence failed: {0}")]
    Repository(#[from] StagedLorebookRepositoryError),
    #[error("staged lorebook job persistence failed: {0}")]
    Job(#[from] StoreError),
}

#[derive(Debug)]
pub struct StagedLorebookCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> StagedLorebookCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(repository: &'a R, jobs: &'a J) -> Self {
        Self { repository, jobs }
    }
}

impl<R: StagedLorebookRepository + ?Sized, J: JobStore + ?Sized>
    StagedLorebookCoordinator<'_, R, J>
{
    pub fn admit(
        &self,
        request: StagedLorebookAdmissionRequest<'_>,
    ) -> Result<StagedLorebookAdmission, StagedLorebookAdmissionError> {
        validate_request(&request)?;
        let project = StagedLorebookProject::create(
            request.project_id,
            request.brief.clone(),
            request.initial_lorebook_name.clone(),
            request.target_count,
            request.excerpts.clone(),
            request.now,
        )
        .map_err(|_| StagedLorebookAdmissionError::InvalidInput)?;
        match self.repository.load_staged_lorebook(request.request_id) {
            Ok(run) => {
                let expected = run_from(&request, run.job_id, project);
                if !same_admission(&run, &expected) {
                    return Err(StagedLorebookRepositoryError::Conflict.into());
                }
                let job = self
                    .jobs
                    .get(run.job_id)?
                    .ok_or(StagedLorebookAdmissionError::InvalidInput)?;
                validate_job(&run, &job)?;
                return Ok(StagedLorebookAdmission {
                    run,
                    job,
                    created: false,
                });
            }
            Err(StagedLorebookRepositoryError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::CreationRun,
                JobSubject::new(SubjectKind::CreationProject, request.project_id.to_string())
                    .map_err(|_| StagedLorebookAdmissionError::InvalidInput)?,
                OutcomeRef::Request(request.request_id),
            )
            .with_idempotency_key(
                IdempotencyKey::new(format!("staged-lorebook-{}", request.request_id))
                    .map_err(|_| StagedLorebookAdmissionError::InvalidInput)?,
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
        let run =
            self.repository
                .admit_staged_lorebook(run_from(&request, admitted.job.id, project))?;
        validate_job(&run, &admitted.job)?;
        Ok(StagedLorebookAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }

    pub fn start_planning(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookAdmissionError> {
        self.repository
            .start_staged_lorebook_planning(request_id, expected_revision, now)
            .map_err(Into::into)
    }

    pub fn approve_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookAdmissionError> {
        self.repository
            .approve_staged_lorebook_outline(request_id, expected_revision, now)
            .map_err(Into::into)
    }

    pub fn edit_draft(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        edit: StagedLorebookDraftEdit,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookAdmissionError> {
        self.repository
            .edit_staged_lorebook_draft(request_id, expected_revision, edit, now)
            .map_err(Into::into)
    }

    pub fn set_draft_approved(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        plan_id: LorebookEntryId,
        approved: bool,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookAdmissionError> {
        self.repository
            .set_staged_lorebook_draft_approved(
                request_id,
                expected_revision,
                plan_id,
                approved,
                now,
            )
            .map_err(Into::into)
    }

    pub fn submit_coherence(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        proposals: Vec<StagedLorebookCoherenceChange>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookAdmissionError> {
        self.repository
            .submit_staged_lorebook_coherence(request_id, expected_revision, proposals, now)
            .map_err(Into::into)
    }

    pub fn apply_coherence(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        accepted_change_ids: Vec<String>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookAdmissionError> {
        self.repository
            .apply_staged_lorebook_coherence(
                request_id,
                expected_revision,
                accepted_change_ids,
                now,
            )
            .map_err(Into::into)
    }
}

fn same_admission(
    stored: &StagedLorebookPlanningRun,
    expected: &StagedLorebookPlanningRun,
) -> bool {
    stored.request_id == expected.request_id
        && stored.job_id == expected.job_id
        && stored.planner_profile == expected.planner_profile
        && stored.planner_prompt_id == expected.planner_prompt_id
        && stored.planner_prompt_revision == expected.planner_prompt_revision
        && stored.project.id == expected.project.id
        && stored.project.brief == expected.project.brief
        && stored.project.initial_lorebook_name == expected.project.initial_lorebook_name
        && stored.project.target_count == expected.project.target_count
        && stored.project.excerpts == expected.project.excerpts
        && stored.project.created_at == expected.project.created_at
}

fn validate_request(
    request: &StagedLorebookAdmissionRequest<'_>,
) -> Result<(), StagedLorebookAdmissionError> {
    if request.planner_prompt.status != LifecycleStatus::Active
        || request.planner_prompt.purpose != PromptPurpose::LorebookGeneratorPlanner
        || request
            .planner_profile
            .chat_profile
            .capabilities
            .input_modalities
            .get(Modality::Text)
            != CapabilityStatus::Supported
        || request
            .planner_profile
            .chat_profile
            .capabilities
            .output_modalities
            .get(Modality::Text)
            != CapabilityStatus::Supported
    {
        return Err(StagedLorebookAdmissionError::InvalidInput);
    }
    Ok(())
}

fn run_from(
    request: &StagedLorebookAdmissionRequest<'_>,
    job_id: lettuce_types::JobId,
    project: StagedLorebookProject,
) -> StagedLorebookPlanningRun {
    StagedLorebookPlanningRun {
        request_id: request.request_id,
        job_id,
        project,
        planner_profile: request.planner_profile.clone(),
        planner_prompt_id: request.planner_prompt.id,
        planner_prompt_revision: request.planner_prompt.revision,
        planner_attempt: None,
    }
}

fn validate_job(
    run: &StagedLorebookPlanningRun,
    job: &JobSnapshot,
) -> Result<(), StagedLorebookAdmissionError> {
    if job.id != run.job_id
        || job.kind != JobKind::CreationRun
        || job.subject.kind != SubjectKind::CreationProject
        || job.subject.id.as_str() != run.project.id.to_string()
    {
        return Err(StagedLorebookAdmissionError::InvalidInput);
    }
    Ok(())
}
