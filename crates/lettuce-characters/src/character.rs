use std::collections::{BTreeMap, HashSet};

use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, ModelArtifactId, ModelProfileId, PromptDocumentId,
    Revision, SceneId, TimestampMillis, VoiceProfileId,
};
use serde::{Deserialize, Serialize};

use crate::constants::{
    MAX_COLLECTION_ITEMS, MAX_TAGS_OR_SOURCES, validate_collection, validate_name,
    validate_non_blank, validate_scalar_limit, validate_text,
};
use crate::presentation::CharacterPresentationV1;
use crate::{InteractionMode, LifecycleStatus, MemoryPolicy, ValidationError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterProfile {
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub design_description: Option<String>,
}

impl CharacterProfile {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name("character.name", &self.name)?;
        for value in [
            self.nickname.as_ref(),
            self.description.as_ref(),
            self.definition.as_ref(),
            self.design_description.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_text("character.profile", value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterProvenance {
    pub creator: Option<String>,
    pub creator_notes: Option<String>,
    pub localized_creator_notes: BTreeMap<String, String>,
    pub sources: Vec<String>,
    pub tags: Vec<String>,
}

impl CharacterProvenance {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for value in [self.creator.as_ref(), self.creator_notes.as_ref()]
            .into_iter()
            .flatten()
        {
            validate_text("character.provenance", value)?;
        }
        for (locale, notes) in &self.localized_creator_notes {
            validate_non_blank("character.provenance.locale", locale)?;
            validate_scalar_limit("character.provenance.locale", locale, 64)?;
            validate_text("character.provenance.localized_creator_notes", notes)?;
        }
        validate_collection("character.sources", &self.sources, MAX_TAGS_OR_SOURCES)?;
        validate_collection("character.tags", &self.tags, MAX_TAGS_OR_SOURCES)?;
        for source in &self.sources {
            validate_non_blank("character.source", source)?;
            validate_scalar_limit("character.source", source, 1024)?;
            if source.len() > 1024 {
                return Err(ValidationError::TooLarge {
                    field: "character.source",
                });
            }
        }
        for tag in &self.tags {
            validate_non_blank("character.tag", tag)?;
            validate_scalar_limit("character.tag", tag, 1024)?;
            if tag.len() > 1024 {
                return Err(ValidationError::TooLarge {
                    field: "character.tag",
                });
            }
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
pub enum VoicePreference {
    VoiceProfile(VoiceProfileId),
    UnresolvedLegacy(LegacyVoiceLocatorV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyVoiceLocatorV1 {
    pub locator: String,
}

impl LegacyVoiceLocatorV1 {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_non_blank("voice.locator", &self.locator)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRecommendation {
    pub artifact_id: Option<ModelArtifactId>,
    pub unresolved_legacy_name: Option<String>,
    pub strength: f32,
}

impl ImageRecommendation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.artifact_id.is_some() == self.unresolved_legacy_name.is_some() {
            return Err(ValidationError::Invariant {
                field: "image_recommendation.target",
            });
        }
        if !self.strength.is_finite() || !(0.0..=2.0).contains(&self.strength) {
            return Err(ValidationError::InvalidValue {
                field: "image_recommendation.strength",
            });
        }
        if let Some(name) = &self.unresolved_legacy_name {
            validate_non_blank("image_recommendation.legacy_name", name)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDefaults {
    pub interaction_mode: InteractionMode,
    pub memory_policy: MemoryPolicy,
    pub model_profile_id: Option<ModelProfileId>,
    pub default_scene_id: Option<SceneId>,
    pub default_starter_id: Option<ConversationStarterId>,
    pub direct_prompt_id: Option<PromptDocumentId>,
    pub group_conversation_prompt_id: Option<PromptDocumentId>,
    pub group_roleplay_prompt_id: Option<PromptDocumentId>,
    pub voice: Option<VoicePreference>,
    pub voice_autoplay: bool,
}

impl Default for CharacterDefaults {
    fn default() -> Self {
        Self {
            interaction_mode: InteractionMode::Roleplay,
            memory_policy: MemoryPolicy::Manual,
            model_profile_id: None,
            default_scene_id: None,
            default_starter_id: None,
            direct_prompt_id: None,
            group_conversation_prompt_id: None,
            group_roleplay_prompt_id: None,
            voice: None,
            voice_autoplay: false,
        }
    }
}

impl CharacterDefaults {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(VoicePreference::UnresolvedLegacy(locator)) = &self.voice {
            locator.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterMediaSlot {
    AvatarOriginal,
    Background,
    DesignReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterMediaLink {
    pub asset_id: AssetId,
    pub slot: CharacterMediaSlot,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMedia {
    pub links: Vec<CharacterMediaLink>,
}

impl CharacterMedia {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_collection("character.media", &self.links, MAX_COLLECTION_ITEMS)?;
        let mut avatars = 0;
        let mut backgrounds = 0;
        let mut design = Vec::new();
        for link in &self.links {
            match link.slot {
                CharacterMediaSlot::AvatarOriginal => avatars += 1,
                CharacterMediaSlot::Background => backgrounds += 1,
                CharacterMediaSlot::DesignReference => design.push(link),
            }
        }
        if avatars > 1 || backgrounds > 1 {
            return Err(ValidationError::Invariant {
                field: "character.media.single_slot",
            });
        }
        let mut design_asset_ids = HashSet::new();
        if design
            .iter()
            .any(|link| !design_asset_ids.insert(link.asset_id))
        {
            return Err(ValidationError::Duplicate {
                field: "character.media.design_reference.asset_ids",
            });
        }
        let mut ordinals: Vec<_> = design.iter().map(|link| link.ordinal).collect();
        ordinals.sort_unstable();
        crate::constants::validate_contiguous("character.media.design_reference.order", ordinals)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: CharacterId,
    pub status: LifecycleStatus,
    pub profile: CharacterProfile,
    pub provenance: CharacterProvenance,
    pub defaults: CharacterDefaults,
    pub presentation: CharacterPresentationV1,
    pub image_recommendation: Option<ImageRecommendation>,
    pub media: CharacterMedia,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl Character {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: CharacterId,
        profile: CharacterProfile,
        provenance: CharacterProvenance,
        defaults: CharacterDefaults,
        presentation: CharacterPresentationV1,
        image_recommendation: Option<ImageRecommendation>,
        media: CharacterMedia,
        created_at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        let character = Self {
            id,
            status: LifecycleStatus::Active,
            profile,
            provenance,
            defaults,
            presentation,
            image_recommendation,
            media,
            revision: Revision::INITIAL,
            created_at,
            updated_at: created_at,
        };
        character.validate()?;
        Ok(character)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        self.profile.validate()?;
        self.provenance.validate()?;
        self.defaults.validate()?;
        self.presentation.validate()?;
        self.media.validate()?;
        if let Some(recommendation) = &self.image_recommendation {
            recommendation.validate()?;
        }
        Ok(())
    }

    pub fn bump_revision(&mut self, now: TimestampMillis) -> Result<(), ValidationError> {
        self.revision = self.revision.next()?;
        self.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Character, CharacterDefaults, CharacterMedia, CharacterMediaLink, CharacterMediaSlot,
        CharacterProfile, CharacterProvenance, ImageRecommendation,
    };
    use crate::presentation::CharacterPresentationV1;
    use lettuce_types::{AssetId, CharacterId, ModelArtifactId, TimestampMillis};

    fn profile() -> CharacterProfile {
        CharacterProfile {
            name: "Ada".into(),
            nickname: None,
            description: Some("A meticulous engineer".into()),
            definition: None,
            design_description: None,
        }
    }

    #[test]
    fn image_recommendation_requires_exactly_one_target() {
        let mut recommendation = ImageRecommendation {
            artifact_id: Some(ModelArtifactId::new()),
            unresolved_legacy_name: None,
            strength: 1.0,
        };
        assert!(recommendation.validate().is_ok());
        recommendation.artifact_id = None;
        assert!(recommendation.validate().is_err());
        recommendation.unresolved_legacy_name = Some("legacy-lora".into());
        assert!(recommendation.validate().is_ok());
        recommendation.strength = 2.1;
        assert!(recommendation.validate().is_err());
    }

    #[test]
    fn character_round_trip_preserves_provenance_and_presentation() {
        let character = Character::new(
            CharacterId::new(),
            profile(),
            CharacterProvenance {
                creator: Some("source".into()),
                tags: vec!["engineer".into()],
                ..CharacterProvenance::default()
            },
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(7),
        )
        .expect("fixture character should validate");
        let encoded = serde_json::to_string(&character).expect("character should serialize");
        let decoded: Character = serde_json::from_str(&encoded).expect("character should decode");
        assert_eq!(decoded, character);
    }

    #[test]
    fn design_references_are_unique_assets_in_contiguous_order() {
        let asset_id = AssetId::new();
        let media = CharacterMedia {
            links: vec![
                CharacterMediaLink {
                    asset_id,
                    slot: CharacterMediaSlot::DesignReference,
                    ordinal: 0,
                },
                CharacterMediaLink {
                    asset_id,
                    slot: CharacterMediaSlot::DesignReference,
                    ordinal: 1,
                },
            ],
        };
        assert!(media.validate().is_err());
    }
}
