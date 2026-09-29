use lettuce_conversations::{
    SceneFollowUp, SceneFollowUpChange, SceneFollowUpError, SceneFollowUpMode,
    SceneFollowUpRepository, SceneFollowUpState, SceneFollowUpTarget,
};
use lettuce_types::{ConversationId, MessageId, RequestId, TimestampMillis};
use rusqlite::{OptionalExtension, Row, params};

use crate::Database;

const COLUMNS: &str = "conversation_id, message_id, prompt, mode, state, generation, attempt, request_id, failure, created_at, updated_at, target_kind, target_id";

fn storage<E>(_: E) -> SceneFollowUpError {
    SceneFollowUpError::Storage
}

pub(crate) const fn mode_text(mode: SceneFollowUpMode) -> &'static str {
    match mode {
        SceneFollowUpMode::Auto => "auto",
        SceneFollowUpMode::AskFirst => "ask_first",
        SceneFollowUpMode::Manual => "manual",
    }
}

pub(crate) const fn state_text(state: SceneFollowUpState) -> &'static str {
    match state {
        SceneFollowUpState::Pending => "pending",
        SceneFollowUpState::Approved => "approved",
        SceneFollowUpState::Running => "running",
        SceneFollowUpState::AwaitingTurn => "awaiting_turn",
        SceneFollowUpState::Done => "done",
        SceneFollowUpState::Failed => "failed",
        SceneFollowUpState::Dismissed => "dismissed",
    }
}

fn parse_mode(text: &str) -> Result<SceneFollowUpMode, SceneFollowUpError> {
    Ok(match text {
        "auto" => SceneFollowUpMode::Auto,
        "ask_first" => SceneFollowUpMode::AskFirst,
        "manual" => SceneFollowUpMode::Manual,
        _ => return Err(SceneFollowUpError::Storage),
    })
}

fn parse_state(text: &str) -> Result<SceneFollowUpState, SceneFollowUpError> {
    Ok(match text {
        "pending" => SceneFollowUpState::Pending,
        "approved" => SceneFollowUpState::Approved,
        "running" => SceneFollowUpState::Running,
        "awaiting_turn" => SceneFollowUpState::AwaitingTurn,
        "done" => SceneFollowUpState::Done,
        "failed" => SceneFollowUpState::Failed,
        "dismissed" => SceneFollowUpState::Dismissed,
        _ => return Err(SceneFollowUpError::Storage),
    })
}

fn read(row: &Row<'_>) -> rusqlite::Result<Result<SceneFollowUp, SceneFollowUpError>> {
    let conversation_id: String = row.get(0)?;
    let message_id: String = row.get(1)?;
    let mode: String = row.get(3)?;
    let state: String = row.get(4)?;
    let request_id: Option<String> = row.get(7)?;
    let generation: i64 = row.get(5)?;
    let attempt: i64 = row.get(6)?;
    let created_at: i64 = row.get(9)?;
    let updated_at: i64 = row.get(10)?;
    let prompt: String = row.get(2)?;
    let failure: Option<String> = row.get(8)?;
    let target_kind: String = row.get(11)?;
    let target_id: String = row.get(12)?;
    Ok((|| {
        Ok(SceneFollowUp {
            conversation_id: conversation_id.parse().map_err(storage)?,
            message_id: message_id.parse().map_err(storage)?,
            target: match target_kind.as_str() {
                "candidate" => SceneFollowUpTarget::Candidate(target_id.parse().map_err(storage)?),
                "starter_revision" => {
                    SceneFollowUpTarget::StarterRevision(target_id.parse().map_err(storage)?)
                }
                _ => return Err(SceneFollowUpError::Storage),
            },
            prompt,
            mode: parse_mode(&mode)?,
            state: parse_state(&state)?,
            generation: u32::try_from(generation).map_err(storage)?,
            attempt: u32::try_from(attempt).map_err(storage)?,
            request_id: request_id
                .map(|id| id.parse::<RequestId>())
                .transpose()
                .map_err(storage)?,
            failure,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        })
    })())
}

pub(crate) fn one(
    connection: &rusqlite::Connection,
    conversation_id: ConversationId,
    target: SceneFollowUpTarget,
) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
    connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM scene_image_follow_ups WHERE conversation_id = ?1 AND target_kind = ?2 AND target_id = ?3"
            ),
            params![conversation_id.to_string(), target.kind(), target.id().to_string()],
            read,
        )
        .optional()
        .map_err(storage)?
        .transpose()
}

pub(crate) fn all_in(
    connection: &rusqlite::Connection,
) -> Result<Vec<SceneFollowUp>, SceneFollowUpError> {
    connection.prepare(&format!("SELECT {COLUMNS} FROM scene_image_follow_ups ORDER BY conversation_id, target_kind, target_id")).map_err(storage)?.query_map([], read).map_err(storage)?.collect::<rusqlite::Result<Vec<_>>>().map_err(storage)?.into_iter().collect()
}

pub(crate) fn insert_restored_in(
    connection: &rusqlite::Connection,
    follow_up: &SceneFollowUp,
) -> Result<(), SceneFollowUpError> {
    let target = follow_up.target;
    connection.execute("INSERT INTO scene_image_follow_ups (conversation_id, target_kind, target_id, candidate_id, starter_revision_id, message_id, prompt, mode, state, generation, attempt, request_id, failure, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)", params![follow_up.conversation_id.to_string(), target.kind(), target.id(), if let SceneFollowUpTarget::Candidate(id) = target { Some(id.to_string()) } else { None }, if let SceneFollowUpTarget::StarterRevision(id) = target { Some(id.to_string()) } else { None }, follow_up.message_id.to_string(), follow_up.prompt, mode_text(follow_up.mode), state_text(follow_up.state), follow_up.generation, follow_up.attempt, follow_up.request_id.map(|id| id.to_string()), follow_up.failure, follow_up.created_at.get(), follow_up.updated_at.get()]).map_err(storage)?;
    Ok(())
}

impl SceneFollowUpRepository for Database {
    fn get_follow_up(
        &self,
        conversation_id: ConversationId,
        target: SceneFollowUpTarget,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        let connection = self.connection().map_err(storage)?;
        one(&connection, conversation_id, target)
    }

    fn follow_ups_of(
        &self,
        conversation_id: ConversationId,
        targets: &[SceneFollowUpTarget],
    ) -> Result<Vec<SceneFollowUp>, SceneFollowUpError> {
        let connection = self.connection().map_err(storage)?;
        let mut found = Vec::new();
        for target in targets {
            if let Some(follow_up) = one(&connection, conversation_id, *target)? {
                found.push(follow_up);
            }
        }
        Ok(found)
    }

    fn follow_up_of_request(
        &self,
        request_id: RequestId,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        self.connection()
            .map_err(storage)?
            .query_row(
                &format!("SELECT {COLUMNS} FROM scene_image_follow_ups WHERE request_id = ?1"),
                [request_id.to_string()],
                read,
            )
            .optional()
            .map_err(storage)?
            .transpose()
    }

    fn follow_ups_in(
        &self,
        states: &[SceneFollowUpState],
    ) -> Result<Vec<SceneFollowUp>, SceneFollowUpError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {COLUMNS} FROM scene_image_follow_ups WHERE state IN (SELECT value FROM json_each(?1)) ORDER BY created_at, target_kind, target_id"
            ))
            .map_err(storage)?;
        let states =
            serde_json::to_string(&states.iter().copied().map(state_text).collect::<Vec<_>>())
                .map_err(storage)?;
        let rows = statement
            .query_map([states], read)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage)?;
        rows.into_iter().collect()
    }

    fn ensure_follow_up(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
        target: SceneFollowUpTarget,
        prompt: &str,
        mode: SceneFollowUpMode,
        now: TimestampMillis,
    ) -> Result<SceneFollowUp, SceneFollowUpError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "INSERT INTO scene_image_follow_ups (conversation_id, target_kind, target_id, candidate_id, starter_revision_id, message_id, prompt, mode, state, generation, attempt, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', 0, 0, ?9, ?9) ON CONFLICT (conversation_id, target_kind, target_id) DO NOTHING",
                params![conversation_id.to_string(), target.kind(), target.id().to_string(), if let SceneFollowUpTarget::Candidate(id) = target { Some(id.to_string()) } else { None }, if let SceneFollowUpTarget::StarterRevision(id) = target { Some(id.to_string()) } else { None }, message_id.to_string(), prompt, mode_text(mode), now.get()],
            )
            .map_err(storage)?;
        one(&connection, conversation_id, target)?.ok_or(SceneFollowUpError::Storage)
    }

    fn change_follow_up(
        &self,
        conversation_id: ConversationId,
        target: SceneFollowUpTarget,
        from: &[SceneFollowUpState],
        change: &SceneFollowUpChange,
        now: TimestampMillis,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(storage)?;
        let Some(current) = one(&transaction, conversation_id, target)? else {
            return Ok(None);
        };
        if !from.contains(&current.state) {
            return Ok(None);
        }
        let state = change.state.unwrap_or(current.state);
        let request_id = match &change.request_id {
            Some(request_id) => *request_id,
            None => current.request_id,
        };
        let failure = match &change.failure {
            Some(failure) => failure.clone(),
            None => current.failure.clone(),
        };
        let generation = current.generation + u32::from(change.next_generation);
        transaction
            .execute(
                "UPDATE scene_image_follow_ups SET prompt = ?4, mode = ?5, state = ?6, generation = ?7, attempt = ?8, request_id = ?9, failure = ?10, updated_at = ?11 WHERE conversation_id = ?1 AND target_kind = ?2 AND target_id = ?3",
                params![conversation_id.to_string(), target.kind(), target.id().to_string(), change.prompt.as_deref().unwrap_or(&current.prompt), mode_text(change.mode.unwrap_or(current.mode)), state_text(state), generation, change.attempt.unwrap_or(current.attempt), request_id.map(|id| id.to_string()), failure, now.get().max(current.updated_at.get())],
            )
            .map_err(storage)?;
        let changed = one(&transaction, conversation_id, target)?;
        transaction.commit().map_err(storage)?;
        Ok(changed)
    }
}
