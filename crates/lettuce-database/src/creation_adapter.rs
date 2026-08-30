use std::str::FromStr;

use lettuce_creation::{
    CreationOperationOutcome, CreationProposal, CreationRepositoryError, CreationStage,
    CreationTurn, CreationWorkflow, CreationWorkflowRepository, NewCreationTurn,
    NewCreationWorkflow,
};
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

fn storage(_: impl std::fmt::Debug) -> CreationRepositoryError {
    CreationRepositoryError::Storage
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, CreationRepositoryError> {
    serde_json::to_string(value).map_err(|_| CreationRepositoryError::Invalid)
}

fn decode<T: serde::de::DeserializeOwned>(value: String) -> rusqlite::Result<T> {
    serde_json::from_str(&value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn stage_name(stage: CreationStage) -> &'static str {
    match stage {
        CreationStage::Drafting => "drafting",
        CreationStage::AwaitingReview => "awaiting_review",
        CreationStage::AwaitingConfirmation => "awaiting_confirmation",
    }
}

fn parse_stage(value: &str) -> rusqlite::Result<CreationStage> {
    match value {
        "drafting" => Ok(CreationStage::Drafting),
        "awaiting_review" => Ok(CreationStage::AwaitingReview),
        "awaiting_confirmation" => Ok(CreationStage::AwaitingConfirmation),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn parse_id<T: FromStr>(value: String) -> rusqlite::Result<T> {
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn revision(value: i64) -> rusqlite::Result<Revision> {
    let value = u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)?;
    if value == 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(Revision::new(value))
}

fn sql_u64(value: u64) -> Result<i64, CreationRepositoryError> {
    i64::try_from(value).map_err(|_| CreationRepositoryError::Storage)
}

fn load_workflow_conn(
    connection: &Connection,
    id: CreationWorkflowId,
) -> Result<CreationWorkflow, CreationRepositoryError> {
    let workflow = connection
        .query_row(
            "SELECT target_json,stage,current_proposal_id,revision,created_at,updated_at \
             FROM creation_workflows WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationWorkflow {
                    id,
                    target: decode(row.get(0)?)?,
                    stage: parse_stage(&row.get::<_, String>(1)?)?,
                    current_proposal_id: parse_id(
                        row.get::<_, Option<String>>(2)?
                            .ok_or(rusqlite::Error::InvalidQuery)?,
                    )?,
                    revision: revision(row.get(3)?)?,
                    created_at: TimestampMillis::new(row.get(4)?),
                    updated_at: TimestampMillis::new(row.get(5)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    workflow
        .target
        .validate()
        .map_err(|_| CreationRepositoryError::Invalid)?;
    let proposal = load_proposal_conn(connection, workflow.current_proposal_id)?;
    if proposal.stage != workflow.stage || proposal.draft.kind() != workflow.target.kind() {
        return Err(CreationRepositoryError::Invalid);
    }
    Ok(workflow)
}

fn load_proposal_conn(
    connection: &Connection,
    id: CreationProposalId,
) -> Result<CreationProposal, CreationRepositoryError> {
    let proposal = connection
        .query_row(
            "SELECT turn_id,parent_id,ordinal,stage,draft_json,outcomes_json,created_at \
             FROM creation_proposals WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationProposal {
                    id,
                    turn_id: row.get::<_, Option<String>>(0)?.map(parse_id).transpose()?,
                    parent_id: row.get::<_, Option<String>>(1)?.map(parse_id).transpose()?,
                    ordinal: u32::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    stage: parse_stage(&row.get::<_, String>(3)?)?,
                    draft: decode(row.get(4)?)?,
                    outcomes: decode::<Vec<CreationOperationOutcome>>(row.get(5)?)?,
                    created_at: TimestampMillis::new(row.get(6)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    proposal
        .validate()
        .map_err(|_| CreationRepositoryError::Invalid)?;
    Ok(proposal)
}

fn load_turn_conn(
    connection: &Connection,
    id: CreationTurnId,
) -> Result<CreationTurn, CreationRepositoryError> {
    let turn = connection
        .query_row(
            "SELECT workflow_id,ordinal,base_proposal_id,user_message,created_at \
             FROM creation_turns WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationTurn {
                    id,
                    workflow_id: parse_id(row.get(0)?)?,
                    ordinal: u32::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    base_proposal_id: parse_id(row.get(2)?)?,
                    user_message: row.get(3)?,
                    created_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    NewCreationTurn {
        id: turn.id,
        workflow_id: turn.workflow_id,
        base_proposal_id: turn.base_proposal_id,
        user_message: turn.user_message.clone(),
        now: turn.created_at,
    }
    .validate()
    .map_err(|_| CreationRepositoryError::Invalid)?;
    Ok(turn)
}

fn next_ordinal(
    transaction: &Transaction<'_>,
    table: &str,
    workflow_id: CreationWorkflowId,
) -> Result<u32, CreationRepositoryError> {
    let query = format!("SELECT COALESCE(MAX(ordinal), -1) + 1 FROM {table} WHERE workflow_id=?1");
    let ordinal: i64 = transaction
        .query_row(&query, [workflow_id.to_string()], |row| row.get(0))
        .map_err(storage)?;
    u32::try_from(ordinal).map_err(|_| CreationRepositoryError::Storage)
}

fn insert_proposal(
    transaction: &Transaction<'_>,
    workflow_id: CreationWorkflowId,
    proposal: &CreationProposal,
) -> Result<(), CreationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO creation_proposals \
             (id,workflow_id,turn_id,parent_id,ordinal,stage,draft_json,outcomes_json,created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                proposal.id.to_string(),
                workflow_id.to_string(),
                proposal.turn_id.map(|id| id.to_string()),
                proposal.parent_id.map(|id| id.to_string()),
                i64::from(proposal.ordinal),
                stage_name(proposal.stage),
                encode(&proposal.draft)?,
                encode(&proposal.outcomes)?,
                proposal.created_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

impl CreationWorkflowRepository for Database {
    fn create_workflow(
        &self,
        input: NewCreationWorkflow,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        input
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let initial = CreationProposal::initial(
            input.initial_proposal_id,
            input.initial_draft.clone(),
            input.now,
        )
        .map_err(|_| CreationRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_workflow_conn(&transaction, input.id) {
            Ok(existing) => {
                let proposal = load_proposal_conn(&transaction, existing.current_proposal_id)?;
                if existing.target == input.target && proposal == initial {
                    transaction.commit().map_err(storage)?;
                    return Ok(existing);
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        transaction
            .execute(
                "INSERT INTO creation_workflows \
                 (id,target_json,stage,current_proposal_id,revision,created_at,updated_at) \
                 VALUES (?1,?2,'drafting',NULL,1,?3,?3)",
                params![
                    input.id.to_string(),
                    encode(&input.target)?,
                    input.now.get()
                ],
            )
            .map_err(storage)?;
        insert_proposal(&transaction, input.id, &initial)?;
        transaction
            .execute(
                "UPDATE creation_workflows SET current_proposal_id=?2 WHERE id=?1",
                params![input.id.to_string(), input.initial_proposal_id.to_string()],
            )
            .map_err(storage)?;
        let workflow = load_workflow_conn(&transaction, input.id)?;
        transaction.commit().map_err(storage)?;
        Ok(workflow)
    }

    fn load_workflow(
        &self,
        id: CreationWorkflowId,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        load_workflow_conn(&*self.connection().map_err(storage)?, id)
    }

    fn load_proposal(
        &self,
        id: CreationProposalId,
    ) -> Result<CreationProposal, CreationRepositoryError> {
        load_proposal_conn(&*self.connection().map_err(storage)?, id)
    }

    fn record_user_turn(
        &self,
        input: NewCreationTurn,
    ) -> Result<CreationTurn, CreationRepositoryError> {
        input
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_turn_conn(&transaction, input.id) {
            Ok(existing) => {
                if existing.workflow_id == input.workflow_id
                    && existing.base_proposal_id == input.base_proposal_id
                    && existing.user_message == input.user_message
                    && existing.created_at == input.now
                {
                    transaction.commit().map_err(storage)?;
                    return Ok(existing);
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        let workflow = load_workflow_conn(&transaction, input.workflow_id)?;
        if workflow.current_proposal_id != input.base_proposal_id
            || input.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let ordinal = next_ordinal(&transaction, "creation_turns", input.workflow_id)?;
        transaction
            .execute(
                "INSERT INTO creation_turns \
                 (id,workflow_id,ordinal,base_proposal_id,user_message,created_at) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    input.id.to_string(),
                    input.workflow_id.to_string(),
                    i64::from(ordinal),
                    input.base_proposal_id.to_string(),
                    input.user_message,
                    input.now.get(),
                ],
            )
            .map_err(storage)?;
        let turn = load_turn_conn(&transaction, input.id)?;
        transaction.commit().map_err(storage)?;
        Ok(turn)
    }

    fn append_proposal(
        &self,
        workflow_id: CreationWorkflowId,
        expected_workflow_revision: Revision,
        proposal: CreationProposal,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        proposal
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let workflow = load_workflow_conn(&transaction, workflow_id)?;
        if workflow.current_proposal_id == proposal.id {
            let existing = load_proposal_conn(&transaction, proposal.id)?;
            if existing == proposal {
                transaction.commit().map_err(storage)?;
                return Ok(workflow);
            }
            return Err(CreationRepositoryError::Conflict);
        }
        if workflow.revision != expected_workflow_revision
            || proposal.parent_id != Some(workflow.current_proposal_id)
            || proposal.draft.kind() != workflow.target.kind()
            || proposal.created_at.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let base = load_proposal_conn(&transaction, workflow.current_proposal_id)?;
        let expected = base
            .apply(
                proposal.id,
                proposal.turn_id.ok_or(CreationRepositoryError::Invalid)?,
                proposal
                    .outcomes
                    .iter()
                    .map(|outcome| outcome.operation.clone())
                    .collect(),
                proposal.created_at,
            )
            .map_err(|_| CreationRepositoryError::Invalid)?;
        if expected != proposal {
            return Err(CreationRepositoryError::Conflict);
        }
        let stage_allowed = matches!(
            (workflow.stage, proposal.stage),
            (
                CreationStage::Drafting,
                CreationStage::Drafting | CreationStage::AwaitingReview
            ) | (
                CreationStage::AwaitingReview,
                CreationStage::AwaitingReview | CreationStage::AwaitingConfirmation
            ) | (
                CreationStage::AwaitingConfirmation,
                CreationStage::AwaitingConfirmation
            )
        );
        if proposal.ordinal
            != base
                .ordinal
                .checked_add(1)
                .ok_or(CreationRepositoryError::Storage)?
            || !stage_allowed
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let turn = load_turn_conn(
            &transaction,
            proposal.turn_id.ok_or(CreationRepositoryError::Invalid)?,
        )?;
        if turn.workflow_id != workflow_id
            || turn.base_proposal_id != workflow.current_proposal_id
            || turn.created_at.get() > proposal.created_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let expected_ordinal = next_ordinal(&transaction, "creation_proposals", workflow_id)?;
        if proposal.ordinal != expected_ordinal {
            return Err(CreationRepositoryError::Conflict);
        }
        insert_proposal(&transaction, workflow_id, &proposal)?;
        let next_revision = workflow
            .revision
            .next()
            .map_err(|_| CreationRepositoryError::Storage)?;
        let changed = transaction
            .execute(
                "UPDATE creation_workflows SET stage=?2,current_proposal_id=?3,revision=?4,updated_at=?5 \
                 WHERE id=?1 AND revision=?6 AND current_proposal_id=?7",
                params![
                    workflow_id.to_string(),
                    stage_name(proposal.stage),
                    proposal.id.to_string(),
                    sql_u64(next_revision.get())?,
                    proposal.created_at.get(),
                    sql_u64(expected_workflow_revision.get())?,
                    workflow.current_proposal_id.to_string(),
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(CreationRepositoryError::Conflict);
        }
        let updated = load_workflow_conn(&transaction, workflow_id)?;
        transaction.commit().map_err(storage)?;
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::ProposedToolCall;
    use lettuce_creation::{
        AdmittedCreationToolCall, CreationDraft, CreationOperation, CreationOperationError,
        CreationStage, CreationTarget, CreationToolApply, CreationWorkflowRepository,
        NewCreationTurn, NewCreationWorkflow, apply_creation_tool_calls,
    };
    use lettuce_types::{
        CreationProposalId, CreationTurnId, CreationWorkflowId, Revision, SceneId, TimestampMillis,
    };

    use crate::Database;

    fn admitted(name: &str, arguments: serde_json::Value) -> AdmittedCreationToolCall {
        AdmittedCreationToolCall {
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("call-{name}")),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
        }
    }

    #[test]
    fn durable_turns_proposals_and_review_transitions_are_atomic_and_retryable() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let initial_id = CreationProposalId::new();
        let input = NewCreationWorkflow {
            id: workflow_id,
            initial_proposal_id: initial_id,
            target: CreationTarget::NewCharacter,
            initial_draft: CreationDraft::Character {
                name: None,
                definition: None,
                scenes: Vec::new(),
            },
            now: TimestampMillis::new(10),
        };
        let workflow = database
            .create_workflow(input.clone())
            .expect("create workflow");
        assert_eq!(
            database.create_workflow(input).expect("exact retry"),
            workflow
        );

        let turn_id = CreationTurnId::new();
        let turn_input = NewCreationTurn {
            id: turn_id,
            workflow_id,
            base_proposal_id: initial_id,
            user_message: "Build a quiet guide".to_owned(),
            now: TimestampMillis::new(11),
        };
        let turn = database
            .record_user_turn(turn_input.clone())
            .expect("durable turn");
        assert_eq!(
            database
                .record_user_turn(turn_input)
                .expect("turn exact retry"),
            turn
        );
        assert_eq!(
            database
                .load_workflow(workflow_id)
                .expect("workflow before inference")
                .current_proposal_id,
            initial_id
        );

        let base = database.load_proposal(initial_id).expect("base proposal");
        let competing_turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: initial_id,
                user_message: "Use another name".to_owned(),
                now: TimestampMillis::new(11),
            })
            .expect("competing durable turn");
        let competing = base
            .apply(
                CreationProposalId::new(),
                competing_turn.id,
                vec![CreationOperation::SetName {
                    value: "Bramble".to_owned(),
                }],
                TimestampMillis::new(12),
            )
            .expect("competing proposal");
        let scene_id = SceneId::new();
        let proposal = base
            .apply(
                CreationProposalId::new(),
                turn_id,
                vec![
                    CreationOperation::UpdateScene {
                        id: SceneId::new(),
                        content: "missing".to_owned(),
                        direction: None,
                    },
                    CreationOperation::SetName {
                        value: "Aster".to_owned(),
                    },
                    CreationOperation::AddScene {
                        id: scene_id,
                        content: "Welcome.".to_owned(),
                        direction: Some("calmly".to_owned()),
                    },
                    CreationOperation::ShowPreview,
                ],
                TimestampMillis::new(12),
            )
            .expect("reduce proposal");
        assert_eq!(
            proposal.outcomes[0].error,
            Some(CreationOperationError::NotFound)
        );
        assert!(
            proposal.outcomes[1..]
                .iter()
                .all(|outcome| outcome.succeeded())
        );
        let mut forged = proposal.clone();
        forged.stage = CreationStage::AwaitingConfirmation;
        assert_eq!(
            database.append_proposal(workflow_id, Revision::INITIAL, forged),
            Err(lettuce_creation::CreationRepositoryError::Conflict)
        );
        let reviewed = database
            .append_proposal(workflow_id, Revision::INITIAL, proposal.clone())
            .expect("append reviewed proposal");
        assert_eq!(reviewed.stage, CreationStage::AwaitingReview);
        assert_eq!(reviewed.revision, Revision::new(2));
        assert_eq!(
            database.append_proposal(workflow_id, Revision::INITIAL, competing),
            Err(lettuce_creation::CreationRepositoryError::Conflict)
        );
        assert_eq!(
            database
                .append_proposal(workflow_id, Revision::INITIAL, proposal.clone())
                .expect("proposal exact retry"),
            reviewed
        );

        let stale_turn = database.record_user_turn(NewCreationTurn {
            id: CreationTurnId::new(),
            workflow_id,
            base_proposal_id: initial_id,
            user_message: "stale".to_owned(),
            now: TimestampMillis::new(13),
        });
        assert_eq!(
            stale_turn,
            Err(lettuce_creation::CreationRepositoryError::Conflict)
        );

        let confirm_turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: proposal.id,
                user_message: "Looks good".to_owned(),
                now: TimestampMillis::new(14),
            })
            .expect("confirmation turn");
        let confirmation = proposal
            .apply(
                CreationProposalId::new(),
                confirm_turn.id,
                vec![CreationOperation::RequestConfirmation],
                TimestampMillis::new(15),
            )
            .expect("confirmation proposal");
        let confirmed = database
            .append_proposal(workflow_id, reviewed.revision, confirmation.clone())
            .expect("append confirmation");
        assert_eq!(confirmed.stage, CreationStage::AwaitingConfirmation);
        assert_eq!(
            database
                .load_proposal(confirmation.id)
                .expect("stored confirmation"),
            confirmation
        );

        let connection = database.connection().expect("database lock");
        let authored_rows: i64 = connection
            .query_row(
                "SELECT (SELECT count(*) FROM characters) + \
                        (SELECT count(*) FROM personas) + \
                        (SELECT count(*) FROM lorebooks)",
                [],
                |row| row.get(0),
            )
            .expect("authored rows");
        assert_eq!(authored_rows, 0);
        assert!(
            connection
                .execute(
                    "UPDATE creation_proposals SET stage='drafting' WHERE id=?1",
                    [confirmation.id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE creation_turns SET user_message='changed' WHERE id=?1",
                    [confirm_turn.id.to_string()],
                )
                .is_err()
        );
    }

    #[test]
    fn native_tool_bridge_persists_one_retry_stable_proposal_and_outputs() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let initial_id = CreationProposalId::new();
        let workflow = database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: initial_id,
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: None,
                    description: None,
                },
                now: TimestampMillis::new(1),
            })
            .expect("workflow");
        let turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: initial_id,
                user_message: "Create a navigator persona".to_owned(),
                now: TimestampMillis::new(2),
            })
            .expect("turn");
        let proposal_id = CreationProposalId::new();
        let calls = vec![
            admitted("set_persona_name", serde_json::json!({"name": "Navigator"})),
            admitted(
                "set_persona_description",
                serde_json::json!({"description": "Charts careful routes."}),
            ),
            admitted("show_preview", serde_json::json!({})),
        ];
        let committed = apply_creation_tool_calls(
            &database,
            CreationToolApply {
                workflow_id,
                expected_workflow_revision: workflow.revision,
                base_proposal_id: initial_id,
                proposal_id,
                turn_id: turn.id,
                calls: calls.clone(),
                now: TimestampMillis::new(3),
            },
        )
        .expect("tool commit");
        assert_eq!(committed.workflow.stage, CreationStage::AwaitingReview);
        assert_eq!(committed.outputs.len(), calls.len());
        assert!(committed.outputs.iter().all(|output| !output.is_error));
        assert_eq!(committed.outputs[0].value["tool"], "set_persona_name");
        let retry = apply_creation_tool_calls(
            &database,
            CreationToolApply {
                workflow_id,
                expected_workflow_revision: workflow.revision,
                base_proposal_id: initial_id,
                proposal_id,
                turn_id: turn.id,
                calls,
                now: TimestampMillis::new(3),
            },
        )
        .expect("exact tool retry");
        assert_eq!(retry, committed);
    }
}
