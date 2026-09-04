use std::str::FromStr;

use lettuce_companions::{
    CompanionScheduledNote, CompanionScheduledNoteError, CompanionScheduledNoteRepository,
    ScheduledNoteRecurrence,
};
use lettuce_types::{CharacterId, TimestampMillis};
use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

use crate::Database;

fn failure(_: impl std::fmt::Debug) -> CompanionScheduledNoteError {
    CompanionScheduledNoteError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> CompanionScheduledNoteError {
    CompanionScheduledNoteError::Corrupt
}

fn recurrence_name(recurrence: ScheduledNoteRecurrence) -> &'static str {
    match recurrence {
        ScheduledNoteRecurrence::None => "none",
        ScheduledNoteRecurrence::Daily => "daily",
        ScheduledNoteRecurrence::Weekly => "weekly",
        ScheduledNoteRecurrence::Monthly => "monthly",
        ScheduledNoteRecurrence::Yearly => "yearly",
    }
}

fn recurrence_from_name(
    value: &str,
) -> Result<ScheduledNoteRecurrence, CompanionScheduledNoteError> {
    match value {
        "none" => Ok(ScheduledNoteRecurrence::None),
        "daily" => Ok(ScheduledNoteRecurrence::Daily),
        "weekly" => Ok(ScheduledNoteRecurrence::Weekly),
        "monthly" => Ok(ScheduledNoteRecurrence::Monthly),
        "yearly" => Ok(ScheduledNoteRecurrence::Yearly),
        _ => Err(CompanionScheduledNoteError::Corrupt),
    }
}

fn from_row(row: &Row<'_>) -> Result<CompanionScheduledNote, CompanionScheduledNoteError> {
    let window = row.get::<_, Option<i64>>(7).map_err(corrupt)?;
    let note = CompanionScheduledNote {
        id: Uuid::parse_str(&row.get::<_, String>(0).map_err(corrupt)?).map_err(corrupt)?,
        character_id: CharacterId::from_str(&row.get::<_, String>(1).map_err(corrupt)?)
            .map_err(corrupt)?,
        label: row.get(2).map_err(corrupt)?,
        content: row.get(3).map_err(corrupt)?,
        available_at: TimestampMillis::new(row.get(4).map_err(corrupt)?),
        expires_at: row
            .get::<_, Option<i64>>(5)
            .map_err(corrupt)?
            .map(TimestampMillis::new),
        recurrence: recurrence_from_name(&row.get::<_, String>(6).map_err(corrupt)?)?,
        recurrence_window_ms: window.map(u64::try_from).transpose().map_err(corrupt)?,
        enabled: match row.get::<_, i64>(8).map_err(corrupt)? {
            0 => false,
            1 => true,
            _ => return Err(CompanionScheduledNoteError::Corrupt),
        },
        created_at: TimestampMillis::new(row.get(9).map_err(corrupt)?),
        updated_at: TimestampMillis::new(row.get(10).map_err(corrupt)?),
    };
    note.validate()
        .map_err(|_| CompanionScheduledNoteError::Corrupt)?;
    Ok(note)
}

fn load_in(
    tx: &Transaction<'_>,
    id: Uuid,
) -> Result<Option<CompanionScheduledNote>, CompanionScheduledNoteError> {
    tx.query_row(
        "SELECT id, character_id, label, content, available_at, expires_at, recurrence,
                recurrence_window_ms, enabled, created_at, updated_at
           FROM companion_scheduled_notes WHERE id = ?1",
        [id.to_string()],
        |row| from_row(row).map_err(|_| rusqlite::Error::InvalidQuery),
    )
    .optional()
    .map_err(corrupt)
}

fn ensure_companion(
    tx: &Transaction<'_>,
    character_id: CharacterId,
) -> Result<(), CompanionScheduledNoteError> {
    let mode = tx
        .query_row(
            "SELECT interaction_mode FROM characters WHERE id = ?1",
            [character_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(failure)?;
    if mode.as_deref() != Some("companion") {
        return Err(CompanionScheduledNoteError::Invalid);
    }
    Ok(())
}

impl CompanionScheduledNoteRepository for Database {
    fn list_scheduled_notes(
        &self,
        character_id: CharacterId,
    ) -> Result<Vec<CompanionScheduledNote>, CompanionScheduledNoteError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        ensure_companion(&tx, character_id)?;
        let notes = {
            let mut statement = tx
                .prepare(
                    "SELECT id, character_id, label, content, available_at, expires_at, recurrence,
                            recurrence_window_ms, enabled, created_at, updated_at
                       FROM companion_scheduled_notes WHERE character_id = ?1
                       ORDER BY available_at ASC, id ASC",
                )
                .map_err(failure)?;
            statement
                .query_map([character_id.to_string()], |row| {
                    from_row(row).map_err(|_| rusqlite::Error::InvalidQuery)
                })
                .map_err(failure)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(corrupt)?
        };
        tx.commit().map_err(failure)?;
        Ok(notes)
    }

    fn upsert_scheduled_note(
        &self,
        note: CompanionScheduledNote,
    ) -> Result<CompanionScheduledNote, CompanionScheduledNoteError> {
        let note = note.normalize()?;
        let window = note
            .recurrence_window_ms
            .map(i64::try_from)
            .transpose()
            .map_err(|_| CompanionScheduledNoteError::Invalid)?;
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        ensure_companion(&tx, note.character_id)?;
        tx.execute(
            "INSERT INTO companion_scheduled_notes (
                id, character_id, label, content, available_at, expires_at, recurrence,
                recurrence_window_ms, enabled, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                character_id = excluded.character_id,
                label = excluded.label,
                content = excluded.content,
                available_at = excluded.available_at,
                expires_at = excluded.expires_at,
                recurrence = excluded.recurrence,
                recurrence_window_ms = excluded.recurrence_window_ms,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at",
            params![
                note.id.to_string(),
                note.character_id.to_string(),
                note.label,
                note.content,
                note.available_at.get(),
                note.expires_at.map(TimestampMillis::get),
                recurrence_name(note.recurrence),
                window,
                i64::from(note.enabled),
                note.created_at.get(),
                note.updated_at.get(),
            ],
        )
        .map_err(failure)?;
        let stored = load_in(&tx, note.id)?.ok_or(CompanionScheduledNoteError::Failure)?;
        tx.commit().map_err(failure)?;
        Ok(stored)
    }

    fn delete_scheduled_note(&self, id: Uuid) -> Result<(), CompanionScheduledNoteError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        if let Some(note) = load_in(&tx, id)? {
            ensure_companion(&tx, note.character_id)?;
            tx.execute(
                "DELETE FROM companion_scheduled_notes WHERE id = ?1",
                [id.to_string()],
            )
            .map_err(failure)?;
        }
        tx.commit().map_err(failure)?;
        Ok(())
    }
}
