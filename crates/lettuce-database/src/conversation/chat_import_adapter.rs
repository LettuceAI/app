use lettuce_companions::{CompanionRuntimeState, CompanionStateOwner};
use lettuce_transfer::{
    ChatImportRepository, ChatImportRepositoryError, LegacyCompanionConversation,
    LegacyCompanionEpisodeRecord, LegacyConversationRecord,
};
use lettuce_types::{ContentHash, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

fn storage<E>(_: E) -> ChatImportRepositoryError {
    ChatImportRepositoryError::Storage
}

impl ChatImportRepository for Database {
    fn import_chat(
        &self,
        record: LegacyConversationRecord,
        companion: Option<(CompanionStateOwner, CompanionRuntimeState)>,
        now: TimestampMillis,
    ) -> Result<(), ChatImportRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let companion = companion
            .map(|(owner, initial)| {
                Ok::<_, ChatImportRepositoryError>(LegacyCompanionConversation {
                    episode: next_episode(&tx, owner, now)?,
                    owner,
                    initial,
                })
            })
            .transpose()?;
        let conversation_id = record.history.aggregate.conversation.id;
        let operation = lettuce_conversations::OperationToken {
            key: lettuce_conversations::IdempotencyKey::new(format!(
                "chat-import.{conversation_id}"
            ))
            .map_err(|_| ChatImportRepositoryError::InvalidInput)?,
            request_digest: ContentHash::parse(
                blake3::hash(conversation_id.to_string().as_bytes())
                    .to_hex()
                    .to_string(),
            )
            .map_err(|_| ChatImportRepositoryError::InvalidInput)?,
        };
        crate::conversation::conversation_history_writer::insert_historical_conversation(
            &tx,
            crate::conversation::conversation_history_writer::HistoricalConversation {
                history: &record.history,
                turns: &record.turns,
                usage: &record.usage,
                snapshots: record.snapshots,
                creation:
                    crate::conversation::conversation_history_writer::HistoricalCreation::Generated(
                        operation,
                    ),
                memory: record.memory.as_ref(),
                pool: record.pool.as_ref(),
                memory_projections: &record.memory_projections,
                runtime: &[],
                companion: companion.as_ref(),
            },
        )
        .map_err(|error| match error {
            lettuce_conversations::ConversationRepositoryError::Conflict => {
                ChatImportRepositoryError::Conflict
            }
            lettuce_conversations::ConversationRepositoryError::Invalid(_)
            | lettuce_conversations::ConversationRepositoryError::ArtifactReference(_) => {
                ChatImportRepositoryError::InvalidInput
            }
            _ => ChatImportRepositoryError::Storage,
        })?;
        tx.commit().map_err(storage)?;
        Ok(())
    }
}

/// The episode after the companion's latest one for this persona, ending
/// that one at `now`.
fn next_episode(
    tx: &Transaction<'_>,
    owner: CompanionStateOwner,
    now: TimestampMillis,
) -> Result<LegacyCompanionEpisodeRecord, ChatImportRepositoryError> {
    let persona_key = owner
        .persona_id
        .map_or_else(|| "__default__".to_owned(), |id| id.to_string());
    let previous = tx
        .query_row(
            "SELECT conversation_id FROM companion_continuity_episodes
             WHERE character_id = ?1 AND persona_key = ?2
             ORDER BY started_at DESC, episode_index DESC LIMIT 1",
            params![owner.character_id.to_string(), persona_key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    let next_index: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(episode_index), 0) + 1 FROM companion_continuity_episodes
             WHERE character_id = ?1 AND persona_key = ?2",
            params![owner.character_id.to_string(), persona_key],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if let Some(previous) = &previous {
        tx.execute(
            "UPDATE companion_continuity_episodes
             SET ended_at = ?1, updated_at = max(updated_at, ?1)
             WHERE conversation_id = ?2 AND ended_at IS NULL",
            params![now.get(), previous],
        )
        .map_err(storage)?;
    }
    Ok(LegacyCompanionEpisodeRecord {
        episode_index: u32::try_from(next_index)
            .map_err(|_| ChatImportRepositoryError::InvalidInput)?,
        previous_conversation_id: previous
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| ChatImportRepositoryError::Storage)?,
        started_at: now,
        ended_at: None,
        updated_at: now,
    })
}
