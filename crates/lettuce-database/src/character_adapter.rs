//! SQLite persistence for the authored character graph.
//!
//! This module deliberately contains the storage representation and never
//! exposes it as part of the public database API. The domain ports remain the
//! only contract consumed by the composition root.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lettuce_characters::{
    Character, CharacterDefaults, CharacterDependencyReader, CharacterMedia, CharacterMediaLink,
    CharacterMediaSlot, CharacterPresentationV1, CharacterProfile, CharacterProvenance,
    CharacterRepository, CharacterSearch, ConversationStarter, ConversationStarterDraftUpdate,
    CreateCharacterPlan, DependencyReference, DependencyReport, IdRemap, ImageRecommendation,
    LifecycleStatus, ProfileDuplicateRepository, ProfileDuplicateRequest, ProfileDuplicateResult,
    RepositoryError, RetainedExternalReferences, Scene, SceneAssetLink, SceneAssetSlot,
    SceneDocumentV1, SceneDraftUpdate, SceneOwner, ScenePart, SceneRepository, SceneVariant,
    SceneVariantDraftUpdate, Selection, StarterMessage, StarterRepository, StarterRole,
    UnresolvedLegacyReference, WidgetImageSource, WidgetNode,
};
use lettuce_companions::{SoulOwner, initial_soul_state};
use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, LorebookId, Page, PageRequest, PromptDocumentId,
    Revision, SceneAssetLinkId, SceneId, SceneVariantId, StarterMessageId, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::Database;

const PROFILE_VERSION: u32 = 1;
const PROVENANCE_VERSION: u32 = 1;
const DEFAULTS_VERSION: u32 = 1;
const PRESENTATION_VERSION: u32 = 1;
const RECOMMENDATION_VERSION: u32 = 1;
const DOCUMENT_VERSION: u32 = 1;
const LOREBOOK_VERSION: u32 = 1;
const CHARACTER_CURSOR_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    format_version: u32,
    value: T,
}

fn encode<T: Serialize>(value: &T, version: u32) -> Result<String, RepositoryError> {
    serde_json::to_string(&Envelope {
        format_version: version,
        value,
    })
    .map_err(|_| {
        RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
            field: "character.encoded",
        })
    })
}

fn decode<T: DeserializeOwned, S: AsRef<str>>(
    payload: S,
    version: u32,
) -> Result<T, rusqlite::Error> {
    let envelope: Envelope<T> =
        serde_json::from_str(payload.as_ref()).map_err(|_| rusqlite::Error::InvalidQuery)?;
    if envelope.format_version != version {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(envelope.value)
}

fn invalid() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn db_error(error: rusqlite::Error) -> RepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => {
            RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
                field: "character.storage",
            })
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == 1555 || code.extended_code == 2067 =>
        {
            RepositoryError::AlreadyExists
        }
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 787 => {
            RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidReference {
                field: "character.reference",
            })
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
                field: "character.storage",
            })
        }
        _ => RepositoryError::Storage,
    }
}

fn validate(value: Result<(), lettuce_characters::ValidationError>) -> Result<(), RepositoryError> {
    value.map_err(RepositoryError::Invalid)
}

fn status_name(value: LifecycleStatus) -> &'static str {
    match value {
        LifecycleStatus::Active => "active",
        LifecycleStatus::Archived => "archived",
    }
}

fn status_from_name(value: &str) -> Result<LifecycleStatus, rusqlite::Error> {
    match value {
        "active" => Ok(LifecycleStatus::Active),
        "archived" => Ok(LifecycleStatus::Archived),
        _ => Err(invalid()),
    }
}

fn canonical_name(value: &str) -> String {
    value.to_lowercase()
}

fn interaction_name(value: lettuce_characters::InteractionMode) -> &'static str {
    match value {
        lettuce_characters::InteractionMode::Roleplay => "roleplay",
        lettuce_characters::InteractionMode::Companion => "companion",
    }
}

fn interaction_from_name(
    value: &str,
) -> Result<lettuce_characters::InteractionMode, rusqlite::Error> {
    match value {
        "roleplay" => Ok(lettuce_characters::InteractionMode::Roleplay),
        "companion" => Ok(lettuce_characters::InteractionMode::Companion),
        _ => Err(invalid()),
    }
}

fn memory_name(value: lettuce_characters::MemoryPolicy) -> &'static str {
    match value {
        lettuce_characters::MemoryPolicy::Manual => "manual",
        lettuce_characters::MemoryPolicy::Dynamic => "dynamic",
    }
}

fn memory_from_name(value: &str) -> Result<lettuce_characters::MemoryPolicy, rusqlite::Error> {
    match value {
        "manual" => Ok(lettuce_characters::MemoryPolicy::Manual),
        "dynamic" => Ok(lettuce_characters::MemoryPolicy::Dynamic),
        _ => Err(invalid()),
    }
}

fn media_slot_name(value: CharacterMediaSlot) -> &'static str {
    match value {
        CharacterMediaSlot::AvatarOriginal => "avatar_original",
        CharacterMediaSlot::Background => "background",
        CharacterMediaSlot::DesignReference => "design_reference",
    }
}

fn media_slot_from_name(value: &str) -> Result<CharacterMediaSlot, rusqlite::Error> {
    match value {
        "avatar_original" => Ok(CharacterMediaSlot::AvatarOriginal),
        "background" => Ok(CharacterMediaSlot::Background),
        "design_reference" => Ok(CharacterMediaSlot::DesignReference),
        _ => Err(invalid()),
    }
}

fn scene_slot_name(value: SceneAssetSlot) -> &'static str {
    match value {
        SceneAssetSlot::Background => "background",
        SceneAssetSlot::Inline => "inline",
    }
}

fn scene_slot_from_name(value: &str) -> Result<SceneAssetSlot, rusqlite::Error> {
    match value {
        "background" => Ok(SceneAssetSlot::Background),
        "inline" => Ok(SceneAssetSlot::Inline),
        _ => Err(invalid()),
    }
}

fn role_name(value: StarterRole) -> &'static str {
    match value {
        StarterRole::User => "user",
        StarterRole::Assistant => "assistant",
    }
}

fn role_from_name(value: &str) -> Result<StarterRole, rusqlite::Error> {
    match value {
        "user" => Ok(StarterRole::User),
        "assistant" => Ok(StarterRole::Assistant),
        _ => Err(invalid()),
    }
}

fn id<T: std::str::FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| invalid())
}

fn parse_id<T: std::str::FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| invalid())
}

fn sql_u64(value: u64) -> Result<i64, RepositoryError> {
    i64::try_from(value).map_err(|_| RepositoryError::Storage)
}

fn rev(value: i64) -> Result<Revision, rusqlite::Error> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| invalid())
}

fn next_revision(value: Revision) -> Result<Revision, RepositoryError> {
    value.next().map_err(|_| RepositoryError::Storage)
}

fn id_text<T: ToString>(value: Option<T>) -> Option<String> {
    value.map(|value| value.to_string())
}

fn cursor_encode(updated_at: TimestampMillis, id: CharacterId) -> Result<String, RepositoryError> {
    let envelope = Envelope {
        format_version: CHARACTER_CURSOR_VERSION,
        value: (updated_at.get(), id.to_string()),
    };
    serde_json::to_vec(&envelope)
        .map(|bytes| super::hex_encode(&bytes))
        .map_err(|_| {
            RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
                field: "character.cursor",
            })
        })
}

fn cursor_decode(value: Option<&str>) -> Result<Option<(i64, CharacterId)>, RepositoryError> {
    let Some(value) = value else { return Ok(None) };
    let bytes = super::hex_decode(value).map_err(|_| {
        RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidValue {
            field: "character.cursor",
        })
    })?;
    let envelope: Envelope<(i64, String)> = serde_json::from_slice(&bytes).map_err(|_| {
        RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidValue {
            field: "character.cursor",
        })
    })?;
    if envelope.format_version != CHARACTER_CURSOR_VERSION {
        return Err(RepositoryError::Invalid(
            lettuce_characters::ValidationError::InvalidValue {
                field: "character.cursor",
            },
        ));
    }
    Ok(Some((
        envelope.value.0,
        envelope.value.1.parse().map_err(|_| {
            RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidValue {
                field: "character.cursor",
            })
        })?,
    )))
}

fn image_assets(
    connection: &Connection,
    ids: impl IntoIterator<Item = AssetId>,
) -> Result<(), RepositoryError> {
    let ids: Vec<_> = ids.into_iter().collect();
    for asset_id in ids {
        let kind = connection
            .query_row(
                "SELECT blob_kind FROM media_assets WHERE id=?1",
                [asset_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.asset",
                },
            ))?;
        if kind != "image" {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.image_asset",
                },
            ));
        }
    }
    Ok(())
}

fn character_row(
    connection: &Connection,
    id: CharacterId,
) -> Result<Option<Character>, rusqlite::Error> {
    connection
        .query_row(
            "SELECT status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at FROM characters WHERE id=?1",
            [id.to_string()],
            |row| parse_character_row(row, id),
        )
        .optional()
}

fn parse_character_row(row: &Row<'_>, id: CharacterId) -> rusqlite::Result<Character> {
    let stored_name = row.get::<_, String>(1)?;
    let stored_nickname = row.get::<_, Option<String>>(2)?;
    let normalized_name = row.get::<_, String>(3)?;
    let normalized_nickname = row.get::<_, Option<String>>(4)?;
    let expected_normalized_nickname = stored_nickname.as_deref().map(canonical_name);
    let profile: CharacterProfile = decode(row.get::<_, String>(5)?, PROFILE_VERSION)?;
    if profile.name != stored_name
        || profile.nickname != stored_nickname
        || canonical_name(&stored_name) != normalized_name
        || expected_normalized_nickname.as_deref() != normalized_nickname.as_deref()
    {
        return Err(invalid());
    }
    let provenance: CharacterProvenance = decode(row.get::<_, String>(6)?, PROVENANCE_VERSION)?;
    let defaults: CharacterDefaults = decode(row.get::<_, String>(7)?, DEFAULTS_VERSION)?;
    if defaults.interaction_mode != interaction_from_name(&row.get::<_, String>(8)?)?
        || defaults.memory_policy != memory_from_name(&row.get::<_, String>(9)?)?
        || defaults.model_profile_id
            != row
                .get::<_, Option<String>>(10)?
                .map(parse_id)
                .transpose()?
        || defaults.default_scene_id
            != row
                .get::<_, Option<String>>(11)?
                .map(parse_id)
                .transpose()?
        || defaults.default_starter_id
            != row
                .get::<_, Option<String>>(12)?
                .map(parse_id)
                .transpose()?
        || defaults.direct_prompt_id
            != row
                .get::<_, Option<String>>(13)?
                .map(parse_id)
                .transpose()?
        || defaults.group_conversation_prompt_id
            != row
                .get::<_, Option<String>>(14)?
                .map(parse_id)
                .transpose()?
        || defaults.group_roleplay_prompt_id
            != row
                .get::<_, Option<String>>(15)?
                .map(parse_id)
                .transpose()?
        || defaults.voice_autoplay != (row.get::<_, i64>(18)? != 0)
    {
        return Err(invalid());
    }
    let stored_voice_profile = row
        .get::<_, Option<String>>(16)?
        .map(parse_id)
        .transpose()?;
    let stored_voice_legacy = row.get::<_, Option<String>>(17)?;
    let expected_voice = match (&stored_voice_profile, stored_voice_legacy) {
        (Some(profile_id), None) => Some(lettuce_characters::VoicePreference::VoiceProfile(
            *profile_id,
        )),
        (None, Some(locator)) => Some(lettuce_characters::VoicePreference::UnresolvedLegacy(
            lettuce_characters::LegacyVoiceLocatorV1 {
                locator: locator.clone(),
            },
        )),
        (None, None) => None,
        _ => return Err(invalid()),
    };
    if defaults.voice != expected_voice {
        return Err(invalid());
    }
    let presentation: CharacterPresentationV1 =
        decode(row.get::<_, String>(19)?, PRESENTATION_VERSION)?;
    let recommendation: Option<ImageRecommendation> = match row.get::<_, Option<String>>(20)? {
        Some(payload) => decode(&payload, RECOMMENDATION_VERSION)?,
        None => None,
    };
    let character = Character {
        id,
        status: status_from_name(&row.get::<_, String>(0)?)?,
        profile,
        provenance,
        defaults,
        presentation,
        image_recommendation: recommendation,
        media: CharacterMedia::default(),
        revision: rev(row.get(21)?)?,
        created_at: TimestampMillis::new(row.get(22)?),
        updated_at: TimestampMillis::new(row.get(23)?),
    };
    character.validate().map_err(|_| invalid())?;
    Ok(character)
}

fn load_details(
    connection: &Connection,
    character_id: CharacterId,
) -> Result<Option<lettuce_characters::CharacterDetails>, rusqlite::Error> {
    let Some(mut character) = character_row(connection, character_id)? else {
        return Ok(None);
    };
    let mut media = Vec::new();
    let mut statement = connection.prepare(
        "SELECT asset_id,slot,ordinal FROM character_media WHERE character_id=?1 ORDER BY slot,ordinal,asset_id",
    )?;
    for row in statement.query_map([character_id.to_string()], |row| {
        Ok(CharacterMediaLink {
            asset_id: id(row.get(0)?)?,
            slot: media_slot_from_name(&row.get::<_, String>(1)?)?,
            ordinal: row.get::<_, i64>(2)?.try_into().map_err(|_| invalid())?,
        })
    })? {
        media.push(row?);
    }
    character.media = CharacterMedia { links: media };
    let presentation_ids: BTreeSet<AssetId> = connection
        .prepare("SELECT asset_id FROM character_presentation_asset_refs WHERE character_id=?1 ORDER BY asset_id")?
        .query_map([character_id.to_string()], |row| id(row.get(0)?))?
        .collect::<rusqlite::Result<Vec<AssetId>>>()?
        .into_iter()
        .collect();
    if presentation_ids != character.presentation.referenced_asset_ids() {
        return Err(invalid());
    }
    let mut scenes = Vec::new();
    let mut variants = Vec::new();
    let mut scene_stmt = connection.prepare(
        "SELECT id,status,ordinal,content_json,direction,selected_variant_id,revision,created_at,updated_at FROM scenes WHERE character_id=?1 ORDER BY ordinal,id",
    )?;
    for row in scene_stmt.query_map([character_id.to_string()], |row| {
        let scene_id: SceneId = id(row.get(0)?)?;
        let content: SceneDocumentV1 = decode(row.get::<_, String>(3)?, DOCUMENT_VERSION)?;
        let mut assets = Vec::new();
        let mut asset_stmt = connection.prepare(
            "SELECT id,asset_id,slot,ordinal FROM scene_assets WHERE character_id=?1 AND scene_id=?2 ORDER BY slot,ordinal,id",
        )?;
        for asset in asset_stmt.query_map(params![character_id.to_string(), scene_id.to_string()], |asset| {
            Ok(SceneAssetLink {
                id: id(asset.get(0)?)?,
                asset_id: id(asset.get(1)?)?,
                slot: scene_slot_from_name(&asset.get::<_, String>(2)?)?,
                ordinal: asset.get::<_, i64>(3)?.try_into().map_err(|_| invalid())?,
            })
        })? {
            assets.push(asset?);
        }
        Ok(Scene {
            id: scene_id,
            owner: SceneOwner::Character(character_id),
            status: status_from_name(&row.get::<_, String>(1)?)?,
            ordinal: row.get::<_, i64>(2)?.try_into().map_err(|_| invalid())?,
            content,
            direction: row.get(4)?,
            selected_variant_id: row.get::<_, Option<String>>(5)?.map(id).transpose()?,
            assets,
            revision: rev(row.get(6)?)?,
            created_at: TimestampMillis::new(row.get(7)?),
            updated_at: TimestampMillis::new(row.get(8)?),
        })
    })? {
        let scene = row?;
        let mut variant_stmt = connection.prepare(
            "SELECT id,ordinal,content_json,direction,revision,created_at,updated_at FROM scene_variants WHERE character_id=?1 AND scene_id=?2 ORDER BY ordinal,id",
        )?;
        for variant in variant_stmt.query_map(params![character_id.to_string(), scene.id.to_string()], |variant| {
            Ok(SceneVariant {
                id: id(variant.get(0)?)?,
                scene_id: scene.id,
                ordinal: variant.get::<_, i64>(1)?.try_into().map_err(|_| invalid())?,
                content: decode(variant.get::<_, String>(2)?, DOCUMENT_VERSION)?,
                direction: variant.get(3)?,
                revision: rev(variant.get(4)?)?,
                created_at: TimestampMillis::new(variant.get(5)?),
                updated_at: TimestampMillis::new(variant.get(6)?),
            })
        })? {
            variants.push(variant?);
        }
        scenes.push(scene);
    }
    let mut starters = Vec::new();
    let mut starter_stmt = connection.prepare(
        "SELECT id,name,ordinal,scene_id,prompt_id,lorebooks_json,revision,created_at,updated_at FROM conversation_starters WHERE character_id=?1 ORDER BY ordinal,id",
    )?;
    for row in starter_stmt.query_map([character_id.to_string()], |row| {
        let starter_id: ConversationStarterId = id(row.get(0)?)?;
        let mut messages = Vec::new();
        let mut message_stmt = connection.prepare(
            "SELECT id,role,content FROM starter_messages WHERE character_id=?1 AND starter_id=?2 ORDER BY ordinal,id",
        )?;
        for message in message_stmt.query_map(params![character_id.to_string(), starter_id.to_string()], |message| {
            Ok(StarterMessage {
                id: id(message.get(0)?)?,
                role: role_from_name(&message.get::<_, String>(1)?)?,
                content: message.get(2)?,
            })
        })? {
            messages.push(message?);
        }
        Ok(ConversationStarter {
            id: starter_id,
            character_id,
            name: row.get(1)?,
            ordinal: row.get::<_, i64>(2)?.try_into().map_err(|_| invalid())?,
            messages,
            scene_id: row.get::<_, Option<String>>(3)?.map(id).transpose()?,
            prompt_id: row.get::<_, Option<String>>(4)?.map(id).transpose()?,
            lorebooks: decode(row.get::<_, String>(5)?, LOREBOOK_VERSION)?,
            revision: rev(row.get(6)?)?,
            created_at: TimestampMillis::new(row.get(7)?),
            updated_at: TimestampMillis::new(row.get(8)?),
        })
    })? {
        starters.push(row?);
    }
    let details = lettuce_characters::CharacterDetails {
        character,
        scenes,
        variants,
        starters,
    };
    let scene_ids: BTreeSet<_> = details.scenes.iter().map(|scene| scene.id).collect();
    let mut variant_scene_ids =
        connection.prepare("SELECT scene_id FROM scene_variants WHERE character_id=?1")?;
    for row in variant_scene_ids.query_map([character_id.to_string()], |row| id(row.get(0)?))? {
        if !scene_ids.contains(&row?) {
            return Err(invalid());
        }
    }
    let mut scene_asset_scene_ids =
        connection.prepare("SELECT scene_id FROM scene_assets WHERE character_id=?1")?;
    for row in scene_asset_scene_ids.query_map([character_id.to_string()], |row| id(row.get(0)?))? {
        if !scene_ids.contains(&row?) {
            return Err(invalid());
        }
    }
    let starter_ids: BTreeSet<_> = details.starters.iter().map(|starter| starter.id).collect();
    let mut message_starter_ids =
        connection.prepare("SELECT starter_id FROM starter_messages WHERE character_id=?1")?;
    for row in message_starter_ids.query_map([character_id.to_string()], |row| id(row.get(0)?))? {
        if !starter_ids.contains(&row?) {
            return Err(invalid());
        }
    }
    for asset_id in collect_asset_ids(&details) {
        let kind = connection
            .query_row(
                "SELECT blob_kind FROM media_assets WHERE id=?1",
                [asset_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(invalid)?;
        if kind != "image" {
            return Err(invalid());
        }
    }
    CreateCharacterPlan {
        character: details.character.clone(),
        scenes: details.scenes.clone(),
        variants: details.variants.clone(),
        starters: details.starters.clone(),
    }
    .validate()
    .map_err(|_| invalid())?;
    Ok(Some(details))
}

fn load_character(connection: &Connection, id: CharacterId) -> Result<Character, RepositoryError> {
    load_details(connection, id)
        .map_err(db_error)?
        .map(|details| details.character)
        .ok_or(RepositoryError::NotFound)
}

fn ensure_root(
    tx: &Transaction<'_>,
    id: CharacterId,
    expected: Revision,
    allow_archived: bool,
) -> Result<lettuce_characters::CharacterDetails, RepositoryError> {
    let current = load_details(tx, id)
        .map_err(db_error)?
        .ok_or(RepositoryError::NotFound)?;
    if !allow_archived && current.character.status == LifecycleStatus::Archived {
        return Err(RepositoryError::Archived);
    }
    if current.character.revision != expected {
        return Err(RepositoryError::StaleRevision {
            expected,
            actual: current.character.revision,
        });
    }
    Ok(current)
}

fn bump_root(
    tx: &Transaction<'_>,
    id: CharacterId,
    expected: Revision,
    now: TimestampMillis,
) -> Result<(), RepositoryError> {
    let next = next_revision(expected)?;
    let changed = tx
        .execute(
            "UPDATE characters SET revision=?2,updated_at=?3 WHERE id=?1 AND revision=?4",
            params![
                id.to_string(),
                sql_u64(next.get())?,
                now.get(),
                sql_u64(expected.get())?
            ],
        )
        .map_err(db_error)?;
    if changed != 1 {
        return Err(RepositoryError::StaleRevision {
            expected,
            actual: expected,
        });
    }
    Ok(())
}

fn collect_asset_ids(details: &lettuce_characters::CharacterDetails) -> BTreeSet<AssetId> {
    let mut ids: BTreeSet<_> = details
        .character
        .media
        .links
        .iter()
        .map(|link| link.asset_id)
        .collect();
    ids.extend(details.character.presentation.referenced_asset_ids());
    for scene in &details.scenes {
        ids.extend(scene.assets.iter().map(|link| link.asset_id));
    }
    ids
}

fn validate_scene_graph(
    details: &lettuce_characters::CharacterDetails,
    scene: &Scene,
    variants: &[SceneVariant],
) -> Result<(), RepositoryError> {
    if scene.owner != SceneOwner::Character(details.character.id) {
        return Err(RepositoryError::Invalid(
            lettuce_characters::ValidationError::InvalidReference {
                field: "scene.owner",
            },
        ));
    }
    scene
        .validate_selected_variant(variants)
        .map_err(RepositoryError::Invalid)
}

fn validate_widget_legacy_tokens(node: &WidgetNode) -> bool {
    match node {
        WidgetNode::Image {
            source: WidgetImageSource::UnresolvedLegacy { .. },
            ..
        } => true,
        WidgetNode::Box { children, .. } => children.iter().any(validate_widget_legacy_tokens),
        _ => false,
    }
}

fn duplicate_external_references(
    details: &lettuce_characters::CharacterDetails,
) -> RetainedExternalReferences {
    let mut retained = RetainedExternalReferences {
        asset_ids: collect_asset_ids(details).into_iter().collect(),
        ..RetainedExternalReferences::default()
    };

    let mut prompts = BTreeSet::new();
    for prompt in [
        details.character.defaults.direct_prompt_id,
        details.character.defaults.group_conversation_prompt_id,
        details.character.defaults.group_roleplay_prompt_id,
    ]
    .into_iter()
    .flatten()
    {
        prompts.insert(prompt);
    }
    let mut lorebooks = BTreeSet::new();
    for starter in &details.starters {
        if let Some(prompt) = starter.prompt_id {
            prompts.insert(prompt);
        }
        if let Selection::Explicit(ids) = &starter.lorebooks {
            lorebooks.extend(ids.iter().copied());
        }
    }
    retained.prompt_document_ids = prompts.into_iter().collect();
    retained.lorebook_ids = lorebooks.into_iter().collect();
    if let Some(model_profile_id) = details.character.defaults.model_profile_id {
        retained.model_profile_ids.push(model_profile_id);
    }
    if let Some(voice) = &details.character.defaults.voice {
        match voice {
            lettuce_characters::VoicePreference::VoiceProfile(id) => {
                retained.voice_profile_ids.push(*id);
            }
            lettuce_characters::VoicePreference::UnresolvedLegacy(_) => {
                retained
                    .unresolved_legacy_references
                    .push(UnresolvedLegacyReference::VoiceLocator);
            }
        }
    }
    if let Some(recommendation) = &details.character.image_recommendation {
        if let Some(id) = recommendation.artifact_id {
            retained.model_artifact_ids.push(id);
        } else if recommendation.unresolved_legacy_name.is_some() {
            retained
                .unresolved_legacy_references
                .push(UnresolvedLegacyReference::ImageRecommendation);
        }
    }
    if details
        .character
        .presentation
        .chat_appearance
        .chat_widget_slots
        .left
        .iter()
        .chain(
            details
                .character
                .presentation
                .chat_appearance
                .chat_widget_slots
                .right
                .iter(),
        )
        .any(validate_widget_legacy_tokens)
    {
        retained
            .unresolved_legacy_references
            .push(UnresolvedLegacyReference::WidgetImageToken);
    }
    retained
}

fn validate_plan_assets(
    connection: &Connection,
    plan: &CreateCharacterPlan,
) -> Result<(), RepositoryError> {
    let details = lettuce_characters::CharacterDetails {
        character: plan.character.clone(),
        scenes: plan.scenes.clone(),
        variants: plan.variants.clone(),
        starters: plan.starters.clone(),
    };
    image_assets(connection, collect_asset_ids(&details))
}

fn insert_character(tx: &Transaction<'_>, character: &Character) -> Result<(), RepositoryError> {
    let profile = encode(&character.profile, PROFILE_VERSION)?;
    let provenance = encode(&character.provenance, PROVENANCE_VERSION)?;
    let defaults = encode(&character.defaults, DEFAULTS_VERSION)?;
    let presentation = encode(&character.presentation, PRESENTATION_VERSION)?;
    let recommendation = character
        .image_recommendation
        .as_ref()
        .map(|value| encode(&Some(value), RECOMMENDATION_VERSION))
        .transpose()?;
    let (voice_profile_id, voice_legacy_locator) = match &character.defaults.voice {
        Some(lettuce_characters::VoicePreference::VoiceProfile(id)) => (Some(id.to_string()), None),
        Some(lettuce_characters::VoicePreference::UnresolvedLegacy(locator)) => {
            (None, Some(locator.locator.clone()))
        }
        None => (None, None),
    };
    tx.execute(
        "INSERT INTO characters (id,status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25)",
        params![
            character.id.to_string(), status_name(character.status), character.profile.name,
            character.profile.nickname, canonical_name(&character.profile.name),
            character.profile.nickname.as_deref().map(canonical_name), profile, provenance, defaults,
            interaction_name(character.defaults.interaction_mode), memory_name(character.defaults.memory_policy),
            id_text(character.defaults.model_profile_id), id_text(character.defaults.default_scene_id),
            id_text(character.defaults.default_starter_id), id_text(character.defaults.direct_prompt_id),
            id_text(character.defaults.group_conversation_prompt_id), id_text(character.defaults.group_roleplay_prompt_id),
            voice_profile_id, voice_legacy_locator, character.defaults.voice_autoplay,
            presentation, recommendation, sql_u64(character.revision.get())?, character.created_at.get(), character.updated_at.get()
        ],
    ).map_err(db_error)?;
    Ok(())
}

fn replace_character_media(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    media: &CharacterMedia,
) -> Result<(), RepositoryError> {
    tx.execute(
        "DELETE FROM character_media WHERE character_id=?1",
        [character_id.to_string()],
    )
    .map_err(db_error)?;
    for link in &media.links {
        tx.execute(
            "INSERT INTO character_media(character_id,asset_id,slot,ordinal) VALUES (?1,?2,?3,?4)",
            params![
                character_id.to_string(),
                link.asset_id.to_string(),
                media_slot_name(link.slot),
                link.ordinal
            ],
        )
        .map_err(db_error)?;
    }
    Ok(())
}

fn insert_character_presentation_refs(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    presentation: &CharacterPresentationV1,
) -> Result<(), RepositoryError> {
    for asset_id in presentation.referenced_asset_ids() {
        tx.execute(
            "INSERT INTO character_presentation_asset_refs(character_id,asset_id) VALUES (?1,?2)",
            params![character_id.to_string(), asset_id.to_string()],
        )
        .map_err(db_error)?;
    }
    Ok(())
}

fn insert_scene(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    scene: &Scene,
) -> Result<(), RepositoryError> {
    tx.execute(
        "INSERT INTO scenes (character_id,id,status,ordinal,content_json,direction,selected_variant_id,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![character_id.to_string(), scene.id.to_string(), status_name(scene.status), scene.ordinal,
            encode(&scene.content, DOCUMENT_VERSION)?, scene.direction, id_text(scene.selected_variant_id), sql_u64(scene.revision.get())?, scene.created_at.get(), scene.updated_at.get()],
    ).map_err(db_error)?;
    insert_scene_assets(tx, character_id, scene.id, &scene.assets)
}

fn insert_scene_assets(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    scene_id: SceneId,
    assets: &[SceneAssetLink],
) -> Result<(), RepositoryError> {
    for asset in assets {
        tx.execute(
            "INSERT INTO scene_assets (character_id,scene_id,id,asset_id,slot,ordinal) VALUES (?1,?2,?3,?4,?5,?6)",
            params![character_id.to_string(), scene_id.to_string(), asset.id.to_string(), asset.asset_id.to_string(), scene_slot_name(asset.slot), asset.ordinal],
        ).map_err(db_error)?;
    }
    Ok(())
}

fn insert_variant(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    variant: &SceneVariant,
) -> Result<(), RepositoryError> {
    tx.execute(
        "INSERT INTO scene_variants (character_id,id,scene_id,ordinal,content_json,direction,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![character_id.to_string(), variant.id.to_string(), variant.scene_id.to_string(), variant.ordinal,
            encode(&variant.content, DOCUMENT_VERSION)?, variant.direction, sql_u64(variant.revision.get())?, variant.created_at.get(), variant.updated_at.get()],
    ).map_err(db_error)?;
    Ok(())
}

fn insert_starter(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    starter: &ConversationStarter,
) -> Result<(), RepositoryError> {
    tx.execute(
        "INSERT INTO conversation_starters (character_id,id,name,ordinal,scene_id,prompt_id,lorebooks_json,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![character_id.to_string(), starter.id.to_string(), starter.name, starter.ordinal, id_text(starter.scene_id), id_text(starter.prompt_id), encode(&starter.lorebooks, LOREBOOK_VERSION)?, sql_u64(starter.revision.get())?, starter.created_at.get(), starter.updated_at.get()],
    ).map_err(db_error)?;
    for (ordinal, message) in starter.messages.iter().enumerate() {
        tx.execute(
            "INSERT INTO starter_messages (character_id,starter_id,id,role,content,ordinal) VALUES (?1,?2,?3,?4,?5,?6)",
            params![character_id.to_string(), starter.id.to_string(), message.id.to_string(), role_name(message.role), message.content, i64::try_from(ordinal).map_err(|_| RepositoryError::Storage)?],
        ).map_err(db_error)?;
    }
    Ok(())
}

pub(crate) fn insert_character_plan(
    tx: &Transaction<'_>,
    plan: &CreateCharacterPlan,
) -> Result<lettuce_characters::CharacterDetails, RepositoryError> {
    plan.validate()?;
    validate_plan_assets(tx, plan)?;
    if character_row(tx, plan.character.id)
        .map_err(db_error)?
        .is_some()
    {
        return Err(RepositoryError::AlreadyExists);
    }
    insert_character(tx, &plan.character)?;
    if plan.character.defaults.interaction_mode == lettuce_characters::InteractionMode::Companion {
        let state = initial_soul_state(
            plan.character.defaults.companion_soul.as_ref(),
            plan.character.created_at,
        )
        .map_err(|_| RepositoryError::Storage)?;
        crate::soul_adapter::create_in(
            tx,
            SoulOwner::Character(plan.character.id),
            &state,
            plan.character.created_at,
        )
        .map_err(|_| RepositoryError::Storage)?;
    }
    replace_character_media(tx, plan.character.id, &plan.character.media)?;
    insert_character_presentation_refs(tx, plan.character.id, &plan.character.presentation)?;
    for scene in &plan.scenes {
        insert_scene(tx, plan.character.id, scene)?;
    }
    for variant in &plan.variants {
        insert_variant(tx, plan.character.id, variant)?;
    }
    for starter in &plan.starters {
        insert_starter(tx, plan.character.id, starter)?;
    }
    load_details(tx, plan.character.id)
        .map_err(db_error)?
        .ok_or(RepositoryError::Storage)
}

pub(crate) fn load_character_details(
    connection: &Connection,
    id: CharacterId,
) -> Result<Option<lettuce_characters::CharacterDetails>, RepositoryError> {
    load_details(connection, id).map_err(db_error)
}

pub(crate) fn replace_character_profile_scenes(
    tx: &Transaction<'_>,
    expected_revision: Revision,
    plan: &CreateCharacterPlan,
) -> Result<lettuce_characters::CharacterDetails, RepositoryError> {
    let current = ensure_root(tx, plan.character.id, expected_revision, false)?;
    let expected_character = Character {
        profile: plan.character.profile.clone(),
        revision: expected_revision
            .next()
            .map_err(|_| RepositoryError::Storage)?,
        updated_at: plan.character.updated_at,
        ..current.character.clone()
    };
    let retained_scene_ids: BTreeSet<_> = plan.scenes.iter().map(|scene| scene.id).collect();
    if current
        .character
        .defaults
        .default_scene_id
        .is_some_and(|id| !retained_scene_ids.contains(&id))
        || current.starters.iter().any(|starter| {
            starter
                .scene_id
                .is_some_and(|id| !retained_scene_ids.contains(&id))
        })
    {
        return Err(RepositoryError::HasDependencies);
    }
    let expected_variants: Vec<_> = current
        .variants
        .iter()
        .filter(|variant| retained_scene_ids.contains(&variant.scene_id))
        .cloned()
        .collect();
    if plan.character != expected_character
        || plan.variants != expected_variants
        || plan.starters != current.starters
    {
        return Err(RepositoryError::Invalid(
            lettuce_characters::ValidationError::Invariant {
                field: "character.profile_scene_plan",
            },
        ));
    }
    plan.validate()?;
    validate_plan_assets(tx, plan)?;
    let current_by_id: HashMap<_, _> = current
        .scenes
        .iter()
        .map(|scene| (scene.id, scene))
        .collect();
    let profile = encode(&plan.character.profile, PROFILE_VERSION)?;
    tx.execute(
        "UPDATE characters SET name=?2,nickname=?3,normalized_name=?4,normalized_nickname=?5,profile_json=?6 WHERE id=?1",
        params![
            plan.character.id.to_string(),
            plan.character.profile.name,
            plan.character.profile.nickname,
            canonical_name(&plan.character.profile.name),
            plan.character.profile.nickname.as_deref().map(canonical_name),
            profile,
        ],
    )
    .map_err(db_error)?;
    for scene in &current.scenes {
        if !retained_scene_ids.contains(&scene.id) {
            tx.execute(
                "DELETE FROM scenes WHERE character_id=?1 AND id=?2",
                params![plan.character.id.to_string(), scene.id.to_string()],
            )
            .map_err(db_error)?;
        }
    }
    tx.execute(
        "UPDATE scenes SET ordinal=ordinal+1000000 WHERE character_id=?1",
        [plan.character.id.to_string()],
    )
    .map_err(db_error)?;
    for scene in &plan.scenes {
        if current_by_id.contains_key(&scene.id) {
            tx.execute(
                "UPDATE scenes SET status=?3,ordinal=?4,content_json=?5,direction=?6,selected_variant_id=?7,revision=?8,created_at=?9,updated_at=?10 WHERE character_id=?1 AND id=?2",
                params![
                    plan.character.id.to_string(),
                    scene.id.to_string(),
                    status_name(scene.status),
                    scene.ordinal,
                    encode(&scene.content, DOCUMENT_VERSION)?,
                    scene.direction,
                    id_text(scene.selected_variant_id),
                    sql_u64(scene.revision.get())?,
                    scene.created_at.get(),
                    scene.updated_at.get(),
                ],
            )
            .map_err(db_error)?;
        } else {
            insert_scene(tx, plan.character.id, scene)?;
        }
    }
    bump_root(
        tx,
        plan.character.id,
        expected_revision,
        plan.character.updated_at,
    )?;
    load_details(tx, plan.character.id)
        .map_err(db_error)?
        .ok_or(RepositoryError::Storage)
}

impl CharacterRepository for Database {
    fn create(
        &self,
        plan: CreateCharacterPlan,
    ) -> Result<lettuce_characters::CharacterDetails, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = insert_character_plan(&tx, &plan)?;
        tx.commit().map_err(db_error)?;
        Ok(details)
    }

    fn get(
        &self,
        id: CharacterId,
    ) -> Result<Option<lettuce_characters::CharacterDetails>, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let details = load_details(&tx, id).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(details)
    }

    fn list(
        &self,
        request: PageRequest,
        include_archived: bool,
    ) -> Result<Page<Character>, RepositoryError> {
        let cursor = cursor_decode(request.cursor.as_deref())?;
        let limit = usize::from(request.limit.get()).max(1);
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let mut sql = String::from("SELECT id FROM characters WHERE (?1 OR status='active')");
        if cursor.is_some() {
            sql.push_str(" AND (updated_at < ?2 OR (updated_at = ?2 AND id > ?3))");
        }
        sql.push_str(" ORDER BY updated_at DESC,id ASC LIMIT ?4");
        let (cursor_time, cursor_id) = cursor
            .map(|(time, id)| (Some(time), Some(id.to_string())))
            .unwrap_or((None, None));
        let mut statement = tx.prepare(&sql).map_err(db_error)?;
        let ids = statement
            .query_map(
                params![
                    include_archived,
                    cursor_time,
                    cursor_id,
                    i64::try_from(limit + 1).unwrap_or(i64::MAX)
                ],
                |row| id(row.get(0)?),
            )
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<CharacterId>>>()
            .map_err(db_error)?;
        drop(statement);
        let mut ids = ids;
        let has_more = ids.len() > limit;
        if has_more {
            ids.truncate(limit);
        }
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            items.push(load_character(&tx, id)?);
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| cursor_encode(item.updated_at, item.id))
                .transpose()?
        } else {
            None
        };
        tx.commit().map_err(db_error)?;
        Ok(Page { items, next_cursor })
    }

    fn search(
        &self,
        request: CharacterSearch,
        page: PageRequest,
    ) -> Result<Page<Character>, RepositoryError> {
        if request.text.trim().is_empty() {
            return self.list(page, request.include_archived);
        }
        let cursor = cursor_decode(page.cursor.as_deref())?;
        let limit = usize::from(page.limit.get()).max(1);
        let text = request
            .text
            .to_lowercase()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{text}%");
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let mut sql = String::from(
            "SELECT id FROM characters WHERE (?1 OR status='active') AND (normalized_name LIKE ?2 ESCAPE '\\' OR normalized_nickname LIKE ?2 ESCAPE '\\')",
        );
        if cursor.is_some() {
            sql.push_str(" AND (updated_at < ?3 OR (updated_at = ?3 AND id > ?4))");
        }
        sql.push_str(" ORDER BY updated_at DESC,id ASC LIMIT ?5");
        let (cursor_time, cursor_id) = cursor
            .map(|(time, id)| (Some(time), Some(id.to_string())))
            .unwrap_or((None, None));
        let mut statement = tx.prepare(&sql).map_err(db_error)?;
        let ids = statement
            .query_map(
                params![
                    request.include_archived,
                    pattern,
                    cursor_time,
                    cursor_id,
                    i64::try_from(limit + 1).unwrap_or(i64::MAX)
                ],
                |row| id(row.get(0)?),
            )
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<CharacterId>>>()
            .map_err(db_error)?;
        drop(statement);
        let mut ids = ids;
        let has_more = ids.len() > limit;
        if has_more {
            ids.truncate(limit);
        }
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            items.push(load_character(&tx, id)?);
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| cursor_encode(item.updated_at, item.id))
                .transpose()?
        } else {
            None
        };
        tx.commit().map_err(db_error)?;
        Ok(Page { items, next_cursor })
    }

    fn revise_profile(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        profile: CharacterProfile,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        validate(profile.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, false)?;
        tx.execute(
            "UPDATE characters SET name=?2,nickname=?3,normalized_name=?4,normalized_nickname=?5,profile_json=?6 WHERE id=?1",
            params![
                id.to_string(),
                profile.name,
                profile.nickname,
                canonical_name(&profile.name),
                profile.nickname.as_deref().map(canonical_name),
                encode(&profile, PROFILE_VERSION)?
            ],
        )
        .map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn update_image_recommendation(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        recommendation: Option<ImageRecommendation>,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        if let Some(value) = &recommendation {
            validate(value.validate())?;
        }
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, false)?;
        let payload = recommendation
            .as_ref()
            .map(|value| encode(&Some(value), RECOMMENDATION_VERSION))
            .transpose()?;
        tx.execute(
            "UPDATE characters SET image_recommendation_json=?2 WHERE id=?1",
            params![id.to_string(), payload],
        )
        .map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn revise_provenance(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        provenance: CharacterProvenance,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        validate(provenance.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, false)?;
        tx.execute(
            "UPDATE characters SET provenance_json=?2 WHERE id=?1",
            params![id.to_string(), encode(&provenance, PROVENANCE_VERSION)?],
        )
        .map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn update_defaults(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        defaults: CharacterDefaults,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        validate(defaults.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, false)?;
        let details = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if defaults
            .default_scene_id
            .is_some_and(|value| !details.scenes.iter().any(|scene| scene.id == value))
            || defaults
                .default_starter_id
                .is_some_and(|value| !details.starters.iter().any(|starter| starter.id == value))
        {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.defaults",
                },
            ));
        }
        let (voice_profile_id, voice_legacy_locator) = match &defaults.voice {
            Some(lettuce_characters::VoicePreference::VoiceProfile(value)) => {
                (Some(value.to_string()), None)
            }
            Some(lettuce_characters::VoicePreference::UnresolvedLegacy(value)) => {
                (None, Some(value.locator.clone()))
            }
            None => (None, None),
        };
        tx.execute("UPDATE characters SET defaults_json=?2,interaction_mode=?3,memory_policy=?4,model_profile_id=?5,default_scene_id=?6,default_starter_id=?7,direct_prompt_id=?8,group_conversation_prompt_id=?9,group_roleplay_prompt_id=?10,voice_profile_id=?11,voice_legacy_locator=?12,voice_autoplay=?13 WHERE id=?1", params![id.to_string(), encode(&defaults, DEFAULTS_VERSION)?, interaction_name(defaults.interaction_mode), memory_name(defaults.memory_policy), id_text(defaults.model_profile_id), id_text(defaults.default_scene_id), id_text(defaults.default_starter_id), id_text(defaults.direct_prompt_id), id_text(defaults.group_conversation_prompt_id), id_text(defaults.group_roleplay_prompt_id), voice_profile_id, voice_legacy_locator, defaults.voice_autoplay]).map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn update_presentation(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        presentation: CharacterPresentationV1,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        validate(presentation.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        image_assets(&connection, presentation.referenced_asset_ids())?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, false)?;
        tx.execute(
            "DELETE FROM character_presentation_asset_refs WHERE character_id=?1",
            [id.to_string()],
        )
        .map_err(db_error)?;
        for asset_id in presentation.referenced_asset_ids() {
            tx.execute("INSERT INTO character_presentation_asset_refs(character_id,asset_id) VALUES (?1,?2)", params![id.to_string(), asset_id.to_string()]).map_err(db_error)?;
        }
        tx.execute(
            "UPDATE characters SET presentation_json=?2 WHERE id=?1",
            params![id.to_string(), encode(&presentation, PRESENTATION_VERSION)?],
        )
        .map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn update_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        media: CharacterMedia,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        validate(media.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        image_assets(&connection, media.links.iter().map(|link| link.asset_id))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, false)?;
        replace_character_media(&tx, id, &media)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn attach_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        link: CharacterMediaLink,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        image_assets(&connection, [link.asset_id])?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_root(&tx, id, expected_revision, false)?;
        if current
            .character
            .media
            .links
            .iter()
            .any(|existing| existing.asset_id == link.asset_id)
        {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::Duplicate {
                    field: "character.media.asset_ids",
                },
            ));
        }
        let mut media = current.character.media;
        media.links.push(link.clone());
        validate(media.validate())?;
        tx.execute(
            "INSERT INTO character_media(character_id,asset_id,slot,ordinal) VALUES (?1,?2,?3,?4)",
            params![
                id.to_string(),
                link.asset_id.to_string(),
                media_slot_name(link.slot),
                link.ordinal
            ],
        )
        .map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn detach_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        asset_id: AssetId,
        slot: CharacterMediaSlot,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_root(&tx, id, expected_revision, false)?;
        if !current
            .character
            .media
            .links
            .iter()
            .any(|link| link.asset_id == asset_id && link.slot == slot)
        {
            return Err(RepositoryError::NotFound);
        }
        let mut media = current.character.media;
        media
            .links
            .retain(|link| !(link.asset_id == asset_id && link.slot == slot));
        for (ordinal, link) in media
            .links
            .iter_mut()
            .filter(|link| link.slot == CharacterMediaSlot::DesignReference)
            .enumerate()
        {
            link.ordinal = ordinal as u32;
        }
        replace_character_media(&tx, id, &media)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn reorder_media(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        slot: CharacterMediaSlot,
        asset_id: AssetId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_root(&tx, id, expected_revision, false)?;
        let mut media = current.character.media;
        let Some(index) = media
            .links
            .iter()
            .position(|link| link.asset_id == asset_id && link.slot == slot)
        else {
            return Err(RepositoryError::NotFound);
        };
        let indexes: Vec<_> = media
            .links
            .iter()
            .enumerate()
            .filter_map(|(index, link)| (link.slot == slot).then_some(index))
            .collect();
        if target_ordinal as usize >= indexes.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "character.media.ordinal",
                },
            ));
        }
        let value = media.links.remove(index);
        let insert_at = media
            .links
            .iter()
            .enumerate()
            .filter(|(_, link)| link.slot == slot)
            .nth(target_ordinal as usize)
            .map(|(index, _)| index)
            .unwrap_or(media.links.len());
        media.links.insert(insert_at, value);
        for (ordinal, link) in media
            .links
            .iter_mut()
            .filter(|link| link.slot == slot)
            .enumerate()
        {
            link.ordinal = ordinal as u32;
        }
        replace_character_media(&tx, id, &media)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }

    fn archive(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        self.set_status(id, expected_revision, LifecycleStatus::Archived, now)
    }
    fn restore(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        self.set_status(id, expected_revision, LifecycleStatus::Active, now)
    }
}

impl Database {
    fn set_status(
        &self,
        id: CharacterId,
        expected_revision: Revision,
        status: LifecycleStatus,
        now: TimestampMillis,
    ) -> Result<Character, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_root(&tx, id, expected_revision, true)?;
        tx.execute(
            "UPDATE characters SET status=?2 WHERE id=?1",
            params![id.to_string(), status_name(status)],
        )
        .map_err(db_error)?;
        bump_root(&tx, id, expected_revision, now)?;
        let character = load_character(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(character)
    }
}

impl CharacterDependencyReader for Database {
    fn dependencies(&self, id: CharacterId) -> Result<DependencyReport, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let details = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        let mut references = Vec::new();
        if let Some(scene_id) = details.character.defaults.default_scene_id {
            references.push(DependencyReference::CharacterDefaultScene { scene_id });
        }
        if let Some(starter_id) = details.character.defaults.default_starter_id {
            references.push(DependencyReference::CharacterDefaultStarter { starter_id });
        }
        for starter in &details.starters {
            if let Some(scene_id) = starter.scene_id {
                references.push(DependencyReference::StarterScene {
                    starter_id: starter.id,
                    scene_id,
                });
            }
        }
        let mut group_stmt = tx
            .prepare("SELECT group_id FROM group_members WHERE character_id=?1 ORDER BY group_id")
            .map_err(db_error)?;
        for row in group_stmt
            .query_map([id.to_string()], |row| {
                Ok(DependencyReference::CharacterInGroup {
                    group_id: row.get::<_, String>(0)?.parse().map_err(|_| invalid())?,
                })
            })
            .map_err(db_error)?
        {
            references.push(row.map_err(db_error)?);
        }
        drop(group_stmt);
        references.extend(
            collect_asset_ids(&details)
                .into_iter()
                .map(|asset_id| DependencyReference::Asset { asset_id }),
        );
        let mut prompts = BTreeSet::new();
        prompts.extend(
            [
                details.character.defaults.direct_prompt_id,
                details.character.defaults.group_conversation_prompt_id,
                details.character.defaults.group_roleplay_prompt_id,
            ]
            .into_iter()
            .flatten(),
        );
        prompts.extend(
            details
                .starters
                .iter()
                .filter_map(|starter| starter.prompt_id),
        );
        references.extend(
            prompts
                .into_iter()
                .map(|prompt_id| DependencyReference::Prompt { prompt_id }),
        );
        let mut lorebooks = BTreeSet::new();
        for starter in &details.starters {
            if let Selection::Explicit(ids) = &starter.lorebooks {
                lorebooks.extend(ids.iter().copied());
            }
        }
        references.extend(
            lorebooks
                .into_iter()
                .map(|lorebook_id| DependencyReference::Lorebook { lorebook_id }),
        );
        let report = DependencyReport { references };
        tx.commit().map_err(db_error)?;
        Ok(report)
    }
}

impl ProfileDuplicateRepository for Database {
    fn duplicate_character(
        &self,
        request: ProfileDuplicateRequest,
    ) -> Result<ProfileDuplicateResult, RepositoryError> {
        request.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let source = load_details(&tx, request.source_character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if load_details(&tx, request.destination_character_id)
            .map_err(db_error)?
            .is_some()
        {
            return Err(RepositoryError::AlreadyExists);
        }

        let scene_remaps: Vec<_> = source
            .scenes
            .iter()
            .map(|scene| IdRemap {
                source: scene.id,
                destination: SceneId::new(),
            })
            .collect();
        let scene_ids: HashMap<_, _> = scene_remaps
            .iter()
            .map(|remap| (remap.source, remap.destination))
            .collect();
        let link_remaps: Vec<_> = source
            .scenes
            .iter()
            .flat_map(|scene| scene.assets.iter())
            .map(|link| IdRemap {
                source: link.id,
                destination: SceneAssetLinkId::new(),
            })
            .collect();
        let link_ids: HashMap<_, _> = link_remaps
            .iter()
            .map(|remap| (remap.source, remap.destination))
            .collect();
        let variant_remaps: Vec<_> = source
            .variants
            .iter()
            .map(|variant| IdRemap {
                source: variant.id,
                destination: SceneVariantId::new(),
            })
            .collect();
        let variant_ids: HashMap<_, _> = variant_remaps
            .iter()
            .map(|remap| (remap.source, remap.destination))
            .collect();
        let starter_remaps: Vec<_> = source
            .starters
            .iter()
            .map(|starter| IdRemap {
                source: starter.id,
                destination: ConversationStarterId::new(),
            })
            .collect();
        let starter_ids: HashMap<_, _> = starter_remaps
            .iter()
            .map(|remap| (remap.source, remap.destination))
            .collect();
        let message_remaps: Vec<_> = source
            .starters
            .iter()
            .flat_map(|starter| starter.messages.iter())
            .map(|message| IdRemap {
                source: message.id,
                destination: StarterMessageId::new(),
            })
            .collect();
        let message_ids: HashMap<_, _> = message_remaps
            .iter()
            .map(|remap| (remap.source, remap.destination))
            .collect();

        let mut character = source.character.clone();
        character.id = request.destination_character_id;
        character.status = LifecycleStatus::Active;
        character.revision = Revision::INITIAL;
        character.created_at = request.now;
        character.updated_at = request.now;
        if let Some(name) = &request.destination_name {
            character.profile.name = name.clone();
        }
        character.defaults.default_scene_id = character
            .defaults
            .default_scene_id
            .and_then(|id| scene_ids.get(&id).copied());
        character.defaults.default_starter_id = character
            .defaults
            .default_starter_id
            .and_then(|id| starter_ids.get(&id).copied());
        remap_presentation_character_id(
            &mut character.presentation,
            request.source_character_id,
            request.destination_character_id,
        );

        let scenes: Vec<_> = source
            .scenes
            .iter()
            .map(|scene| {
                let mut copy = scene.clone();
                copy.id = scene_ids[&scene.id];
                copy.owner = SceneOwner::Character(request.destination_character_id);
                copy.selected_variant_id = scene
                    .selected_variant_id
                    .and_then(|id| variant_ids.get(&id).copied());
                copy.content = remap_document(&scene.content, &link_ids);
                copy.assets = scene
                    .assets
                    .iter()
                    .map(|link| {
                        let mut link = link.clone();
                        link.id = link_ids[&link.id];
                        link
                    })
                    .collect();
                copy.revision = Revision::INITIAL;
                copy.created_at = request.now;
                copy.updated_at = request.now;
                copy
            })
            .collect();
        let variants: Vec<_> = source
            .variants
            .iter()
            .map(|variant| {
                let mut copy = variant.clone();
                copy.id = variant_ids[&variant.id];
                copy.scene_id = scene_ids[&variant.scene_id];
                copy.content = remap_document(&variant.content, &link_ids);
                copy.revision = Revision::INITIAL;
                copy.created_at = request.now;
                copy.updated_at = request.now;
                copy
            })
            .collect();
        let starters: Vec<_> = source
            .starters
            .iter()
            .map(|starter| {
                let mut copy = starter.clone();
                copy.id = starter_ids[&starter.id];
                copy.character_id = request.destination_character_id;
                copy.scene_id = starter.scene_id.and_then(|id| scene_ids.get(&id).copied());
                copy.messages = starter
                    .messages
                    .iter()
                    .map(|message| StarterMessage {
                        id: message_ids[&message.id],
                        ..message.clone()
                    })
                    .collect();
                copy.revision = Revision::INITIAL;
                copy.created_at = request.now;
                copy.updated_at = request.now;
                copy
            })
            .collect();
        let plan = CreateCharacterPlan {
            character,
            scenes,
            variants,
            starters,
        };
        plan.validate()?;
        validate_plan_assets(&tx, &plan)?;
        insert_character(&tx, &plan.character)?;
        replace_character_media(&tx, plan.character.id, &plan.character.media)?;
        insert_character_presentation_refs(&tx, plan.character.id, &plan.character.presentation)?;
        for scene in &plan.scenes {
            insert_scene(&tx, plan.character.id, scene)?;
        }
        for variant in &plan.variants {
            insert_variant(&tx, plan.character.id, variant)?;
        }
        for starter in &plan.starters {
            insert_starter(&tx, plan.character.id, starter)?;
        }
        let result = ProfileDuplicateResult {
            character_id: request.destination_character_id,
            remapped_scene_ids: scene_remaps,
            remapped_variant_ids: variant_remaps,
            remapped_scene_asset_link_ids: link_remaps,
            remapped_starter_ids: starter_remaps,
            remapped_starter_message_ids: message_remaps,
            retained_external_references: duplicate_external_references(&source),
        };
        result.validate_for(&request)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}

fn find_scene(
    details: &lettuce_characters::CharacterDetails,
    id: SceneId,
) -> Result<Scene, RepositoryError> {
    details
        .scenes
        .iter()
        .find(|scene| scene.id == id)
        .cloned()
        .ok_or(RepositoryError::NotFound)
}

fn find_variant(
    details: &lettuce_characters::CharacterDetails,
    id: SceneVariantId,
) -> Result<SceneVariant, RepositoryError> {
    details
        .variants
        .iter()
        .find(|variant| variant.id == id)
        .cloned()
        .ok_or(RepositoryError::NotFound)
}

fn find_starter(
    details: &lettuce_characters::CharacterDetails,
    id: ConversationStarterId,
) -> Result<ConversationStarter, RepositoryError> {
    details
        .starters
        .iter()
        .find(|starter| starter.id == id)
        .cloned()
        .ok_or(RepositoryError::NotFound)
}

fn remap_document(
    document: &SceneDocumentV1,
    links: &HashMap<SceneAssetLinkId, SceneAssetLinkId>,
) -> SceneDocumentV1 {
    let mut document = document.clone();
    for part in &mut document.parts {
        if let ScenePart::InlineAsset { link_id } = part {
            if let Some(mapped) = links.get(link_id) {
                *link_id = *mapped;
            }
        }
    }
    document
}

fn remap_widget_character_ids(
    node: &mut WidgetNode,
    source: CharacterId,
    destination: CharacterId,
) {
    match node {
        WidgetNode::CharacterInfo { character_id, .. } => {
            if *character_id == Some(source) {
                *character_id = Some(destination);
            }
        }
        WidgetNode::Box { children, .. } => {
            for child in children {
                remap_widget_character_ids(child, source, destination);
            }
        }
        _ => {}
    }
}

fn remap_presentation_character_id(
    presentation: &mut CharacterPresentationV1,
    source: CharacterId,
    destination: CharacterId,
) {
    for node in presentation
        .chat_appearance
        .chat_widget_slots
        .left
        .iter_mut()
        .chain(
            presentation
                .chat_appearance
                .chat_widget_slots
                .right
                .iter_mut(),
        )
    {
        remap_widget_character_ids(node, source, destination);
    }
}

fn ensure_scene_active(scene: &Scene) -> Result<(), RepositoryError> {
    if scene.status == LifecycleStatus::Archived {
        Err(RepositoryError::Archived)
    } else {
        Ok(())
    }
}

fn rewrite_scene_order(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    ordered: &[SceneId],
    previous: &BTreeMap<SceneId, u32>,
    now: TimestampMillis,
) -> Result<(), RepositoryError> {
    tx.execute(
        "UPDATE scenes SET ordinal=ordinal+1000000 WHERE character_id=?1",
        [character_id.to_string()],
    )
    .map_err(db_error)?;
    for (ordinal, scene_id) in ordered.iter().enumerate() {
        let changed = previous
            .get(scene_id)
            .is_some_and(|old| *old != ordinal as u32);
        tx.execute("UPDATE scenes SET ordinal=?3,revision=revision+?4,updated_at=CASE WHEN ?4=1 THEN ?5 ELSE updated_at END WHERE character_id=?1 AND id=?2", params![character_id.to_string(), scene_id.to_string(), i64::try_from(ordinal).map_err(|_| RepositoryError::Storage)?, changed as i64, now.get()]).map_err(db_error)?;
    }
    Ok(())
}

fn rewrite_variant_order(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    scene_id: SceneId,
    ordered: &[SceneVariantId],
    previous: &BTreeMap<SceneVariantId, u32>,
    now: TimestampMillis,
) -> Result<(), RepositoryError> {
    tx.execute(
        "UPDATE scene_variants SET ordinal=ordinal+1000000 WHERE character_id=?1 AND scene_id=?2",
        params![character_id.to_string(), scene_id.to_string()],
    )
    .map_err(db_error)?;
    for (ordinal, variant_id) in ordered.iter().enumerate() {
        let changed = previous
            .get(variant_id)
            .is_some_and(|old| *old != ordinal as u32);
        tx.execute("UPDATE scene_variants SET ordinal=?3,revision=revision+?4,updated_at=CASE WHEN ?4=1 THEN ?5 ELSE updated_at END WHERE character_id=?1 AND id=?2", params![character_id.to_string(), variant_id.to_string(), i64::try_from(ordinal).map_err(|_| RepositoryError::Storage)?, changed as i64, now.get()]).map_err(db_error)?;
    }
    Ok(())
}

fn rewrite_starter_order(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    ordered: &[ConversationStarterId],
    previous: &BTreeMap<ConversationStarterId, u32>,
    now: TimestampMillis,
) -> Result<(), RepositoryError> {
    tx.execute(
        "UPDATE conversation_starters SET ordinal=ordinal+1000000 WHERE character_id=?1",
        [character_id.to_string()],
    )
    .map_err(db_error)?;
    for (ordinal, starter_id) in ordered.iter().enumerate() {
        let changed = previous
            .get(starter_id)
            .is_some_and(|old| *old != ordinal as u32);
        tx.execute("UPDATE conversation_starters SET ordinal=?3,revision=revision+?4,updated_at=CASE WHEN ?4=1 THEN ?5 ELSE updated_at END WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), i64::try_from(ordinal).map_err(|_| RepositoryError::Storage)?, changed as i64, now.get()]).map_err(db_error)?;
    }
    Ok(())
}

fn rewrite_message_order(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    starter_id: ConversationStarterId,
    ordered: &[StarterMessageId],
) -> Result<(), RepositoryError> {
    tx.execute("UPDATE starter_messages SET ordinal=ordinal+1000000 WHERE character_id=?1 AND starter_id=?2", params![character_id.to_string(), starter_id.to_string()]).map_err(db_error)?;
    for (ordinal, message_id) in ordered.iter().enumerate() {
        tx.execute(
            "UPDATE starter_messages SET ordinal=?3 WHERE character_id=?1 AND id=?2",
            params![
                character_id.to_string(),
                message_id.to_string(),
                i64::try_from(ordinal).map_err(|_| RepositoryError::Storage)?
            ],
        )
        .map_err(db_error)?;
    }
    Ok(())
}

impl SceneRepository for Database {
    fn add_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene: Scene,
        now: TimestampMillis,
    ) -> Result<Scene, RepositoryError> {
        let mut scene = scene;
        scene.updated_at = now;
        validate(scene.validate())?;
        if scene.owner != SceneOwner::Character(character_id) {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "scene.owner",
                },
            ));
        }
        scene.validate_selected_variant(&[])?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        image_assets(&connection, scene.assets.iter().map(|asset| asset.asset_id))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        if details.scenes.iter().any(|value| value.id == scene.id) {
            return Err(RepositoryError::AlreadyExists);
        }
        if scene.ordinal as usize > details.scenes.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "scene.ordinal",
                },
            ));
        }
        let mut ordered: Vec<_> = details.scenes.iter().map(|value| value.id).collect();
        ordered.insert(scene.ordinal as usize, scene.id);
        let previous = details
            .scenes
            .iter()
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_scene_order(&tx, character_id, &ordered, &previous, now)?;
        insert_scene(&tx, character_id, &scene)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        let result = load_details(&tx, character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .scenes
            .into_iter()
            .find(|value| value.id == scene.id)
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn update_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        draft: SceneDraftUpdate,
        now: TimestampMillis,
    ) -> Result<Scene, RepositoryError> {
        draft.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, scene_id)?;
        ensure_scene_active(&scene)?;
        let replacement = Scene {
            content: draft.content.clone(),
            direction: draft.direction.clone(),
            ..scene.clone()
        };
        let variants: Vec<_> = details
            .variants
            .iter()
            .filter(|value| value.scene_id == scene_id)
            .cloned()
            .collect();
        validate_scene_graph(&details, &replacement, &variants)?;
        tx.execute("UPDATE scenes SET content_json=?3,direction=?4,revision=revision+1,updated_at=?5 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), scene_id.to_string(), encode(&draft.content, DOCUMENT_VERSION)?, draft.direction, now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        let result = load_details(&tx, character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .scenes
            .into_iter()
            .find(|value| value.id == scene_id)
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn remove_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        replacement_default: Option<SceneId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, scene_id)?;
        ensure_scene_active(&scene)?;
        if details
            .starters
            .iter()
            .any(|starter| starter.scene_id == Some(scene_id))
        {
            return Err(RepositoryError::HasDependencies);
        }
        let replacement = if details.character.defaults.default_scene_id == Some(scene_id) {
            let value = replacement_default.ok_or(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.default_scene_id",
                },
            ))?;
            if value == scene_id || !details.scenes.iter().any(|candidate| candidate.id == value) {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "character.default_scene_id",
                    },
                ));
            }
            Some(value)
        } else if replacement_default.is_some() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.default_scene_id",
                },
            ));
        } else {
            None
        };
        tx.execute(
            "DELETE FROM scenes WHERE character_id=?1 AND id=?2",
            params![character_id.to_string(), scene_id.to_string()],
        )
        .map_err(db_error)?;
        let ordered: Vec<_> = details
            .scenes
            .iter()
            .filter_map(|value| (value.id != scene_id).then_some(value.id))
            .collect();
        let previous = details
            .scenes
            .iter()
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_scene_order(&tx, character_id, &ordered, &previous, now)?;
        if details.character.defaults.default_scene_id == Some(scene_id) {
            let mut defaults = details.character.defaults;
            defaults.default_scene_id = replacement;
            tx.execute(
                "UPDATE characters SET defaults_json=?2,default_scene_id=?3 WHERE id=?1",
                params![
                    character_id.to_string(),
                    encode(&defaults, DEFAULTS_VERSION)?,
                    id_text(replacement)
                ],
            )
            .map_err(db_error)?;
        }
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn reorder_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, scene_id)?;
        ensure_scene_active(&scene)?;
        if target_ordinal as usize >= details.scenes.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "scene.ordinal",
                },
            ));
        }
        let mut ordered: Vec<_> = details.scenes.iter().map(|value| value.id).collect();
        let value = ordered.remove(scene.ordinal as usize);
        ordered.insert(target_ordinal as usize, value);
        let previous = details
            .scenes
            .iter()
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_scene_order(&tx, character_id, &ordered, &previous, now)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn add_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        variant: SceneVariant,
        now: TimestampMillis,
    ) -> Result<SceneVariant, RepositoryError> {
        let mut variant = variant;
        variant.updated_at = now;
        validate(variant.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, variant.scene_id)?;
        ensure_scene_active(&scene)?;
        if details.variants.iter().any(|value| value.id == variant.id) {
            return Err(RepositoryError::AlreadyExists);
        }
        let siblings: Vec<_> = details
            .variants
            .iter()
            .filter(|value| value.scene_id == variant.scene_id)
            .map(|value| value.id)
            .collect();
        if variant.ordinal as usize > siblings.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "scene.variant.ordinal",
                },
            ));
        }
        let all_variants: Vec<_> = details
            .variants
            .iter()
            .filter(|value| value.scene_id == variant.scene_id)
            .cloned()
            .chain(std::iter::once(variant.clone()))
            .collect();
        validate_scene_graph(&details, &scene, &all_variants)?;
        let mut ordered = siblings;
        ordered.insert(variant.ordinal as usize, variant.id);
        let previous = details
            .variants
            .iter()
            .filter(|value| value.scene_id == variant.scene_id)
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_variant_order(
            &tx,
            character_id,
            variant.scene_id,
            &ordered,
            &previous,
            now,
        )?;
        insert_variant(&tx, character_id, &variant)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        let result = load_details(&tx, character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .variants
            .into_iter()
            .find(|value| value.id == variant.id)
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn update_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        variant_id: SceneVariantId,
        draft: SceneVariantDraftUpdate,
        now: TimestampMillis,
    ) -> Result<SceneVariant, RepositoryError> {
        draft.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let variant = find_variant(&details, variant_id)?;
        let scene = find_scene(&details, variant.scene_id)?;
        ensure_scene_active(&scene)?;
        let replacement = SceneVariant {
            content: draft.content.clone(),
            direction: draft.direction.clone(),
            ..variant.clone()
        };
        let variants: Vec<_> = details
            .variants
            .iter()
            .map(|value| {
                if value.id == variant_id {
                    replacement.clone()
                } else {
                    value.clone()
                }
            })
            .filter(|value| value.scene_id == variant.scene_id)
            .collect();
        validate_scene_graph(&details, &scene, &variants)?;
        tx.execute("UPDATE scene_variants SET content_json=?3,direction=?4,revision=revision+1,updated_at=?5 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), variant_id.to_string(), encode(&draft.content, DOCUMENT_VERSION)?, draft.direction, now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        let result = load_details(&tx, character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .variants
            .into_iter()
            .find(|value| value.id == variant_id)
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn remove_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        variant_id: SceneVariantId,
        replacement_selected: Option<SceneVariantId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, scene_id)?;
        ensure_scene_active(&scene)?;
        let variant = find_variant(&details, variant_id)?;
        if variant.scene_id != scene_id {
            return Err(RepositoryError::NotFound);
        }
        let prospective_selected = if scene.selected_variant_id == Some(variant_id) {
            Some(replacement_selected.ok_or(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "scene.selected_variant_id",
                },
            ))?)
        } else {
            scene.selected_variant_id
        };
        let prospective_scene = Scene {
            selected_variant_id: prospective_selected,
            ..scene.clone()
        };
        let prospective_variants: Vec<_> = details
            .variants
            .iter()
            .filter(|value| value.id != variant_id)
            .filter(|value| value.scene_id == scene_id)
            .cloned()
            .collect();
        validate_scene_graph(&details, &prospective_scene, &prospective_variants)?;
        if scene.selected_variant_id == Some(variant_id) {
            let replacement = replacement_selected.ok_or(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "scene.selected_variant_id",
                },
            ))?;
            if replacement == variant_id
                || !details
                    .variants
                    .iter()
                    .any(|value| value.id == replacement && value.scene_id == scene_id)
            {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "scene.selected_variant_id",
                    },
                ));
            }
            tx.execute("UPDATE scenes SET selected_variant_id=?3,revision=revision+1,updated_at=?4 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), scene_id.to_string(), replacement.to_string(), now.get()]).map_err(db_error)?;
        } else if replacement_selected.is_some() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "scene.selected_variant_id",
                },
            ));
        }
        tx.execute(
            "DELETE FROM scene_variants WHERE character_id=?1 AND id=?2",
            params![character_id.to_string(), variant_id.to_string()],
        )
        .map_err(db_error)?;
        let ordered: Vec<_> = details
            .variants
            .iter()
            .filter_map(|value| {
                (value.scene_id == scene_id && value.id != variant_id).then_some(value.id)
            })
            .collect();
        let previous = details
            .variants
            .iter()
            .filter(|value| value.scene_id == scene_id)
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_variant_order(&tx, character_id, scene_id, &ordered, &previous, now)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn reorder_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        variant_id: SceneVariantId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        ensure_scene_active(&find_scene(&details, scene_id)?)?;
        let variant = find_variant(&details, variant_id)?;
        if variant.scene_id != scene_id {
            return Err(RepositoryError::NotFound);
        }
        let mut ordered: Vec<_> = details
            .variants
            .iter()
            .filter(|value| value.scene_id == scene_id)
            .map(|value| value.id)
            .collect();
        if target_ordinal as usize >= ordered.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "scene.variant.ordinal",
                },
            ));
        }
        let value = ordered.remove(variant.ordinal as usize);
        ordered.insert(target_ordinal as usize, value);
        let previous = details
            .variants
            .iter()
            .filter(|value| value.scene_id == scene_id)
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_variant_order(&tx, character_id, scene_id, &ordered, &previous, now)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn select_variant(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        variant_id: Option<SceneVariantId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, scene_id)?;
        ensure_scene_active(&scene)?;
        if let Some(value) = variant_id {
            if !details
                .variants
                .iter()
                .any(|variant| variant.id == value && variant.scene_id == scene_id)
            {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "scene.selected_variant_id",
                    },
                ));
            }
        }
        tx.execute("UPDATE scenes SET selected_variant_id=?3,revision=revision+1,updated_at=?4 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), scene_id.to_string(), id_text(variant_id), now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn replace_scene_assets(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        scene_id: SceneId,
        assets: Vec<SceneAssetLink>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        image_assets(&connection, assets.iter().map(|asset| asset.asset_id))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let scene = find_scene(&details, scene_id)?;
        ensure_scene_active(&scene)?;
        let mut replacement = scene.clone();
        replacement.assets = assets.clone();
        let variants: Vec<_> = details
            .variants
            .iter()
            .filter(|value| value.scene_id == scene_id)
            .cloned()
            .collect();
        replacement.validate_selected_variant(&variants)?;
        tx.execute(
            "DELETE FROM scene_assets WHERE character_id=?1 AND scene_id=?2",
            params![character_id.to_string(), scene_id.to_string()],
        )
        .map_err(db_error)?;
        insert_scene_assets(&tx, character_id, scene_id, &assets)?;
        tx.execute(
            "UPDATE scenes SET revision=revision+1,updated_at=?3 WHERE character_id=?1 AND id=?2",
            params![character_id.to_string(), scene_id.to_string(), now.get()],
        )
        .map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }
}

impl StarterRepository for Database {
    fn add_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter: ConversationStarter,
        now: TimestampMillis,
    ) -> Result<ConversationStarter, RepositoryError> {
        let mut starter = starter;
        starter.updated_at = now;
        validate(starter.validate())?;
        if starter.character_id != character_id {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "starter.character_id",
                },
            ));
        }
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        if details.starters.iter().any(|value| value.id == starter.id) {
            return Err(RepositoryError::AlreadyExists);
        }
        if let Some(scene_id) = starter.scene_id {
            if !details.scenes.iter().any(|scene| scene.id == scene_id) {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "starter.scene_id",
                    },
                ));
            }
        }
        if starter.ordinal as usize > details.starters.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "starter.ordinal",
                },
            ));
        }
        let mut ordered: Vec<_> = details.starters.iter().map(|value| value.id).collect();
        ordered.insert(starter.ordinal as usize, starter.id);
        let previous = details
            .starters
            .iter()
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_starter_order(&tx, character_id, &ordered, &previous, now)?;
        insert_starter(&tx, character_id, &starter)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        let result = load_details(&tx, character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .starters
            .into_iter()
            .find(|value| value.id == starter.id)
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn update_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        draft: ConversationStarterDraftUpdate,
        now: TimestampMillis,
    ) -> Result<ConversationStarter, RepositoryError> {
        draft.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let current = find_starter(&details, starter_id)?;
        if let Some(scene_id) = draft.scene_id {
            if !details.scenes.iter().any(|scene| scene.id == scene_id) {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "starter.scene_id",
                    },
                ));
            }
        }
        let replacement = ConversationStarter {
            name: draft.name.clone(),
            scene_id: draft.scene_id,
            prompt_id: draft.prompt_id,
            lorebooks: draft.lorebooks.clone(),
            ..current.clone()
        };
        validate(replacement.validate())?;
        tx.execute("UPDATE conversation_starters SET name=?3,scene_id=?4,prompt_id=?5,lorebooks_json=?6,revision=revision+1,updated_at=?7 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), draft.name, id_text(draft.scene_id), id_text(draft.prompt_id), encode(&draft.lorebooks, LOREBOOK_VERSION)?, now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        let result = load_details(&tx, character_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .starters
            .into_iter()
            .find(|value| value.id == starter_id)
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn remove_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        replacement_default: Option<ConversationStarterId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let _starter = find_starter(&details, starter_id)?;
        let replacement = if details.character.defaults.default_starter_id == Some(starter_id) {
            let value = replacement_default.ok_or(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.default_starter_id",
                },
            ))?;
            if value == starter_id || !details.starters.iter().any(|starter| starter.id == value) {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "character.default_starter_id",
                    },
                ));
            }
            Some(value)
        } else if replacement_default.is_some() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "character.default_starter_id",
                },
            ));
        } else {
            None
        };
        tx.execute(
            "DELETE FROM conversation_starters WHERE character_id=?1 AND id=?2",
            params![character_id.to_string(), starter_id.to_string()],
        )
        .map_err(db_error)?;
        let ordered: Vec<_> = details
            .starters
            .iter()
            .filter_map(|value| (value.id != starter_id).then_some(value.id))
            .collect();
        let previous = details
            .starters
            .iter()
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_starter_order(&tx, character_id, &ordered, &previous, now)?;
        if details.character.defaults.default_starter_id == Some(starter_id) {
            let mut defaults = details.character.defaults;
            defaults.default_starter_id = replacement;
            tx.execute(
                "UPDATE characters SET defaults_json=?2,default_starter_id=?3 WHERE id=?1",
                params![
                    character_id.to_string(),
                    encode(&defaults, DEFAULTS_VERSION)?,
                    id_text(replacement)
                ],
            )
            .map_err(db_error)?;
        }
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn reorder_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let starter = find_starter(&details, starter_id)?;
        if target_ordinal as usize >= details.starters.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "starter.ordinal",
                },
            ));
        }
        let mut ordered: Vec<_> = details.starters.iter().map(|value| value.id).collect();
        let value = ordered.remove(starter.ordinal as usize);
        ordered.insert(target_ordinal as usize, value);
        let previous = details
            .starters
            .iter()
            .map(|value| (value.id, value.ordinal))
            .collect();
        rewrite_starter_order(&tx, character_id, &ordered, &previous, now)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn insert_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message: StarterMessage,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        validate(message.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let starter = find_starter(&details, starter_id)?;
        if starter.messages.iter().any(|value| value.id == message.id) {
            return Err(RepositoryError::AlreadyExists);
        }
        if target_ordinal as usize > starter.messages.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "starter.message.ordinal",
                },
            ));
        }
        let mut ordered: Vec<_> = starter.messages.iter().map(|value| value.id).collect();
        ordered.insert(target_ordinal as usize, message.id);
        rewrite_message_order(&tx, character_id, starter_id, &ordered)?;
        tx.execute("INSERT INTO starter_messages(character_id,starter_id,id,role,content,ordinal) VALUES (?1,?2,?3,?4,?5,?6)", params![character_id.to_string(), starter_id.to_string(), message.id.to_string(), role_name(message.role), message.content, i64::from(target_ordinal)]).map_err(db_error)?;
        tx.execute("UPDATE conversation_starters SET revision=revision+1,updated_at=?3 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn update_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message: StarterMessage,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        validate(message.validate())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let starter = find_starter(&details, starter_id)?;
        if !starter.messages.iter().any(|value| value.id == message.id) {
            return Err(RepositoryError::NotFound);
        }
        tx.execute(
            "UPDATE starter_messages SET role=?3,content=?4 WHERE character_id=?1 AND id=?2",
            params![
                character_id.to_string(),
                message.id.to_string(),
                role_name(message.role),
                message.content
            ],
        )
        .map_err(db_error)?;
        tx.execute("UPDATE conversation_starters SET revision=revision+1,updated_at=?3 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn remove_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message_id: StarterMessageId,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let starter = find_starter(&details, starter_id)?;
        if !starter.messages.iter().any(|value| value.id == message_id) {
            return Err(RepositoryError::NotFound);
        }
        tx.execute(
            "DELETE FROM starter_messages WHERE character_id=?1 AND id=?2",
            params![character_id.to_string(), message_id.to_string()],
        )
        .map_err(db_error)?;
        let ordered: Vec<_> = starter
            .messages
            .iter()
            .filter_map(|value| (value.id != message_id).then_some(value.id))
            .collect();
        rewrite_message_order(&tx, character_id, starter_id, &ordered)?;
        tx.execute("UPDATE conversation_starters SET revision=revision+1,updated_at=?3 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn reorder_message(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        message_id: StarterMessageId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let starter = find_starter(&details, starter_id)?;
        let Some(current) = starter
            .messages
            .iter()
            .position(|value| value.id == message_id)
        else {
            return Err(RepositoryError::NotFound);
        };
        if target_ordinal as usize >= starter.messages.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "starter.message.ordinal",
                },
            ));
        }
        let mut ordered: Vec<_> = starter.messages.iter().map(|value| value.id).collect();
        let value = ordered.remove(current);
        ordered.insert(target_ordinal as usize, value);
        rewrite_message_order(&tx, character_id, starter_id, &ordered)?;
        tx.execute("UPDATE conversation_starters SET revision=revision+1,updated_at=?3 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }

    fn change_scene(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        scene_id: Option<SceneId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        self.update_starter_selection(
            character_id,
            expected_character_revision,
            starter_id,
            Some(scene_id),
            None,
            None,
            now,
        )
    }
    fn change_prompt(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        prompt_id: Option<PromptDocumentId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        self.update_starter_selection(
            character_id,
            expected_character_revision,
            starter_id,
            None,
            Some(prompt_id),
            None,
            now,
        )
    }
    fn change_lorebooks(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        lorebooks: Selection<Vec<LorebookId>>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        self.update_starter_selection(
            character_id,
            expected_character_revision,
            starter_id,
            None,
            None,
            Some(lorebooks),
            now,
        )
    }
    fn set_default_starter(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: Option<ConversationStarterId>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        if let Some(value) = starter_id {
            if !details.starters.iter().any(|starter| starter.id == value) {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "character.default_starter_id",
                    },
                ));
            }
        }
        let mut defaults = details.character.defaults;
        defaults.default_starter_id = starter_id;
        tx.execute(
            "UPDATE characters SET defaults_json=?2,default_starter_id=?3 WHERE id=?1",
            params![
                character_id.to_string(),
                encode(&defaults, DEFAULTS_VERSION)?,
                id_text(starter_id)
            ],
        )
        .map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }
}

impl Database {
    #[allow(clippy::too_many_arguments)]
    fn update_starter_selection(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        starter_id: ConversationStarterId,
        scene_id: Option<Option<SceneId>>,
        prompt_id: Option<Option<PromptDocumentId>>,
        lorebooks: Option<Selection<Vec<LorebookId>>>,
        now: TimestampMillis,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let details = ensure_root(&tx, character_id, expected_character_revision, false)?;
        let current = find_starter(&details, starter_id)?;
        let new_scene = scene_id.unwrap_or(current.scene_id);
        if let Some(value) = new_scene {
            if !details.scenes.iter().any(|scene| scene.id == value) {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::InvalidReference {
                        field: "starter.scene_id",
                    },
                ));
            }
        }
        let new_prompt = prompt_id.unwrap_or(current.prompt_id);
        let new_lorebooks = lorebooks.unwrap_or(current.lorebooks);
        let draft = ConversationStarterDraftUpdate {
            name: current.name,
            scene_id: new_scene,
            prompt_id: new_prompt,
            lorebooks: new_lorebooks.clone(),
        };
        draft.validate()?;
        tx.execute("UPDATE conversation_starters SET scene_id=?3,prompt_id=?4,lorebooks_json=?5,revision=revision+1,updated_at=?6 WHERE character_id=?1 AND id=?2", params![character_id.to_string(), starter_id.to_string(), id_text(new_scene), id_text(new_prompt), encode(&new_lorebooks, LOREBOOK_VERSION)?, now.get()]).map_err(db_error)?;
        bump_root(&tx, character_id, expected_character_revision, now)?;
        tx.commit().map_err(db_error)
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use lettuce_characters::{
        CharacterMedia, CharacterPresentationV1, CharacterProvenance, ScenePart,
    };
    use lettuce_companions::{
        CompanionSoulConfig, CompanionSoulIdentity, SoulCategory, SoulFact, SoulFactKind,
        SoulFactPolicy, SoulRepository,
    };
    use lettuce_models::{ModelProfileRepository, ModelRepositoryError};
    use lettuce_types::{PageLimit, PageRequest};

    fn image_asset(database: &Database, byte: u8) -> AssetId {
        let blob_id = lettuce_types::MediaBlobId::new();
        let asset_id = AssetId::new();
        let mut hash = asset_id.to_string().replace('-', "");
        hash.push_str(&hash.clone());
        hash.replace_range(0..2, &format!("{byte:02x}"));
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO media_blobs (id,content_hash,kind,mime_type,byte_size,width,height,duration_ms,validation_version,state,created_at,updated_at) VALUES (?1,?2,'image','image/png',1,1,1,NULL,1,'ready',1,1)",
                params![blob_id.to_string(), hash],
            )
            .expect("blob");
        connection
            .execute(
                "INSERT INTO media_assets (id,blob_id,blob_kind,kind,origin,retention,expires_at,provenance_json,revision,created_at,updated_at) VALUES (?1,?2,'image','other_image','upload','library',NULL,'{}',1,1,1)",
                params![asset_id.to_string(), blob_id.to_string()],
            )
            .expect("asset");
        asset_id
    }

    fn companion_plan(character_id: CharacterId) -> CreateCharacterPlan {
        let companion_soul = CompanionSoulConfig {
            soul: CompanionSoulIdentity {
                essence: "Quietly determined".into(),
                traits: "Patient and observant".into(),
                backstory: "Moved to the coast".into(),
                appearance: "Weathered coat".into(),
                goals: "Build a home".into(),
                likes: "Rain".into(),
                voice: "Low and measured".into(),
                relational_style: "Earns trust slowly".into(),
                vulnerabilities: "Fears dependence".into(),
                fears: "Being forgotten".into(),
                habits: "Counts doorways".into(),
                boundaries: "Needs solitude".into(),
                ..CompanionSoulIdentity::default()
            },
            authored_facts: [
                SoulCategory::Essence,
                SoulCategory::Traits,
                SoulCategory::Backstory,
                SoulCategory::Appearance,
                SoulCategory::Goals,
                SoulCategory::Likes,
                SoulCategory::Voice,
                SoulCategory::RelationalStyle,
                SoulCategory::Vulnerabilities,
                SoulCategory::Fears,
                SoulCategory::Habits,
                SoulCategory::Boundaries,
            ]
            .into_iter()
            .map(|category| SoulFact {
                id: String::new(),
                category,
                value: category.as_str().to_owned(),
                kind: SoulFactKind::Authored,
                policy: if category == SoulCategory::Backstory {
                    SoulFactPolicy::Historical
                } else {
                    SoulFactPolicy::Adaptive
                },
                slot: category.as_str().to_owned(),
                confidence: 1.0,
                evidence_count: 1,
                weight: 1.0,
                valid_from: TimestampMillis::UNIX_EPOCH,
                valid_until: None,
                locked: false,
                source_memory_ids: Vec::new(),
                created_at: TimestampMillis::UNIX_EPOCH,
                supersedes: Vec::new(),
                superseded_by: None,
                superseded_at: None,
            })
            .collect(),
            relationship_defaults: lettuce_companions::RelationshipDefaults::default(),
            prompting: lettuce_companions::CompanionPromptingConfig::default(),
        };
        let character = Character::new(
            character_id,
            CharacterProfile {
                name: "Companion Ada".into(),
                nickname: None,
                description: None,
                definition: Some("A companion".into()),
                design_description: None,
            },
            CharacterProvenance::default(),
            CharacterDefaults {
                interaction_mode: lettuce_characters::InteractionMode::Companion,
                companion_soul: Some(companion_soul),
                ..CharacterDefaults::default()
            },
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(7),
        )
        .expect("companion character");
        CreateCharacterPlan {
            character,
            scenes: Vec::new(),
            variants: Vec::new(),
            starters: Vec::new(),
        }
    }

    #[test]
    fn companion_character_create_seeds_authored_soul_atomically() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let plan = companion_plan(character_id);
        CharacterRepository::create(&database, plan.clone()).expect("create");
        let loaded = CharacterRepository::get(&database, character_id)
            .expect("get character")
            .expect("character");
        assert_eq!(
            loaded.character.defaults.companion_soul,
            plan.character.defaults.companion_soul
        );
        let state = SoulRepository::get(&database, SoulOwner::Character(character_id))
            .expect("get soul")
            .expect("soul");
        assert_eq!(state.revision, Revision::INITIAL);
        assert_eq!(state.facts.len(), 12);
        assert!(state.facts.iter().all(|fact| {
            fact.created_at == TimestampMillis::new(7) && fact.valid_from == TimestampMillis::new(7)
        }));
        assert!(
            state
                .facts
                .iter()
                .find(|fact| fact.category == SoulCategory::Backstory)
                .expect("backstory")
                .locked
        );
    }

    #[test]
    fn roleplay_character_create_does_not_seed_soul_state() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let mut plan = companion_plan(character_id);
        plan.character.defaults.interaction_mode = lettuce_characters::InteractionMode::Roleplay;
        plan.character.defaults.companion_soul = None;
        CharacterRepository::create(&database, plan).expect("create");
        assert_eq!(
            SoulRepository::get(&database, SoulOwner::Character(character_id)).expect("get soul"),
            None
        );
    }

    #[test]
    fn authored_soul_failure_rolls_back_character_root() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        {
            let connection = database.connection().expect("connection");
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_authored_soul
                     BEFORE INSERT ON companion_soul_facts
                     BEGIN
                         SELECT RAISE(ABORT, 'reject authored soul');
                     END;",
                )
                .expect("trigger");
        }
        assert_eq!(
            CharacterRepository::create(&database, companion_plan(character_id)),
            Err(RepositoryError::Storage)
        );
        assert_eq!(
            CharacterRepository::get(&database, character_id).expect("get character"),
            None
        );
        assert_eq!(
            SoulRepository::get(&database, SoulOwner::Character(character_id)).expect("get soul"),
            None
        );
    }

    fn graph_fixture(
        database: &Database,
    ) -> (CreateCharacterPlan, AssetId, SceneId, ConversationStarterId) {
        let media_asset = image_asset(database, 1);
        let scene_asset_a = image_asset(database, 2);
        let scene_asset_b = image_asset(database, 3);
        let character_id = CharacterId::new();
        let scene_id = SceneId::new();
        let link_a = SceneAssetLinkId::new();
        let link_b = SceneAssetLinkId::new();
        let variant_id = SceneVariantId::new();
        let starter_id = ConversationStarterId::new();
        let scene = Scene {
            id: scene_id,
            owner: SceneOwner::Character(character_id),
            status: LifecycleStatus::Active,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::InlineAsset { link_id: link_a }])
                .expect("scene document"),
            direction: Some("Begin quietly".into()),
            selected_variant_id: Some(variant_id),
            assets: vec![
                SceneAssetLink {
                    id: link_a,
                    asset_id: scene_asset_a,
                    slot: SceneAssetSlot::Inline,
                    ordinal: 0,
                },
                SceneAssetLink {
                    id: link_b,
                    asset_id: scene_asset_b,
                    slot: SceneAssetSlot::Inline,
                    ordinal: 1,
                },
            ],
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let variant = SceneVariant {
            id: variant_id,
            scene_id,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::InlineAsset { link_id: link_b }])
                .expect("variant document"),
            direction: Some("Or with urgency".into()),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let starter = ConversationStarter {
            id: starter_id,
            character_id,
            name: "Opening".into(),
            ordinal: 0,
            messages: vec![StarterMessage {
                id: StarterMessageId::new(),
                role: StarterRole::Assistant,
                content: "You are awake.".into(),
            }],
            scene_id: Some(scene_id),
            prompt_id: Some(PromptDocumentId::new()),
            lorebooks: Selection::Explicit(vec![LorebookId::new()]),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let character = Character::new(
            character_id,
            CharacterProfile {
                name: "Graph Ada".into(),
                nickname: Some("Ada".into()),
                description: Some("A complete graph fixture".into()),
                definition: None,
                design_description: None,
            },
            CharacterProvenance::default(),
            CharacterDefaults {
                default_scene_id: Some(scene_id),
                default_starter_id: Some(starter_id),
                ..CharacterDefaults::default()
            },
            CharacterPresentationV1::default(),
            None,
            CharacterMedia {
                links: vec![CharacterMediaLink {
                    asset_id: media_asset,
                    slot: CharacterMediaSlot::AvatarOriginal,
                    ordinal: 0,
                }],
            },
            TimestampMillis::new(1),
        )
        .expect("character");
        (
            CreateCharacterPlan {
                character,
                scenes: vec![scene],
                variants: vec![variant],
                starters: vec![starter],
            },
            media_asset,
            scene_id,
            starter_id,
        )
    }

    #[test]
    fn character_create_and_get_round_trip() {
        let database = Database::open_in_memory().expect("database");
        let character = Character::new(
            CharacterId::new(),
            CharacterProfile {
                name: "Ada".into(),
                nickname: Some("A".into()),
                description: None,
                definition: None,
                design_description: None,
            },
            CharacterProvenance::default(),
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(1),
        )
        .expect("character");
        let details = CharacterRepository::create(
            &database,
            CreateCharacterPlan {
                character: character.clone(),
                scenes: Vec::new(),
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("create");
        assert_eq!(details.character, character);
        assert_eq!(
            CharacterRepository::get(&database, character.id).expect("get"),
            Some(details)
        );
    }

    #[test]
    fn complete_graph_round_trip_and_dependency_report_are_exact() {
        let database = Database::open_in_memory().expect("database");
        let (plan, media_asset, scene_id, starter_id) = graph_fixture(&database);
        let expected = plan.clone();
        let details = CharacterRepository::create(&database, plan).expect("create graph");
        assert_eq!(details.character.media.links[0].asset_id, media_asset);
        assert_eq!(details.scenes[0].assets.len(), 2);
        assert_eq!(details.variants[0].content, expected.variants[0].content);
        assert_eq!(details.starters[0].messages.len(), 1);
        assert_eq!(
            CharacterRepository::get(&database, details.character.id).expect("reload"),
            Some(details.clone())
        );
        let dependencies = CharacterDependencyReader::dependencies(&database, details.character.id)
            .expect("dependencies")
            .references;
        assert!(dependencies.contains(&DependencyReference::CharacterDefaultScene { scene_id }));
        assert!(
            dependencies.contains(&DependencyReference::CharacterDefaultStarter { starter_id })
        );
        assert!(dependencies.contains(&DependencyReference::Asset {
            asset_id: media_asset
        }));
        assert!(dependencies.iter().any(|reference| matches!(
            reference,
            DependencyReference::StarterScene { starter_id: id, scene_id: scene }
                if *id == starter_id && *scene == scene_id
        )));
    }

    #[test]
    fn starter_messages_round_trip_empty_content() {
        let database = Database::open_in_memory().expect("database");
        let (mut plan, _, _, _) = graph_fixture(&database);
        plan.starters[0].messages[0].content.clear();
        let details = CharacterRepository::create(&database, plan).expect("create graph");
        assert_eq!(details.starters[0].messages[0].content, "");
        assert_eq!(
            CharacterRepository::get(&database, details.character.id)
                .expect("reload")
                .expect("character")
                .starters[0]
                .messages[0]
                .content,
            ""
        );
    }

    #[test]
    fn canonical_unicode_names_are_searchable_and_verified_on_read() {
        let database = Database::open_in_memory().expect("database");
        let character = Character::new(
            CharacterId::new(),
            CharacterProfile {
                name: "Äda Lovelace".into(),
                nickname: Some("Женя".into()),
                description: None,
                definition: None,
                design_description: None,
            },
            CharacterProvenance::default(),
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(1),
        )
        .expect("character");
        CharacterRepository::create(
            &database,
            CreateCharacterPlan {
                character: character.clone(),
                scenes: Vec::new(),
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("create");
        let by_name = CharacterRepository::search(
            &database,
            CharacterSearch {
                text: "äDA".into(),
                include_archived: false,
            },
            PageRequest::default(),
        )
        .expect("name search");
        assert_eq!(by_name.items[0].id, character.id);
        let by_nickname = CharacterRepository::search(
            &database,
            CharacterSearch {
                text: "ЖЕН".into(),
                include_archived: false,
            },
            PageRequest::default(),
        )
        .expect("nickname search");
        assert_eq!(by_nickname.items[0].id, character.id);

        let connection = database.connection().expect("database lock");
        let normalized: (String, Option<String>) = connection
            .query_row(
                "SELECT normalized_name,normalized_nickname FROM characters WHERE id=?1",
                [character.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("normalized names");
        assert_eq!(normalized.0, character.profile.name.to_lowercase());
        let expected_nickname = character.profile.nickname.as_deref().map(str::to_lowercase);
        assert_eq!(normalized.1.as_deref(), expected_nickname.as_deref());
        connection
            .execute(
                "UPDATE characters SET normalized_name='corrupt' WHERE id=?1",
                [character.id.to_string()],
            )
            .expect("corrupt normalized name");
        drop(connection);
        assert!(matches!(
            CharacterRepository::get(&database, character.id),
            Err(RepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn aggregate_read_transaction_keeps_a_two_handle_snapshot() {
        let path =
            std::env::temp_dir().join(format!("lettuce-character-snapshot-{}.db", AssetId::new()));
        let setup = Database::open(&path).expect("database");
        let (plan, _, _, starter_id) = graph_fixture(&setup);
        let details = CharacterRepository::create(&setup, plan).expect("create graph");
        let character_id = details.character.id;
        let old_name = details.starters[0].name.clone();
        drop(setup);

        let reader = Database::open(&path).expect("reader");
        let writer = Database::open(&path).expect("writer");
        let mut reader_connection = reader.connection().expect("reader lock");
        let tx = reader_connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("read transaction");
        tx.query_row(
            "SELECT revision FROM characters WHERE id=?1",
            [character_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .expect("establish snapshot");
        let writer_connection = writer.connection().expect("writer lock");
        writer_connection
            .execute(
                "UPDATE conversation_starters SET name='Changed' WHERE character_id=?1 AND id=?2",
                params![character_id.to_string(), starter_id.to_string()],
            )
            .expect("write after snapshot");
        drop(writer_connection);

        let snapshot = load_details(&tx, character_id)
            .expect("read snapshot")
            .expect("character");
        assert_eq!(snapshot.starters[0].name, old_name);
        tx.commit().expect("commit read transaction");
        drop(reader_connection);
        let latest = CharacterRepository::get(&reader, character_id)
            .expect("latest read")
            .expect("latest character");
        assert_eq!(latest.starters[0].name, "Changed");
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn selected_variant_foreign_key_includes_scene_identity() {
        let database = Database::open_in_memory().expect("database");
        let (mut plan, _, first_scene_id, _) = graph_fixture(&database);
        let second_scene_id = SceneId::new();
        let second_variant_id = SceneVariantId::new();
        let character_id = plan.character.id;
        plan.scenes.push(Scene {
            id: second_scene_id,
            owner: SceneOwner::Character(character_id),
            status: LifecycleStatus::Active,
            ordinal: 1,
            content: SceneDocumentV1::new(Vec::new()).expect("scene document"),
            direction: None,
            selected_variant_id: None,
            assets: Vec::new(),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        });
        plan.variants.push(SceneVariant {
            id: second_variant_id,
            scene_id: second_scene_id,
            ordinal: 0,
            content: SceneDocumentV1::new(Vec::new()).expect("variant document"),
            direction: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        });
        CharacterRepository::create(&database, plan).expect("create graph");
        let connection = database.connection().expect("database lock");
        let error = connection
            .execute(
                "UPDATE scenes SET selected_variant_id=?3 WHERE character_id=?1 AND id=?2",
                params![
                    character_id.to_string(),
                    first_scene_id.to_string(),
                    second_variant_id.to_string()
                ],
            )
            .expect_err("cross-scene selected variant must fail");
        assert!(matches!(error, rusqlite::Error::SqliteFailure(_, _)));
        drop(connection);
        assert!(
            CharacterRepository::get(&database, character_id)
                .expect("reload")
                .is_some()
        );
    }

    #[test]
    fn starter_scene_can_be_cleared_and_media_detach_compacts_design_order() {
        let database = Database::open_in_memory().expect("database");
        let (mut plan, _, _scene_id, starter_id) = graph_fixture(&database);
        let design_a = image_asset(&database, 4);
        let design_b = image_asset(&database, 5);
        plan.character.media = CharacterMedia {
            links: vec![
                CharacterMediaLink {
                    asset_id: design_a,
                    slot: CharacterMediaSlot::DesignReference,
                    ordinal: 0,
                },
                CharacterMediaLink {
                    asset_id: design_b,
                    slot: CharacterMediaSlot::DesignReference,
                    ordinal: 1,
                },
            ],
        };
        let details = CharacterRepository::create(&database, plan).expect("create graph");
        StarterRepository::change_scene(
            &database,
            details.character.id,
            details.character.revision,
            starter_id,
            None,
            TimestampMillis::new(2),
        )
        .expect("clear starter scene");
        let after_scene = CharacterRepository::get(&database, details.character.id)
            .expect("get")
            .expect("character");
        assert_eq!(after_scene.starters[0].scene_id, None);
        let after_detach = CharacterRepository::detach_media(
            &database,
            details.character.id,
            after_scene.character.revision,
            design_a,
            CharacterMediaSlot::DesignReference,
            TimestampMillis::new(3),
        )
        .expect("detach");
        assert_eq!(after_detach.media.links[0].asset_id, design_b);
        assert_eq!(after_detach.media.links[0].ordinal, 0);
        assert_eq!(after_detach.revision, Revision::new(3));
    }

    #[test]
    fn duplication_remaps_owned_graph_ids_and_shares_external_assets() {
        let database = Database::open_in_memory().expect("database");
        let (plan, media_asset, _, _) = graph_fixture(&database);
        let source = CharacterRepository::create(&database, plan).expect("source");
        let request = ProfileDuplicateRequest {
            source_character_id: source.character.id,
            destination_character_id: CharacterId::new(),
            destination_name: Some("Copied Ada".into()),
            now: TimestampMillis::new(10),
        };
        let result = ProfileDuplicateRepository::duplicate_character(&database, request.clone())
            .expect("duplicate");
        result.validate_for(&request).expect("valid remaps");
        let copy = CharacterRepository::get(&database, result.character_id)
            .expect("copy get")
            .expect("copy");
        assert_eq!(copy.character.profile.name, "Copied Ada");
        assert_eq!(copy.character.media.links[0].asset_id, media_asset);
        assert_ne!(copy.scenes[0].id, source.scenes[0].id);
        assert_ne!(copy.variants[0].id, source.variants[0].id);
        assert_eq!(copy.character.revision, Revision::INITIAL);
        assert_eq!(copy.scenes[0].revision, Revision::INITIAL);
        assert_eq!(copy.starters[0].revision, Revision::INITIAL);
    }

    #[test]
    fn invalid_inline_asset_replacement_and_corrupt_rows_roll_back_strictly() {
        let database = Database::open_in_memory().expect("database");
        let (plan, _, scene_id, _) = graph_fixture(&database);
        let details = CharacterRepository::create(&database, plan).expect("create");
        let original_revision = details.character.revision;
        let original_assets = details.scenes[0].assets.clone();
        assert!(matches!(
            SceneRepository::replace_scene_assets(
                &database,
                details.character.id,
                original_revision,
                scene_id,
                vec![original_assets[0].clone()],
                TimestampMillis::new(2),
            ),
            Err(RepositoryError::Invalid(_))
        ));
        let unchanged = CharacterRepository::get(&database, details.character.id)
            .expect("get")
            .expect("character");
        assert_eq!(unchanged.character.revision, original_revision);
        assert_eq!(unchanged.scenes[0].assets, original_assets);

        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "UPDATE characters SET profile_json='not-json' WHERE id=?1",
                [details.character.id.to_string()],
            )
            .expect("corrupt row");
        drop(connection);
        assert!(matches!(
            CharacterRepository::get(&database, details.character.id),
            Err(RepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn model_profile_delete_reports_character_default_dependency() {
        let database = Database::open_in_memory().expect("database");
        let provider_id = lettuce_types::ProviderAccountId::new();
        let model_id = lettuce_types::ModelProfileId::new();
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO provider_accounts (id,provider_kind,protocol,label,endpoint,enabled,streaming_enabled,allow_invalid_tls,api_key_secret_ref,secret_owner_id,secret_headers_json,config_json,revision,created_at,updated_at) VALUES (?1,'test','open_ai_compatible','Test',NULL,1,1,0,NULL,?2,'[]','{\"format_version\":1,\"value\":{\"kind\":\"standard\"}}',1,1,1)",
                rusqlite::params![provider_id.to_string(), lettuce_settings::SecretOwnerId::new().as_uuid().to_string()],
            )
            .expect("provider");
        connection
            .execute(
                "INSERT INTO model_profiles (id,provider_account_id,external_model_id,display_name,kind,config_json,revision,created_at,updated_at) VALUES (?1,?2,'test-model','Test Model','chat','{\"format_version\":1,\"value\":{\"chat_parameters\":{},\"capabilities\":{\"format_version\":1,\"evidence\":{\"source\":\"unspecified\",\"source_version\":0,\"observed_at\":0},\"input_modalities\":{\"text\":\"unknown\",\"image\":\"unknown\",\"audio\":\"unknown\"},\"output_modalities\":{\"text\":\"unknown\",\"image\":\"unknown\",\"audio\":\"unknown\"},\"streaming\":\"unknown\",\"tools\":\"unknown\",\"structured_output\":\"unknown\",\"reasoning\":\"unknown\",\"prompt_cache\":\"unknown\",\"context_length\":null,\"max_visible_output_tokens\":null,\"max_total_completion_tokens\":null,\"parameter_support\":{\"temperature\":\"unknown\",\"top_p\":\"unknown\",\"top_k\":\"unknown\",\"frequency_penalty\":\"unknown\",\"presence_penalty\":\"unknown\",\"repetition_penalty\":\"unknown\"}}}}',1,1,1)",
                params![model_id.to_string(), provider_id.to_string()],
            )
            .expect("model");
        drop(connection);
        let mut character = graph_fixture(&database).0.character;
        character.id = CharacterId::new();
        character.defaults = CharacterDefaults {
            model_profile_id: Some(model_id),
            ..CharacterDefaults::default()
        };
        character.media = CharacterMedia::default();
        CharacterRepository::create(
            &database,
            CreateCharacterPlan {
                character,
                scenes: Vec::new(),
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("character");
        assert!(matches!(
            ModelProfileRepository::delete_and_clear_default(&database, model_id),
            Err(ModelRepositoryError::InUse(dependencies)) if dependencies.len() == 1
        ));
    }

    #[test]
    fn list_and_search_use_keyset_pages_without_archived_records() {
        let database = Database::open_in_memory().expect("database");
        let mut ids = Vec::new();
        for name in ["Alpha", "Alpine", "Beta"] {
            let mut plan = graph_fixture(&database).0;
            plan.character.id = CharacterId::new();
            plan.character.profile.name = name.into();
            plan.scenes.clear();
            plan.variants.clear();
            plan.starters.clear();
            plan.character.defaults = CharacterDefaults::default();
            plan.character.media = CharacterMedia::default();
            ids.push(
                CharacterRepository::create(&database, plan)
                    .expect("create")
                    .character
                    .id,
            );
        }
        let first = CharacterRepository::list(
            &database,
            PageRequest {
                cursor: None,
                limit: PageLimit::new(2),
            },
            false,
        )
        .expect("first page");
        assert_eq!(first.items.len(), 2);
        let second = CharacterRepository::list(
            &database,
            PageRequest {
                cursor: first.next_cursor,
                limit: PageLimit::new(2),
            },
            false,
        )
        .expect("second page");
        assert_eq!(second.items.len(), 1);
        let search = CharacterRepository::search(
            &database,
            CharacterSearch {
                text: "alp".into(),
                include_archived: false,
            },
            PageRequest {
                cursor: None,
                limit: PageLimit::new(50),
            },
        )
        .expect("search");
        assert_eq!(search.items.len(), 2);
        CharacterRepository::archive(
            &database,
            ids[0],
            CharacterRepository::get(&database, ids[0])
                .expect("get")
                .expect("character")
                .character
                .revision,
            TimestampMillis::new(20),
        )
        .expect("archive");
        assert_eq!(
            CharacterRepository::search(
                &database,
                CharacterSearch {
                    text: "alp".into(),
                    include_archived: false
                },
                PageRequest::default()
            )
            .expect("search active")
            .items
            .len(),
            1
        );
    }
}
