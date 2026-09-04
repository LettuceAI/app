use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_creation::{
    StagedLorebookDraftStatus, StagedLorebookEntryPlan, StagedLorebookPlanningRun,
    StagedLorebookRepository, StagedLorebookRepositoryError, StagedLorebookSourceExcerpt,
    StagedLorebookStage, StagedLorebookWriterPromptValues, StagedLorebookWriterRun,
    StagedLorebookWriterRunRepository, StagedLorebookWriterRunRepositoryError,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_models::{CapabilityStatus, Modality};
use lettuce_types::{LorebookEntryId, RequestId, TimestampMillis};

#[derive(Debug, Clone)]
pub struct StagedLorebookWriterRequest<'a> {
    pub request_id: RequestId,
    pub project_request_id: RequestId,
    pub plan_id: LorebookEntryId,
    pub profile: ResolvedInferenceProfile,
    pub prompt: &'a PromptDocument,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagedLorebookWriterAdmission {
    pub run: StagedLorebookWriterRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookWriterAdmissionError {
    #[error("staged lorebook writer admission input is invalid")]
    InvalidInput,
    #[error("staged lorebook project persistence failed: {0}")]
    Project(#[from] StagedLorebookRepositoryError),
    #[error("staged lorebook writer run persistence failed: {0}")]
    Run(#[from] StagedLorebookWriterRunRepositoryError),
    #[error("staged lorebook writer job persistence failed: {0}")]
    Job(#[from] StoreError),
}

#[derive(Debug)]
pub struct StagedLorebookWriterCoordinator<'a, P: ?Sized, R: ?Sized, J: ?Sized> {
    projects: &'a P,
    runs: &'a R,
    jobs: &'a J,
}

impl<'a, P: ?Sized, R: ?Sized, J: ?Sized> StagedLorebookWriterCoordinator<'a, P, R, J> {
    #[must_use]
    pub const fn new(projects: &'a P, runs: &'a R, jobs: &'a J) -> Self {
        Self {
            projects,
            runs,
            jobs,
        }
    }
}

impl<P, R, J> StagedLorebookWriterCoordinator<'_, P, R, J>
where
    P: StagedLorebookRepository + ?Sized,
    R: StagedLorebookWriterRunRepository + ?Sized,
    J: JobStore + ?Sized,
{
    pub fn prepare_and_admit(
        &self,
        request: StagedLorebookWriterRequest<'_>,
    ) -> Result<StagedLorebookWriterAdmission, StagedLorebookWriterAdmissionError> {
        validate_request(&request)?;
        match self
            .runs
            .load_staged_lorebook_writer_run(request.request_id)
        {
            Ok(run) => {
                if !same_request(&run, &request) {
                    return Err(StagedLorebookWriterRunRepositoryError::Conflict.into());
                }
                let job = self
                    .jobs
                    .get(run.job_id)?
                    .ok_or(StagedLorebookWriterAdmissionError::InvalidInput)?;
                validate_job(&run, &job)?;
                return Ok(StagedLorebookWriterAdmission {
                    run,
                    job,
                    created: false,
                });
            }
            Err(StagedLorebookWriterRunRepositoryError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let project = self
            .projects
            .load_staged_lorebook(request.project_request_id)?;
        let values = prepare_values(&project, request.plan_id)?;
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::CreationRun,
                JobSubject::new(SubjectKind::CreationProject, project.project.id.to_string())
                    .map_err(|_| StagedLorebookWriterAdmissionError::InvalidInput)?,
                OutcomeRef::Request(request.request_id),
            )
            .with_idempotency_key(
                IdempotencyKey::new(format!("staged-lorebook-writer-{}", request.request_id))
                    .map_err(|_| StagedLorebookWriterAdmissionError::InvalidInput)?,
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
        let run = StagedLorebookWriterRun {
            request_id: request.request_id,
            job_id: admitted.job.id,
            project_request_id: request.project_request_id,
            project_id: project.project.id,
            project_revision: project.project.revision,
            plan_id: request.plan_id,
            profile: request.profile,
            prompt_id: request.prompt.id,
            prompt_revision: request.prompt.revision,
            prompt_values: values,
            created_at: request.now,
        };
        let run = self.runs.admit_staged_lorebook_writer_run(run)?;
        validate_job(&run, &admitted.job)?;
        Ok(StagedLorebookWriterAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }
}

fn validate_request(
    request: &StagedLorebookWriterRequest<'_>,
) -> Result<(), StagedLorebookWriterAdmissionError> {
    if request.prompt.status != LifecycleStatus::Active
        || request.prompt.purpose != PromptPurpose::LorebookGeneratorWriter
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
        return Err(StagedLorebookWriterAdmissionError::InvalidInput);
    }
    Ok(())
}

fn prepare_values(
    run: &StagedLorebookPlanningRun,
    plan_id: LorebookEntryId,
) -> Result<StagedLorebookWriterPromptValues, StagedLorebookWriterAdmissionError> {
    if run.project.stage != StagedLorebookStage::Drafting {
        return Err(StagedLorebookWriterAdmissionError::InvalidInput);
    }
    let plan = run
        .project
        .outline
        .iter()
        .find(|plan| plan.id == plan_id)
        .ok_or(StagedLorebookWriterAdmissionError::InvalidInput)?;
    let draft = run
        .project
        .drafts
        .iter()
        .find(|draft| draft.plan_id == plan_id)
        .ok_or(StagedLorebookWriterAdmissionError::InvalidInput)?;
    if draft.status != StagedLorebookDraftStatus::Pending {
        return Err(StagedLorebookWriterAdmissionError::InvalidInput);
    }
    Ok(StagedLorebookWriterPromptValues {
        brief: run.project.brief.clone(),
        outline: format_outline(&run.project.outline),
        entry_title: plan.title.clone(),
        entry_category: plan.category.clone(),
        entry_proposed_keys: format_keys(&plan.proposed_keys),
        entry_rationale: plan.rationale.clone(),
        relevant_excerpts: relevant_excerpts(plan, &run.project.excerpts),
    })
}

fn format_outline(outline: &[StagedLorebookEntryPlan]) -> String {
    if outline.is_empty() {
        return "(empty)".to_owned();
    }
    outline
        .iter()
        .map(|plan| {
            format!(
                "{}. {} [{}] keys: {}",
                plan.ordinal + 1,
                plan.title,
                plan.category,
                format_keys(&plan.proposed_keys)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_keys(keys: &[String]) -> String {
    if keys.is_empty() {
        "(none)".to_owned()
    } else {
        keys.join(", ")
    }
}

fn relevant_excerpts(
    plan: &StagedLorebookEntryPlan,
    excerpts: &[StagedLorebookSourceExcerpt],
) -> String {
    if plan.source_refs.is_empty() {
        return format_excerpts(excerpts);
    }
    let selected = excerpts
        .iter()
        .filter(|excerpt| plan.source_refs.iter().any(|id| id == &excerpt.source_id))
        .map(format_excerpt)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        format_excerpts(excerpts)
    } else {
        selected.join("\n\n---\n\n")
    }
}

fn format_excerpts(excerpts: &[StagedLorebookSourceExcerpt]) -> String {
    if excerpts.is_empty() {
        return "(none)".to_owned();
    }
    excerpts
        .iter()
        .map(format_excerpt)
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn format_excerpt(excerpt: &StagedLorebookSourceExcerpt) -> String {
    format!(
        "[{}] {}\n{}",
        excerpt.source_id, excerpt.label, excerpt.content
    )
}

fn same_request(run: &StagedLorebookWriterRun, request: &StagedLorebookWriterRequest<'_>) -> bool {
    run.request_id == request.request_id
        && run.project_request_id == request.project_request_id
        && run.plan_id == request.plan_id
        && run.profile == request.profile
        && run.prompt_id == request.prompt.id
        && run.prompt_revision == request.prompt.revision
        && run.created_at == request.now
}

fn validate_job(
    run: &StagedLorebookWriterRun,
    job: &JobSnapshot,
) -> Result<(), StagedLorebookWriterAdmissionError> {
    if job.id != run.job_id
        || job.kind != JobKind::CreationRun
        || job.subject.kind != SubjectKind::CreationProject
        || job.subject.id.as_str() != run.project_id.to_string()
    {
        return Err(StagedLorebookWriterAdmissionError::InvalidInput);
    }
    Ok(())
}
