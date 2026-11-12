use super::*;
use crate::ApiOperationTransaction;
use lettuce_creation::{CreationDraft, CreationOperation};
use lettuce_jobs::{JobSnapshot, JobState, JobStore, NewJob};
use lettuce_types::{Page, PageRequest};

#[derive(Debug, Clone)]
pub struct CreatorDialogue {
    pub turn: CreationTurn,
    pub attempt: CreationInferenceAttempt,
    pub rounds: Vec<CreationInferenceRound>,
    pub job: JobSnapshot,
    pub regenerated: bool,
}

#[derive(Debug, Clone)]
pub struct CreatorWorkspace {
    pub workflow: CreationWorkflow,
    pub proposal: CreationProposal,
    pub history: Vec<CreationProposal>,
    pub applied_id: Option<String>,
    pub dialogue: Vec<CreatorDialogue>,
    /// The newest turn of the session, including hand edits, which have no dialogue entry.
    pub latest_turn_id: Option<CreationTurnId>,
    pub generated_images: Vec<CreatorGeneratedImage>,
}

/// An image a creator tool call asked for. Until `asset_id` is set or `failed` is true, its `job_id` is still making
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatorGeneratedImage {
    pub attempt_id: GenerationAttemptId,
    pub turn_id: CreationTurnId,
    pub call_ordinal: u16,
    pub job_id: Option<JobId>,
    pub asset_id: Option<AssetId>,
    pub failed: bool,
    pub prompt: String,
    pub source_asset_id: Option<AssetId>,
    pub created_at: TimestampMillis,
}

pub(crate) fn generated_images_in(
    connection: &Connection,
    filter: &str,
    id: String,
) -> Result<Vec<CreatorGeneratedImage>, CreationRepositoryError> {
    connection
        .prepare(&format!(
            "SELECT image.attempt_id,attempt.turn_id,image.call_ordinal,image.job_id,image.asset_id,\
                    image.failed,image.prompt,image.source_asset_id,image.created_at \
             FROM creation_generated_images image \
             JOIN creation_inference_attempts attempt ON attempt.id=image.attempt_id \
             WHERE image.{filter}=?1 ORDER BY image.created_at,image.attempt_id,image.call_ordinal"
        ))
        .map_err(storage)?
        .query_map([id], |row| {
            Ok(CreatorGeneratedImage {
                attempt_id: parse_id(row.get(0)?)?,
                turn_id: parse_id(row.get(1)?)?,
                call_ordinal: u16::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                job_id: row.get::<_, Option<String>>(3)?.map(parse_id).transpose()?,
                asset_id: row.get::<_, Option<String>>(4)?.map(parse_id).transpose()?,
                failed: row.get(5)?,
                prompt: row.get(6)?,
                source_asset_id: row.get::<_, Option<String>>(7)?.map(parse_id).transpose()?,
                created_at: TimestampMillis::new(row.get(8)?),
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)
}

/// A generated image row to start, before its job is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCreatorGeneratedImage {
    pub workflow_id: CreationWorkflowId,
    pub attempt_id: GenerationAttemptId,
    pub call_ordinal: u16,
    pub prompt: String,
    pub source_asset_id: Option<AssetId>,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone)]
pub struct CreatorUserEdit {
    pub workflow_id: CreationWorkflowId,
    pub expected_revision: Revision,
    pub turn_id: CreationTurnId,
    pub proposal_id: CreationProposalId,
    pub draft: CreationDraft,
    pub review: bool,
    pub now: TimestampMillis,
}

fn applied_id(
    connection: &Connection,
    id: CreationWorkflowId,
) -> Result<Option<String>, CreationRepositoryError> {
    connection.query_row(
        "SELECT character_id FROM creation_character_apply_receipts WHERE workflow_id=?1 UNION ALL SELECT persona_id FROM creation_apply_receipts WHERE workflow_id=?1",
        [id.to_string()], |row| row.get(0),
    ).optional().map_err(storage)
}

fn proposals(
    connection: &Connection,
    id: CreationWorkflowId,
) -> Result<Vec<CreationProposal>, CreationRepositoryError> {
    let ids = connection
        .prepare("SELECT id FROM creation_proposals WHERE workflow_id=?1 ORDER BY ordinal,id")
        .map_err(storage)?
        .query_map([id.to_string()], |row| parse_id(row.get(0)?))
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<CreationProposalId>>>()
        .map_err(storage)?;
    ids.into_iter()
        .map(|id| load_proposal_conn(connection, id))
        .collect()
}

fn editable(
    transaction: &Transaction<'_>,
    id: CreationWorkflowId,
    revision: Revision,
) -> Result<CreationWorkflow, CreationRepositoryError> {
    let workflow = load_workflow_conn(transaction, id)?;
    if workflow.target.kind() == CreationTargetKind::Lorebook
        || workflow.revision != revision
        || workflow_applied(transaction, id)?
        || workflow_has_active_attempt(transaction, id)?
    {
        return Err(CreationRepositoryError::Conflict);
    }
    Ok(workflow)
}

impl ApiOperationTransaction<'_, '_> {
    pub fn creator_apply_new_character(
        &self,
        request: ConfirmedCharacterApply,
    ) -> Result<CreationCharacterApplyReceipt, CreationRepositoryError> {
        let (workflow_id, proposal_id, now) =
            (request.workflow_id, request.proposal_id, request.now);
        let receipt = apply_new_character_in(self.transaction, request)?;
        if receipt.applied_at == now
            && let Some(avatar) = self.changed_avatar(workflow_id, proposal_id)?
        {
            self.set_library_character_media(
                receipt.character_id,
                receipt.character_revision,
                lettuce_characters::CharacterMediaSlot::AvatarOriginal,
                0,
                avatar,
                now,
            )
            .map_err(authored_error)?;
        }
        Ok(receipt)
    }

    pub fn creator_apply_existing_character(
        &self,
        request: ConfirmedCharacterRevisionApply,
    ) -> Result<CreationCharacterApplyReceipt, CreationRepositoryError> {
        let (workflow_id, proposal_id, now) =
            (request.workflow_id, request.proposal_id, request.now);
        let receipt = apply_existing_character_in(self.transaction, request)?;
        if receipt.applied_at == now
            && let Some(avatar) = self.changed_avatar(workflow_id, proposal_id)?
        {
            self.set_library_character_media(
                receipt.character_id,
                receipt.character_revision,
                lettuce_characters::CharacterMediaSlot::AvatarOriginal,
                0,
                avatar,
                now,
            )
            .map_err(authored_error)?;
        }
        Ok(receipt)
    }

    pub fn creator_apply_new_persona(
        &self,
        request: ConfirmedPersonaApply,
    ) -> Result<CreationApplyReceipt, CreationRepositoryError> {
        let (workflow_id, proposal_id, now) =
            (request.workflow_id, request.proposal_id, request.now);
        let receipt = apply_new_persona_in(self.transaction, request)?;
        self.apply_persona_avatar(&receipt, workflow_id, proposal_id, now)?;
        Ok(receipt)
    }

    pub fn creator_apply_existing_persona(
        &self,
        request: ConfirmedPersonaRevisionApply,
    ) -> Result<CreationApplyReceipt, CreationRepositoryError> {
        let (workflow_id, proposal_id, now) =
            (request.workflow_id, request.proposal_id, request.now);
        let receipt = apply_existing_persona_in(self.transaction, request)?;
        self.apply_persona_avatar(&receipt, workflow_id, proposal_id, now)?;
        Ok(receipt)
    }

    fn apply_persona_avatar(
        &self,
        receipt: &CreationApplyReceipt,
        workflow_id: CreationWorkflowId,
        proposal_id: CreationProposalId,
        now: TimestampMillis,
    ) -> Result<(), CreationRepositoryError> {
        if receipt.applied_at == now
            && let Some(avatar) = self.changed_avatar(workflow_id, proposal_id)?
        {
            self.set_library_persona_media(
                receipt.persona_id,
                receipt.persona_revision,
                lettuce_characters::PersonaMediaSlot::Avatar,
                0,
                avatar,
                now,
            )
            .map_err(authored_error)?;
        }
        Ok(())
    }

    /// The avatar to write when the applied draft's avatar differs from the one the session started from;
    /// `Some(None)` removes it.
    fn changed_avatar(
        &self,
        workflow_id: CreationWorkflowId,
        proposal_id: CreationProposalId,
    ) -> Result<Option<Option<AssetId>>, CreationRepositoryError> {
        let applied = load_proposal_conn(self.transaction, proposal_id)?
            .draft
            .avatar();
        let initial_id: CreationProposalId = self
            .transaction
            .query_row(
                "SELECT id FROM creation_proposals WHERE workflow_id=?1 AND ordinal=0",
                [workflow_id.to_string()],
                |row| parse_id(row.get(0)?),
            )
            .map_err(storage)?;
        let initial = load_proposal_conn(self.transaction, initial_id)?
            .draft
            .avatar();
        Ok((applied != initial).then_some(applied))
    }

    pub fn creator_check_proposal(
        &self,
        id: CreationWorkflowId,
        proposal: CreationProposalId,
    ) -> Result<(), CreationRepositoryError> {
        if load_workflow_conn(self.transaction, id)?.current_proposal_id != proposal {
            return Err(CreationRepositoryError::Conflict);
        }
        Ok(())
    }
    pub fn creator_create(
        &self,
        input: NewCreationWorkflow,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        input
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid {
                reason: "invalid creation draft",
            })?;
        if input.target.kind() == CreationTargetKind::Lorebook {
            return Err(CreationRepositoryError::Invalid {
                reason: "unsupported creator target",
            });
        }
        let initial = CreationProposal::initial(
            input.initial_proposal_id,
            input.initial_draft.clone(),
            input.now,
        )
        .map_err(|_| CreationRepositoryError::Invalid {
            reason: "invalid creation draft",
        })?;
        insert_workflow_in(self.transaction, &input, &initial)?;
        load_workflow_conn(self.transaction, input.id)
    }

    pub fn creator_edit(
        &self,
        input: CreatorUserEdit,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        let CreatorUserEdit {
            workflow_id: id,
            expected_revision: expected,
            turn_id,
            proposal_id,
            draft,
            review,
            now,
        } = input;
        let workflow = editable(self.transaction, id, expected)?;
        if now < workflow.updated_at {
            return Err(CreationRepositoryError::Conflict);
        }
        let base = load_proposal_conn(self.transaction, workflow.current_proposal_id)?;
        let mut operations = vec![CreationOperation::ReplaceDraft { draft }];
        if review {
            operations.push(CreationOperation::RequestConfirmation);
        }
        let proposal = base
            .apply(proposal_id, turn_id, operations, now)
            .map_err(|_| CreationRepositoryError::Invalid {
                reason: "invalid creation draft",
            })?;
        if proposal.outcomes.iter().any(|outcome| !outcome.succeeded()) {
            return Err(CreationRepositoryError::Invalid {
                reason: "invalid creation draft",
            });
        }
        let ordinal = next_ordinal(self.transaction, "creation_turns", id)?;
        self.transaction.execute("INSERT INTO creation_turns(id,workflow_id,ordinal,base_proposal_id,user_message,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![turn_id.to_string(),id.to_string(),ordinal,base.id.to_string(),turn_id.to_string(),now.get()]).map_err(storage)?;
        insert_proposal(self.transaction, id, &proposal)?;
        let next = expected.next().map_err(storage)?;
        self.transaction.execute("UPDATE creation_workflows SET stage=?2,current_proposal_id=?3,revision=?4,updated_at=?5 WHERE id=?1",
            params![id.to_string(),stage_name(proposal.stage),proposal.id.to_string(),sql_u64(next.get())?,now.get()]).map_err(storage)?;
        load_workflow_conn(self.transaction, id)
    }

    pub fn creator_enqueue(
        &self,
        spec: NewJob,
        key: &str,
        digest: &str,
        detail: &serde_json::Value,
        mut input: NewCreationTurnAttempt,
        regenerate: Option<CreationTurnId>,
    ) -> Result<JobSnapshot, CreationRepositoryError> {
        editable(
            self.transaction,
            input.workflow_id,
            input.expected_workflow_revision,
        )?;
        let (job, _, _) =
            crate::job_adapter::admit_job_detail_in(self.transaction, spec, key, digest, detail)
                .map_err(storage)?;
        input.job_id = job.id;
        if let Some(regenerated_turn_id) = regenerate {
            admit_regeneration_in(
                self.transaction,
                NewCreationRegeneration {
                    workflow_id: input.workflow_id,
                    expected_workflow_revision: input.expected_workflow_revision,
                    regenerated_turn_id,
                    turn_id: input.turn_id,
                    attempt_id: input.attempt_id,
                    planned_proposal_id: input.planned_proposal_id,
                    job_id: job.id,
                    profile_fingerprint: input.profile_fingerprint,
                    now: input.now,
                },
            )?;
        } else {
            admit_turn_in(self.transaction, input)?;
        }
        Ok(job)
    }
}

impl Database {
    pub fn creator_reconcile(
        &self,
        id: CreationWorkflowId,
        now: TimestampMillis,
    ) -> Result<(), CreationRepositoryError> {
        let attempts = {
            let connection = self.connection().map_err(storage)?;
            connection.prepare("SELECT id FROM creation_inference_attempts WHERE workflow_id=?1 AND status IN ('created','running')")
                .map_err(storage)?.query_map([id.to_string()], |row| parse_id(row.get(0)?))
                .map_err(storage)?.collect::<rusqlite::Result<Vec<GenerationAttemptId>>>().map_err(storage)?
        };
        for id in attempts {
            let attempt = self.load_creation_attempt(id)?;
            let job = JobStore::get(self, attempt.job_id)
                .map_err(storage)?
                .ok_or(CreationRepositoryError::NotFound)?;
            if job.state.is_terminal() {
                let status = if attempt.status == CreationAttemptStatus::Created
                    || job.state == JobState::Cancelled
                {
                    CreationAttemptStatus::Cancelled
                } else {
                    CreationAttemptStatus::Interrupted
                };
                if let Err(error) = self.transition_creation_attempt(
                    id,
                    attempt.revision,
                    status,
                    None,
                    now.max(attempt.updated_at),
                ) && (error != CreationRepositoryError::Conflict
                    || !self.load_creation_attempt(id)?.status.is_terminal())
                {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    pub fn creator_workspace(
        &self,
        id: CreationWorkflowId,
    ) -> Result<CreatorWorkspace, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let workflow = load_workflow_conn(&transaction, id)?;
        let history = proposals(&transaction, id)?;
        let applied_id = applied_id(&transaction, id)?;
        let ids = transaction.prepare("SELECT a.id,EXISTS(SELECT 1 FROM creation_turns child WHERE child.regenerated_turn_id=t.id) FROM creation_turns t JOIN creation_inference_attempts a ON a.turn_id=t.id WHERE t.workflow_id=?1 AND a.ordinal=(SELECT MAX(other.ordinal) FROM creation_inference_attempts other WHERE other.turn_id=t.id) ORDER BY t.ordinal")
                .map_err(storage)?.query_map([id.to_string()], |row| Ok((parse_id(row.get(0)?)?,row.get::<_,bool>(1)?)))
                .map_err(storage)?.collect::<rusqlite::Result<Vec<(GenerationAttemptId,bool)>>>().map_err(storage)?;
        if workflow.target.kind() == CreationTargetKind::Lorebook {
            return Err(CreationRepositoryError::NotFound);
        }
        let proposal = load_proposal_conn(&transaction, workflow.current_proposal_id)?;
        let dialogue = ids
            .into_iter()
            .map(|(id, regenerated)| {
                let attempt = load_attempt_conn(&transaction, id)?;
                let turn = load_turn_conn(&transaction, attempt.turn_id)?;
                let rounds = list_rounds_in(
                    &transaction,
                    CreationAttemptOwner {
                        workflow_id: workflow.id,
                        turn_id: turn.id,
                    },
                    id,
                )?;
                let encoded: String = transaction
                    .query_row(
                        "SELECT snapshot_json FROM jobs WHERE id=?1",
                        [attempt.job_id.to_string()],
                        |row| row.get(0),
                    )
                    .map_err(storage)?;
                let job = decode_versioned(&encoded, 1).map_err(storage)?;
                Ok(CreatorDialogue {
                    turn,
                    attempt,
                    rounds,
                    job,
                    regenerated,
                })
            })
            .collect::<Result<Vec<_>, CreationRepositoryError>>()?;
        let latest_turn_id = transaction
            .query_row(
                "SELECT id FROM creation_turns WHERE workflow_id=?1 ORDER BY ordinal DESC LIMIT 1",
                [id.to_string()],
                |row| parse_id(row.get(0)?),
            )
            .optional()
            .map_err(storage)?;
        let generated_images = generated_images_in(&transaction, "workflow_id", id.to_string())?;
        transaction.commit().map_err(storage)?;
        Ok(CreatorWorkspace {
            workflow,
            proposal,
            history,
            applied_id,
            dialogue,
            latest_turn_id,
            generated_images,
        })
    }

    /// The images uploaded with the session's messages, in message order, each once.
    pub fn creator_uploads(
        &self,
        id: CreationWorkflowId,
    ) -> Result<Vec<AssetId>, CreationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let ids = connection
            .prepare(
                "SELECT image.value FROM creation_turns turn, json_each(turn.inputs_json,'$.images') image \
                 WHERE turn.workflow_id=?1 ORDER BY turn.ordinal, image.key",
            )
            .map_err(storage)?
            .query_map([id.to_string()], |row| parse_id(row.get(0)?))
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<AssetId>>>()
            .map_err(storage)?;
        let mut seen = std::collections::HashSet::new();
        Ok(ids.into_iter().filter(|id| seen.insert(*id)).collect())
    }

    pub fn creator_attempt_images(
        &self,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<CreatorGeneratedImage>, CreationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        generated_images_in(&connection, "attempt_id", attempt_id.to_string())
    }

    /// Records a generated image the creator asked for; an existing row for the same call is kept as is.
    pub fn creator_begin_image(
        &self,
        input: NewCreatorGeneratedImage,
    ) -> Result<(), CreationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO creation_generated_images \
                 (attempt_id,call_ordinal,workflow_id,prompt,source_asset_id,created_at) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    input.attempt_id.to_string(),
                    i64::from(input.call_ordinal),
                    input.workflow_id.to_string(),
                    input.prompt,
                    input.source_asset_id.map(|id| id.to_string()),
                    input.now.get(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    pub fn creator_image_job(
        &self,
        attempt_id: GenerationAttemptId,
        call_ordinal: u16,
        job_id: JobId,
    ) -> Result<(), CreationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "UPDATE creation_generated_images SET job_id=?3 \
                 WHERE attempt_id=?1 AND call_ordinal=?2 AND asset_id IS NULL AND failed=0",
                params![
                    attempt_id.to_string(),
                    i64::from(call_ordinal),
                    job_id.to_string()
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Settles a generated image with its asset, or as failed when `asset_id` is `None`.
    pub fn creator_settle_image(
        &self,
        attempt_id: GenerationAttemptId,
        call_ordinal: u16,
        asset_id: Option<AssetId>,
    ) -> Result<(), CreationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "UPDATE creation_generated_images SET asset_id=?3,failed=?4 \
                 WHERE attempt_id=?1 AND call_ordinal=?2 AND asset_id IS NULL AND failed=0",
                params![
                    attempt_id.to_string(),
                    i64::from(call_ordinal),
                    asset_id.map(|id| id.to_string()),
                    asset_id.is_none(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    pub fn creator_images(
        &self,
        id: CreationWorkflowId,
    ) -> Result<Vec<AssetId>, CreationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        workflow_images_in(&connection, id)
    }

    /// The most recently updated unsaved session that edits the same existing character or persona.
    pub fn creator_open_workflow(
        &self,
        target: &CreationTarget,
    ) -> Result<Option<CreationWorkflowId>, CreationRepositoryError> {
        let (kind, target_id) = match target {
            CreationTarget::ExistingCharacter { id, .. } => ("existing_character", id.to_string()),
            CreationTarget::ExistingPersona { id, .. } => ("existing_persona", id.to_string()),
            _ => return Ok(None),
        };
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                "SELECT w.id FROM creation_workflows w \
                 WHERE json_extract(w.target_json,'$.kind')=?1 AND json_extract(w.target_json,'$.id')=?2 \
                 AND NOT EXISTS(SELECT 1 FROM creation_apply_receipts r WHERE r.workflow_id=w.id) \
                 AND NOT EXISTS(SELECT 1 FROM creation_character_apply_receipts r WHERE r.workflow_id=w.id) \
                 ORDER BY w.updated_at DESC, w.id DESC LIMIT 1",
                params![kind, target_id],
                |row| parse_id(row.get(0)?),
            )
            .optional()
            .map_err(storage)
    }

    pub fn creator_workflows(
        &self,
        page: PageRequest,
    ) -> Result<Page<CreationWorkflow>, CreationRepositoryError> {
        let cursor = page
            .cursor
            .as_deref()
            .map(|cursor| {
                serde_json::from_str::<(i64, String)>(cursor).map_err(|_| {
                    CreationRepositoryError::Invalid {
                        reason: "invalid creator cursor",
                    }
                })
            })
            .transpose()?;
        let connection = self.connection().map_err(storage)?;
        let ids = connection.prepare("SELECT id FROM creation_workflows WHERE json_extract(target_json,'$.kind') IN ('new_character','existing_character','new_persona','existing_persona') AND (?1 IS NULL OR (updated_at,id)<(?1,?2)) ORDER BY updated_at DESC,id DESC LIMIT ?3")
            .map_err(storage)?.query_map(params![cursor.as_ref().map(|c|c.0),cursor.as_ref().map(|c|c.1.as_str()),u32::from(page.limit.get())+1], |row| parse_id(row.get(0)?))
            .map_err(storage)?.collect::<rusqlite::Result<Vec<CreationWorkflowId>>>().map_err(storage)?;
        let mut items = ids
            .into_iter()
            .map(|id| load_workflow_conn(&connection, id))
            .collect::<Result<Vec<_>, _>>()?;
        let more = items.len() > usize::from(page.limit.get());
        if more {
            items.pop();
        }
        let next_cursor = if more {
            items
                .last()
                .map(|workflow| {
                    serde_json::to_string(&(workflow.updated_at.get(), workflow.id.to_string()))
                        .map_err(storage)
                })
                .transpose()?
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }
}
