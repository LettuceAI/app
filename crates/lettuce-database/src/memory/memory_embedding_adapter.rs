use std::str::FromStr;

use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingProjectionError, EmbeddingVector, MemoryEmbeddingProjection,
    MemoryEmbeddingRepair, MemoryEmbeddingRepository, ProjectionWrite,
};
use lettuce_types::{MemoryId, MemorySpaceId, TimestampMillis};
use rusqlite::{TransactionBehavior, params};

use crate::Database;

fn storage(_: impl std::fmt::Debug) -> EmbeddingProjectionError {
    EmbeddingProjectionError::Repository("sqlite embedding projection operation failed".to_owned())
}

fn dimensions(value: i64) -> Result<EmbeddingDimensions, EmbeddingProjectionError> {
    match value {
        64 => Ok(EmbeddingDimensions::D64),
        128 => Ok(EmbeddingDimensions::D128),
        256 => Ok(EmbeddingDimensions::D256),
        512 => Ok(EmbeddingDimensions::D512),
        768 => Ok(EmbeddingDimensions::D768),
        _ => Err(storage(value)),
    }
}

fn encode(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode(bytes: &[u8]) -> Result<Vec<f32>, EmbeddingProjectionError> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let raw: [u8; 4] = chunk.try_into().map_err(storage)?;
            let value = f32::from_le_bytes(raw);
            value
                .is_finite()
                .then_some(value)
                .ok_or_else(|| storage(value))
        })
        .collect()
}

fn put_current(
    database: &Database,
    projection: &MemoryEmbeddingProjection,
    token_count: Option<u32>,
) -> Result<ProjectionWrite, EmbeddingProjectionError> {
    projection.validate()?;
    let mut connection = database.connection().map_err(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let written = transaction
        .execute(
            "INSERT INTO memory_embedding_projections (
                space_id, memory_id, source_revision, dimensions, source_text, status, vector, updated_at
             )
             SELECT ?1, ?2, ?3, ?4, ?5, 'ready', ?6, ?7
              WHERE EXISTS (
                    SELECT 1 FROM memory_items WHERE space_id = ?1 AND id = ?2 AND text = ?5
              )
             ON CONFLICT(space_id, memory_id, source_revision, dimensions) DO UPDATE SET
                source_text = excluded.source_text, status = 'ready',
                vector = excluded.vector, updated_at = excluded.updated_at",
            params![
                projection.space_id.to_string(),
                projection.memory_id.to_string(),
                projection.vector.source_revision,
                i64::try_from(projection.dimensions.get()).map_err(storage)?,
                projection.source_text,
                encode(&projection.vector.values),
                projection.updated_at.get(),
            ],
        )
        .map_err(storage)?;
    if written == 0 {
        return Ok(ProjectionWrite::Superseded);
    }
    if let Some(token_count) = token_count {
        transaction
            .execute(
                "UPDATE memory_items SET token_count = ?4
                  WHERE space_id = ?1 AND id = ?2 AND text = ?3",
                params![
                    projection.space_id.to_string(),
                    projection.memory_id.to_string(),
                    projection.source_text,
                    i64::from(token_count),
                ],
            )
            .map_err(storage)?;
    }
    transaction.commit().map_err(storage)?;
    Ok(ProjectionWrite::Stored)
}

impl MemoryEmbeddingRepository for Database {
    fn list_ready(
        &self,
        space_id: MemorySpaceId,
        source_revision: &str,
        requested_dimensions: EmbeddingDimensions,
    ) -> Result<Vec<MemoryEmbeddingProjection>, EmbeddingProjectionError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(
                "SELECT p.memory_id, p.source_text, p.vector, p.dimensions, p.updated_at
                   FROM memory_embedding_projections p
                   JOIN memory_items i
                     ON i.space_id = p.space_id AND i.id = p.memory_id AND i.text = p.source_text
                  WHERE p.space_id = ?1 AND p.source_revision = ?2
                    AND p.dimensions = ?3 AND p.status = 'ready'
                  ORDER BY i.ordinal",
            )
            .map_err(storage)?;
        let mut rows = statement
            .query(params![
                space_id.to_string(),
                source_revision,
                i64::try_from(requested_dimensions.get()).map_err(storage)?,
            ])
            .map_err(storage)?;
        let mut projections = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            let projection = MemoryEmbeddingProjection {
                space_id,
                memory_id: MemoryId::from_str(&row.get::<_, String>(0).map_err(storage)?)
                    .map_err(storage)?,
                source_text: row.get(1).map_err(storage)?,
                vector: EmbeddingVector {
                    source_revision: source_revision.to_owned(),
                    values: decode(&row.get::<_, Vec<u8>>(2).map_err(storage)?)?,
                },
                dimensions: dimensions(row.get(3).map_err(storage)?)?,
                updated_at: TimestampMillis::new(row.get(4).map_err(storage)?),
            };
            projection.validate()?;
            projections.push(projection);
        }
        Ok(projections)
    }

    fn list_repairs(
        &self,
        space_id: MemorySpaceId,
        source_revision: &str,
        requested_dimensions: EmbeddingDimensions,
    ) -> Result<Vec<MemoryEmbeddingRepair>, EmbeddingProjectionError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(
                "SELECT i.id, i.text, ?3, COALESCE(p.updated_at, i.last_accessed_at)
                   FROM memory_items i
                   LEFT JOIN memory_embedding_projections p
                     ON p.space_id = i.space_id AND p.memory_id = i.id AND p.source_text = i.text
                    AND p.source_revision = ?2 AND p.dimensions = ?3
                  WHERE i.space_id = ?1
                    AND (p.status IS NULL OR p.status = 'repair_needed')
                  ORDER BY i.ordinal",
            )
            .map_err(storage)?;
        let mut rows = statement
            .query(params![
                space_id.to_string(),
                source_revision,
                i64::try_from(requested_dimensions.get()).map_err(storage)?,
            ])
            .map_err(storage)?;
        let mut repairs = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            let repair = MemoryEmbeddingRepair {
                space_id,
                memory_id: MemoryId::from_str(&row.get::<_, String>(0).map_err(storage)?)
                    .map_err(storage)?,
                source_text: row.get(1).map_err(storage)?,
                source_revision: source_revision.to_owned(),
                dimensions: dimensions(row.get(2).map_err(storage)?)?,
                updated_at: TimestampMillis::new(row.get(3).map_err(storage)?),
            };
            repair.validate()?;
            repairs.push(repair);
        }
        Ok(repairs)
    }

    fn put_ready(
        &self,
        projection: MemoryEmbeddingProjection,
    ) -> Result<ProjectionWrite, EmbeddingProjectionError> {
        put_current(self, &projection, None)
    }

    fn put_reembedded(
        &self,
        projection: MemoryEmbeddingProjection,
        token_count: u32,
    ) -> Result<ProjectionWrite, EmbeddingProjectionError> {
        put_current(self, &projection, Some(token_count))
    }

    fn mark_repair_needed(
        &self,
        repair: MemoryEmbeddingRepair,
    ) -> Result<(), EmbeddingProjectionError> {
        repair.validate()?;
        let connection = self.connection().map_err(storage)?;
        connection.execute(
            "INSERT INTO memory_embedding_projections (
                space_id, memory_id, source_revision, dimensions, source_text, status, vector, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'repair_needed', NULL, ?6)
             ON CONFLICT(space_id, memory_id, source_revision, dimensions) DO UPDATE SET
                source_text = excluded.source_text, status = 'repair_needed',
                vector = NULL, updated_at = excluded.updated_at",
            params![
                repair.space_id.to_string(), repair.memory_id.to_string(), repair.source_revision,
                i64::try_from(repair.dimensions.get()).map_err(storage)?, repair.source_text,
                repair.updated_at.get(),
            ],
        ).map_err(storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lettuce_embeddings::{
        EmbeddingDimensions, EmbeddingVector, MemoryEmbeddingProjection, MemoryEmbeddingRepository,
    };
    use lettuce_memory::{
        MemoryCategory, MemoryChangeSet, MemoryItem, MemoryRepository, MemorySpaceSnapshot, Score,
    };
    use lettuce_types::{MemoryId, MemorySpaceId, Revision, TimestampMillis};

    use crate::Database;

    fn item(id: MemoryId, text: &str) -> MemoryItem {
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 2,
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

    #[test]
    fn ready_projection_survives_item_set_replacement_without_blob_rewrite() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let memory_id = MemoryId::new();
        let memory = item(memory_id, "stable memory");
        database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![memory.clone()],
            })
            .expect("space");
        assert_eq!(
            database
                .list_repairs(space_id, "v4", EmbeddingDimensions::D128)
                .expect("implicit repair")
                .len(),
            1
        );
        database
            .put_ready(MemoryEmbeddingProjection {
                space_id,
                memory_id,
                source_text: memory.text.clone(),
                vector: EmbeddingVector {
                    source_revision: "v4".to_owned(),
                    values: vec![0.0; 128],
                },
                dimensions: EmbeddingDimensions::D128,
                updated_at: TimestampMillis::new(2),
            })
            .expect("projection");
        database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![memory],
            })
            .expect("replace items");

        let loaded = database
            .list_ready(space_id, "v4", EmbeddingDimensions::D128)
            .expect("list");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].memory_id, memory_id);
        assert!(
            database
                .list_repairs(space_id, "v4", EmbeddingDimensions::D128)
                .expect("repairs")
                .is_empty()
        );
    }

    #[test]
    fn a_model_switch_leaves_every_space_needing_vectors_in_the_new_space() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let embedded = MemoryId::new();
        let superseded = MemoryId::new();
        let mut replaced = item(superseded, "old memory");
        replaced.superseded_by = Some(embedded);
        replaced.superseded_at = Some(TimestampMillis::new(1));
        database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![item(embedded, "kept memory"), replaced],
            })
            .expect("space");
        database
            .put_ready(MemoryEmbeddingProjection {
                space_id,
                memory_id: embedded,
                source_text: "kept memory".to_owned(),
                vector: EmbeddingVector {
                    source_revision: "v4".to_owned(),
                    values: vec![0.5; 768],
                },
                dimensions: EmbeddingDimensions::D768,
                updated_at: TimestampMillis::new(2),
            })
            .expect("v4 projection");
        let needing = |revision: &str, dimensions| {
            database
                .list_repairs(space_id, revision, dimensions)
                .expect("repairs")
                .into_iter()
                .map(|repair| repair.memory_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(needing("v4", EmbeddingDimensions::D768), vec![superseded]);
        assert_eq!(
            needing("v5", EmbeddingDimensions::D768),
            vec![embedded, superseded]
        );
        assert_eq!(
            needing("v4", EmbeddingDimensions::D256),
            vec![embedded, superseded]
        );
        assert_eq!(
            database
                .list_ready(space_id, "v4", EmbeddingDimensions::D768)
                .expect("v4 kept")
                .len(),
            1
        );
    }

    fn projection(
        space_id: MemorySpaceId,
        memory_id: MemoryId,
        text: &str,
        value: f32,
    ) -> MemoryEmbeddingProjection {
        MemoryEmbeddingProjection {
            space_id,
            memory_id,
            source_text: text.to_owned(),
            vector: EmbeddingVector {
                source_revision: "v4".to_owned(),
                values: vec![value; 768],
            },
            dimensions: EmbeddingDimensions::D768,
            updated_at: TimestampMillis::new(2),
        }
    }

    fn recounted_space(database: &Database) -> (MemorySpaceId, MemorySpaceSnapshot) {
        let space_id = MemorySpaceId::new();
        let memory_id = MemoryId::new();
        let snapshot = database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![
                    item(memory_id, "a memory"),
                    item(MemoryId::new(), "another"),
                ],
            })
            .expect("space");
        assert_eq!(
            database
                .put_reembedded(projection(space_id, memory_id, "a memory", 0.25), 9)
                .expect("recount"),
            lettuce_embeddings::ProjectionWrite::Stored
        );
        (space_id, snapshot)
    }

    #[test]
    fn a_snapshot_from_before_a_recount_keeps_the_recounted_tokens() {
        let database = Database::open_in_memory().expect("database");
        let (space_id, before) = recounted_space(&database);
        let recounted = database.get(space_id).expect("get").expect("space");
        assert_eq!(recounted.revision, before.revision);
        assert_eq!(recounted.items[0].token_count, 9);
        let mut items = before.items.clone();
        items[1].text = "another, edited".to_owned();
        items[1].token_count = 5;
        let applied = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: before.revision,
                items,
            })
            .expect("older snapshot still applies");
        assert_eq!(applied.items[0].token_count, 9);
        assert_eq!(applied.items[1].token_count, 5);
    }

    #[test]
    fn a_synced_item_with_unchanged_text_keeps_the_recounted_tokens() {
        let database = Database::open_in_memory().expect("database");
        let (space_id, before) = recounted_space(&database);
        let conversation_id = lettuce_types::ConversationId::new();
        {
            let connection = database.connection().expect("connection");
            connection
                .execute_batch("PRAGMA foreign_keys = OFF")
                .expect("fixture mode");
            connection
                .execute(
                    "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                    rusqlite::params![conversation_id.to_string(), space_id.to_string()],
                )
                .expect("binding");
        }
        let owner = format!("conversation:{conversation_id}");
        let put = |item: &lettuce_memory::MemoryItem| {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let placed = crate::sync::memory_sync_adapter::sync_put_memory_item(
                &transaction,
                &format!("{owner}/{}", item.id),
                item,
            )
            .expect("sync put");
            transaction.commit().expect("commit");
            placed
        };
        let mut remote = before.items[0].clone();
        remote.token_count = 2;
        remote.access_count = 4;
        assert!(put(&remote));
        let synced = database.get(space_id).expect("get").expect("space");
        assert_eq!(synced.items[0].token_count, 9);
        assert_eq!(synced.items[0].access_count, 4);
        remote.text = "a memory, edited elsewhere".to_owned();
        remote.token_count = 6;
        assert!(put(&remote));
        let synced = database.get(space_id).expect("get").expect("space");
        let edited = synced
            .items
            .iter()
            .find(|item| item.id == remote.id)
            .expect("edited item");
        assert_eq!(edited.token_count, 6);
    }

    #[test]
    fn stale_projection_is_excluded_after_its_memory_disappears() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let memory_id = MemoryId::new();
        let memory = item(memory_id, "removed memory");
        database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![memory.clone()],
            })
            .expect("space");
        database
            .put_ready(MemoryEmbeddingProjection {
                space_id,
                memory_id,
                source_text: memory.text,
                vector: EmbeddingVector {
                    source_revision: "v4".to_owned(),
                    values: vec![0.0; 128],
                },
                dimensions: EmbeddingDimensions::D128,
                updated_at: TimestampMillis::new(2),
            })
            .expect("projection");
        database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![],
            })
            .expect("delete memory");

        assert!(
            database
                .list_ready(space_id, "v4", EmbeddingDimensions::D128)
                .expect("list")
                .is_empty()
        );
    }
}
