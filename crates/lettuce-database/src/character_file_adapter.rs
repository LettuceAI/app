use std::str::FromStr;

use lettuce_characters::{CharacterDetails, RepositoryError};
use lettuce_companions::CompanionScheduledNoteError;
use lettuce_context::LorebookRepositoryError;
use lettuce_transfer::{
    CharacterFileImport, CharacterFileReferences, CharacterFileRepository,
    CharacterFileRepositoryError,
};
use lettuce_types::{LorebookId, ModelProfileId, VoiceProfileId};
use rusqlite::{Connection, TransactionBehavior, params};

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
        let details = crate::character_adapter::load_details(&tx, import.character.character.id)
            .map_err(|_| CharacterFileRepositoryError::Storage)?
            .ok_or(CharacterFileRepositoryError::Storage)?;
        tx.commit()
            .map_err(|_| CharacterFileRepositoryError::Storage)?;
        Ok(details)
    }
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
    let mut voices = connection.prepare("SELECT id FROM user_voices")?;
    for id in voices.query_map([], |row| row.get::<_, String>(0))? {
        if let Ok(id) = VoiceProfileId::from_str(&id?) {
            references.voice_ids.insert(id);
        }
    }
    Ok(references)
}
