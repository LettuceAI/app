use std::str::FromStr;

use lettuce_characters::{CharacterDetails, RepositoryError};
use lettuce_companions::{CompanionScheduledNoteError, RelationshipState, SoulOwner};
use lettuce_context::LorebookRepositoryError;
use lettuce_transfer::{
    CharacterExportRecord, CharacterFileCompanionMemory, CharacterFileImport,
    CharacterFileReferences, CharacterFileRelationship, CharacterFileRepository,
    CharacterFileRepositoryError,
};
use lettuce_types::{
    CharacterId, LorebookId, ModelProfileId, PersonaId, TimestampMillis, VoiceProfileId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::Database;

impl CharacterFileRepository for Database {
    fn character_file_references(
        &self,
    ) -> Result<CharacterFileReferences, CharacterFileRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        load_references(&connection).map_err(|_| CharacterFileRepositoryError::Storage)
    }

    fn import_character_file(
        &self,
        import: &CharacterFileImport,
    ) -> Result<CharacterDetails, CharacterFileRepositoryError> {
        lettuce_context::validate_bindings(&import.lorebook_bindings)
            .map_err(|_| CharacterFileRepositoryError::InvalidInput)?;
        let mut connection = self
            .connection()
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        for lorebook in &import.lorebooks {
            crate::lorebook_adapter::insert_lorebook_details(&tx, lorebook).map_err(|error| {
                match error {
                    LorebookRepositoryError::Invalid(_) => {
                        CharacterFileRepositoryError::InvalidInput
                    }
                    LorebookRepositoryError::Failure(_) => CharacterFileRepositoryError::Storage,
                    _ => CharacterFileRepositoryError::Conflict,
                }
            })?;
        }
        crate::character_adapter::insert_character_plan(&tx, &import.character).map_err(
            |error| match error {
                RepositoryError::Storage => CharacterFileRepositoryError::Storage,
                RepositoryError::Invalid(_) => CharacterFileRepositoryError::InvalidInput,
                _ => CharacterFileRepositoryError::Conflict,
            },
        )?;
        let character_id = import.character.character.id.to_string();
        for binding in &import.lorebook_bindings {
            let revision = i64::try_from(binding.revision.get())
                .map_err(|_| CharacterFileRepositoryError::InvalidInput)?;
            tx.execute(
                "INSERT INTO character_lorebook_bindings (character_id,lorebook_id,enabled,ordinal,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    character_id,
                    binding.lorebook_id.to_string(),
                    binding.enabled,
                    i64::from(binding.ordinal),
                    revision,
                    binding.created_at.get(),
                    binding.updated_at.get()
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => {
                    CharacterFileRepositoryError::Conflict
                }
                _ => CharacterFileRepositoryError::Storage,
            })?;
        }
        for note in &import.scheduled_notes {
            crate::scheduled_note_adapter::insert_note_in(&tx, note).map_err(
                |error| match error {
                    CompanionScheduledNoteError::Failure | CompanionScheduledNoteError::Corrupt => {
                        CharacterFileRepositoryError::Storage
                    }
                    CompanionScheduledNoteError::Conflict => CharacterFileRepositoryError::Conflict,
                    _ => CharacterFileRepositoryError::InvalidInput,
                },
            )?;
        }
        if let Some(memory) = &import.companion_memory {
            insert_companion_memory(&tx, import.character.character.id, memory)?;
        }
        let details = crate::character_adapter::load_details(&tx, import.character.character.id)
            .map_err(|_| CharacterFileRepositoryError::Storage)?
            .ok_or(CharacterFileRepositoryError::Storage)?;
        tx.commit()
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        Ok(details)
    }

    fn character_export_record(
        &self,
        id: CharacterId,
    ) -> Result<Option<CharacterExportRecord>, CharacterFileRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        let record =
            load_export_record(&tx, id).map_err(|_| CharacterFileRepositoryError::Storage)?;
        tx.commit()
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        Ok(record)
    }
}

fn insert_companion_memory(
    tx: &rusqlite::Transaction<'_>,
    character_id: CharacterId,
    memory: &CharacterFileCompanionMemory,
) -> Result<(), CharacterFileRepositoryError> {
    let storage = |_| CharacterFileRepositoryError::Storage;
    if let Some(pool) = &memory.pool {
        pool.validate()
            .map_err(|_| CharacterFileRepositoryError::InvalidInput)?;
        tx.execute(
            "INSERT INTO memory_spaces (id, revision) VALUES (?1, 1)",
            [pool.id.to_string()],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO companion_memory_pools (character_id, space_id) VALUES (?1, ?2)",
            params![character_id.to_string(), pool.id.to_string()],
        )
        .map_err(storage)?;
        crate::memory_adapter::insert_items(tx, pool.id, &pool.items)
            .map_err(|_| CharacterFileRepositoryError::InvalidInput)?;
    }
    if let Some(facts) = &memory.soul_facts {
        crate::soul_adapter::replace_facts_in(
            tx,
            SoulOwner::Character(character_id),
            facts,
            memory.updated_at,
        )
        .map_err(|_| CharacterFileRepositoryError::InvalidInput)?;
    }
    for relationship in &memory.relationships {
        let state = &relationship.state;
        tx.execute(
            "INSERT INTO companion_relationship_states (character_id, persona_key, persona_id, closeness, trust, affection, tension, stability, interaction_count, last_interaction_at, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12)",
            params![
                character_id.to_string(),
                relationship
                    .persona_id
                    .map_or_else(|| "__default__".to_owned(), |id| id.to_string()),
                relationship.persona_id.map(|id| id.to_string()),
                state.closeness,
                state.trust,
                state.affection,
                state.tension,
                state.stability,
                i64::from(state.interaction_count),
                state.last_interaction_at.get(),
                memory.created_at.get(),
                memory.updated_at.get(),
            ],
        )
        .map_err(|error| match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::ConstraintViolation) => {
                CharacterFileRepositoryError::InvalidInput
            }
            _ => CharacterFileRepositoryError::Storage,
        })?;
    }
    Ok(())
}

fn load_export_record(
    connection: &rusqlite::Transaction<'_>,
    id: CharacterId,
) -> rusqlite::Result<Option<CharacterExportRecord>> {
    let Some(details) = crate::character_adapter::load_details(connection, id)? else {
        return Ok(None);
    };
    let mut bindings = connection.prepare(
        "SELECT lorebook_id FROM character_lorebook_bindings
         WHERE character_id = ?1 AND enabled = 1 ORDER BY ordinal",
    )?;
    let mut lorebooks = Vec::new();
    for lorebook_id in bindings.query_map([id.to_string()], |row| row.get::<_, String>(0))? {
        let Ok(lorebook_id) = LorebookId::from_str(&lorebook_id?) else {
            continue;
        };
        if let Some(lorebook) = crate::lorebook_adapter::load_details(connection, lorebook_id)? {
            lorebooks.push(lorebook);
        }
    }
    let mut notes = connection.prepare(
        "SELECT id, character_id, label, content, available_at, expires_at, recurrence,
                recurrence_window_ms, enabled, created_at, updated_at
           FROM companion_scheduled_notes WHERE character_id = ?1
           ORDER BY available_at ASC, id ASC",
    )?;
    let scheduled_notes = notes
        .query_map([id.to_string()], |row| {
            crate::scheduled_note_adapter::from_row(row).map_err(|_| rusqlite::Error::InvalidQuery)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let companion_memory = load_companion_memory(connection, id)?;
    Ok(Some(CharacterExportRecord {
        details,
        lorebooks,
        scheduled_notes,
        companion_memory,
    }))
}

fn load_companion_memory(
    tx: &rusqlite::Transaction<'_>,
    id: CharacterId,
) -> rusqlite::Result<Option<CharacterFileCompanionMemory>> {
    let pool_id = tx
        .query_row(
            "SELECT space_id FROM companion_memory_pools WHERE character_id = ?1",
            [id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let pool = match pool_id {
        Some(pool_id) => {
            let pool_id = pool_id.parse().map_err(|_| rusqlite::Error::InvalidQuery)?;
            crate::memory_adapter::get_in(tx, pool_id).map_err(invalid)?
        }
        None => None,
    };
    let soul_facts = crate::soul_adapter::get_in(tx, SoulOwner::Character(id))
        .map_err(invalid)?
        .map(|state| state.facts);
    let mut relationships = Vec::new();
    let mut created_at = None::<i64>;
    let mut updated_at = None::<i64>;
    let mut statement = tx.prepare(
        "SELECT persona_id, closeness, trust, affection, tension, stability, interaction_count,
                last_interaction_at, created_at, updated_at
           FROM companion_relationship_states WHERE character_id = ?1 ORDER BY persona_key",
    )?;
    let rows = statement.query_map([id.to_string()], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            RelationshipState {
                closeness: row.get(1)?,
                trust: row.get(2)?,
                affection: row.get(3)?,
                tension: row.get(4)?,
                stability: row.get(5)?,
                interaction_count: row.get(6)?,
                last_interaction_at: TimestampMillis::new(row.get(7)?),
            },
            row.get::<_, i64>(8)?,
            row.get::<_, i64>(9)?,
        ))
    })?;
    for row in rows {
        let (persona_id, state, created, updated) = row?;
        let persona_id = persona_id
            .map(|id| PersonaId::from_str(&id))
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        created_at = Some(created_at.map_or(created, |value| value.min(created)));
        updated_at = Some(updated_at.map_or(updated, |value| value.max(updated)));
        relationships.push(CharacterFileRelationship { persona_id, state });
    }
    if pool.is_none() && soul_facts.is_none() && relationships.is_empty() {
        return Ok(None);
    }
    let created_at = created_at.unwrap_or_else(|| {
        pool.as_ref()
            .and_then(|pool| pool.items.iter().map(|item| item.created_at.get()).min())
            .unwrap_or(0)
    });
    let updated_at = updated_at.unwrap_or(created_at).max(created_at);
    Ok(Some(CharacterFileCompanionMemory {
        pool,
        soul_facts,
        relationships,
        created_at: TimestampMillis::new(created_at),
        updated_at: TimestampMillis::new(updated_at),
    }))
}

fn invalid<E>(_: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn load_references(connection: &Connection) -> rusqlite::Result<CharacterFileReferences> {
    let mut references = CharacterFileReferences::default();
    let mut models = connection.prepare("SELECT id, kind FROM model_profiles")?;
    for row in models.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, kind) = row?;
        let Ok(id) = ModelProfileId::from_str(&id) else {
            continue;
        };
        references.model_ids.insert(id);
        if kind == "chat" {
            references.chat_model_ids.insert(id);
        }
    }
    let mut prompts =
        connection.prepare("SELECT id, purpose FROM prompt_documents WHERE status = 'active'")?;
    for row in prompts.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, purpose) = row?;
        references
            .prompt_purposes
            .insert(id, crate::prompt_adapter::purpose_from_name(&purpose)?);
    }
    let mut lorebooks = connection.prepare("SELECT id FROM lorebooks")?;
    for id in lorebooks.query_map([], |row| row.get::<_, String>(0))? {
        if let Ok(id) = LorebookId::from_str(&id?) {
            references.lorebook_ids.insert(id);
        }
    }
    let mut personas = connection.prepare("SELECT id FROM personas")?;
    for id in personas.query_map([], |row| row.get::<_, String>(0))? {
        if let Ok(id) = PersonaId::from_str(&id?) {
            references.persona_ids.insert(id);
        }
    }
    let mut voices = connection.prepare("SELECT id FROM user_voices")?;
    for id in voices.query_map([], |row| row.get::<_, String>(0))? {
        if let Ok(id) = VoiceProfileId::from_str(&id?) {
            references.voice_ids.insert(id);
        }
    }
    Ok(references)
}

#[cfg(test)]
mod tests {
    use lettuce_transfer::{CharacterFileAssets, CharacterFileRepository, CharacterPackage};
    use serde_json::json;
    use uuid::Uuid;

    use crate::Database;

    #[test]
    fn a_companion_file_writes_its_pool_soul_and_relationships_with_the_character() {
        let database = Database::open_in_memory().expect("database");
        let package: CharacterPackage = serde_json::from_value(json!({
            "version": 1,
            "character": {
                "name": "Ada",
                "description": "Keeper",
                "rules": [],
                "scenes": [],
                "defaultSceneId": null,
                "defaultModelId": null,
                "mode": "companion",
                "companion": {"soul": {"essence": "Warm"}},
                "companionSharedMemory": {
                    "memories": ["Ada loves tea", "Ada fears storms"],
                    "soulGrowth": [{
                        "id": "fact-1", "category": "traits", "value": "Patient",
                        "kind": "add", "policy": "adaptive", "slot": "temperament",
                        "confidence": 0.9, "evidenceCount": 2, "weight": 0.8,
                        "validFrom": 10, "locked": false, "createdAt": 10
                    }],
                    "relationshipStates": {
                        "__default__": {"closeness": 0.6, "trust": 0.5, "affection": 0.3, "tension": 0.1, "stability": 0.6, "interactionCount": 3, "lastInteractionAt": 90}
                    },
                    "createdAt": 20,
                    "updatedAt": 30
                },
                "promptTemplateId": null,
                "systemPrompt": null,
                "voiceConfig": null,
                "voiceAutoplay": null,
                "disableAvatarGradient": false,
                "avatarCrop": null,
                "customGradientEnabled": null,
                "customGradientColors": null,
                "customTextColor": null,
                "customTextSecondary": null
            },
            "avatarData": null,
            "backgroundImageData": null
        }))
        .expect("package");
        let references = database.character_file_references().expect("references");
        let plan = lettuce_transfer::plan_character_file(&package, &references, 50, Uuid::new_v4)
            .expect("plan");
        let import = plan
            .import(
                &references,
                &CharacterFileAssets::default(),
                50,
                Uuid::new_v4,
            )
            .expect("import plan");
        let details = database.import_character_file(&import).expect("import");
        let id = details.character.id.to_string();
        let connection = database.connection().expect("connection");
        let texts = connection
            .prepare(
                "SELECT item.text FROM memory_items item
                 JOIN companion_memory_pools pool ON pool.space_id = item.space_id
                 WHERE pool.character_id = ?1 ORDER BY item.ordinal",
            )
            .expect("pool query")
            .query_map([&id], |row| row.get::<_, String>(0))
            .expect("pool rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("pool items");
        assert_eq!(texts, ["Ada loves tea", "Ada fears storms"]);
        let relationship: (String, f64, i64) = connection
            .query_row(
                "SELECT persona_key, closeness, updated_at FROM companion_relationship_states WHERE character_id = ?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("relationship");
        assert_eq!(relationship, ("__default__".to_owned(), 0.6, 30));
        let facts: i64 = connection
            .query_row(
                "SELECT count(*) FROM companion_soul_facts WHERE character_id = ?1 AND scope = ''",
                [&id],
                |row| row.get(0),
            )
            .expect("soul facts");
        assert_eq!(facts, 1);
        drop(connection);
        let graph = crate::restore_writer::tests::assert_backup_round_trip(&database);
        assert_eq!(graph.memory.unbound_pools.len(), 1);
        assert_eq!(graph.memory.unbound_pools[0].items.len(), 2);
    }
}
