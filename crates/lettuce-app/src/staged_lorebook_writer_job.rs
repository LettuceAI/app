use lettuce_context::{LifecycleStatus, PromptDocument, PromptPurpose};
use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_creation::{
    StagedLorebookDraftStatus, StagedLorebookEntryPlan, StagedLorebookPlanningRun,
    StagedLorebookRefinement, StagedLorebookRepository, StagedLorebookRepositoryError,
    StagedLorebookSourceExcerpt, StagedLorebookStage, StagedLorebookWriterPromptValues,
    StagedLorebookWriterRun, StagedLorebookWriterRunRepository,
    StagedLorebookWriterRunRepositoryError,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_models::{CapabilityStatus, Modality};
use lettuce_types::{LorebookEntryId, RequestId, Revision, TimestampMillis};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StagedLorebookWriterRequest<'a> {
    pub request_id: RequestId,
    pub project_request_id: RequestId,
    pub plan_id: LorebookEntryId,
    pub profile: ResolvedInferenceProfile,
    pub prompt: &'a PromptDocument,
    pub now: TimestampMillis,
}

struct WriterAdmissionInput {
    prompt_snapshot: Option<PromptDocument>,
    request_id: RequestId,
    project_request_id: RequestId,
    plan_id: LorebookEntryId,
    profile: ResolvedInferenceProfile,
    prompt_id: lettuce_types::PromptDocumentId,
    prompt_revision: Revision,
    now: TimestampMillis,
}

#[derive(Debug, Clone)]
pub struct StagedLorebookRefineRequest<'a> {
    pub request_id: RequestId,
    pub project_request_id: RequestId,
    pub plan_id: LorebookEntryId,
    pub feedback: String,
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

#[derive(Debug, Clone)]
pub struct StagedLorebookConfiguredRefineRequest {
    pub request_id: RequestId,
    pub project_request_id: RequestId,
    pub plan_id: LorebookEntryId,
    pub feedback: String,
    pub overrides: lettuce_settings::LorebookGeneratorSelection,
    pub safety_policy: lettuce_conversations::SafetyContext,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagedLorebookWriterBatchAdmission {
    pub project: StagedLorebookPlanningRun,
    pub writers: Vec<StagedLorebookWriterAdmission>,
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookWriterAdmissionError {
    #[error("staged lorebook writer admission input is invalid")]
    InvalidInput,
    #[error("staged lorebook configuration failed: {0}")]
    Configuration(#[from] crate::StagedLorebookAdmissionError),
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
        self.admit_writer(WriterAdmissionInput {
            prompt_snapshot: Some(request.prompt.clone()),
            request_id: request.request_id,
            project_request_id: request.project_request_id,
            plan_id: request.plan_id,
            profile: request.profile,
            prompt_id: request.prompt.id,
            prompt_revision: request.prompt.revision,
            now: request.now,
        })
    }

    fn admit_writer(
        &self,
        request: WriterAdmissionInput,
    ) -> Result<StagedLorebookWriterAdmission, StagedLorebookWriterAdmissionError> {
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
            prompt_snapshot: request.prompt_snapshot,
            configured_overrides: None,
            request_id: request.request_id,
            job_id: admitted.job.id,
            project_request_id: request.project_request_id,
            project_id: project.project.id,
            project_revision: project.project.revision,
            plan_id: request.plan_id,
            profile: request.profile,
            prompt_id: request.prompt_id,
            prompt_revision: request.prompt_revision,
            prompt_values: values,
            refinement: None,
            created_at: request.now,
            attempt: None,
        };
        let run = self.runs.admit_staged_lorebook_writer_run(run)?;
        validate_job(&run, &admitted.job)?;
        Ok(StagedLorebookWriterAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }

    pub fn start_configured_batch(
        &self,
        project_request_id: RequestId,
        expected_revision: Revision,
        overrides: &lettuce_settings::LorebookGeneratorSelection,
        builtins: &crate::BuiltInPromptIds,
        safety_policy: lettuce_conversations::SafetyContext,
        now: TimestampMillis,
    ) -> Result<StagedLorebookWriterBatchAdmission, StagedLorebookWriterAdmissionError>
    where
        P: lettuce_settings::GlobalSettingsStore
            + lettuce_models::ModelProfileRepository
            + lettuce_models::ProviderAccountRepository
            + lettuce_context::PromptRepository,
    {
        let current = self.projects.load_staged_lorebook(project_request_id)?;
        if matches!(
            current.project.stage,
            StagedLorebookStage::Drafting | StagedLorebookStage::DraftsReady
        ) && current
            .project
            .drafts
            .iter()
            .any(|draft| draft.status == StagedLorebookDraftStatus::Drafting)
        {
            let inputs = current
                .writer_batch_inputs
                .clone()
                .ok_or(StagedLorebookWriterAdmissionError::InvalidInput)?;
            if &inputs.overrides != overrides || inputs.profile.safety_policy != safety_policy {
                return Err(StagedLorebookWriterRunRepositoryError::Conflict.into());
            }
            return self.admit_batch_writers(
                current,
                inputs.profile,
                inputs.prompt_id,
                inputs.prompt_revision,
                inputs.prompt_snapshot,
            );
        }
        let coordinator = crate::StagedLorebookCoordinator::new(self.projects, self.jobs);
        let selected = coordinator.project_overrides(project_request_id, overrides)?;
        let (profile, prompt, _) = coordinator.resolve_configured_stage(
            &selected,
            builtins,
            PromptPurpose::LorebookGeneratorWriter,
            safety_policy,
        )?;
        validate_writer_inputs(&profile, &prompt)?;
        let inputs = lettuce_creation::StagedLorebookWriterBatchInputs {
            prompt_snapshot: Some(prompt.clone()),
            overrides: overrides.clone(),
            profile: profile.clone(),
            prompt_id: prompt.id,
            prompt_revision: prompt.revision,
        };
        let project = self.projects.start_staged_lorebook_draft_batch(
            project_request_id,
            expected_revision,
            Some(inputs),
            now,
        )?;
        self.admit_batch_writers(project, profile, prompt.id, prompt.revision, Some(prompt))
    }

    pub fn start_batch(
        &self,
        project_request_id: RequestId,
        expected_revision: Revision,
        profile: ResolvedInferenceProfile,
        prompt: &PromptDocument,
        now: TimestampMillis,
    ) -> Result<StagedLorebookWriterBatchAdmission, StagedLorebookWriterAdmissionError> {
        validate_writer_inputs(&profile, prompt)?;
        let project = self.projects.start_staged_lorebook_draft_batch(
            project_request_id,
            expected_revision,
            None,
            now,
        )?;
        self.admit_batch_writers(
            project,
            profile,
            prompt.id,
            prompt.revision,
            Some(prompt.clone()),
        )
    }

    fn admit_batch_writers(
        &self,
        project: StagedLorebookPlanningRun,
        profile: ResolvedInferenceProfile,
        prompt_id: lettuce_types::PromptDocumentId,
        prompt_revision: Revision,
        prompt_snapshot: Option<PromptDocument>,
    ) -> Result<StagedLorebookWriterBatchAdmission, StagedLorebookWriterAdmissionError> {
        if project.writer_batch_inputs.as_ref().is_some_and(|inputs| {
            inputs.profile != profile
                || inputs.prompt_id != prompt_id
                || inputs.prompt_revision != prompt_revision
        }) {
            return Err(StagedLorebookWriterRunRepositoryError::Conflict.into());
        }
        let plan_ids = project
            .project
            .drafts
            .iter()
            .filter(|draft| draft.status == StagedLorebookDraftStatus::Drafting)
            .map(|draft| draft.plan_id)
            .collect::<Vec<_>>();
        let mut writers = Vec::with_capacity(plan_ids.len());
        for plan_id in plan_ids {
            let batch = project
                .project
                .draft_batch
                .as_ref()
                .ok_or(StagedLorebookWriterAdmissionError::InvalidInput)?;
            let request_id = RequestId::from_uuid(Uuid::new_v5(
                &project.project.id.as_uuid(),
                format!("writer-{plan_id}-{}", batch.revision.get()).as_bytes(),
            ));
            writers.push(
                self.admit_writer(WriterAdmissionInput {
                    prompt_snapshot: project
                        .writer_batch_inputs
                        .as_ref()
                        .and_then(|inputs| inputs.prompt_snapshot.clone())
                        .or_else(|| prompt_snapshot.clone()),
                    request_id,
                    project_request_id: project.request_id,
                    plan_id,
                    profile: profile.clone(),
                    prompt_id,
                    prompt_revision,
                    now: batch.started_at,
                })?,
            );
        }
        Ok(StagedLorebookWriterBatchAdmission { project, writers })
    }

    pub fn prepare_and_admit_configured_refinement(
        &self,
        request: StagedLorebookConfiguredRefineRequest,
        builtins: &crate::BuiltInPromptIds,
    ) -> Result<StagedLorebookWriterAdmission, StagedLorebookWriterAdmissionError>
    where
        P: lettuce_settings::GlobalSettingsStore
            + lettuce_models::ModelProfileRepository
            + lettuce_models::ProviderAccountRepository
            + lettuce_context::PromptRepository,
    {
        match self
            .runs
            .load_staged_lorebook_writer_run(request.request_id)
        {
            Ok(run) => {
                if run.project_request_id != request.project_request_id
                    || run.plan_id != request.plan_id
                    || run.created_at != request.now
                    || run.profile.safety_policy != request.safety_policy
                    || run.configured_overrides.as_ref() != Some(&request.overrides)
                    || run.refinement.as_ref().map(|value| value.feedback.as_str())
                        != Some(request.feedback.trim())
                {
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
        let coordinator = crate::StagedLorebookCoordinator::new(self.projects, self.jobs);
        let overrides =
            coordinator.project_overrides(request.project_request_id, &request.overrides)?;
        let (profile, prompt, _) = coordinator.resolve_configured_stage(
            &overrides,
            builtins,
            PromptPurpose::LorebookGeneratorRefine,
            request.safety_policy,
        )?;
        self.admit_refinement(
            StagedLorebookRefineRequest {
                request_id: request.request_id,
                project_request_id: request.project_request_id,
                plan_id: request.plan_id,
                feedback: request.feedback,
                profile,
                prompt: &prompt,
                now: request.now,
            },
            Some(request.overrides),
        )
    }

    pub fn prepare_and_admit_refinement(
        &self,
        request: StagedLorebookRefineRequest<'_>,
    ) -> Result<StagedLorebookWriterAdmission, StagedLorebookWriterAdmissionError> {
        self.admit_refinement(request, None)
    }

    fn admit_refinement(
        &self,
        request: StagedLorebookRefineRequest<'_>,
        configured_overrides: Option<lettuce_settings::LorebookGeneratorSelection>,
    ) -> Result<StagedLorebookWriterAdmission, StagedLorebookWriterAdmissionError> {
        validate_refine_request(&request)?;
        match self
            .runs
            .load_staged_lorebook_writer_run(request.request_id)
        {
            Ok(run) => {
                if !same_refine_request(&run, &request)
                    || configured_overrides.as_ref().is_some_and(|overrides| {
                        run.configured_overrides.as_ref() != Some(overrides)
                    })
                {
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
        let (values, refinement) =
            prepare_refine_values(&project, request.plan_id, &request.feedback)?;
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::CreationRun,
                JobSubject::new(SubjectKind::CreationProject, project.project.id.to_string())
                    .map_err(|_| StagedLorebookWriterAdmissionError::InvalidInput)?,
                OutcomeRef::Request(request.request_id),
            )
            .with_idempotency_key(
                IdempotencyKey::new(format!("staged-lorebook-refine-{}", request.request_id))
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
        let run = self
            .runs
            .admit_staged_lorebook_writer_run(StagedLorebookWriterRun {
                prompt_snapshot: Some(request.prompt.clone()),
                configured_overrides,
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
                refinement: Some(refinement),
                created_at: request.now,
                attempt: None,
            })?;
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
    validate_writer_inputs(&request.profile, request.prompt)
}

fn validate_writer_inputs(
    profile: &ResolvedInferenceProfile,
    prompt: &PromptDocument,
) -> Result<(), StagedLorebookWriterAdmissionError> {
    if prompt.validate().is_err()
        || prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::LorebookGeneratorWriter
        || prompt.revision.get() == 0
        || profile
            .chat_profile
            .capabilities
            .input_modalities
            .get(Modality::Text)
            != CapabilityStatus::Supported
        || profile
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

fn validate_refine_request(
    request: &StagedLorebookRefineRequest<'_>,
) -> Result<(), StagedLorebookWriterAdmissionError> {
    if request.feedback.trim().is_empty()
        || request.prompt.validate().is_err()
        || request.prompt.status != LifecycleStatus::Active
        || request.prompt.purpose != PromptPurpose::LorebookGeneratorRefine
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
    if !matches!(
        draft.status,
        StagedLorebookDraftStatus::Pending | StagedLorebookDraftStatus::Drafting
    ) {
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
        entry_keywords: String::new(),
        entry_always_active: String::new(),
        entry_content: String::new(),
        user_feedback: String::new(),
    })
}

fn prepare_refine_values(
    run: &StagedLorebookPlanningRun,
    plan_id: LorebookEntryId,
    feedback: &str,
) -> Result<
    (StagedLorebookWriterPromptValues, StagedLorebookRefinement),
    StagedLorebookWriterAdmissionError,
> {
    if !matches!(
        run.project.stage,
        StagedLorebookStage::Drafting | StagedLorebookStage::DraftsReady
    ) {
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
        .cloned()
        .ok_or(StagedLorebookWriterAdmissionError::InvalidInput)?;
    let feedback = feedback.trim().to_owned();
    let values = StagedLorebookWriterPromptValues {
        brief: run.project.brief.clone(),
        outline: format_outline(&run.project.outline),
        entry_title: draft.title.clone(),
        entry_category: String::new(),
        entry_proposed_keys: String::new(),
        entry_rationale: String::new(),
        relevant_excerpts: relevant_excerpts(plan, &run.project.excerpts),
        entry_keywords: format_keys(&draft.keywords),
        entry_always_active: draft.always_active.to_string(),
        entry_content: draft.content.clone(),
        user_feedback: feedback.clone(),
    };
    Ok((
        values,
        StagedLorebookRefinement {
            feedback,
            base_draft: draft,
        },
    ))
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

fn same_request(run: &StagedLorebookWriterRun, request: &WriterAdmissionInput) -> bool {
    run.request_id == request.request_id
        && run.project_request_id == request.project_request_id
        && run.plan_id == request.plan_id
        && run.profile == request.profile
        && run.prompt_id == request.prompt_id
        && run.prompt_revision == request.prompt_revision
        && run.created_at == request.now
        && run.refinement.is_none()
}

fn same_refine_request(
    run: &StagedLorebookWriterRun,
    request: &StagedLorebookRefineRequest<'_>,
) -> bool {
    run.request_id == request.request_id
        && run.project_request_id == request.project_request_id
        && run.plan_id == request.plan_id
        && run.profile == request.profile
        && run.prompt_id == request.prompt.id
        && run.prompt_revision == request.prompt.revision
        && run.created_at == request.now
        && run
            .refinement
            .as_ref()
            .is_some_and(|refinement| refinement.feedback == request.feedback.trim())
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
