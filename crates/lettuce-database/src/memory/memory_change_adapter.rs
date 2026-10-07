use lettuce_memory::MemoryRepositoryError;
use lettuce_types::ConversationId;
use rusqlite::params;

use crate::Database;

fn storage(_: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure("memory change read failed".into())
}

impl Database {
    /// The newest position of the memory change journal.
    pub fn memory_change_position(&self) -> Result<u64, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let position: Option<i64> = connection
            .query_row("SELECT max(position) FROM memory_changes", [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        u64::try_from(position.unwrap_or(0)).map_err(storage)
    }

    /// The conversations whose memory view changed after `after`, oldest
    /// first, each once.
    pub fn memory_changes_since(
        &self,
        after: u64,
        limit: u32,
    ) -> Result<Vec<(u64, ConversationId)>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let rows = connection
            .prepare("SELECT position, conversation_id FROM memory_changes WHERE position > ?1 ORDER BY position LIMIT ?2")
            .and_then(|mut statement| {
                statement
                    .query_map(
                        params![i64::try_from(after).map_err(|_| rusqlite::Error::InvalidQuery)?, i64::from(limit)],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                    )?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        rows.into_iter()
            .map(|(position, id)| {
                Ok((
                    u64::try_from(position).map_err(storage)?,
                    id.parse().map_err(storage)?,
                ))
            })
            .collect()
    }
}
