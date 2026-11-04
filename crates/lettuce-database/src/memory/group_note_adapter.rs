use std::str::FromStr;

use lettuce_companions::{
    GroupScheduledNote, GroupScheduledNoteError, GroupScheduledNoteRepository,
};
use lettuce_types::{CharacterId, GroupId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

use super::scheduled_note_adapter::{recurrence_from_name, recurrence_name};
use crate::Database;

pub(crate) const NOTE_COLUMNS: &str = "id, group_id, visible_to_json, label, content, available_at, expires_at, recurrence, recurrence_window_ms, enabled, created_at, updated_at";

fn failure(error: impl std::fmt::Debug) -> GroupScheduledNoteError {
    GroupScheduledNoteError::Failure {
        reason: format!("{error:?}"),
    }
}

fn corrupt(error: impl std::fmt::Debug) -> GroupScheduledNoteError {
    GroupScheduledNoteError::Corrupt {
        reason: format!("{error:?}"),
    }
}

pub(crate) fn from_row(row: &Row<'_>) -> Result<GroupScheduledNote, GroupScheduledNoteError> {
    let visible_to: Vec<CharacterId> =
        serde_json::from_str(&row.get::<_, String>(2).map_err(corrupt)?).map_err(corrupt)?;
    let window = row.get::<_, Option<i64>>(8).map_err(corrupt)?;
    let note = GroupScheduledNote {
        id: Uuid::parse_str(&row.get::<_, String>(0).map_err(corrupt)?).map_err(corrupt)?,
        group_id: GroupId::from_str(&row.get::<_, String>(1).map_err(corrupt)?)
            .map_err(corrupt)?,
        visible_to,
        label: row.get(3).map_err(corrupt)?,
        content: row.get(4).map_err(corrupt)?,
        available_at: TimestampMillis::new(row.get(5).map_err(corrupt)?),
        expires_at: row
            .get::<_, Option<i64>>(6)
            .map_err(corrupt)?
            .map(TimestampMillis::new),
        recurrence: recurrence_from_name(&row.get::<_, String>(7).map_err(corrupt)?)
            .map_err(corrupt)?,
        recurrence_window_ms: window.map(u64::try_from).transpose().map_err(corrupt)?,
        enabled: match row.get::<_, i64>(9).map_err(corrupt)? {
            0 => false,
            1 => true,
            _ => {
                return Err(GroupScheduledNoteError::Corrupt {
                    reason: "from row: a stored value is not recognised".to_owned(),
                });
            }
        },
        created_at: TimestampMillis::new(row.get(10).map_err(corrupt)?),
        updated_at: TimestampMillis::new(row.get(11).map_err(corrupt)?),
    };
    note.validate()
        .map_err(|error| GroupScheduledNoteError::Corrupt {
            reason: error.to_string(),
        })?;
    Ok(note)
}

pub(crate) fn load_in(
    connection: &Connection,
    id: Uuid,
) -> Result<Option<GroupScheduledNote>, GroupScheduledNoteError> {
    connection
        .query_row(
            &format!("SELECT {NOTE_COLUMNS} FROM group_scheduled_notes WHERE id = ?1"),
            [id.to_string()],
            |row| from_row(row).map_err(|_| rusqlite::Error::InvalidQuery),
        )
        .optional()
        .map_err(corrupt)
}

pub(crate) fn list_in(
    connection: &Connection,
    group_id: GroupId,
) -> Result<Vec<GroupScheduledNote>, GroupScheduledNoteError> {
    ensure_group(connection, group_id)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {NOTE_COLUMNS} FROM group_scheduled_notes WHERE group_id = ?1
             ORDER BY available_at ASC, id ASC"
        ))
        .map_err(failure)?;
    statement
        .query_map([group_id.to_string()], |row| {
            from_row(row).map_err(|_| rusqlite::Error::InvalidQuery)
        })
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(corrupt)
}

fn ensure_group(
    connection: &Connection,
    group_id: GroupId,
) -> Result<(), GroupScheduledNoteError> {
    connection
        .query_row(
            "SELECT 1 FROM groups WHERE id = ?1",
            [group_id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(failure)?
        .ok_or(GroupScheduledNoteError::GroupNotFound)
}

fn ensure_members(
    connection: &Connection,
    note: &GroupScheduledNote,
) -> Result<(), GroupScheduledNoteError> {
    for character in &note.visible_to {
        connection
            .query_row(
                "SELECT 1 FROM group_members WHERE group_id = ?1 AND character_id = ?2",
                params![note.group_id.to_string(), character.to_string()],
                |_| Ok(()),
            )
            .optional()
            .map_err(failure)?
            .ok_or(GroupScheduledNoteError::NotAMember)?;
    }
    Ok(())
}

fn note_params(
    note: &GroupScheduledNote,
) -> Result<
    (
        String,
        String,
        String,
        Option<i64>,
    ),
    GroupScheduledNoteError,
> {
    let window = note
        .recurrence_window_ms
        .map(i64::try_from)
        .transpose()
        .map_err(|_| GroupScheduledNoteError::Invalid {
            reason: "a value could not be read or converted",
        })?;
    Ok((
        note.id.to_string(),
        note.group_id.to_string(),
        serde_json::to_string(&note.visible_to).map_err(failure)?,
        window,
    ))
}

fn insert_sql(conflict: &str) -> String {
    format!(
        "INSERT INTO group_scheduled_notes ({NOTE_COLUMNS})
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12){conflict}"
    )
}

fn write(
    connection: &Connection,
    note: &GroupScheduledNote,
    conflict: &str,
) -> Result<(), GroupScheduledNoteError> {
    let (id, group_id, visible_to, window) = note_params(note)?;
    connection
        .execute(
            &insert_sql(conflict),
            params![
                id,
                group_id,
                visible_to,
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
    Ok(())
}

/// Inserts or updates a note on the caller's transaction after checking the
/// group and the members the note is visible to.
pub(crate) fn upsert_in(
    tx: &Transaction<'_>,
    note: GroupScheduledNote,
) -> Result<GroupScheduledNote, GroupScheduledNoteError> {
    let note = note.normalize()?;
    ensure_group(tx, note.group_id)?;
    ensure_members(tx, &note)?;
    write(
        tx,
        &note,
        " ON CONFLICT(id) DO UPDATE SET
            group_id = excluded.group_id,
            visible_to_json = excluded.visible_to_json,
            label = excluded.label,
            content = excluded.content,
            available_at = excluded.available_at,
            expires_at = excluded.expires_at,
            recurrence = excluded.recurrence,
            recurrence_window_ms = excluded.recurrence_window_ms,
            enabled = excluded.enabled,
            updated_at = excluded.updated_at",
    )?;
    load_in(tx, note.id)?.ok_or_else(|| GroupScheduledNoteError::Failure {
        reason: "upsert in: a required value is missing".to_owned(),
    })
}

/// Writes a backed-up or synced note exactly, without checking the group or
/// its members.
pub(crate) fn insert_restored_note_in(
    connection: &Connection,
    note: &GroupScheduledNote,
) -> Result<(), GroupScheduledNoteError> {
    write(connection, note, "")
}

impl GroupScheduledNoteRepository for Database {
    fn list_group_notes(
        &self,
        group_id: GroupId,
    ) -> Result<Vec<GroupScheduledNote>, GroupScheduledNoteError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let notes = list_in(&tx, group_id)?;
        tx.commit().map_err(failure)?;
        Ok(notes)
    }

    fn upsert_group_note(
        &self,
        note: GroupScheduledNote,
    ) -> Result<GroupScheduledNote, GroupScheduledNoteError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let stored = upsert_in(&tx, note)?;
        tx.commit().map_err(failure)?;
        Ok(stored)
    }

    fn delete_group_note(&self, id: Uuid) -> Result<(), GroupScheduledNoteError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        delete_in(&tx, id)?;
        tx.commit().map_err(failure)?;
        Ok(())
    }
}

pub(crate) fn delete_in(
    connection: &Connection,
    id: Uuid,
) -> Result<(), GroupScheduledNoteError> {
    connection
        .execute(
            "DELETE FROM group_scheduled_notes WHERE id = ?1",
            [id.to_string()],
        )
        .map_err(failure)?;
    Ok(())
}

pub(crate) fn sync_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare("SELECT id FROM group_scheduled_notes ORDER BY id")?
        .query_map([], |row| row.get(0))?
        .collect()
}

pub(crate) fn sync_replace(
    tx: &Transaction<'_>,
    note: &GroupScheduledNote,
) -> Result<(), GroupScheduledNoteError> {
    note.validate()?;
    ensure_group(tx, note.group_id)?;
    delete_in(tx, note.id)?;
    insert_restored_note_in(tx, note)
}
