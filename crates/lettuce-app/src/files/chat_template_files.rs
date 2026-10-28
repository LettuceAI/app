use lettuce_characters::{
    CharacterRepository, ConversationStarter, RepositoryError, Selection, StarterMessage,
    StarterRepository, StarterRole,
};
use lettuce_context::{
    LifecycleStatus as LibraryStatus, LorebookRepository, PromptPurpose, PromptRepository,
};
use lettuce_transfer::ChatTemplateTransferError;
use lettuce_types::{
    CharacterId, ConversationStarterId, LorebookId, PromptDocumentId, Revision, SceneId,
    StarterMessageId, TimestampMillis,
};

#[derive(Debug, thiserror::Error)]
pub enum ChatTemplateFileError {
    #[error(transparent)]
    Format(#[from] ChatTemplateTransferError),
    #[error("character storage failed: {0}")]
    Repository(#[from] RepositoryError),
    #[error("Character not found")]
    CharacterNotFound,
    #[error("Chat template not found")]
    NotFound,
}

/// Chat template files read into a character's conversation starters.
#[derive(Debug)]
pub struct ChatTemplateFileCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> ChatTemplateFileCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R> ChatTemplateFileCoordinator<'_, R>
where
    R: CharacterRepository + StarterRepository + PromptRepository + LorebookRepository + ?Sized,
{
    /// Adds the file's template as the character's last starter. A scene of
    /// another character, a prompt of another purpose and lorebooks that do
    /// not exist here are left off.
    pub fn import(
        &self,
        character_id: CharacterId,
        json: &str,
        now: TimestampMillis,
    ) -> Result<ConversationStarter, ChatTemplateFileError> {
        let (revision, starter) = self.prepare(character_id, json, now)?;
        Ok(self
            .repository
            .add_starter(character_id, revision, starter, now)?)
    }

    pub fn prepare(
        &self,
        character_id: CharacterId,
        json: &str,
        now: TimestampMillis,
    ) -> Result<(Revision, ConversationStarter), ChatTemplateFileError> {
        let imported = lettuce_transfer::parse_chat_template_import(json)?;
        let details = CharacterRepository::get(self.repository, character_id)?
            .ok_or(ChatTemplateFileError::CharacterNotFound)?;
        let scene_id = imported
            .scene_id
            .as_deref()
            .and_then(|id| id.parse::<SceneId>().ok())
            .filter(|id| details.scenes.iter().any(|scene| scene.id == *id));
        let purpose = if crate::launch::policy::is_companion(&details.character.defaults) {
            PromptPurpose::CompanionChat
        } else {
            PromptPurpose::DirectChat
        };
        let prompt_id = match imported
            .prompt_template_id
            .as_deref()
            .and_then(|id| id.parse::<PromptDocumentId>().ok())
        {
            Some(id) => PromptRepository::get(self.repository, id)
                .map_err(|error| {
                    ChatTemplateFileError::Repository(RepositoryError::Storage {
                        reason: error.to_string(),
                    })
                })?
                .filter(|document| {
                    document.purpose == purpose && document.status == LibraryStatus::Active
                })
                .map(|document| document.id),
            None => None,
        };
        let lorebooks = match &imported.lorebook_ids_override {
            None => Selection::Inherit,
            Some(ids) => {
                let mut kept = Vec::new();
                for id in ids.iter().filter_map(|id| id.parse::<LorebookId>().ok()) {
                    let active = LorebookRepository::get(self.repository, id)
                        .map_err(|error| {
                            ChatTemplateFileError::Repository(RepositoryError::Storage {
                                reason: error.to_string(),
                            })
                        })?
                        .is_some_and(|lorebook| lorebook.book.status == LibraryStatus::Active);
                    if active && !kept.contains(&id) {
                        kept.push(id);
                    }
                }
                Selection::Explicit(kept)
            }
        };
        let starter = ConversationStarter {
            id: ConversationStarterId::new(),
            character_id,
            name: imported.name,
            ordinal: u32::try_from(details.starters.len()).unwrap_or(u32::MAX),
            messages: imported
                .messages
                .into_iter()
                .map(|(role, content)| StarterMessage {
                    id: StarterMessageId::new(),
                    role: if role == "user" {
                        StarterRole::User
                    } else {
                        StarterRole::Assistant
                    },
                    content,
                })
                .collect(),
            scene_id,
            prompt_id,
            lorebooks,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        Ok((details.character.revision, starter))
    }
}

#[cfg(test)]
mod tests {
    use lettuce_characters::{
        Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
        CharacterProvenance, CreateCharacterPlan,
    };

    use super::*;

    #[test]
    fn a_chat_template_file_becomes_a_character_starter() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let character_id = CharacterId::new();
        CharacterRepository::create(
            backend.database(),
            CreateCharacterPlan {
                character: Character::new(
                    character_id,
                    CharacterProfile {
                        name: "Ada".into(),
                        nickname: None,
                        description: Some("Keeper".into()),
                        definition: None,
                        design_description: None,
                        scenario: None,
                    },
                    CharacterProvenance::default(),
                    CharacterDefaults::default(),
                    CharacterPresentationV1::default(),
                    None,
                    CharacterMedia::default(),
                    TimestampMillis::new(1),
                )
                .expect("character"),
                scenes: Vec::new(),
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("create character");
        let files = backend.chat_template_files();
        let card = serde_json::json!({
            "schema": {"name": "USC", "version": "1.0"},
            "kind": "chat_template",
            "payload": {
                "id": "t1",
                "name": " Greeting ",
                "messages": [
                    {"id": "m1", "role": "assistant", "content": "Welcome back."},
                    {"id": "m2", "role": "system", "content": "dropped"},
                    {"id": "m3", "role": "user", "content": "Hi"}
                ],
                "sceneId": "00000000-0000-0000-0000-00000000aaaa",
                "systemPromptTemplate": {"kind": "system_prompt_template", "id": "missing"},
                "lorebookIdsOverride": ["00000000-0000-0000-0000-00000000bbbb"],
                "createdAt": 3
            }
        });
        let starter = files
            .import(character_id, &card.to_string(), TimestampMillis::new(10))
            .expect("import");
        assert_eq!(starter.name, "Greeting");
        assert_eq!(starter.messages.len(), 2);
        assert_eq!(starter.scene_id, None);
        assert_eq!(starter.prompt_id, None);
        assert_eq!(starter.lorebooks, Selection::Explicit(Vec::new()));
    }
}
