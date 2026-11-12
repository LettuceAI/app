use super::{ClaimedJob, JobHandler, JobLane, JobProgressSink, local::internal};
use crate::api::{ApiContext, creator::CreatorJobDetail, error::api_error};
use async_trait::async_trait;
use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_creation::{CreationAttemptRepository, CreationAttemptStatus};
use lettuce_jobs::{
    ClaimRef, JobError, JobErrorCode, JobKind, JobMutation, JobOutcome, JobSnapshot, JobState,
    JobStore, OutcomeRef, ResourceAvailability, WorkerId,
    handle::{CancellationToken, JobHandle},
};
use lettuce_types::RequestId;
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Copy)]
pub struct CreatorHandler;

#[async_trait]
impl JobHandler for CreatorHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::CreationRun]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        job.idempotency_key
            .as_ref()
            .filter(|key| key.as_str().starts_with("creator-"))
            .map(|_| JobLane(format!("creator:{}", job.subject.id.as_str())))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let id = job.id;
        context
            .blocking(move |context| {
                let db = context.backend().database();
                let record = db
                    .job_detail(id)
                    .map_err(internal)?
                    .ok_or_else(|| internal("creator job detail missing"))?;
                let detail: CreatorJobDetail =
                    serde_json::from_value(record.detail).map_err(internal)?;
                let Some(claim) = db
                    .claim(
                        id,
                        worker,
                        context.now(),
                        Duration::from_secs(3600),
                        &ResourceAvailability::all(),
                    )
                    .map_err(internal)?
                else {
                    return Ok(None);
                };
                db.append_and_transition(JobMutation::Start {
                    claim: claim.claim.clone(),
                    at: context.now(),
                })
                .map_err(internal)?;
                Ok(Some(Box::new(CreatorWork {
                    claim: claim.claim,
                    detail,
                    cancellation: CancellationToken::new(),
                }) as Box<dyn ClaimedJob>))
            })
            .await
    }
}

struct CreatorWork {
    claim: ClaimRef,
    detail: CreatorJobDetail,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct CreatorRoundObserver<'a> {
    context: &'a ApiContext,
    job_id: lettuce_types::JobId,
}

impl crate::CreationRoundObserver for CreatorRoundObserver<'_> {
    fn committed(&self) {
        notify(self.context, self.job_id);
    }
}

#[derive(Debug)]
struct CreatorSessionMedia<'a> {
    context: &'a ApiContext,
    job_id: lettuce_types::JobId,
    image_input: bool,
}

/// Tells watchers the creator job changed, so the workspace shows a new round or image.
fn notify(context: &ApiContext, job_id: lettuce_types::JobId) {
    let result = JobStore::get(context.backend().database(), job_id)
        .map_err(internal)
        .and_then(|job| job.ok_or_else(|| internal("creator job missing")))
        .and_then(|job| super::job_view(context, &job));
    match result {
        Ok(job) => context.emit(lettuce_contracts::ApiEvent::JobUpdated { job: Box::new(job) }),
        Err(error) => tracing::warn!(code = ?error.code, "creator round notification failed"),
    }
}

struct ImageWatch(tokio::sync::mpsc::UnboundedSender<lettuce_contracts::JobEvent>);

impl crate::api::events::JobEventSink for ImageWatch {
    fn emit(&self, event: lettuce_contracts::JobEvent) -> bool {
        self.0.send(event).is_ok()
    }
}

/// Waits for the image job to finish; cancelling `cancellation` cancels it and keeps waiting for it to stop.
async fn settled_image_job(
    context: &ApiContext,
    job_id: lettuce_types::JobId,
    cancellation: &CancellationToken,
) -> Result<lettuce_contracts::JobView, ApiError> {
    use lettuce_contracts::{JobEvent, JobStateDto};
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    let first = super::job_watch(
        context,
        lettuce_contracts::JobWatchRequest {
            job_id: job_id.to_string(),
        },
        Arc::new(ImageWatch(sender)),
    )
    .await?;
    if matches!(
        first.state,
        JobStateDto::Succeeded
            | JobStateDto::Failed
            | JobStateDto::Cancelled
            | JobStateDto::Interrupted
    ) {
        return Ok(first);
    }
    let mut cancelling = false;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(JobEvent::Completed { job } | JobEvent::Failed { job } | JobEvent::Cancelled { job }) => return Ok(job),
                Some(_) => {}
                None => return Err(internal("the image job watch ended")),
            },
            () = cancellation.cancelled(), if !cancelling => {
                cancelling = true;
                if let Err(error) = super::job_cancel(context, lettuce_contracts::JobCancelRequest { job_id: job_id.to_string() }).await {
                    tracing::warn!(code = ?error.code, "creator image cancellation failed");
                }
            }
        }
    }
}

#[async_trait]
impl crate::CreationSessionMedia for CreatorSessionMedia<'_> {
    fn images(
        &self,
        workflow_id: lettuce_types::CreationWorkflowId,
    ) -> Result<crate::CreationSessionImages, lettuce_creation::CreationRepositoryError> {
        let db = self.context.backend().database();
        Ok(crate::CreationSessionImages {
            known: db.creator_images(workflow_id)?,
            gallery: {
                let uploads = db
                    .creator_uploads(workflow_id)?
                    .into_iter()
                    .map(
                        |asset_id| crate::creation::creation_prompt::CreationPromptImage {
                            asset_id,
                            origin: crate::creation::creation_prompt::CreationImageOrigin::Uploaded,
                        },
                    );
                let generated = db
                    .creator_workspace(workflow_id)?
                    .generated_images
                    .into_iter()
                    .filter_map(|image| {
                        Some(crate::creation::creation_prompt::CreationPromptImage {
                            asset_id: image.asset_id?,
                            origin: if image.source_asset_id.is_some() {
                                crate::creation::creation_prompt::CreationImageOrigin::Edited
                            } else {
                                crate::creation::creation_prompt::CreationImageOrigin::Generated
                            },
                        })
                    });
                uploads.chain(generated).collect()
            },
        })
    }

    fn reference(
        &self,
        reference: &lettuce_creation::CreationReference,
    ) -> Option<crate::creation::creation_prompt::CreationReferenceText> {
        use lettuce_characters::{CharacterRepository, LifecycleStatus, PersonaRepository};
        let db = self.context.backend().database();
        match reference {
            lettuce_creation::CreationReference::Character { id } => {
                let found = CharacterRepository::get(db, *id).ok().flatten()?;
                (found.character.status == LifecycleStatus::Active).then(|| {
                    let profile = found.character.profile;
                    crate::creation::creation_prompt::CreationReferenceText {
                        name: profile.name,
                        text: profile.definition.or(profile.description),
                    }
                })
            }
            lettuce_creation::CreationReference::Persona { id } => {
                let found = PersonaRepository::get(db, *id).ok().flatten()?;
                (found.status == LifecycleStatus::Active).then(|| {
                    crate::creation::creation_prompt::CreationReferenceText {
                        name: found.title,
                        text: Some(found.description),
                    }
                })
            }
        }
    }

    fn image_input(&self) -> bool {
        self.image_input
    }

    fn made_images(
        &self,
        attempt_id: lettuce_types::GenerationAttemptId,
    ) -> Result<
        std::collections::HashMap<u16, lettuce_types::AssetId>,
        lettuce_creation::CreationRepositoryError,
    > {
        Ok(self
            .context
            .backend()
            .database()
            .creator_attempt_images(attempt_id)?
            .into_iter()
            .filter_map(|image| Some((image.call_ordinal, image.asset_id?)))
            .collect())
    }

    async fn make_image(
        &self,
        request: crate::CreationImageRequest,
        cancellation: &CancellationToken,
    ) -> Result<(), lettuce_creation::CreationRepositoryError> {
        let db = self.context.backend().database();
        db.creator_begin_image(lettuce_database::NewCreatorGeneratedImage {
            workflow_id: request.workflow_id,
            attempt_id: request.attempt_id,
            call_ordinal: request.call_ordinal,
            prompt: request.prompt.clone(),
            source_asset_id: request.source,
            now: self.context.now(),
        })?;
        let images = db.creator_workspace(request.workflow_id)?.generated_images;
        let row = images.iter().find(|image| {
            image.attempt_id == request.attempt_id && image.call_ordinal == request.call_ordinal
        });
        if row.is_some_and(|image| image.asset_id.is_some() || image.failed) {
            return Ok(());
        }
        let job_id = match row.and_then(|image| image.job_id) {
            Some(job_id) => job_id,
            None => {
                let source_prompt = request.source.and_then(|source| {
                    images
                        .iter()
                        .find(|image| image.asset_id == Some(source))
                        .map(|image| image.prompt.clone())
                });
                let kind = match request.source {
                    None => lettuce_contracts::AvatarPromptKind::Generation {
                        subject_name: request.subject_name,
                        subject_description: request.subject_description,
                        avatar_request: request.prompt,
                    },
                    Some(_) => lettuce_contracts::AvatarPromptKind::Edit {
                        subject_name: request.subject_name,
                        subject_description: request.subject_description,
                        current_avatar_prompt: source_prompt.unwrap_or_default(),
                        edit_request: request.prompt,
                    },
                };
                let request_id = crate::api::jobs::local::stable_uuid(&[
                    "creator-image",
                    &request.attempt_id.to_string(),
                    &request.call_ordinal.to_string(),
                ]);
                let accepted = crate::api::avatar_generate(
                    self.context,
                    lettuce_contracts::AvatarGenerateRequest {
                        request_id: request_id.to_string(),
                        model_id: None,
                        kind,
                        source_asset_id: request.source.map(|source| source.to_string()),
                        character_id: None,
                    },
                )
                .await
                .and_then(|accepted| {
                    accepted
                        .job_id
                        .parse::<lettuce_types::JobId>()
                        .map_err(internal)
                });
                match accepted {
                    Ok(job_id) => {
                        db.creator_image_job(request.attempt_id, request.call_ordinal, job_id)?;
                        notify(self.context, self.job_id);
                        job_id
                    }
                    Err(error) => {
                        tracing::warn!(code = ?error.code, "creator image could not start");
                        db.creator_settle_image(request.attempt_id, request.call_ordinal, None)?;
                        notify(self.context, self.job_id);
                        return Ok(());
                    }
                }
            }
        };
        let asset = match settled_image_job(self.context, job_id, cancellation).await {
            Ok(job) => match job.result {
                Some(lettuce_contracts::JobResultDto::ImageGeneration { images, .. }) => images
                    .first()
                    .and_then(|image| image.asset.asset_id.parse::<lettuce_types::AssetId>().ok()),
                _ => None,
            },
            Err(error) => {
                tracing::warn!(code = ?error.code, "creator image job could not be watched");
                None
            }
        };
        db.creator_settle_image(request.attempt_id, request.call_ordinal, asset)?;
        notify(self.context, self.job_id);
        Ok(())
    }
}

struct RuntimeCancellation<'a> {
    runtime: &'a lettuce_inference::InferenceRuntime,
    job_id: lettuce_types::JobId,
}

impl Drop for RuntimeCancellation<'_> {
    fn drop(&mut self) {
        let _ = self.runtime.unregister_cancellation(self.job_id);
    }
}

#[async_trait]
impl ClaimedJob for CreatorWork {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let runtime = context.backend().inference_runtime();
        runtime
            .register_cancellation(self.claim.job_id, self.cancellation.clone())
            .map_err(internal)?;
        let _cancellation = RuntimeCancellation {
            runtime,
            job_id: self.claim.job_id,
        };
        let sink = RequestId::new();
        let mut receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(async move {
            while let Some(envelope) = receiver.recv().await {
                match envelope.event {
                    lettuce_conversations::GenerationStreamEvent::TextDelta { text } => {
                        progress.text_delta(Some(text), None)
                    }
                    lettuce_conversations::GenerationStreamEvent::ReasoningDelta { text } => {
                        progress.text_delta(None, Some(text))
                    }
                }
            }
        });
        let inference = crate::companion::companion_memory_host::JobOutputInference::new(
            context.inference(),
            Some(runtime),
            self.detail.streaming.then_some(sink),
        );
        let handle = JobHandle::with_cancellation(self.claim.job_id, self.cancellation.clone());
        let observer = CreatorRoundObserver {
            context: &context,
            job_id: self.claim.job_id,
        };
        let media = CreatorSessionMedia {
            context: &context,
            job_id: self.claim.job_id,
            image_input: self.detail.image_input,
        };
        let result =
            crate::CreationContinuationCoordinator::new(context.backend().database(), &inference)
                .with_tool_fallback(self.detail.fallback)
                .with_prompt_documents(self.detail.helper_prompt, self.detail.runtime_prompt)
                .with_round_observer(&observer)
                .with_session_media(&media)
                .run(
                    self.detail.attempt_id,
                    self.detail.profile,
                    &handle,
                    self.detail.streaming.then_some(sink),
                    context.now(),
                )
                .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        let workflow_id = self.detail.workflow_id;
        let attempt_id = self.detail.attempt_id;
        let claim = self.claim;
        let succeeded = result.is_ok();
        let cancelled = matches!(result, Err(crate::CreationContinuationError::Cancelled));
        context
            .blocking(move |context| {
                let db = context.backend().database();
                let job = JobStore::get(db, claim.job_id)
                    .map_err(internal)?
                    .ok_or_else(|| internal("creator job missing"))?;
                if job.state.is_terminal() {
                    return Ok(());
                }
                let at = context.now().max(job.updated_at);
                if job.state == JobState::CancellationRequested {
                    db.append_and_transition(JobMutation::RequestCleanup {
                        claim: claim.clone(),
                        at,
                    })
                    .map_err(internal)?;
                    db.append_and_transition(JobMutation::FinishCancellation { claim, at })
                        .map_err(internal)?;
                } else if succeeded {
                    db.append_and_transition(JobMutation::Succeed {
                        claim,
                        outcome: JobOutcome::Success {
                            result_ref: OutcomeRef::Request(RequestId::from_uuid(
                                workflow_id.as_uuid(),
                            )),
                        },
                        at,
                    })
                    .map_err(internal)?;
                } else {
                    db.append_and_transition(JobMutation::Fail {
                        claim,
                        error: JobError::new(
                            if cancelled {
                                JobErrorCode::Cancelled
                            } else {
                                JobErrorCode::WorkerFailed
                            },
                            true,
                            "creator-generation-failed",
                        )
                        .map_err(internal)?,
                        at,
                    })
                    .map_err(internal)?;
                }
                let attempt = db
                    .load_creation_attempt(attempt_id)
                    .map_err(crate::api::creator::failure)?;
                if !attempt.status.is_terminal() {
                    let next = if attempt.status == CreationAttemptStatus::Created
                        || cancelled
                        || job.state == JobState::CancellationRequested
                    {
                        CreationAttemptStatus::Cancelled
                    } else {
                        CreationAttemptStatus::Interrupted
                    };
                    db.transition_creation_attempt(
                        attempt_id,
                        attempt.revision,
                        next,
                        None,
                        at.max(attempt.updated_at),
                    )
                    .map_err(crate::api::creator::failure)?;
                }
                Ok(())
            })
            .await?;
        result.map(|_| ()).map_err(|error| {
            api_error(
                if cancelled {
                    ApiErrorCode::Cancelled
                } else {
                    ApiErrorCode::Unavailable
                },
                error.to_string(),
            )
        })
    }
}
