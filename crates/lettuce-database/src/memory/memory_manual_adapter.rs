use std::str::FromStr;

use lettuce_memory::{
    MemoryChangeSet, MemoryContextRevision, MemoryManualEdit, MemoryManualHistory,
    MemoryRepositoryError, reduce_manual_memory,
};
use lettuce_types::MessageId;
use rusqlite::{OptionalExtension, params};

fn storage(_: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure("sqlite manual memory edit failed".into())
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn apply_memory_manual_edit(
        &self,
        edit: &MemoryManualEdit,
        projection: Option<&lettuce_embeddings::MemoryEmbeddingProjection>,
    ) -> Result<MemoryManualHistory, MemoryRepositoryError> {
        let tx = self.transaction;
        let head: Option<Option<String>> = tx.query_row(
            "SELECT coalesce(branch.head_message_id, branch.fork_message_id) FROM conversations conversation JOIN conversation_branches branch ON branch.conversation_id = conversation.id AND branch.id = conversation.active_branch_id WHERE conversation.id = ?1 AND branch.id = ?2 AND conversation.revision = ?3 AND conversation.lifecycle <> 'tombstoned' AND branch.status = 'active'",
            params![edit.conversation_id.to_string(), edit.branch_id.to_string(), i64::try_from(edit.conversation_revision.get()).map_err(storage)?],
            |row| row.get(0),
        ).optional().map_err(storage)?;
        let head = head.ok_or(MemoryRepositoryError::Conflict)?;
        if super::memory_adapter::branch_space_id_in(tx, edit.conversation_id, edit.branch_id)
            .map_err(storage)?
            != Some(edit.space_id)
        {
            return Err(MemoryRepositoryError::Conflict);
        }
        for context in &edit.context_revisions {
            let (sql, id, expected) = match context {
                MemoryContextRevision::Character { id, revision } => (
                    "SELECT revision FROM characters WHERE id = ?1",
                    id.to_string(),
                    *revision,
                ),
                MemoryContextRevision::Group { id, revision } => (
                    "SELECT revision FROM groups WHERE id = ?1",
                    id.to_string(),
                    *revision,
                ),
                MemoryContextRevision::Settings { revision } => (
                    "SELECT revision FROM app_settings WHERE id = ?1",
                    "1".into(),
                    *revision,
                ),
            };
            let actual: Option<i64> = tx
                .query_row(sql, [id], |row| row.get(0))
                .optional()
                .map_err(storage)?;
            if actual != Some(i64::try_from(expected.get()).map_err(storage)?) {
                return Err(MemoryRepositoryError::Conflict);
            }
        }
        let current = super::memory_adapter::get_in(tx, edit.space_id)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        if current.revision != edit.expected_revision {
            return Err(MemoryRepositoryError::Conflict);
        }
        let before_summary = super::memory_adapter::get_summary_in(tx, edit.space_id)?;
        let coverage = if matches!(
            edit.mutation,
            lettuce_memory::MemoryManualMutation::Summary { .. }
        ) {
            Some(super::memory_adapter::summary_cursor_in(
                tx,
                edit.space_id,
                edit.conversation_id,
                edit.branch_id,
            )?)
        } else {
            None
        };
        let reduction = reduce_manual_memory(&current, &edit.mutation, edit.at)?;
        if let Some(summary) = reduction
            .summary
            .as_ref()
            .and_then(|summary| summary.as_ref())
        {
            if summary.space_id != edit.space_id || summary.branch_id != edit.branch_id {
                return Err(MemoryRepositoryError::Conflict);
            }
        }
        let updated = super::memory_adapter::compare_and_apply_in(
            tx,
            &MemoryChangeSet {
                space_id: edit.space_id,
                expected_revision: edit.expected_revision,
                items: reduction.items,
            },
        )?;
        if let Some(summary) = &reduction.summary {
            super::memory_adapter::replace_summary_in(tx, edit.space_id, summary.as_ref())?;
        }
        if let Some(coverage) = coverage.filter(|coverage| *coverage > 0) {
            tx.execute(
                "INSERT INTO memory_synced_cursors (conversation_id,branch_id,window_end) VALUES (?1,?2,?3)
                 ON CONFLICT(conversation_id,branch_id) DO UPDATE SET window_end=max(window_end,excluded.window_end)",
                params![
                    edit.conversation_id.to_string(),
                    edit.branch_id.to_string(),
                    i64::try_from(coverage).map_err(storage)?
                ],
            )
            .map_err(storage)?;
        }
        if let Some(projection) = projection {
            if projection.space_id != edit.space_id
                || reduction.after_item.as_ref().is_none_or(|item| {
                    item.id != projection.memory_id || item.text != projection.source_text
                })
            {
                return Err(MemoryRepositoryError::Conflict);
            }
            if super::memory_embedding_adapter::put_current_in(tx, projection, None)
                .map_err(storage)?
                != lettuce_embeddings::ProjectionWrite::Stored
            {
                return Err(MemoryRepositoryError::Conflict);
            }
        }
        let anchor: Option<String> = if let Some(head) = head {
            tx.query_row("WITH RECURSIVE path(id,depth) AS (SELECT ?2,0 UNION ALL SELECT message.parent_message_id,path.depth+1 FROM conversation_messages message JOIN path ON message.id=path.id WHERE message.conversation_id=?1 AND message.parent_message_id IS NOT NULL) SELECT message.id FROM path JOIN conversation_messages message ON message.id=path.id WHERE message.conversation_id=?1 AND message.role IN ('user','assistant') AND message.visibility='visible' ORDER BY path.depth LIMIT 1", params![edit.conversation_id.to_string(), head], |row| row.get(0)).optional().map_err(storage)?
        } else {
            None
        };
        let anchor_message_id = anchor
            .map(|id| MessageId::from_str(&id).map_err(storage))
            .transpose()?;
        let message_position = anchor_message_id
            .map(|id| {
                super::memory_branch_adapter::message_position_in(tx, edit.conversation_id, id)
                    .map_err(storage)
            })
            .transpose()?
            .unwrap_or(0);
        let sequence: i64 = tx
            .query_row(
                "SELECT coalesce(max(sequence),0)+1 FROM memory_manual_edits",
                [],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let result = MemoryManualHistory {
            sequence: u64::try_from(sequence).map_err(storage)?,
            edit: edit.clone(),
            space_id: edit.space_id,
            anchor_message_id,
            message_position,
            before_item: reduction.before_item,
            after_item: reduction.after_item,
            before_summary,
            after_summary: super::memory_adapter::get_summary_in(tx, edit.space_id)?,
            resulting_revision: updated.revision,
        };
        insert_manual_record_in(
            tx,
            &lettuce_memory::MemoryManualEditRecord {
                history: result.clone(),
                undone_at: None,
            },
        )?;
        Ok(result)
    }
}

pub(crate) fn insert_manual_record_in(
    tx: &rusqlite::Transaction<'_>,
    record: &lettuce_memory::MemoryManualEditRecord,
) -> Result<(), MemoryRepositoryError> {
    let history = &record.history;
    history.validate()?;
    let edit = &history.edit;
    tx.execute("INSERT INTO memory_manual_edits (sequence,id,conversation_id,branch_id,space_id,anchor_message_id,message_position,history_json,created_at,undone_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![i64::try_from(history.sequence).map_err(storage)?, edit.id.to_string(), edit.conversation_id.to_string(), edit.branch_id.to_string(), history.space_id.to_string(), history.anchor_message_id.map(|id| id.to_string()), i64::try_from(history.message_position).map_err(storage)?, serde_json::to_string(history).map_err(storage)?, edit.at.get(), record.undone_at.map(|at| at.get())]).map_err(storage)?;
    Ok(())
}

pub(crate) fn history_in(
    tx: &rusqlite::Transaction<'_>,
    conversation_id: lettuce_types::ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
    space_id: lettuce_types::MemorySpaceId,
) -> Result<Vec<MemoryManualHistory>, MemoryRepositoryError> {
    tx.prepare("SELECT history_json FROM memory_manual_edits WHERE conversation_id=?1 AND branch_id=?2 AND space_id=?3 AND undone_at IS NULL ORDER BY sequence DESC")
        .and_then(|mut statement| statement.query_map(params![conversation_id.to_string(),branch_id.to_string(),space_id.to_string()], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()).map_err(storage)?
        .into_iter().map(|json| serde_json::from_str(&json).map_err(storage)).collect()
}

pub(crate) fn retained_history_in(
    tx: &rusqlite::Transaction<'_>,
    space_id: lettuce_types::MemorySpaceId,
    undone: &[MemoryManualHistory],
) -> Result<Vec<MemoryManualHistory>, MemoryRepositoryError> {
    let rows = tx.prepare("SELECT history_json FROM memory_manual_edits WHERE space_id = ?1 AND undone_at IS NULL ORDER BY sequence")
        .and_then(|mut statement| statement.query_map([space_id.to_string()], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>())
        .map_err(storage)?;
    rows.into_iter().map(|json| serde_json::from_str::<MemoryManualHistory>(&json).map_err(storage))
        .filter_map(|result| match result {
            Ok(edit) if undone.iter().any(|undo| undo.edit.id == edit.edit.id) => None,
            result => Some(result),
        }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_memory::{MemoryItem, MemoryManualMutation, MemoryRepository, MemoryShortId};
    use lettuce_types::{MemoryId, OperationId, Revision, TimestampMillis};

    #[derive(Debug)]
    struct Failure;
    impl From<crate::ApiOperationError> for Failure {
        fn from(_: crate::ApiOperationError) -> Self {
            Self
        }
    }
    impl From<MemoryRepositoryError> for Failure {
        fn from(_: MemoryRepositoryError) -> Self {
            Self
        }
    }

    #[test]
    fn manual_edit_receipt_and_history_commit_once_and_replay() {
        let database = crate::Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) =
            super::super::dynamic_memory_run_adapter::tests::conversation_fixture(&database);
        let branch_id = super::super::dynamic_memory_run_adapter::tests::fixture_branch(
            &database,
            conversation_id,
        );
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?2 WHERE id = ?1",
                params![
                    branch_id.to_string(),
                    messages.last().expect("head").message_id.to_string()
                ],
            )
            .expect("branch head");
        let item_id = MemoryId::new();
        let edit = MemoryManualEdit {
            id: OperationId::new(),
            conversation_id,
            branch_id,
            conversation_revision: Revision::INITIAL,
            expected_revision: Revision::INITIAL,
            space_id,
            context_revisions: vec![],
            mutation: MemoryManualMutation::Add {
                item: MemoryItem::written(
                    item_id,
                    MemoryShortId::derived(item_id),
                    "A manual memory".into(),
                    TimestampMillis::new(5),
                ),
            },
            at: TimestampMillis::new(5),
        };
        let apply = || {
            database.commit_api_operation(
                "memory_add",
                "manual-operation",
                "same-request",
                edit.at,
                |scope| {
                    scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                },
            )
        };
        let result = apply().expect("atomic edit");
        let replay = apply().expect("replay");
        assert_eq!(result, replay);
        assert_eq!(result.message_position, 2);
        assert!(result.anchor_message_id.is_some());
        let current = MemoryRepository::get(&database, space_id)
            .expect("space")
            .expect("present");
        assert_eq!(current.revision, Revision::new(2));
        assert_eq!(current.items.len(), 1);
        assert_eq!(current.items[0].token_count, None);
        assert!(
            database
                .commit_api_operation::<MemoryManualHistory, Failure>(
                    "memory_add",
                    "manual-operation",
                    "changed-request",
                    edit.at,
                    |scope| scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                )
                .is_err()
        );
        assert_eq!(
            MemoryRepository::get(&database, space_id).expect("space"),
            Some(current)
        );
    }

    #[test]
    fn failed_history_insert_rolls_back_the_item_and_receipt() {
        let database = crate::Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) =
            super::super::dynamic_memory_run_adapter::tests::conversation_fixture(&database);
        let branch_id = super::super::dynamic_memory_run_adapter::tests::fixture_branch(
            &database,
            conversation_id,
        );
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?2 WHERE id = ?1",
                params![
                    branch_id.to_string(),
                    messages.last().expect("head").message_id.to_string()
                ],
            )
            .expect("branch head");
        let item_id = MemoryId::new();
        let edit = MemoryManualEdit {
            id: OperationId::new(),
            conversation_id,
            branch_id,
            conversation_revision: Revision::INITIAL,
            expected_revision: Revision::INITIAL,
            space_id,
            context_revisions: vec![],
            mutation: MemoryManualMutation::Add {
                item: MemoryItem::written(
                    item_id,
                    MemoryShortId::derived(item_id),
                    "A manual memory".into(),
                    TimestampMillis::new(5),
                ),
            },
            at: TimestampMillis::new(5),
        };
        let before = MemoryRepository::get(&database, space_id).expect("space");
        database.connection().expect("connection").execute_batch("CREATE TRIGGER fail_manual_history BEFORE INSERT ON memory_manual_edits BEGIN SELECT RAISE(ABORT, 'injected history failure'); END;").expect("inject crash");
        assert!(
            database
                .commit_api_operation::<MemoryManualHistory, Failure>(
                    "memory_add",
                    "crashed-operation",
                    "request",
                    edit.at,
                    |scope| scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                )
                .is_err()
        );
        assert_eq!(
            MemoryRepository::get(&database, space_id).expect("space"),
            before
        );
        assert!(
            database
                .lookup_api_operation("memory_add", "crashed-operation")
                .expect("receipt")
                .is_none()
        );
    }
    #[test]
    fn manual_edit_rejects_stale_revision_and_another_conversations_space() {
        let database = crate::Database::open_in_memory().expect("database");
        let (conversation_id, space_id, _) =
            super::super::dynamic_memory_run_adapter::tests::conversation_fixture(&database);
        let branch_id = super::super::dynamic_memory_run_adapter::tests::fixture_branch(
            &database,
            conversation_id,
        );
        let id = MemoryId::new();
        let mut edit = MemoryManualEdit {
            id: OperationId::new(),
            conversation_id,
            branch_id,
            conversation_revision: Revision::INITIAL,
            expected_revision: Revision::new(2),
            space_id,
            context_revisions: vec![],
            mutation: MemoryManualMutation::Add {
                item: MemoryItem::written(
                    id,
                    MemoryShortId::derived(id),
                    "Manual memory".into(),
                    TimestampMillis::new(5),
                ),
            },
            at: TimestampMillis::new(5),
        };
        let before = MemoryRepository::get(&database, space_id).expect("space");
        assert!(
            database
                .commit_api_operation::<MemoryManualHistory, Failure>(
                    "memory_add",
                    "stale",
                    "request",
                    edit.at,
                    |scope| scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                )
                .is_err()
        );
        edit.expected_revision = Revision::INITIAL;
        edit.space_id = lettuce_types::MemorySpaceId::new();
        assert!(
            database
                .commit_api_operation::<MemoryManualHistory, Failure>(
                    "memory_add",
                    "foreign",
                    "request",
                    edit.at,
                    |scope| scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                )
                .is_err()
        );
        assert_eq!(
            MemoryRepository::get(&database, space_id).expect("space"),
            before
        );
        assert!(
            database
                .lookup_api_operation("memory_add", "stale")
                .expect("receipt")
                .is_none()
        );
        assert!(
            database
                .lookup_api_operation("memory_add", "foreign")
                .expect("receipt")
                .is_none()
        );
    }

    #[test]
    fn manual_history_and_receipt_survive_reopening_without_a_fifty_edit_cap() {
        let path =
            std::env::temp_dir().join(format!("manual-memory-{}.sqlite", OperationId::new()));
        let database = crate::Database::open(&path).expect("database");
        let (conversation_id, space_id, _) =
            super::super::dynamic_memory_run_adapter::tests::conversation_fixture(&database);
        let branch_id = super::super::dynamic_memory_run_adapter::tests::fixture_branch(
            &database,
            conversation_id,
        );
        let mut last = None;
        for ordinal in 1..=55 {
            let id = MemoryId::new();
            let edit = MemoryManualEdit {
                id: OperationId::new(),
                conversation_id,
                branch_id,
                conversation_revision: Revision::INITIAL,
                expected_revision: Revision::new(ordinal),
                space_id,
                context_revisions: vec![],
                mutation: MemoryManualMutation::Add {
                    item: MemoryItem::written(
                        id,
                        MemoryShortId::derived(id),
                        format!("Memory {ordinal}"),
                        TimestampMillis::new(5),
                    ),
                },
                at: TimestampMillis::new(5),
            };
            last = Some(
                database
                    .commit_api_operation::<MemoryManualHistory, Failure>(
                        "memory_add",
                        &format!("edit-{ordinal}"),
                        "request",
                        edit.at,
                        |scope| {
                            scope
                                .apply_memory_manual_edit(&edit, None)
                                .map_err(Failure::from)
                        },
                    )
                    .expect("edit"),
            );
        }
        drop(database);
        let database = crate::Database::open(&path).expect("reopen");
        let count: i64 = database
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM memory_manual_edits", [], |row| {
                row.get(0)
            })
            .expect("history");
        assert_eq!(count, 55);
        let replay = database
            .commit_api_operation::<MemoryManualHistory, Failure>(
                "memory_add",
                "edit-55",
                "request",
                TimestampMillis::new(9),
                |_| panic!("replay must bypass mutation"),
            )
            .expect("replay");
        assert_eq!(Some(replay), last);
        drop(database);
        std::fs::remove_file(path).expect("remove database");
    }
}
