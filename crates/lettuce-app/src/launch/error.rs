use lettuce_context::{BindingRepositoryError, LorebookRepositoryError, PromptRepositoryError};
use lettuce_conversations::{
    ArtifactError, ConversationRepositoryError, PreparedConversationLaunchError, SnapshotSource,
};
use lettuce_models::ModelRepositoryError;
use lettuce_settings::GlobalSettingsStoreError;
use lettuce_types::{
    CharacterId, ConversationId, ConversationStarterId, LorebookId, ModelProfileId, PersonaId,
    PromptDocumentId, ProviderAccountId, SceneId,
};

/// A read failure of one launch source, kept separate per source so a caller
/// can tell which dependency was unavailable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchSourceError {
    #[error("character source failed: {0}")]
    Character(lettuce_characters::RepositoryError),
    #[error("persona source failed: {0}")]
    Persona(lettuce_characters::RepositoryError),
    #[error("prompt source failed: {0}")]
    Prompt(PromptRepositoryError),
    #[error("lorebook source failed: {0}")]
    Lorebook(LorebookRepositoryError),
    #[error("lorebook binding source failed: {0}")]
    Binding(BindingRepositoryError),
    #[error("model source failed: {0}")]
    Model(ModelRepositoryError),
    #[error("global settings source failed: {0}")]
    Settings(GlobalSettingsStoreError),
    #[error("conversation repository failed: {0}")]
    Conversation(ConversationRepositoryError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DirectLaunchError {
    #[error("launch request field {field} is invalid")]
    InvalidRequest { field: &'static str },
    #[error("character {character_id} was not found")]
    CharacterNotFound { character_id: CharacterId },
    #[error("character {character_id} is archived")]
    CharacterArchived { character_id: CharacterId },
    #[error("character {character_id} runs in companion mode, which cannot be launched yet")]
    CompanionUnsupported { character_id: CharacterId },
    #[error("scene {scene_id} was not found")]
    SceneNotFound { scene_id: SceneId },
    #[error("scene {scene_id} is not an active scene of character {character_id}")]
    SceneNotOwned {
        scene_id: SceneId,
        character_id: CharacterId,
    },
    #[error("conversation starter {starter_id} was not found")]
    StarterNotFound { starter_id: ConversationStarterId },
    #[error("conversation starter {starter_id} does not belong to character {character_id}")]
    StarterNotOwned {
        starter_id: ConversationStarterId,
        character_id: CharacterId,
    },
    #[error("persona {persona_id} was not found")]
    PersonaNotFound { persona_id: PersonaId },
    #[error("persona {persona_id} is not active")]
    PersonaInactive { persona_id: PersonaId },
    #[error("prompt {prompt_id} was not found")]
    PromptNotFound { prompt_id: PromptDocumentId },
    #[error("prompt {prompt_id} does not have the direct chat purpose")]
    PromptWrongPurpose { prompt_id: PromptDocumentId },
    #[error("prompt {prompt_id} is archived")]
    PromptArchived { prompt_id: PromptDocumentId },
    #[error("lorebook {lorebook_id} was not found")]
    LorebookNotFound { lorebook_id: LorebookId },
    #[error("lorebook {lorebook_id} is archived")]
    LorebookArchived { lorebook_id: LorebookId },
    #[error("model profile {model_profile_id} was not found")]
    ModelNotFound { model_profile_id: ModelProfileId },
    #[error("provider account {provider_account_id} was not found")]
    ProviderNotFound {
        provider_account_id: ProviderAccountId,
    },
    #[error("provider account {provider_account_id} is disabled")]
    ProviderDisabled {
        provider_account_id: ProviderAccountId,
    },
    #[error("model profile {model_profile_id} is not a chat model")]
    NonChatModel { model_profile_id: ModelProfileId },
    #[error("the launch resolves more than {max} lorebooks")]
    TooManyLorebooks { max: usize },
    #[error("the launch would open with more than {max} messages")]
    TooManyInitialMessages { max: usize },
    #[error("launch source {changed:?} changed while the launch was being planned")]
    SourceChanged { changed: SnapshotSource },
    #[error("conversation {conversation_id} was already launched with this operation key")]
    AlreadyLaunched { conversation_id: ConversationId },
    #[error("a launch snapshot could not be encoded: {0}")]
    ArtifactEncode(ArtifactError),
    #[error("the prepared launch is not internally consistent: {0}")]
    InvalidLaunch(PreparedConversationLaunchError),
    #[error("the operation key was already used for a different launch")]
    CreateConflict,
    #[error(transparent)]
    Repository(#[from] LaunchSourceError),
}

impl From<PreparedConversationLaunchError> for DirectLaunchError {
    fn from(value: PreparedConversationLaunchError) -> Self {
        Self::InvalidLaunch(value)
    }
}

impl From<ArtifactError> for DirectLaunchError {
    fn from(value: ArtifactError) -> Self {
        Self::ArtifactEncode(value)
    }
}
