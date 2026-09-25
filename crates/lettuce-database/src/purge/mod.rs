//! Hard deletion of conversations and characters with every row that only
//! they own. Append-only history is removed through `purge_authorizations`;
//! the assets the deleted rows mentioned are queued for media collection.

mod media_gc;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use lettuce_types::{CharacterId, ConversationId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, types::Value};

use crate::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PurgeError {
    #[error("the conversation or character to delete was not found")]
    NotFound,
    #[error("a generation or memory run is still active")]
    Busy,
    #[error("the deletion would leave a dangling reference")]
    Integrity,
    #[error("purge storage failed")]
    Storage,
}

/// What one purge deleted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeReceipt {
    pub conversations: Vec<ConversationId>,
    pub characters: Vec<CharacterId>,
    /// Assets the deleted rows referenced, now awaiting media collection.
    pub media_candidates: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PurgeKind {
    Conversation,
    Character,
}

impl PurgeKind {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Character => "character",
        }
    }

    pub(crate) fn from_sync_kind(kind: &str) -> Option<Self> {
        match kind {
            lettuce_sync::CONVERSATION_SYNC_KIND => Some(Self::Conversation),
            lettuce_sync::CHARACTER_SYNC_KIND => Some(Self::Character),
            _ => None,
        }
    }
}

fn storage(_: impl std::fmt::Debug) -> PurgeError {
    PurgeError::Storage
}

const TERMINAL_TURN: &str = "('succeeded', 'failed', 'cancelled', 'interrupted')";
const RUNS: &str = "SELECT id FROM dynamic_memory_runs WHERE conversation_id = ?1 OR space_id IN (SELECT value FROM json_each(?2))";

/// Rows deleted by one purge transaction and what they mentioned.
struct Purge<'c> {
    connection: &'c Connection,
    tables: BTreeSet<String>,
    uuids: BTreeSet<String>,
    snapshot_artifacts: BTreeSet<String>,
    replay_artifacts: BTreeSet<String>,
    receipt: PurgeReceipt,
    now: TimestampMillis,
}

impl<'c> Purge<'c> {
    fn new(connection: &'c Connection, now: TimestampMillis) -> Self {
        Self {
            connection,
            tables: BTreeSet::new(),
            uuids: BTreeSet::new(),
            snapshot_artifacts: BTreeSet::new(),
            replay_artifacts: BTreeSet::new(),
            receipt: PurgeReceipt::default(),
            now,
        }
    }

    fn authorize(&self, owner: &str) -> Result<(), PurgeError> {
        self.connection
            .execute(
                "INSERT OR IGNORE INTO purge_authorizations (owner_id) VALUES (?1)",
                [owner],
            )
            .map(|_| ())
            .map_err(storage)
    }

    fn strings(&self, sql: &str, values: &[Value]) -> Result<Vec<String>, PurgeError> {
        self.connection
            .prepare(sql)
            .and_then(|mut statement| {
                statement
                    .query_map(rusqlite::params_from_iter(values), |row| row.get(0))?
                    .collect()
            })
            .map_err(storage)
    }

    fn exists(&self, sql: &str, values: &[Value]) -> Result<bool, PurgeError> {
        self.connection
            .query_row(sql, rusqlite::params_from_iter(values), |row| row.get(0))
            .map_err(storage)
    }

    /// Deletes the rows of `table` that `filter` selects, first recording the
    /// UUIDs their text mentions (asset candidates) and the artifacts their
    /// foreign keys name.
    fn delete(&mut self, table: &str, filter: &str, values: &[Value]) -> Result<usize, PurgeError> {
        self.scan(table, filter, values)?;
        let deleted = self
            .connection
            .execute(
                &format!("DELETE FROM \"{table}\" WHERE {filter}"),
                rusqlite::params_from_iter(values),
            )
            .map_err(storage)?;
        if deleted > 0 {
            self.tables.insert(table.to_owned());
        }
        Ok(deleted)
    }

    fn scan(&mut self, table: &str, filter: &str, values: &[Value]) -> Result<(), PurgeError> {
        let columns = text_columns(self.connection, table)?;
        let artifact_columns: Vec<(String, String)> = self
            .connection
            .prepare(
                "SELECT f.\"from\", f.\"table\" FROM pragma_foreign_key_list(?1) AS f
                 WHERE f.\"table\" IN ('conversation_snapshot_artifacts', 'conversation_replay_artifacts')
                   AND f.\"to\" = 'artifact_id'",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([table], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect()
            })
            .map_err(storage)?;
        if columns.is_empty() {
            return Ok(());
        }
        let skip_bytes = table == "conversation_replay_artifacts";
        let list = columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut statement = self
            .connection
            .prepare(&format!("SELECT {list} FROM \"{table}\" WHERE {filter}"))
            .map_err(storage)?;
        let mut rows = statement
            .query(rusqlite::params_from_iter(values))
            .map_err(storage)?;
        while let Some(row) = rows.next().map_err(storage)? {
            for (index, column) in columns.iter().enumerate() {
                let value = row.get_ref(index).map_err(storage)?;
                let bytes = match value {
                    rusqlite::types::ValueRef::Text(bytes) => bytes,
                    rusqlite::types::ValueRef::Blob(bytes) if !skip_bytes => bytes,
                    _ => continue,
                };
                if let Some((_, parent)) = artifact_columns.iter().find(|(name, _)| name == column)
                {
                    let id = String::from_utf8_lossy(bytes).into_owned();
                    if parent == "conversation_snapshot_artifacts" {
                        self.snapshot_artifacts.insert(id);
                    } else {
                        self.replay_artifacts.insert(id);
                    }
                }
                collect_uuids(bytes, &mut self.uuids);
            }
        }
        Ok(())
    }

    fn conversation_busy(&self, id: &str) -> Result<bool, PurgeError> {
        self.exists(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND status NOT IN {TERMINAL_TURN})
                 OR EXISTS(SELECT 1 FROM dynamic_memory_run_attempts attempt
                           JOIN dynamic_memory_runs run ON run.id = attempt.run_id
                           WHERE run.conversation_id = ?1 AND attempt.status IN ('created', 'processing'))
                 OR EXISTS(SELECT 1 FROM companion_turn_effects WHERE conversation_id = ?1 AND status = 'processing')"
            ),
            &[Value::Text(id.to_owned())],
        )
    }

    fn conversation_entry(&mut self, id: &str) -> Result<(), PurgeError> {
        let text = Value::Text(id.to_owned());
        if !self.exists(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            std::slice::from_ref(&text),
        )? {
            return Err(PurgeError::NotFound);
        }
        if self.conversation_busy(id)? {
            return Err(PurgeError::Busy);
        }
        self.conversation(id)
    }

    /// Deletes one conversation (direct or group), its own memory space and
    /// everything recorded for it; a companion pool it shares stays, minus
    /// the rows this conversation produced in it.
    fn conversation(&mut self, id: &str) -> Result<(), PurgeError> {
        let text = Value::Text(id.to_owned());
        let own_spaces = self.strings(
            "SELECT link.space_id FROM conversation_memory_spaces link
             WHERE link.conversation_id = ?1 AND link.pooled = 0
               AND NOT EXISTS (SELECT 1 FROM companion_memory_pools pool WHERE pool.space_id = link.space_id)
               AND NOT EXISTS (SELECT 1 FROM conversation_memory_spaces other
                               WHERE other.space_id = link.space_id AND other.conversation_id <> ?1)",
            std::slice::from_ref(&text),
        )?;
        self.authorize(id)?;
        for space in &own_spaces {
            self.authorize(space)?;
        }
        let one = std::slice::from_ref(&text);
        let effects =
            "effect_id IN (SELECT id FROM companion_turn_effects WHERE conversation_id = ?1)";
        self.delete("companion_turn_effect_memory_changes", effects, one)?;
        self.delete("companion_turn_effect_source_messages", effects, one)?;
        for table in [
            "companion_turn_effect_invalidations",
            "companion_turn_effects",
            "companion_turn_effect_emotion_deltas",
            "companion_turn_effect_signal_changes",
            "companion_turn_effect_drafts",
            "companion_state_apply_receipts",
            "companion_emotion_vectors",
            "companion_state_signals",
            "companion_session_states",
        ] {
            self.delete(table, "conversation_id = ?1", one)?;
        }
        self.connection
            .execute(
                "UPDATE companion_continuity_episodes
                 SET previous_conversation_id = (
                     SELECT previous_conversation_id FROM companion_continuity_episodes
                     WHERE conversation_id = ?1)
                 WHERE previous_conversation_id = ?1",
                [id],
            )
            .map_err(storage)?;
        self.delete("companion_continuity_episodes", "conversation_id = ?1", one)?;
        self.delete("companion_consolidation_runs", "conversation_id = ?1", one)?;
        self.delete("creation_lorebook_entry_runs", "conversation_id = ?1", one)?;
        self.memory(Some(id), &own_spaces)?;
        for table in [
            "generation_initial_replay_refs",
            "generation_initial_dispatches",
            "generation_speaker_dispatches",
            "generation_checkpoints",
            "tool_executions",
            "turn_lorebooks",
            "revision_media_refs",
            "candidate_media_refs",
            "conversation_usage_refs",
            "conversation_message_candidates",
            "conversation_message_revisions",
            "conversation_initial_message_origins",
            "conversation_turns",
            "generation_attempts",
            "sync_conversation_forks",
            "conversation_messages",
            "conversation_branches",
            "conversation_snapshot_refs",
            "conversation_outbox",
            "conversation_operations",
            "conversation_settings",
            "conversation_participants",
        ] {
            self.delete(table, "conversation_id = ?1", one)?;
        }
        self.delete("conversations", "id = ?1", one)?;
        self.delete(
            "purge_queue",
            "entity_kind = 'conversation' AND entity_id = ?1",
            one,
        )?;
        self.receipt
            .conversations
            .push(id.parse().map_err(|_| PurgeError::Storage)?);
        Ok(())
    }

    /// Deletes the memory rows a conversation produced (its runs, accesses,
    /// rewinds, summary and links) and every row of `spaces`, which are
    /// deleted outright.
    fn memory(&mut self, conversation: Option<&str>, spaces: &[String]) -> Result<(), PurgeError> {
        let values = [
            conversation.map_or(Value::Null, |id| Value::Text(id.to_owned())),
            Value::Text(serde_json::to_string(spaces).map_err(storage)?),
        ];
        let in_spaces = "space_id IN (SELECT value FROM json_each(?2))";
        let by_owner = format!("(conversation_id = ?1 OR {in_spaces})");
        let by_run = format!("run_id IN ({RUNS})");
        self.delete(
            "companion_growth_runs",
            &format!(
                "(conversation_id = ?1 OR memory_run_id IN ({RUNS})
                  OR memory_attempt_id IN (SELECT id FROM dynamic_memory_run_attempts WHERE run_id IN ({RUNS})))"
            ),
            &values,
        )?;
        for table in [
            "dynamic_memory_background_tool_results",
            "dynamic_memory_background_round_settlements",
            "dynamic_memory_admitted_tool_calls",
            "dynamic_memory_inference_rounds",
            "dynamic_memory_summary_checkpoints",
            "dynamic_memory_run_source_messages",
            "dynamic_memory_run_attempts",
        ] {
            self.delete(table, &by_run, &values)?;
        }
        self.delete(
            "companion_turn_effect_invalidations",
            &format!(
                "(conversation_id = ?1 OR operation_id IN (
                    SELECT operation_id FROM dynamic_memory_suffix_rewinds
                    WHERE conversation_id = ?1 OR {in_spaces}))"
            ),
            &values,
        )?;
        self.delete(
            "dynamic_memory_suffix_rewinds",
            &format!(
                "({by_owner} OR invalid_run_id IN ({RUNS}) OR restored_summary_run_id IN ({RUNS}))"
            ),
            &values,
        )?;
        for table in [
            "dynamic_memory_runs",
            "memory_retrieval_accesses",
            "memory_summary_source_messages",
            "memory_summaries",
        ] {
            self.delete(table, &by_owner, &values)?;
        }
        for table in ["memory_embedding_projections", "memory_items"] {
            self.delete(table, &format!("({in_spaces})"), &values)?;
        }
        for table in ["dynamic_memory_pending_approvals", "memory_synced_cursors"] {
            self.delete(table, "conversation_id = ?1", &values[..1])?;
        }
        self.delete("conversation_memory_spaces", &by_owner, &values)?;
        self.delete("companion_memory_pools", &format!("({in_spaces})"), &values)?;
        self.delete(
            "memory_spaces",
            "id IN (SELECT value FROM json_each(?2))",
            &values,
        )?;
        Ok(())
    }

    fn character_entry(&mut self, id: &str) -> Result<(), PurgeError> {
        let text = Value::Text(id.to_owned());
        let one = std::slice::from_ref(&text);
        if !self.exists("SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)", one)? {
            return Err(PurgeError::NotFound);
        }
        for conversation in direct_conversations(self.connection, id)? {
            if self.conversation_busy(&conversation)? {
                return Err(PurgeError::Busy);
            }
        }
        if self.exists(
            "SELECT EXISTS(SELECT 1 FROM dynamic_memory_run_attempts attempt
                           JOIN dynamic_memory_runs run ON run.id = attempt.run_id
                           JOIN companion_memory_pools pool ON pool.space_id = run.space_id
                           WHERE pool.character_id = ?1 AND attempt.status IN ('created', 'processing'))",
            one,
        )? {
            return Err(PurgeError::Busy);
        }
        self.character(id)
    }

    /// Deletes a character with its direct conversations, its companion
    /// pool and companion state, and its catalog rows (scenes, starters,
    /// media links, lorebook bindings), and takes it out of every group.
    /// Group conversations it took part in stay readable with it as a past
    /// participant.
    fn character(&mut self, id: &str) -> Result<(), PurgeError> {
        for conversation in direct_conversations(self.connection, id)? {
            self.conversation(&conversation)?;
        }
        self.leave_groups(id)?;
        let text = Value::Text(id.to_owned());
        let one = std::slice::from_ref(&text);
        let pools = self.strings(
            "SELECT space_id FROM companion_memory_pools WHERE character_id = ?1",
            one,
        )?;
        self.authorize(id)?;
        for pool in &pools {
            self.authorize(pool)?;
        }
        self.memory(None, &pools)?;
        let sessions = "conversation_id IN (SELECT conversation_id FROM companion_session_states WHERE character_id = ?1)";
        self.delete("companion_state_apply_receipts", "character_id = ?1", one)?;
        self.delete("companion_emotion_vectors", sessions, one)?;
        self.delete("companion_state_signals", sessions, one)?;
        for table in [
            "companion_session_states",
            "companion_continuity_episodes",
            "companion_relationship_states",
            "companion_soul_apply_receipts",
            "companion_soul_fact_supersedes",
            "companion_soul_fact_sources",
            "companion_soul_facts",
            "companion_soul_states",
            "companion_growth_runs",
            "companion_consolidation_runs",
            "companion_scheduled_notes",
            "creation_lorebook_entry_runs",
            "starter_messages",
            "conversation_starters",
            "scene_assets",
            "scene_variants",
            "scenes",
            "character_presentation_asset_refs",
            "character_media",
            "character_lorebook_bindings",
        ] {
            self.delete(table, "character_id = ?1", one)?;
        }
        self.delete("characters", "id = ?1", one)?;
        self.delete(
            "purge_queue",
            "entity_kind = 'character' AND entity_id = ?1",
            one,
        )?;
        self.receipt
            .characters
            .push(id.parse().map_err(|_| PurgeError::Storage)?);
        Ok(())
    }

    /// Removes the character from each group that lists it, as legacy reads
    /// skipped a deleted character left in a group's list: the remaining
    /// members keep their order and mute state, and the group's revision
    /// moves while its update time stays, so two devices removing the same
    /// character reach the same group content. A group left with fewer than
    /// two members keeps all its settings and gets a notice; its chats still
    /// run with the members it has.
    fn leave_groups(&mut self, character: &str) -> Result<(), PurgeError> {
        let groups = self.strings(
            "SELECT group_id FROM group_members WHERE character_id = ?1 ORDER BY group_id",
            &[Value::Text(character.to_owned())],
        )?;
        for group in groups {
            let values = [
                Value::Text(group.clone()),
                Value::Text(character.to_owned()),
            ];
            self.delete(
                "group_members",
                "group_id = ?1 AND character_id = ?2",
                &values,
            )?;
            let remaining = self.strings(
                "SELECT character_id FROM group_members WHERE group_id = ?1 ORDER BY ordinal",
                &values[..1],
            )?;
            self.connection
                .execute(
                    "UPDATE group_members SET ordinal = ordinal + 1000000 WHERE group_id = ?1",
                    [&group],
                )
                .map_err(storage)?;
            for (ordinal, member) in remaining.iter().enumerate() {
                self.connection
                    .execute(
                        "UPDATE group_members SET ordinal = ?3 WHERE group_id = ?1 AND character_id = ?2",
                        params![group, member, i64::try_from(ordinal).map_err(storage)?],
                    )
                    .map_err(storage)?;
            }
            self.connection
                .execute(
                    "UPDATE groups SET revision = revision + 1 WHERE id = ?1",
                    [&group],
                )
                .map_err(storage)?;
            if remaining.len() < 2 {
                record_notice(
                    self.connection,
                    PurgeNoticeEntity::Group,
                    &group,
                    PurgeNoticeReason::GroupBelowTwoMembers,
                    self.now,
                )?;
            }
        }
        Ok(())
    }

    /// Deletes the launch snapshots and provider replays the deleted rows
    /// named that nothing references any more, queues the assets the deleted
    /// rows mentioned, verifies every foreign key into the touched tables and
    /// withdraws the authorizations.
    fn finish(&mut self, now: TimestampMillis) -> Result<(), PurgeError> {
        for (table, ids) in [
            (
                "conversation_snapshot_artifacts",
                std::mem::take(&mut self.snapshot_artifacts),
            ),
            (
                "conversation_replay_artifacts",
                std::mem::take(&mut self.replay_artifacts),
            ),
        ] {
            let references = referencing_columns(self.connection, table, "artifact_id")?;
            let mut filter = String::from("artifact_id = ?1");
            for (child, column) in &references {
                filter.push_str(&format!(
                    " AND NOT EXISTS (SELECT 1 FROM \"{child}\" WHERE \"{column}\" = ?1)"
                ));
            }
            for id in ids {
                self.delete(table, &filter, &[Value::Text(id)])?;
            }
        }
        let uuids = serde_json::to_string(&self.uuids).map_err(storage)?;
        self.receipt.media_candidates = self
            .connection
            .execute(
                "INSERT OR IGNORE INTO media_gc_candidates (asset_id, queued_at)
                 SELECT id, ?2 FROM media_assets
                 WHERE id IN (SELECT value FROM json_each(?1)) AND retention <> 'library'",
                params![uuids, now.get()],
            )
            .map_err(storage)?;
        for conversation in &self.receipt.conversations {
            self.connection
                .execute(
                    "DELETE FROM sync_conversation_marks WHERE conversation_id = ?1",
                    [conversation.to_string()],
                )
                .map_err(storage)?;
        }
        self.connection
            .execute("DELETE FROM purge_authorizations", [])
            .map_err(storage)?;
        self.check_foreign_keys()
    }

    fn check_foreign_keys(&self) -> Result<(), PurgeError> {
        let mut tables = self.tables.clone();
        for parent in &self.tables {
            for (child, _) in referencing_columns(self.connection, parent, "")? {
                tables.insert(child);
            }
        }
        for table in tables {
            let dangling: Option<String> = self
                .connection
                .query_row(
                    &format!("PRAGMA foreign_key_check(\"{table}\")"),
                    [],
                    |row| row.get(2),
                )
                .optional()
                .map_err(storage)?;
            if let Some(parent) = dangling {
                tracing::error!(table, parent, "a purge would leave a dangling reference");
                return Err(PurgeError::Integrity);
            }
        }
        Ok(())
    }
}

/// The TEXT, BLOB and untyped columns of `table`.
fn text_columns(connection: &Connection, table: &str) -> Result<Vec<String>, PurgeError> {
    connection
        .prepare(
            "SELECT name FROM pragma_table_info(?1)
             WHERE upper(type) IN ('TEXT', 'BLOB', 'ANY', '') ORDER BY cid",
        )
        .and_then(|mut statement| statement.query_map([table], |row| row.get(0))?.collect())
        .map_err(storage)
}

/// Every `(table, column)` whose foreign key points at `parent` (at its
/// `to` column when one is given).
fn referencing_columns(
    connection: &Connection,
    parent: &str,
    to: &str,
) -> Result<Vec<(String, String)>, PurgeError> {
    connection
        .prepare(
            "SELECT m.name, f.\"from\" FROM sqlite_schema AS m
             JOIN pragma_foreign_key_list(m.name) AS f
             WHERE m.type = 'table' AND f.\"table\" = ?1
               AND (?2 = '' OR f.\"to\" = ?2 OR f.\"to\" IS NULL)
             ORDER BY m.name, f.\"from\"",
        )
        .and_then(|mut statement| {
            statement
                .query_map([parent, to], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(storage)
}

/// Every hyphenated UUID in `bytes`, lowercased.
fn collect_uuids(bytes: &[u8], out: &mut BTreeSet<String>) {
    const LENGTH: usize = 36;
    let mut start = 0;
    while start + LENGTH <= bytes.len() {
        let candidate = &bytes[start..start + LENGTH];
        let is_uuid = candidate.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        });
        if is_uuid {
            out.insert(String::from_utf8_lossy(candidate).to_ascii_lowercase());
            start += LENGTH;
        } else {
            start += 1;
        }
    }
}

fn direct_conversations(
    connection: &Connection,
    character: &str,
) -> Result<Vec<String>, PurgeError> {
    connection
        .prepare(
            "SELECT DISTINCT conversation.id FROM conversations conversation
             JOIN conversation_participants participant ON participant.conversation_id = conversation.id
             WHERE conversation.kind = 'direct' AND participant.source_kind = 'character'
               AND participant.source_id = ?1
             ORDER BY conversation.id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([character], |row| row.get(0))?
                .collect()
        })
        .map_err(storage)
}

/// Runs `work` in one immediate transaction with foreign key enforcement
/// off (restrict cycles between branches, messages and turns cannot be
/// deleted row by row otherwise); `finish` then checks every foreign key into
/// the touched tables before the commit, and enforcement is restored
/// whatever happened.
fn purge_on(
    connection: &mut Connection,
    foreign_keys_lost: &AtomicBool,
    now: TimestampMillis,
    work: impl FnOnce(&mut Purge<'_>) -> Result<(), PurgeError>,
) -> Result<PurgeReceipt, PurgeError> {
    connection
        .pragma_update(None, "foreign_keys", false)
        .map_err(storage)?;
    let result = purge_transaction(connection, now, work);
    if !restore_foreign_keys(connection) {
        foreign_keys_lost.store(true, Ordering::SeqCst);
        tracing::error!("foreign key enforcement could not be restored after a purge");
        return Err(PurgeError::Storage);
    }
    result
}

fn restore_foreign_keys(connection: &Connection) -> bool {
    (0..3).any(|_| {
        connection.pragma_update(None, "foreign_keys", true).is_ok()
            && connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .is_ok_and(|enabled| enabled == 1)
    })
}

fn purge_transaction(
    connection: &mut Connection,
    now: TimestampMillis,
    work: impl FnOnce(&mut Purge<'_>) -> Result<(), PurgeError>,
) -> Result<PurgeReceipt, PurgeError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let receipt = {
        let mut purge = Purge::new(&transaction, now);
        work(&mut purge)?;
        purge.finish(now)?;
        purge.receipt
    };
    transaction.commit().map_err(storage)?;
    Ok(receipt)
}

pub(crate) fn queue_purge(
    connection: &Connection,
    kind: PurgeKind,
    id: &str,
    change: &lettuce_sync::CanonicalChange,
    now: TimestampMillis,
) -> rusqlite::Result<()> {
    connection
        .execute(
            "INSERT INTO purge_queue (entity_kind, entity_id, change_id, queued_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (entity_kind, entity_id) DO UPDATE SET change_id = excluded.change_id",
            params![
                kind.name(),
                id,
                change.id().as_uuid().to_string(),
                now.get()
            ],
        )
        .map(|_| ())
}

/// Whether a received delete of this synced entity still waits to run, or
/// the entity was kept and waits to be sent back (the scan also holds back
/// everything such an entity owns).
pub(crate) fn purge_queued(
    connection: &Connection,
    sync_kind: &str,
    id: &str,
) -> rusqlite::Result<bool> {
    let Some(kind) = PurgeKind::from_sync_kind(sync_kind) else {
        return Ok(false);
    };
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM purge_queue WHERE entity_kind = ?1 AND entity_id = ?2)
             OR EXISTS(SELECT 1 FROM purge_rejournals WHERE entity_kind = ?1 AND entity_id = ?2)",
        params![kind.name(), id],
        |row| row.get(0),
    )
}

/// Runs the purges received through sync. One that finds its entity gone is
/// dropped; one that is busy or fails stays queued for the next run.
/// How many times a received delete is tried before it is given up.
const MAX_QUEUED_PURGE_FAILURES: i64 = 8;

pub(crate) fn run_queued_purges_on(
    connection: &mut Connection,
    foreign_keys_lost: &AtomicBool,
    now: TimestampMillis,
) -> Result<Vec<PurgeReceipt>, PurgeError> {
    let queued: Vec<(String, String, String)> = connection
        .prepare(
            "SELECT entity_kind, entity_id, change_id FROM purge_queue
             ORDER BY queued_at, entity_kind, entity_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect()
        })
        .map_err(storage)?;
    let mut receipts = Vec::new();
    for (kind, id, change_id) in queued {
        let purge_kind = if kind == PurgeKind::Character.name() {
            PurgeKind::Character
        } else {
            PurgeKind::Conversation
        };
        if keep_on_recheck(connection, purge_kind, &id, &change_id, now)? {
            continue;
        }
        let result = if kind == PurgeKind::Character.name() {
            purge_on(connection, foreign_keys_lost, now, |purge| {
                purge.character_entry(&id)
            })
        } else {
            purge_on(connection, foreign_keys_lost, now, |purge| {
                purge.conversation_entry(&id)
            })
        };
        match result {
            Ok(receipt) => receipts.push(receipt),
            Err(PurgeError::NotFound) => {
                connection
                    .execute(
                        "DELETE FROM purge_queue WHERE entity_kind = ?1 AND entity_id = ?2",
                        params![kind, id],
                    )
                    .map_err(storage)?;
            }
            Err(_) if foreign_keys_lost.load(Ordering::SeqCst) => {
                return Err(PurgeError::Storage);
            }
            Err(PurgeError::Busy) => {}
            Err(error) => {
                tracing::warn!(%error, kind, "a received delete could not run yet");
                give_up_after_failures(connection, &kind, &id, now)?;
            }
        }
    }
    Ok(receipts)
}

/// Decides a queued delete again against what the device holds now (the
/// user may have chatted while it waited); a kept entity leaves the queue.
fn keep_on_recheck(
    connection: &mut Connection,
    kind: PurgeKind,
    id: &str,
    change_id: &str,
    now: TimestampMillis,
) -> Result<bool, PurgeError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let exists: bool = transaction
        .query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {} WHERE id = ?1)",
                match kind {
                    PurgeKind::Conversation => "conversations",
                    PurgeKind::Character => "characters",
                }
            ),
            [id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let kept = exists
        && crate::sync::sync_adapter::keep_queued_delete(&transaction, kind, change_id, now)?;
    if kept {
        transaction
            .execute(
                "DELETE FROM purge_queue WHERE entity_kind = ?1 AND entity_id = ?2",
                params![kind.name(), id],
            )
            .map_err(storage)?;
    }
    transaction.commit().map_err(storage)?;
    Ok(kept)
}

/// Counts one more failure of a queued delete; at the limit the delete is
/// dropped with a notice, and the next scan journals the entity again so
/// every device keeps it.
fn give_up_after_failures(
    connection: &Connection,
    kind: &str,
    id: &str,
    now: TimestampMillis,
) -> Result<(), PurgeError> {
    let failures: i64 = connection
        .query_row(
            "UPDATE purge_queue SET failures = failures + 1
             WHERE entity_kind = ?1 AND entity_id = ?2 RETURNING failures",
            params![kind, id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if failures < MAX_QUEUED_PURGE_FAILURES {
        return Ok(());
    }
    connection
        .execute(
            "DELETE FROM purge_queue WHERE entity_kind = ?1 AND entity_id = ?2",
            params![kind, id],
        )
        .map_err(storage)?;
    let entity = if kind == PurgeKind::Character.name() {
        PurgeNoticeEntity::Character
    } else {
        PurgeNoticeEntity::Conversation
    };
    record_notice(
        connection,
        entity,
        id,
        PurgeNoticeReason::DroppedAfterFailures,
        now,
    )
}

/// What a notice is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeNoticeEntity {
    Conversation,
    Character,
    Group,
    /// Another database file in the app's database directory, by file name.
    DatabaseFile,
    /// A media asset, by id.
    MediaAsset,
    /// A synced entity, as `<kind>/<id>`.
    SyncEntity,
}

impl PurgeNoticeEntity {
    const ALL: [Self; 6] = [
        Self::Conversation,
        Self::Character,
        Self::Group,
        Self::DatabaseFile,
        Self::MediaAsset,
        Self::SyncEntity,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Character => "character",
            Self::Group => "group",
            Self::DatabaseFile => "database_file",
            Self::MediaAsset => "media_asset",
            Self::SyncEntity => "sync_entity",
        }
    }
}

/// Why a delete needs the user's attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeNoticeReason {
    /// Another device deleted it, but this device had changes that device
    /// had not seen, so it was kept here and is sent back.
    KeptUnsentLocalChanges,
    /// A kept entity cannot be sent back whole yet: a snapshot or a
    /// referenced media blob is missing here, or content cannot be encoded.
    /// It is sent once complete.
    RejournalIncomplete,
    /// A kept entity was sent back without this asset or entity, which was
    /// still unavailable after repeated attempts.
    RejournalDropped,
    /// A delete received from another device kept failing here and was
    /// given up; the entity is kept and sent back.
    DroppedAfterFailures,
    /// Deleting a character left a group with fewer than two members; the
    /// group keeps its settings and needs members before it can start a chat.
    GroupBelowTwoMembers,
    /// Media collection was skipped because another database file could not
    /// be read; nothing is deleted until it can be.
    MediaCollectionSkipped,
    /// A synced entity cannot be encoded for sync here (for example it
    /// exceeds the largest change a session carries), so other devices do not
    /// receive its current state and changes received for it wait. It syncs
    /// again once it can be encoded.
    NotSynced,
    /// A sync conflict still waited for the user's choice when the sync
    /// journal started over; both sides are kept in
    /// `sync_carried_conflicts` and the entity keeps the side that was
    /// current.
    ConflictCarried,
}

impl PurgeNoticeReason {
    const ALL: [Self; 8] = [
        Self::KeptUnsentLocalChanges,
        Self::RejournalIncomplete,
        Self::RejournalDropped,
        Self::DroppedAfterFailures,
        Self::GroupBelowTwoMembers,
        Self::MediaCollectionSkipped,
        Self::NotSynced,
        Self::ConflictCarried,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::KeptUnsentLocalChanges => "kept_unsent_local_changes",
            Self::RejournalIncomplete => "rejournal_incomplete",
            Self::RejournalDropped => "rejournal_dropped",
            Self::MediaCollectionSkipped => "media_collection_skipped",
            Self::DroppedAfterFailures => "dropped_after_failures",
            Self::GroupBelowTwoMembers => "group_below_two_members",
            Self::NotSynced => "not_synced",
            Self::ConflictCarried => "conflict_carried",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeNotice {
    pub id: i64,
    pub entity: PurgeNoticeEntity,
    pub entity_id: String,
    pub reason: PurgeNoticeReason,
    pub recorded_at: TimestampMillis,
}

pub(crate) fn record_notice(
    connection: &Connection,
    entity: PurgeNoticeEntity,
    id: &str,
    reason: PurgeNoticeReason,
    now: TimestampMillis,
) -> Result<(), PurgeError> {
    connection
        .execute(
            "INSERT INTO purge_notices (entity_kind, entity_id, reason, recorded_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![entity.name(), id, reason.name(), now.get()],
        )
        .map(|_| ())
        .map_err(storage)
}

impl Database {
    /// How many concurrent-change conflicts of any entity kind wait for a
    /// resolution; the evidence of settled deletes and re-sent entities is
    /// visible here.
    pub fn unresolved_sync_conflict_count(&self) -> Result<u64, PurgeError> {
        let count: i64 = self
            .connection()
            .map_err(storage)?
            .query_row(
                "SELECT count(*) FROM sync_conflicts WHERE status = 'unresolved'",
                [],
                |row| row.get(0),
            )
            .map_err(storage)?;
        u64::try_from(count).map_err(storage)
    }

    /// The notices the user has not dismissed yet, oldest first.
    pub fn purge_notices(&self) -> Result<Vec<PurgeNotice>, PurgeError> {
        let connection = self.connection().map_err(storage)?;
        let rows: Vec<(i64, String, String, String, i64)> = connection
            .prepare(
                "SELECT id, entity_kind, entity_id, reason, recorded_at FROM purge_notices
                 WHERE dismissed_at IS NULL ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    })?
                    .collect()
            })
            .map_err(storage)?;
        rows.into_iter()
            .map(|(id, entity, entity_id, reason, recorded_at)| {
                Ok(PurgeNotice {
                    id,
                    entity: PurgeNoticeEntity::ALL
                        .into_iter()
                        .find(|value| value.name() == entity)
                        .ok_or(PurgeError::Storage)?,
                    entity_id,
                    reason: PurgeNoticeReason::ALL
                        .into_iter()
                        .find(|value| value.name() == reason)
                        .ok_or(PurgeError::Storage)?,
                    recorded_at: TimestampMillis::new(recorded_at),
                })
            })
            .collect()
    }

    /// Records that media collection was skipped because the named database
    /// file could not be read, unless such a notice is still open.
    pub fn record_media_collection_skipped(
        &self,
        file_name: &str,
        now: TimestampMillis,
    ) -> Result<(), PurgeError> {
        let connection = self.connection().map_err(storage)?;
        let open: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM purge_notices WHERE entity_kind = 'database_file'
                   AND entity_id = ?1 AND reason = 'media_collection_skipped' AND dismissed_at IS NULL)",
                [file_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if open {
            return Ok(());
        }
        record_notice(
            &connection,
            PurgeNoticeEntity::DatabaseFile,
            file_name,
            PurgeNoticeReason::MediaCollectionSkipped,
            now,
        )
    }

    /// Marks a notice as seen; `false` when there was no such open notice.
    pub fn dismiss_purge_notice(&self, id: i64, now: TimestampMillis) -> Result<bool, PurgeError> {
        self.connection()
            .map_err(storage)?
            .execute(
                "UPDATE purge_notices SET dismissed_at = ?2 WHERE id = ?1 AND dismissed_at IS NULL",
                params![id, now.get()],
            )
            .map(|updated| updated == 1)
            .map_err(storage)
    }
}

/// Records when the user deleted a synced entity, so its sync delete carries
/// that time.
fn record_deleted_at(
    connection: &Connection,
    kind: &str,
    id: &str,
    now: TimestampMillis,
) -> Result<(), PurgeError> {
    connection
        .execute(
            "INSERT INTO sync_deleted_entities (entity_kind, entity_id, deleted_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entity_kind, entity_id) DO UPDATE SET deleted_at = excluded.deleted_at",
            params![kind, id, now.get()],
        )
        .map(|_| ())
        .map_err(storage)
}

impl Database {
    /// Deletes a conversation (direct or group) and every row it owns.
    /// Usage events stay. Refused while a generation or memory run is active.
    pub fn purge_conversation(
        &self,
        id: ConversationId,
        now: TimestampMillis,
    ) -> Result<PurgeReceipt, PurgeError> {
        let mut connection = self.connection().map_err(storage)?;
        let receipt = purge_on(&mut connection, &self.foreign_keys_lost, now, |purge| {
            purge.conversation_entry(&id.to_string())
        })?;
        record_deleted_at(&connection, "conversation", &id.to_string(), now)?;
        Ok(receipt)
    }

    /// Deletes a character, its direct conversations and its companion
    /// memory and state, and takes it out of every group. Refused while one
    /// of its conversations is busy.
    pub fn purge_character(
        &self,
        id: CharacterId,
        now: TimestampMillis,
    ) -> Result<PurgeReceipt, PurgeError> {
        let mut connection = self.connection().map_err(storage)?;
        let receipt = purge_on(&mut connection, &self.foreign_keys_lost, now, |purge| {
            purge.character_entry(&id.to_string())
        })?;
        record_deleted_at(&connection, "character", &id.to_string(), now)?;
        Ok(receipt)
    }

    /// Runs the deletes received through sync that have not run yet.
    pub fn run_queued_purges(&self, now: TimestampMillis) -> Result<Vec<PurgeReceipt>, PurgeError> {
        let mut connection = self.connection().map_err(storage)?;
        run_queued_purges_on(&mut connection, &self.foreign_keys_lost, now)
    }
}

#[cfg(test)]
pub(crate) mod tests;
