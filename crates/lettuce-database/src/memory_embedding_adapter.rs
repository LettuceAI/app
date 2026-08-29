use std::str::FromStr;

use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingProjectionError, EmbeddingVector, MemoryEmbeddingProjection,
    MemoryEmbeddingRepair, MemoryEmbeddingRepository,
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

    fn put_ready(
        &self,
        projection: MemoryEmbeddingProjection,
    ) -> Result<(), EmbeddingProjectionError> {
        projection.validate()?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM memory_items WHERE space_id = ?1 AND id = ?2 AND text = ?3)",
                params![projection.space_id.to_string(), projection.memory_id.to_string(), projection.source_text],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage)?;
        if !exists {
            return Err(storage("projection owner is not live"));
        }
        transaction.execute(
            "INSERT INTO memory_embedding_projections (
                space_id, memory_id, source_revision, dimensions, source_text, status, vector, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'ready', ?6, ?7)
             ON CONFLICT(space_id, memory_id, source_revision, dimensions) DO UPDATE SET
                source_text = excluded.source_text, status = 'ready',
                vector = excluded.vector, updated_at = excluded.updated_at",
            params![
                projection.space_id.to_string(), projection.memory_id.to_string(),
                projection.vector.source_revision,
                i64::try_from(projection.dimensions.get()).map_err(storage)?,
                projection.source_text, encode(&projection.vector.values), projection.updated_at.get(),
            ],
        ).map_err(storage)?;
        transaction.commit().map_err(storage)
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
            text: text.to_owned(),
            category: MemoryCategory::Other,
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
