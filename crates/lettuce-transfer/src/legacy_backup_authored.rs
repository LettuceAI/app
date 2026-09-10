use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use lettuce_characters::{
    CardStyle, CharacterPresentationV1, CharacterProfile, CharacterProvenance, ChatMode,
    ConversationStarter, Crop, GradientSource, GroupMember, GroupProfile, InteractionMode,
    LifecycleStatus, MemoryPolicy, SceneDocumentV1, ScenePart, Selection, SpeakerSelection,
    StarterMessage, StarterRole,
};
use lettuce_context::LorebookBinding;
use lettuce_types::{
    CharacterId, ConversationStarterId, GroupId, LorebookId, ModelProfileId, PersonaId, Revision,
    SceneId, SceneVariantId, StarterMessageId, TimestampMillis,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    BackupLorebookBindings, LegacyBackupConfigurationError, LegacyBackupConfigurationPlan,
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyCrop, LegacyImageRecommendation, LegacyKeywordMatchMode, LegacyLorebookCandidate,
    LegacyLorebookDetectionPolicy, LegacyLorebookEntryCandidate, LegacyLorebookPlan,
    LegacyMediaReference, LegacyPersonaCandidate, LegacyPersonaPlan,
};

const CHARACTER_LIMIT: usize = 10_000;
const SCENE_LIMIT: usize = 100_000;
const CHILD_LIMIT: usize = 512;

#[derive(Debug)]
pub struct LegacyBackupAuthoredPlan {
    pub personas: LegacyPersonaPlan,
    pub lorebooks: LegacyLorebookPlan,
    pub characters: Vec<LegacyBackupCharacterCandidate>,
    pub character_lorebooks: Vec<BackupLorebookBindings<CharacterId>>,
    pub persona_lorebooks: Vec<BackupLorebookBindings<PersonaId>>,
    pub groups: Vec<LegacyBackupGroupCandidate>,
    pub group_lorebooks: Vec<BackupLorebookBindings<GroupId>>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub configuration: LegacyBackupConfigurationPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCharacterCandidate {
    pub id: CharacterId,
    pub profile: CharacterProfile,
    pub provenance: CharacterProvenance,
    pub scenario: Option<String>,
    pub rules: Vec<String>,
    pub defaults: LegacyBackupCharacterDefaults,
    pub presentation: LegacyBackupCharacterPresentation,
    pub media: LegacyBackupCharacterMedia,
    pub image_recommendation: Option<LegacyImageRecommendation>,
    pub active_lorebook_ids: Vec<LorebookId>,
    pub scenes: Vec<LegacyBackupSceneCandidate>,
    pub starters: Vec<LegacyBackupStarterCandidate>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCharacterDefaults {
    pub interaction_mode: InteractionMode,
    pub memory_policy: MemoryPolicy,
    pub model_profile_id: Option<ModelProfileId>,
    pub default_scene_id: Option<SceneId>,
    pub default_starter_source_id: Option<String>,
    pub direct_prompt_source_id: Option<String>,
    pub group_conversation_prompt_source_id: Option<String>,
    pub group_roleplay_prompt_source_id: Option<String>,
    pub system_prompt: Option<String>,
    pub companion: Option<Value>,
    pub voice_config: Option<Value>,
    pub voice_autoplay: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCharacterPresentation {
    pub card_style: CardStyle,
    pub avatar_crop: Option<Crop>,
    pub banner_crop: Option<Crop>,
    pub disable_gradient: bool,
    pub gradient_source: GradientSource,
    pub custom_gradient_enabled: bool,
    pub custom_gradient_colors: Vec<String>,
    pub primary_text_color: Option<String>,
    pub secondary_text_color: Option<String>,
    pub chat_appearance: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyBackupCharacterMedia {
    pub avatar: Option<LegacyMediaReference>,
    pub background: Option<LegacyMediaReference>,
    pub design_references: Vec<LegacyMediaReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupSceneCandidate {
    pub id: SceneId,
    pub ordinal: u32,
    pub content: SceneDocumentV1,
    pub direction: Option<String>,
    pub background: Option<LegacyMediaReference>,
    pub selected_variant_id: Option<SceneVariantId>,
    pub variants: Vec<LegacyBackupSceneVariantCandidate>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupSceneVariantCandidate {
    pub id: SceneVariantId,
    pub ordinal: u32,
    pub content: SceneDocumentV1,
    pub direction: Option<String>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupStarterCandidate {
    pub id: ConversationStarterId,
    pub source_id: String,
    pub name: String,
    pub ordinal: u32,
    pub messages: Vec<StarterMessage>,
    pub scene_id: Option<SceneId>,
    pub prompt_source_id: Option<String>,
    pub lorebook_ids: Option<Vec<LorebookId>>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupGroupCandidate {
    pub id: GroupId,
    pub status: LifecycleStatus,
    pub name: String,
    pub chat_mode: ChatMode,
    pub persona: Selection<PersonaId>,
    pub speaker_selection: SpeakerSelection,
    pub memory_policy: MemoryPolicy,
    pub disable_character_lorebooks: bool,
    pub group_conversation_prompt_source_id: Option<String>,
    pub group_roleplay_prompt_source_id: Option<String>,
    pub chat_appearance: Option<Value>,
    pub members: Vec<GroupMember>,
    pub starting_scene: Option<LegacyBackupSceneCandidate>,
    pub background: Option<LegacyMediaReference>,
    pub lorebook_ids: Vec<LorebookId>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupAuthoredError {
    #[error("legacy backup authored document is malformed")]
    Malformed {
        document: LegacyBackupDocumentKind,
        field: String,
    },
    #[error("legacy backup authored document exceeds its record limit")]
    LimitExceeded { document: LegacyBackupDocumentKind },
    #[error("legacy backup authored graph contains an orphaned ownership link")]
    Orphan {
        document: LegacyBackupDocumentKind,
        field: String,
    },
}

#[derive(Deserialize)]
struct PersonaRow {
    id: String,
    title: String,
    description: String,
    nickname: Option<String>,
    avatar_path: Option<String>,
    avatar_crop_x: Option<f64>,
    avatar_crop_y: Option<f64>,
    avatar_crop_scale: Option<f64>,
    design_description: Option<String>,
    design_reference_image_ids: Option<String>,
    lora_name: Option<String>,
    lora_strength: Option<f64>,
    #[serde(default = "empty_json_array")]
    active_lorebook_ids: String,
    #[serde(default, deserialize_with = "nullable_bool")]
    is_default: bool,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct LorebookRow {
    id: String,
    name: String,
    avatar_path: Option<String>,
    #[serde(default = "default_detection_policy")]
    keyword_detection_mode: String,
    #[serde(default)]
    entries: Vec<LorebookEntryRow>,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct LorebookEntryRow {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_true", deserialize_with = "nullable_true")]
    enabled: bool,
    #[serde(default, deserialize_with = "nullable_bool")]
    always_active: bool,
    #[serde(default = "empty_json_array")]
    keywords: String,
    #[serde(default, deserialize_with = "nullable_bool")]
    case_sensitive: bool,
    #[serde(default = "default_keyword_match")]
    keyword_match_mode: String,
    content: String,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    display_order: i32,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct CharacterRow {
    id: String,
    name: String,
    avatar_path: Option<String>,
    avatar_crop_x: Option<f64>,
    avatar_crop_y: Option<f64>,
    avatar_crop_scale: Option<f64>,
    banner_crop_x: Option<f64>,
    banner_crop_y: Option<f64>,
    banner_crop_scale: Option<f64>,
    #[serde(default = "default_card_style")]
    card_type: String,
    design_description: Option<String>,
    design_reference_image_ids: Option<String>,
    lora_name: Option<String>,
    lora_strength: Option<f64>,
    background_image_path: Option<String>,
    description: Option<String>,
    definition: Option<String>,
    nickname: Option<String>,
    scenario: Option<String>,
    creator_notes: Option<String>,
    creator: Option<String>,
    creator_notes_multilingual: Option<String>,
    source: Option<String>,
    tags: Option<String>,
    default_scene_id: Option<String>,
    default_model_id: Option<String>,
    #[serde(default = "default_mode")]
    mode: String,
    companion: Option<Value>,
    #[serde(default = "default_memory")]
    memory_type: String,
    #[serde(default = "empty_json_array")]
    active_lorebook_ids: String,
    prompt_template_id: Option<String>,
    group_chat_prompt_template_id: Option<String>,
    group_chat_roleplay_prompt_template_id: Option<String>,
    system_prompt: Option<String>,
    voice_config: Option<String>,
    #[serde(default, deserialize_with = "bool_or_integer")]
    voice_autoplay: bool,
    #[serde(default, deserialize_with = "nullable_bool")]
    disable_avatar_gradient: bool,
    #[serde(default = "default_gradient_source")]
    avatar_gradient_source: String,
    #[serde(default, deserialize_with = "nullable_bool")]
    custom_gradient_enabled: bool,
    custom_gradient_colors: Option<String>,
    custom_text_color: Option<String>,
    custom_text_secondary: Option<String>,
    chat_appearance: Option<String>,
    default_chat_template_id: Option<String>,
    #[serde(default)]
    rules: Vec<Value>,
    #[serde(default)]
    scenes: Vec<SceneRow>,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct SceneRow {
    id: String,
    content: String,
    direction: Option<String>,
    background_image_path: Option<String>,
    created_at: i64,
    selected_variant_id: Option<String>,
    #[serde(default)]
    variants: Vec<SceneVariantRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct SceneVariantRow {
    id: String,
    content: String,
    direction: Option<String>,
    created_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct CharacterLorebookRow {
    character_id: String,
    lorebook_id: String,
    #[serde(default = "default_true", deserialize_with = "nullable_true")]
    enabled: bool,
    #[serde(default)]
    display_order: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct GroupRow {
    id: String,
    name: String,
    #[serde(default = "empty_json_array")]
    character_ids: String,
    #[serde(default = "empty_json_array")]
    muted_character_ids: String,
    persona_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(default, deserialize_with = "nullable_bool")]
    archived: bool,
    #[serde(default = "default_chat_type")]
    chat_type: String,
    starting_scene: Option<String>,
    background_image_path: Option<String>,
    #[serde(default = "empty_json_array")]
    lorebook_ids: String,
    #[serde(default, deserialize_with = "nullable_bool")]
    disable_character_lorebooks: bool,
    chat_appearance: Option<String>,
    #[serde(default = "default_speaker_selection")]
    speaker_selection_method: String,
    #[serde(default = "default_memory")]
    memory_type: String,
    character_model_overrides: Option<String>,
    group_chat_prompt_template_id: Option<String>,
    group_chat_roleplay_prompt_template_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct GroupStartingSceneRow {
    id: String,
    content: String,
    direction: Option<String>,
    #[serde(alias = "backgroundImagePath")]
    background_image_path: Option<String>,
    #[serde(alias = "createdAt")]
    created_at: i64,
    #[serde(alias = "selectedVariantId")]
    selected_variant_id: Option<String>,
    #[serde(default)]
    variants: Vec<GroupSceneVariantRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct GroupSceneVariantRow {
    id: String,
    content: String,
    direction: Option<String>,
    #[serde(alias = "createdAt")]
    created_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_authored(
    mut configuration: LegacyBackupConfigurationPlan,
) -> Result<LegacyBackupAuthoredPlan, LegacyBackupAuthoredError> {
    let persona_rows: Vec<PersonaRow> = document_rows(
        &configuration,
        LegacyBackupDocumentKind::Personas,
        CHARACTER_LIMIT,
    )?;
    let lorebook_rows: Vec<LorebookRow> = document_rows(
        &configuration,
        LegacyBackupDocumentKind::Lorebooks,
        CHARACTER_LIMIT,
    )?;
    let character_rows: Vec<CharacterRow> = document_rows(
        &configuration,
        LegacyBackupDocumentKind::Characters,
        CHARACTER_LIMIT,
    )?;
    let group_rows: Vec<GroupRow> = document_rows(
        &configuration,
        LegacyBackupDocumentKind::GroupCharacters,
        CHARACTER_LIMIT,
    )?;
    let explicit_binding_rows: Option<Vec<CharacterLorebookRow>> = optional_rows(
        &configuration,
        LegacyBackupDocumentKind::CharacterLorebooks,
        SCENE_LIMIT,
    )?;
    for kind in [
        LegacyBackupDocumentKind::Personas,
        LegacyBackupDocumentKind::Lorebooks,
        LegacyBackupDocumentKind::Characters,
        LegacyBackupDocumentKind::GroupCharacters,
    ] {
        if !configuration
            .source
            .documents
            .iter()
            .any(|document| document.kind == kind)
        {
            configuration
                .notices
                .push(notice(LegacyBackupConversionNoticeKind::Absent, kind, "$"));
        }
    }

    let personas = map_personas(persona_rows, &mut configuration.notices)?;
    let lorebooks = map_lorebooks(lorebook_rows, &mut configuration.notices)?;
    let lorebook_ids = lorebooks
        .lorebooks
        .iter()
        .map(|book| book.id)
        .collect::<BTreeSet<_>>();
    validate_persona_lorebooks(&personas, &lorebook_ids)?;
    let persona_lorebooks = map_persona_bindings(&personas);
    let characters = map_characters(
        &configuration.provider_models,
        &configuration.prompts,
        &configuration.chat_templates,
        character_rows,
        &lorebook_ids,
        &mut configuration.notices,
    )?;
    let character_lorebooks = map_character_bindings(
        &characters,
        &lorebook_ids,
        explicit_binding_rows,
        &mut configuration.notices,
    )?;
    validate_starter_owners(&characters, &configuration.chat_templates)?;
    let groups = map_groups(
        &characters,
        &personas,
        &lorebooks,
        &configuration.provider_models,
        &configuration.prompts,
        group_rows,
        &mut configuration.notices,
    )?;
    let group_lorebooks = map_group_bindings(&groups);
    configuration.notices.sort();
    configuration.notices.dedup();
    Ok(LegacyBackupAuthoredPlan {
        personas,
        lorebooks,
        characters,
        character_lorebooks,
        persona_lorebooks,
        groups,
        group_lorebooks,
        notices: configuration.notices.clone(),
        configuration,
    })
}

fn map_personas(
    rows: Vec<PersonaRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyPersonaPlan, LegacyBackupAuthoredError> {
    let mut personas = Vec::with_capacity(rows.len());
    let mut ids = BTreeSet::new();
    let mut default_persona_id = None;
    for row in rows {
        report_extra(LegacyBackupDocumentKind::Personas, &row.extra, notices);
        let id = parse_id(&row.id, LegacyBackupDocumentKind::Personas, "id")?;
        require_unique(&mut ids, id, LegacyBackupDocumentKind::Personas, "id")?;
        require_nonblank(&row.title, LegacyBackupDocumentKind::Personas, "title")?;
        require_nonblank(
            &row.description,
            LegacyBackupDocumentKind::Personas,
            "description",
        )?;
        validate_timestamps(
            row.created_at,
            row.updated_at,
            LegacyBackupDocumentKind::Personas,
        )?;
        if row.is_default && default_persona_id.replace(id).is_some() {
            return Err(malformed(LegacyBackupDocumentKind::Personas, "is_default"));
        }
        personas.push(LegacyPersonaCandidate {
            id,
            title: row.title,
            description: row.description,
            nickname: normalize(row.nickname),
            avatar: media(
                row.avatar_path,
                LegacyBackupDocumentKind::Personas,
                "avatar_path",
            )?,
            avatar_crop: legacy_crop(
                row.avatar_crop_x,
                row.avatar_crop_y,
                row.avatar_crop_scale,
                LegacyBackupDocumentKind::Personas,
            )?,
            design_description: normalize(row.design_description),
            design_references: media_list(
                row.design_reference_image_ids,
                LegacyBackupDocumentKind::Personas,
                "design_reference_image_ids",
            )?,
            image_recommendation: recommendation(
                row.lora_name,
                row.lora_strength,
                LegacyBackupDocumentKind::Personas,
            )?,
            active_lorebook_ids: id_list(
                &row.active_lorebook_ids,
                LegacyBackupDocumentKind::Personas,
                "active_lorebook_ids",
            )?,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        });
    }
    Ok(LegacyPersonaPlan {
        personas,
        default_persona_id,
    })
}

fn map_lorebooks(
    rows: Vec<LorebookRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyLorebookPlan, LegacyBackupAuthoredError> {
    let mut books = Vec::with_capacity(rows.len());
    let mut book_ids = BTreeSet::new();
    let mut entry_count = 0usize;
    for row in rows {
        report_extra(LegacyBackupDocumentKind::Lorebooks, &row.extra, notices);
        let id = parse_id(&row.id, LegacyBackupDocumentKind::Lorebooks, "id")?;
        require_unique(&mut book_ids, id, LegacyBackupDocumentKind::Lorebooks, "id")?;
        require_nonblank(&row.name, LegacyBackupDocumentKind::Lorebooks, "name")?;
        validate_timestamps(
            row.created_at,
            row.updated_at,
            LegacyBackupDocumentKind::Lorebooks,
        )?;
        if row.entries.len() > CHILD_LIMIT {
            return Err(limit(LegacyBackupDocumentKind::Lorebooks));
        }
        entry_count = entry_count
            .checked_add(row.entries.len())
            .ok_or_else(|| limit(LegacyBackupDocumentKind::Lorebooks))?;
        if entry_count > SCENE_LIMIT {
            return Err(limit(LegacyBackupDocumentKind::Lorebooks));
        }
        let detection_policy = match row.keyword_detection_mode.as_str() {
            "recent_message_window" => LegacyLorebookDetectionPolicy::RecentMessageWindow,
            "latest_user_message" => LegacyLorebookDetectionPolicy::LatestUserMessage,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::Lorebooks,
                    "keyword_detection_mode",
                ));
            }
        };
        let mut entry_ids = BTreeSet::new();
        let mut entries = Vec::with_capacity(row.entries.len());
        for entry in row.entries {
            report_extra(LegacyBackupDocumentKind::Lorebooks, &entry.extra, notices);
            let entry_id = parse_id(&entry.id, LegacyBackupDocumentKind::Lorebooks, "entries.id")?;
            require_unique(
                &mut entry_ids,
                entry_id,
                LegacyBackupDocumentKind::Lorebooks,
                "entries.id",
            )?;
            validate_timestamps(
                entry.created_at,
                entry.updated_at,
                LegacyBackupDocumentKind::Lorebooks,
            )?;
            let keywords = serde_json::from_str::<Vec<String>>(&entry.keywords)
                .map_err(|_| malformed(LegacyBackupDocumentKind::Lorebooks, "entries.keywords"))?;
            let match_mode = match entry.keyword_match_mode.as_str() {
                "literal" => LegacyKeywordMatchMode::Literal,
                "regex" => LegacyKeywordMatchMode::Regex,
                _ => {
                    return Err(malformed(
                        LegacyBackupDocumentKind::Lorebooks,
                        "entries.keyword_match_mode",
                    ));
                }
            };
            entries.push(LegacyLorebookEntryCandidate {
                id: entry_id,
                title: entry.title,
                enabled: entry.enabled,
                always_active: entry.always_active,
                keywords,
                case_sensitive: entry.case_sensitive,
                match_mode,
                content: entry.content,
                priority: entry.priority,
                display_order: entry.display_order,
                created_at: TimestampMillis::new(entry.created_at),
                updated_at: TimestampMillis::new(entry.updated_at),
            });
        }
        entries.sort_by_key(|entry| (entry.display_order, entry.created_at, entry.id));
        books.push(LegacyLorebookCandidate {
            id,
            name: row.name,
            avatar: media(
                row.avatar_path,
                LegacyBackupDocumentKind::Lorebooks,
                "avatar_path",
            )?,
            detection_policy,
            entries,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        });
    }
    Ok(LegacyLorebookPlan { lorebooks: books })
}

fn map_characters(
    provider_models: &crate::LegacyProviderModelPlan,
    prompts: &crate::LegacyPromptPlan,
    chat_templates: &[crate::LegacyBackupChatTemplateCandidate],
    rows: Vec<CharacterRow>,
    lorebook_ids: &BTreeSet<LorebookId>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCharacterCandidate>, LegacyBackupAuthoredError> {
    let model_ids = provider_models
        .model_profiles
        .iter()
        .map(|model| model.id)
        .collect::<BTreeSet<_>>();
    let prompt_ids = prompts
        .prompts
        .iter()
        .map(|prompt| prompt.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut characters = Vec::with_capacity(rows.len());
    for row in rows {
        report_extra(LegacyBackupDocumentKind::Characters, &row.extra, notices);
        let id = parse_id(&row.id, LegacyBackupDocumentKind::Characters, "id")?;
        require_unique(&mut ids, id, LegacyBackupDocumentKind::Characters, "id")?;
        require_nonblank(&row.name, LegacyBackupDocumentKind::Characters, "name")?;
        validate_timestamps(
            row.created_at,
            row.updated_at,
            LegacyBackupDocumentKind::Characters,
        )?;
        let profile = CharacterProfile {
            name: row.name,
            nickname: normalize(row.nickname),
            description: normalize(row.description.clone()),
            definition: normalize(row.definition.or(row.description)),
            design_description: normalize(row.design_description),
        };
        profile
            .validate()
            .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, "profile"))?;
        let provenance = CharacterProvenance {
            creator: normalize(row.creator),
            creator_notes: normalize(row.creator_notes),
            localized_creator_notes: parse_string_map(
                row.creator_notes_multilingual,
                "creator_notes_multilingual",
            )?,
            sources: parse_string_vec(row.source, "source")?,
            tags: parse_string_vec(row.tags, "tags")?,
        };
        provenance
            .validate()
            .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, "provenance"))?;
        let scenes = map_scenes(row.scenes, notices)?;
        let scene_ids = scenes.iter().map(|scene| scene.id).collect::<BTreeSet<_>>();
        let default_scene_id = optional_id(
            row.default_scene_id,
            LegacyBackupDocumentKind::Characters,
            "default_scene_id",
        )?;
        if default_scene_id.is_some_and(|scene| !scene_ids.contains(&scene)) {
            return Err(orphan(
                LegacyBackupDocumentKind::Characters,
                "default_scene_id",
            ));
        }
        let model_profile_id = normalize(row.default_model_id).map(|source_id| {
            crate::legacy_backup_configuration::canonical_model_id(
                &source_id,
                notices,
                "characters.default_model_id",
            )
        });
        if model_profile_id.is_some_and(|model| !model_ids.contains(&model)) {
            return Err(orphan(
                LegacyBackupDocumentKind::Characters,
                "default_model_id",
            ));
        }
        for (field, prompt) in [
            ("prompt_template_id", row.prompt_template_id.as_deref()),
            (
                "group_chat_prompt_template_id",
                row.group_chat_prompt_template_id.as_deref(),
            ),
            (
                "group_chat_roleplay_prompt_template_id",
                row.group_chat_roleplay_prompt_template_id.as_deref(),
            ),
        ] {
            if prompt.is_some_and(|prompt| !prompt_ids.contains(prompt)) {
                return Err(orphan(LegacyBackupDocumentKind::Characters, field));
            }
        }
        let active_lorebooks = id_list(
            &row.active_lorebook_ids,
            LegacyBackupDocumentKind::Characters,
            "active_lorebook_ids",
        )?;
        validate_ids(
            &active_lorebooks,
            lorebook_ids,
            LegacyBackupDocumentKind::Characters,
            "active_lorebook_ids",
        )?;
        let starters = map_starters(prompts, chat_templates, id, &scene_ids, lorebook_ids)?;
        if row
            .default_chat_template_id
            .as_ref()
            .is_some_and(|default| !starters.iter().any(|starter| &starter.source_id == default))
        {
            return Err(orphan(
                LegacyBackupDocumentKind::Characters,
                "default_chat_template_id",
            ));
        }
        let rules = row
            .rules
            .into_iter()
            .map(|rule| {
                rule.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| malformed(LegacyBackupDocumentKind::Characters, "rules"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if rules.len() > CHILD_LIMIT || scenes.len() > CHILD_LIMIT {
            return Err(limit(LegacyBackupDocumentKind::Characters));
        }
        let defaults = LegacyBackupCharacterDefaults {
            interaction_mode: match row.mode.as_str() {
                "roleplay" => InteractionMode::Roleplay,
                "companion" => InteractionMode::Companion,
                _ => return Err(malformed(LegacyBackupDocumentKind::Characters, "mode")),
            },
            memory_policy: match row.memory_type.as_str() {
                "manual" => MemoryPolicy::Manual,
                "dynamic" => MemoryPolicy::Dynamic,
                _ => {
                    return Err(malformed(
                        LegacyBackupDocumentKind::Characters,
                        "memory_type",
                    ));
                }
            },
            model_profile_id,
            default_scene_id,
            default_starter_source_id: normalize(row.default_chat_template_id),
            direct_prompt_source_id: normalize(row.prompt_template_id),
            group_conversation_prompt_source_id: normalize(row.group_chat_prompt_template_id),
            group_roleplay_prompt_source_id: normalize(row.group_chat_roleplay_prompt_template_id),
            system_prompt: normalize(row.system_prompt),
            companion: row.companion.and_then(normalize_value),
            voice_config: parse_json(row.voice_config, "voice_config")?,
            voice_autoplay: row.voice_autoplay,
        };
        let presentation = LegacyBackupCharacterPresentation {
            card_style: match row.card_type.as_str() {
                "circle" => CardStyle::Circle,
                "banner" => CardStyle::Banner,
                _ => return Err(malformed(LegacyBackupDocumentKind::Characters, "card_type")),
            },
            avatar_crop: crop(row.avatar_crop_x, row.avatar_crop_y, row.avatar_crop_scale)?,
            banner_crop: crop(row.banner_crop_x, row.banner_crop_y, row.banner_crop_scale)?,
            disable_gradient: row.disable_avatar_gradient,
            gradient_source: match row.avatar_gradient_source.as_str() {
                "base" => GradientSource::Base,
                "round" => GradientSource::Round,
                _ => {
                    return Err(malformed(
                        LegacyBackupDocumentKind::Characters,
                        "avatar_gradient_source",
                    ));
                }
            },
            custom_gradient_enabled: row.custom_gradient_enabled,
            custom_gradient_colors: parse_string_vec(
                row.custom_gradient_colors,
                "custom_gradient_colors",
            )?,
            primary_text_color: normalize(row.custom_text_color),
            secondary_text_color: normalize(row.custom_text_secondary),
            chat_appearance: parse_json(row.chat_appearance, "chat_appearance")?,
        };
        validate_presentation(&presentation)?;
        characters.push(LegacyBackupCharacterCandidate {
            id,
            profile,
            provenance,
            scenario: normalize(row.scenario),
            rules,
            defaults,
            presentation,
            media: LegacyBackupCharacterMedia {
                avatar: media(
                    row.avatar_path,
                    LegacyBackupDocumentKind::Characters,
                    "avatar_path",
                )?,
                background: media(
                    row.background_image_path,
                    LegacyBackupDocumentKind::Characters,
                    "background_image_path",
                )?,
                design_references: media_list(
                    row.design_reference_image_ids,
                    LegacyBackupDocumentKind::Characters,
                    "design_reference_image_ids",
                )?,
            },
            image_recommendation: recommendation(
                row.lora_name,
                row.lora_strength,
                LegacyBackupDocumentKind::Characters,
            )?,
            active_lorebook_ids: active_lorebooks,
            scenes,
            starters,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        });
    }
    Ok(characters)
}

fn map_scenes(
    rows: Vec<SceneRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupSceneCandidate>, LegacyBackupAuthoredError> {
    let mut ids = BTreeSet::new();
    let mut total_variants = 0usize;
    let mut scenes = Vec::with_capacity(rows.len());
    for (ordinal, row) in rows.into_iter().enumerate() {
        report_extra(LegacyBackupDocumentKind::Characters, &row.extra, notices);
        if row.variants.len() > CHILD_LIMIT {
            return Err(limit(LegacyBackupDocumentKind::Characters));
        }
        total_variants += row.variants.len();
        if total_variants > SCENE_LIMIT {
            return Err(limit(LegacyBackupDocumentKind::Characters));
        }
        let id = parse_id(&row.id, LegacyBackupDocumentKind::Characters, "scenes.id")?;
        require_unique(
            &mut ids,
            id,
            LegacyBackupDocumentKind::Characters,
            "scenes.id",
        )?;
        let selected_variant_id = optional_id(
            row.selected_variant_id,
            LegacyBackupDocumentKind::Characters,
            "scenes.selected_variant_id",
        )?;
        let mut variant_ids = BTreeSet::new();
        let variants = row
            .variants
            .into_iter()
            .enumerate()
            .map(|(variant_ordinal, variant)| {
                report_extra(
                    LegacyBackupDocumentKind::Characters,
                    &variant.extra,
                    notices,
                );
                let id = parse_id(
                    &variant.id,
                    LegacyBackupDocumentKind::Characters,
                    "scenes.variants.id",
                )?;
                require_unique(
                    &mut variant_ids,
                    id,
                    LegacyBackupDocumentKind::Characters,
                    "scenes.variants.id",
                )?;
                Ok::<_, LegacyBackupAuthoredError>(LegacyBackupSceneVariantCandidate {
                    id,
                    ordinal: u32::try_from(variant_ordinal)
                        .map_err(|_| limit(LegacyBackupDocumentKind::Characters))?,
                    content: text_document(variant.content)?,
                    direction: normalize(variant.direction),
                    created_at: TimestampMillis::new(variant.created_at),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if selected_variant_id.is_some_and(|selected| !variant_ids.contains(&selected)) {
            return Err(orphan(
                LegacyBackupDocumentKind::Characters,
                "scenes.selected_variant_id",
            ));
        }
        scenes.push(LegacyBackupSceneCandidate {
            id,
            ordinal: u32::try_from(ordinal)
                .map_err(|_| limit(LegacyBackupDocumentKind::Characters))?,
            content: text_document(row.content)?,
            direction: normalize(row.direction),
            background: media(
                row.background_image_path,
                LegacyBackupDocumentKind::Characters,
                "scenes.background_image_path",
            )?,
            selected_variant_id,
            variants,
            created_at: TimestampMillis::new(row.created_at),
        });
    }
    Ok(scenes)
}

fn map_starters(
    prompts: &crate::LegacyPromptPlan,
    chat_templates: &[crate::LegacyBackupChatTemplateCandidate],
    character_id: CharacterId,
    scene_ids: &BTreeSet<SceneId>,
    lorebook_ids: &BTreeSet<LorebookId>,
) -> Result<Vec<LegacyBackupStarterCandidate>, LegacyBackupAuthoredError> {
    let prompt_ids = prompts
        .prompts
        .iter()
        .map(|prompt| prompt.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let owned = chat_templates
        .iter()
        .map(|starter| {
            let owner = parse_id::<CharacterId>(
                &starter.character_source_id,
                LegacyBackupDocumentKind::ChatTemplates,
                "character_id",
            )?;
            Ok((owner, starter))
        })
        .collect::<Result<Vec<_>, LegacyBackupAuthoredError>>()?;
    owned
        .into_iter()
        .filter(|(owner, _)| *owner == character_id)
        .map(|(_, starter)| starter)
        .enumerate()
        .map(|(ordinal, starter)| {
            let scene_id = optional_id(
                starter.scene_source_id.clone(),
                LegacyBackupDocumentKind::ChatTemplates,
                "scene_id",
            )?;
            if scene_id.is_some_and(|scene| !scene_ids.contains(&scene)) {
                return Err(orphan(LegacyBackupDocumentKind::ChatTemplates, "scene_id"));
            }
            if starter
                .prompt_source_id
                .as_deref()
                .is_some_and(|prompt| !prompt_ids.contains(prompt))
            {
                return Err(orphan(
                    LegacyBackupDocumentKind::ChatTemplates,
                    "prompt_template_id",
                ));
            }
            let parsed_lorebooks = starter
                .lorebook_source_ids
                .iter()
                .map(|id| {
                    parse_id(
                        id,
                        LegacyBackupDocumentKind::ChatTemplates,
                        "lorebook_ids_override",
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            validate_ids(
                &parsed_lorebooks,
                lorebook_ids,
                LegacyBackupDocumentKind::ChatTemplates,
                "lorebook_ids_override",
            )?;
            let messages = starter
                .messages
                .iter()
                .map(|message| {
                    Ok(StarterMessage {
                        id: stable_id::<StarterMessageId>("starter-message", &message.source_id)?,
                        role: match message.role.as_str() {
                            "user" => StarterRole::User,
                            "assistant" => StarterRole::Assistant,
                            _ => {
                                return Err(malformed(
                                    LegacyBackupDocumentKind::ChatTemplates,
                                    "messages.role",
                                ));
                            }
                        },
                        content: message.content.clone(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let starter_id = stable_id::<ConversationStarterId>("starter", &starter.source_id)?;
            ConversationStarter::new(
                starter_id,
                character_id,
                starter.name.clone(),
                u32::try_from(ordinal)
                    .map_err(|_| limit(LegacyBackupDocumentKind::ChatTemplates))?,
                messages.clone(),
                starter.created_at,
            )
            .map_err(|_| malformed(LegacyBackupDocumentKind::ChatTemplates, "starter"))?;
            Ok(LegacyBackupStarterCandidate {
                id: starter_id,
                source_id: starter.source_id.clone(),
                name: starter.name.clone(),
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| limit(LegacyBackupDocumentKind::ChatTemplates))?,
                messages,
                scene_id,
                prompt_source_id: starter.prompt_source_id.clone(),
                lorebook_ids: starter.has_lorebook_override.then_some(parsed_lorebooks),
                created_at: starter.created_at,
            })
        })
        .collect()
}

fn map_groups(
    characters: &[LegacyBackupCharacterCandidate],
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
    provider_models: &crate::LegacyProviderModelPlan,
    prompts: &crate::LegacyPromptPlan,
    rows: Vec<GroupRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupGroupCandidate>, LegacyBackupAuthoredError> {
    let character_ids = characters
        .iter()
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    let persona_ids = personas
        .personas
        .iter()
        .map(|persona| persona.id)
        .collect::<BTreeSet<_>>();
    let lorebook_ids = lorebooks
        .lorebooks
        .iter()
        .map(|lorebook| lorebook.id)
        .collect::<BTreeSet<_>>();
    let model_ids = provider_models
        .model_profiles
        .iter()
        .map(|model| model.id)
        .collect::<BTreeSet<_>>();
    let prompt_ids = prompts
        .prompts
        .iter()
        .map(|prompt| prompt.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut group_ids = BTreeSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        report_extra(
            LegacyBackupDocumentKind::GroupCharacters,
            &row.extra,
            notices,
        );
        let id = parse_id(&row.id, LegacyBackupDocumentKind::GroupCharacters, "id")?;
        require_unique(
            &mut group_ids,
            id,
            LegacyBackupDocumentKind::GroupCharacters,
            "id",
        )?;
        require_nonblank(&row.name, LegacyBackupDocumentKind::GroupCharacters, "name")?;
        validate_timestamps(
            row.created_at,
            row.updated_at,
            LegacyBackupDocumentKind::GroupCharacters,
        )?;
        let member_ids = id_list::<CharacterId>(
            &row.character_ids,
            LegacyBackupDocumentKind::GroupCharacters,
            "character_ids",
        )?;
        validate_ids(
            &member_ids,
            &character_ids,
            LegacyBackupDocumentKind::GroupCharacters,
            "character_ids",
        )?;
        let muted_ids = id_list::<CharacterId>(
            &row.muted_character_ids,
            LegacyBackupDocumentKind::GroupCharacters,
            "muted_character_ids",
        )?;
        if muted_ids.iter().any(|member| !member_ids.contains(member)) {
            return Err(orphan(
                LegacyBackupDocumentKind::GroupCharacters,
                "muted_character_ids",
            ));
        }
        let raw_overrides = row
            .character_model_overrides
            .map(|value| {
                serde_json::from_str::<BTreeMap<String, String>>(&value).map_err(|_| {
                    malformed(
                        LegacyBackupDocumentKind::GroupCharacters,
                        "character_model_overrides",
                    )
                })
            })
            .transpose()?
            .unwrap_or_default();
        let mut overrides = BTreeMap::new();
        for (member_source, model_source) in raw_overrides {
            let member = parse_id::<CharacterId>(
                &member_source,
                LegacyBackupDocumentKind::GroupCharacters,
                "character_model_overrides.character_id",
            )?;
            if !member_ids.contains(&member) {
                return Err(orphan(
                    LegacyBackupDocumentKind::GroupCharacters,
                    "character_model_overrides.character_id",
                ));
            }
            let model = crate::legacy_backup_configuration::canonical_model_id(
                &model_source,
                notices,
                "group_characters.character_model_overrides",
            );
            if !model_ids.contains(&model) {
                return Err(orphan(
                    LegacyBackupDocumentKind::GroupCharacters,
                    "character_model_overrides.model_id",
                ));
            }
            overrides.insert(member, model);
        }
        let members = member_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, character_id)| GroupMember {
                character_id,
                ordinal: ordinal as u32,
                muted: muted_ids.contains(&character_id),
                model_profile_override: overrides.get(&character_id).copied(),
            })
            .collect::<Vec<_>>();
        let persona = optional_id::<PersonaId>(
            row.persona_id,
            LegacyBackupDocumentKind::GroupCharacters,
            "persona_id",
        )?
        .map(Selection::Explicit)
        .unwrap_or(Selection::Inherit);
        if let Selection::Explicit(persona_id) = persona
            && !persona_ids.contains(&persona_id)
        {
            return Err(orphan(
                LegacyBackupDocumentKind::GroupCharacters,
                "persona_id",
            ));
        }
        let chat_mode = match row.chat_type.as_str() {
            "conversation" => ChatMode::Conversation,
            "roleplay" => ChatMode::Roleplay,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::GroupCharacters,
                    "chat_type",
                ));
            }
        };
        let speaker_selection = match row.speaker_selection_method.as_str() {
            "llm" => SpeakerSelection::Llm,
            "heuristic" => SpeakerSelection::Heuristic,
            "round_robin" => SpeakerSelection::RoundRobin,
            "director" => SpeakerSelection::Director,
            "director_action" => SpeakerSelection::DirectorAction,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::GroupCharacters,
                    "speaker_selection_method",
                ));
            }
        };
        let memory_policy = match row.memory_type.as_str() {
            "manual" => MemoryPolicy::Manual,
            "dynamic" => MemoryPolicy::Dynamic,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::GroupCharacters,
                    "memory_type",
                ));
            }
        };
        for (field, prompt) in [
            (
                "group_chat_prompt_template_id",
                row.group_chat_prompt_template_id.as_deref(),
            ),
            (
                "group_chat_roleplay_prompt_template_id",
                row.group_chat_roleplay_prompt_template_id.as_deref(),
            ),
        ] {
            if prompt.is_some_and(|prompt| !prompt_ids.contains(prompt)) {
                return Err(orphan(LegacyBackupDocumentKind::GroupCharacters, field));
            }
        }
        let bound_lorebooks = id_list::<LorebookId>(
            &row.lorebook_ids,
            LegacyBackupDocumentKind::GroupCharacters,
            "lorebook_ids",
        )?;
        validate_ids(
            &bound_lorebooks,
            &lorebook_ids,
            LegacyBackupDocumentKind::GroupCharacters,
            "lorebook_ids",
        )?;
        let starting_scene = map_group_starting_scene(row.starting_scene, notices)?;
        let status = if row.archived {
            LifecycleStatus::Archived
        } else {
            LifecycleStatus::Active
        };
        let validation = GroupProfile {
            id,
            status,
            name: row.name.clone(),
            chat_mode,
            persona: persona.clone(),
            speaker_selection,
            memory_policy,
            disable_character_lorebooks: row.disable_character_lorebooks,
            group_conversation_prompt_id: None,
            group_roleplay_prompt_id: None,
            presentation: lettuce_characters::ChatAppearanceV1::default(),
            members: members.clone(),
            starting_scene_id: starting_scene.as_ref().map(|scene| scene.id),
            background_asset_id: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        };
        validation
            .validate()
            .map_err(|_| malformed(LegacyBackupDocumentKind::GroupCharacters, "group_profile"))?;
        result.push(LegacyBackupGroupCandidate {
            id,
            status,
            name: row.name,
            chat_mode,
            persona,
            speaker_selection,
            memory_policy,
            disable_character_lorebooks: row.disable_character_lorebooks,
            group_conversation_prompt_source_id: normalize(row.group_chat_prompt_template_id),
            group_roleplay_prompt_source_id: normalize(row.group_chat_roleplay_prompt_template_id),
            chat_appearance: parse_json_document(
                row.chat_appearance,
                LegacyBackupDocumentKind::GroupCharacters,
                "chat_appearance",
            )?,
            members,
            starting_scene,
            background: media(
                row.background_image_path,
                LegacyBackupDocumentKind::GroupCharacters,
                "background_image_path",
            )?,
            lorebook_ids: bound_lorebooks,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        });
    }
    Ok(result)
}

fn map_group_starting_scene(
    value: Option<String>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Option<LegacyBackupSceneCandidate>, LegacyBackupAuthoredError> {
    let Some(value) = normalize(value) else {
        return Ok(None);
    };
    let row = serde_json::from_str::<GroupStartingSceneRow>(&value)
        .map_err(|_| malformed(LegacyBackupDocumentKind::GroupCharacters, "starting_scene"))?;
    report_extra(
        LegacyBackupDocumentKind::GroupCharacters,
        &row.extra,
        notices,
    );
    if row.variants.len() > CHILD_LIMIT {
        return Err(limit(LegacyBackupDocumentKind::GroupCharacters));
    }
    let id = parse_id(
        &row.id,
        LegacyBackupDocumentKind::GroupCharacters,
        "starting_scene.id",
    )?;
    let selected_variant_id = optional_id(
        row.selected_variant_id,
        LegacyBackupDocumentKind::GroupCharacters,
        "starting_scene.selected_variant_id",
    )?;
    let mut variant_ids = BTreeSet::new();
    let variants = row
        .variants
        .into_iter()
        .enumerate()
        .map(|(ordinal, variant)| {
            report_extra(
                LegacyBackupDocumentKind::GroupCharacters,
                &variant.extra,
                notices,
            );
            let id = parse_id(
                &variant.id,
                LegacyBackupDocumentKind::GroupCharacters,
                "starting_scene.variants.id",
            )?;
            require_unique(
                &mut variant_ids,
                id,
                LegacyBackupDocumentKind::GroupCharacters,
                "starting_scene.variants.id",
            )?;
            Ok::<_, LegacyBackupAuthoredError>(LegacyBackupSceneVariantCandidate {
                id,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| limit(LegacyBackupDocumentKind::GroupCharacters))?,
                content: text_document_for(
                    variant.content,
                    LegacyBackupDocumentKind::GroupCharacters,
                )?,
                direction: normalize(variant.direction),
                created_at: TimestampMillis::new(variant.created_at),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if selected_variant_id.is_some_and(|selected| !variant_ids.contains(&selected)) {
        return Err(orphan(
            LegacyBackupDocumentKind::GroupCharacters,
            "starting_scene.selected_variant_id",
        ));
    }
    Ok(Some(LegacyBackupSceneCandidate {
        id,
        ordinal: 0,
        content: text_document_for(row.content, LegacyBackupDocumentKind::GroupCharacters)?,
        direction: normalize(row.direction),
        background: media(
            row.background_image_path,
            LegacyBackupDocumentKind::GroupCharacters,
            "starting_scene.background_image_path",
        )?,
        selected_variant_id,
        variants,
        created_at: TimestampMillis::new(row.created_at),
    }))
}

fn map_group_bindings(
    groups: &[LegacyBackupGroupCandidate],
) -> Vec<BackupLorebookBindings<GroupId>> {
    groups
        .iter()
        .map(|group| BackupLorebookBindings {
            owner_id: group.id,
            bindings: group
                .lorebook_ids
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, lorebook_id)| LorebookBinding {
                    lorebook_id,
                    enabled: true,
                    ordinal: ordinal as u32,
                    revision: Revision::INITIAL,
                    created_at: group.created_at,
                    updated_at: group.updated_at,
                })
                .collect(),
        })
        .collect()
}

fn map_character_bindings(
    characters: &[LegacyBackupCharacterCandidate],
    lorebook_ids: &BTreeSet<LorebookId>,
    explicit: Option<Vec<CharacterLorebookRow>>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<BackupLorebookBindings<CharacterId>>, LegacyBackupAuthoredError> {
    let character_ids = characters
        .iter()
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<CharacterId, Vec<(i64, usize, LorebookId, bool)>>::new();
    if let Some(rows) = explicit {
        for (source_index, row) in rows.into_iter().enumerate() {
            report_extra(
                LegacyBackupDocumentKind::CharacterLorebooks,
                &row.extra,
                notices,
            );
            let character_id = parse_id(
                &row.character_id,
                LegacyBackupDocumentKind::CharacterLorebooks,
                "character_id",
            )?;
            let lorebook_id = parse_id(
                &row.lorebook_id,
                LegacyBackupDocumentKind::CharacterLorebooks,
                "lorebook_id",
            )?;
            if !character_ids.contains(&character_id) || !lorebook_ids.contains(&lorebook_id) {
                return Err(orphan(
                    LegacyBackupDocumentKind::CharacterLorebooks,
                    "ownership",
                ));
            }
            grouped.entry(character_id).or_default().push((
                row.display_order,
                source_index,
                lorebook_id,
                row.enabled,
            ));
        }
    } else {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Absent,
            LegacyBackupDocumentKind::CharacterLorebooks,
            "$",
        ));
        for character in characters {
            let ids = character.active_lorebook_ids.clone();
            grouped.insert(
                character.id,
                ids.into_iter()
                    .enumerate()
                    .map(|(index, id)| (index as i64, index, id, true))
                    .collect(),
            );
        }
    }
    let mut result = Vec::new();
    for character in characters {
        let mut rows = grouped.remove(&character.id).unwrap_or_default();
        rows.sort_by_key(|(order, source_index, _, _)| (*order, *source_index));
        let mut seen = BTreeSet::new();
        let bindings = rows
            .into_iter()
            .enumerate()
            .map(
                |(ordinal, (display_order, _, lorebook_id, enabled))| -> Result<_, LegacyBackupAuthoredError> {
                    if display_order != ordinal as i64 {
                        notices.push(notice(
                            LegacyBackupConversionNoticeKind::Lossy,
                            LegacyBackupDocumentKind::CharacterLorebooks,
                            "display_order",
                        ));
                    }
                    require_unique(
                        &mut seen,
                        lorebook_id,
                        LegacyBackupDocumentKind::CharacterLorebooks,
                        "lorebook_id",
                    )?;
                    Ok(LorebookBinding {
                        lorebook_id,
                        enabled,
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| limit(LegacyBackupDocumentKind::CharacterLorebooks))?,
                        revision: Revision::INITIAL,
                        created_at: character.created_at,
                        updated_at: character.updated_at,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        result.push(BackupLorebookBindings {
            owner_id: character.id,
            bindings,
        });
    }
    Ok(result)
}

fn validate_persona_lorebooks(
    plan: &LegacyPersonaPlan,
    known: &BTreeSet<LorebookId>,
) -> Result<(), LegacyBackupAuthoredError> {
    for persona in &plan.personas {
        validate_ids(
            &persona.active_lorebook_ids,
            known,
            LegacyBackupDocumentKind::Personas,
            "active_lorebook_ids",
        )?;
    }
    Ok(())
}

fn map_persona_bindings(personas: &LegacyPersonaPlan) -> Vec<BackupLorebookBindings<PersonaId>> {
    personas
        .personas
        .iter()
        .map(|persona| BackupLorebookBindings {
            owner_id: persona.id,
            bindings: persona
                .active_lorebook_ids
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, lorebook_id)| LorebookBinding {
                    lorebook_id,
                    enabled: true,
                    ordinal: ordinal as u32,
                    revision: Revision::INITIAL,
                    created_at: persona.created_at,
                    updated_at: persona.updated_at,
                })
                .collect(),
        })
        .collect()
}

fn validate_starter_owners(
    characters: &[LegacyBackupCharacterCandidate],
    starters: &[crate::LegacyBackupChatTemplateCandidate],
) -> Result<(), LegacyBackupAuthoredError> {
    let character_ids = characters
        .iter()
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    for starter in starters {
        let owner = parse_id::<CharacterId>(
            &starter.character_source_id,
            LegacyBackupDocumentKind::ChatTemplates,
            "character_id",
        )?;
        if !character_ids.contains(&owner) {
            return Err(orphan(
                LegacyBackupDocumentKind::ChatTemplates,
                "character_id",
            ));
        }
    }
    Ok(())
}

fn document_rows<T: for<'de> Deserialize<'de>>(
    configuration: &LegacyBackupConfigurationPlan,
    kind: LegacyBackupDocumentKind,
    max: usize,
) -> Result<Vec<T>, LegacyBackupAuthoredError> {
    Ok(optional_rows(configuration, kind, max)?.unwrap_or_default())
}

fn optional_rows<T: for<'de> Deserialize<'de>>(
    configuration: &LegacyBackupConfigurationPlan,
    kind: LegacyBackupDocumentKind,
    max: usize,
) -> Result<Option<Vec<T>>, LegacyBackupAuthoredError> {
    let Some(document) = configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == kind)
    else {
        return Ok(None);
    };
    let rows =
        serde_json::from_slice::<Vec<T>>(&document.bytes).map_err(|_| malformed(kind, "$"))?;
    if rows.len() > max {
        return Err(limit(kind));
    }
    Ok(Some(rows))
}

fn media(
    value: Option<String>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Option<LegacyMediaReference>, LegacyBackupAuthoredError> {
    normalize(value)
        .map(|locator| {
            require_nonblank(&locator, document, field)?;
            Ok(LegacyMediaReference { locator })
        })
        .transpose()
}

fn media_list(
    value: Option<String>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Vec<LegacyMediaReference>, LegacyBackupAuthoredError> {
    parse_string_vec(value, field)?
        .into_iter()
        .map(|locator| {
            require_nonblank(&locator, document, field)?;
            Ok(LegacyMediaReference { locator })
        })
        .collect()
}

fn recommendation(
    name: Option<String>,
    strength: Option<f64>,
    document: LegacyBackupDocumentKind,
) -> Result<Option<LegacyImageRecommendation>, LegacyBackupAuthoredError> {
    match (normalize(name), strength) {
        (None, None) => Ok(None),
        (Some(model_name), Some(strength))
            if strength.is_finite() && (0.0..=2.0).contains(&strength) =>
        {
            Ok(Some(LegacyImageRecommendation {
                model_name,
                strength,
            }))
        }
        _ => Err(malformed(document, "lora")),
    }
}

fn legacy_crop(
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
    document: LegacyBackupDocumentKind,
) -> Result<Option<LegacyCrop>, LegacyBackupAuthoredError> {
    match (x, y, scale) {
        (None, None, None) => Ok(None),
        (Some(x), Some(y), Some(scale))
            if x.is_finite() && y.is_finite() && scale.is_finite() && scale > 0.0 =>
        {
            Ok(Some(LegacyCrop { x, y, scale }))
        }
        _ => Err(malformed(document, "crop")),
    }
}

fn crop(
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
) -> Result<Option<Crop>, LegacyBackupAuthoredError> {
    let Some(value) = legacy_crop(x, y, scale, LegacyBackupDocumentKind::Characters)? else {
        return Ok(None);
    };
    Crop::new(value.x as f32, value.y as f32, value.scale as f32)
        .map(Some)
        .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, "crop"))
}

fn text_document(text: String) -> Result<SceneDocumentV1, LegacyBackupAuthoredError> {
    text_document_for(text, LegacyBackupDocumentKind::Characters)
}

fn text_document_for(
    text: String,
    document: LegacyBackupDocumentKind,
) -> Result<SceneDocumentV1, LegacyBackupAuthoredError> {
    SceneDocumentV1::new(vec![ScenePart::Text { text }])
        .map_err(|_| malformed(document, "scenes.content"))
}

fn validate_presentation(
    value: &LegacyBackupCharacterPresentation,
) -> Result<(), LegacyBackupAuthoredError> {
    CharacterPresentationV1 {
        format_version: 1,
        card_style: value.card_style,
        avatar_crop: value.avatar_crop,
        banner_crop: value.banner_crop,
        disable_gradient: value.disable_gradient,
        gradient_source: value.gradient_source,
        custom_gradient_enabled: value.custom_gradient_enabled,
        custom_gradient_colors: value.custom_gradient_colors.clone(),
        primary_text_color: value.primary_text_color.clone(),
        secondary_text_color: value.secondary_text_color.clone(),
        ..CharacterPresentationV1::default()
    }
    .validate()
    .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, "presentation"))
}

fn parse_string_map(
    value: Option<String>,
    field: &str,
) -> Result<BTreeMap<String, String>, LegacyBackupAuthoredError> {
    value
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, field))
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn parse_string_vec(
    value: Option<String>,
    field: &str,
) -> Result<Vec<String>, LegacyBackupAuthoredError> {
    value
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, field))
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn parse_json(
    value: Option<String>,
    field: &str,
) -> Result<Option<Value>, LegacyBackupAuthoredError> {
    value
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, field))
        })
        .transpose()
        .map(|value| value.and_then(normalize_value))
}

fn parse_json_document(
    value: Option<String>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Option<Value>, LegacyBackupAuthoredError> {
    value
        .map(|value| serde_json::from_str(&value).map_err(|_| malformed(document, field)))
        .transpose()
        .map(|value| value.and_then(normalize_value))
}

fn normalize_value(value: Value) -> Option<Value> {
    (!value.is_null()).then_some(value)
}

fn id_list<T: FromStr + Ord>(
    value: &str,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Vec<T>, LegacyBackupAuthoredError> {
    let values =
        serde_json::from_str::<Vec<String>>(value).map_err(|_| malformed(document, field))?;
    let ids = values
        .into_iter()
        .map(|value| parse_id(&value, document, field))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(malformed(document, field));
    }
    Ok(ids)
}

fn validate_ids<T: Ord>(
    ids: &[T],
    known: &BTreeSet<T>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<(), LegacyBackupAuthoredError> {
    if ids.iter().any(|id| !known.contains(id)) {
        Err(orphan(document, field))
    } else {
        Ok(())
    }
}

fn optional_id<T: FromStr>(
    value: Option<String>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Option<T>, LegacyBackupAuthoredError> {
    normalize(value)
        .map(|value| parse_id(&value, document, field))
        .transpose()
}

fn parse_id<T: FromStr>(
    value: &str,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<T, LegacyBackupAuthoredError> {
    value.parse().map_err(|_| malformed(document, field))
}

fn stable_id<T: FromStr>(kind: &str, source: &str) -> Result<T, LegacyBackupAuthoredError> {
    let value = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("lettuce-backup-v1:{kind}:{source}").as_bytes(),
    );
    parse_id(
        &value.to_string(),
        LegacyBackupDocumentKind::ChatTemplates,
        "messages.id",
    )
}

fn require_unique<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    value: T,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<(), LegacyBackupAuthoredError> {
    if set.insert(value) {
        Ok(())
    } else {
        Err(malformed(document, field))
    }
}

fn require_nonblank(
    value: &str,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<(), LegacyBackupAuthoredError> {
    if value.trim().is_empty() {
        Err(malformed(document, field))
    } else {
        Ok(())
    }
}

fn validate_timestamps(
    created: i64,
    updated: i64,
    document: LegacyBackupDocumentKind,
) -> Result<(), LegacyBackupAuthoredError> {
    if updated < created {
        Err(malformed(document, "timestamps"))
    } else {
        Ok(())
    }
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn report_extra(
    document: LegacyBackupDocumentKind,
    extra: &BTreeMap<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    notices.extend(extra.keys().map(|field| {
        notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            document,
            field.clone(),
        )
    }));
}

fn notice(
    kind: LegacyBackupConversionNoticeKind,
    document: LegacyBackupDocumentKind,
    field: impl Into<String>,
) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document,
        field: field.into(),
    }
}
fn malformed(
    document: LegacyBackupDocumentKind,
    field: impl Into<String>,
) -> LegacyBackupAuthoredError {
    LegacyBackupAuthoredError::Malformed {
        document,
        field: field.into(),
    }
}
fn orphan(
    document: LegacyBackupDocumentKind,
    field: impl Into<String>,
) -> LegacyBackupAuthoredError {
    LegacyBackupAuthoredError::Orphan {
        document,
        field: field.into(),
    }
}
fn limit(document: LegacyBackupDocumentKind) -> LegacyBackupAuthoredError {
    LegacyBackupAuthoredError::LimitExceeded { document }
}
fn empty_json_array() -> String {
    "[]".to_owned()
}
fn default_true() -> bool {
    true
}
fn default_detection_policy() -> String {
    "recent_message_window".to_owned()
}
fn default_keyword_match() -> String {
    "literal".to_owned()
}
fn default_card_style() -> String {
    "circle".to_owned()
}
fn default_mode() -> String {
    "roleplay".to_owned()
}
fn default_memory() -> String {
    "manual".to_owned()
}
fn default_gradient_source() -> String {
    "base".to_owned()
}
fn default_chat_type() -> String {
    "conversation".to_owned()
}
fn default_speaker_selection() -> String {
    "llm".to_owned()
}

fn bool_or_integer<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(false),
        Value::Bool(value) => Ok(value),
        Value::Number(value) => value
            .as_i64()
            .map(|value| value != 0)
            .ok_or_else(|| serde::de::Error::custom("expected integer")),
        _ => Err(serde::de::Error::custom("expected boolean or integer")),
    }
}

fn nullable_bool<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Option::<bool>::deserialize(deserializer).map(Option::unwrap_or_default)
}

fn nullable_true<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Option::<bool>::deserialize(deserializer).map(|value| value.unwrap_or(true))
}

impl From<LegacyBackupConfigurationError> for LegacyBackupAuthoredError {
    fn from(value: LegacyBackupConfigurationError) -> Self {
        match value {
            LegacyBackupConfigurationError::Malformed { document, field } => {
                Self::Malformed { document, field }
            }
            LegacyBackupConfigurationError::LimitExceeded { document } => {
                Self::LimitExceeded { document }
            }
            LegacyBackupConfigurationError::Orphan { document, field } => {
                Self::Orphan { document, field }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::*;
    use crate::{LegacyBackupDocument, LegacyBackupInventory, plan_legacy_backup_configuration};

    fn id(value: u128) -> String {
        uuid::Uuid::from_u128(value).to_string()
    }

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture should encode")),
        }
    }

    fn inventory(documents: Vec<LegacyBackupDocument>) -> crate::LegacyBackupInventory {
        LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("22".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        }
    }

    fn plan(
        documents: Vec<LegacyBackupDocument>,
    ) -> Result<LegacyBackupAuthoredPlan, LegacyBackupAuthoredError> {
        let configuration = plan_legacy_backup_configuration(inventory(documents))
            .expect("configuration should plan");
        plan_legacy_backup_authored(configuration)
    }

    #[test]
    fn authored_plan_preserves_profiles_children_media_and_disabled_bindings() {
        let persona_id = id(1);
        let character_id = id(2);
        let lorebook_id = id(3);
        let entry_id = id(4);
        let scene_id = id(5);
        let variant_id = id(6);
        let documents = vec![
            document(
                LegacyBackupDocumentKind::Personas,
                json!([{
                    "id": persona_id,
                    "title": "Writer",
                    "description": "Writes the user role",
                    "avatar_path": "avatars/persona.png",
                    "design_reference_image_ids": "[\"images/persona-ref.png\"]",
                    "active_lorebook_ids": format!("[\"{lorebook_id}\"]"),
                    "is_default": true,
                    "created_at": 10,
                    "updated_at": 11
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Lorebooks,
                json!([{
                    "id": lorebook_id,
                    "name": "World",
                    "avatar_path": "avatars/world.png",
                    "keyword_detection_mode": "latest_user_message",
                    "created_at": 7,
                    "updated_at": 8,
                    "entries": [{
                        "id": entry_id,
                        "title": "Harbor",
                        "keywords": "[\"harbor\"]",
                        "content": "A sheltered harbor",
                        "display_order": 4,
                        "created_at": 7,
                        "updated_at": 8
                    }]
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                json!([{
                    "id": character_id,
                    "name": "Mira",
                    "avatar_path": "avatars/mira.png",
                    "background_image_path": "images/mira-bg.png",
                    "design_reference_image_ids": "[\"images/mira-ref.png\"]",
                    "description": "Navigator",
                    "scenario": "At the harbor",
                    "creator": "Author",
                    "creator_notes_multilingual": "{\"de\":\"Localized note\"}",
                    "source": "[\"card\"]",
                    "tags": "[\"adventure\"]",
                    "default_scene_id": scene_id,
                    "mode": "companion",
                    "companion": {"legacyMood": "calm"},
                    "memory_type": "dynamic",
                    "active_lorebook_ids": format!("[\"{lorebook_id}\"]"),
                    "system_prompt": "Keep watch",
                    "voice_config": "{\"provider\":\"legacy\",\"voice\":\"mira\"}",
                    "voice_autoplay": 1,
                    "custom_gradient_colors": "[\"#112233\"]",
                    "chat_appearance": "{\"fontSize\":\"medium\"}",
                    "default_chat_template_id": "starter-1",
                    "rules": ["Stay in character"],
                    "scenes": [{
                        "id": scene_id,
                        "content": "The harbor at dawn",
                        "background_image_path": "images/harbor.png",
                        "created_at": 12,
                        "selected_variant_id": variant_id,
                        "variants": [{
                            "id": variant_id,
                            "content": "The harbor at night",
                            "created_at": 13
                        }]
                    }],
                    "created_at": 10,
                    "updated_at": 14
                }]),
            ),
            document(
                LegacyBackupDocumentKind::ChatTemplates,
                json!([{
                    "id": "starter-1",
                    "character_id": character_id,
                    "name": "Arrival",
                    "scene_id": scene_id,
                    "lorebook_ids_override": "[]",
                    "created_at": 15,
                    "messages": [{
                        "id": "message-1",
                        "idx": 0,
                        "role": "assistant",
                        "content": "Welcome aboard"
                    }]
                }]),
            ),
            document(
                LegacyBackupDocumentKind::CharacterLorebooks,
                json!([{
                    "character_id": character_id,
                    "lorebook_id": lorebook_id,
                    "enabled": false,
                    "display_order": 9
                }]),
            ),
        ];

        let plan = plan(documents).expect("authored graph should plan");
        assert_eq!(
            plan.personas.default_persona_id,
            Some(
                parse_id(&persona_id, LegacyBackupDocumentKind::Personas, "id")
                    .expect("fixture persona id"),
            )
        );
        assert_eq!(
            plan.personas.personas[0].design_references[0].locator,
            "images/persona-ref.png"
        );
        assert_eq!(plan.lorebooks.lorebooks[0].entries[0].display_order, 4);
        let character = &plan.characters[0];
        assert_eq!(character.profile.definition.as_deref(), Some("Navigator"));
        assert_eq!(character.scenario.as_deref(), Some("At the harbor"));
        assert_eq!(character.rules, ["Stay in character"]);
        assert_eq!(
            character
                .media
                .background
                .as_ref()
                .map(|value| value.locator.as_str()),
            Some("images/mira-bg.png")
        );
        assert_eq!(character.scenes[0].variants[0].id.to_string(), variant_id);
        assert_eq!(character.starters[0].lorebook_ids, Some(Vec::new()));
        assert!(character.defaults.voice_autoplay);
        assert_eq!(plan.character_lorebooks[0].bindings.len(), 1);
        assert!(!plan.character_lorebooks[0].bindings[0].enabled);
        assert_eq!(plan.persona_lorebooks[0].bindings.len(), 1);
        assert!(plan.persona_lorebooks[0].bindings[0].enabled);
    }

    #[test]
    fn missing_character_binding_document_uses_the_embedded_order() {
        let character_id = id(30);
        let first_lorebook = id(31);
        let second_lorebook = id(32);
        let plan = plan(vec![
            document(
                LegacyBackupDocumentKind::Lorebooks,
                json!([
                    {"id": first_lorebook, "name": "First", "created_at": 1, "updated_at": 1},
                    {"id": second_lorebook, "name": "Second", "created_at": 1, "updated_at": 1}
                ]),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                json!([{
                    "id": character_id,
                    "name": "Mira",
                    "active_lorebook_ids": format!("[\"{second_lorebook}\",\"{first_lorebook}\"]"),
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
        ])
        .expect("embedded bindings should plan");
        let bindings = &plan.character_lorebooks[0].bindings;
        assert_eq!(bindings[0].lorebook_id.to_string(), second_lorebook);
        assert_eq!(bindings[1].lorebook_id.to_string(), first_lorebook);
        assert_eq!(bindings[0].ordinal, 0);
        assert_eq!(bindings[1].ordinal, 1);
    }

    #[test]
    fn group_profiles_preserve_director_policy_members_scene_and_media() {
        let first_character = id(40);
        let second_character = id(41);
        let persona_id = id(42);
        let lorebook_id = id(43);
        let group_id = id(44);
        let scene_id = id(45);
        let variant_id = id(46);
        let starting_scene = serde_json::to_string(&json!({
            "id": scene_id,
            "content": "The crew meets",
            "direction": "At dusk",
            "backgroundImagePath": "images/group-scene.png",
            "createdAt": 8,
            "selectedVariantId": variant_id,
            "variants": [{
                "id": variant_id,
                "content": "The crew meets at dawn",
                "createdAt": 9
            }]
        }))
        .expect("starting scene fixture");
        let plan = plan(vec![
            document(
                LegacyBackupDocumentKind::Personas,
                json!([{
                    "id": persona_id,
                    "title": "Director",
                    "description": "Represents the user",
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Lorebooks,
                json!([{
                    "id": lorebook_id,
                    "name": "World",
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                json!([
                    {"id": first_character, "name": "Mira", "created_at": 1, "updated_at": 1},
                    {"id": second_character, "name": "Sol", "created_at": 1, "updated_at": 1}
                ]),
            ),
            document(
                LegacyBackupDocumentKind::GroupCharacters,
                json!([{
                    "id": group_id,
                    "name": "Crew",
                    "character_ids": format!("[\"{first_character}\",\"{second_character}\"]"),
                    "muted_character_ids": format!("[\"{second_character}\"]"),
                    "persona_id": persona_id,
                    "created_at": 5,
                    "updated_at": 10,
                    "archived": true,
                    "chat_type": "roleplay",
                    "starting_scene": starting_scene,
                    "background_image_path": "images/group.png",
                    "lorebook_ids": format!("[\"{lorebook_id}\"]"),
                    "disable_character_lorebooks": true,
                    "chat_appearance": "{\"fontSize\":\"large\"}",
                    "speaker_selection_method": "director",
                    "memory_type": "dynamic"
                }]),
            ),
        ])
        .expect("group profile should plan");

        let group = &plan.groups[0];
        assert_eq!(group.status, LifecycleStatus::Archived);
        assert_eq!(group.speaker_selection, SpeakerSelection::Director);
        assert_eq!(
            group.persona,
            Selection::Explicit(
                parse_id(&persona_id, LegacyBackupDocumentKind::Personas, "id")
                    .expect("persona id")
            )
        );
        assert_eq!(group.members.len(), 2);
        assert!(!group.members[0].muted);
        assert!(group.members[1].muted);
        assert_eq!(
            group
                .starting_scene
                .as_ref()
                .map(|scene| scene.id.to_string()),
            Some(scene_id)
        );
        assert_eq!(
            group
                .background
                .as_ref()
                .map(|media| media.locator.as_str()),
            Some("images/group.png")
        );
        assert_eq!(
            plan.group_lorebooks[0].bindings[0].lorebook_id.to_string(),
            lorebook_id
        );
    }

    #[test]
    fn missing_authored_documents_are_explicit_and_keep_the_source_plan() {
        let plan = plan(Vec::new()).expect("optional authored documents may be absent");
        assert!(plan.characters.is_empty());
        assert_eq!(plan.configuration.source.version, 1);
        for kind in [
            LegacyBackupDocumentKind::Personas,
            LegacyBackupDocumentKind::Lorebooks,
            LegacyBackupDocumentKind::Characters,
            LegacyBackupDocumentKind::GroupCharacters,
        ] {
            assert!(plan.notices.iter().any(|notice| notice.kind
                == LegacyBackupConversionNoticeKind::Absent
                && notice.document == kind));
        }
    }

    #[test]
    fn orphaned_selected_variant_rejects_the_complete_plan() {
        let character_id = id(20);
        let scene_id = id(21);
        let missing_variant_id = id(22);
        let error = plan(vec![document(
            LegacyBackupDocumentKind::Characters,
            json!([{
                "id": character_id,
                "name": "Mira",
                "scenes": [{
                    "id": scene_id,
                    "content": "Scene",
                    "created_at": 1,
                    "selected_variant_id": missing_variant_id
                }],
                "created_at": 1,
                "updated_at": 1
            }]),
        )])
        .expect_err("orphaned selected variant must reject");
        assert_eq!(
            error,
            LegacyBackupAuthoredError::Orphan {
                document: LegacyBackupDocumentKind::Characters,
                field: "scenes.selected_variant_id".into(),
            }
        );
    }
}
