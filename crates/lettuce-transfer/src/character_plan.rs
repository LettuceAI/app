//! The character, scenes, variants and starters a planned legacy character
//! becomes.

use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterMediaLink, CharacterMediaSlot,
    CharacterPresentationV1, ConversationStarter, CreateCharacterPlan, ImageRecommendation,
    LifecycleStatus, Scene, SceneAssetLink, SceneAssetSlot, SceneOwner, SceneVariant, Selection,
};
use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, Lorebook, LorebookBehaviorVersion, LorebookDetails,
    LorebookEntry,
};
use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, LorebookEntryId, LorebookId, ModelProfileId,
    PromptDocumentId, Revision, SceneAssetLinkId, SceneId, SceneVariantId, StarterMessageId,
    VoiceProfileId,
};

use crate::{
    LegacyBackupCharacterCandidate, LegacyKeywordMatchMode, LegacyLorebookCandidate,
    LegacyLorebookDetectionPolicy, LegacyMediaUse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CharacterPlanError {
    #[error("a character reference has no destination")]
    MissingReference,
    #[error("the character is invalid")]
    InvalidInput,
}

/// Where each planned id and media use lands.
pub trait CharacterPlanResolver {
    fn character(&self, planned: CharacterId) -> CharacterId;
    fn scene(&self, planned: SceneId) -> SceneId;
    fn variant(&self, planned: SceneVariantId) -> SceneVariantId;
    fn starter(&self, planned: ConversationStarterId) -> ConversationStarterId;
    fn starter_message(&self, planned: StarterMessageId) -> StarterMessageId;
    fn voice_profile(&self, planned: VoiceProfileId) -> VoiceProfileId;
    fn model(&self, planned: ModelProfileId) -> Result<ModelProfileId, CharacterPlanError>;
    fn prompt(&self, source_id: &str) -> Option<PromptDocumentId>;
    fn lorebook(&self, planned: LorebookId) -> Result<LorebookId, CharacterPlanError>;
    fn asset(&self, media_use: &LegacyMediaUse) -> Result<AssetId, CharacterPlanError>;
}

/// The character a planned candidate becomes, with every destination id
/// and asset from `resolver`.
pub fn character_plan_from_candidate<R: CharacterPlanResolver + ?Sized>(
    candidate: &LegacyBackupCharacterCandidate,
    resolver: &R,
) -> Result<CreateCharacterPlan, CharacterPlanError> {
    let character_id = candidate.id;
    let destination_character = resolver.character(character_id);
    let scene_id = |id: lettuce_types::SceneId| resolver.scene(id);
    let variant_id = |id: lettuce_types::SceneVariantId| resolver.variant(id);
    let asset = |media_use: LegacyMediaUse| resolver.asset(&media_use);
    let prompt = |source_id: &Option<String>| {
        source_id
            .as_deref()
            .and_then(|source_id| resolver.prompt(source_id))
    };
    let lorebook = |legacy_id: &LorebookId| resolver.lorebook(*legacy_id);
    let mut links = Vec::new();
    if candidate.media.avatar.is_some() {
        links.push(CharacterMediaLink {
            asset_id: asset(LegacyMediaUse::CharacterAvatar { character_id })?,
            slot: CharacterMediaSlot::AvatarOriginal,
            ordinal: 0,
        });
    }
    if candidate.media.background.is_some() {
        links.push(CharacterMediaLink {
            asset_id: asset(LegacyMediaUse::CharacterBackground { character_id })?,
            slot: CharacterMediaSlot::Background,
            ordinal: 0,
        });
    }
    for (ordinal, _) in candidate.media.design_references.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).map_err(|_| CharacterPlanError::InvalidInput)?;
        links.push(CharacterMediaLink {
            asset_id: asset(LegacyMediaUse::CharacterDesignReference {
                character_id,
                ordinal,
            })?,
            slot: CharacterMediaSlot::DesignReference,
            ordinal,
        });
    }
    let model_profile_id = candidate
        .defaults
        .model_profile_id
        .map(|legacy_id| resolver.model(legacy_id))
        .transpose()?;
    let mut companion_soul = candidate.defaults.companion_soul.clone();
    if let Some(soul) = &mut companion_soul {
        soul.prompting.prompt_template_id = prompt(&candidate.defaults.companion_prompt_source_id);
    }
    let presentation = &candidate.presentation;
    let character = Character {
        id: destination_character,
        status: LifecycleStatus::Active,
        profile: candidate.profile.clone(),
        provenance: candidate.provenance.clone(),
        defaults: CharacterDefaults {
            interaction_mode: candidate.defaults.interaction_mode,
            memory_policy: candidate.defaults.memory_policy,
            model_profile_id,
            default_scene_id: candidate.defaults.default_scene_id.map(scene_id),
            default_starter_id: candidate
                .defaults
                .default_starter_source_id
                .as_ref()
                .and_then(|source_id| {
                    candidate
                        .starters
                        .iter()
                        .find(|starter| &starter.source_id == source_id)
                })
                .map(|starter| resolver.starter(starter.id)),
            direct_prompt_id: prompt(&candidate.defaults.direct_prompt_source_id),
            group_conversation_prompt_id: prompt(
                &candidate.defaults.group_conversation_prompt_source_id,
            ),
            group_roleplay_prompt_id: prompt(&candidate.defaults.group_roleplay_prompt_source_id),
            voice: candidate.defaults.voice.clone().map(|voice| match voice {
                lettuce_characters::VoicePreference::VoiceProfile(id) => {
                    lettuce_characters::VoicePreference::VoiceProfile(resolver.voice_profile(id))
                }
                unresolved => unresolved,
            }),
            voice_autoplay: candidate.defaults.voice_autoplay,
            companion_soul,
        },
        presentation: CharacterPresentationV1 {
            format_version: 1,
            card_style: presentation.card_style,
            avatar_crop: presentation.avatar_crop,
            banner_crop: presentation.banner_crop,
            disable_gradient: presentation.disable_gradient,
            gradient_source: presentation.gradient_source,
            custom_gradient_enabled: presentation.custom_gradient_enabled,
            custom_gradient_colors: presentation.custom_gradient_colors.clone(),
            primary_text_color: presentation.primary_text_color.clone(),
            secondary_text_color: presentation.secondary_text_color.clone(),
            chat_appearance: presentation.chat_appearance.clone(),
        },
        image_recommendation: candidate.image_recommendation.as_ref().map(|value| {
            ImageRecommendation {
                artifact_id: None,
                unresolved_legacy_name: Some(value.model_name.clone()),
                strength: value.strength as f32,
            }
        }),
        media: CharacterMedia { links },
        revision: Revision::INITIAL,
        created_at: candidate.created_at,
        updated_at: candidate.updated_at,
    };
    let mut scenes = Vec::with_capacity(candidate.scenes.len());
    let mut variants = Vec::new();
    for scene in &candidate.scenes {
        let mut assets = Vec::new();
        if scene.background.is_some() {
            assets.push(SceneAssetLink {
                id: SceneAssetLinkId::from_uuid(uuid::Uuid::new_v5(
                    &scene_id(scene.id).as_uuid(),
                    b"legacy-background",
                )),
                asset_id: asset(LegacyMediaUse::CharacterSceneBackground {
                    character_id,
                    scene_id: scene.id,
                })?,
                slot: SceneAssetSlot::Background,
                ordinal: 0,
            });
        }
        scenes.push(Scene {
            id: scene_id(scene.id),
            owner: SceneOwner::Character(destination_character),
            status: LifecycleStatus::Active,
            ordinal: scene.ordinal,
            content: scene.content.clone(),
            direction: scene.direction.clone(),
            selected_variant_id: scene.selected_variant_id.map(variant_id),
            assets,
            revision: Revision::INITIAL,
            created_at: scene.created_at,
            updated_at: scene.created_at,
        });
        variants.extend(scene.variants.iter().map(|variant| SceneVariant {
            id: variant_id(variant.id),
            scene_id: scene_id(scene.id),
            ordinal: variant.ordinal,
            content: variant.content.clone(),
            direction: variant.direction.clone(),
            revision: Revision::INITIAL,
            created_at: variant.created_at,
            updated_at: variant.created_at,
        }));
    }
    let starters = candidate
        .starters
        .iter()
        .map(|starter| {
            Ok(ConversationStarter {
                id: resolver.starter(starter.id),
                character_id: destination_character,
                name: starter.name.clone(),
                ordinal: starter.ordinal,
                messages: starter
                    .messages
                    .iter()
                    .map(|message| lettuce_characters::StarterMessage {
                        id: resolver.starter_message(message.id),
                        ..message.clone()
                    })
                    .collect(),
                scene_id: starter.scene_id.map(scene_id),
                prompt_id: prompt(&starter.prompt_source_id),
                lorebooks: match &starter.lorebook_ids {
                    None => Selection::Inherit,
                    Some(ids) => Selection::Explicit(
                        ids.iter().map(lorebook).collect::<Result<Vec<_>, _>>()?,
                    ),
                },
                revision: Revision::INITIAL,
                created_at: starter.created_at,
                updated_at: starter.created_at,
            })
        })
        .collect::<Result<Vec<_>, CharacterPlanError>>()?;
    Ok(CreateCharacterPlan {
        character,
        scenes,
        variants,
        starters,
    })
}

/// The lorebook a planned candidate becomes under `id`, its entries renamed
/// by `entry_id` and ordered as planned.
pub fn lorebook_details_from_candidate(
    candidate: &LegacyLorebookCandidate,
    id: LorebookId,
    icon_asset_id: Option<AssetId>,
    entry_id: impl Fn(LorebookEntryId) -> Option<LorebookEntryId>,
) -> Result<LorebookDetails, CharacterPlanError> {
    let entries = candidate
        .entries
        .iter()
        .enumerate()
        .map(|(ordinal, entry)| {
            Ok(LorebookEntry {
                id: entry_id(entry.id).ok_or(CharacterPlanError::MissingReference)?,
                lorebook_id: id,
                title: entry.title.clone(),
                enabled: entry.enabled,
                always_active: entry.always_active,
                keywords: entry.keywords.clone(),
                case_sensitive: entry.case_sensitive,
                match_mode: match entry.match_mode {
                    LegacyKeywordMatchMode::Literal => KeywordMatchMode::Literal,
                    LegacyKeywordMatchMode::Regex => KeywordMatchMode::Regex,
                },
                content: entry.content.clone(),
                priority: entry.priority,
                ordinal: u32::try_from(ordinal).map_err(|_| CharacterPlanError::InvalidInput)?,
                revision: Revision::INITIAL,
                created_at: entry.created_at,
                updated_at: entry.updated_at,
            })
        })
        .collect::<Result<Vec<_>, CharacterPlanError>>()?;
    Ok(LorebookDetails {
        book: Lorebook {
            id,
            status: lettuce_context::LifecycleStatus::Active,
            name: candidate.name.clone(),
            detection_policy: match candidate.detection_policy {
                LegacyLorebookDetectionPolicy::RecentMessageWindow => {
                    DetectionPolicy::RecentMessageWindow
                }
                LegacyLorebookDetectionPolicy::LatestUserMessage => {
                    DetectionPolicy::LatestUserMessage
                }
            },
            icon_asset_id,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
            revision: Revision::INITIAL,
            created_at: candidate.created_at,
            updated_at: candidate.updated_at,
        },
        entries,
    })
}
