use lettuce_companions::{CompanionRuntimeState, CompanionStateOwner};
use lettuce_transfer::{
    ChatImportRepositoryError, LegacyCompanionConversation,
    LegacyCompanionEpisodeRecord, LegacyConversationRecord,
};
use lettuce_types::{ContentHash, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, params};


fn storage<E>(_: E) -> ChatImportRepositoryError {
    ChatImportRepositoryError::Storage {
        reason: "storage: stored data failed a consistency check",
    }
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn import_chat(
        &self,
        group: Option<lettuce_characters::CreateGroupPlan>,
        record: LegacyConversationRecord,
        companion: Option<(CompanionStateOwner, CompanionRuntimeState)>,
        now: TimestampMillis,
    ) -> Result<(), ChatImportRepositoryError> {
        if let Some(group) = group {
            self.save_group(group, None, now).map_err(|error| {
                tracing::warn!(%error, "import chat failed");
                ChatImportRepositoryError::InvalidInput {
                    reason: "import chat: a value could not be read or converted",
                }
            })?;
        }
        import_chat_in(self.transaction, record, companion, now)
    }
}

pub(crate) fn import_chat_in(
    tx: &Transaction<'_>,
    record: LegacyConversationRecord,
    companion: Option<(CompanionStateOwner, CompanionRuntimeState)>,
    now: TimestampMillis,
) -> Result<(), ChatImportRepositoryError> {
    let companion = companion
        .map(|(owner, initial)| {
            Ok::<_, ChatImportRepositoryError>(LegacyCompanionConversation {
                episode: next_episode(tx, owner, now)?,
                owner,
                initial,
            })
        })
        .transpose()?;
    let conversation_id = record.history.aggregate.conversation.id;
    let operation = lettuce_conversations::OperationToken {
        key: lettuce_conversations::IdempotencyKey::new(format!("chat-import.{conversation_id}"))
            .map_err(|error| {
            tracing::warn!(%error, "import chat in failed");
            ChatImportRepositoryError::InvalidInput {
                reason: "import chat in: a value could not be read or converted",
            }
        })?,
        request_digest: ContentHash::parse(
            blake3::hash(conversation_id.to_string().as_bytes())
                .to_hex()
                .to_string(),
        )
        .map_err(|error| {
            tracing::warn!(%error, "import chat in failed");
            ChatImportRepositoryError::InvalidInput {
                reason: "import chat in: a value could not be read or converted",
            }
        })?,
    };
    crate::conversation::conversation_history_writer::insert_historical_conversation(
        tx,
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
            ChatImportRepositoryError::InvalidInput {
                reason: "import chat in: a value failed validation",
            }
        }
        _ => ChatImportRepositoryError::Storage {
            reason: "import chat in: a stored value is not recognised",
        },
    })?;
    Ok(())
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
        episode_index: u32::try_from(next_index).map_err(|error| {
            tracing::warn!(%error, "next episode failed");
            ChatImportRepositoryError::InvalidInput {
                reason: "next episode: a value could not be read or converted",
            }
        })?,
        previous_conversation_id: previous.map(|id| id.parse()).transpose().map_err(|error| {
            tracing::warn!(%error, "next episode failed");
            ChatImportRepositoryError::Storage {
                reason: "next episode: a storage step failed",
            }
        })?,
        started_at: now,
        ended_at: None,
        updated_at: now,
    })
}
