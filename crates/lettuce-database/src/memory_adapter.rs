use std::str::FromStr;

use lettuce_memory::{
    MemoryCategory, MemoryChangeSet, MemoryItem, MemoryRepository, MemoryRepositoryError,
    MemorySpaceSnapshot, Score,
};
use lettuce_types::{ConversationId, MemorySpaceId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

const SELECT_ITEM: &str = "
    SELECT id, text, category, source_message_id, token_count, is_cold, is_pinned,
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
                    space_id, id, ordinal, text, category, source_message_id, token_count, is_cold, is_pinned,
                    importance, persistence_importance, prompt_importance, volatility,
                    access_count, created_at, last_accessed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    space_id.to_string(),
                    item.id.to_string(),
                    i64::try_from(ordinal).map_err(storage)?,
                    item.text,
                    category_name(item.category),
                    item.source_message_id.map(|id| id.to_string()),
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
                token_count: row.get(4).map_err(storage)?,
                is_cold: row.get(5).map_err(storage)?,
                is_pinned: row.get(6).map_err(storage)?,
                importance: parse_score(row.get(7).map_err(storage)?)?,
                persistence_importance: parse_score(row.get(8).map_err(storage)?)?,
                prompt_importance: parse_score(row.get(9).map_err(storage)?)?,
                volatility: parse_score(row.get(10).map_err(storage)?)?,
                access_count: row.get(11).map_err(storage)?,
                created_at: TimestampMillis::new(row.get(12).map_err(storage)?),
                last_accessed_at: TimestampMillis::new(row.get(13).map_err(storage)?),
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

#[cfg(test)]
mod tests {
    use lettuce_memory::{
        MemoryCategory, MemoryChangeSet, MemoryItem, MemoryRepository, MemoryRepositoryError,
        MemorySpaceSnapshot, Score,
    };
    use lettuce_types::{MemoryId, MemorySpaceId, MessageId, Revision, TimestampMillis};

    use super::Database;

    fn item(id: MemoryId, text: &str) -> MemoryItem {
        MemoryItem {
            id,
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
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
}
