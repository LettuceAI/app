//! Clears the conversation-setting overrides that name a lorebook or prompt
//! about to be hard deleted, inside the deleting transaction.

use std::collections::BTreeSet;

use lettuce_conversations::{
    ConversationKind, ConversationRepositoryError, LorebookLaunchSnapshot, PromptLaunchSnapshot,
};
use lettuce_types::{ConversationId, LorebookId, PromptDocumentId, TimestampMillis};
use rusqlite::{Transaction, params};

use crate::conversation::conversation_mutation_kernel as kernel;
use crate::conversation::conversation_vertical_slice as slice;

/// Removes `lorebook_id` from every conversation's own lorebook selection.
/// A selection left empty selects no lorebook, as the deleted book did not
/// apply either. Returns the conversations that changed.
pub(crate) fn clear_lorebook_overrides_in(
    transaction: &Transaction<'_>,
    lorebook_id: LorebookId,
    now: TimestampMillis,
) -> Result<BTreeSet<ConversationId>, ConversationRepositoryError> {
    let rows = transaction
        .prepare(
            "SELECT conversation_id, lorebooks_json FROM conversation_settings WHERE lorebooks_provenance = 'current_override' AND instr(lorebooks_json, ?1) > 0 ORDER BY conversation_id",
        )
        .map_err(slice::db)?
        .query_map([lorebook_id.to_string()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(slice::db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(slice::db)?;
    let mut changed = BTreeSet::new();
    for (conversation, payload) in rows {
        let mut books: Vec<LorebookLaunchSnapshot> = slice::decode(&payload)?;
        let before = books.len();
        books.retain(|book| book.source_id != lorebook_id);
        if books.len() == before {
            continue;
        }
        let (json, provenance) = if books.is_empty() {
            (None, "disabled")
        } else {
            (Some(slice::encode(&books)?), "current_override")
        };
        transaction
            .execute(
                "UPDATE conversation_settings SET lorebooks_json = ?2, lorebooks_provenance = ?3, revision = revision + 1, updated_at = max(updated_at, ?4) WHERE conversation_id = ?1",
                params![conversation, json, provenance, now.get()],
            )
            .map_err(slice::db)?;
        changed.insert(slice::parse_id(conversation)?);
    }
    bump(transaction, &changed, now)?;
    Ok(changed)
}

/// Removes `lorebook_id` from the explicit lorebook selections recorded at
/// launch, which a chat that follows its launch values reads on every turn.
/// Returns the conversations that changed.
pub(crate) fn clear_launch_lorebooks_in(
    transaction: &Transaction<'_>,
    lorebook_id: LorebookId,
    now: TimestampMillis,
) -> Result<BTreeSet<ConversationId>, ConversationRepositoryError> {
    let rows = transaction
        .prepare(
            "SELECT id, kind_json FROM conversations WHERE instr(kind_json, ?1) > 0 ORDER BY id",
        )
        .map_err(slice::db)?
        .query_map([lorebook_id.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(slice::db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(slice::db)?;
    let mut changed = BTreeSet::new();
    for (conversation, payload) in rows {
        let mut kind: ConversationKind = slice::decode(&payload)?;
        if !kind.remove_explicit_lorebook(lorebook_id) {
            continue;
        }
        transaction
            .execute(
                "UPDATE conversations SET kind_json = ?2 WHERE id = ?1",
                params![conversation, slice::encode(&kind)?],
            )
            .map_err(slice::db)?;
        changed.insert(slice::parse_id(conversation)?);
    }
    bump(transaction, &changed, now)?;
    Ok(changed)
}

/// Resets every conversation's own prompt selection that names `prompt_id`
/// to the launch default. Returns the conversations that changed.
pub(crate) fn clear_prompt_overrides_in(
    transaction: &Transaction<'_>,
    prompt_id: PromptDocumentId,
    now: TimestampMillis,
) -> Result<BTreeSet<ConversationId>, ConversationRepositoryError> {
    let mut changed = BTreeSet::new();
    for (json_column, provenance_column) in [
        ("prompt_json", "prompt_provenance"),
        ("roleplay_prompt_json", "roleplay_prompt_provenance"),
    ] {
        let rows = transaction
            .prepare(&format!(
                "SELECT conversation_id, {json_column} FROM conversation_settings WHERE {provenance_column} = 'current_override' AND instr({json_column}, ?1) > 0 ORDER BY conversation_id"
            ))
            .map_err(slice::db)?
            .query_map([prompt_id.to_string()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(slice::db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(slice::db)?;
        for (conversation, payload) in rows {
            let prompt: PromptLaunchSnapshot = slice::decode(&payload)?;
            if prompt.source_id != prompt_id {
                continue;
            }
            transaction
                .execute(
                    &format!(
                        "UPDATE conversation_settings SET {json_column} = NULL, {provenance_column} = 'launch_inherited', revision = revision + 1, updated_at = max(updated_at, ?2) WHERE conversation_id = ?1"
                    ),
                    params![conversation, now.get()],
                )
                .map_err(slice::db)?;
            changed.insert(slice::parse_id(conversation)?);
        }
    }
    bump(transaction, &changed, now)?;
    Ok(changed)
}

fn bump(
    transaction: &Transaction<'_>,
    conversations: &BTreeSet<ConversationId>,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    for conversation in conversations {
        kernel::bump_conversation(transaction, *conversation, now)?;
    }
    Ok(())
}
