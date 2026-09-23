//! A stored character written back as the old app's character package, the
//! form every character file format is built from.

use std::collections::BTreeMap;

use lettuce_characters::{
    CharacterDetails, InteractionMode, LifecycleStatus, MemoryPolicy, SceneDocumentV1, ScenePart,
    StarterRole, VoicePreference,
};
use lettuce_companions::{CompanionScheduledNote, CompanionSoulConfig, ScheduledNoteRecurrence};
use lettuce_context::{DetectionPolicy, KeywordMatchMode, LorebookDetails};
use lettuce_types::SceneId;
use serde_json::{Map, Value, json};

use crate::{
    CharacterPackage, CharacterPackageData, ChatTemplateMessagePackage, ChatTemplatePackage,
    CompanionScheduledNotePackage, LorebookPackage, PackageCrop, PackagedKeywordDetectionMode,
    PackagedKeywordMatchMode, PackagedLorebook, PackagedLorebookEntry, ScenePackage,
    SceneVariantPackage, strip_legacy_card_prompt_sections,
};

/// A stored character with its enabled lorebooks in binding order and its
/// scheduled notes.
#[derive(Debug, Clone)]
pub struct CharacterExportRecord {
    pub details: CharacterDetails,
    pub lorebooks: Vec<LorebookDetails>,
    pub scheduled_notes: Vec<CompanionScheduledNote>,
    pub companion_memory: Option<crate::CharacterFileCompanionMemory>,
}

/// A stored character with what its file carries alongside it.
#[derive(Debug, Clone)]
pub struct CharacterExportSource {
    pub details: CharacterDetails,
    /// The character's enabled lorebooks in binding order.
    pub lorebooks: Vec<LorebookDetails>,
    pub scheduled_notes: Vec<CompanionScheduledNote>,
    pub companion_memory: Option<crate::CharacterFileCompanionMemory>,
    pub avatar_data: Option<String>,
    pub background_image_data: Option<String>,
    pub scene_backgrounds: BTreeMap<SceneId, String>,
}

/// The package the old app exported for this character.
#[must_use]
pub fn character_package(source: &CharacterExportSource, exported_at: i64) -> CharacterPackage {
    let character = &source.details.character;
    let profile = &character.profile;
    let provenance = &character.provenance;
    let defaults = &character.defaults;
    let presentation = &character.presentation;
    let mut scenes = source
        .details
        .scenes
        .iter()
        .filter(|scene| scene.status == LifecycleStatus::Active)
        .collect::<Vec<_>>();
    scenes.sort_by_key(|scene| scene.ordinal);
    let scenes: Vec<ScenePackage> = scenes
        .into_iter()
        .map(|scene| {
            let mut variants = source
                .details
                .variants
                .iter()
                .filter(|variant| variant.scene_id == scene.id)
                .collect::<Vec<_>>();
            variants.sort_by_key(|variant| variant.ordinal);
            ScenePackage {
                id: scene.id.to_string(),
                content: document_text(&scene.content),
                direction: scene.direction.clone(),
                background_image_path: source.scene_backgrounds.get(&scene.id).cloned(),
                created_at: Some(scene.created_at.get()),
                selected_variant_id: scene.selected_variant_id.map(|id| id.to_string()),
                variants: variants
                    .into_iter()
                    .map(|variant| SceneVariantPackage {
                        id: variant.id.to_string(),
                        content: document_text(&variant.content),
                        direction: variant.direction.clone(),
                        created_at: Some(variant.created_at.get()),
                    })
                    .collect(),
            }
        })
        .collect();
    let mut starters = source.details.starters.iter().collect::<Vec<_>>();
    starters.sort_by_key(|starter| starter.ordinal);
    let chat_templates = starters
        .into_iter()
        .map(|starter| ChatTemplatePackage {
            id: starter.id.to_string(),
            name: starter.name.clone(),
            messages: starter
                .messages
                .iter()
                .map(|message| ChatTemplateMessagePackage {
                    id: message.id.to_string(),
                    role: match message.role {
                        StarterRole::User => "user",
                        StarterRole::Assistant => "assistant",
                    }
                    .to_owned(),
                    content: message.content.clone(),
                })
                .collect(),
            created_at: Some(starter.created_at.get()),
        })
        .collect();
    let crop = |crop: Option<lettuce_characters::Crop>| {
        let decimal = |value: f32| value.to_string().parse().unwrap_or(f64::from(value));
        crop.map(|crop| PackageCrop {
            x: decimal(crop.x),
            y: decimal(crop.y),
            scale: decimal(crop.scale),
        })
    };
    let non_empty = |values: &Vec<String>| (!values.is_empty()).then(|| values.clone());
    let default_scene_id = defaults
        .default_scene_id
        .map(|id| id.to_string())
        .filter(|id| scenes.iter().any(|scene: &ScenePackage| &scene.id == id));
    CharacterPackage {
        version: 1,
        exported_at,
        character: CharacterPackageData {
            name: profile.name.clone(),
            description: profile.description.clone(),
            definition: profile
                .definition
                .as_deref()
                .map(strip_legacy_card_prompt_sections)
                .filter(|definition| !definition.is_empty())
                .or_else(|| profile.description.clone()),
            scenario: profile.scenario.clone(),
            nickname: profile.nickname.clone(),
            creator: provenance.creator.clone(),
            creator_notes: provenance.creator_notes.clone(),
            creator_notes_multilingual: (!provenance.localized_creator_notes.is_empty())
                .then(|| json!(provenance.localized_creator_notes)),
            source: non_empty(&provenance.sources),
            tags: non_empty(&provenance.tags),
            character_book: None,
            rules: profile.rules.clone(),
            scenes,
            default_scene_id,
            default_model_id: defaults.model_profile_id.map(|id| id.to_string()),
            mode: Some(
                match defaults.interaction_mode {
                    InteractionMode::Roleplay => "roleplay",
                    InteractionMode::Companion => "companion",
                }
                .to_owned(),
            ),
            companion: defaults.companion_soul.as_ref().map(legacy_companion_value),
            companion_scheduled_notes: source
                .scheduled_notes
                .iter()
                .map(scheduled_note_package)
                .collect(),
            companion_shared_memory: source.companion_memory.as_ref().map(shared_memory_package),
            memory_type: Some(
                match defaults.memory_policy {
                    MemoryPolicy::Manual => "manual",
                    MemoryPolicy::Dynamic => "dynamic",
                }
                .to_owned(),
            ),
            active_lorebook_ids: source
                .lorebooks
                .iter()
                .map(|lorebook| lorebook.book.id.to_string())
                .collect(),
            lorebooks: source.lorebooks.iter().map(lorebook_package).collect(),
            prompt_template_id: defaults.direct_prompt_id.map(|id| id.to_string()),
            system_prompt: None,
            voice_config: defaults.voice.as_ref().map(|voice| match voice {
                VoicePreference::VoiceProfile(id) => {
                    json!({"source": "user", "userVoiceId": id.to_string()})
                }
                VoicePreference::UnresolvedLegacy(locator) => {
                    serde_json::from_str(&locator.locator)
                        .unwrap_or_else(|_| Value::String(locator.locator.clone()))
                }
            }),
            voice_autoplay: Some(defaults.voice_autoplay),
            disable_avatar_gradient: presentation.disable_gradient,
            avatar_crop: crop(presentation.avatar_crop),
            banner_crop: crop(presentation.banner_crop),
            custom_gradient_enabled: Some(presentation.custom_gradient_enabled),
            custom_gradient_colors: non_empty(&presentation.custom_gradient_colors),
            custom_text_color: presentation.primary_text_color.clone(),
            custom_text_secondary: presentation.secondary_text_color.clone(),
            chat_templates,
            default_chat_template_id: defaults.default_starter_id.map(|id| id.to_string()),
        },
        avatar_data: source.avatar_data.clone(),
        background_image_data: source.background_image_data.clone(),
    }
}

fn document_text(document: &SceneDocumentV1) -> String {
    document
        .parts
        .iter()
        .filter_map(|part| match part {
            ScenePart::Text { text } => Some(text.as_str()),
            ScenePart::InlineAsset { .. } => None,
        })
        .collect()
}

fn legacy_companion_value(config: &CompanionSoulConfig) -> Value {
    let mut object = Map::new();
    let section = |value: Result<Value, serde_json::Error>| value.unwrap_or(Value::Null);
    object.insert("soul".into(), section(serde_json::to_value(&config.soul)));
    object.insert(
        "authoredFacts".into(),
        section(serde_json::to_value(&config.authored_facts)),
    );
    object.insert(
        "relationshipDefaults".into(),
        section(serde_json::to_value(&config.relationship_defaults)),
    );
    object.insert(
        "memory".into(),
        json!({"sharedAcrossSessions": config.share_memory_across_chats}),
    );
    object.insert(
        "prompting".into(),
        json!({
            "promptTemplateId": config.prompting.prompt_template_id.map(|id| id.to_string()),
            "styleNotes": config.prompting.style_notes,
        }),
    );
    object.insert("timeAwareness".into(), Value::Bool(config.time_awareness));
    Value::Object(object)
}

fn shared_memory_package(
    memory: &crate::CharacterFileCompanionMemory,
) -> crate::CompanionSharedMemoryPackage {
    let memories = memory
        .pool
        .iter()
        .flat_map(|pool| &pool.items)
        .filter(|item| item.superseded_by.is_none())
        .map(|item| Value::String(item.text.clone()))
        .collect();
    let relationship_states = memory
        .relationships
        .iter()
        .map(|relationship| {
            let state = &relationship.state;
            (
                relationship
                    .persona_id
                    .map_or_else(|| "__default__".to_owned(), |id| id.to_string()),
                json!({
                    "closeness": state.closeness,
                    "trust": state.trust,
                    "affection": state.affection,
                    "tension": state.tension,
                    "stability": state.stability,
                    "interactionCount": state.interaction_count,
                    "lastInteractionAt": state.last_interaction_at.get(),
                }),
            )
        })
        .collect::<Map<_, _>>();
    crate::CompanionSharedMemoryPackage {
        memories: Value::Array(memories),
        memory_summary: None,
        memory_summary_token_count: 0,
        memory_tool_events: Value::Array(Vec::new()),
        memory_status: None,
        memory_error: None,
        memory_progress_step: None,
        soul_growth: memory
            .soul_facts
            .as_ref()
            .and_then(|facts| serde_json::to_value(facts).ok())
            .unwrap_or_else(|| Value::Array(Vec::new())),
        relationship_states: Value::Object(relationship_states),
        created_at: memory.created_at.get(),
        updated_at: memory.updated_at.get(),
    }
}

fn scheduled_note_package(note: &CompanionScheduledNote) -> CompanionScheduledNotePackage {
    CompanionScheduledNotePackage {
        id: Some(note.id.to_string()),
        character_id: Some(note.character_id.to_string()),
        label: note.label.clone(),
        content: note.content.clone(),
        available_at: note.available_at.get(),
        expires_at: note.expires_at.map(|value| value.get()),
        recurrence: match note.recurrence {
            ScheduledNoteRecurrence::None => "none",
            ScheduledNoteRecurrence::Daily => "daily",
            ScheduledNoteRecurrence::Weekly => "weekly",
            ScheduledNoteRecurrence::Monthly => "monthly",
            ScheduledNoteRecurrence::Yearly => "yearly",
        }
        .to_owned(),
        recurrence_window_ms: note
            .recurrence_window_ms
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        enabled: note.enabled,
        created_at: note.created_at.get(),
        updated_at: note.updated_at.get(),
    }
}

fn lorebook_package(details: &LorebookDetails) -> LorebookPackage {
    let mut entries = details.entries.iter().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.ordinal);
    LorebookPackage {
        lorebook: PackagedLorebook {
            id: details.book.id.to_string(),
            name: details.book.name.clone(),
            avatar_path: None,
            keyword_detection_mode: match details.book.detection_policy {
                DetectionPolicy::RecentMessageWindow => {
                    PackagedKeywordDetectionMode::RecentMessageWindow
                }
                DetectionPolicy::LatestUserMessage => {
                    PackagedKeywordDetectionMode::LatestUserMessage
                }
            },
            created_at: details.book.created_at.get(),
            updated_at: details.book.updated_at.get(),
        },
        entries: entries
            .into_iter()
            .map(|entry| PackagedLorebookEntry {
                id: entry.id.to_string(),
                lorebook_id: details.book.id.to_string(),
                title: entry.title.clone(),
                enabled: entry.enabled,
                always_active: entry.always_active,
                keywords: entry.keywords.clone(),
                case_sensitive: entry.case_sensitive,
                keyword_match_mode: match entry.match_mode {
                    KeywordMatchMode::Literal => PackagedKeywordMatchMode::Literal,
                    KeywordMatchMode::Regex => PackagedKeywordMatchMode::Regex,
                },
                content: entry.content.clone(),
                priority: entry.priority,
                display_order: i32::try_from(entry.ordinal).unwrap_or(i32::MAX),
                created_at: entry.created_at.get(),
                updated_at: entry.updated_at.get(),
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CharacterExportError {
    #[error("Character Card V1 export is read-only")]
    CardV1ReadOnly,
    #[error("Legacy JSON export is not supported")]
    LegacyJson,
    #[error(transparent)]
    Package(#[from] crate::EntityPackageError),
    #[error(transparent)]
    Card(#[from] crate::CharacterCardError),
}

/// The package written as a character file of `format`.
pub fn export_character_file(
    package: &CharacterPackage,
    format: crate::CharacterFileFormat,
    character_id: &str,
    created_at: i64,
    updated_at: i64,
) -> Result<String, CharacterExportError> {
    let card = || crate::CharacterCardSource {
        name: package.character.name.clone(),
        description: package.character.description.clone(),
        definition: package.character.definition.clone(),
        character_book: package
            .character
            .character_book
            .clone()
            .and_then(|book| serde_json::from_value(book).ok()),
        scenario: package.character.scenario.clone(),
        nickname: package.character.nickname.clone(),
        creator: package.character.creator.clone(),
        creator_notes: package.character.creator_notes.clone(),
        creator_notes_multilingual: package.character.creator_notes_multilingual.clone(),
        source: package.character.source.clone(),
        tags: package.character.tags.clone(),
        scene_contents: package
            .character
            .scenes
            .iter()
            .map(|scene| scene.content.clone())
            .collect(),
        avatar: package.avatar_data.clone(),
        created_at: Some(created_at),
        updated_at: Some(updated_at),
    };
    match format {
        crate::CharacterFileFormat::Uec => Ok(crate::build_character_uec(
            package,
            character_id,
            Some(created_at),
            Some(updated_at),
        )?),
        crate::CharacterFileFormat::CharaCardV3 => Ok(crate::export_chara_card_v3(&card())?),
        crate::CharacterFileFormat::CharaCardV2 => Ok(crate::export_chara_card_v2(&card())?),
        crate::CharacterFileFormat::CharaCardV1 => Err(CharacterExportError::CardV1ReadOnly),
        crate::CharacterFileFormat::LegacyJson => Err(CharacterExportError::LegacyJson),
    }
}
