use crate::api::jobs::local::{digest, operation_key, stable_uuid};
use crate::api::lorebooks::Failure;
use crate::api::{
    ApiContext,
    error::{IntoApiError, api_error, invalid_field, parse_id},
};
use lettuce_characters::{CharacterRepository, LifecycleStatus, PersonaRepository, ScenePart};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{MessagePart, ResolvedInferenceProfile};
use lettuce_creation::{
    CreationDraft, CreationOperation, CreationProposal, CreationReference, CreationRepositoryError,
    CreationScene, CreationStage, CreationTarget, CreationTurnInputs, CreationWorkflowRepository,
    NewCreationTurnAttempt, NewCreationWorkflow,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{
    AssetId, CharacterId, CreationProposalId, CreationTurnId, CreationWorkflowId,
    GenerationAttemptId, JobId, ModelProfileId, PageRequest, PersonaId, RequestId,
};

#[cfg(test)]
mod tests;

pub(crate) fn failure(error: CreationRepositoryError) -> ApiError {
    api_error(
        match error {
            CreationRepositoryError::NotFound => ApiErrorCode::NotFound,
            CreationRepositoryError::Conflict => ApiErrorCode::Conflict,
            CreationRepositoryError::Invalid { .. } => ApiErrorCode::InvalidInput,
            CreationRepositoryError::Storage { .. } => ApiErrorCode::Internal,
        },
        error.to_string(),
    )
}

fn draft_view(context: &ApiContext, draft: &CreationDraft) -> Result<dto::CreatorDraft, ApiError> {
    let (name, definition, scenes) = match draft {
        CreationDraft::Character {
            name,
            definition,
            scenes,
            ..
        } => (name, definition, scenes.as_slice()),
        CreationDraft::Persona {
            name, description, ..
        } => (name, description, &[][..]),
        CreationDraft::Lorebook { .. } => {
            return Err(api_error(
                ApiErrorCode::Unsupported,
                "unsupported creator target",
            ));
        }
    };
    Ok(dto::CreatorDraft {
        name: name.clone().unwrap_or_default(),
        definition: definition.clone().unwrap_or_default(),
        scenes: scenes
            .iter()
            .map(|scene| dto::CreatorScene {
                id: scene.id.to_string(),
                content: scene.content.clone(),
                direction: scene.direction.clone(),
            })
            .collect(),
        avatar: draft
            .avatar()
            .map(|asset_id| context.image_asset_ref(asset_id)),
    })
}

/// Refuses an asset that is missing, not ready or not an image.
fn ensure_image(
    context: &ApiContext,
    asset_id: AssetId,
    field: &'static str,
) -> Result<(), ApiError> {
    let media = context
        .media()
        .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "media storage is unavailable"))?;
    let image = media
        .open_ready(asset_id)
        .map_err(IntoApiError::into_api_error)?;
    if image.asset.kind.blob_kind() != lettuce_media::MediaKind::Image {
        return Err(invalid_field(field, "the asset is not an image"));
    }
    Ok(())
}

fn domain_draft(
    kind: dto::CreatorKind,
    draft: dto::CreatorDraft,
) -> Result<CreationDraft, ApiError> {
    let text = |text: String| (!text.trim().is_empty()).then_some(text);
    let avatar = draft
        .avatar
        .as_ref()
        .map(|asset| parse_id(&asset.asset_id, "avatar"))
        .transpose()?;
    let result = match kind {
        dto::CreatorKind::Character => CreationDraft::Character {
            name: text(draft.name),
            definition: text(draft.definition),
            scenes: draft
                .scenes
                .into_iter()
                .map(|scene| {
                    Ok(CreationScene {
                        id: parse_id(&scene.id, "scenes.id")?,
                        content: scene.content,
                        direction: scene.direction.and_then(text),
                    })
                })
                .collect::<Result<_, ApiError>>()?,
            avatar,
        },
        dto::CreatorKind::Persona => {
            if !draft.scenes.is_empty() {
                return Err(invalid_field("scenes", "a persona has no scenes"));
            }
            CreationDraft::Persona {
                name: text(draft.name),
                description: text(draft.definition),
                avatar,
            }
        }
    };
    NewCreationWorkflow {
        id: CreationWorkflowId::new(),
        initial_proposal_id: CreationProposalId::new(),
        target: match kind {
            dto::CreatorKind::Character => CreationTarget::NewCharacter,
            dto::CreatorKind::Persona => CreationTarget::NewPersona,
        },
        initial_draft: result.clone(),
        now: lettuce_types::TimestampMillis::UNIX_EPOCH,
    }
    .validate()
    .map_err(|_| invalid_field("draft", "invalid creation draft"))?;
    Ok(result)
}

fn proposal_source(proposal: &CreationProposal) -> dto::CreatorProposalSource {
    if proposal.parent_id.is_none() {
        return dto::CreatorProposalSource::Start;
    }
    match proposal.outcomes.first().map(|outcome| &outcome.operation) {
        Some(CreationOperation::ReplaceDraft { .. }) if human_review(proposal) => {
            dto::CreatorProposalSource::Review
        }
        Some(CreationOperation::ReplaceDraft { .. }) => dto::CreatorProposalSource::Edit,
        _ => dto::CreatorProposalSource::Creator,
    }
}

fn proposal_view(
    context: &ApiContext,
    proposal: &CreationProposal,
) -> Result<dto::CreatorProposalView, ApiError> {
    Ok(dto::CreatorProposalView {
        id: proposal.id.to_string(),
        parent_id: proposal.parent_id.map(|id| id.to_string()),
        ordinal: proposal.ordinal,
        source: proposal_source(proposal),
        draft: draft_view(context, &proposal.draft)?,
        created_at: proposal.created_at.get(),
    })
}

fn summary(
    context: &ApiContext,
    work: &lettuce_database::CreatorWorkspace,
) -> Result<dto::CreatorSummary, ApiError> {
    let (kind, target_id) = match work.workflow.target {
        CreationTarget::NewCharacter => (dto::CreatorKind::Character, None),
        CreationTarget::ExistingCharacter { id, .. } => {
            (dto::CreatorKind::Character, Some(id.to_string()))
        }
        CreationTarget::NewPersona => (dto::CreatorKind::Persona, None),
        CreationTarget::ExistingPersona { id, .. } => {
            (dto::CreatorKind::Persona, Some(id.to_string()))
        }
        _ => {
            return Err(api_error(
                ApiErrorCode::Unsupported,
                "unsupported creator target",
            ));
        }
    };
    Ok(dto::CreatorSummary {
        id: work.workflow.id.to_string(),
        kind,
        name: draft_view(context, &work.proposal.draft)?.name,
        target_id,
        applied_id: work.applied_id.clone(),
        updated_at: work.workflow.updated_at.get(),
    })
}

/// The live name of a referenced character or persona; `None` once it was deleted or archived.
fn reference_view(
    db: &lettuce_database::Database,
    reference: &CreationReference,
) -> dto::CreatorReferenceView {
    match reference {
        CreationReference::Character { id } => dto::CreatorReferenceView {
            kind: dto::CreatorKind::Character,
            id: id.to_string(),
            name: CharacterRepository::get(db, *id)
                .ok()
                .flatten()
                .filter(|found| found.character.status == LifecycleStatus::Active)
                .map(|found| found.character.profile.name),
        },
        CreationReference::Persona { id } => dto::CreatorReferenceView {
            kind: dto::CreatorKind::Persona,
            id: id.to_string(),
            name: PersonaRepository::get(db, *id)
                .ok()
                .flatten()
                .filter(|found| found.status == LifecycleStatus::Active)
                .map(|found| found.title),
        },
    }
}

/// The session's images: uploads in message order, then what the creator generated or edited.
fn session_images(
    context: &ApiContext,
    work: &lettuce_database::CreatorWorkspace,
) -> Vec<dto::CreatorImageView> {
    let mut seen = std::collections::HashSet::new();
    let uploads = work.dialogue.iter().flat_map(|turn| {
        turn.turn
            .inputs
            .images
            .iter()
            .map(move |asset_id| (*asset_id, dto::CreatorImageOrigin::Upload, turn.turn.id))
    });
    let generated = work.generated_images.iter().filter_map(|image| {
        let origin = if image.source_asset_id.is_some() {
            dto::CreatorImageOrigin::Edited
        } else {
            dto::CreatorImageOrigin::Generated
        };
        Some((image.asset_id?, origin, image.turn_id))
    });
    uploads
        .chain(generated)
        .filter(|(asset_id, ..)| seen.insert(*asset_id))
        .map(|(asset_id, origin, turn_id)| dto::CreatorImageView {
            asset: context.image_asset_ref(asset_id),
            origin,
            turn_id: turn_id.to_string(),
        })
        .collect()
}

fn view(
    context: &ApiContext,
    work: lettuce_database::CreatorWorkspace,
    target_changed: bool,
    model_profile_id: Option<String>,
    images: &[AssetId],
) -> Result<dto::CreatorView, ApiError> {
    let db = context.backend().database();
    let gallery = session_images(context, &work);
    let active_job_id = work
        .dialogue
        .iter()
        .find(|turn| !turn.job.state.is_terminal())
        .map(|turn| turn.job.id.to_string());
    let reviewed =
        work.workflow.stage == CreationStage::AwaitingConfirmation && human_review(&work.proposal);
    let open = work.applied_id.is_none() && active_job_id.is_none();
    let current = work.workflow.current_proposal_id;
    let latest_turn_id = work.latest_turn_id;
    Ok(dto::CreatorView {
        summary: summary(context, &work)?,
        revision: work.workflow.revision.get(),
        proposal: proposal_view(context, &work.proposal)?,
        history: work
            .history
            .iter()
            .map(|proposal| proposal_view(context, proposal))
            .collect::<Result<_, _>>()?,
        active_job_id,
        reviewed,
        target_changed,
        model_profile_id,
        images: gallery,
        turns: work
            .dialogue
            .into_iter()
            .map(|turn| {
                let status = if turn.job.state == lettuce_jobs::JobState::Interrupted {
                    dto::CreatorTurnStatus::Interrupted
                } else {
                    match turn.attempt.status {
                        lettuce_creation::CreationAttemptStatus::Succeeded => {
                            dto::CreatorTurnStatus::Succeeded
                        }
                        lettuce_creation::CreationAttemptStatus::Failed => {
                            dto::CreatorTurnStatus::Failed
                        }
                        lettuce_creation::CreationAttemptStatus::Cancelled => {
                            dto::CreatorTurnStatus::Cancelled
                        }
                        lettuce_creation::CreationAttemptStatus::Interrupted => {
                            dto::CreatorTurnStatus::Interrupted
                        }
                        _ => dto::CreatorTurnStatus::Pending,
                    }
                };
                let reply = turn
                    .rounds
                    .iter()
                    .flat_map(|round| &round.parts)
                    .filter_map(|part| match part {
                        MessagePart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let reasoning = turn
                    .rounds
                    .iter()
                    .flat_map(|round| &round.parts)
                    .filter_map(|part| match part {
                        MessagePart::ReasoningSummary { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let mut steps = Vec::new();
                let mut call_index = 0_u32;
                for round in &turn.rounds {
                    for part in &round.parts {
                        if let MessagePart::Text { text } = part
                            && !text.is_empty()
                        {
                            steps.push(dto::CreatorTurnStep::Text { text: text.clone() });
                        }
                    }
                    for _ in &round.calls {
                        steps.push(dto::CreatorTurnStep::Tool { index: call_index });
                        call_index += 1;
                    }
                }
                let result_proposal = work
                    .history
                    .iter()
                    .find(|proposal| proposal.turn_id == Some(turn.turn.id))
                    .map(|proposal| proposal.id);
                let regenerable = open
                    && status != dto::CreatorTurnStatus::Pending
                    && latest_turn_id == Some(turn.turn.id)
                    && result_proposal.unwrap_or(turn.turn.base_proposal_id) == current;
                let result_proposal_id = result_proposal.map(|id| id.to_string());
                let tools = tool_views(
                    context,
                    &turn,
                    &work.history,
                    images,
                    &work.generated_images,
                );
                let references = turn
                    .turn
                    .inputs
                    .references
                    .iter()
                    .map(|reference| reference_view(db, reference))
                    .collect();
                let turn_images = turn
                    .turn
                    .inputs
                    .images
                    .iter()
                    .map(|asset_id| context.image_asset_ref(*asset_id))
                    .collect();
                dto::CreatorTurnView {
                    id: turn.turn.id.to_string(),
                    message: turn.turn.user_message,
                    reply,
                    reasoning,
                    status,
                    job_id: turn.job.id.to_string(),
                    base_proposal_id: turn.turn.base_proposal_id.to_string(),
                    result_proposal_id,
                    regenerated: turn.regenerated,
                    regenerable,
                    references,
                    images: turn_images,
                    tools,
                    steps,
                }
            })
            .collect(),
    })
}

/// The tool calls of a turn with their outcome. A finished turn reads the outcomes its proposal recorded; a turn
/// still running reduces the calls admitted so far against its base proposal. While the job runs, the calls of the
/// newest round show as running, since the creator is still acting on their results.
fn tool_views(
    context: &ApiContext,
    turn: &lettuce_database::CreatorDialogue,
    history: &[CreationProposal],
    images: &[AssetId],
    generated: &[lettuce_database::CreatorGeneratedImage],
) -> Vec<dto::CreatorToolView> {
    let calls = turn
        .rounds
        .iter()
        .flat_map(|round| round.calls.iter().map(move |call| (round.ordinal, call)))
        .collect::<Vec<_>>();
    if calls.is_empty() {
        return Vec::new();
    }
    let running = !turn.job.state.is_terminal();
    let last_round = turn.rounds.iter().map(|round| round.ordinal).max();
    let recorded = history
        .iter()
        .find(|proposal| proposal.turn_id == Some(turn.turn.id))
        .map(|proposal| proposal.outcomes.clone());
    let outcomes = recorded.or_else(|| {
        let base = history
            .iter()
            .find(|proposal| proposal.id == turn.turn.base_proposal_id)?;
        let admitted = calls
            .iter()
            .map(|(_, evidence)| lettuce_creation::AdmittedCreationToolCall {
                definition_version: evidence.definition_version,
                call: evidence.call.clone(),
            })
            .collect::<Vec<_>>();
        lettuce_creation::reduce_creation_tool_calls(
            base,
            turn.attempt.planned_proposal_id,
            turn.turn.id,
            &admitted,
            images,
            turn.attempt.updated_at,
        )
        .ok()
        .map(|batch| batch.proposal.outcomes)
    });
    calls
        .iter()
        .enumerate()
        .map(|(index, (round, evidence))| {
            let image = generated.iter().find(|image| {
                image.attempt_id == turn.attempt.id && usize::from(image.call_ordinal) == index
            });
            let making = image.is_some_and(|image| image.asset_id.is_none() && !image.failed);
            let status = if running && (Some(*round) == last_round || making) {
                dto::CreatorToolStatus::Running
            } else if image.is_some_and(|image| image.failed) {
                dto::CreatorToolStatus::Failed
            } else if outcomes
                .as_ref()
                .and_then(|outcomes| outcomes.get(index))
                .is_some_and(lettuce_creation::CreationOperationOutcome::succeeded)
            {
                dto::CreatorToolStatus::Succeeded
            } else {
                dto::CreatorToolStatus::Failed
            };
            dto::CreatorToolView {
                name: evidence.call.name.clone(),
                status,
                image: image.map(|image| dto::CreatorToolImage {
                    asset: image
                        .asset_id
                        .map(|asset_id| context.image_asset_ref(asset_id)),
                    job_id: image.job_id.map(|job_id| job_id.to_string()),
                    failed: image.failed || (!running && image.asset_id.is_none()),
                }),
            }
        })
        .collect()
}

pub async fn creator_get(
    context: &ApiContext,
    request: dto::CreatorGetRequest,
) -> Result<dto::CreatorView, ApiError> {
    let id = parse_id(&request.workflow_id, "workflow_id")?;
    context
        .blocking(move |context| {
            let db = context.backend().database();
            db.creator_reconcile(id, context.now()).map_err(failure)?;
            let work = db.creator_workspace(id).map_err(failure)?;
            let changed = target_changed(db, &work)?;
            let model = latest_model(db, &work)?;
            let images = db.creator_images(id).map_err(failure)?;
            view(context, work, changed, model, &images)
        })
        .await
}

fn target_changed(
    db: &lettuce_database::Database,
    work: &lettuce_database::CreatorWorkspace,
) -> Result<bool, ApiError> {
    if work.applied_id.is_some() {
        return Ok(false);
    }
    Ok(match work.workflow.target {
        CreationTarget::ExistingCharacter { id, revision } => CharacterRepository::get(db, id)
            .map_err(IntoApiError::into_api_error)?
            .is_none_or(|current| {
                current.character.revision != revision
                    || current.character.status != LifecycleStatus::Active
            }),
        CreationTarget::ExistingPersona { id, revision } => PersonaRepository::get(db, id)
            .map_err(IntoApiError::into_api_error)?
            .is_none_or(|current| {
                current.revision != revision || current.status != LifecycleStatus::Active
            }),
        _ => false,
    })
}

fn latest_model(
    db: &lettuce_database::Database,
    work: &lettuce_database::CreatorWorkspace,
) -> Result<Option<String>, ApiError> {
    let Some(latest) = work.dialogue.last() else {
        return Ok(None);
    };
    let Some(record) = db
        .job_detail(latest.job.id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
    else {
        return Ok(None);
    };
    Ok(record
        .detail
        .get("selected_model_profile_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

pub async fn creators_list(
    context: &ApiContext,
    request: dto::CreatorsListRequest,
) -> Result<dto::CreatorPage, ApiError> {
    context
        .blocking(move |context| {
            let db = context.backend().database();
            let page = db
                .creator_workflows(PageRequest {
                    cursor: request.cursor,
                    limit: crate::api::mapping::page_limit(request.limit),
                })
                .map_err(failure)?;
            Ok(dto::CreatorPage {
                items: page
                    .items
                    .into_iter()
                    .map(|workflow| {
                        summary(
                            context,
                            &db.creator_workspace(workflow.id).map_err(failure)?,
                        )
                    })
                    .collect::<Result<_, _>>()?,
                next_cursor: page.next_cursor,
            })
        })
        .await
}

pub async fn creator_start(
    context: &ApiContext,
    request: dto::CreatorStartRequest,
) -> Result<dto::CreatorView, ApiError> {
    let request_digest = digest(&request)?;
    operation_key("creator_start", &request.client_operation_id)?;
    let id = context
        .blocking(move |context| {
            let db = context.backend().database();
            if let Some(prior) = db
                .lookup_api_operation("creator_start", &request.client_operation_id)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            {
                if prior.request_digest != request_digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "creator operation changed",
                    ));
                }
                return serde_json::from_value::<String>(prior.result)
                    .map_err(|_| api_error(ApiErrorCode::Internal, "invalid creator receipt"));
            }
            let (target, initial) = match (request.kind, request.target_id.as_deref()) {
                (dto::CreatorKind::Character, Some(id)) => {
                    let id: CharacterId = parse_id(id, "target_id")?;
                    let character = CharacterRepository::get(db, id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "character missing"))?;
                    if character.character.status != LifecycleStatus::Active {
                        return Err(api_error(
                            ApiErrorCode::Unavailable,
                            "character is archived",
                        ));
                    }
                    if character.scenes.iter().any(|scene| {
                        scene.content.parts.len() != 1
                            || !matches!(scene.content.parts[0], ScenePart::Text { .. })
                    }) {
                        return Err(api_error(
                            ApiErrorCode::Unsupported,
                            "rich scenes require the full character editor",
                        ));
                    }
                    let avatar = crate::api::mapping::character_avatar(&character.character);
                    let draft = CreationDraft::Character {
                        name: Some(character.character.profile.name),
                        definition: character.character.profile.definition,
                        scenes: character
                            .scenes
                            .into_iter()
                            .map(|scene| CreationScene {
                                id: scene.id,
                                content: scene
                                    .content
                                    .parts
                                    .into_iter()
                                    .filter_map(|part| match part {
                                        ScenePart::Text { text } => Some(text),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                direction: scene.direction,
                            })
                            .collect(),
                        avatar,
                    };
                    (
                        CreationTarget::ExistingCharacter {
                            id,
                            revision: character.character.revision,
                        },
                        draft,
                    )
                }
                (dto::CreatorKind::Persona, Some(id)) => {
                    let id: PersonaId = parse_id(id, "target_id")?;
                    let persona = PersonaRepository::get(db, id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "persona missing"))?;
                    if persona.status != LifecycleStatus::Active {
                        return Err(api_error(ApiErrorCode::Unavailable, "persona is archived"));
                    }
                    (
                        CreationTarget::ExistingPersona {
                            id,
                            revision: persona.revision,
                        },
                        CreationDraft::Persona {
                            name: Some(persona.title),
                            description: Some(persona.description),
                            avatar: persona
                                .media
                                .links
                                .iter()
                                .find(|link| {
                                    link.slot == lettuce_characters::PersonaMediaSlot::Avatar
                                })
                                .map(|link| link.asset_id),
                        },
                    )
                }
                (dto::CreatorKind::Character, None) => (
                    CreationTarget::NewCharacter,
                    CreationDraft::Character {
                        name: None,
                        definition: None,
                        scenes: Vec::new(),
                        avatar: None,
                    },
                ),
                (dto::CreatorKind::Persona, None) => (
                    CreationTarget::NewPersona,
                    CreationDraft::Persona {
                        name: None,
                        description: None,
                        avatar: None,
                    },
                ),
            };
            let requested = request
                .draft
                .map(|draft| domain_draft(request.kind, draft))
                .transpose()?;
            if requested.is_none()
                && let Some(open) = db.creator_open_workflow(&target).map_err(failure)?
            {
                return Ok(open.to_string());
            }
            let (initial, edit) = match (&target, requested) {
                (CreationTarget::NewCharacter | CreationTarget::NewPersona, Some(draft)) => {
                    (draft, None)
                }
                (_, requested) => {
                    let edit = requested.filter(|draft| draft != &initial);
                    (initial, edit)
                }
            };
            let id = CreationWorkflowId::new();
            db.commit_api_operation(
                "creator_start",
                &request.client_operation_id,
                &request_digest,
                context.now(),
                |scope| {
                    let workflow = scope
                        .creator_create(NewCreationWorkflow {
                            id,
                            initial_proposal_id: CreationProposalId::new(),
                            target,
                            initial_draft: initial,
                            now: context.now(),
                        })
                        .map_err(|error| Failure(failure(error)))?;
                    if let Some(draft) = edit {
                        scope
                            .creator_edit(lettuce_database::CreatorUserEdit {
                                workflow_id: id,
                                expected_revision: workflow.revision,
                                turn_id: CreationTurnId::new(),
                                proposal_id: CreationProposalId::new(),
                                draft,
                                review: false,
                                now: context.now(),
                            })
                            .map_err(|error| Failure(failure(error)))?;
                    }
                    Ok(id.to_string())
                },
            )
            .map_err(|error: Failure| error.0)
        })
        .await?;
    creator_get(context, dto::CreatorGetRequest { workflow_id: id }).await
}

pub async fn creator_edit(
    context: &ApiContext,
    request: dto::CreatorEditRequest,
) -> Result<dto::CreatorView, ApiError> {
    let id = parse_id(&request.workflow_id, "workflow_id")?;
    let expected = crate::api::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let request_digest = digest(&request)?;
    operation_key("creator_edit", &request.client_operation_id)?;
    context
        .blocking(move |context| {
            let db = context.backend().database();
            db.creator_reconcile(id, context.now()).map_err(failure)?;
            let work = db.creator_workspace(id).map_err(failure)?;
            let draft = domain_draft(summary(context, &work)?.kind, request.draft)?;
            if let Some(avatar) = draft.avatar()
                && work.proposal.draft.avatar() != Some(avatar)
            {
                ensure_image(context, avatar, "draft.avatar")?;
            }
            db.commit_api_operation(
                "creator_edit",
                &request.client_operation_id,
                &request_digest,
                context.now(),
                |scope| {
                    scope
                        .creator_edit(lettuce_database::CreatorUserEdit {
                            workflow_id: id,
                            expected_revision: expected,
                            turn_id: CreationTurnId::new(),
                            proposal_id: CreationProposalId::new(),
                            draft,
                            review: false,
                            now: context.now(),
                        })
                        .map_err(|error| Failure(failure(error)))?;
                    Ok(())
                },
            )
            .map_err(|error: Failure| error.0)
        })
        .await?;
    creator_get(
        context,
        dto::CreatorGetRequest {
            workflow_id: request.workflow_id,
        },
    )
    .await
}

fn human_review(proposal: &CreationProposal) -> bool {
    matches!(
        proposal.outcomes.as_slice(),
        [first, second]
            if matches!(first.operation, CreationOperation::ReplaceDraft { .. })
                && matches!(second.operation, CreationOperation::RequestConfirmation)
                && first.succeeded()
                && second.succeeded()
    )
}

pub async fn creator_review(
    context: &ApiContext,
    request: dto::CreatorRevisionRequest,
) -> Result<dto::CreatorView, ApiError> {
    let id = parse_id(&request.workflow_id, "workflow_id")?;
    let expected = crate::api::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let proposal_id: CreationProposalId = parse_id(&request.proposal_id, "proposal_id")?;
    let request_digest = digest(&request)?;
    operation_key("creator_review", &request.client_operation_id)?;
    context
        .blocking(move |context| {
            let db = context.backend().database();
            db.creator_reconcile(id, context.now()).map_err(failure)?;
            let proposal = db.load_proposal(proposal_id).map_err(failure)?;
            let draft = draft_view(context, &proposal.draft)?;
            if draft.name.trim().is_empty() || draft.definition.trim().is_empty() {
                return Err(invalid_field("draft", "name and definition are required"));
            }
            db.commit_api_operation(
                "creator_review",
                &request.client_operation_id,
                &request_digest,
                context.now(),
                |scope| {
                    scope
                        .creator_check_proposal(id, proposal_id)
                        .map_err(|error| Failure(failure(error)))?;
                    scope
                        .creator_edit(lettuce_database::CreatorUserEdit {
                            workflow_id: id,
                            expected_revision: expected,
                            turn_id: CreationTurnId::new(),
                            proposal_id: CreationProposalId::new(),
                            draft: proposal.draft,
                            review: true,
                            now: context.now(),
                        })
                        .map_err(|error| Failure(failure(error)))?;
                    Ok(())
                },
            )
            .map_err(|error: Failure| error.0)
        })
        .await?;
    creator_get(
        context,
        dto::CreatorGetRequest {
            workflow_id: request.workflow_id,
        },
    )
    .await
}

pub async fn creator_apply(
    context: &ApiContext,
    request: dto::CreatorRevisionRequest,
) -> Result<dto::CreatorView, ApiError> {
    let id = parse_id(&request.workflow_id, "workflow_id")?;
    let expected = crate::api::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let proposal_id: CreationProposalId = parse_id(&request.proposal_id, "proposal_id")?;
    operation_key("creator_apply", &request.client_operation_id)?;
    let request_digest = digest(&request)?;
    context
        .blocking(move |context| {
            let db = context.backend().database();
            db.creator_reconcile(id, context.now()).map_err(failure)?;
            let workflow = db.load_workflow(id).map_err(failure)?;
            let proposal = db.load_proposal(proposal_id).map_err(failure)?;
            if !human_review(&proposal) {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "the draft needs user review",
                ));
            }
            let now = context.now();
            db.commit_api_operation(
                "creator_apply",
                &request.client_operation_id,
                &request_digest,
                now,
                |scope| {
                    match workflow.target {
                        CreationTarget::NewCharacter => {
                            scope
                                .creator_apply_new_character(
                                    lettuce_creation::ConfirmedCharacterApply {
                                        workflow_id: id,
                                        expected_workflow_revision: expected,
                                        proposal_id,
                                        destination_character_id: CharacterId::from_uuid(
                                            stable_uuid(&["creator-character", &id.to_string()]),
                                        ),
                                        now,
                                    },
                                )
                                .map_err(|error| Failure(failure(error)))?;
                        }
                        CreationTarget::ExistingCharacter {
                            id: character_id,
                            revision,
                        } => {
                            scope
                                .creator_apply_existing_character(
                                    lettuce_creation::ConfirmedCharacterRevisionApply {
                                        workflow_id: id,
                                        expected_workflow_revision: expected,
                                        proposal_id,
                                        character_id,
                                        expected_character_revision: revision,
                                        now,
                                    },
                                )
                                .map_err(|error| Failure(failure(error)))?;
                        }
                        CreationTarget::NewPersona => {
                            scope
                                .creator_apply_new_persona(
                                    lettuce_creation::ConfirmedPersonaApply {
                                        workflow_id: id,
                                        expected_workflow_revision: expected,
                                        proposal_id,
                                        destination_persona_id: PersonaId::from_uuid(stable_uuid(
                                            &["creator-persona", &id.to_string()],
                                        )),
                                        now,
                                    },
                                )
                                .map_err(|error| Failure(failure(error)))?;
                        }
                        CreationTarget::ExistingPersona {
                            id: persona_id,
                            revision,
                        } => {
                            scope
                                .creator_apply_existing_persona(
                                    lettuce_creation::ConfirmedPersonaRevisionApply {
                                        workflow_id: id,
                                        expected_workflow_revision: expected,
                                        proposal_id,
                                        persona_id,
                                        expected_persona_revision: revision,
                                        now,
                                    },
                                )
                                .map_err(|error| Failure(failure(error)))?;
                        }
                        _ => {
                            return Err(Failure(api_error(
                                ApiErrorCode::Unsupported,
                                "unsupported creator target",
                            )));
                        }
                    }
                    Ok(())
                },
            )
            .map_err(|error: Failure| error.0)
        })
        .await?;
    creator_get(
        context,
        dto::CreatorGetRequest {
            workflow_id: request.workflow_id,
        },
    )
    .await
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CreatorJobDetail {
    pub workflow_id: CreationWorkflowId,
    pub attempt_id: GenerationAttemptId,
    pub profile: ResolvedInferenceProfile,
    pub fallback: lettuce_settings::CreationHelperToolFallback,
    pub streaming: bool,
    pub helper_prompt: lettuce_context::PromptDocument,
    pub runtime_prompt: lettuce_context::PromptDocument,
    #[serde(default)]
    pub selected_model_profile_id: Option<String>,
    #[serde(default)]
    pub image_input: bool,
}

fn profile(
    context: &ApiContext,
    selected: Option<ModelProfileId>,
) -> Result<
    (
        ResolvedInferenceProfile,
        lettuce_settings::CreationHelperSettings,
        bool,
    ),
    ApiError,
> {
    use lettuce_models::GlobalModelSettingsRepository;
    let db = context.backend().database();
    let mut settings = GlobalSettingsStore::load(db)
        .map_err(|_| api_error(ApiErrorCode::Internal, "settings unavailable"))?;
    if let Some(id) = selected {
        settings.settings.creation_helper.model_profile_id = Some(id);
    }
    let model = crate::image::image_feature_models::creation_helper_model(
        db,
        &settings.settings,
        settings.default_model_profile_id,
    )
    .map_err(|error| api_error(ApiErrorCode::ModelUnavailable, error.to_string()))?;
    if !model.account.enabled
        || model.profile.kind != lettuce_models::ModelKind::Chat
        || model.profile.config.capabilities.input_modalities.text
            == lettuce_models::CapabilityStatus::Unsupported
        || model.profile.config.capabilities.output_modalities.text
            == lettuce_models::CapabilityStatus::Unsupported
    {
        return Err(api_error(
            ApiErrorCode::ModelUnavailable,
            "the creator requires an enabled text model",
        ));
    }
    if settings.settings.creation_helper.tool_fallback
        == lettuce_settings::CreationHelperToolFallback::Native
        && model.profile.config.capabilities.tools == lettuce_models::CapabilityStatus::Unsupported
    {
        return Err(api_error(
            ApiErrorCode::Unsupported,
            "the model requires a structured tool fallback",
        ));
    }
    let image_input = model.profile.config.capabilities.input_modalities.image
        == lettuce_models::CapabilityStatus::Supported;
    let parameters = crate::feature_parameter_input(
        &model.profile.config.feature_parameters.creation_helper,
        crate::CREATION_HELPER_DEFAULTS,
        crate::FeatureRequestFields::Sampling,
        model.account.protocol,
        &GlobalModelSettingsRepository::global_model_settings(db)
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            .0,
    );
    let resolved = lettuce_models::resolve_chat_profile(
        &lettuce_models::ExpectedModelIdentity {
            model_profile_id: model.profile.id,
            model_revision: model.profile.revision,
            provider_account_id: model.account.id,
            provider_account_revision: model.account.revision,
            external_model_id: model.profile.external_model_id.clone(),
            display_name: model.profile.display_name.clone(),
            provider_protocol: model.account.protocol,
            model_kind: model.profile.kind,
        },
        &model.profile,
        &model.account,
        &parameters,
        &Default::default(),
    )
    .map_err(|error| api_error(ApiErrorCode::ModelUnavailable, error.to_string()))?;
    Ok((
        ResolvedInferenceProfile {
            chat_profile: resolved,
            tool_policy: lettuce_conversations::ToolPolicy::Allowed,
            output_policy: lettuce_conversations::OutputPolicy::Plain,
            safety_policy: lettuce_conversations::SafetyContext::Standard,
            correlation_id: None,
        },
        settings.settings.creation_helper,
        image_input,
    ))
}

pub async fn creator_send(
    context: &ApiContext,
    request: dto::CreatorSendRequest,
) -> Result<dto::JobAccepted, ApiError> {
    enqueue(context, request, None).await
}

pub async fn creator_regenerate(
    context: &ApiContext,
    request: dto::CreatorRegenerateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let turn = parse_id(&request.turn_id, "turn_id")?;
    enqueue(
        context,
        dto::CreatorSendRequest {
            workflow_id: request.workflow_id,
            expected_revision: request.expected_revision,
            message: String::new(),
            references: Vec::new(),
            images: Vec::new(),
            model_profile_id: request.model_profile_id,
            client_operation_id: request.client_operation_id,
        },
        Some(turn),
    )
    .await
}

/// What the user attached to a message, checked to exist: active characters and personas, and ready images.
fn turn_inputs(
    context: &ApiContext,
    request: &dto::CreatorSendRequest,
) -> Result<CreationTurnInputs, ApiError> {
    let db = context.backend().database();
    let references = request
        .references
        .iter()
        .map(|reference| {
            let missing = || api_error(ApiErrorCode::NotFound, "the referenced item is missing");
            Ok(match reference.kind {
                dto::CreatorKind::Character => {
                    let id: CharacterId = parse_id(&reference.id, "references.id")?;
                    CharacterRepository::get(db, id)
                        .map_err(IntoApiError::into_api_error)?
                        .filter(|found| found.character.status == LifecycleStatus::Active)
                        .ok_or_else(missing)?;
                    CreationReference::Character { id }
                }
                dto::CreatorKind::Persona => {
                    let id: PersonaId = parse_id(&reference.id, "references.id")?;
                    PersonaRepository::get(db, id)
                        .map_err(IntoApiError::into_api_error)?
                        .filter(|found| found.status == LifecycleStatus::Active)
                        .ok_or_else(missing)?;
                    CreationReference::Persona { id }
                }
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let images = request
        .images
        .iter()
        .map(|id| {
            let asset_id: AssetId = parse_id(id, "images")?;
            ensure_image(context, asset_id, "images")?;
            Ok(asset_id)
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let inputs = CreationTurnInputs { references, images };
    inputs
        .validate()
        .map_err(|_| invalid_field("references", "an attachment is repeated"))?;
    Ok(inputs)
}

async fn enqueue(
    context: &ApiContext,
    request: dto::CreatorSendRequest,
    regenerate: Option<CreationTurnId>,
) -> Result<dto::JobAccepted, ApiError> {
    let id = parse_id(&request.workflow_id, "workflow_id")?;
    let expected = crate::api::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let selected = request
        .model_profile_id
        .as_deref()
        .map(|id| parse_id(id, "model_profile_id"))
        .transpose()?;
    let key = operation_key("creator", &request.client_operation_id)?;
    let request_digest = digest(&(&request, regenerate))?;
    let job_id = context
        .blocking(move |context| {
            let db = context.backend().database();
            if let Some(prior) = db
                .job_operation(&key)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            {
                if prior.request_digest != request_digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "creator operation changed",
                    ));
                }
                return Ok(prior.job_id.to_string());
            }
            db.creator_reconcile(id, context.now()).map_err(failure)?;
            let workflow = db.load_workflow(id).map_err(failure)?;
            let inputs = turn_inputs(context, &request)?;
            let (profile, settings, image_input) = profile(context, selected)?;
            let attempt_id = GenerationAttemptId::new();
            let helper_prompt = crate::generation::runtime_text::RuntimeText::load(
                db,
                crate::BuiltInPromptId::CreationHelper,
            )
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?
            .document()
            .clone();
            let runtime_prompt = crate::generation::runtime_text::RuntimeText::load(
                db,
                crate::BuiltInPromptId::CreationRuntime,
            )
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?
            .document()
            .clone();
            let detail = CreatorJobDetail {
                workflow_id: id,
                attempt_id,
                profile: profile.clone(),
                fallback: settings.tool_fallback,
                streaming: settings.streaming
                    && settings.tool_fallback
                        == lettuce_settings::CreationHelperToolFallback::Native,
                helper_prompt,
                runtime_prompt,
                selected_model_profile_id: request.model_profile_id.clone(),
                image_input,
            };
            let request_id = RequestId::new();
            let mut spec =
                crate::jobs::one_shot_job::one_shot_spec(crate::jobs::one_shot_job::OneShotJob {
                    name: "creator",
                    stage: "creator",
                    subject_kind: lettuce_jobs::SubjectKind::CreationProject,
                    subject: &id.to_string(),
                    request_id,
                });
            spec.recovery_policy = lettuce_jobs::RecoveryPolicy::MarkInterrupted;
            let input = NewCreationTurnAttempt {
                workflow_id: id,
                expected_workflow_revision: expected,
                base_proposal_id: workflow.current_proposal_id,
                turn_id: CreationTurnId::new(),
                attempt_id,
                planned_proposal_id: CreationProposalId::new(),
                user_message: request.message,
                inputs,
                job_id: JobId::new(),
                profile_fingerprint: lettuce_creation::creation_inference_profile_fingerprint(
                    &profile,
                )
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?,
                now: context.now(),
            };
            let detail = serde_json::to_value(detail)
                .map_err(|_| api_error(ApiErrorCode::Internal, "creator job cannot be encoded"))?;
            db.commit_api_operation(
                "creator_enqueue",
                &request.client_operation_id,
                &request_digest,
                context.now(),
                |scope| {
                    let job = scope
                        .creator_enqueue(spec, &key, &request_digest, &detail, input, regenerate)
                        .map_err(|error| Failure(failure(error)))?;
                    Ok(job.id.to_string())
                },
            )
            .map_err(|error: Failure| error.0)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted { job_id })
}
