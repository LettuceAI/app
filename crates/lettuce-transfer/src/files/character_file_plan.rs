//! A character file's package planned as a new character, the way the old
//! app's import wrote it: every id new, bundled lorebooks created, and
//! references kept only when they resolve in this app.

use std::collections::{BTreeMap, BTreeSet};

use lettuce_characters::{
    CharacterDetails, CreateCharacterPlan, InteractionMode, MAX_LOCALE_SCALARS,
};
use lettuce_companions::{
    CompanionScheduledNote, RelationshipState, ScheduledNoteRecurrence, SoulFact,
};
use lettuce_context::{LorebookBinding, LorebookDetails, PromptPurpose};
use lettuce_memory::{
    MAX_MEMORY_ITEMS, MAX_MEMORY_TEXT_BYTES, MemoryItem, MemoryShortId, MemorySpaceSnapshot,
};
use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, LorebookId, MemoryId, MemorySpaceId,
    ModelProfileId, PersonaId, PromptDocumentId, Revision, SceneId, SceneVariantId,
    StarterMessageId, TimestampMillis, VoiceProfileId,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::legacy::legacy_backup_authored::{
    CharacterRowReferences, map_character_row, map_lorebooks,
};
use crate::legacy::legacy_backup_json_values::LegacyJsonContext;
use crate::{
    CharacterPackage, CompanionScheduledNotePackage, CompanionSharedMemoryPackage,
    LegacyBackupCharacterCandidate, LegacyBackupChatTemplateCandidate,
    LegacyBackupChatTemplateMessage, LegacyBackupConversionNotice, LegacyImportSkip,
    LegacyLorebookCandidate, PackagedKeywordDetectionMode, PackagedKeywordMatchMode,
};
use crate::{
    CharacterPlanError, CharacterPlanResolver, LegacyMediaUse, character_plan_from_candidate,
    lorebook_details_from_candidate,
};

/// What already exists in the app a file's references can point at.
#[derive(Debug, Clone, Default)]
pub struct CharacterFileReferences {
    pub model_ids: BTreeSet<ModelProfileId>,
    pub chat_model_ids: BTreeSet<ModelProfileId>,
    pub prompt_purposes: BTreeMap<String, PromptPurpose>,
    pub lorebook_ids: BTreeSet<LorebookId>,
    pub voice_ids: BTreeSet<VoiceProfileId>,
    pub persona_ids: BTreeSet<PersonaId>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CharacterFilePlanError {
    #[error("Unsupported export version: {0}. Please update your app.")]
    UnsupportedVersion(u32),
    #[error("Invalid import data: {0}")]
    Invalid(String),
}

/// A new character with its bundled lorebooks; the image data URLs and
/// companion records are materialized alongside it.
#[derive(Debug, Clone)]
pub struct CharacterFilePlan {
    pub character: LegacyBackupCharacterCandidate,
    pub lorebooks: Vec<LegacyLorebookCandidate>,
    pub avatar_data: Option<String>,
    pub background_image_data: Option<String>,
    pub scene_backgrounds: BTreeMap<SceneId, String>,
    pub scheduled_notes: Vec<CompanionScheduledNotePackage>,
    pub shared_memory: Option<CompanionSharedMemoryPackage>,
    pub skipped: Vec<LegacyImportSkip>,
    pub notices: Vec<LegacyBackupConversionNotice>,
}

fn is_remote(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_data_url(value: &str) -> bool {
    value.starts_with("data:")
}

/// Plans the package as a new character; `new_id` supplies every fresh id.
pub fn plan_character_file(
    package: &CharacterPackage,
    references: &CharacterFileReferences,
    now: i64,
    mut new_id: impl FnMut() -> Uuid,
) -> Result<CharacterFilePlan, CharacterFilePlanError> {
    if package.version > 1 {
        return Err(CharacterFilePlanError::UnsupportedVersion(package.version));
    }
    let character = &package.character;
    let invalid = |error: crate::LegacyBackupAuthoredError| {
        CharacterFilePlanError::Invalid(error.to_string())
    };
    let mut notices = Vec::new();
    let mut lorebook_map = BTreeMap::new();
    let lorebook_rows = character
        .lorebooks
        .iter()
        .map(|bundled| {
            let id = new_id();
            lorebook_map.insert(bundled.lorebook.id.clone(), id.to_string());
            json!({
                "id": id.to_string(),
                "name": bundled.lorebook.name,
                "avatar_path": Value::Null,
                "keyword_detection_mode": match bundled.lorebook.keyword_detection_mode {
                    PackagedKeywordDetectionMode::RecentMessageWindow => "recent_message_window",
                    PackagedKeywordDetectionMode::LatestUserMessage => "latest_user_message",
                },
                "entries": bundled.entries.iter().map(|entry| json!({
                    "id": new_id().to_string(),
                    "title": entry.title,
                    "enabled": entry.enabled,
                    "always_active": entry.always_active,
                    "keywords": serde_json::to_string(&entry.keywords).unwrap_or_else(|_| "[]".to_owned()),
                    "case_sensitive": entry.case_sensitive,
                    "keyword_match_mode": match entry.keyword_match_mode {
                        PackagedKeywordMatchMode::Literal => "literal",
                        PackagedKeywordMatchMode::Regex => "regex",
                    },
                    "content": entry.content,
                    "priority": entry.priority,
                    "display_order": entry.display_order,
                    "created_at": now,
                    "updated_at": now,
                })).collect::<Vec<_>>(),
                "created_at": now,
                "updated_at": now,
            })
        })
        .collect::<Vec<_>>();
    let mut lorebook_rows = lorebook_rows;
    let character_book_id = character
        .character_book
        .as_ref()
        .and_then(|book| character_book_row(book, &character.name, now, &mut new_id))
        .map(|(id, row)| {
            lorebook_rows.push(row);
            id
        });
    let lorebook_rows = serde_json::from_value(Value::Array(lorebook_rows))
        .map_err(|error| CharacterFilePlanError::Invalid(error.to_string()))?;
    let lorebooks = map_lorebooks(lorebook_rows, &mut notices).map_err(invalid)?;
    let mut skipped = lorebooks.skipped;
    let lorebooks = lorebooks.lorebooks;

    let character_id = new_id();
    let mut scene_ids = BTreeMap::new();
    let mut scene_backgrounds = BTreeMap::new();
    let mut default_scene_id = None;
    let scenes = character
        .scenes
        .iter()
        .enumerate()
        .map(|(index, scene)| {
            let id = new_id();
            scene_ids.insert(scene.id.clone(), id);
            if index == 0 {
                default_scene_id = Some(id);
            }
            let background = scene
                .background_image_path
                .as_deref()
                .filter(|value| is_data_url(value));
            if let Some(background) = background
                && let Ok(scene_id) = id.to_string().parse()
            {
                scene_backgrounds.insert(scene_id, background.to_owned());
            }
            let mut variant_ids = BTreeMap::new();
            let variants = scene
                .variants
                .iter()
                .map(|variant| {
                    let id = new_id();
                    variant_ids.insert(variant.id.clone(), id);
                    json!({
                        "id": id.to_string(),
                        "content": variant.content,
                        "direction": variant.direction,
                        "created_at": variant.created_at.unwrap_or(now),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "id": id.to_string(),
                "content": scene.content,
                "direction": scene.direction,
                "background_image_path": background.map(|_| "background"),
                "selected_variant_id": scene
                    .selected_variant_id
                    .as_ref()
                    .and_then(|old| variant_ids.get(old))
                    .map(Uuid::to_string),
                "variants": variants,
                "created_at": scene.created_at.unwrap_or(now),
            })
        })
        .collect::<Vec<_>>();
    if let Some(default) = character
        .default_scene_id
        .as_ref()
        .and_then(|id| scene_ids.get(id))
    {
        default_scene_id = Some(*default);
    }

    let mut template_ids = BTreeMap::new();
    let chat_templates = character
        .chat_templates
        .iter()
        .map(|template| {
            let id = new_id().to_string();
            template_ids.insert(template.id.clone(), id.clone());
            LegacyBackupChatTemplateCandidate {
                source_id: id,
                character_source_id: character_id.to_string(),
                name: template.name.clone(),
                scene_source_id: None,
                prompt_source_id: None,
                has_lorebook_override: false,
                lorebook_source_ids: Vec::new(),
                messages: template
                    .messages
                    .iter()
                    .enumerate()
                    .map(|(index, message)| LegacyBackupChatTemplateMessage {
                        source_id: new_id().to_string(),
                        index: u32::try_from(index).unwrap_or(u32::MAX),
                        role: message.role.clone(),
                        content: message.content.clone(),
                    })
                    .collect(),
                created_at: lettuce_types::TimestampMillis::new(template.created_at.unwrap_or(now)),
            }
        })
        .collect::<Vec<_>>();

    let active_lorebook_ids = character
        .active_lorebook_ids
        .iter()
        .map(|id| lorebook_map.get(id).cloned().unwrap_or_else(|| id.clone()))
        .chain(character_book_id)
        .collect::<Vec<_>>();
    let background = package
        .background_image_data
        .clone()
        .filter(|value| !is_remote(value));
    let crop = |crop: &Option<crate::PackageCrop>| {
        crop.as_ref().map_or((None, None, None), |crop| {
            (Some(crop.x), Some(crop.y), Some(crop.scale))
        })
    };
    let (avatar_x, avatar_y, avatar_scale) = crop(&character.avatar_crop);
    let (banner_x, banner_y, banner_scale) = crop(&character.banner_crop);
    let json_text = |value: &Option<Value>| {
        value
            .as_ref()
            .filter(|value| !value.is_null())
            .map(Value::to_string)
    };
    let tags = character.tags.as_ref().map(|tags| {
        tags.iter()
            .flat_map(|tag| tag.split(','))
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>()
    });
    let sources = character.source.as_ref().map(|sources| {
        sources
            .iter()
            .filter(|source| !source.trim().is_empty())
            .collect::<Vec<_>>()
    });
    let creator_notes_multilingual = character
        .creator_notes_multilingual
        .as_ref()
        .and_then(Value::as_object)
        .map(|notes| {
            notes
                .iter()
                .filter(|(locale, notes)| {
                    let kept = notes.is_string()
                        && !locale.trim().is_empty()
                        && locale.chars().count() <= MAX_LOCALE_SCALARS;
                    if !kept {
                        skipped.push(crate::legacy_value_skip(
                            "creator_notes_multilingual",
                            locale,
                            crate::LegacyImportSkipReason::MalformedLegacyValue,
                        ));
                    }
                    kept
                })
                .map(|(locale, notes)| (locale.clone(), notes.clone()))
                .collect::<serde_json::Map<_, _>>()
        })
        .map(|notes| Value::Object(notes).to_string());
    let row = json!({
        "id": character_id.to_string(),
        "name": character.name,
        "avatar_path": package.avatar_data.as_ref().map(|_| "avatar"),
        "avatar_crop_x": avatar_x,
        "avatar_crop_y": avatar_y,
        "avatar_crop_scale": avatar_scale,
        "banner_crop_x": banner_x,
        "banner_crop_y": banner_y,
        "banner_crop_scale": banner_scale,
        "background_image_path": background.as_ref().map(|_| "background"),
        "description": character.description,
        "definition": character.definition.clone().or_else(|| character.description.clone()),
        "nickname": character.nickname,
        "scenario": character.scenario,
        "creator_notes": character.creator_notes,
        "creator": character.creator,
        "creator_notes_multilingual": creator_notes_multilingual,
        "source": sources.map(|value| json!(value).to_string()),
        "tags": tags.map(|value| json!(value).to_string()),
        "default_scene_id": default_scene_id.map(|id| id.to_string()),
        "default_model_id": character.default_model_id,
        "mode": if character.mode.as_deref() == Some("companion") { "companion" } else { "roleplay" },
        "companion": character.companion,
        "memory_type": if character.memory_type.as_deref() == Some("dynamic") { "dynamic" } else { "manual" },
        "active_lorebook_ids": json!(active_lorebook_ids).to_string(),
        "prompt_template_id": character.prompt_template_id,
        "system_prompt": character.system_prompt,
        "voice_config": json_text(&character.voice_config),
        "voice_autoplay": character.voice_autoplay.unwrap_or(false),
        "disable_avatar_gradient": character.disable_avatar_gradient,
        "custom_gradient_enabled": character.custom_gradient_enabled.unwrap_or(false),
        "custom_gradient_colors": character.custom_gradient_colors.as_ref().map(|value| json!(value).to_string()),
        "custom_text_color": character.custom_text_color,
        "custom_text_secondary": character.custom_text_secondary,
        "default_chat_template_id": character
            .default_chat_template_id
            .as_ref()
            .and_then(|id| template_ids.get(id)),
        "rules": character.rules,
        "scenes": scenes,
        "created_at": now,
        "updated_at": now,
    });
    let row = serde_json::from_value(row)
        .map_err(|error| CharacterFilePlanError::Invalid(error.to_string()))?;
    let lorebook_ids = references
        .lorebook_ids
        .iter()
        .copied()
        .chain(lorebooks.iter().map(|lorebook| lorebook.id))
        .collect::<BTreeSet<_>>();
    let json_context = LegacyJsonContext::for_file(references.voice_ids.clone());
    let row_references = CharacterRowReferences {
        model_ids: references.model_ids.clone(),
        chat_model_ids: references.chat_model_ids.clone(),
        prompt_purposes: references
            .prompt_purposes
            .iter()
            .map(|(id, purpose)| (id.as_str(), *purpose))
            .collect(),
        chat_templates: &chat_templates,
        lorebook_ids: &lorebook_ids,
        json_context: &json_context,
    };
    let candidate = map_character_row(
        row,
        &row_references,
        &mut BTreeSet::new(),
        &mut skipped,
        &mut notices,
    )
    .map_err(invalid)?;
    Ok(CharacterFilePlan {
        character: candidate,
        lorebooks,
        avatar_data: package.avatar_data.clone(),
        background_image_data: background,
        scene_backgrounds,
        scheduled_notes: character.companion_scheduled_notes.clone(),
        shared_memory: character.companion_shared_memory.clone(),
        skipped,
        notices,
    })
}

/// A card's embedded `character_book` as a new lorebook row and its id, or
/// `None` without entries. Keys and secondary keys merge into keywords, a
/// `constant` entry is always active, and `insertion_order` orders entries.
fn character_book_row(
    book: &Value,
    character_name: &str,
    now: i64,
    new_id: &mut impl FnMut() -> Uuid,
) -> Option<(String, Value)> {
    let entries = book
        .get("entries")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty())?;
    let text = |value: Option<&Value>| {
        value
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    };
    let name = Some(text(book.get("name")))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("{} Lorebook", character_name.trim()));
    let entries = entries
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut keywords: Vec<String> = Vec::new();
            for key in ["keys", "secondary_keys"]
                .into_iter()
                .filter_map(|field| item.get(field).and_then(Value::as_array))
                .flatten()
                .map(|key| text(Some(key)))
                .filter(|key| !key.is_empty())
            {
                if !keywords.contains(&key) {
                    keywords.push(key);
                }
            }
            let title = Some(text(item.get("name")))
                .filter(|title| !title.is_empty())
                .or_else(|| keywords.first().cloned())
                .unwrap_or_else(|| format!("Entry {}", index + 1));
            let number = |field: &str| {
                item.get(field)
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
            };
            json!({
                "id": new_id().to_string(),
                "title": title,
                "enabled": item.get("enabled") != Some(&Value::Bool(false)),
                "always_active": item.get("constant") == Some(&Value::Bool(true)),
                "keywords": serde_json::to_string(&keywords).unwrap_or_else(|_| "[]".to_owned()),
                "case_sensitive": item.get("case_sensitive") == Some(&Value::Bool(true)),
                "keyword_match_mode": "literal",
                "content": text(item.get("content")),
                "priority": number("priority").unwrap_or(0),
                "display_order": number("insertion_order")
                    .unwrap_or_else(|| i32::try_from(index).unwrap_or(i32::MAX)),
                "created_at": now,
                "updated_at": now,
            })
        })
        .collect::<Vec<_>>();
    let id = new_id().to_string();
    let row = json!({
        "id": id,
        "name": name,
        "avatar_path": Value::Null,
        "keyword_detection_mode": "recent_message_window",
        "entries": entries,
        "created_at": now,
        "updated_at": now,
    });
    Some((id, row))
}

/// The stored images a plan's data URLs became.
#[derive(Debug, Clone, Default)]
pub struct CharacterFileAssets {
    pub avatar: Option<AssetId>,
    pub background: Option<AssetId>,
    pub scene_backgrounds: BTreeMap<SceneId, AssetId>,
}

/// Everything a character file writes, in one transaction.
#[derive(Debug, Clone)]
pub struct CharacterFileImport {
    pub lorebooks: Vec<LorebookDetails>,
    pub character: CreateCharacterPlan,
    pub lorebook_bindings: Vec<LorebookBinding>,
    pub scheduled_notes: Vec<CompanionScheduledNote>,
    pub companion_memory: Option<CharacterFileCompanionMemory>,
    pub skipped: Vec<LegacyImportSkip>,
    pub notices: Vec<LegacyBackupConversionNotice>,
}

/// A companion's shared memory from its file: the memory pool its chats
/// share, its grown soul and its relationship with each persona.
#[derive(Debug, Clone)]
pub struct CharacterFileCompanionMemory {
    pub pool: Option<MemorySpaceSnapshot>,
    pub soul_facts: Option<Vec<SoulFact>>,
    pub relationships: Vec<CharacterFileRelationship>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone)]
pub struct CharacterFileRelationship {
    pub persona_id: Option<PersonaId>,
    pub state: RelationshipState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CharacterFileRepositoryError {
    #[error("the character file conflicts with stored data")]
    Conflict,
    #[error("the character file is invalid")]
    InvalidInput,
    #[error("character file storage failed")]
    Storage,
}

pub trait CharacterFileRepository: Send + Sync {
    fn character_file_references(
        &self,
    ) -> Result<CharacterFileReferences, CharacterFileRepositoryError>;

    fn import_character_file(
        &self,
        import: &CharacterFileImport,
    ) -> Result<CharacterDetails, CharacterFileRepositoryError>;

    fn character_export_record(
        &self,
        id: CharacterId,
    ) -> Result<Option<crate::CharacterExportRecord>, CharacterFileRepositoryError>;
}

impl CharacterFilePlan {
    /// What the plan writes once its images are stored as `assets`; an image
    /// that was not stored is left off the character.
    pub fn import(
        &self,
        references: &CharacterFileReferences,
        assets: &CharacterFileAssets,
        now: i64,
        mut new_id: impl FnMut() -> Uuid,
    ) -> Result<CharacterFileImport, CharacterPlanError> {
        let mut character = self.character.clone();
        if assets.avatar.is_none() {
            character.media.avatar = None;
        }
        if assets.background.is_none() {
            character.media.background = None;
        }
        for scene in &mut character.scenes {
            if !assets.scene_backgrounds.contains_key(&scene.id) {
                scene.background = None;
            }
        }
        let plan = character_plan_from_candidate(&character, &FileResolver { references, assets })?;
        let lorebooks = self
            .lorebooks
            .iter()
            .map(|lorebook| lorebook_details_from_candidate(lorebook, lorebook.id, None, Some))
            .collect::<Result<Vec<_>, _>>()?;
        let lorebook_bindings = character
            .active_lorebook_ids
            .iter()
            .enumerate()
            .map(|(ordinal, lorebook_id)| {
                Ok(LorebookBinding {
                    lorebook_id: *lorebook_id,
                    enabled: true,
                    ordinal: u32::try_from(ordinal)
                        .map_err(|_| CharacterPlanError::InvalidInput)?,
                    revision: Revision::INITIAL,
                    created_at: character.created_at,
                    updated_at: character.updated_at,
                })
            })
            .collect::<Result<Vec<_>, CharacterPlanError>>()?;
        let mut skipped = Vec::new();
        let mut scheduled_notes = Vec::new();
        if character.defaults.interaction_mode == InteractionMode::Companion {
            for (index, note) in self.scheduled_notes.iter().enumerate() {
                if note.content.trim().is_empty() {
                    continue;
                }
                match scheduled_note(note, plan.character.id, now, new_id()) {
                    Some(note) => scheduled_notes.push(note),
                    None => skipped.push(crate::legacy_value_skip(
                        "companion_scheduled_notes",
                        note.id.as_deref().unwrap_or(&index.to_string()),
                        crate::LegacyImportSkipReason::MalformedLegacyValue,
                    )),
                }
            }
        }
        let mut notices = Vec::new();
        let companion_memory = if character.defaults.interaction_mode == InteractionMode::Companion
        {
            self.shared_memory.as_ref().map(|shared| {
                companion_memory(
                    shared,
                    references,
                    now,
                    &mut new_id,
                    &mut skipped,
                    &mut notices,
                )
            })
        } else {
            None
        };
        Ok(CharacterFileImport {
            lorebooks,
            character: plan,
            lorebook_bindings,
            scheduled_notes,
            companion_memory,
            skipped,
            notices,
        })
    }
}

fn companion_memory(
    shared: &CompanionSharedMemoryPackage,
    references: &CharacterFileReferences,
    now: i64,
    new_id: &mut impl FnMut() -> Uuid,
    skipped: &mut Vec<LegacyImportSkip>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> CharacterFileCompanionMemory {
    let positive_or_now = |value: i64| TimestampMillis::new(if value > 0 { value } else { now });
    let created_at = positive_or_now(shared.created_at);
    let updated_at = positive_or_now(shared.updated_at).max(created_at);
    let mut items: Vec<MemoryItem> = Vec::new();
    for (index, text) in shared.memories.as_array().into_iter().flatten().enumerate() {
        let Some(text) = text
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty() && text.len() <= MAX_MEMORY_TEXT_BYTES)
        else {
            skipped.push(crate::legacy_value_skip(
                "companion_shared_memory.memories",
                &index.to_string(),
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            ));
            continue;
        };
        if items.len() == MAX_MEMORY_ITEMS {
            skipped.push(crate::legacy_value_skip(
                "companion_shared_memory.memories",
                &index.to_string(),
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            ));
            continue;
        }
        let id = MemoryId::from_uuid(new_id());
        let short_id = MemoryShortId::allocate(id, |candidate| {
            items.iter().any(|item| item.short_id == candidate)
        });
        items.push(MemoryItem::written(
            id,
            short_id,
            text.to_owned(),
            updated_at,
        ));
    }
    let pool = (!items.is_empty()).then(|| MemorySpaceSnapshot {
        id: MemorySpaceId::from_uuid(new_id()),
        revision: Revision::INITIAL,
        items,
    });
    let soul_facts = shared
        .soul_growth
        .as_array()
        .filter(|facts| !facts.is_empty())
        .and_then(|_| {
            crate::legacy::legacy_backup_json_values::legacy_soul_growth(
                &shared.soul_growth,
                "character",
                updated_at,
                skipped,
            )
        });
    let mut relationships = Vec::new();
    for (key, value) in shared.relationship_states.as_object().into_iter().flatten() {
        let persona_id = if key == "__default__" {
            None
        } else {
            match key.parse::<PersonaId>() {
                Ok(id) if references.persona_ids.contains(&id) => Some(id),
                _ => {
                    skipped.push(crate::LegacyImportSkip {
                        kind: crate::LegacyImportSkipKind::PersonaReference,
                        source_key: format!("companion_shared_memory.relationship_states:{key}"),
                        reason: crate::LegacyImportSkipReason::MissingPersona,
                    });
                    continue;
                }
            }
        };
        match crate::legacy::legacy_backup_companion_shared_memory::legacy_relationship_state(value)
        {
            Some(state) => relationships.push(CharacterFileRelationship { persona_id, state }),
            None => skipped.push(crate::legacy_value_skip(
                "companion_shared_memory.relationship_states",
                key,
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            )),
        }
    }
    if shared
        .memory_summary
        .as_deref()
        .is_some_and(|summary| !summary.trim().is_empty())
    {
        notices.push(LegacyBackupConversionNotice {
            kind: crate::LegacyBackupConversionNoticeKind::Lossy,
            document: crate::LegacyBackupDocumentKind::CompanionSharedMemory,
            field: "memory_summary".to_owned(),
        });
    }
    CharacterFileCompanionMemory {
        pool,
        soul_facts,
        relationships,
        created_at,
        updated_at,
    }
}

fn scheduled_note(
    note: &CompanionScheduledNotePackage,
    character_id: CharacterId,
    now: i64,
    id: Uuid,
) -> Option<CompanionScheduledNote> {
    let positive_or_now = |value: i64| TimestampMillis::new(if value > 0 { value } else { now });
    CompanionScheduledNote {
        id,
        character_id,
        label: note.label.clone(),
        content: note.content.clone(),
        available_at: TimestampMillis::new(note.available_at.max(0)),
        expires_at: note
            .expires_at
            .map(|value| TimestampMillis::new(value.max(0))),
        recurrence: match note.recurrence.trim().to_ascii_lowercase().as_str() {
            "daily" => ScheduledNoteRecurrence::Daily,
            "weekly" => ScheduledNoteRecurrence::Weekly,
            "monthly" => ScheduledNoteRecurrence::Monthly,
            "yearly" => ScheduledNoteRecurrence::Yearly,
            _ => ScheduledNoteRecurrence::None,
        },
        recurrence_window_ms: note
            .recurrence_window_ms
            .map(|value| u64::try_from(value.max(0)).unwrap_or(0)),
        enabled: note.enabled,
        created_at: positive_or_now(note.created_at),
        updated_at: positive_or_now(note.updated_at),
    }
    .normalize()
    .ok()
}

struct FileResolver<'a> {
    references: &'a CharacterFileReferences,
    assets: &'a CharacterFileAssets,
}

impl CharacterPlanResolver for FileResolver<'_> {
    fn character(&self, planned: CharacterId) -> CharacterId {
        planned
    }

    fn scene(&self, planned: SceneId) -> SceneId {
        planned
    }

    fn variant(&self, planned: SceneVariantId) -> SceneVariantId {
        planned
    }

    fn starter(&self, planned: ConversationStarterId) -> ConversationStarterId {
        planned
    }

    fn starter_message(&self, planned: StarterMessageId) -> StarterMessageId {
        planned
    }

    fn voice_profile(&self, planned: VoiceProfileId) -> VoiceProfileId {
        planned
    }

    fn model(&self, planned: ModelProfileId) -> Result<ModelProfileId, CharacterPlanError> {
        Ok(planned)
    }

    fn prompt(&self, source_id: &str) -> Option<PromptDocumentId> {
        self.references
            .prompt_purposes
            .contains_key(source_id)
            .then(|| source_id.parse().ok())
            .flatten()
    }

    fn lorebook(&self, planned: LorebookId) -> Result<LorebookId, CharacterPlanError> {
        Ok(planned)
    }

    fn asset(&self, media_use: &LegacyMediaUse) -> Result<AssetId, CharacterPlanError> {
        match media_use {
            LegacyMediaUse::CharacterAvatar { .. } => self.assets.avatar,
            LegacyMediaUse::CharacterBackground { .. } => self.assets.background,
            LegacyMediaUse::CharacterSceneBackground { scene_id, .. } => {
                self.assets.scene_backgrounds.get(scene_id).copied()
            }
            _ => None,
        }
        .ok_or(CharacterPlanError::MissingReference)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ids() -> impl FnMut() -> Uuid {
        let mut next = 0_u128;
        move || {
            next += 1;
            Uuid::from_u128(next)
        }
    }

    #[test]
    fn a_package_becomes_a_new_character_with_its_bundled_lorebooks() {
        let package: CharacterPackage = serde_json::from_value(json!({
            "version": 1,
            "exportedAt": 5,
            "character": {
                "name": "Ada",
                "description": "Keeper",
                "rules": ["Be kind"],
                "scenes": [
                    {"id": "scene-1", "content": "Harbour", "selectedVariantId": "v1", "variants": [{"id": "v1", "content": "Rainy harbour"}]},
                    {"id": "scene-2", "content": "Market", "backgroundImagePath": "data:image/png;base64,AA==", "selectedVariantId": null, "variants": []}
                ],
                "defaultSceneId": "scene-2",
                "defaultModelId": "00000000-0000-0000-0000-00000000abcd",
                "mode": "companion",
                "companion": {"soul": {"essence": "Warm"}},
                "memoryType": "dynamic",
                "activeLorebookIds": ["book-1", "00000000-0000-0000-0000-00000000beef"],
                "lorebooks": [{
                    "lorebook": {"id": "book-1", "name": "Harbour", "avatarPath": null, "keywordDetectionMode": "latestUserMessage", "createdAt": 1, "updatedAt": 1},
                    "entries": [{"id": "e1", "lorebookId": "book-1", "title": "Key", "enabled": true, "alwaysActive": false, "keywords": ["key"], "caseSensitive": false, "keywordMatchMode": "regex", "content": "Ada keeps it", "priority": 2, "displayOrder": 0, "createdAt": 1, "updatedAt": 1}]
                }],
                "promptTemplateId": "prompt-1",
                "systemPrompt": null,
                "voiceConfig": null,
                "voiceAutoplay": null,
                "disableAvatarGradient": false,
                "avatarCrop": null,
                "customGradientEnabled": null,
                "customGradientColors": null,
                "customTextColor": null,
                "customTextSecondary": null,
                "chatTemplates": [{"id": "t1", "name": "Hello", "messages": [{"id": "m1", "role": "user", "content": "Hi"}]}],
                "defaultChatTemplateId": "t1"
            },
            "avatarData": "data:image/png;base64,AA==",
            "backgroundImageData": "https://example.test/bg.png"
        }))
        .expect("package");
        let references = CharacterFileReferences {
            prompt_purposes: BTreeMap::from([("prompt-1".to_owned(), PromptPurpose::DirectChat)]),
            lorebook_ids: BTreeSet::from(["00000000-0000-0000-0000-00000000beef"
                .parse()
                .expect("id")]),
            ..CharacterFileReferences::default()
        };
        let plan = plan_character_file(&package, &references, 50, ids()).expect("plan");
        let character = &plan.character;
        assert_eq!(character.profile.name, "Ada");
        assert_eq!(character.profile.rules, vec!["Be kind".to_owned()]);
        assert_eq!(character.scenes.len(), 2);
        assert_eq!(
            character.defaults.default_scene_id,
            Some(character.scenes[1].id)
        );
        assert_eq!(
            character.scenes[0].selected_variant_id,
            Some(character.scenes[0].variants[0].id)
        );
        assert_eq!(character.defaults.model_profile_id, None);
        assert_eq!(
            character.defaults.direct_prompt_source_id.as_deref(),
            Some("prompt-1")
        );
        assert_eq!(
            character.defaults.interaction_mode,
            lettuce_characters::InteractionMode::Companion
        );
        assert_eq!(
            character.defaults.memory_policy,
            lettuce_characters::MemoryPolicy::Dynamic
        );
        assert_eq!(
            character
                .defaults
                .companion_soul
                .as_ref()
                .expect("soul")
                .soul
                .essence,
            "Warm"
        );
        assert_eq!(plan.lorebooks.len(), 1);
        assert_eq!(
            character.active_lorebook_ids,
            vec![
                plan.lorebooks[0].id,
                "00000000-0000-0000-0000-00000000beef".parse().expect("id")
            ]
        );
        assert_eq!(character.starters.len(), 1);
        assert_eq!(
            character.defaults.default_starter_source_id.as_deref(),
            Some(character.starters[0].source_id.as_str())
        );
        assert!(character.media.avatar.is_some());
        assert!(character.media.background.is_none());
        assert_eq!(plan.background_image_data, None);
        assert_eq!(plan.scene_backgrounds.len(), 1);
        assert!(character.scenes[1].background.is_some());
        assert!(
            plan.skipped
                .iter()
                .any(|skip| skip.reason == crate::LegacyImportSkipReason::MissingModelProfile)
        );
        assert!(matches!(
            plan_character_file(
                &CharacterPackage {
                    version: 2,
                    ..package
                },
                &references,
                0,
                ids()
            ),
            Err(CharacterFilePlanError::UnsupportedVersion(2))
        ));
    }

    fn card_package(data: &Value) -> CharacterPackage {
        let card = json!({"spec": "chara_card_v2", "spec_version": "2.0", "data": data});
        let mut next = 0_u32;
        crate::parse_character_import(&card, 10, || {
            next += 1;
            format!("file-{next}")
        })
        .expect("card")
        .0
    }

    #[test]
    fn card_tags_sources_and_localized_notes_are_normalized_like_the_legacy_character_form() {
        let long_tag = "t".repeat(4096);
        let name = "N".repeat(300);
        let package = card_package(&json!({
            "name": name,
            "description": "Keeper",
            "first_mes": "Hello",
            "tags": ["", " fantasy ", "sea, storm", long_tag]
        }));
        let mut package = package;
        package.character.creator_notes_multilingual =
            Some(json!({"en": "Hello", "fr": 3, " ": "blank"}));
        package.character.source = Some(vec![" ".to_owned(), "https://example.test".to_owned()]);
        let plan = plan_character_file(&package, &CharacterFileReferences::default(), 50, ids())
            .expect("a lenient card still imports");
        let provenance = &plan.character.provenance;
        assert_eq!(plan.character.profile.name, name);
        assert_eq!(
            provenance.tags,
            vec![
                "fantasy".to_owned(),
                "sea".to_owned(),
                "storm".to_owned(),
                long_tag
            ]
        );
        assert_eq!(provenance.sources, vec!["https://example.test".to_owned()]);
        assert_eq!(
            provenance.localized_creator_notes,
            BTreeMap::from([("en".to_owned(), "Hello".to_owned())])
        );
        assert_eq!(
            plan.skipped
                .iter()
                .filter(|skip| skip.source_key.starts_with("creator_notes_multilingual"))
                .count(),
            2
        );

        package.character.creator_notes_multilingual = Some(json!(["not", "a", "map"]));
        let plan = plan_character_file(&package, &CharacterFileReferences::default(), 50, ids())
            .expect("a non-object note map is dropped");
        assert!(plan.character.provenance.localized_creator_notes.is_empty());
    }

    #[test]
    fn a_card_character_book_becomes_a_bound_lorebook_like_the_legacy_character_form() {
        let package = card_package(&json!({
            "name": " Mira ",
            "description": "Keeper",
            "first_mes": "Hello",
            "character_book": {"entries": [
                {"keys": [" lamp ", ""], "secondary_keys": ["lamp", "tower"], "content": " The lamp burns. ", "enabled": true, "constant": true, "case_sensitive": true, "priority": 4, "insertion_order": 7},
                {"name": "Harbour", "keys": ["dock"], "content": "Boats", "enabled": false, "insertion_order": 1},
                {"content": "No keys", "enabled": true}
            ]}
        }));
        let plan = plan_character_file(&package, &CharacterFileReferences::default(), 50, ids())
            .expect("plan");
        assert_eq!(plan.lorebooks.len(), 1);
        let book = &plan.lorebooks[0];
        assert_eq!(book.name, "Mira Lorebook");
        assert_eq!(plan.character.active_lorebook_ids, vec![book.id]);
        assert_eq!(book.entries.len(), 3);
        let by_title = |title: &str| {
            book.entries
                .iter()
                .find(|entry| entry.title == title)
                .expect("entry")
        };
        let lamp = by_title("lamp");
        assert_eq!(lamp.keywords, vec!["lamp".to_owned(), "tower".to_owned()]);
        assert_eq!(lamp.content, "The lamp burns.");
        assert!(lamp.enabled && lamp.always_active && lamp.case_sensitive);
        assert_eq!((lamp.priority, lamp.display_order), (4, 7));
        let harbour = by_title("Harbour");
        assert!(!harbour.enabled && !harbour.always_active);
        assert_eq!(harbour.display_order, 1);
        let fallback = by_title("Entry 3");
        assert!(fallback.keywords.is_empty() && fallback.enabled);
        assert_eq!(fallback.display_order, 0);
        let raw = json!({"entries": [{"keys": ["a"], "content": "x"}, {"content": "y", "enabled": null}]});
        let (_, row) = character_book_row(&raw, "Mira", 1, &mut ids()).expect("character book row");
        assert_eq!(row["entries"][1]["title"], "Entry 2");
        assert_eq!(row["entries"][1]["display_order"], 1);
        assert_eq!(row["entries"][1]["enabled"], true);

        let mut empty = package;
        empty.character.character_book = Some(json!({"name": "Empty", "entries": []}));
        let plan = plan_character_file(&empty, &CharacterFileReferences::default(), 50, ids())
            .expect("plan");
        assert!(plan.lorebooks.is_empty());
        assert!(plan.character.active_lorebook_ids.is_empty());
    }
}
