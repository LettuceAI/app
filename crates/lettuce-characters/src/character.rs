use std::collections::{BTreeMap, HashSet};

use lettuce_companions::{CompanionSoulConfig, initial_soul_state};
use lettuce_types::{
    AssetId, AudioProviderId, CharacterId, ConversationStarterId, ModelArtifactId, ModelProfileId,
    PromptDocumentId, Revision, SceneId, TimestampMillis, VoiceProfileId,
};
use serde::{Deserialize, Serialize};

use crate::constants::{
    validate_name, validate_non_blank, validate_revision_timestamps, validate_scalar_limit,
    validate_text,
};
use crate::presentation::CharacterPresentationV1;
use crate::{InteractionMode, LifecycleStatus, MemoryPolicy, ValidationError};

/// The longest locale key a localized creator note may use.
pub const MAX_LOCALE_SCALARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterProfile {
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub design_description: Option<String>,
    #[serde(default)]
    pub scenario: Option<String>,
}

impl CharacterProfile {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name("character.name", &self.name)?;
        for value in [
            self.nickname.as_ref(),
            self.description.as_ref(),
            self.definition.as_ref(),
            self.design_description.as_ref(),
            self.scenario.as_ref(),
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
            validate_scalar_limit("character.provenance.locale", locale, MAX_LOCALE_SCALARS)?;
            validate_text("character.provenance.localized_creator_notes", notes)?;
        }
        for source in &self.sources {
            validate_non_blank("character.source", source)?;
            validate_text("character.source", source)?;
        }
        for tag in &self.tags {
            validate_non_blank("character.tag", tag)?;
            validate_text("character.tag", tag)?;
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
    Provider {
        provider_id: AudioProviderId,
        voice_id: String,
        model_id: Option<String>,
        voice_name: Option<String>,
    },
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CharacterDefaultsRecord")]
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
    #[serde(default)]
    pub companion_soul: Option<CompanionSoulConfig>,
    /// Whether new direct conversations with this character start time aware.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub time_awareness: bool,
}

/// The stored shape of [`CharacterDefaults`]. A companion soul written
/// before time awareness became a character setting carries `timeAwareness`
/// inside the soul; decoding lifts it into the character setting.
/// The stored document is JSON, which this record relies on; it is not a
/// bincode type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterDefaultsRecord {
    interaction_mode: InteractionMode,
    memory_policy: MemoryPolicy,
    model_profile_id: Option<ModelProfileId>,
    default_scene_id: Option<SceneId>,
    default_starter_id: Option<ConversationStarterId>,
    direct_prompt_id: Option<PromptDocumentId>,
    group_conversation_prompt_id: Option<PromptDocumentId>,
    group_roleplay_prompt_id: Option<PromptDocumentId>,
    voice: Option<VoicePreference>,
    voice_autoplay: bool,
    #[serde(default)]
    companion_soul: Option<serde_json::Value>,
    #[serde(default)]
    time_awareness: bool,
}

impl TryFrom<CharacterDefaultsRecord> for CharacterDefaults {
    type Error = String;

    fn try_from(record: CharacterDefaultsRecord) -> Result<Self, Self::Error> {
        let mut time_awareness = record.time_awareness;
        let companion_soul = match record.companion_soul {
            Some(serde_json::Value::Object(mut soul)) => {
                if let Some(serde_json::Value::Bool(lifted)) = soul.get("timeAwareness") {
                    time_awareness |= *lifted;
                    soul.remove("timeAwareness");
                }
                Some(
                    serde_json::from_value::<CompanionSoulConfig>(serde_json::Value::Object(soul))
                        .map_err(|error| error.to_string())?,
                )
            }
            Some(other) => Some(
                serde_json::from_value::<CompanionSoulConfig>(other)
                    .map_err(|error| error.to_string())?,
            ),
            None => None,
        };
        Ok(Self {
            interaction_mode: record.interaction_mode,
            memory_policy: record.memory_policy,
            model_profile_id: record.model_profile_id,
            default_scene_id: record.default_scene_id,
            default_starter_id: record.default_starter_id,
            direct_prompt_id: record.direct_prompt_id,
            group_conversation_prompt_id: record.group_conversation_prompt_id,
            group_roleplay_prompt_id: record.group_roleplay_prompt_id,
            voice: record.voice,
            voice_autoplay: record.voice_autoplay,
            companion_soul,
            time_awareness,
        })
    }
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
            companion_soul: None,
            time_awareness: false,
        }
    }
}

impl CharacterDefaults {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(VoicePreference::Provider {
            voice_id,
            model_id,
            voice_name,
            ..
        }) = &self.voice
        {
            validate_non_blank("voice.voice_id", voice_id)?;
            if let Some(model) = model_id {
                validate_non_blank("voice.model_id", model)?;
            }
            if let Some(name) = voice_name {
                validate_text("voice.voice_name", name)?;
            }
        }
        if let Some(VoicePreference::UnresolvedLegacy(locator)) = &self.voice {
            locator.validate()?;
        }
        if let Some(config) = &self.companion_soul {
            for value in config.soul.values() {
                validate_text("character.companion_soul.identity", value)?;
            }
            if !config.soul.baseline_affect.is_unit() {
                return Err(ValidationError::InvalidValue {
                    field: "character.companion_soul.baseline_affect",
                });
            }
            if !config.soul.regulation_style.is_unit() {
                return Err(ValidationError::InvalidValue {
                    field: "character.companion_soul.regulation_style",
                });
            }
            if !config.relationship_defaults.is_valid() {
                return Err(ValidationError::InvalidValue {
                    field: "character.companion_soul.relationship_defaults",
                });
            }
            initial_soul_state(Some(config), TimestampMillis::new(1)).map_err(|_| {
                ValidationError::Invariant {
                    field: "character.companion_soul.authored_facts",
                }
            })?;
            validate_text(
                "character.companion_soul.prompting.style_notes",
                &config.prompting.style_notes,
            )?;
        }
        if self.interaction_mode != InteractionMode::Companion && self.companion_soul.is_some() {
            return Err(ValidationError::Invariant {
                field: "character.companion_soul.mode",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterMediaSlot {
    AvatarOriginal,
    Banner,
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
        let mut avatars = 0;
        let mut banners = 0;
        let mut backgrounds = 0;
        let mut design = Vec::new();
        for link in &self.links {
            match link.slot {
                CharacterMediaSlot::AvatarOriginal => avatars += 1,
                CharacterMediaSlot::Banner => banners += 1,
                CharacterMediaSlot::Background => backgrounds += 1,
                CharacterMediaSlot::DesignReference => design.push(link),
            }
        }
        if avatars > 1 || banners > 1 || backgrounds > 1 {
            return Err(ValidationError::Invariant {
                field: "character.media.single_slot",
            });
        }
        let mut design_asset_ids = HashSet::new();
        let mut all_asset_ids = HashSet::new();
        for link in &self.links {
            if !all_asset_ids.insert(link.asset_id) {
                return Err(ValidationError::Duplicate {
                    field: "character.media.asset_ids",
                });
            }
        }
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
        validate_revision_timestamps(
            "character.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
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
    use crate::InteractionMode;
    use crate::ValidationError;
    use crate::presentation::CharacterPresentationV1;
    use lettuce_companions::CompanionSoulConfig;
    use lettuce_types::{AssetId, CharacterId, ModelArtifactId, Revision, TimestampMillis};

    fn profile() -> CharacterProfile {
        CharacterProfile {
            name: "Ada".into(),
            nickname: None,
            description: Some("A meticulous engineer".into()),
            definition: None,
            design_description: None,
            scenario: None,
        }
    }

    #[test]
    fn tags_sources_and_gradient_colors_have_no_count_limit_like_legacy() {
        let provenance = CharacterProvenance {
            tags: (0..300).map(|index| format!("tag {index}")).collect(),
            sources: (0..300)
                .map(|index| format!("https://source/{index}"))
                .collect(),
            ..CharacterProvenance::default()
        };
        assert_eq!(provenance.validate(), Ok(()));
        let presentation = CharacterPresentationV1 {
            custom_gradient_colors: vec!["#abcdef".into(); 300],
            ..CharacterPresentationV1::default()
        };
        assert_eq!(presentation.validate(), Ok(()));
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
    fn banner_media_round_trips_and_rejects_multiple_banners() {
        let slot: CharacterMediaSlot = serde_json::from_str("\"banner\"").expect("banner slot");
        let mut media = CharacterMedia {
            links: vec![CharacterMediaLink { asset_id: AssetId::new(), slot, ordinal: 0 }],
        };
        media.validate().expect("single banner");
        let encoded = serde_json::to_string(&media).expect("encode");
        assert_eq!(serde_json::from_str::<CharacterMedia>(&encoded).expect("decode"), media);
        media.links.push(CharacterMediaLink { asset_id: AssetId::new(), slot, ordinal: 1 });
        assert!(media.validate().is_err());
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

    #[test]
    fn media_asset_ids_are_unique_across_all_slots() {
        let asset_id = AssetId::new();
        let media = CharacterMedia {
            links: vec![
                CharacterMediaLink {
                    asset_id,
                    slot: CharacterMediaSlot::AvatarOriginal,
                    ordinal: 0,
                },
                CharacterMediaLink {
                    asset_id,
                    slot: CharacterMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        };
        assert!(media.validate().is_err());
    }

    #[test]
    fn authored_root_rejects_zero_revision_and_reversed_timestamps() {
        let mut character = Character::new(
            CharacterId::new(),
            profile(),
            CharacterProvenance::default(),
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(10),
        )
        .expect("valid character");
        character.revision = Revision::new(0);
        assert_eq!(character.validate(), Err(ValidationError::ZeroRevision));
        character.revision = Revision::INITIAL;
        character.updated_at = TimestampMillis::new(9);
        assert!(matches!(
            character.validate(),
            Err(ValidationError::InvalidTimestampOrder {
                field: "character.timestamps"
            })
        ));
    }

    fn defaults_json(
        soul: Option<serde_json::Value>,
        time_awareness: Option<bool>,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(CharacterDefaults {
            interaction_mode: InteractionMode::Companion,
            ..CharacterDefaults::default()
        })
        .expect("encode defaults");
        let object = value.as_object_mut().expect("defaults object");
        if let Some(soul) = soul {
            object.insert("companion_soul".into(), soul);
        }
        if let Some(flag) = time_awareness {
            object.insert("time_awareness".into(), flag.into());
        }
        value
    }

    #[test]
    fn time_awareness_is_a_character_default_and_off_is_not_written() {
        let off = serde_json::to_value(CharacterDefaults::default()).expect("encode off");
        assert!(off.get("time_awareness").is_none());
        let on = CharacterDefaults {
            time_awareness: true,
            ..CharacterDefaults::default()
        };
        let encoded = serde_json::to_value(&on).expect("encode on");
        assert_eq!(encoded["time_awareness"], serde_json::json!(true));
        let decoded: CharacterDefaults = serde_json::from_value(encoded).expect("decode on");
        assert!(decoded.time_awareness);
        let old: CharacterDefaults =
            serde_json::from_value(defaults_json(None, None)).expect("decode without the key");
        assert!(!old.time_awareness);
    }

    #[test]
    fn an_old_companion_soul_time_awareness_lifts_into_the_character_default() {
        let lifted: CharacterDefaults = serde_json::from_value(defaults_json(
            Some(serde_json::json!({"timeAwareness": true})),
            None,
        ))
        .expect("decode old row");
        assert!(lifted.time_awareness);
        assert_eq!(lifted.companion_soul, Some(CompanionSoulConfig::default()));
        let reencoded = serde_json::to_value(&lifted).expect("encode lifted");
        assert!(reencoded["companion_soul"].get("timeAwareness").is_none());
        assert_eq!(reencoded["time_awareness"], serde_json::json!(true));

        let off: CharacterDefaults = serde_json::from_value(defaults_json(
            Some(serde_json::json!({"shareMemoryAcrossChats": false})),
            None,
        ))
        .expect("decode old row without the key");
        assert!(!off.time_awareness);
        assert!(
            !off.companion_soul
                .expect("companion soul")
                .share_memory_across_chats
        );

        let both: CharacterDefaults = serde_json::from_value(defaults_json(
            Some(serde_json::json!({"timeAwareness": false})),
            Some(true),
        ))
        .expect("decode mixed row");
        assert!(both.time_awareness);
    }

    #[test]
    fn unknown_companion_soul_keys_are_still_rejected() {
        let rejected = serde_json::from_value::<CharacterDefaults>(defaults_json(
            Some(serde_json::json!({"timeAwareness": "yes"})),
            None,
        ));
        assert!(rejected.is_err());
        let rejected = serde_json::from_value::<CharacterDefaults>(defaults_json(
            Some(serde_json::json!({"unexpected": true})),
            None,
        ));
        assert!(rejected.is_err());
    }

    #[test]
    fn companion_soul_config_requires_companion_mode() {
        let mut defaults = CharacterDefaults {
            companion_soul: Some(CompanionSoulConfig::default()),
            ..CharacterDefaults::default()
        };
        assert!(defaults.validate().is_err());
        defaults.interaction_mode = InteractionMode::Companion;
        assert!(defaults.validate().is_ok());
    }

    #[test]
    fn companion_soul_config_rejects_invalid_runtime_math_inputs() {
        let mut defaults = CharacterDefaults {
            interaction_mode: InteractionMode::Companion,
            companion_soul: Some(CompanionSoulConfig::default()),
            ..CharacterDefaults::default()
        };
        defaults
            .companion_soul
            .as_mut()
            .expect("companion config")
            .soul
            .baseline_affect
            .warmth = f64::NAN;
        assert!(matches!(
            defaults.validate(),
            Err(ValidationError::InvalidValue {
                field: "character.companion_soul.baseline_affect"
            })
        ));

        let config = defaults.companion_soul.as_mut().expect("companion config");
        config.soul.baseline_affect.warmth = 0.5;
        config.soul.regulation_style.volatility = 1.1;
        assert!(matches!(
            defaults.validate(),
            Err(ValidationError::InvalidValue {
                field: "character.companion_soul.regulation_style"
            })
        ));

        let config = defaults.companion_soul.as_mut().expect("companion config");
        config.soul.regulation_style.volatility = 0.25;
        config.relationship_defaults.trust = -1.1;
        assert!(matches!(
            defaults.validate(),
            Err(ValidationError::InvalidValue {
                field: "character.companion_soul.relationship_defaults"
            })
        ));
    }
}
