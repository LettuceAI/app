use super::lorebook_jobs::{failure, prompt, replay, request_id, text_profile};
use super::lorebooks::revision;
use super::{
    ApiContext,
    error::{api_error, parse_id},
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_creation::{
    StagedLorebookPlanningRun, StagedLorebookRepository, StagedLorebookStage as Stage,
};
use lettuce_jobs::{JobState, JobStore};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{CreationWorkflowId, LorebookEntryId, PageRequest, RequestId};

fn load(context: &ApiContext, project: &str) -> Result<StagedLorebookPlanningRun, ApiError> {
    parse_id::<CreationWorkflowId>(project, "project_id")?;
    let db = context.backend().database();
    let id = db
        .staged_lorebook_request_for_project(project)
        .map_err(failure)?;
    db.load_staged_lorebook(id).map_err(failure)
}

fn view(
    context: &ApiContext,
    run: &StagedLorebookPlanningRun,
) -> Result<dto::LorebookProjectView, ApiError> {
    let db = context.backend().database();
    let project = &run.project;
    let planner = db
        .get(run.job_id)
        .map_err(failure)?
        .ok_or_else(|| api_error(ApiErrorCode::Internal, "planner job missing"))?;
    let failed = matches!(planner.state, JobState::Failed | JobState::Interrupted)
        && project.stage == Stage::Planning;
    let mut jobs = vec![run.job_id];
    jobs.extend(run.coherence_runs.iter().map(|run| run.job_id));
    for draft in &project.drafts {
        let id = RequestId::from_uuid(uuid::Uuid::new_v5(
            &project.id.as_uuid(),
            format!(
                "writer-{}-{}",
                draft.plan_id,
                project
                    .draft_batch
                    .as_ref()
                    .map_or(0, |batch| batch.revision.get())
            )
            .as_bytes(),
        ));
        if let Ok(writer) =
            lettuce_creation::StagedLorebookWriterRunRepository::load_staged_lorebook_writer_run(
                db, id,
            )
        {
            jobs.push(writer.job_id);
        }
    }
    let active_job_ids = jobs
        .into_iter()
        .map(|id| db.get(id).map_err(failure))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .filter(|job| !job.state.is_terminal())
        .map(|job| job.id.to_string())
        .collect();
    let coherence_changes = project
        .coherence_proposals
        .iter()
        .map(|change| {
            use dto::LorebookCoherenceChangeView as D;
            use lettuce_creation::StagedLorebookCoherenceChange as C;
            match change {
                C::MergeKeys {
                    id,
                    plan_id,
                    remove_keys,
                    reason,
                } => D::MergeKeys {
                    id: id.clone(),
                    plan_id: plan_id.to_string(),
                    remove_keys: remove_keys.clone(),
                    reason: reason.clone(),
                },
                C::RenameTerm {
                    id,
                    old_term,
                    new_term,
                    target_plan_ids,
                    reason,
                } => D::RenameTerm {
                    id: id.clone(),
                    old_term: old_term.clone(),
                    new_term: new_term.clone(),
                    plan_ids: target_plan_ids
                        .as_ref()
                        .map(|ids| ids.iter().map(ToString::to_string).collect()),
                    reason: reason.clone(),
                },
                C::FlagContradiction {
                    id,
                    plan_ids,
                    description,
                } => D::FlagContradiction {
                    id: id.clone(),
                    plan_ids: plan_ids.iter().map(ToString::to_string).collect(),
                    description: description.clone(),
                },
                C::ToggleAlwaysActive {
                    id,
                    plan_id,
                    new_value,
                    reason,
                } => D::ToggleAlwaysActive {
                    id: id.clone(),
                    plan_id: plan_id.to_string(),
                    new_value: *new_value,
                    reason: reason.clone(),
                },
            }
        })
        .collect();
    Ok(dto::LorebookProjectView {
        project_id: project.id.to_string(),
        brief: project.brief.clone(),
        lorebook_name: project.initial_lorebook_name.clone(),
        target_count: project.target_count,
        stage: if failed {
            dto::LorebookProjectStage::PlanFailed
        } else {
            match project.stage {
                Stage::Created => dto::LorebookProjectStage::Created,
                Stage::Planning => dto::LorebookProjectStage::Planning,
                Stage::AwaitingOutlineApproval => {
                    dto::LorebookProjectStage::AwaitingOutlineApproval
                }
                Stage::Drafting => dto::LorebookProjectStage::Drafting,
                Stage::DraftsReady => dto::LorebookProjectStage::DraftsReady,
                Stage::CoherenceReview => dto::LorebookProjectStage::CoherenceReview,
                Stage::Committed => dto::LorebookProjectStage::Committed,
                Stage::Cancelled => dto::LorebookProjectStage::Cancelled,
            }
        },
        sources: project
            .excerpts
            .iter()
            .map(|source| dto::LorebookProjectSourceView {
                source_id: source.source_id.clone(),
                label: source.label.clone(),
                document: source.asset_id.map(|id| context.asset_ref(id)),
            })
            .collect(),
        outline: project
            .outline
            .iter()
            .map(|plan| dto::LorebookPlanView {
                plan_id: plan.id.to_string(),
                title: plan.title.clone(),
                category: plan.category.clone(),
                proposed_keys: plan.proposed_keys.clone(),
                rationale: plan.rationale.clone(),
                source_refs: plan.source_refs.clone(),
            })
            .collect(),
        drafts: project
            .drafts
            .iter()
            .map(|draft| dto::LorebookDraftView {
                plan_id: draft.plan_id.to_string(),
                title: draft.title.clone(),
                keywords: draft.keywords.clone(),
                content: draft.content.clone(),
                always_active: draft.always_active,
                status: match draft.status {
                    lettuce_creation::StagedLorebookDraftStatus::Pending => {
                        dto::LorebookDraftStatus::Pending
                    }
                    lettuce_creation::StagedLorebookDraftStatus::Drafting => {
                        dto::LorebookDraftStatus::Drafting
                    }
                    lettuce_creation::StagedLorebookDraftStatus::Drafted => {
                        dto::LorebookDraftStatus::Drafted
                    }
                    lettuce_creation::StagedLorebookDraftStatus::Approved => {
                        dto::LorebookDraftStatus::Approved
                    }
                    lettuce_creation::StagedLorebookDraftStatus::Failed => {
                        dto::LorebookDraftStatus::Failed
                    }
                },
                revisions: draft
                    .revisions
                    .iter()
                    .map(|revision| dto::LorebookDraftRevisionView {
                        feedback: revision.feedback.clone(),
                        content: revision.content.clone(),
                        at: revision.timestamp.get(),
                    })
                    .collect(),
            })
            .collect(),
        coherence_changes,
        commit: project
            .commit_receipt
            .as_ref()
            .map(|receipt| {
                Ok(dto::LorebookProjectCommitView {
                    lorebook_id: receipt.lorebook_id.to_string(),
                    lorebook_name: receipt.lorebook_name.clone(),
                    lorebook_deleted: lettuce_context::LorebookRepository::get(
                        db,
                        receipt.lorebook_id,
                    )
                    .map_err(failure)?
                    .is_none(),
                    entry_ids: receipt
                        .created_entry_ids
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                })
            })
            .transpose()?,
        active_job_ids,
        plan_failure: if failed {
            super::jobs::job_view(context, &planner)?.failure
        } else {
            None
        },
        revision: project.revision.get(),
        created_at: project.created_at.get(),
        updated_at: project.updated_at.get(),
    })
}

pub async fn lorebook_project_create(
    context: &ApiContext,
    request: dto::LorebookProjectCreateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = request_id(
        "lorebook_project_create",
        &request.client_operation_id,
        &request,
    )?;
    let project_id = CreationWorkflowId::from_uuid(id.as_uuid());
    context
        .blocking(move |context| {
            let db = context.backend().database();
            let sources = request
                .sources
                .iter()
                .map(|source| match source {
                    dto::LorebookProjectSourceInput::Text { label, text } => {
                        Ok(crate::StagedLorebookIntakeSource::Text { label, body: text })
                    }
                    dto::LorebookProjectSourceInput::Document { asset_id } => {
                        Ok(crate::StagedLorebookIntakeSource::Document {
                            asset_id: parse_id(asset_id, "asset_id")?,
                            label: asset_id,
                        })
                    }
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let excerpts = if sources
                .iter()
                .any(|source| matches!(source, crate::StagedLorebookIntakeSource::Document { .. }))
            {
                crate::prepare_staged_lorebook_intake(
                    context.media().ok_or_else(|| {
                        api_error(ApiErrorCode::Unavailable, "media store unavailable")
                    })?,
                    &sources,
                )
                .map_err(failure)?
            } else {
                let inputs = sources
                    .iter()
                    .filter_map(|source| match source {
                        crate::StagedLorebookIntakeSource::Text { label, body } => {
                            Some(lettuce_creation::StagedLorebookSourceInput::Text { label, body })
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                lettuce_creation::prepare_staged_lorebook_sources(&inputs).map_err(failure)?
            };
            let stored = GlobalSettingsStore::load(db).map_err(failure)?;
            let project = lettuce_creation::StagedLorebookProject::create(
                project_id,
                request.brief,
                request.lorebook_name,
                request
                    .target_count
                    .unwrap_or(stored.settings.lorebook_generator.target_count()),
                excerpts,
                context.now(),
            )
            .map_err(failure)?;
            let project = db
                .commit_api_operation(
                    "lorebook_project_create",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .create_lorebook_project(project)
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            Ok(pending_view(context, &project))
        })
        .await
}

fn pending_view(
    context: &ApiContext,
    project: &lettuce_creation::StagedLorebookProject,
) -> dto::LorebookProjectView {
    dto::LorebookProjectView {
        project_id: project.id.to_string(),
        brief: project.brief.clone(),
        lorebook_name: project.initial_lorebook_name.clone(),
        target_count: project.target_count,
        stage: if project.stage == Stage::Cancelled {
            dto::LorebookProjectStage::Cancelled
        } else {
            dto::LorebookProjectStage::Created
        },
        sources: project
            .excerpts
            .iter()
            .map(|source| dto::LorebookProjectSourceView {
                source_id: source.source_id.clone(),
                label: source.label.clone(),
                document: source.asset_id.map(|id| context.asset_ref(id)),
            })
            .collect(),
        outline: vec![],
        drafts: vec![],
        coherence_changes: vec![],
        commit: None,
        active_job_ids: vec![],
        plan_failure: None,
        revision: project.revision.get(),
        created_at: project.created_at.get(),
        updated_at: project.updated_at.get(),
    }
}

fn project_view(
    context: &ApiContext,
    project_id: &str,
) -> Result<dto::LorebookProjectView, ApiError> {
    let id = parse_id(project_id, "project_id")?;
    if let Some(project) = context
        .backend()
        .database()
        .pending_lorebook_project(id)
        .map_err(failure)?
    {
        return Ok(pending_view(context, &project));
    }
    view(context, &load(context, project_id)?)
}

pub async fn lorebook_project_get(
    context: &ApiContext,
    request: dto::LorebookProjectGetRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    context
        .blocking(move |context| project_view(context, &request.project_id))
        .await
}

pub async fn lorebook_projects_list(
    context: &ApiContext,
    request: dto::LorebookProjectsListRequest,
) -> Result<dto::LorebookProjectPage, ApiError> {
    context
        .blocking(move |context| {
            let page = context
                .backend()
                .database()
                .lorebook_project_page(PageRequest {
                    cursor: request.cursor,
                    limit: super::mapping::page_limit(request.limit),
                })
                .map_err(failure)?;
            let items = page
                .items
                .iter()
                .map(|id| {
                    project_view(context, &id.to_string()).map(|view| dto::LorebookProjectSummary {
                        project_id: view.project_id,
                        brief: view.brief,
                        lorebook_name: view.lorebook_name,
                        stage: view.stage,
                        updated_at: view.updated_at,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(dto::LorebookProjectPage {
                items,
                next_cursor: page.next_cursor,
            })
        })
        .await
}

pub async fn lorebook_project_plan(
    context: &ApiContext,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let retry = request_id(
        "lorebook_project_plan",
        &request.client_operation_id,
        &request,
    )?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let job = context
        .blocking(move |context| {
            let db = context.backend().database();
            if let Some(receipt) = db
                .lookup_api_operation("lorebook_project_plan", &request.client_operation_id)
                .map_err(failure)?
            {
                if receipt.request_digest != digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the operation key was reused",
                    ));
                }
                return serde_json::from_value(receipt.result).map_err(failure);
            }
            let project_id = parse_id(&request.project_id, "project_id")?;
            if let Some(project) = db.pending_lorebook_project(project_id).map_err(failure)? {
                if project.revision != expected {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the project changed since it was read",
                    ));
                }
                let stored = GlobalSettingsStore::load(db).map_err(failure)?;
                let selected = crate::select_staged_lorebook_settings(
                    &stored,
                    &Default::default(),
                    context.backend().built_in_prompt_ids(),
                );
                let profile = text_profile(
                    context,
                    stored
                        .settings
                        .lorebook_generator
                        .selection
                        .model_profile_id,
                    true,
                )?;
                let prompt = prompt(
                    context,
                    selected.planner_prompt_id.ok_or_else(|| {
                        api_error(ApiErrorCode::Unavailable, "planner prompt missing")
                    })?,
                    lettuce_context::PromptPurpose::LorebookGeneratorPlanner,
                )?;
                let project = project.start_planning(context.now()).map_err(failure)?;
                let id = RequestId::from_uuid(project.id.as_uuid());
                let spec = lettuce_jobs::JobSpec::new(
                    lettuce_jobs::JobKind::CreationRun,
                    lettuce_jobs::JobSubject::new(
                        lettuce_jobs::SubjectKind::CreationProject,
                        project.id.to_string(),
                    )
                    .map_err(failure)?,
                    lettuce_jobs::OutcomeRef::Request(id),
                )
                .with_idempotency_key(
                    lettuce_jobs::IdempotencyKey::new(format!("staged-lorebook-{id}"))
                        .map_err(failure)?,
                )
                .with_resources(vec![
                    lettuce_jobs::ResourceClass::Network,
                    lettuce_jobs::ResourceClass::ModelLoad,
                    lettuce_jobs::ResourceClass::DiskRead,
                    lettuce_jobs::ResourceClass::DiskWrite,
                    lettuce_jobs::ResourceClass::Cpu,
                ])
                .with_priority(lettuce_jobs::JobPriority::Interactive)
                .with_policies(
                    lettuce_jobs::RecoveryPolicy::Restart,
                    lettuce_jobs::CancellationPolicy::Cooperative,
                );
                let run = StagedLorebookPlanningRun {
                    request_id: id,
                    job_id: lettuce_types::JobId::new(),
                    project,
                    planner_profile: profile,
                    planner_prompt_id: prompt.id,
                    planner_prompt_name: prompt.name.clone(),
                    planner_prompt_revision: prompt.revision,
                    planner_prompt_snapshot: Some(prompt),
                    configured_inputs: None,
                    writer_batch_inputs: None,
                    planner_attempt: None,
                    planner_retries: vec![],
                    coherence_runs: vec![],
                };
                return db
                    .commit_api_operation(
                        "lorebook_project_plan",
                        &request.client_operation_id,
                        &digest,
                        context.now(),
                        |scope| {
                            scope
                                .plan_pending_lorebook_project(expected, spec, run)
                                .map_err(|error| super::lorebooks::Failure(failure(error)))
                        },
                    )
                    .map_err(|error| error.0);
            }
            let run = load(context, &request.project_id)?;
            context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_plan",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let next = if run.project.stage == Stage::Created {
                            scope.start_staged_lorebook_planning(
                                run.request_id,
                                expected,
                                context.now(),
                            )
                        } else {
                            scope.retry_staged_lorebook_planner(
                                run.request_id,
                                retry,
                                expected,
                                context.now(),
                            )
                        }
                        .map_err(|error| super::lorebooks::Failure(failure(error)))?;
                        Ok::<_, super::lorebooks::Failure>(next.job_id)
                    },
                )
                .map_err(|error| error.0)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.to_string(),
    })
}

pub async fn lorebook_project_outline_update(
    context: &ApiContext,
    request: dto::LorebookProjectOutlineUpdateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let outline = request
        .outline
        .into_iter()
        .enumerate()
        .map(|(ordinal, plan)| {
            Ok(lettuce_creation::StagedLorebookEntryPlan {
                id: plan
                    .plan_id
                    .as_deref()
                    .map(|id| parse_id(id, "plan_id"))
                    .transpose()?
                    .unwrap_or_else(LorebookEntryId::new),
                ordinal: u32::try_from(ordinal).map_err(failure)?,
                title: plan.title,
                category: plan.category,
                proposed_keys: plan.proposed_keys,
                rationale: plan.rationale,
                source_refs: plan.source_refs,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    context
        .blocking(move |context| {
            let run = load(context, &request.project_id)?;
            let next = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_outline_update",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .edit_staged_lorebook_outline(
                                run.request_id,
                                expected,
                                outline,
                                context.now(),
                            )
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &next)
        })
        .await
}

pub async fn lorebook_project_outline_approve(
    context: &ApiContext,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            let run = load(context, &request.project_id)?;
            let next = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_outline_approve",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .approve_staged_lorebook_outline(
                                run.request_id,
                                expected,
                                context.now(),
                            )
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &next)
        })
        .await
}

pub async fn lorebook_project_draft_update(
    context: &ApiContext,
    request: dto::LorebookProjectDraftUpdateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let plan_id = parse_id(&request.plan_id, "plan_id")?;
    context
        .blocking(move |context| {
            let run = load(context, &request.project_id)?;
            let edit = lettuce_creation::StagedLorebookDraftEdit {
                plan_id,
                title: request.title,
                keywords: request.keywords,
                content: request.content,
                always_active: request.always_active,
            };
            let next = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_draft_update",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .edit_staged_lorebook_draft(
                                run.request_id,
                                expected,
                                edit,
                                context.now(),
                            )
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &next)
        })
        .await
}

pub async fn lorebook_project_draft_set_approved(
    context: &ApiContext,
    request: dto::LorebookProjectDraftApprovalRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let plan_id = parse_id(&request.plan_id, "plan_id")?;
    context
        .blocking(move |context| {
            let run = load(context, &request.project_id)?;
            let next = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_draft_set_approved",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .set_staged_lorebook_draft_approved(
                                run.request_id,
                                expected,
                                plan_id,
                                request.approved,
                                context.now(),
                            )
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &next)
        })
        .await
}

pub async fn lorebook_project_coherence_apply(
    context: &ApiContext,
    request: dto::LorebookProjectCoherenceApplyRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            let run = load(context, &request.project_id)?;
            let next = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_coherence_apply",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .apply_staged_lorebook_coherence(
                                run.request_id,
                                expected,
                                request.accepted_change_ids,
                                context.now(),
                            )
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &next)
        })
        .await
}

pub async fn lorebook_project_cancel(
    context: &ApiContext,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            let id = parse_id(&request.project_id, "project_id")?;
            let db = context.backend().database();
            if db.pending_lorebook_project(id).map_err(failure)?.is_some() {
                let next = db
                    .commit_api_operation(
                        "lorebook_project_cancel",
                        &request.client_operation_id,
                        &digest,
                        context.now(),
                        |scope| {
                            scope
                                .cancel_pending_lorebook_project(id, expected, context.now())
                                .map_err(|error| super::lorebooks::Failure(failure(error)))
                        },
                    )
                    .map_err(|error| error.0)?;
                return Ok(pending_view(context, &next));
            }
            let run = load(context, &request.project_id)?;
            let next = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_cancel",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .cancel_staged_lorebook(run.request_id, expected, context.now())
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &next)
        })
        .await
}

pub async fn lorebook_generator_defaults(
    context: &ApiContext,
) -> Result<dto::LorebookGeneratorDefaults, ApiError> {
    context
        .blocking(|context| {
            let settings =
                GlobalSettingsStore::load(context.backend().database()).map_err(failure)?;
            Ok(dto::LorebookGeneratorDefaults {
                target_count: settings.settings.lorebook_generator.target_count(),
                min_target_count: lettuce_creation::MIN_STAGED_LOREBOOK_TARGET_COUNT,
                max_target_count: lettuce_creation::MAX_STAGED_LOREBOOK_TARGET_COUNT,
            })
        })
        .await
}

fn overrides(
    context: &ApiContext,
) -> Result<lettuce_settings::LorebookGeneratorSelection, ApiError> {
    let stored = GlobalSettingsStore::load(context.backend().database()).map_err(failure)?;
    let mut selected = crate::select_staged_lorebook_settings(
        &stored,
        &Default::default(),
        context.backend().built_in_prompt_ids(),
    );
    selected.model_profile_id = Some(
        text_profile(
            context,
            stored
                .settings
                .lorebook_generator
                .selection
                .model_profile_id,
            true,
        )?
        .chat_profile
        .model_profile_id,
    );
    Ok(selected)
}

pub async fn lorebook_project_draft_next(
    context: &ApiContext,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::LorebookProjectBatchAccepted, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let result = context
        .blocking(move |context| {
            let db = context.backend().database();
            if let Some(receipt) = db
                .lookup_api_operation("lorebook_project_draft_next", &request.client_operation_id)
                .map_err(failure)?
            {
                if receipt.request_digest != digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the operation key was reused",
                    ));
                }
                return serde_json::from_value(receipt.result).map_err(failure);
            }
            let run = load(context, &request.project_id)?;
            if run
                .project
                .drafts
                .iter()
                .any(|draft| draft.status == lettuce_creation::StagedLorebookDraftStatus::Drafting)
            {
                return Err(ApiError {
                    code: ApiErrorCode::Busy,
                    message: "a writer batch is running".into(),
                    details: Some(dto::ApiErrorDetails::LorebookBatchRunning {
                        job_ids: view(context, &run)?.active_job_ids,
                    }),
                });
            }
            if run.project.revision != expected {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "the project changed since it was read",
                ));
            }
            let selected = overrides(context)?;
            prompt(
                context,
                selected
                    .writer_prompt_id
                    .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "writer prompt missing"))?,
                lettuce_context::PromptPurpose::LorebookGeneratorWriter,
            )?;
            let (inputs, writers) = context
                .backend()
                .staged_lorebook_writer_coordinator()
                .prepare_atomic_batch(
                    &run,
                    &selected,
                    context.backend().built_in_prompt_ids(),
                    context.now(),
                )
                .map_err(failure)?;
            db.commit_api_operation(
                "lorebook_project_draft_next",
                &request.client_operation_id,
                &digest,
                context.now(),
                |scope| {
                    scope
                        .admit_staged_lorebook_batch(
                            run.request_id,
                            expected,
                            inputs,
                            writers,
                            context.now(),
                        )
                        .map(|jobs| dto::LorebookProjectBatchAccepted {
                            job_ids: jobs.into_iter().map(|id| id.to_string()).collect(),
                        })
                        .map_err(|error| super::lorebooks::Failure(failure(error)))
                },
            )
            .map_err(|error| error.0)
        })
        .await?;
    context.jobs().wake();
    Ok(result)
}

pub async fn lorebook_project_refine(
    context: &ApiContext,
    request: dto::LorebookProjectRefineRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let key =
        super::jobs::local::operation_key("lorebook_project_refine", &request.client_operation_id)?;
    let digest = super::jobs::local::digest(&request)?;
    let id = request_id(
        "lorebook_project_refine",
        &request.client_operation_id,
        &request,
    )?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let plan_id: LorebookEntryId = parse_id(&request.plan_id, "plan_id")?;
    let job = context
        .blocking(move |context| {
            if let Some(id) = replay(context, &key, &digest)? {
                return Ok(id);
            }
            let run = load(context, &request.project_id)?;
            let selected = overrides(context)?;
            prompt(
                context,
                selected
                    .refine_prompt_id
                    .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "refine prompt missing"))?,
                lettuce_context::PromptPurpose::LorebookGeneratorRefine,
            )?;
            context
                .backend()
                .staged_lorebook_writer_coordinator()
                .with_operation(key, digest)
                .with_project_revision(expected)
                .prepare_and_admit_configured_refinement(
                    crate::StagedLorebookConfiguredRefineRequest {
                        request_id: id,
                        project_request_id: run.request_id,
                        plan_id,
                        feedback: request.feedback,
                        overrides: selected,
                        safety_policy: lettuce_conversations::SafetyContext::Standard,
                        now: context.now(),
                    },
                    context.backend().built_in_prompt_ids(),
                )
                .map(|admission| admission.job.id)
                .map_err(failure)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.to_string(),
    })
}

pub async fn lorebook_project_coherence(
    context: &ApiContext,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let key = super::jobs::local::operation_key(
        "lorebook_project_coherence",
        &request.client_operation_id,
    )?;
    let digest = super::jobs::local::digest(&request)?;
    let id = request_id(
        "lorebook_project_coherence",
        &request.client_operation_id,
        &request,
    )?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let job = context
        .blocking(move |context| {
            if let Some(id) = replay(context, &key, &digest)? {
                return Ok(id);
            }
            let run = load(context, &request.project_id)?;
            if run.project.revision != expected {
                return Err(api_error(ApiErrorCode::Conflict, "the project changed"));
            }
            let selected = overrides(context)?;
            prompt(
                context,
                selected.coherence_prompt_id.ok_or_else(|| {
                    api_error(ApiErrorCode::Unavailable, "coherence prompt missing")
                })?,
                lettuce_context::PromptPurpose::LorebookGeneratorCoherence,
            )?;
            context
                .backend()
                .staged_lorebook_coordinator()
                .with_operation(key, digest)
                .with_project_revision(expected)
                .admit_configured_coherence(
                    crate::StagedLorebookConfiguredCoherenceRequest {
                        request_id: id,
                        project_request_id: run.request_id,
                        overrides: selected,
                        safety_policy: lettuce_conversations::SafetyContext::Standard,
                        now: context.now(),
                    },
                    context.backend().built_in_prompt_ids(),
                )
                .map(|admission| admission.job.id)
                .map_err(failure)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.to_string(),
    })
}

pub async fn lorebook_project_commit(
    context: &ApiContext,
    request: dto::LorebookProjectCommitRequest,
) -> Result<dto::LorebookProjectCommitView, ApiError> {
    let id = request_id(
        "lorebook_project_commit",
        &request.client_operation_id,
        &request,
    )?;
    let digest = super::jobs::local::digest(&request)?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = match request.target {
        dto::LorebookProjectCommitTarget::NewLorebook { name } => {
            lettuce_creation::StagedLorebookCommitTarget::New {
                id: lettuce_types::LorebookId::from_uuid(id.as_uuid()),
                name,
            }
        }
        dto::LorebookProjectCommitTarget::ExistingLorebook {
            lorebook_id,
            expected_revision,
        } => lettuce_creation::StagedLorebookCommitTarget::Existing {
            id: parse_id(&lorebook_id, "lorebook_id")?,
            expected_revision: revision(expected_revision, "target.expected_revision")?,
        },
    };
    context
        .blocking(move |context| {
            let run = load(context, &request.project_id)?;
            let receipt = context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_project_commit",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .commit_staged_lorebook(lettuce_creation::StagedLorebookCommitRequest {
                                project_request_id: run.request_id,
                                expected_project_revision: expected,
                                target,
                                now: context.now(),
                            })
                            .map_err(|error| super::lorebooks::Failure(failure(error)))
                    },
                )
                .map_err(|error| error.0)?;
            Ok(dto::LorebookProjectCommitView {
                lorebook_id: receipt.lorebook_id.to_string(),
                lorebook_name: receipt.lorebook_name,
                lorebook_deleted: lettuce_context::LorebookRepository::get(
                    context.backend().database(),
                    receipt.lorebook_id,
                )
                .map_err(failure)?
                .is_none(),
                entry_ids: receipt
                    .created_entry_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
            })
        })
        .await
}
