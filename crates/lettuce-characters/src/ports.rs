use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, GroupId, LorebookId, ModelArtifactId,
    ModelProfileId, Page, PageRequest, PersonaId, PromptDocumentId, Revision, SceneId,
    SceneVariantId, StarterMessageId, TimestampMillis, VoiceProfileId,
};
use serde::{Deserialize, Serialize};

use crate::{
    Character, CharacterDefaults, CharacterMedia, CharacterMediaLink, CharacterProfile,
    CharacterProvenance, ConversationStarter, GroupMember, GroupProfile, Persona, PersonaMedia,
    PersonaMediaLink, RepositoryError, Scene, SceneAssetLink, SceneDocumentV1, SceneOwner,
    SceneVariant, Selection, ValidationError,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCharacterPlan {
    pub character: Character,
    pub scenes: Vec<Scene>,
    pub variants: Vec<SceneVariant>,
    pub starters: Vec<ConversationStarter>,
}

impl CreateCharacterPlan {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.character.validate()?;
        let character_id = self.character.id;
        let mut scene_ordinals = Vec::with_capacity(self.scenes.len());
        crate::constants::validate_unique(
            "character.scene_ids",
            self.scenes.iter().map(|scene| scene.id),
        )?;
        crate::constants::validate_unique(
            "character.variant_ids",
            self.variants.iter().map(|variant| variant.id),
        )?;
        for scene in &self.scenes {
            if scene.owner != SceneOwner::Character(character_id) {
                return Err(ValidationError::InvalidReference {
                    field: "character.scene.owner",
                });
            }
            scene.validate()?;
            scene_ordinals.push(scene.ordinal);
        }
        crate::constants::validate_contiguous("character.scene.order", scene_ordinals)?;
        for scene in &self.scenes {
            let variants: Vec<_> = self
                .variants
                .iter()
                .filter(|variant| variant.scene_id == scene.id)
                .cloned()
                .collect();
            scene.validate_selected_variant(&variants)?;
            crate::constants::validate_contiguous(
                "scene.variant.order",
                variants.iter().map(|variant| variant.ordinal),
            )?;
        }
        if self
            .variants
            .iter()
            .any(|variant| !self.scenes.iter().any(|scene| scene.id == variant.scene_id))
        {
            return Err(ValidationError::InvalidReference {
                field: "character.variant.scene",
            });
        }
        if self
            .character
            .defaults
            .default_scene_id
            .is_some_and(|id| !self.scenes.iter().any(|scene| scene.id == id))
        {
            return Err(ValidationError::InvalidReference {
                field: "character.defaults.default_scene_id",
            });
        }
        let mut starter_ordinals = Vec::with_capacity(self.starters.len());
        crate::constants::validate_unique(
            "character.starter_ids",
            self.starters.iter().map(|starter| starter.id),
        )?;
        for starter in &self.starters {
            if starter.character_id != character_id {
                return Err(ValidationError::InvalidReference {
                    field: "character.starter.character_id",
                });
            }
            starter.validate()?;
            starter_ordinals.push(starter.ordinal);
            if starter
                .scene_id
                .is_some_and(|id| !self.scenes.iter().any(|scene| scene.id == id))
            {
                return Err(ValidationError::InvalidReference {
                    field: "starter.scene_id",
                });
            }
        }
        crate::constants::validate_contiguous("character.starter.order", starter_ordinals)?;
        if self
            .character
            .defaults
            .default_starter_id
            .is_some_and(|id| !self.starters.iter().any(|starter| starter.id == id))
        {
            return Err(ValidationError::InvalidReference {
                field: "character.defaults.default_starter_id",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterDetails {
    pub character: Character,
    pub scenes: Vec<Scene>,
    pub variants: Vec<SceneVariant>,
    pub starters: Vec<ConversationStarter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupStartingScene {
    pub scene: Scene,
    pub variants: Vec<SceneVariant>,
}

impl GroupStartingScene {
    pub fn validate(&self, group_id: GroupId) -> Result<(), ValidationError> {
        if self.scene.owner != SceneOwner::Group(group_id) || self.scene.ordinal != 0 {
            return Err(ValidationError::InvalidReference {
                field: "group.starting_scene",
            });
        }
        crate::constants::validate_unique(
            "group.starting_scene.variant_ids",
            self.variants.iter().map(|variant| variant.id),
        )?;
        self.scene.validate_selected_variant(&self.variants)?;
        crate::constants::validate_contiguous(
            "group.starting_scene.variant.order",
            self.variants.iter().map(|variant| variant.ordinal),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateGroupPlan {
    pub group: GroupProfile,
    pub starting_scene: Option<GroupStartingScene>,
}

impl CreateGroupPlan {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.group.validate()?;
        if let Some(starting_scene) = &self.starting_scene {
            starting_scene.validate(self.group.id)?;
            if self.group.starting_scene_id != Some(starting_scene.scene.id) {
                return Err(ValidationError::InvalidReference {
                    field: "group.starting_scene_id",
                });
            }
        } else if self.group.starting_scene_id.is_some() {
            return Err(ValidationError::InvalidReference {
                field: "group.starting_scene_id",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupDetails {
    pub group: GroupProfile,
    pub starting_scene: Option<GroupStartingScene>,
}

impl GroupDetails {
    pub fn validate(&self) -> Result<(), ValidationError> {
        CreateGroupPlan {
            group: self.group.clone(),
            starting_scene: self.starting_scene.clone(),
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSearch {
    pub text: String,
    pub include_archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneDraftUpdate {
    pub content: SceneDocumentV1,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneVariantDraftUpdate {
    pub content: SceneDocumentV1,
    pub direction: Option<String>,
}

impl SceneVariantDraftUpdate {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.content.validate()?;
        if let Some(direction) = &self.direction {
            crate::constants::validate_text("scene_variant.direction", direction)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationStarterDraftUpdate {
    pub name: String,
    pub scene_id: Option<SceneId>,
    pub prompt_id: Option<PromptDocumentId>,
    pub lorebooks: Selection<Vec<LorebookId>>,
}

impl ConversationStarterDraftUpdate {
    pub fn validate(&self) -> Result<(), ValidationError> {
        crate::constants::validate_name("starter.name", &self.name)?;
        if let Selection::Explicit(lorebooks) = &self.lorebooks {
            crate::constants::validate_collection(
                "starter.lorebooks",
                lorebooks,
                crate::constants::MAX_COLLECTION_ITEMS,
            )?;
            crate::constants::validate_unique("starter.lorebook_ids", lorebooks.iter().copied())?;
        }
        Ok(())
    }
}

impl SceneDraftUpdate {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.content.validate()?;
        if let Some(direction) = &self.direction {
            crate::constants::validate_text("scene.direction", direction)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDuplicateRequest {
    pub source_character_id: CharacterId,
    pub destination_character_id: CharacterId,
    /// If present, overrides only the copied root character name. All other
    /// destination data comes from the source graph loaded by the adapter.
    pub destination_name: Option<String>,
    /// Operation timestamp used for the newly duplicated destination graph;
    /// the adapter applies it consistently to destination authored records.
    pub now: TimestampMillis,
}

impl ProfileDuplicateRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.source_character_id == self.destination_character_id {
            return Err(ValidationError::InvalidReference {
                field: "duplicate.destination_character_id",
            });
        }
        if let Some(name) = &self.destination_name {
            crate::constants::validate_name("duplicate.destination_name", name)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdRemap<T> {
    pub source: T,
    pub destination: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedLegacyReference {
    VoiceLocator,
    ImageRecommendation,
    WidgetImageToken,
}

/// Typed external references retained by a duplication. ID vectors are
/// required to be sorted and unique; `asset_ids` covers character
/// media, scene links, and logical assets referenced by presentation widgets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RetainedExternalReferences {
    pub asset_ids: Vec<AssetId>,
    pub prompt_document_ids: Vec<PromptDocumentId>,
    pub lorebook_ids: Vec<LorebookId>,
    pub model_profile_ids: Vec<ModelProfileId>,
    pub voice_profile_ids: Vec<VoiceProfileId>,
    pub model_artifact_ids: Vec<ModelArtifactId>,
    pub unresolved_legacy_references: Vec<UnresolvedLegacyReference>,
}

impl RetainedExternalReferences {
    pub fn validate(&self) -> Result<(), ValidationError> {
        fn sorted_unique<T: Ord>(values: &[T]) -> bool {
            values.windows(2).all(|pair| pair[0] < pair[1])
        }
        if !sorted_unique(&self.asset_ids)
            || !sorted_unique(&self.prompt_document_ids)
            || !sorted_unique(&self.lorebook_ids)
            || !sorted_unique(&self.model_profile_ids)
            || !sorted_unique(&self.voice_profile_ids)
            || !sorted_unique(&self.model_artifact_ids)
        {
            return Err(ValidationError::Invariant {
                field: "duplicate.retained_external_references.order",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDuplicateResult {
    pub character_id: CharacterId,
    pub remapped_scene_ids: Vec<IdRemap<SceneId>>,
    pub remapped_variant_ids: Vec<IdRemap<SceneVariantId>>,
    pub remapped_scene_asset_link_ids: Vec<IdRemap<lettuce_types::SceneAssetLinkId>>,
    pub remapped_starter_ids: Vec<IdRemap<ConversationStarterId>>,
    pub remapped_starter_message_ids: Vec<IdRemap<StarterMessageId>>,
    pub retained_external_references: RetainedExternalReferences,
}

impl ProfileDuplicateResult {
    /// Validates that an adapter returned a complete, internally consistent
    /// source-to-destination mapping for the requested destination root.
    pub fn validate_for(&self, request: &ProfileDuplicateRequest) -> Result<(), ValidationError> {
        request.validate()?;
        if self.character_id != request.destination_character_id {
            return Err(ValidationError::InvalidReference {
                field: "duplicate.result.character_id",
            });
        }
        self.retained_external_references.validate()?;

        fn validate_remaps<T: Copy + Eq + std::hash::Hash>(
            field: &'static str,
            remaps: &[IdRemap<T>],
        ) -> Result<(), ValidationError> {
            let mut sources = std::collections::HashSet::new();
            let mut destinations = std::collections::HashSet::new();
            for remap in remaps {
                if remap.source == remap.destination {
                    return Err(ValidationError::InvalidReference { field });
                }
                if !sources.insert(remap.source) || !destinations.insert(remap.destination) {
                    return Err(ValidationError::Duplicate { field });
                }
            }
            Ok(())
        }

        validate_remaps("duplicate.result.scene_ids", &self.remapped_scene_ids)?;
        validate_remaps("duplicate.result.variant_ids", &self.remapped_variant_ids)?;
        validate_remaps(
            "duplicate.result.scene_asset_link_ids",
            &self.remapped_scene_asset_link_ids,
        )?;
        validate_remaps("duplicate.result.starter_ids", &self.remapped_starter_ids)?;
        validate_remaps(
            "duplicate.result.starter_message_ids",
            &self.remapped_starter_message_ids,
        )?;
        Ok(())
    }
}

pub trait CharacterRepository: Send + Sync {
    fn create(&self, plan: CreateCharacterPlan) -> Result<CharacterDetails, RepositoryError>;
    fn get(&self, id: CharacterId) -> Result<Option<CharacterDetails>, RepositoryError>;
    fn list(
        &self,
        request: PageRequest,
        include_archived: bool,
    ) -> Result<Page<Character>, RepositoryError>;
    fn search(
        &self,
        request: CharacterSearch,
        page: PageRequest,
    ) -> Result<Page<Character>, RepositoryError>;
    fn revise_profile(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        profile: CharacterProfile,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn update_image_recommendation(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        recommendation: Option<crate::ImageRecommendation>,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn revise_provenance(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        provenance: CharacterProvenance,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn update_defaults(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        defaults: CharacterDefaults,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn update_presentation(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        presentation: crate::CharacterPresentationV1,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn update_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        media: CharacterMedia,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn attach_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        link: CharacterMediaLink,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn detach_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        asset_id: lettuce_types::AssetId,
        slot: crate::CharacterMediaSlot,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn reorder_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        slot: crate::CharacterMediaSlot,
        asset_id: lettuce_types::AssetId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn archive(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
    fn restore(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError>;
}

pub trait SceneRepository: Send + Sync {
    /// Every mutation uses the character root revision as its CAS token. A
    /// successful child mutation increments that root exactly once; child
    /// revisions are incremented only when their own data changes.
    fn add_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene: Scene,
        now: TimestampMillis,
    ) -> Result<Scene, RepositoryError>;
    /// Updates only scene draft scalars. IDs, ownership, ordinal, assets,
    /// variants, and timestamps are adapter-owned and cannot be replaced by
    /// this operation.
    fn update_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        draft: SceneDraftUpdate,
        now: TimestampMillis,
    ) -> Result<Scene, RepositoryError>;
    /// Removes a scene only when no starter references it. A
    /// `DependencyReference::StarterScene` must produce `HasDependencies`,
    /// even when `replacement_default` is supplied; that replacement only
    /// handles the character's default-scene pointer.
    fn remove_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        replacement_default: Option<SceneId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn reorder_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn add_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        variant: SceneVariant,
        now: TimestampMillis,
    ) -> Result<SceneVariant, RepositoryError>;
    fn update_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        variant_id: SceneVariantId,
        draft: SceneVariantDraftUpdate,
        now: TimestampMillis,
    ) -> Result<SceneVariant, RepositoryError>;
    fn remove_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        variant_id: SceneVariantId,
        replacement_selected: Option<SceneVariantId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn reorder_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        variant_id: SceneVariantId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn select_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        variant_id: Option<SceneVariantId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    /// Replaces the complete asset list for a scene. The adapter must load
    /// base and variant documents, apply this list, run
    /// `validate_selected_variant`, and commit the operation atomically.
    fn replace_scene_assets(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        assets: Vec<SceneAssetLink>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
}

pub trait StarterRepository: Send + Sync {
    /// Every starter/message mutation uses the character root revision as its
    /// CAS token. A successful mutation increments that root exactly once;
    /// starter revisions are adapter-incremented only when changed.
    fn add_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter: ConversationStarter,
        now: TimestampMillis,
    ) -> Result<ConversationStarter, RepositoryError>;
    fn update_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        draft: ConversationStarterDraftUpdate,
        now: TimestampMillis,
    ) -> Result<ConversationStarter, RepositoryError>;
    fn remove_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        replacement_default: Option<ConversationStarterId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn reorder_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn insert_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message: crate::StarterMessage,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn update_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message: crate::StarterMessage,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn remove_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message_id: StarterMessageId,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn reorder_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message_id: StarterMessageId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn change_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        scene_id: Option<SceneId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn change_prompt(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        prompt_id: Option<PromptDocumentId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn change_lorebooks(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        lorebooks: Selection<Vec<LorebookId>>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
    fn set_default_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: Option<ConversationStarterId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError>;
}

pub trait PersonaRepository: Send + Sync {
    fn create(&self, persona: Persona) -> Result<Persona, RepositoryError>;
    fn get(&self, id: PersonaId) -> Result<Option<Persona>, RepositoryError>;
    fn get_default(&self) -> Result<Option<Persona>, RepositoryError>;
    fn list(
        &self,
        request: PageRequest,
        include_archived: bool,
    ) -> Result<Page<Persona>, RepositoryError>;
    fn search(&self, text: &str, page: PageRequest) -> Result<Page<Persona>, RepositoryError>;
    fn revise(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        persona: Persona,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn update_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        media: PersonaMedia,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn attach_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        link: PersonaMediaLink,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn detach_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        asset_id: lettuce_types::AssetId,
        slot: crate::PersonaMediaSlot,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn reorder_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        slot: crate::PersonaMediaSlot,
        asset_id: lettuce_types::AssetId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn set_default(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn clear_default(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn archive(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
    fn restore(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError>;
}

pub trait GroupRepository: Send + Sync {
    fn create(&self, plan: CreateGroupPlan) -> Result<GroupDetails, RepositoryError>;
    fn get(&self, id: GroupId) -> Result<Option<GroupDetails>, RepositoryError>;
    fn list(
        &self,
        request: PageRequest,
        include_archived: bool,
    ) -> Result<Page<GroupProfile>, RepositoryError>;
    fn rename(
        &self,
        id: GroupId,
        expected_revision: Revision,
        name: String,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_persona(
        &self,
        id: GroupId,
        expected_revision: Revision,
        persona: Selection<PersonaId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_chat_mode(
        &self,
        id: GroupId,
        expected_revision: Revision,
        mode: crate::ChatMode,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_speaker_selection(
        &self,
        id: GroupId,
        expected_revision: Revision,
        selection: crate::SpeakerSelection,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_memory_policy(
        &self,
        id: GroupId,
        expected_revision: Revision,
        policy: crate::MemoryPolicy,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_disable_character_lorebooks(
        &self,
        id: GroupId,
        expected_revision: Revision,
        disabled: bool,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_prompt_defaults(
        &self,
        id: GroupId,
        expected_revision: Revision,
        conversation_prompt_id: Option<PromptDocumentId>,
        roleplay_prompt_id: Option<PromptDocumentId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_presentation(
        &self,
        id: GroupId,
        expected_revision: Revision,
        presentation: crate::ChatAppearanceV1,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_background(
        &self,
        id: GroupId,
        expected_revision: Revision,
        asset_id: Option<lettuce_types::AssetId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_starting_scene(
        &self,
        id: GroupId,
        expected_revision: Revision,
        starting_scene: Option<GroupStartingScene>,
        now: TimestampMillis,
    ) -> Result<GroupDetails, RepositoryError>;
    fn replace_members(
        &self,
        id: GroupId,
        expected_revision: Revision,
        members: Vec<GroupMember>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn reorder_member(
        &self,
        id: GroupId,
        expected_revision: Revision,
        character_id: CharacterId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_member_muted(
        &self,
        id: GroupId,
        expected_revision: Revision,
        character_id: CharacterId,
        muted: bool,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn set_member_model_override(
        &self,
        id: GroupId,
        expected_revision: Revision,
        character_id: CharacterId,
        model_profile_id: Option<lettuce_types::ModelProfileId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn archive(
        &self,
        id: GroupId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
    fn restore(
        &self,
        id: GroupId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyReference {
    CharacterInGroup {
        group_id: GroupId,
    },
    CharacterDefaultScene {
        scene_id: SceneId,
    },
    CharacterDefaultStarter {
        starter_id: ConversationStarterId,
    },
    StarterScene {
        starter_id: ConversationStarterId,
        scene_id: SceneId,
    },
    PersonaInGroup {
        group_id: GroupId,
    },
    GroupStartingScene {
        scene_id: SceneId,
    },
    Asset {
        asset_id: lettuce_types::AssetId,
    },
    Prompt {
        prompt_id: PromptDocumentId,
    },
    Lorebook {
        lorebook_id: LorebookId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DependencyReport {
    pub references: Vec<DependencyReference>,
}

pub trait CharacterDependencyReader: Send + Sync {
    fn dependencies(&self, id: CharacterId) -> Result<DependencyReport, RepositoryError>;
}

pub trait PersonaDependencyReader: Send + Sync {
    fn dependencies(&self, id: PersonaId) -> Result<DependencyReport, RepositoryError>;
}

pub trait GroupDependencyReader: Send + Sync {
    fn dependencies(&self, id: GroupId) -> Result<DependencyReport, RepositoryError>;
}

pub trait ProfileDuplicateRepository: Send + Sync {
    /// Implementations execute this as one named database transaction. The
    /// operation remaps owned IDs, shares logical assets and external refs,
    /// and never copies conversations or live companion/memory state.
    fn duplicate_character(
        &self,
        request: ProfileDuplicateRequest,
    ) -> Result<ProfileDuplicateResult, RepositoryError>;
}

// Keep these imports visible in generated API documentation: these are the
// concrete association values a database adapter consumes for named writes.
#[allow(unused_imports)]
use CharacterMediaLink as _CharacterMediaLink;

#[cfg(test)]
mod tests {
    use super::{
        ConversationStarterDraftUpdate, CreateGroupPlan, GroupStartingScene, IdRemap,
        ProfileDuplicateRequest, ProfileDuplicateResult, RetainedExternalReferences,
        SceneDraftUpdate, SceneVariantDraftUpdate, UnresolvedLegacyReference,
    };
    use crate::{
        GroupMember, GroupProfile, Scene, SceneDocumentV1, SceneOwner, ScenePart, Selection,
        ValidationError,
    };
    use lettuce_types::{CharacterId, GroupId, SceneId, TimestampMillis};

    fn group(group_id: GroupId) -> GroupProfile {
        GroupProfile::new(
            group_id,
            "Cast".into(),
            vec![
                GroupMember {
                    character_id: CharacterId::new(),
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: CharacterId::new(),
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::UNIX_EPOCH,
        )
        .expect("fixture group should validate")
    }

    #[test]
    fn group_starting_scene_is_one_complete_owned_scene() {
        let group_id = GroupId::new();
        let mut group = group(group_id);
        let scene = Scene::new(
            SceneId::new(),
            SceneOwner::Group(group_id),
            0,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "The cast arrives.".into(),
            }])
            .expect("fixture document should validate"),
            TimestampMillis::UNIX_EPOCH,
        )
        .expect("fixture scene should validate");
        group.starting_scene_id = Some(scene.id);
        let plan = CreateGroupPlan {
            group,
            starting_scene: Some(GroupStartingScene {
                scene,
                variants: Vec::new(),
            }),
        };
        assert!(plan.validate().is_ok());

        let wrong_owner = CreateGroupPlan {
            group: plan.group.clone(),
            starting_scene: Some(GroupStartingScene {
                scene: Scene::new(
                    SceneId::new(),
                    SceneOwner::Group(GroupId::new()),
                    0,
                    SceneDocumentV1::new(vec![ScenePart::Text {
                        text: "Wrong group.".into(),
                    }])
                    .expect("fixture document should validate"),
                    TimestampMillis::UNIX_EPOCH,
                )
                .expect("fixture scene should validate"),
                variants: Vec::new(),
            }),
        };
        assert_eq!(
            wrong_owner.validate(),
            Err(ValidationError::InvalidReference {
                field: "group.starting_scene",
            })
        );
    }

    #[test]
    fn scene_draft_update_is_scalar_only_and_validates_its_document() {
        let update = SceneDraftUpdate {
            content: SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Updated direction".into(),
            }])
            .expect("document"),
            direction: Some("Enter carefully".into()),
        };
        assert!(update.validate().is_ok());
        let encoded = serde_json::to_string(&update).expect("draft serializes");
        assert!(serde_json::from_str::<SceneDraftUpdate>(&encoded).is_ok());
    }

    #[test]
    fn duplicate_request_uses_only_root_override_and_typed_remaps() {
        let request = ProfileDuplicateRequest {
            source_character_id: CharacterId::new(),
            destination_character_id: CharacterId::new(),
            destination_name: Some("Copy".into()),
            now: TimestampMillis::new(1),
        };
        assert!(request.validate().is_ok());
        let same_root = ProfileDuplicateRequest {
            destination_character_id: request.source_character_id,
            ..request
        };
        assert!(same_root.validate().is_err());
        let remap = IdRemap {
            source: SceneId::new(),
            destination: SceneId::new(),
        };
        let encoded = serde_json::to_string(&remap).expect("remap serializes");
        assert!(serde_json::from_str::<IdRemap<SceneId>>(&encoded).is_ok());
    }

    #[test]
    fn child_draft_updates_exclude_identity_and_validate_authored_fields() {
        let variant = SceneVariantDraftUpdate {
            content: SceneDocumentV1::new(Vec::new()).expect("document"),
            direction: Some("Take the left path".into()),
        };
        assert!(variant.validate().is_ok());
        let starter = ConversationStarterDraftUpdate {
            name: "Opening".into(),
            scene_id: Some(SceneId::new()),
            prompt_id: None,
            lorebooks: Selection::Explicit(Vec::new()),
        };
        assert!(starter.validate().is_ok());
        let invalid = ConversationStarterDraftUpdate {
            name: " ".into(),
            ..starter
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn retained_external_references_are_typed_sorted_and_complete() {
        let mut references = RetainedExternalReferences {
            unresolved_legacy_references: vec![UnresolvedLegacyReference::WidgetImageToken],
            ..RetainedExternalReferences::default()
        };
        references.asset_ids = vec![lettuce_types::AssetId::new(), lettuce_types::AssetId::new()];
        references.asset_ids.sort();
        references.prompt_document_ids = vec![lettuce_types::PromptDocumentId::new()];
        references.lorebook_ids = vec![lettuce_types::LorebookId::new()];
        references.model_profile_ids = vec![lettuce_types::ModelProfileId::new()];
        references.voice_profile_ids = vec![lettuce_types::VoiceProfileId::new()];
        references.model_artifact_ids = vec![lettuce_types::ModelArtifactId::new()];
        assert!(references.validate().is_ok());
        references.asset_ids.push(references.asset_ids[0]);
        assert!(references.validate().is_err());
    }

    #[test]
    fn duplicate_result_validation_rejects_self_duplicate_and_mismatched_mappings() {
        let request = ProfileDuplicateRequest {
            source_character_id: CharacterId::new(),
            destination_character_id: CharacterId::new(),
            destination_name: None,
            now: TimestampMillis::new(1),
        };
        let valid = || ProfileDuplicateResult {
            character_id: request.destination_character_id,
            remapped_scene_ids: Vec::new(),
            remapped_variant_ids: Vec::new(),
            remapped_scene_asset_link_ids: Vec::new(),
            remapped_starter_ids: Vec::new(),
            remapped_starter_message_ids: Vec::new(),
            retained_external_references: RetainedExternalReferences::default(),
        };
        assert!(valid().validate_for(&request).is_ok());

        let mut self_mapping = valid();
        let id = SceneId::new();
        self_mapping.remapped_scene_ids.push(IdRemap {
            source: id,
            destination: id,
        });
        assert!(self_mapping.validate_for(&request).is_err());

        let mut duplicate_mapping = valid();
        let source = SceneId::new();
        duplicate_mapping.remapped_scene_ids = vec![
            IdRemap {
                source,
                destination: SceneId::new(),
            },
            IdRemap {
                source,
                destination: SceneId::new(),
            },
        ];
        assert!(duplicate_mapping.validate_for(&request).is_err());

        let mut mismatched = valid();
        mismatched.character_id = CharacterId::new();
        assert!(mismatched.validate_for(&request).is_err());
    }
}
