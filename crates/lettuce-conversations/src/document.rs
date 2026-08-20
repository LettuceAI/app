//! Versioned payload documents carried inside protected launch snapshots.
//!
//! Bodies are self-contained structural copies of authored values: media is an
//! [`AssetId`] link, and secrets, provider auth, and native paths never travel.

use std::collections::HashSet;

use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, GroupId, LorebookEntryId, LorebookId,
    ModelArtifactId, ModelProfileId, PersonaId, PromptDocumentId, PromptEntryId, ProviderAccountId,
    Revision, SceneAssetLinkId, SceneId, SceneVariantId, SnapshotArtifactId, StarterMessageId,
    VoiceProfileId,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::artifact::MAX_PROTECTED_ARTIFACT_BYTES;
use crate::validation::{
    MAX_AUTHORED_TEXT_BYTES, MAX_CONDITION_DEPTH, MAX_CONDITION_NODES, MAX_DISPLAY_CHARS,
    MAX_DOCUMENT_COLLECTION, MAX_DOCUMENT_ENTRIES, MAX_ENTRY_KEYWORDS, MAX_LOREBOOKS,
    MAX_PARTICIPANTS, validate_collection, validate_text, validate_unique,
};
use crate::{
    ArtifactCodec, ArtifactError, ArtifactRetention, ProtectedArtifactBytes, SnapshotArtifactDraft,
    SnapshotSource, ValidationError,
};

pub const SNAPSHOT_DOCUMENT_FORMAT_V1: u32 = 1;

const MAX_NAME_BYTES: usize = MAX_DISPLAY_CHARS * 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotDocumentKind {
    Character,
    Persona,
    Scene,
    Starter,
    Prompt,
    Lorebook,
    Model,
    Group,
}

impl SnapshotDocumentKind {
    #[must_use]
    pub const fn matches_source(self, source: SnapshotSource) -> bool {
        matches!(
            (self, source),
            (Self::Character, SnapshotSource::Character(_))
                | (Self::Persona, SnapshotSource::Persona(_))
                | (Self::Scene, SnapshotSource::Scene(_))
                | (Self::Starter, SnapshotSource::Starter(_))
                | (Self::Prompt, SnapshotSource::Prompt(_))
                | (Self::Lorebook, SnapshotSource::Lorebook(_))
                | (Self::Model, SnapshotSource::Model(_))
                | (Self::Group, SnapshotSource::Group(_))
        )
    }
}

/// A payload names its own kind and subject, so an envelope cannot claim a
/// document it does not contain.
pub trait SnapshotDocumentBody: Serialize + DeserializeOwned {
    const KIND: SnapshotDocumentKind;

    fn source(&self) -> SnapshotSource;

    fn validate(&self) -> Result<(), ValidationError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEnvelopeV1<T> {
    pub document_kind: SnapshotDocumentKind,
    pub format_version: u32,
    pub source: SnapshotSource,
    pub source_revision: Revision,
    pub payload: T,
}

impl<T: SnapshotDocumentBody> SnapshotEnvelopeV1<T> {
    pub fn new(source_revision: Revision, payload: T) -> Result<Self, ValidationError> {
        let envelope = Self {
            document_kind: T::KIND,
            format_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
            source: payload.source(),
            source_revision,
            payload,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != SNAPSHOT_DOCUMENT_FORMAT_V1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "snapshot_document.format_version",
                version: self.format_version,
            });
        }
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        if self.document_kind != T::KIND || !self.document_kind.matches_source(self.source) {
            return Err(ValidationError::InvalidReference {
                field: "snapshot_document.document_kind",
            });
        }
        if self.payload.source() != self.source {
            return Err(ValidationError::InvalidReference {
                field: "snapshot_document.source",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DocumentSelectionV1<T> {
    Inherit,
    Explicit(T),
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionModeV1 {
    Roleplay,
    Companion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPolicyV1 {
    Manual,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatModeV1 {
    Conversation,
    Roleplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerSelectionV1 {
    Llm,
    Heuristic,
    RoundRobin,
    Director,
    DirectorAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum VoicePreferenceV1 {
    VoiceProfile(VoiceProfileId),
    UnresolvedLegacy { locator: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRecommendationV1 {
    pub artifact_id: Option<ModelArtifactId>,
    pub unresolved_legacy_name: Option<String>,
    pub strength: f32,
}

impl ImageRecommendationV1 {
    pub fn validate(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.artifact_id.is_some() == self.unresolved_legacy_name.is_some() {
            return Err(ValidationError::Invariant { field });
        }
        if !self.strength.is_finite() || !(0.0..=2.0).contains(&self.strength) {
            return Err(ValidationError::InvalidValue { field });
        }
        if let Some(name) = &self.unresolved_legacy_name {
            validate_text(field, name, MAX_NAME_BYTES, false)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterMediaSlotV1 {
    AvatarOriginal,
    Background,
    DesignReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterMediaLinkV1 {
    pub asset_id: AssetId,
    pub slot: CharacterMediaSlotV1,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterSnapshotBodyV1 {
    pub character_id: CharacterId,
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub design_description: Option<String>,
    pub interaction_mode: InteractionModeV1,
    pub memory_policy: MemoryPolicyV1,
    pub model_profile_id: Option<ModelProfileId>,
    pub default_scene_id: Option<SceneId>,
    pub default_starter_id: Option<ConversationStarterId>,
    pub direct_prompt_id: Option<PromptDocumentId>,
    pub group_conversation_prompt_id: Option<PromptDocumentId>,
    pub group_roleplay_prompt_id: Option<PromptDocumentId>,
    pub voice: Option<VoicePreferenceV1>,
    pub voice_autoplay: bool,
    pub image_recommendation: Option<ImageRecommendationV1>,
    pub media: Vec<CharacterMediaLinkV1>,
    pub presentation_asset_ids: Vec<AssetId>,
}

impl SnapshotDocumentBody for CharacterSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Character;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Character(self.character_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text("character_document.name", &self.name, MAX_NAME_BYTES, false)?;
        for value in [
            self.nickname.as_deref(),
            self.description.as_deref(),
            self.definition.as_deref(),
            self.design_description.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_text(
                "character_document.profile",
                value,
                MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
        }
        if let Some(VoicePreferenceV1::UnresolvedLegacy { locator }) = &self.voice {
            validate_text(
                "character_document.voice.locator",
                locator,
                MAX_NAME_BYTES,
                false,
            )?;
        }
        if let Some(recommendation) = &self.image_recommendation {
            recommendation.validate("character_document.image_recommendation")?;
        }
        validate_collection(
            "character_document.media",
            &self.media,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "character_document.media.asset_ids",
            self.media.iter().map(|link| link.asset_id),
        )?;
        let mut avatars = 0;
        let mut backgrounds = 0;
        let mut design_ordinals = Vec::new();
        for link in &self.media {
            match link.slot {
                CharacterMediaSlotV1::AvatarOriginal => avatars += 1,
                CharacterMediaSlotV1::Background => backgrounds += 1,
                CharacterMediaSlotV1::DesignReference => design_ordinals.push(link.ordinal),
            }
        }
        if avatars > 1 || backgrounds > 1 {
            return Err(ValidationError::Invariant {
                field: "character_document.media.single_slot",
            });
        }
        validate_sorted_ordinals(
            "character_document.media.design_reference.order",
            design_ordinals,
        )?;
        validate_collection(
            "character_document.presentation_asset_ids",
            &self.presentation_asset_ids,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "character_document.presentation_asset_ids",
            self.presentation_asset_ids.iter().copied(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonaMediaSlotV1 {
    Avatar,
    DesignReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaMediaLinkV1 {
    pub asset_id: AssetId,
    pub slot: PersonaMediaSlotV1,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaSnapshotBodyV1 {
    pub persona_id: PersonaId,
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub design_description: Option<String>,
    pub image_recommendation: Option<ImageRecommendationV1>,
    pub media: Vec<PersonaMediaLinkV1>,
}

impl SnapshotDocumentBody for PersonaSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Persona;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Persona(self.persona_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text("persona_document.title", &self.title, MAX_NAME_BYTES, false)?;
        validate_text(
            "persona_document.description",
            &self.description,
            MAX_AUTHORED_TEXT_BYTES,
            false,
        )?;
        for value in [self.nickname.as_deref(), self.design_description.as_deref()]
            .into_iter()
            .flatten()
        {
            validate_text(
                "persona_document.profile",
                value,
                MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
        }
        if let Some(recommendation) = &self.image_recommendation {
            recommendation.validate("persona_document.image_recommendation")?;
        }
        validate_collection(
            "persona_document.media",
            &self.media,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "persona_document.media.asset_ids",
            self.media.iter().map(|link| link.asset_id),
        )?;
        let mut avatars = 0;
        let mut design_ordinals = Vec::new();
        for link in &self.media {
            match link.slot {
                PersonaMediaSlotV1::Avatar => avatars += 1,
                PersonaMediaSlotV1::DesignReference => design_ordinals.push(link.ordinal),
            }
        }
        if avatars > 1 {
            return Err(ValidationError::Invariant {
                field: "persona_document.media.avatar",
            });
        }
        validate_sorted_ordinals(
            "persona_document.media.design_reference.order",
            design_ordinals,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SceneOwnerV1 {
    Character(CharacterId),
    Group(GroupId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenePartV1 {
    Text { text: String },
    InlineAsset { link_id: SceneAssetLinkId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneAssetSlotV1 {
    Background,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneAssetLinkV1 {
    pub id: SceneAssetLinkId,
    pub asset_id: AssetId,
    pub slot: SceneAssetSlotV1,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneVariantBodyV1 {
    pub variant_id: SceneVariantId,
    pub ordinal: u32,
    pub content: Vec<ScenePartV1>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSnapshotBodyV1 {
    pub scene_id: SceneId,
    pub owner: SceneOwnerV1,
    pub ordinal: u32,
    pub content: Vec<ScenePartV1>,
    pub direction: Option<String>,
    pub selected_variant_id: Option<SceneVariantId>,
    pub variants: Vec<SceneVariantBodyV1>,
    pub assets: Vec<SceneAssetLinkV1>,
}

impl SnapshotDocumentBody for SceneSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Scene;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Scene(self.scene_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_scene_parts("scene_document.content", &self.content)?;
        if let Some(direction) = &self.direction {
            validate_text(
                "scene_document.direction",
                direction,
                MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
        }
        validate_collection(
            "scene_document.assets",
            &self.assets,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "scene_document.assets.link_ids",
            self.assets.iter().map(|link| link.id),
        )?;
        validate_unique(
            "scene_document.assets.asset_ids",
            self.assets.iter().map(|link| link.asset_id),
        )?;
        let mut backgrounds = 0;
        let mut inline_ordinals = Vec::new();
        let mut linked_inline_ids = HashSet::new();
        for link in &self.assets {
            match link.slot {
                SceneAssetSlotV1::Background => {
                    backgrounds += 1;
                    if link.ordinal != 0 {
                        return Err(ValidationError::Invariant {
                            field: "scene_document.background.ordinal",
                        });
                    }
                }
                SceneAssetSlotV1::Inline => {
                    inline_ordinals.push(link.ordinal);
                    linked_inline_ids.insert(link.id);
                }
            }
        }
        if backgrounds > 1 {
            return Err(ValidationError::Invariant {
                field: "scene_document.background",
            });
        }
        validate_sorted_ordinals("scene_document.assets.inline.order", inline_ordinals)?;

        let mut referenced_inline_ids = collect_inline_links(
            &self.content,
            &linked_inline_ids,
            "scene_document.content.inline_link_ids",
        )?;

        validate_collection(
            "scene_document.variants",
            &self.variants,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "scene_document.variants.ids",
            self.variants.iter().map(|variant| variant.variant_id),
        )?;
        validate_positional_ordinals(
            "scene_document.variants.order",
            self.variants.iter().map(|variant| variant.ordinal),
        )?;
        for variant in &self.variants {
            validate_scene_parts("scene_document.variant.content", &variant.content)?;
            if let Some(direction) = &variant.direction {
                validate_text(
                    "scene_document.variant.direction",
                    direction,
                    MAX_AUTHORED_TEXT_BYTES,
                    true,
                )?;
            }
            referenced_inline_ids.extend(collect_inline_links(
                &variant.content,
                &linked_inline_ids,
                "scene_document.variant.inline_link_ids",
            )?);
        }
        if referenced_inline_ids != linked_inline_ids {
            return Err(ValidationError::InvalidReference {
                field: "scene_document.assets.inline.orphan",
            });
        }
        if let Some(selected) = self.selected_variant_id
            && !self
                .variants
                .iter()
                .any(|variant| variant.variant_id == selected)
        {
            return Err(ValidationError::InvalidReference {
                field: "scene_document.selected_variant_id",
            });
        }
        Ok(())
    }
}

fn validate_scene_parts(field: &'static str, parts: &[ScenePartV1]) -> Result<(), ValidationError> {
    validate_collection(field, parts, MAX_DOCUMENT_COLLECTION)?;
    for part in parts {
        if let ScenePartV1::Text { text } = part {
            validate_text(field, text, MAX_AUTHORED_TEXT_BYTES, true)?;
        }
    }
    Ok(())
}

fn collect_inline_links(
    parts: &[ScenePartV1],
    linked: &HashSet<SceneAssetLinkId>,
    field: &'static str,
) -> Result<HashSet<SceneAssetLinkId>, ValidationError> {
    let mut referenced = HashSet::new();
    for part in parts {
        if let ScenePartV1::InlineAsset { link_id } = part {
            if !referenced.insert(*link_id) {
                return Err(ValidationError::Duplicate { field });
            }
            if !linked.contains(link_id) {
                return Err(ValidationError::InvalidReference { field });
            }
        }
    }
    Ok(referenced)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterRoleV1 {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterMessageV1 {
    pub message_id: StarterMessageId,
    pub role: StarterRoleV1,
    pub content: String,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterSnapshotBodyV1 {
    pub starter_id: ConversationStarterId,
    pub character_id: CharacterId,
    pub name: String,
    pub ordinal: u32,
    pub messages: Vec<StarterMessageV1>,
    pub scene_id: Option<SceneId>,
    pub prompt_id: Option<PromptDocumentId>,
    pub lorebooks: DocumentSelectionV1<Vec<LorebookId>>,
}

impl SnapshotDocumentBody for StarterSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Starter;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Starter(self.starter_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text("starter_document.name", &self.name, MAX_NAME_BYTES, false)?;
        validate_collection(
            "starter_document.messages",
            &self.messages,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "starter_document.messages.ids",
            self.messages.iter().map(|message| message.message_id),
        )?;
        validate_positional_ordinals(
            "starter_document.messages.order",
            self.messages.iter().map(|message| message.ordinal),
        )?;
        for message in &self.messages {
            validate_text(
                "starter_document.messages.content",
                &message.content,
                MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
        }
        if let DocumentSelectionV1::Explicit(lorebooks) = &self.lorebooks {
            validate_collection("starter_document.lorebooks", lorebooks, MAX_LOREBOOKS)?;
            validate_unique("starter_document.lorebooks", lorebooks.iter().copied())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPurposeV1 {
    Undefined,
    DirectChat,
    CompanionChat,
    GroupChatRoleplay,
    GroupChatConversational,
    DynamicMemorySummarizer,
    DynamicMemoryManager,
    ReplyHelperRoleplay,
    ReplyHelperConversational,
    LorebookEntryWriter,
    LorebookKeywordGenerator,
    LorebookGeneratorPlanner,
    LorebookGeneratorWriter,
    LorebookGeneratorRefine,
    LorebookGeneratorCoherence,
    AvatarGeneration,
    AvatarEditRequest,
    SceneGeneration,
    ScenePromptWriter,
    DesignReferenceWriter,
    CompanionSoulWriter,
    CompanionGrowthcycle,
    CompanionConsolidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBehaviorVersionV1 {
    LegacyV1,
    DeterministicV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryRoleV1 {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryPositionV1 {
    Relative,
    InChat,
    Conditional,
    Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryImageSlotV1 {
    Character,
    Persona,
    ChatBackground,
    Avatar,
    References,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptEntryPayloadV1 {
    ImageSlot { slot: PromptEntryImageSlotV1 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryChatModeV1 {
    Direct,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryInfoSourceV1 {
    Messages,
    Memory,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptEntryConditionV1 {
    ChatMode { value: PromptEntryChatModeV1 },
    InfoSource { value: PromptEntryInfoSourceV1 },
    SceneGenerationEnabled { value: bool },
    AvatarGenerationEnabled { value: bool },
    IsLocalImageGenerationModel { value: bool },
    IsSceneGenerationLocalImageModel { value: bool },
    HasScene { value: bool },
    HasSceneDirection { value: bool },
    HasPersona { value: bool },
    MessageCountAtLeast { value: u32 },
    ParticipantCountAtLeast { value: u32 },
    KeywordAny { values: Vec<String> },
    KeywordAll { values: Vec<String> },
    KeywordNone { values: Vec<String> },
    DynamicMemoryEnabled { value: bool },
    HasMemorySummary { value: bool },
    HasKeyMemories { value: bool },
    HasLorebookContent { value: bool },
    DoesAuthorNoteExists { value: bool },
    HasActiveScheduledNote { value: bool },
    HasSubjectDescription { value: bool },
    HasCurrentDescription { value: bool },
    HasCharacterReferenceImages { value: bool },
    HasChatBackground { value: bool },
    HasPersonaReferenceImages { value: bool },
    HasCharacterReferenceText { value: bool },
    HasPersonaReferenceText { value: bool },
    InputScopeAny { values: Vec<String> },
    OutputScopeAny { values: Vec<String> },
    ProviderIdAny { values: Vec<String> },
    ReasoningEnabled { value: bool },
    VisionEnabled { value: bool },
    IsTimeAwarenessEnabled { value: bool },
    IsCompanionMode { value: bool },
    All { conditions: Vec<Self> },
    Any { conditions: Vec<Self> },
    Not { condition: Box<Self> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptEntryV1 {
    pub entry_id: PromptEntryId,
    pub built_in_entry_key: Option<String>,
    pub name: String,
    pub role: PromptEntryRoleV1,
    pub content: String,
    pub enabled: bool,
    pub injection_position: PromptEntryPositionV1,
    pub depth: u32,
    pub conditional_min_messages: Option<u32>,
    pub interval_turns: Option<u32>,
    pub system_prompt: bool,
    pub condition: Option<PromptEntryConditionV1>,
    pub payload: Option<PromptEntryPayloadV1>,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptSnapshotBodyV1 {
    pub prompt_id: PromptDocumentId,
    pub name: String,
    pub purpose: PromptPurposeV1,
    pub condense: bool,
    pub behavior_version: PromptBehaviorVersionV1,
    pub entries: Vec<PromptEntryV1>,
}

impl SnapshotDocumentBody for PromptSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Prompt;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Prompt(self.prompt_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text("prompt_document.name", &self.name, MAX_NAME_BYTES, false)?;
        if self.purpose == PromptPurposeV1::Undefined {
            return Err(ValidationError::InvalidValue {
                field: "prompt_document.purpose",
            });
        }
        validate_collection(
            "prompt_document.entries",
            &self.entries,
            MAX_DOCUMENT_ENTRIES,
        )?;
        validate_unique(
            "prompt_document.entries.ids",
            self.entries.iter().map(|entry| entry.entry_id),
        )?;
        validate_positional_ordinals(
            "prompt_document.entries.order",
            self.entries.iter().map(|entry| entry.ordinal),
        )?;
        validate_unique(
            "prompt_document.entries.built_in_entry_keys",
            self.entries
                .iter()
                .filter_map(|entry| entry.built_in_entry_key.as_deref()),
        )?;
        for entry in &self.entries {
            validate_text(
                "prompt_document.entries.name",
                &entry.name,
                MAX_NAME_BYTES,
                true,
            )?;
            validate_text(
                "prompt_document.entries.content",
                &entry.content,
                MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
            if let Some(key) = &entry.built_in_entry_key {
                if key != key.trim() {
                    return Err(ValidationError::InvalidValue {
                        field: "prompt_document.entries.built_in_entry_key",
                    });
                }
                validate_text(
                    "prompt_document.entries.built_in_entry_key",
                    key,
                    MAX_NAME_BYTES,
                    false,
                )?;
            }
            if entry.injection_position == PromptEntryPositionV1::Interval
                && entry.interval_turns.unwrap_or_default() == 0
            {
                return Err(ValidationError::InvalidValue {
                    field: "prompt_document.entries.interval_turns",
                });
            }
            if let Some(condition) = &entry.condition {
                let mut nodes = 0;
                validate_condition(
                    condition,
                    "prompt_document.entries.condition",
                    1,
                    &mut nodes,
                )?;
            }
        }
        Ok(())
    }
}

fn validate_condition(
    condition: &PromptEntryConditionV1,
    field: &'static str,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), ValidationError> {
    if depth > MAX_CONDITION_DEPTH {
        return Err(ValidationError::OutOfBounds { field });
    }
    *nodes += 1;
    if *nodes > MAX_CONDITION_NODES {
        return Err(ValidationError::TooMany {
            field,
            max: MAX_CONDITION_NODES,
        });
    }
    match condition {
        PromptEntryConditionV1::KeywordAny { values }
        | PromptEntryConditionV1::KeywordAll { values }
        | PromptEntryConditionV1::KeywordNone { values }
        | PromptEntryConditionV1::InputScopeAny { values }
        | PromptEntryConditionV1::OutputScopeAny { values }
        | PromptEntryConditionV1::ProviderIdAny { values } => {
            validate_collection(field, values, MAX_DOCUMENT_COLLECTION)?;
            for value in values {
                validate_text(field, value, MAX_NAME_BYTES, false)?;
            }
        }
        PromptEntryConditionV1::All { conditions } | PromptEntryConditionV1::Any { conditions } => {
            validate_collection(field, conditions, MAX_CONDITION_NODES)?;
            for child in conditions {
                validate_condition(child, field, depth + 1, nodes)?;
            }
        }
        PromptEntryConditionV1::Not { condition } => {
            validate_condition(condition, field, depth + 1, nodes)?;
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionPolicyV1 {
    RecentMessageWindow,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeywordMatchModeV1 {
    Literal,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LorebookBehaviorVersionV1 {
    LegacyV1,
    DeterministicV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryV1 {
    pub entry_id: LorebookEntryId,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub match_mode: KeywordMatchModeV1,
    pub content: String,
    pub priority: i32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookSnapshotBodyV1 {
    pub lorebook_id: LorebookId,
    pub name: String,
    pub detection_policy: DetectionPolicyV1,
    pub icon_asset_id: Option<AssetId>,
    pub behavior_version: LorebookBehaviorVersionV1,
    pub entries: Vec<LorebookEntryV1>,
}

impl SnapshotDocumentBody for LorebookSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Lorebook;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Lorebook(self.lorebook_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text("lorebook_document.name", &self.name, MAX_NAME_BYTES, false)?;
        validate_collection(
            "lorebook_document.entries",
            &self.entries,
            MAX_DOCUMENT_ENTRIES,
        )?;
        validate_unique(
            "lorebook_document.entries.ids",
            self.entries.iter().map(|entry| entry.entry_id),
        )?;
        validate_positional_ordinals(
            "lorebook_document.entries.order",
            self.entries.iter().map(|entry| entry.ordinal),
        )?;
        for entry in &self.entries {
            validate_text(
                "lorebook_document.entries.title",
                &entry.title,
                MAX_NAME_BYTES,
                false,
            )?;
            validate_text(
                "lorebook_document.entries.content",
                &entry.content,
                MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
            validate_collection(
                "lorebook_document.entries.keywords",
                &entry.keywords,
                MAX_ENTRY_KEYWORDS,
            )?;
            for keyword in &entry.keywords {
                validate_text(
                    "lorebook_document.entries.keywords",
                    keyword,
                    MAX_NAME_BYTES,
                    false,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKindV1 {
    Chat,
    Image,
    Embedding,
    Speech,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityV1 {
    Text,
    Image,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocolV1 {
    OpenAiCompatible,
    Anthropic,
    Gemini,
    Ollama,
    LlamaCpp,
    StableDiffusion,
}

/// The non-secret half of a provider account: no endpoint, auth, custom paths,
/// or secret headers ever reach a launch document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotProviderDescriptorV1 {
    pub provider_account_id: ProviderAccountId,
    pub provider_revision: Revision,
    pub provider_kind_label: String,
    pub protocol: ProviderProtocolV1,
    pub display_name: String,
}

impl SnapshotProviderDescriptorV1 {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.provider_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_text(
            "model_document.provider.provider_kind_label",
            &self.provider_kind_label,
            MAX_NAME_BYTES,
            false,
        )?;
        validate_text(
            "model_document.provider.display_name",
            &self.display_name,
            MAX_NAME_BYTES,
            false,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSnapshotBodyV1 {
    pub model_profile_id: ModelProfileId,
    pub external_model_id: String,
    pub display_name: String,
    pub kind: ModelKindV1,
    pub input_modalities: Vec<ModalityV1>,
    pub output_modalities: Vec<ModalityV1>,
    pub temperature: Option<f64>,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub provider: SnapshotProviderDescriptorV1,
}

impl SnapshotDocumentBody for ModelSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Model;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Model(self.model_profile_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text(
            "model_document.external_model_id",
            &self.external_model_id,
            MAX_NAME_BYTES,
            false,
        )?;
        validate_text(
            "model_document.display_name",
            &self.display_name,
            MAX_NAME_BYTES,
            false,
        )?;
        if self.input_modalities.is_empty() {
            return Err(ValidationError::Blank {
                field: "model_document.input_modalities",
            });
        }
        if self.output_modalities.is_empty() {
            return Err(ValidationError::Blank {
                field: "model_document.output_modalities",
            });
        }
        validate_collection(
            "model_document.input_modalities",
            &self.input_modalities,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_collection(
            "model_document.output_modalities",
            &self.output_modalities,
            MAX_DOCUMENT_COLLECTION,
        )?;
        validate_unique(
            "model_document.input_modalities",
            self.input_modalities.iter().copied(),
        )?;
        validate_unique(
            "model_document.output_modalities",
            self.output_modalities.iter().copied(),
        )?;
        if self.temperature.is_some_and(|value| !value.is_finite()) {
            return Err(ValidationError::InvalidValue {
                field: "model_document.temperature",
            });
        }
        self.provider.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMemberV1 {
    pub character_id: CharacterId,
    pub ordinal: u32,
    pub muted: bool,
    pub model_profile_override: Option<ModelProfileId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupSnapshotBodyV1 {
    pub group_id: GroupId,
    pub name: String,
    pub chat_mode: ChatModeV1,
    pub speaker_selection: SpeakerSelectionV1,
    pub memory_policy: MemoryPolicyV1,
    pub disable_character_lorebooks: bool,
    pub persona: DocumentSelectionV1<PersonaId>,
    pub group_conversation_prompt_id: Option<PromptDocumentId>,
    pub group_roleplay_prompt_id: Option<PromptDocumentId>,
    pub members: Vec<GroupMemberV1>,
    pub starting_scene_id: Option<SceneId>,
    pub background_asset_id: Option<AssetId>,
}

impl SnapshotDocumentBody for GroupSnapshotBodyV1 {
    const KIND: SnapshotDocumentKind = SnapshotDocumentKind::Group;

    fn source(&self) -> SnapshotSource {
        SnapshotSource::Group(self.group_id)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_text("group_document.name", &self.name, MAX_NAME_BYTES, false)?;
        validate_collection("group_document.members", &self.members, MAX_PARTICIPANTS)?;
        if self.members.len() < 2 {
            return Err(ValidationError::Invariant {
                field: "group_document.members.minimum",
            });
        }
        validate_unique(
            "group_document.members.character_ids",
            self.members.iter().map(|member| member.character_id),
        )?;
        validate_positional_ordinals(
            "group_document.members.order",
            self.members.iter().map(|member| member.ordinal),
        )?;
        if self.members.iter().all(|member| member.muted) {
            return Err(ValidationError::Invariant {
                field: "group_document.members.active",
            });
        }
        Ok(())
    }
}

fn validate_positional_ordinals(
    field: &'static str,
    ordinals: impl IntoIterator<Item = u32>,
) -> Result<(), ValidationError> {
    for (index, ordinal) in ordinals.into_iter().enumerate() {
        if ordinal as usize != index {
            return Err(ValidationError::InvalidValue { field });
        }
    }
    Ok(())
}

fn validate_sorted_ordinals(
    field: &'static str,
    ordinals: impl IntoIterator<Item = u32>,
) -> Result<(), ValidationError> {
    let mut sorted: Vec<u32> = ordinals.into_iter().collect();
    sorted.sort_unstable();
    validate_positional_ordinals(field, sorted)
}

pub fn build_snapshot_draft<T: SnapshotDocumentBody>(
    artifact_id: SnapshotArtifactId,
    envelope: &SnapshotEnvelopeV1<T>,
) -> Result<SnapshotArtifactDraft, ArtifactError> {
    envelope
        .validate()
        .map_err(ArtifactError::InvalidReference)?;
    envelope
        .payload
        .validate()
        .map_err(ArtifactError::InvalidReference)?;
    let encoded = serde_json::to_vec(envelope).map_err(|_| {
        ArtifactError::InvalidReference(ValidationError::InvalidValue {
            field: "snapshot_document.payload",
        })
    })?;
    let bytes = ProtectedArtifactBytes::new(encoded)?;
    let digest = bytes.digest();
    let byte_size = bytes.len() as u64;
    let draft = SnapshotArtifactDraft {
        source: envelope.source,
        source_revision: envelope.source_revision,
        artifact_id,
        digest,
        schema_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
        byte_size,
        codec: ArtifactCodec::Json,
        retention: ArtifactRetention::Conversation,
        bytes,
    };
    draft.validate()?;
    Ok(draft)
}

#[derive(Deserialize)]
struct VersionProbe {
    format_version: u32,
}

pub fn decode_snapshot_document<T: SnapshotDocumentBody>(
    bytes: &[u8],
    expected_kind: SnapshotDocumentKind,
) -> Result<SnapshotEnvelopeV1<T>, ArtifactError> {
    if bytes.is_empty() {
        return Err(ArtifactError::Empty);
    }
    if bytes.len() > MAX_PROTECTED_ARTIFACT_BYTES {
        return Err(ArtifactError::TooLarge {
            size: bytes.len(),
            max: MAX_PROTECTED_ARTIFACT_BYTES,
        });
    }
    if let Ok(probe) = serde_json::from_slice::<VersionProbe>(bytes)
        && probe.format_version != SNAPSHOT_DOCUMENT_FORMAT_V1
    {
        return Err(ArtifactError::InvalidReference(
            ValidationError::UnsupportedVersion {
                field: "snapshot_document.format_version",
                version: probe.format_version,
            },
        ));
    }
    if expected_kind != T::KIND {
        return Err(ArtifactError::InvalidReference(
            ValidationError::InvalidReference {
                field: "snapshot_document.document_kind",
            },
        ));
    }
    let envelope: SnapshotEnvelopeV1<T> = serde_json::from_slice(bytes).map_err(|_| {
        ArtifactError::InvalidReference(ValidationError::InvalidValue {
            field: "snapshot_document.payload",
        })
    })?;
    envelope
        .validate()
        .map_err(ArtifactError::InvalidReference)?;
    envelope
        .payload
        .validate()
        .map_err(ArtifactError::InvalidReference)?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtectedSnapshotRef;

    fn character_body() -> CharacterSnapshotBodyV1 {
        CharacterSnapshotBodyV1 {
            character_id: CharacterId::new(),
            name: "Ada".into(),
            nickname: Some("A".into()),
            description: Some("A meticulous engineer".into()),
            definition: Some("Speaks in precise sentences".into()),
            design_description: None,
            interaction_mode: InteractionModeV1::Roleplay,
            memory_policy: MemoryPolicyV1::Dynamic,
            model_profile_id: Some(ModelProfileId::new()),
            default_scene_id: Some(SceneId::new()),
            default_starter_id: Some(ConversationStarterId::new()),
            direct_prompt_id: Some(PromptDocumentId::new()),
            group_conversation_prompt_id: None,
            group_roleplay_prompt_id: None,
            voice: Some(VoicePreferenceV1::VoiceProfile(VoiceProfileId::new())),
            voice_autoplay: true,
            image_recommendation: Some(ImageRecommendationV1 {
                artifact_id: Some(ModelArtifactId::new()),
                unresolved_legacy_name: None,
                strength: 0.85,
            }),
            media: vec![
                CharacterMediaLinkV1 {
                    asset_id: AssetId::new(),
                    slot: CharacterMediaSlotV1::AvatarOriginal,
                    ordinal: 0,
                },
                CharacterMediaLinkV1 {
                    asset_id: AssetId::new(),
                    slot: CharacterMediaSlotV1::DesignReference,
                    ordinal: 0,
                },
            ],
            presentation_asset_ids: vec![AssetId::new()],
        }
    }

    fn persona_body() -> PersonaSnapshotBodyV1 {
        PersonaSnapshotBodyV1 {
            persona_id: PersonaId::new(),
            title: "Traveller".into(),
            description: "A wandering cartographer".into(),
            nickname: None,
            design_description: Some("Weathered coat".into()),
            image_recommendation: Some(ImageRecommendationV1 {
                artifact_id: None,
                unresolved_legacy_name: Some("legacy-lora".into()),
                strength: 1.0,
            }),
            media: vec![PersonaMediaLinkV1 {
                asset_id: AssetId::new(),
                slot: PersonaMediaSlotV1::Avatar,
                ordinal: 0,
            }],
        }
    }

    fn scene_body() -> SceneSnapshotBodyV1 {
        let link_id = SceneAssetLinkId::new();
        let variant_link_id = SceneAssetLinkId::new();
        let variant_id = SceneVariantId::new();
        SceneSnapshotBodyV1 {
            scene_id: SceneId::new(),
            owner: SceneOwnerV1::Character(CharacterId::new()),
            ordinal: 0,
            content: vec![
                ScenePartV1::Text {
                    text: "A quiet harbour at dawn.".into(),
                },
                ScenePartV1::InlineAsset { link_id },
            ],
            direction: Some("Keep the pacing slow.".into()),
            selected_variant_id: Some(variant_id),
            variants: vec![SceneVariantBodyV1 {
                variant_id,
                ordinal: 0,
                content: vec![
                    ScenePartV1::Text {
                        text: "The same harbour at dusk.".into(),
                    },
                    ScenePartV1::InlineAsset {
                        link_id: variant_link_id,
                    },
                ],
                direction: Some("Warmer light.".into()),
            }],
            assets: vec![
                SceneAssetLinkV1 {
                    id: link_id,
                    asset_id: AssetId::new(),
                    slot: SceneAssetSlotV1::Inline,
                    ordinal: 0,
                },
                SceneAssetLinkV1 {
                    id: variant_link_id,
                    asset_id: AssetId::new(),
                    slot: SceneAssetSlotV1::Inline,
                    ordinal: 1,
                },
                SceneAssetLinkV1 {
                    id: SceneAssetLinkId::new(),
                    asset_id: AssetId::new(),
                    slot: SceneAssetSlotV1::Background,
                    ordinal: 0,
                },
            ],
        }
    }

    fn starter_body() -> StarterSnapshotBodyV1 {
        StarterSnapshotBodyV1 {
            starter_id: ConversationStarterId::new(),
            character_id: CharacterId::new(),
            name: "Opening".into(),
            ordinal: 0,
            messages: vec![
                StarterMessageV1 {
                    message_id: StarterMessageId::new(),
                    role: StarterRoleV1::User,
                    content: "Hello".into(),
                    ordinal: 0,
                },
                StarterMessageV1 {
                    message_id: StarterMessageId::new(),
                    role: StarterRoleV1::Assistant,
                    content: "Welcome aboard".into(),
                    ordinal: 1,
                },
            ],
            scene_id: Some(SceneId::new()),
            prompt_id: Some(PromptDocumentId::new()),
            lorebooks: DocumentSelectionV1::Explicit(vec![LorebookId::new()]),
        }
    }

    fn prompt_body() -> PromptSnapshotBodyV1 {
        PromptSnapshotBodyV1 {
            prompt_id: PromptDocumentId::new(),
            name: "Direct chat".into(),
            purpose: PromptPurposeV1::DirectChat,
            condense: false,
            behavior_version: PromptBehaviorVersionV1::DeterministicV2,
            entries: vec![
                PromptEntryV1 {
                    entry_id: PromptEntryId::new(),
                    built_in_entry_key: Some("core.system".into()),
                    name: "System".into(),
                    role: PromptEntryRoleV1::System,
                    content: "You are {{char}}.".into(),
                    enabled: true,
                    injection_position: PromptEntryPositionV1::Relative,
                    depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: true,
                    condition: Some(PromptEntryConditionV1::All {
                        conditions: vec![
                            PromptEntryConditionV1::HasPersona { value: true },
                            PromptEntryConditionV1::Not {
                                condition: Box::new(PromptEntryConditionV1::KeywordAny {
                                    values: vec!["harbour".into()],
                                }),
                            },
                        ],
                    }),
                    payload: None,
                    ordinal: 0,
                },
                PromptEntryV1 {
                    entry_id: PromptEntryId::new(),
                    built_in_entry_key: None,
                    name: "Character image".into(),
                    role: PromptEntryRoleV1::User,
                    content: String::new(),
                    enabled: true,
                    injection_position: PromptEntryPositionV1::Interval,
                    depth: 2,
                    conditional_min_messages: Some(4),
                    interval_turns: Some(3),
                    system_prompt: false,
                    condition: None,
                    payload: Some(PromptEntryPayloadV1::ImageSlot {
                        slot: PromptEntryImageSlotV1::Character,
                    }),
                    ordinal: 1,
                },
            ],
        }
    }

    fn lorebook_body() -> LorebookSnapshotBodyV1 {
        LorebookSnapshotBodyV1 {
            lorebook_id: LorebookId::new(),
            name: "Harbour lore".into(),
            detection_policy: DetectionPolicyV1::RecentMessageWindow,
            icon_asset_id: Some(AssetId::new()),
            behavior_version: LorebookBehaviorVersionV1::LegacyV1,
            entries: vec![
                LorebookEntryV1 {
                    entry_id: LorebookEntryId::new(),
                    title: "The lighthouse".into(),
                    enabled: true,
                    always_active: false,
                    keywords: vec!["lighthouse".into(), "beacon".into()],
                    case_sensitive: false,
                    match_mode: KeywordMatchModeV1::Literal,
                    content: "The lighthouse has been dark for years.".into(),
                    priority: 10,
                    ordinal: 0,
                },
                LorebookEntryV1 {
                    entry_id: LorebookEntryId::new(),
                    title: "The harbourmaster".into(),
                    enabled: true,
                    always_active: false,
                    keywords: vec!["harbourmaster".into()],
                    case_sensitive: false,
                    match_mode: KeywordMatchModeV1::Literal,
                    content: "She keeps every ledger by hand.".into(),
                    priority: 5,
                    ordinal: 1,
                },
            ],
        }
    }

    fn model_body() -> ModelSnapshotBodyV1 {
        ModelSnapshotBodyV1 {
            model_profile_id: ModelProfileId::new(),
            external_model_id: "gpt-4o-mini".into(),
            display_name: "Fast chat".into(),
            kind: ModelKindV1::Chat,
            input_modalities: vec![ModalityV1::Text, ModalityV1::Image],
            output_modalities: vec![ModalityV1::Text],
            temperature: Some(0.7),
            context_length: Some(128_000),
            max_output_tokens: Some(4096),
            provider: SnapshotProviderDescriptorV1 {
                provider_account_id: ProviderAccountId::new(),
                provider_revision: Revision::INITIAL,
                provider_kind_label: "openai".into(),
                protocol: ProviderProtocolV1::OpenAiCompatible,
                display_name: "Personal key".into(),
            },
        }
    }

    fn group_body() -> GroupSnapshotBodyV1 {
        GroupSnapshotBodyV1 {
            group_id: GroupId::new(),
            name: "Cast".into(),
            chat_mode: ChatModeV1::Roleplay,
            speaker_selection: SpeakerSelectionV1::RoundRobin,
            memory_policy: MemoryPolicyV1::Manual,
            disable_character_lorebooks: false,
            persona: DocumentSelectionV1::Explicit(PersonaId::new()),
            group_conversation_prompt_id: None,
            group_roleplay_prompt_id: Some(PromptDocumentId::new()),
            members: vec![
                GroupMemberV1 {
                    character_id: CharacterId::new(),
                    ordinal: 0,
                    muted: false,
                    model_profile_override: Some(ModelProfileId::new()),
                },
                GroupMemberV1 {
                    character_id: CharacterId::new(),
                    ordinal: 1,
                    muted: true,
                    model_profile_override: None,
                },
            ],
            starting_scene_id: Some(SceneId::new()),
            background_asset_id: Some(AssetId::new()),
        }
    }

    fn envelope<T: SnapshotDocumentBody>(payload: T) -> SnapshotEnvelopeV1<T> {
        SnapshotEnvelopeV1::new(Revision::new(3), payload).expect("envelope")
    }

    fn round_trip<T>(payload: T)
    where
        T: SnapshotDocumentBody + PartialEq + std::fmt::Debug,
    {
        let envelope = envelope(payload);
        let draft = build_snapshot_draft(SnapshotArtifactId::new(), &envelope).expect("draft");
        draft.validate().expect("draft validates");
        let reference = draft.reference();
        reference.validate().expect("reference validates");
        assert_eq!(reference.schema_version, SNAPSHOT_DOCUMENT_FORMAT_V1);
        let bytes = draft.bytes.into_store_bytes();
        let decoded: SnapshotEnvelopeV1<T> =
            decode_snapshot_document(&bytes, T::KIND).expect("decode");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn every_body_kind_round_trips_through_the_protected_payload() {
        round_trip(character_body());
        round_trip(persona_body());
        round_trip(scene_body());
        round_trip(starter_body());
        round_trip(prompt_body());
        round_trip(lorebook_body());
        round_trip(model_body());
        round_trip(group_body());
    }

    #[test]
    fn equal_envelopes_produce_identical_digest_and_size() {
        let body = character_body();
        let first = envelope(body.clone());
        let second = envelope(body);
        let first_draft = build_snapshot_draft(SnapshotArtifactId::new(), &first).expect("draft");
        let second_draft = build_snapshot_draft(SnapshotArtifactId::new(), &second).expect("draft");
        assert_eq!(first_draft.digest, second_draft.digest);
        assert_eq!(first_draft.byte_size, second_draft.byte_size);
    }

    #[test]
    fn draft_reference_round_trips_through_protected_snapshot_ref() {
        let envelope = envelope(lorebook_body());
        let draft = build_snapshot_draft(SnapshotArtifactId::new(), &envelope).expect("draft");
        let reference = draft.reference();
        reference.validate().expect("reference validates");
        let encoded = serde_json::to_string(&reference).expect("reference serializes");
        let decoded: ProtectedSnapshotRef =
            serde_json::from_str(&encoded).expect("reference decodes");
        assert_eq!(decoded, reference);
        assert_eq!(decoded.source, envelope.source);
        assert_eq!(decoded.source_revision, envelope.source_revision);
    }

    #[test]
    fn envelope_rejects_a_kind_that_disagrees_with_its_source() {
        let mismatched = SnapshotEnvelopeV1 {
            document_kind: SnapshotDocumentKind::Character,
            format_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
            source: SnapshotSource::Persona(PersonaId::new()),
            source_revision: Revision::INITIAL,
            payload: persona_body(),
        };
        assert_eq!(
            mismatched.validate(),
            Err(ValidationError::InvalidReference {
                field: "snapshot_document.document_kind"
            })
        );
        assert!(matches!(
            build_snapshot_draft(SnapshotArtifactId::new(), &mismatched),
            Err(ArtifactError::InvalidReference(_))
        ));
    }

    #[test]
    fn a_body_cannot_travel_under_another_subjects_source() {
        let body = persona_body();
        let foreign = SnapshotEnvelopeV1 {
            document_kind: SnapshotDocumentKind::Persona,
            format_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
            source: SnapshotSource::Persona(PersonaId::new()),
            source_revision: Revision::INITIAL,
            payload: body,
        };
        assert_eq!(
            foreign.validate(),
            Err(ValidationError::InvalidReference {
                field: "snapshot_document.source"
            })
        );
        assert!(matches!(
            build_snapshot_draft(SnapshotArtifactId::new(), &foreign),
            Err(ArtifactError::InvalidReference(
                ValidationError::InvalidReference {
                    field: "snapshot_document.source"
                }
            ))
        ));
        let bytes = serde_json::to_vec(&foreign).expect("encode");
        assert!(matches!(
            decode_snapshot_document::<PersonaSnapshotBodyV1>(
                &bytes,
                SnapshotDocumentKind::Persona
            ),
            Err(ArtifactError::InvalidReference(
                ValidationError::InvalidReference {
                    field: "snapshot_document.source"
                }
            ))
        ));
    }

    #[test]
    fn voice_and_provider_account_sources_have_no_document_kind() {
        for source in [
            SnapshotSource::Voice(VoiceProfileId::new()),
            SnapshotSource::ProviderAccount(ProviderAccountId::new()),
        ] {
            for kind in [
                SnapshotDocumentKind::Character,
                SnapshotDocumentKind::Persona,
                SnapshotDocumentKind::Scene,
                SnapshotDocumentKind::Starter,
                SnapshotDocumentKind::Prompt,
                SnapshotDocumentKind::Lorebook,
                SnapshotDocumentKind::Model,
                SnapshotDocumentKind::Group,
            ] {
                assert!(!kind.matches_source(source));
            }
            let envelope = SnapshotEnvelopeV1 {
                document_kind: SnapshotDocumentKind::Model,
                format_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
                source,
                source_revision: Revision::INITIAL,
                payload: model_body(),
            };
            assert!(envelope.validate().is_err());
        }
    }

    #[test]
    fn envelope_rejects_a_zero_source_revision() {
        let body = group_body();
        let envelope = SnapshotEnvelopeV1 {
            document_kind: SnapshotDocumentKind::Group,
            format_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
            source: SnapshotSource::Group(body.group_id),
            source_revision: Revision::new(0),
            payload: body,
        };
        assert_eq!(envelope.validate(), Err(ValidationError::ZeroRevision));
    }

    #[test]
    fn decode_rejects_unsupported_format_versions() {
        let body = persona_body();
        let source = body.source();
        for version in [0, 2] {
            let envelope = SnapshotEnvelopeV1 {
                document_kind: SnapshotDocumentKind::Persona,
                format_version: version,
                source,
                source_revision: Revision::INITIAL,
                payload: body.clone(),
            };
            let bytes = serde_json::to_vec(&envelope).expect("encode");
            assert!(matches!(
                decode_snapshot_document::<PersonaSnapshotBodyV1>(
                    &bytes,
                    SnapshotDocumentKind::Persona
                ),
                Err(ArtifactError::InvalidReference(
                    ValidationError::UnsupportedVersion { .. }
                ))
            ));
        }
    }

    #[test]
    fn a_future_version_is_reported_as_such_even_with_unknown_fields() {
        let body = persona_body();
        let envelope = SnapshotEnvelopeV1 {
            document_kind: SnapshotDocumentKind::Persona,
            format_version: 2,
            source: body.source(),
            source_revision: Revision::INITIAL,
            payload: body,
        };
        let encoded = serde_json::to_string(&envelope).expect("encode");
        let with_future_field = encoded.replacen('{', "{\"retention_policy\":\"pinned\",", 1);
        assert!(matches!(
            decode_snapshot_document::<PersonaSnapshotBodyV1>(
                with_future_field.as_bytes(),
                SnapshotDocumentKind::Persona
            ),
            Err(ArtifactError::InvalidReference(
                ValidationError::UnsupportedVersion {
                    field: "snapshot_document.format_version",
                    version: 2
                }
            ))
        ));
    }

    #[test]
    fn decode_rejects_unknown_fields_and_corrupt_json() {
        let envelope = envelope(starter_body());
        let encoded = serde_json::to_string(&envelope).expect("encode");

        let with_unknown = encoded.replacen('{', "{\"unexpected\":1,", 1);
        assert!(matches!(
            decode_snapshot_document::<StarterSnapshotBodyV1>(
                with_unknown.as_bytes(),
                SnapshotDocumentKind::Starter
            ),
            Err(ArtifactError::InvalidReference(
                ValidationError::InvalidValue { .. }
            ))
        ));

        let truncated = &encoded.as_bytes()[..encoded.len() / 2];
        assert!(matches!(
            decode_snapshot_document::<StarterSnapshotBodyV1>(
                truncated,
                SnapshotDocumentKind::Starter
            ),
            Err(ArtifactError::InvalidReference(_))
        ));

        assert!(matches!(
            decode_snapshot_document::<StarterSnapshotBodyV1>(b"", SnapshotDocumentKind::Starter),
            Err(ArtifactError::Empty)
        ));
    }

    #[test]
    fn decode_rejects_a_document_whose_source_disagrees_with_the_expected_kind() {
        let envelope = envelope(scene_body());
        let draft = build_snapshot_draft(SnapshotArtifactId::new(), &envelope).expect("draft");
        let bytes = draft.bytes.into_store_bytes();
        assert!(matches!(
            decode_snapshot_document::<SceneSnapshotBodyV1>(&bytes, SnapshotDocumentKind::Starter),
            Err(ArtifactError::InvalidReference(
                ValidationError::InvalidReference {
                    field: "snapshot_document.document_kind"
                }
            ))
        ));
    }

    #[test]
    fn build_rejects_a_body_that_fails_its_own_bounds() {
        let mut body = model_body();
        body.temperature = Some(f64::NAN);
        let envelope = SnapshotEnvelopeV1 {
            document_kind: SnapshotDocumentKind::Model,
            format_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
            source: body.source(),
            source_revision: Revision::INITIAL,
            payload: body,
        };
        assert!(matches!(
            build_snapshot_draft(SnapshotArtifactId::new(), &envelope),
            Err(ArtifactError::InvalidReference(
                ValidationError::InvalidValue {
                    field: "model_document.temperature"
                }
            ))
        ));
    }

    #[test]
    fn a_v1_snapshot_draft_must_be_json() {
        let bytes = ProtectedArtifactBytes::new(b"payload".to_vec()).expect("payload");
        let draft = SnapshotArtifactDraft {
            source: SnapshotSource::Character(CharacterId::new()),
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: bytes.digest(),
            schema_version: SNAPSHOT_DOCUMENT_FORMAT_V1,
            byte_size: bytes.len() as u64,
            codec: ArtifactCodec::Cbor,
            retention: ArtifactRetention::Conversation,
            bytes,
        };
        assert!(matches!(
            draft.validate(),
            Err(ArtifactError::InvalidReference(
                ValidationError::InvalidValue {
                    field: "snapshot_artifact.codec"
                }
            ))
        ));
    }

    #[test]
    fn character_body_rejects_a_blank_name_and_a_duplicate_media_asset() {
        let mut body = character_body();
        body.name = "  ".into();
        assert_eq!(
            body.validate(),
            Err(ValidationError::Blank {
                field: "character_document.name"
            })
        );

        let mut body = character_body();
        let asset_id = AssetId::new();
        body.media = vec![
            CharacterMediaLinkV1 {
                asset_id,
                slot: CharacterMediaSlotV1::AvatarOriginal,
                ordinal: 0,
            },
            CharacterMediaLinkV1 {
                asset_id,
                slot: CharacterMediaSlotV1::DesignReference,
                ordinal: 0,
            },
        ];
        assert_eq!(
            body.validate(),
            Err(ValidationError::Duplicate {
                field: "character_document.media.asset_ids"
            })
        );
    }

    #[test]
    fn image_recommendation_requires_one_finite_bounded_target() {
        let mut body = character_body();
        body.image_recommendation = Some(ImageRecommendationV1 {
            artifact_id: Some(ModelArtifactId::new()),
            unresolved_legacy_name: Some("legacy-lora".into()),
            strength: 1.0,
        });
        assert_eq!(
            body.validate(),
            Err(ValidationError::Invariant {
                field: "character_document.image_recommendation"
            })
        );

        let mut body = persona_body();
        body.image_recommendation = Some(ImageRecommendationV1 {
            artifact_id: None,
            unresolved_legacy_name: Some("legacy-lora".into()),
            strength: f32::NAN,
        });
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidValue {
                field: "persona_document.image_recommendation"
            })
        );
    }

    #[test]
    fn starter_body_rejects_a_duplicate_ordinal() {
        let mut body = starter_body();
        body.messages[1].ordinal = 0;
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidValue {
                field: "starter_document.messages.order"
            })
        );
    }

    #[test]
    fn permuted_ordinals_do_not_satisfy_document_order() {
        let mut lorebook = lorebook_body();
        lorebook.entries[0].ordinal = 1;
        lorebook.entries[1].ordinal = 0;
        assert_eq!(
            lorebook.validate(),
            Err(ValidationError::InvalidValue {
                field: "lorebook_document.entries.order"
            })
        );

        let mut group = group_body();
        group.members[0].ordinal = 1;
        group.members[1].ordinal = 0;
        assert_eq!(
            group.validate(),
            Err(ValidationError::InvalidValue {
                field: "group_document.members.order"
            })
        );
    }

    #[test]
    fn lorebook_body_rejects_an_oversized_entry_collection() {
        let mut body = lorebook_body();
        let template = body.entries[0].clone();
        body.entries = (0..=MAX_DOCUMENT_ENTRIES)
            .map(|ordinal| LorebookEntryV1 {
                entry_id: LorebookEntryId::new(),
                ordinal: u32::try_from(ordinal).expect("ordinal fits"),
                ..template.clone()
            })
            .collect();
        assert_eq!(
            body.validate(),
            Err(ValidationError::TooMany {
                field: "lorebook_document.entries",
                max: MAX_DOCUMENT_ENTRIES
            })
        );

        let mut body = lorebook_body();
        body.entries[0].keywords = vec!["beacon".into(); MAX_ENTRY_KEYWORDS + 1];
        assert_eq!(
            body.validate(),
            Err(ValidationError::TooMany {
                field: "lorebook_document.entries.keywords",
                max: MAX_ENTRY_KEYWORDS
            })
        );
    }

    #[test]
    fn scene_body_rejects_a_dangling_part_and_an_orphan_asset_link() {
        let mut body = scene_body();
        body.content.push(ScenePartV1::InlineAsset {
            link_id: SceneAssetLinkId::new(),
        });
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidReference {
                field: "scene_document.content.inline_link_ids"
            })
        );

        let mut body = scene_body();
        body.assets.push(SceneAssetLinkV1 {
            id: SceneAssetLinkId::new(),
            asset_id: AssetId::new(),
            slot: SceneAssetSlotV1::Inline,
            ordinal: 2,
        });
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidReference {
                field: "scene_document.assets.inline.orphan"
            })
        );
    }

    #[test]
    fn group_body_rejects_a_single_member_and_an_all_muted_cast() {
        let mut body = group_body();
        body.members.truncate(1);
        assert_eq!(
            body.validate(),
            Err(ValidationError::Invariant {
                field: "group_document.members.minimum"
            })
        );

        let mut body = group_body();
        for member in &mut body.members {
            member.muted = true;
        }
        assert_eq!(
            body.validate(),
            Err(ValidationError::Invariant {
                field: "group_document.members.active"
            })
        );
    }

    #[test]
    fn prompt_body_rejects_an_undefined_purpose_and_an_overdeep_condition() {
        let mut body = prompt_body();
        body.purpose = PromptPurposeV1::Undefined;
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidValue {
                field: "prompt_document.purpose"
            })
        );

        let mut condition = PromptEntryConditionV1::HasPersona { value: true };
        for _ in 0..MAX_CONDITION_DEPTH {
            condition = PromptEntryConditionV1::Not {
                condition: Box::new(condition),
            };
        }
        let mut body = prompt_body();
        body.entries[0].condition = Some(condition);
        assert_eq!(
            body.validate(),
            Err(ValidationError::OutOfBounds {
                field: "prompt_document.entries.condition"
            })
        );
    }

    #[test]
    fn prompt_body_mirrors_built_in_key_and_interval_rules() {
        let mut body = prompt_body();
        body.entries[1].built_in_entry_key = Some("core.system".into());
        assert_eq!(
            body.validate(),
            Err(ValidationError::Duplicate {
                field: "prompt_document.entries.built_in_entry_keys"
            })
        );

        let mut body = prompt_body();
        body.entries[0].built_in_entry_key = Some(" core.system ".into());
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidValue {
                field: "prompt_document.entries.built_in_entry_key"
            })
        );

        let mut body = prompt_body();
        body.entries[1].interval_turns = None;
        assert_eq!(
            body.validate(),
            Err(ValidationError::InvalidValue {
                field: "prompt_document.entries.interval_turns"
            })
        );
    }

    #[test]
    fn model_body_rejects_blank_provider_fields_and_empty_modalities() {
        let mut body = model_body();
        body.provider.provider_kind_label = String::new();
        assert_eq!(
            body.validate(),
            Err(ValidationError::Blank {
                field: "model_document.provider.provider_kind_label"
            })
        );

        let mut body = model_body();
        body.provider.provider_revision = Revision::new(0);
        assert_eq!(body.validate(), Err(ValidationError::ZeroRevision));

        let mut body = model_body();
        body.input_modalities.clear();
        assert_eq!(
            body.validate(),
            Err(ValidationError::Blank {
                field: "model_document.input_modalities"
            })
        );

        let mut body = model_body();
        body.output_modalities = vec![ModalityV1::Text, ModalityV1::Text];
        assert_eq!(
            body.validate(),
            Err(ValidationError::Duplicate {
                field: "model_document.output_modalities"
            })
        );
    }
}
