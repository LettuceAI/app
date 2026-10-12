use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ClaimRef, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy,
    ResourceAvailability, ResourceClass, SubjectKind, WorkerId, handle::CancellationToken,
};
use lettuce_types::{CharacterId, RequestId};

use super::{
    ApiContext,
    entity_files::{ImportFile, read_with_progress, stem, text},
    error::{IntoApiError, api_error, invalid_field, parse_id},
    jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink},
    lorebooks::Failure,
};

fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}

fn prepare(
    context: &ApiContext,
    file: &ImportFile,
    options: &dto::ImportOptions,
) -> Result<crate::PreparedChatFile, ApiError> {
    let raw = text(file)?;
    let chat = lettuce_transfer::parse_chat_jsonl(raw, context.now().get())
        .map_err(|e| invalid_field("source", e.to_string()))?;
    if chat.is_group() {
        let mapping = options
            .speakers
            .iter()
            .map(|(name, id)| {
                Ok((
                    name.clone(),
                    parse_id::<CharacterId>(id, "options.speakers")?,
                ))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, ApiError>>()?;
        for speaker in chat.speakers() {
            if !mapping.contains_key(&speaker) {
                return Err(invalid_field(
                    "options.speakers",
                    format!("unmapped speaker: {speaker}"),
                ));
            }
        }
        context
            .backend()
            .chat_files()
            .prepare_group(chat, &mapping, context.now())
            .map_err(IntoApiError::into_api_error)
    } else {
        let id = options
            .character_id
            .as_deref()
            .map(|id| parse_id(id, "options.character_id"))
            .transpose()?;
        context
            .backend()
            .chat_files()
            .prepare_direct(chat, stem(&file.name), id, context.now())
            .map_err(IntoApiError::into_api_error)
    }
}

pub(super) async fn admit(
    context: &ApiContext,
    request: dto::ImportCommitRequest,
    digest: String,
) -> Result<dto::ImportCommitResult, ApiError> {
    let result = context
        .blocking(move |context| {
            let preview = context
                .chat_previews()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(&request.digest)
                .cloned()
                .ok_or_else(|| invalid_field("source", "preview the transcript before importing"))?;
            let mut ids = Vec::new();
            if preview.group {
                let mapping = request.options.speakers.iter().map(|(name, id)| {
                    Ok((name, parse_id::<CharacterId>(id, "options.speakers")?))
                }).collect::<Result<std::collections::BTreeMap<_, _>, ApiError>>()?;
                if preview.speakers.is_empty() {
                    return Err(invalid_field("options.speakers", "a group requires characters"));
                }
                for speaker in &preview.speakers {
                    let id = mapping.get(speaker)
                        .ok_or_else(|| invalid_field("options.speakers", format!("unmapped speaker: {speaker}")))?;
                    ids.push(*id);
                }
            } else {
                let id = request.options.character_id.as_deref()
                    .ok_or_else(|| invalid_field("options.character_id", "target character is required"))?;
                ids.push(parse_id::<CharacterId>(id, "options.character_id")?);
            }
            for id in ids {
                lettuce_characters::CharacterRepository::get(context.backend().database(), id)
                    .map_err(IntoApiError::into_api_error)?
                    .filter(|details| details.character.status == lettuce_characters::LifecycleStatus::Active)
                    .ok_or_else(|| invalid_field("options", "target character is not active"))?;
            }
            let database = context.backend().database();
            let detail = serde_json::to_value(&request).map_err(internal)?;
            let spec = JobSpec::new(
                JobKind::TransferImport,
                JobSubject::new(SubjectKind::TransferPlan, "entity-chat-import")
                    .map_err(internal)?,
                OutcomeRef::Request(RequestId::new()),
            )
            .with_resources(vec![ResourceClass::DiskWrite])
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative);
            database
                .commit_api_operation(
                    "import_commit",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        if context.shutdown_token().is_cancelled() {
                            return Err(Failure(api_error(
                                ApiErrorCode::Cancelled,
                                "import was cancelled",
                            )));
                        }
                        let job = scope
                            .admit_entity_import_job(
                                spec,
                                &format!("entity-import:{}", request.client_operation_id),
                                &digest,
                                &detail,
                            )
                            .map_err(|e| Failure(e.into_api_error()))?;
                        Ok::<_, Failure>(dto::ImportCommitResult::Job {
                            job_id: job.id.to_string(),
                        })
                    },
                )
                .map_err(|e| e.0)
        })
        .await?;
    context.jobs().wake();
    Ok(result)
}

pub(super) struct ImportHandler;

#[async_trait]
impl JobHandler for ImportHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::TransferImport]
    }
    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        (job.subject.id.as_str() == "entity-chat-import").then(|| JobLane("entity-import".into()))
    }
    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let at = context.now().max(job.updated_at);
                let Some(claim) = database
                    .claim(
                        job.id,
                        worker,
                        at,
                        Duration::from_secs(60 * 60),
                        &ResourceAvailability::all(),
                    )
                    .map_err(IntoApiError::into_api_error)?
                else {
                    return Ok(None);
                };
                if let Err(error) = database.append_and_transition(JobMutation::Start {
                    claim: claim.claim.clone(),
                    at,
                }) {
                    if database
                        .get(job.id)
                        .map_err(IntoApiError::into_api_error)?
                        .is_some_and(|job| job.state == JobState::CancellationRequested)
                    {
                        crate::models::artifact_install::finish_claimed_cancellation(
                            database,
                            &claim.claim,
                            at,
                        )
                        .map_err(IntoApiError::into_api_error)?;
                        return Ok(None);
                    }
                    return Err(error.into_api_error());
                }
                Ok(Some(Box::new(ImportClaim {
                    claim: claim.claim,
                    cancellation: CancellationToken::new(),
                }) as Box<dyn ClaimedJob>))
            })
            .await
    }
}

struct ImportClaim {
    claim: ClaimRef,
    cancellation: CancellationToken,
}

#[async_trait]
impl ClaimedJob for ImportClaim {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let result = context
            .blocking(move |context| {
                let database = context.backend().database();
                let run = (|| {
                    let detail = database
                        .job_detail(self.claim.job_id)
                        .map_err(internal)?
                        .ok_or_else(|| internal("import job detail is missing"))?;
                    let request: dto::ImportCommitRequest =
                        serde_json::from_value(detail.detail).map_err(internal)?;
                    if self.cancellation.is_cancelled() || context.shutdown_token().is_cancelled() {
                        return Err(api_error(ApiErrorCode::Cancelled, "import was cancelled"));
                    }
                    let total = context
                        .files()
                        .describe(&request.source.uri)
                        .map_err(IntoApiError::into_api_error)?
                        .size;
                    let file = read_with_progress(context, &request.source, &mut |completed| {
                        if self.cancellation.is_cancelled() {
                            return Err(api_error(ApiErrorCode::Cancelled, "import was cancelled"));
                        }
                        database
                            .append_and_transition(JobMutation::Progress {
                                claim: self.claim.clone(),
                                progress: lettuce_jobs::ProgressSnapshot {
                                    bytes: Some(
                                        lettuce_jobs::BytesProgress::new(
                                            completed,
                                            Some(total.max(completed)),
                                        )
                                        .map_err(internal)?,
                                    ),
                                    ..Default::default()
                                },
                                at: context.now(),
                            })
                            .map_err(IntoApiError::into_api_error)?;
                        Ok(())
                    })?;
                    if file.digest != request.digest {
                        return Err(api_error(ApiErrorCode::Conflict, "import file changed"));
                    }
                    let prepared = prepare(context, &file, &request.options)?;
                    let id = prepared.record.history.aggregate.conversation.id;
                    database
                        .apply_import_job(|scope| {
                            if self.cancellation.is_cancelled()
                                || context.shutdown_token().is_cancelled()
                            {
                                return Err(Failure(api_error(
                                    ApiErrorCode::Cancelled,
                                    "import was cancelled",
                                )));
                            }
                            scope
                                .import_chat(
                                    prepared.group,
                                    prepared.record,
                                    prepared.companion,
                                    context.now(),
                                )
                                .map_err(|e| Failure(internal(e)))?;
                            scope
                                .transition_import_job(JobMutation::Succeed {
                                    claim: self.claim.clone(),
                                    outcome: JobOutcome::Success {
                                        result_ref: OutcomeRef::Conversation(id),
                                    },
                                    at: context.now(),
                                })
                                .map_err(|e| Failure(e.into_api_error()))?;
                            if self.cancellation.is_cancelled()
                                || context.shutdown_token().is_cancelled()
                            {
                                return Err(Failure(api_error(
                                    ApiErrorCode::Cancelled,
                                    "import was cancelled",
                                )));
                            }
                            Ok::<_, Failure>(())
                        })
                        .map_err(|e| e.0)?;
                    Ok(())
                })();
                if let Err(error) = run {
                    let job = database
                        .get(self.claim.job_id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| internal("import job is missing"))?;
                    if job.state.is_terminal() {
                        return Ok(());
                    }
                    if context.shutdown_token().is_cancelled()
                        && job.state != JobState::CancellationRequested
                    {
                        return Ok(());
                    }
                    let at = context.now().max(job.updated_at);
                    if job.state == JobState::CancellationRequested
                        || error.code == ApiErrorCode::Cancelled
                    {
                        if job.state != JobState::CancellationRequested {
                            database
                                .append_and_transition(JobMutation::RequestCancellation {
                                    id: self.claim.job_id,
                                    reason: CancellationReason::Shutdown,
                                    at,
                                })
                                .map_err(IntoApiError::into_api_error)?;
                        }
                        crate::models::artifact_install::finish_claimed_cancellation(
                            database,
                            &self.claim,
                            at,
                        )
                        .map_err(IntoApiError::into_api_error)?;
                    } else {
                        database
                            .settle_job_with_detail(
                                JobMutation::Fail {
                                    claim: self.claim,
                                    error: JobError::new(
                                        if error.code == ApiErrorCode::InvalidInput {
                                            JobErrorCode::InvalidInput
                                        } else {
                                            JobErrorCode::ResourceUnavailable
                                        },
                                        false,
                                        "entity-import-failed",
                                    )
                                    .map_err(internal)?,
                                    at,
                                },
                                None,
                                Some(&serde_json::to_value(error).map_err(internal)?),
                            )
                            .map_err(IntoApiError::into_api_error)?;
                    }
                }
                Ok(())
            })
            .await;
        context.jobs().wake();
        result
    }
}

pub(super) fn failure_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<ApiError>, ApiError> {
    if job.kind != JobKind::TransferImport
        || job.subject.id.as_str() != "entity-chat-import"
        || job.error.is_none()
    {
        return Ok(None);
    }
    let detail = context
        .backend()
        .database()
        .job_detail(job.id)
        .map_err(internal)?
        .ok_or_else(|| internal("import job detail is missing"))?;
    let failure = detail
        .failure
        .ok_or_else(|| internal("import job failure is missing"))?;
    serde_json::from_value(failure).map(Some).map_err(internal)
}
