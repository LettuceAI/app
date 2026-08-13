use lettuce_types::{
    CharacterId, ConversationStarterId, LorebookId, PromptDocumentId, Revision, SceneId,
    StarterMessageId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::constants::{
    MAX_COLLECTION_ITEMS, validate_collection, validate_name, validate_text, validate_unique,
};
use crate::{Selection, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterMessage {
    pub id: StarterMessageId,
    pub role: StarterRole,
    pub content: String,
}

impl StarterMessage {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_text("starter_message.content", &self.content)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationStarter {
    pub id: ConversationStarterId,
    pub character_id: CharacterId,
    pub name: String,
    pub ordinal: u32,
    pub messages: Vec<StarterMessage>,
    pub scene_id: Option<SceneId>,
    pub prompt_id: Option<PromptDocumentId>,
    pub lorebooks: Selection<Vec<LorebookId>>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl ConversationStarter {
    pub fn new(
        id: ConversationStarterId,
        character_id: CharacterId,
        name: String,
        ordinal: u32,
        messages: Vec<StarterMessage>,
        created_at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        let starter = Self {
            id,
            character_id,
            name,
            ordinal,
            messages,
            scene_id: None,
            prompt_id: None,
            lorebooks: Selection::Inherit,
            revision: Revision::INITIAL,
            created_at,
            updated_at: created_at,
        };
        starter.validate()?;
        Ok(starter)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name("starter.name", &self.name)?;
        validate_collection("starter.messages", &self.messages, MAX_COLLECTION_ITEMS)?;
        validate_unique(
            "starter.message_ids",
            self.messages.iter().map(|message| message.id),
        )?;
        for message in &self.messages {
            message.validate()?;
        }
        if let Selection::Explicit(lorebooks) = &self.lorebooks {
            validate_collection("starter.lorebooks", lorebooks, MAX_COLLECTION_ITEMS)?;
            validate_unique("starter.lorebook_ids", lorebooks.iter().copied())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationStarter, StarterMessage, StarterRole};
    use crate::Selection;
    use lettuce_types::{
        CharacterId, ConversationStarterId, Revision, StarterMessageId, TimestampMillis,
    };

    #[test]
    fn explicit_empty_lorebooks_are_not_inherit_or_disabled() {
        let starter = ConversationStarter {
            id: ConversationStarterId::new(),
            character_id: CharacterId::new(),
            name: "Opening".into(),
            ordinal: 0,
            messages: vec![StarterMessage {
                id: StarterMessageId::new(),
                role: StarterRole::User,
                content: "Hello".into(),
            }],
            scene_id: None,
            prompt_id: None,
            lorebooks: Selection::Explicit(Vec::new()),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        assert!(starter.validate().is_ok());
        assert_ne!(starter.lorebooks, Selection::Inherit);
        assert_ne!(starter.lorebooks, Selection::Disabled);
    }
}
