use std::str::FromStr;

use lettuce_memory::{
    DynamicMemoryApprovalRepository, DynamicMemoryPendingApproval, MemoryCategory, MemoryChangeSet,
    MemoryItem, MemoryRepository, MemoryRepositoryError, MemorySpaceSnapshot, MemorySummary,
    MemorySummaryChange, MemorySummaryCommit, MemorySummaryRepository, Score,
};
use lettuce_types::{ConversationId, MemorySpaceId, MessageId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

const SELECT_ITEM: &str = "
    SELECT id, text, category, source_message_id, source_role, observed_at, observed_time_precision,
           superseded_by, superseded_at, supersedes_json, token_count, is_cold, is_pinned,
           importance, persistence_importance, prompt_importance, volatility,
           access_count, created_at, last_accessed_at
      FROM memory_items
     WHERE space_id = ?1
     ORDER BY ordinal";

fn storage(_: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure("sqlite memory operation failed".to_owned())
}

fn category_name(value: MemoryCategory) -> &'static str {
    match value {
        MemoryCategory::CharacterTrait => "character_trait",
        MemoryCategory::Relationship => "relationship",
        MemoryCategory::PlotEvent => "plot_event",
        MemoryCategory::WorldDetail => "world_detail",
        MemoryCategory::Preference => "preference",
        MemoryCategory::Other => "other",
    }
}

fn parse_category(value: &str) -> Result<MemoryCategory, MemoryRepositoryError> {
    match value {
        "character_trait" => Ok(MemoryCategory::CharacterTrait),
        "relationship" => Ok(MemoryCategory::Relationship),
        "plot_event" => Ok(MemoryCategory::PlotEvent),
        "world_detail" => Ok(MemoryCategory::WorldDetail),
        "preference" => Ok(MemoryCategory::Preference),
        "other" => Ok(MemoryCategory::Other),
        _ => Err(storage(value)),
    }
}

pub(super) fn sql_revision(value: Revision) -> Result<i64, MemoryRepositoryError> {
    i64::try_from(value.get()).map_err(storage)
}

pub(super) fn parse_revision(value: i64) -> Result<Revision, MemoryRepositoryError> {
    u64::try_from(value).map(Revision::new).map_err(storage)
}

fn parse_id<T: FromStr>(value: String) -> Result<T, MemoryRepositoryError> {
    value.parse().map_err(|_| storage(value))
}

fn parse_score(value: i64) -> Result<Score, MemoryRepositoryError> {
    u16::try_from(value)
        .ok()
        .and_then(Score::from_basis_points)
        .ok_or_else(|| storage(value))
}

pub(super) fn insert_items(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    items: &[MemoryItem],
) -> Result<(), MemoryRepositoryError> {
    for (ordinal, item) in items.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO memory_items (
                    space_id, id, ordinal, text, category, source_message_id, source_role, observed_at, observed_time_precision,
                    superseded_by, superseded_at, supersedes_json, token_count, is_cold, is_pinned,
                    importance, persistence_importance, prompt_importance, volatility,
                    access_count, created_at, last_accessed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
                params![
                    space_id.to_string(),
                    item.id.to_string(),
                    i64::try_from(ordinal).map_err(storage)?,
                    item.text,
                    category_name(item.category),
                    item.source_message_id.map(|id| id.to_string()),
                    item.source_role.map(|role| match role {
                        lettuce_conversations::MessageRole::User => "user",
                        lettuce_conversations::MessageRole::Assistant => "assistant",
                        _ => "invalid",
                    }),
                    item.observed_at.map(TimestampMillis::get),
                    item.observed_time_precision,
                    item.superseded_by.map(|id| id.to_string()),
                    item.superseded_at.map(TimestampMillis::get),
                    serde_json::to_string(&item.supersedes).map_err(storage)?,
                    i64::from(item.token_count),
                    item.is_cold,
                    item.is_pinned,
                    i64::from(item.importance.basis_points()),
                    i64::from(item.persistence_importance.basis_points()),
                    i64::from(item.prompt_importance.basis_points()),
                    i64::from(item.volatility.basis_points()),
                    i64::from(item.access_count),
                    item.created_at.get(),
                    item.last_accessed_at.get(),
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
}

pub(crate) fn create_conversation_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<MemorySpaceId, lettuce_conversations::ConversationRepositoryError> {
    let space_id = MemorySpaceId::new();
    transaction
        .execute(
            "INSERT INTO memory_spaces (id, revision) VALUES (?1, 1)",
            [space_id.to_string()],
        )
        .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    transaction
        .execute(
            "INSERT INTO conversation_memory_spaces (conversation_id, space_id)
             VALUES (?1, ?2)",
            params![conversation_id.to_string(), space_id.to_string()],
        )
        .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    Ok(space_id)
}

pub(super) fn get_in(
    transaction: &Transaction<'_>,
    id: MemorySpaceId,
) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
    let revision = transaction
        .query_row(
            "SELECT revision FROM memory_spaces WHERE id = ?1",
            [id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?;
    let Some(revision) = revision else {
        return Ok(None);
    };
    let items = {
        let mut statement = transaction.prepare(SELECT_ITEM).map_err(storage)?;
        let mut rows = statement.query([id.to_string()]).map_err(storage)?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            items.push(MemoryItem {
                id: parse_id(row.get::<_, String>(0).map_err(storage)?)?,
                text: row.get(1).map_err(storage)?,
                category: parse_category(&row.get::<_, String>(2).map_err(storage)?)?,
                source_message_id: row
                    .get::<_, Option<String>>(3)
                    .map_err(storage)?
                    .map(parse_id)
                    .transpose()?,
                source_role: match row.get::<_, Option<String>>(4).map_err(storage)?.as_deref() {
                    Some("user") => Some(lettuce_conversations::MessageRole::User),
                    Some("assistant") => Some(lettuce_conversations::MessageRole::Assistant),
                    None => None,
                    _ => return Err(storage("invalid memory source role")),
                },
                observed_at: row
                    .get::<_, Option<i64>>(5)
                    .map_err(storage)?
                    .map(TimestampMillis::new),
                observed_time_precision: row.get(6).map_err(storage)?,
                superseded_by: row
                    .get::<_, Option<String>>(7)
                    .map_err(storage)?
                    .map(parse_id)
                    .transpose()?,
                superseded_at: row
                    .get::<_, Option<i64>>(8)
                    .map_err(storage)?
                    .map(TimestampMillis::new),
                supersedes: serde_json::from_str(&row.get::<_, String>(9).map_err(storage)?)
                    .map_err(storage)?,
                token_count: row.get(10).map_err(storage)?,
                is_cold: row.get(11).map_err(storage)?,
                is_pinned: row.get(12).map_err(storage)?,
                importance: parse_score(row.get(13).map_err(storage)?)?,
                persistence_importance: parse_score(row.get(14).map_err(storage)?)?,
                prompt_importance: parse_score(row.get(15).map_err(storage)?)?,
                volatility: parse_score(row.get(16).map_err(storage)?)?,
                access_count: row.get(17).map_err(storage)?,
                created_at: TimestampMillis::new(row.get(18).map_err(storage)?),
                last_accessed_at: TimestampMillis::new(row.get(19).map_err(storage)?),
            });
        }
        items
    };
    let snapshot = MemorySpaceSnapshot {
        id,
        revision: parse_revision(revision)?,
        items,
    };
    snapshot.validate()?;
    Ok(Some(snapshot))
}

pub(super) fn compare_and_apply_in(
    transaction: &Transaction<'_>,
    change: &MemoryChangeSet,
) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
    change.validate()?;
    let next_revision = change
        .expected_revision
        .next()
        .map_err(|_| MemoryRepositoryError::Conflict)?;
    let current_revision = transaction
        .query_row(
            "SELECT revision FROM memory_spaces WHERE id = ?1",
            [change.space_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or(MemoryRepositoryError::NotFound)?;
    if parse_revision(current_revision)? != change.expected_revision {
        return Err(MemoryRepositoryError::Conflict);
    }
    transaction
        .execute(
            "DELETE FROM memory_items WHERE space_id = ?1",
            [change.space_id.to_string()],
        )
        .map_err(storage)?;
    insert_items(transaction, change.space_id, &change.items)?;
    let updated = transaction
        .execute(
            "UPDATE memory_spaces SET revision = ?2 WHERE id = ?1 AND revision = ?3",
            params![
                change.space_id.to_string(),
                sql_revision(next_revision)?,
                sql_revision(change.expected_revision)?,
            ],
        )
        .map_err(storage)?;
    if updated != 1 {
        return Err(MemoryRepositoryError::Conflict);
    }
    get_in(transaction, change.space_id)?.ok_or(MemoryRepositoryError::NotFound)
}

fn get_summary_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT text, token_count, window_start, window_end, updated_at
               FROM memory_summaries
              WHERE space_id = ?1",
            [space_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((text, token_count, window_start, window_end, updated_at)) = row else {
        return Ok(None);
    };
    let source_message_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT message_id
                   FROM memory_summary_source_messages
                  WHERE space_id = ?1
                  ORDER BY ordinal",
            )
            .map_err(storage)?;
        let mut rows = statement.query([space_id.to_string()]).map_err(storage)?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            ids.push(parse_id::<MessageId>(
                row.get::<_, String>(0).map_err(storage)?,
            )?);
        }
        ids
    };
    let summary = MemorySummary {
        space_id,
        text,
        token_count: u32::try_from(token_count).map_err(storage)?,
        window_start: u64::try_from(window_start).map_err(storage)?,
        window_end: u64::try_from(window_end).map_err(storage)?,
        source_message_ids,
        updated_at: TimestampMillis::new(updated_at),
    };
    summary.validate()?;
    Ok(Some(summary))
}

pub(super) fn compare_and_apply_summary_in(
    transaction: &Transaction<'_>,
    change: &MemorySummaryChange,
) -> Result<MemorySummaryCommit, MemoryRepositoryError> {
    change.validate()?;
    let space_id = change.summary.space_id;
    let current_revision = transaction
        .query_row(
            "SELECT revision FROM memory_spaces WHERE id = ?1",
            [space_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or(MemoryRepositoryError::NotFound)?;
    if parse_revision(current_revision)? != change.expected_revision {
        return Err(MemoryRepositoryError::Conflict);
    }
    let conversation_id = transaction
        .query_row(
            "SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = ?1",
            [space_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or(MemoryRepositoryError::NotFound)?;
    transaction
        .execute(
            "DELETE FROM memory_summary_source_messages WHERE space_id = ?1",
            [space_id.to_string()],
        )
        .map_err(storage)?;
    transaction
        .execute(
            "INSERT INTO memory_summaries (
                space_id, conversation_id, text, token_count,
                window_start, window_end, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(space_id) DO UPDATE SET
                conversation_id = excluded.conversation_id,
                text = excluded.text,
                token_count = excluded.token_count,
                window_start = excluded.window_start,
                window_end = excluded.window_end,
                updated_at = excluded.updated_at",
            params![
                space_id.to_string(),
                conversation_id,
                change.summary.text,
                i64::from(change.summary.token_count),
                i64::try_from(change.summary.window_start).map_err(storage)?,
                i64::try_from(change.summary.window_end).map_err(storage)?,
                change.summary.updated_at.get(),
            ],
        )
        .map_err(storage)?;
    for (ordinal, message_id) in change.summary.source_message_ids.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO memory_summary_source_messages (
                    space_id, conversation_id, message_id, ordinal
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    space_id.to_string(),
                    conversation_id,
                    message_id.to_string(),
                    i64::try_from(ordinal).map_err(storage)?,
                ],
            )
            .map_err(storage)?;
    }
    let next_revision = change
        .expected_revision
        .next()
        .map_err(|_| MemoryRepositoryError::Conflict)?;
    let updated = transaction
        .execute(
            "UPDATE memory_spaces SET revision = ?2 WHERE id = ?1 AND revision = ?3",
            params![
                space_id.to_string(),
                sql_revision(next_revision)?,
                sql_revision(change.expected_revision)?,
            ],
        )
        .map_err(storage)?;
    if updated != 1 {
        return Err(MemoryRepositoryError::Conflict);
    }
    let memory = get_in(transaction, space_id)?.ok_or(MemoryRepositoryError::NotFound)?;
    let summary =
        get_summary_in(transaction, space_id)?.ok_or_else(|| storage("missing summary"))?;
    Ok(MemorySummaryCommit { memory, summary })
}

impl MemoryRepository for Database {
    fn create(
        &self,
        snapshot: MemorySpaceSnapshot,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
        snapshot.validate()?;
        if snapshot.revision != Revision::INITIAL {
            return Err(MemoryRepositoryError::Invalid(
                lettuce_memory::MemoryValidationError::InvalidInitialRevision,
            ));
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO memory_spaces (id, revision) VALUES (?1, ?2)",
                params![snapshot.id.to_string(), sql_revision(snapshot.revision)?],
            )
            .map_err(storage)?;
        if inserted != 1 {
            return Err(MemoryRepositoryError::AlreadyExists);
        }
        insert_items(&transaction, snapshot.id, &snapshot.items)?;
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }

    fn get(&self, id: MemorySpaceId) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let snapshot = get_in(&transaction, id)?;
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }

    fn get_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let space_id = transaction
            .query_row(
                "SELECT space_id FROM conversation_memory_spaces WHERE conversation_id = ?1",
                [conversation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?
            .map(parse_id)
            .transpose()?;
        let snapshot = space_id
            .map(|space_id| get_in(&transaction, space_id))
            .transpose()?
            .flatten();
        if space_id.is_some() && snapshot.is_none() {
            return Err(storage("conversation memory space is missing"));
        }
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }

    fn compare_and_apply(
        &self,
        change: MemoryChangeSet,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let snapshot = compare_and_apply_in(&transaction, &change)?;
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }
}

impl MemorySummaryRepository for Database {
    fn get_summary(
        &self,
        space_id: MemorySpaceId,
    ) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let summary = get_summary_in(&transaction, space_id)?;
        transaction.commit().map_err(storage)?;
        Ok(summary)
    }

    fn compare_and_apply_summary(
        &self,
        change: MemorySummaryChange,
    ) -> Result<MemorySummaryCommit, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let commit = compare_and_apply_summary_in(&transaction, &change)?;
        transaction.commit().map_err(storage)?;
        Ok(commit)
    }
}

fn get_pending_approval_in(
    connection: &rusqlite::Connection,
    conversation_id: ConversationId,
) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
    connection
        .query_row(
            "SELECT prompted_message_count,pending,skipped,updated_at
               FROM dynamic_memory_pending_approvals WHERE conversation_id=?1",
            [conversation_id.to_string()],
            |row| {
                Ok(DynamicMemoryPendingApproval {
                    conversation_id,
                    prompted_message_count: u64::try_from(row.get::<_, i64>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    pending: row.get(1)?,
                    skipped: row.get(2)?,
                    updated_at: TimestampMillis::new(row.get(3)?),
                })
            },
        )
        .optional()
        .map_err(storage)
}

impl DynamicMemoryApprovalRepository for Database {
    fn get_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        get_pending_approval_in(&connection, conversation_id)
    }

    fn prompt_dynamic_memory_if_due(
        &self,
        conversation_id: ConversationId,
        unsummarized_message_count: u64,
        message_interval: u32,
        at: TimestampMillis,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
        if message_interval == 0 || unsummarized_message_count == 0 {
            return Err(storage("invalid dynamic memory approval input"));
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let existing = get_pending_approval_in(&transaction, conversation_id)?;
        let baseline = existing
            .as_ref()
            .map_or(0, |approval| approval.prompted_message_count);
        if unsummarized_message_count.saturating_sub(baseline) < u64::from(message_interval) {
            transaction.commit().map_err(storage)?;
            return Ok(None);
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_pending_approvals
                    (conversation_id,prompted_message_count,pending,skipped,updated_at)
                 VALUES (?1,?2,1,0,?3)
                 ON CONFLICT(conversation_id) DO UPDATE SET
                    prompted_message_count=excluded.prompted_message_count,
                    pending=1,
                    updated_at=excluded.updated_at",
                params![
                    conversation_id.to_string(),
                    i64::try_from(unsummarized_message_count).map_err(storage)?,
                    at.get(),
                ],
            )
            .map_err(storage)?;
        let approval = get_pending_approval_in(&transaction, conversation_id)?
            .ok_or_else(|| storage("missing dynamic memory approval"))?;
        transaction.commit().map_err(storage)?;
        Ok(Some(approval))
    }

    fn clear_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
    ) -> Result<(), MemoryRepositoryError> {
        self.connection()
            .map_err(storage)?
            .execute(
                "DELETE FROM dynamic_memory_pending_approvals WHERE conversation_id=?1",
                [conversation_id.to_string()],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn skip_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
        at: TimestampMillis,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "UPDATE dynamic_memory_pending_approvals
                    SET pending=0,skipped=1,updated_at=?2
                  WHERE conversation_id=?1 AND pending=1",
                params![conversation_id.to_string(), at.get()],
            )
            .map_err(storage)?;
        get_pending_approval_in(&connection, conversation_id)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_memory::{
        MemoryCategory, MemoryChangeSet, MemoryItem, MemoryRepository, MemoryRepositoryError,
        MemorySpaceSnapshot, MemorySummary, MemorySummaryChange, MemorySummaryRepository, Score,
    };
    use lettuce_types::{MemoryId, MemorySpaceId, MessageId, Revision, TimestampMillis};

    use super::Database;

    fn item(id: MemoryId, text: &str) -> MemoryItem {
        MemoryItem {
            id,
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 3,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    }

    fn snapshot(id: MemorySpaceId, items: Vec<MemoryItem>) -> MemorySpaceSnapshot {
        MemorySpaceSnapshot {
            id,
            revision: Revision::INITIAL,
            items,
        }
    }

    #[test]
    fn creates_reads_and_cas_replaces_a_memory_space() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let mut first = item(MemoryId::new(), "first");
        first.source_message_id = Some(MessageId::new());
        first.source_role = Some(lettuce_conversations::MessageRole::User);
        first.observed_at = Some(TimestampMillis::new(42));
        first.observed_time_precision = Some("turn".to_owned());
        first.superseded_by = Some(MemoryId::new());
        first.superseded_at = Some(TimestampMillis::new(43));
        first.supersedes = vec![MemoryId::new()];
        let created = database
            .create(snapshot(space_id, vec![first.clone()]))
            .expect("create");
        assert_eq!(database.get(space_id).expect("get"), Some(created));

        let second = item(MemoryId::new(), "second");
        let changed = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![first, second],
            })
            .expect("compare and apply");
        assert_eq!(changed.revision, Revision::new(2));
        assert_eq!(changed.items.len(), 2);
        assert_eq!(database.get(space_id).expect("get"), Some(changed));
    }

    #[test]
    fn stale_compare_and_apply_keeps_the_committed_snapshot() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let original = database
            .create(snapshot(space_id, vec![item(MemoryId::new(), "original")]))
            .expect("create");
        let current = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: original.revision,
                items: vec![item(MemoryId::new(), "current")],
            })
            .expect("first change");
        assert_eq!(
            database.compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: original.revision,
                items: vec![item(MemoryId::new(), "stale")],
            }),
            Err(MemoryRepositoryError::Conflict)
        );
        assert_eq!(database.get(space_id).expect("get"), Some(current));
    }

    #[test]
    fn item_collision_rolls_back_new_space_creation() {
        let database = Database::open_in_memory().expect("database");
        let item_id = MemoryId::new();
        database
            .create(snapshot(MemorySpaceId::new(), vec![item(item_id, "owned")]))
            .expect("first space");
        let second_space_id = MemorySpaceId::new();
        assert!(matches!(
            database.create(snapshot(second_space_id, vec![item(item_id, "collision")])),
            Err(MemoryRepositoryError::Failure(_))
        ));
        assert_eq!(database.get(second_space_id).expect("get"), None);
    }

    #[test]
    fn summary_cas_persists_ordered_cursor_and_advances_root_revision() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let created = database
            .create(snapshot(space_id, vec![item(MemoryId::new(), "memory")]))
            .expect("create");
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                rusqlite::params!["conversation", space_id.to_string()],
            )
            .expect("binding");
        let source_message_ids = vec![MessageId::new(), MessageId::new()];
        let summary = MemorySummary {
            space_id,
            text: "Mira learned the route.".to_owned(),
            token_count: 6,
            window_start: 0,
            window_end: 2,
            source_message_ids,
            updated_at: TimestampMillis::new(50),
        };
        let committed = database
            .compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: summary.clone(),
            })
            .expect("summary commit");
        assert_eq!(committed.memory.revision, Revision::new(2));
        assert_eq!(committed.memory.items, created.items);
        assert_eq!(committed.summary, summary);
        assert_eq!(database.get_summary(space_id).expect("get"), Some(summary));
    }

    #[test]
    fn stale_summary_cas_preserves_current_summary() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let created = database.create(snapshot(space_id, vec![])).expect("create");
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                rusqlite::params!["conversation", space_id.to_string()],
            )
            .expect("binding");
        let source_id = MessageId::new();
        let current = MemorySummary {
            space_id,
            text: "Current summary".to_owned(),
            token_count: 2,
            window_start: 0,
            window_end: 1,
            source_message_ids: vec![source_id],
            updated_at: TimestampMillis::new(10),
        };
        database
            .compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: current.clone(),
            })
            .expect("first commit");
        assert_eq!(
            database.compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: MemorySummary {
                    text: "Stale summary".to_owned(),
                    ..current.clone()
                },
            }),
            Err(MemoryRepositoryError::Conflict)
        );
        assert_eq!(database.get_summary(space_id).expect("get"), Some(current));
    }
}
