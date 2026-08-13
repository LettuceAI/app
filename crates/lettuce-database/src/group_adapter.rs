//! SQLite persistence for reusable authored group profiles.
//!
//! A group is one aggregate: its members, presentation asset associations and
//! optional starting scene are read and written in the same transaction.  The
//! tables intentionally do not mention sessions or conversation history.

use std::collections::BTreeSet;

use lettuce_characters::{
    ChatAppearanceV1, ChatMode, CreateGroupPlan, DependencyReference, DependencyReport,
    GroupDetails, GroupMember, GroupProfile, GroupRepository, GroupStartingScene, LifecycleStatus,
    MemoryPolicy, RepositoryError, Scene, SceneAssetLink, SceneAssetSlot, SceneOwner, SceneVariant,
    Selection, SpeakerSelection, ValidationError,
};
use lettuce_types::{
    AssetId, CharacterId, GroupId, ModelProfileId, Page, PageRequest, PersonaId, PromptDocumentId,
    Revision, SceneId, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use unicode_normalization::UnicodeNormalization;

use super::Database;

const PRESENTATION_VERSION: u32 = 1;
const DOCUMENT_VERSION: u32 = 1;
const CURSOR_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    format_version: u32,
    value: T,
}

fn invalid() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn db_error(error: rusqlite::Error) -> RepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => RepositoryError::Invalid(ValidationError::Invariant {
            field: "group.storage",
        }),
        rusqlite::Error::SqliteFailure(code, _) if matches!(code.extended_code, 1555 | 2067) => {
            RepositoryError::AlreadyExists
        }
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 787 => {
            RepositoryError::Invalid(ValidationError::InvalidReference {
                field: "group.reference",
            })
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            RepositoryError::Invalid(ValidationError::Invariant {
                field: "group.storage",
            })
        }
        _ => RepositoryError::Storage,
    }
}

fn encode<T: Serialize>(value: &T, version: u32) -> Result<String, RepositoryError> {
    serde_json::to_string(&Envelope {
        format_version: version,
        value,
    })
    .map_err(|_| {
        RepositoryError::Invalid(ValidationError::Invariant {
            field: "group.encoded",
        })
    })
}

fn decode<T: DeserializeOwned>(value: &str, version: u32) -> Result<T, rusqlite::Error> {
    let envelope: Envelope<T> = serde_json::from_str(value).map_err(|_| invalid())?;
    if envelope.format_version != version {
        return Err(invalid());
    }
    Ok(envelope.value)
}

fn canonical_name(value: &str) -> String {
    value
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .nfkc()
        .collect()
}

fn id<T: std::str::FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| invalid())
}

fn revision(value: i64) -> Result<Revision, rusqlite::Error> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| invalid())
}

fn sql_revision(value: Revision) -> Result<i64, RepositoryError> {
    i64::try_from(value.get()).map_err(|_| RepositoryError::Storage)
}

fn status_name(value: LifecycleStatus) -> &'static str {
    match value {
        LifecycleStatus::Active => "active",
        LifecycleStatus::Archived => "archived",
    }
}

fn status(value: &str) -> Result<LifecycleStatus, rusqlite::Error> {
    match value {
        "active" => Ok(LifecycleStatus::Active),
        "archived" => Ok(LifecycleStatus::Archived),
        _ => Err(invalid()),
    }
}

fn chat_mode_name(value: ChatMode) -> &'static str {
    match value {
        ChatMode::Conversation => "conversation",
        ChatMode::Roleplay => "roleplay",
    }
}

fn chat_mode(value: &str) -> Result<ChatMode, rusqlite::Error> {
    match value {
        "conversation" => Ok(ChatMode::Conversation),
        "roleplay" => Ok(ChatMode::Roleplay),
        _ => Err(invalid()),
    }
}

fn selection_kind<T>(value: &Selection<T>) -> &'static str {
    match value {
        Selection::Inherit => "inherit",
        Selection::Explicit(_) => "explicit",
        Selection::Disabled => "disabled",
    }
}

fn persona_selection(
    value: &str,
    id: Option<PersonaId>,
) -> Result<Selection<PersonaId>, rusqlite::Error> {
    match (value, id) {
        ("inherit", None) => Ok(Selection::Inherit),
        ("explicit", Some(id)) => Ok(Selection::Explicit(id)),
        ("disabled", None) => Ok(Selection::Disabled),
        _ => Err(invalid()),
    }
}

fn speaker_name(value: SpeakerSelection) -> &'static str {
    match value {
        SpeakerSelection::Llm => "llm",
        SpeakerSelection::Heuristic => "heuristic",
        SpeakerSelection::RoundRobin => "round_robin",
        SpeakerSelection::Director => "director",
        SpeakerSelection::DirectorAction => "director_action",
    }
}

fn speaker(value: &str) -> Result<SpeakerSelection, rusqlite::Error> {
    match value {
        "llm" => Ok(SpeakerSelection::Llm),
        "heuristic" => Ok(SpeakerSelection::Heuristic),
        "round_robin" => Ok(SpeakerSelection::RoundRobin),
        "director" => Ok(SpeakerSelection::Director),
        "director_action" => Ok(SpeakerSelection::DirectorAction),
        _ => Err(invalid()),
    }
}

fn memory_name(value: MemoryPolicy) -> &'static str {
    match value {
        MemoryPolicy::Manual => "manual",
        MemoryPolicy::Dynamic => "dynamic",
    }
}

fn memory(value: &str) -> Result<MemoryPolicy, rusqlite::Error> {
    match value {
        "manual" => Ok(MemoryPolicy::Manual),
        "dynamic" => Ok(MemoryPolicy::Dynamic),
        _ => Err(invalid()),
    }
}

fn scene_slot_name(value: SceneAssetSlot) -> &'static str {
    match value {
        SceneAssetSlot::Background => "background",
        SceneAssetSlot::Inline => "inline",
    }
}

fn scene_slot(value: &str) -> Result<SceneAssetSlot, rusqlite::Error> {
    match value {
        "background" => Ok(SceneAssetSlot::Background),
        "inline" => Ok(SceneAssetSlot::Inline),
        _ => Err(invalid()),
    }
}

fn cursor_encode(updated_at: TimestampMillis, id: GroupId) -> Result<String, RepositoryError> {
    let bytes = serde_json::to_vec(&Envelope {
        format_version: CURSOR_VERSION,
        value: (updated_at.get(), id.to_string()),
    })
    .map_err(|_| RepositoryError::Storage)?;
    Ok(super::hex_encode(&bytes))
}

fn cursor_decode(cursor: Option<&str>) -> Result<Option<(i64, GroupId)>, RepositoryError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let bytes = super::hex_decode(cursor).map_err(|_| {
        RepositoryError::Invalid(ValidationError::InvalidValue {
            field: "group.cursor",
        })
    })?;
    let envelope: Envelope<(i64, String)> = serde_json::from_slice(&bytes).map_err(|_| {
        RepositoryError::Invalid(ValidationError::InvalidValue {
            field: "group.cursor",
        })
    })?;
    if envelope.format_version != CURSOR_VERSION {
        return Err(RepositoryError::Invalid(ValidationError::InvalidValue {
            field: "group.cursor",
        }));
    }
    Ok(Some((
        envelope.value.0,
        envelope.value.1.parse().map_err(|_| {
            RepositoryError::Invalid(ValidationError::InvalidValue {
                field: "group.cursor",
            })
        })?,
    )))
}

fn group_row(
    connection: &Connection,
    group_id: GroupId,
) -> Result<Option<GroupProfile>, rusqlite::Error> {
    connection
        .query_row(
            "SELECT id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at FROM groups WHERE id=?1",
            [group_id.to_string()],
            |row| {
                let status_text: String = row.get(1)?;
                let name: String = row.get(2)?;
                let normalized: String = row.get(3)?;
                if canonical_name(&name) != normalized {
                    return Err(invalid());
                }
                if row.get::<_, String>(14)? != "image" {
                    return Err(invalid());
                }
                let persona_id = row.get::<_, Option<String>>(6)?.map(id).transpose()?;
                let presentation: ChatAppearanceV1 = decode(&row.get::<_, String>(12)?, PRESENTATION_VERSION)?;
                let disable_character_lorebooks: i64 = row.get(9)?;
                if !matches!(disable_character_lorebooks, 0 | 1) {
                    return Err(invalid());
                }
                Ok(GroupProfile {
                    id: id(row.get(0)?)?,
                    status: status(&status_text)?,
                    name,
                    chat_mode: chat_mode(&row.get::<_, String>(4)?)?,
                    persona: persona_selection(&row.get::<_, String>(5)?, persona_id)?,
                    speaker_selection: speaker(&row.get::<_, String>(7)?)?,
                    memory_policy: memory(&row.get::<_, String>(8)?)?,
                    disable_character_lorebooks: disable_character_lorebooks != 0,
                    group_conversation_prompt_id: row.get::<_, Option<String>>(10)?.map(id).transpose()?,
                    group_roleplay_prompt_id: row.get::<_, Option<String>>(11)?.map(id).transpose()?,
                    presentation,
                    members: Vec::new(),
                    starting_scene_id: row.get::<_, Option<String>>(15)?.map(id).transpose()?,
                    background_asset_id: row.get::<_, Option<String>>(13)?.map(id).transpose()?,
                    revision: revision(row.get(16)?)?,
                    created_at: TimestampMillis::new(row.get(17)?),
                    updated_at: TimestampMillis::new(row.get(18)?),
                })
            },
        )
        .optional()
}

fn load_scene(
    tx: &Connection,
    group_id: GroupId,
    scene_id: SceneId,
) -> Result<GroupStartingScene, rusqlite::Error> {
    let scene = tx.query_row(
        "SELECT id,status,ordinal,content_json,direction,selected_variant_id,revision,created_at,updated_at FROM group_starting_scenes WHERE group_id=?1 AND id=?2",
        params![group_id.to_string(), scene_id.to_string()],
        |row| {
            let scene_uuid: SceneId = id(row.get(0)?)?;
            let mut assets = Vec::new();
            let mut stmt = tx.prepare("SELECT id,asset_id,slot,ordinal,blob_kind FROM group_scene_assets WHERE group_id=?1 AND scene_id=?2 ORDER BY slot,ordinal,id")?;
            for asset in stmt.query_map(params![group_id.to_string(), scene_id.to_string()], |asset| {
                if asset.get::<_, String>(4)? != "image" { return Err(invalid()); }
                Ok(SceneAssetLink {
                    id: id(asset.get(0)?)?,
                    asset_id: id(asset.get(1)?)?,
                    slot: scene_slot(&asset.get::<_, String>(2)?)?,
                    ordinal: u32::try_from(asset.get::<_, i64>(3)?).map_err(|_| invalid())?,
                })
            })? { assets.push(asset?); }
            Ok(Scene {
                id: scene_uuid,
                owner: SceneOwner::Group(group_id),
                status: status(&row.get::<_, String>(1)?)?,
                ordinal: u32::try_from(row.get::<_, i64>(2)?).map_err(|_| invalid())?,
                content: decode(&row.get::<_, String>(3)?, DOCUMENT_VERSION)?,
                direction: row.get(4)?,
                selected_variant_id: row.get::<_, Option<String>>(5)?.map(id).transpose()?,
                assets,
                revision: revision(row.get(6)?)?,
                created_at: TimestampMillis::new(row.get(7)?),
                updated_at: TimestampMillis::new(row.get(8)?),
            })
        },
    ).optional()?.ok_or_else(invalid)?;
    let mut variants = Vec::new();
    let mut stmt = tx.prepare("SELECT id,ordinal,content_json,direction,revision,created_at,updated_at FROM group_scene_variants WHERE group_id=?1 AND scene_id=?2 ORDER BY ordinal,id")?;
    for row in stmt.query_map(params![group_id.to_string(), scene_id.to_string()], |row| {
        Ok(SceneVariant {
            id: id(row.get(0)?)?,
            scene_id,
            ordinal: u32::try_from(row.get::<_, i64>(1)?).map_err(|_| invalid())?,
            content: decode(&row.get::<_, String>(2)?, DOCUMENT_VERSION)?,
            direction: row.get(3)?,
            revision: revision(row.get(4)?)?,
            created_at: TimestampMillis::new(row.get(5)?),
            updated_at: TimestampMillis::new(row.get(6)?),
        })
    })? {
        variants.push(row?);
    }
    let starting = GroupStartingScene { scene, variants };
    starting.validate(group_id).map_err(|_| invalid())?;
    Ok(starting)
}

fn load_details(
    tx: &Connection,
    group_id: GroupId,
) -> Result<Option<GroupDetails>, rusqlite::Error> {
    let Some(mut group) = group_row(tx, group_id)? else {
        return Ok(None);
    };
    let mut members = Vec::new();
    let mut member_stmt = tx.prepare("SELECT character_id,ordinal,muted,model_profile_override_id FROM group_members WHERE group_id=?1 ORDER BY ordinal,character_id")?;
    for row in member_stmt.query_map([group_id.to_string()], |row| {
        Ok(GroupMember {
            character_id: id(row.get(0)?)?,
            ordinal: u32::try_from(row.get::<_, i64>(1)?).map_err(|_| invalid())?,
            muted: match row.get::<_, i64>(2)? {
                0 => false,
                1 => true,
                _ => return Err(invalid()),
            },
            model_profile_override: row.get::<_, Option<String>>(3)?.map(id).transpose()?,
        })
    })? {
        members.push(row?);
    }
    group.members = members;
    for member in &group.members {
        let character_exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM characters WHERE id=?1",
                [member.character_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if character_exists.is_none() {
            return Err(invalid());
        }
        if let Some(model_id) = member.model_profile_override {
            let kind: Option<String> = tx
                .query_row(
                    "SELECT kind FROM model_profiles WHERE id=?1",
                    [model_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if kind.as_deref() != Some("chat") {
                return Err(invalid());
            }
        }
    }
    if let Selection::Explicit(persona_id) = group.persona {
        let persona_exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM personas WHERE id=?1",
                [persona_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if persona_exists.is_none() {
            return Err(invalid());
        }
    }
    let stored_refs: BTreeSet<AssetId> = tx
        .prepare("SELECT asset_id,blob_kind FROM group_presentation_asset_refs WHERE group_id=?1 ORDER BY asset_id")?
        .query_map([group_id.to_string()], |row| {
            if row.get::<_, String>(1)? != "image" { return Err(invalid()); }
            id(row.get(0)? )
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?.into_iter().collect();
    if stored_refs != group.presentation.referenced_asset_ids() {
        return Err(invalid());
    }
    for asset_id in &stored_refs {
        verify_image_asset(tx, *asset_id)?;
    }
    if let Some(asset_id) = group.background_asset_id {
        verify_image_asset(tx, asset_id)?;
    }
    let starting_scene = match group.starting_scene_id {
        Some(scene_id) => {
            let scene = load_scene(tx, group_id, scene_id)?;
            for link in &scene.scene.assets {
                verify_image_asset(tx, link.asset_id)?;
            }
            Some(scene)
        }
        None => {
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM group_starting_scenes WHERE group_id=?1",
                [group_id.to_string()],
                |row| row.get(0),
            )?;
            if count != 0 {
                return Err(invalid());
            }
            None
        }
    };
    let details = GroupDetails {
        group,
        starting_scene,
    };
    details.validate().map_err(|_| invalid())?;
    Ok(Some(details))
}

fn ensure_group(
    tx: &Transaction<'_>,
    id: GroupId,
    expected: Revision,
    allow_archived: bool,
) -> Result<GroupDetails, RepositoryError> {
    let current = load_details(tx, id)
        .map_err(db_error)?
        .ok_or(RepositoryError::NotFound)?;
    if !allow_archived && current.group.status == LifecycleStatus::Archived {
        return Err(RepositoryError::Archived);
    }
    if current.group.revision != expected {
        return Err(RepositoryError::StaleRevision {
            expected,
            actual: current.group.revision,
        });
    }
    Ok(current)
}

fn next_revision(value: Revision) -> Result<Revision, RepositoryError> {
    value.next().map_err(|_| RepositoryError::Storage)
}

fn bump_group(
    tx: &Transaction<'_>,
    id: GroupId,
    expected: Revision,
    now: TimestampMillis,
) -> Result<(), RepositoryError> {
    let next = next_revision(expected)?;
    let changed = tx
        .execute(
            "UPDATE groups SET revision=?2,updated_at=?3 WHERE id=?1 AND revision=?4",
            params![
                id.to_string(),
                sql_revision(next)?,
                now.get(),
                sql_revision(expected)?
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

fn ensure_active_character(tx: &Transaction<'_>, id: CharacterId) -> Result<(), RepositoryError> {
    let status: Option<String> = tx
        .query_row(
            "SELECT status FROM characters WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    match status.as_deref() {
        Some("active") => Ok(()),
        Some("archived") => Err(RepositoryError::Archived),
        Some(_) => Err(RepositoryError::Storage),
        None => Err(RepositoryError::NotFound),
    }
}

fn ensure_active_persona(tx: &Transaction<'_>, id: PersonaId) -> Result<(), RepositoryError> {
    let status: Option<String> = tx
        .query_row(
            "SELECT status FROM personas WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    match status.as_deref() {
        Some("active") => Ok(()),
        Some("archived") => Err(RepositoryError::Archived),
        Some(_) => Err(RepositoryError::Storage),
        None => Err(RepositoryError::NotFound),
    }
}

fn ensure_chat_model(tx: &Transaction<'_>, id: ModelProfileId) -> Result<(), RepositoryError> {
    let kind: Option<String> = tx
        .query_row(
            "SELECT kind FROM model_profiles WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    match kind.as_deref() {
        Some("chat") => Ok(()),
        Some(_) => Err(RepositoryError::Invalid(
            ValidationError::InvalidReference {
                field: "group.member.model_profile_override",
            },
        )),
        None => Err(RepositoryError::NotFound),
    }
}

fn ensure_image_asset(tx: &Transaction<'_>, id: AssetId) -> Result<(), RepositoryError> {
    let kind: Option<String> = tx
        .query_row(
            "SELECT blob_kind FROM media_assets WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    match kind.as_deref() {
        Some("image") => Ok(()),
        Some(_) => Err(RepositoryError::Invalid(
            ValidationError::InvalidReference {
                field: "group.image_asset",
            },
        )),
        None => Err(RepositoryError::NotFound),
    }
}

fn verify_image_asset(connection: &Connection, id: AssetId) -> Result<(), rusqlite::Error> {
    let kind: Option<String> = connection
        .query_row(
            "SELECT blob_kind FROM media_assets WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if kind.as_deref() != Some("image") {
        return Err(invalid());
    }
    Ok(())
}

fn validate_member_assignments(
    tx: &Transaction<'_>,
    members: &[GroupMember],
) -> Result<(), RepositoryError> {
    for member in members {
        ensure_active_character(tx, member.character_id)?;
        if let Some(model) = member.model_profile_override {
            ensure_chat_model(tx, model)?;
        }
    }
    Ok(())
}

fn insert_scene(
    tx: &Transaction<'_>,
    group_id: GroupId,
    starting: &GroupStartingScene,
) -> Result<(), RepositoryError> {
    starting.validate(group_id)?;
    tx.execute("INSERT INTO group_starting_scenes (group_id,id,status,ordinal,content_json,direction,selected_variant_id,revision,created_at,updated_at) VALUES (?1,?2,?3,0,?4,?5,?6,?7,?8,?9)", params![group_id.to_string(), starting.scene.id.to_string(), status_name(starting.scene.status), encode(&starting.scene.content, DOCUMENT_VERSION)?, starting.scene.direction, starting.scene.selected_variant_id.map(|id| id.to_string()), sql_revision(starting.scene.revision)?, starting.scene.created_at.get(), starting.scene.updated_at.get()]).map_err(db_error)?;
    for variant in &starting.variants {
        tx.execute("INSERT INTO group_scene_variants (group_id,id,scene_id,ordinal,content_json,direction,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![group_id.to_string(), variant.id.to_string(), variant.scene_id.to_string(), variant.ordinal, encode(&variant.content, DOCUMENT_VERSION)?, variant.direction, sql_revision(variant.revision)?, variant.created_at.get(), variant.updated_at.get()]).map_err(db_error)?;
    }
    for link in &starting.scene.assets {
        ensure_image_asset(tx, link.asset_id)?;
        tx.execute("INSERT INTO group_scene_assets (group_id,scene_id,id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,?3,?4,'image',?5,?6)", params![group_id.to_string(), starting.scene.id.to_string(), link.id.to_string(), link.asset_id.to_string(), scene_slot_name(link.slot), link.ordinal]).map_err(db_error)?;
    }
    Ok(())
}

fn scene_authored_changed(old: &Scene, new: &Scene) -> bool {
    old.status != new.status
        || old.ordinal != new.ordinal
        || old.content != new.content
        || old.direction != new.direction
        || old.selected_variant_id != new.selected_variant_id
        || old.assets != new.assets
}

fn variant_authored_changed(old: &SceneVariant, new: &SceneVariant) -> bool {
    old.ordinal != new.ordinal || old.content != new.content || old.direction != new.direction
}

fn normalize_starting_scene(
    old: Option<&GroupStartingScene>,
    mut draft: GroupStartingScene,
    now: TimestampMillis,
) -> Result<GroupStartingScene, RepositoryError> {
    let old_scene = old.map(|value| &value.scene);
    let same_scene = old_scene.is_some_and(|value| value.id == draft.scene.id);
    let scene_changed =
        !same_scene || old_scene.is_some_and(|value| scene_authored_changed(value, &draft.scene));
    if same_scene {
        let old_scene = old_scene.expect("same_scene implies an old scene");
        draft.scene.created_at = old_scene.created_at;
        draft.scene.revision = if scene_changed {
            old_scene
                .revision
                .next()
                .map_err(ValidationError::from)
                .map_err(RepositoryError::from)?
        } else {
            old_scene.revision
        };
        draft.scene.updated_at = if scene_changed {
            now
        } else {
            old_scene.updated_at
        };
    } else {
        draft.scene.created_at = now;
        draft.scene.revision = Revision::INITIAL;
        draft.scene.updated_at = now;
    }

    for variant in &mut draft.variants {
        let old_variant = same_scene
            .then(|| old.and_then(|value| value.variants.iter().find(|item| item.id == variant.id)))
            .flatten();
        if let Some(old_variant) = old_variant {
            let changed = variant_authored_changed(old_variant, variant);
            variant.created_at = old_variant.created_at;
            variant.revision = if changed {
                old_variant
                    .revision
                    .next()
                    .map_err(ValidationError::from)
                    .map_err(RepositoryError::from)?
            } else {
                old_variant.revision
            };
            variant.updated_at = if changed { now } else { old_variant.updated_at };
        } else {
            variant.created_at = now;
            variant.revision = Revision::INITIAL;
            variant.updated_at = now;
        }
    }
    Ok(draft)
}

fn write_members(
    tx: &Transaction<'_>,
    group_id: GroupId,
    members: &[GroupMember],
) -> Result<(), RepositoryError> {
    tx.execute(
        "DELETE FROM group_members WHERE group_id=?1",
        [group_id.to_string()],
    )
    .map_err(db_error)?;
    for member in members {
        tx.execute("INSERT INTO group_members (group_id,character_id,ordinal,muted,model_profile_override_id) VALUES (?1,?2,?3,?4,?5)", params![group_id.to_string(), member.character_id.to_string(), member.ordinal, member.muted, member.model_profile_override.map(|id| id.to_string())]).map_err(db_error)?;
    }
    Ok(())
}

fn write_presentation(
    tx: &Transaction<'_>,
    group_id: GroupId,
    presentation: &ChatAppearanceV1,
) -> Result<(), RepositoryError> {
    presentation.validate()?;
    tx.execute(
        "DELETE FROM group_presentation_asset_refs WHERE group_id=?1",
        [group_id.to_string()],
    )
    .map_err(db_error)?;
    for asset_id in presentation.referenced_asset_ids() {
        ensure_image_asset(tx, asset_id)?;
        tx.execute("INSERT INTO group_presentation_asset_refs (group_id,asset_id,blob_kind) VALUES (?1,?2,'image')", params![group_id.to_string(), asset_id.to_string()]).map_err(db_error)?;
    }
    tx.execute(
        "UPDATE groups SET presentation_json=?2 WHERE id=?1",
        params![
            group_id.to_string(),
            encode(presentation, PRESENTATION_VERSION)?
        ],
    )
    .map_err(db_error)?;
    Ok(())
}

impl GroupRepository for Database {
    fn create(&self, plan: CreateGroupPlan) -> Result<GroupDetails, RepositoryError> {
        plan.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if load_details(&tx, plan.group.id)
            .map_err(db_error)?
            .is_some()
        {
            return Err(RepositoryError::AlreadyExists);
        }
        validate_member_assignments(&tx, &plan.group.members)?;
        if let Selection::Explicit(persona) = plan.group.persona {
            ensure_active_persona(&tx, persona)?;
        }
        if let Some(asset) = plan.group.background_asset_id {
            ensure_image_asset(&tx, asset)?;
        }
        tx.execute("INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'image',?15,?16,?17,?18)", params![plan.group.id.to_string(), status_name(plan.group.status), plan.group.name, canonical_name(&plan.group.name), chat_mode_name(plan.group.chat_mode), selection_kind(&plan.group.persona), match plan.group.persona { Selection::Explicit(id) => Some(id.to_string()), _ => None }, speaker_name(plan.group.speaker_selection), memory_name(plan.group.memory_policy), plan.group.disable_character_lorebooks, plan.group.group_conversation_prompt_id.map(|id| id.to_string()), plan.group.group_roleplay_prompt_id.map(|id| id.to_string()), encode(&plan.group.presentation, PRESENTATION_VERSION)?, plan.group.background_asset_id.map(|id| id.to_string()), plan.group.starting_scene_id.map(|id| id.to_string()), sql_revision(plan.group.revision)?, plan.group.created_at.get(), plan.group.updated_at.get()]).map_err(db_error)?;
        write_members(&tx, plan.group.id, &plan.group.members)?;
        write_presentation(&tx, plan.group.id, &plan.group.presentation)?;
        if let Some(starting) = &plan.starting_scene {
            insert_scene(&tx, plan.group.id, starting)?;
        }
        let details = load_details(&tx, plan.group.id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?;
        details.validate()?;
        tx.commit().map_err(db_error)?;
        Ok(details)
    }

    fn get(&self, id: GroupId) -> Result<Option<GroupDetails>, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let result = load_details(&tx, id).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn list(
        &self,
        request: PageRequest,
        include_archived: bool,
    ) -> Result<Page<GroupProfile>, RepositoryError> {
        let cursor = cursor_decode(request.cursor.as_deref())?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let limit = i64::from(request.limit.get());
        let mut sql = String::from("SELECT id,updated_at FROM groups WHERE ");
        if !include_archived {
            sql.push_str("status='active' AND ");
        }
        if cursor.is_some() {
            sql.push_str("(updated_at < ?1 OR (updated_at = ?1 AND id > ?2)) AND ");
        }
        if cursor.is_some() {
            sql.push_str("1=1 ORDER BY updated_at DESC,id ASC LIMIT ?3");
        } else {
            sql.push_str("1=1 ORDER BY updated_at DESC,id ASC LIMIT ?1");
        }
        let mut statement = tx.prepare(&sql).map_err(db_error)?;
        let mut rows = if let Some((updated, id)) = cursor {
            statement
                .query(params![updated, id.to_string(), limit + 1])
                .map_err(db_error)?
        } else {
            statement.query(params![limit + 1]).map_err(db_error)?
        };
        let mut keys = Vec::new();
        while let Some(row) = rows.next().map_err(db_error)? {
            let key_id: GroupId =
                id(row.get::<_, String>(0).map_err(db_error)?).map_err(db_error)?;
            let updated_at = TimestampMillis::new(row.get(1).map_err(db_error)?);
            keys.push((key_id, updated_at));
        }
        drop(rows);
        drop(statement);
        let has_next = keys.len() > usize::try_from(limit).map_err(|_| RepositoryError::Storage)?;
        keys.truncate(usize::try_from(limit).map_err(|_| RepositoryError::Storage)?);
        let mut items = Vec::with_capacity(keys.len());
        for (group_id, _) in &keys {
            let details = load_details(&tx, *group_id)
                .map_err(db_error)?
                .ok_or(RepositoryError::Storage)?;
            items.push(details.group);
        }
        let next_cursor = if has_next {
            keys.last()
                .map(|(id, updated)| cursor_encode(*updated, *id))
                .transpose()?
        } else {
            None
        };
        tx.commit().map_err(db_error)?;
        Ok(Page { items, next_cursor })
    }

    fn rename(
        &self,
        id: GroupId,
        expected_revision: Revision,
        name: String,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        let mut proposed = current.group.clone();
        proposed.name = name.clone();
        proposed.updated_at = now;
        proposed.revision = next_revision(expected_revision)?;
        proposed.validate()?;
        tx.execute(
            "UPDATE groups SET name=?2,normalized_name=?3 WHERE id=?1",
            params![
                id.to_string(),
                proposed.name,
                canonical_name(&proposed.name)
            ],
        )
        .map_err(db_error)?;
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn set_persona(
        &self,
        id: GroupId,
        expected_revision: Revision,
        persona: Selection<PersonaId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        if let Selection::Explicit(persona_id) = persona {
            ensure_active_persona(&tx, persona_id)?;
        }
        let mut proposed = current.group.clone();
        proposed.persona = persona.clone();
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        proposed.validate()?;
        tx.execute(
            "UPDATE groups SET persona_selection_kind=?2,persona_id=?3 WHERE id=?1",
            params![
                id.to_string(),
                selection_kind(&persona),
                match persona {
                    Selection::Explicit(id) => Some(id.to_string()),
                    _ => None,
                }
            ],
        )
        .map_err(db_error)?;
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn set_chat_mode(
        &self,
        id: GroupId,
        expected_revision: Revision,
        mode: ChatMode,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.scalar_mode(
            id,
            expected_revision,
            now,
            "chat_mode",
            chat_mode_name(mode),
            |g| g.chat_mode = mode,
        )
    }
    fn set_speaker_selection(
        &self,
        id: GroupId,
        expected_revision: Revision,
        selection: SpeakerSelection,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.scalar_mode(
            id,
            expected_revision,
            now,
            "speaker_selection",
            speaker_name(selection),
            |g| g.speaker_selection = selection,
        )
    }
    fn set_memory_policy(
        &self,
        id: GroupId,
        expected_revision: Revision,
        policy: MemoryPolicy,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.scalar_mode(
            id,
            expected_revision,
            now,
            "memory_policy",
            memory_name(policy),
            |g| g.memory_policy = policy,
        )
    }
    fn set_disable_character_lorebooks(
        &self,
        id: GroupId,
        expected_revision: Revision,
        disabled: bool,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.scalar_mode(
            id,
            expected_revision,
            now,
            "disable_character_lorebooks",
            if disabled { 1_i64 } else { 0_i64 },
            |g| g.disable_character_lorebooks = disabled,
        )
    }

    fn set_prompt_defaults(
        &self,
        id: GroupId,
        expected_revision: Revision,
        conversation_prompt_id: Option<PromptDocumentId>,
        roleplay_prompt_id: Option<PromptDocumentId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        let mut proposed = current.group.clone();
        proposed.group_conversation_prompt_id = conversation_prompt_id;
        proposed.group_roleplay_prompt_id = roleplay_prompt_id;
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        proposed.validate()?;
        tx.execute("UPDATE groups SET group_conversation_prompt_id=?2,group_roleplay_prompt_id=?3 WHERE id=?1",params![id.to_string(),conversation_prompt_id.map(|id|id.to_string()),roleplay_prompt_id.map(|id|id.to_string())]).map_err(db_error)?;
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn set_presentation(
        &self,
        id: GroupId,
        expected_revision: Revision,
        presentation: ChatAppearanceV1,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        let mut proposed = current.group.clone();
        proposed.presentation = presentation.clone();
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        proposed.validate()?;
        write_presentation(&tx, id, &presentation)?;
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn set_background(
        &self,
        id: GroupId,
        expected_revision: Revision,
        asset_id: Option<AssetId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        if let Some(asset) = asset_id {
            ensure_image_asset(&tx, asset)?;
        }
        let mut proposed = current.group.clone();
        proposed.background_asset_id = asset_id;
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        proposed.validate()?;
        tx.execute(
            "UPDATE groups SET background_asset_id=?2 WHERE id=?1",
            params![id.to_string(), asset_id.map(|id| id.to_string())],
        )
        .map_err(db_error)?;
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn set_starting_scene(
        &self,
        id: GroupId,
        expected_revision: Revision,
        mut starting_scene: Option<GroupStartingScene>,
        now: TimestampMillis,
    ) -> Result<GroupDetails, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        if let Some(scene) = starting_scene.take() {
            starting_scene = Some(normalize_starting_scene(
                current.starting_scene.as_ref(),
                scene,
                now,
            )?);
        }
        if let Some(scene) = &starting_scene {
            scene.validate(id)?;
        }
        let new_id = starting_scene.as_ref().map(|scene| scene.scene.id);
        let mut proposed = current.clone();
        proposed.group.starting_scene_id = new_id;
        proposed.group.revision = next_revision(expected_revision)?;
        proposed.group.updated_at = now;
        proposed.starting_scene = starting_scene.clone();
        proposed.validate()?;
        tx.execute(
            "UPDATE groups SET starting_scene_id=?2 WHERE id=?1",
            params![id.to_string(), new_id.map(|id| id.to_string())],
        )
        .map_err(db_error)?;
        tx.execute(
            "DELETE FROM group_starting_scenes WHERE group_id=?1",
            [id.to_string()],
        )
        .map_err(db_error)?;
        if let Some(scene) = &starting_scene {
            insert_scene(&tx, id, scene)?;
        }
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn replace_members(
        &self,
        id: GroupId,
        expected_revision: Revision,
        members: Vec<GroupMember>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected_revision, false)?;
        let mut proposed = current.group.clone();
        proposed.members = members.clone();
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        proposed.validate()?;
        validate_member_assignments(&tx, &members)?;
        write_members(&tx, id, &members)?;
        bump_group(&tx, id, expected_revision, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn reorder_member(
        &self,
        id: GroupId,
        expected_revision: Revision,
        character_id: CharacterId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.mutate_members(id, expected_revision, now, |members| {
            let index = members
                .iter()
                .position(|m| m.character_id == character_id)
                .ok_or(RepositoryError::NotFound)?;
            let member = members.remove(index);
            let target = usize::try_from(target_ordinal).map_err(|_| RepositoryError::Storage)?;
            if target > members.len() {
                return Err(RepositoryError::Invalid(ValidationError::InvalidValue {
                    field: "group.member.ordinal",
                }));
            }
            members.insert(target, member);
            for (ordinal, member) in members.iter_mut().enumerate() {
                member.ordinal = u32::try_from(ordinal).map_err(|_| RepositoryError::Storage)?;
            }
            Ok(())
        })
    }

    fn set_member_muted(
        &self,
        id: GroupId,
        expected_revision: Revision,
        character_id: CharacterId,
        muted: bool,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.mutate_members(id, expected_revision, now, |members| {
            let member = members
                .iter_mut()
                .find(|m| m.character_id == character_id)
                .ok_or(RepositoryError::NotFound)?;
            member.muted = muted;
            Ok(())
        })
    }

    fn set_member_model_override(
        &self,
        id: GroupId,
        expected_revision: Revision,
        character_id: CharacterId,
        model_profile_id: Option<ModelProfileId>,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.mutate_members(id, expected_revision, now, |members| {
            let member = members
                .iter_mut()
                .find(|m| m.character_id == character_id)
                .ok_or(RepositoryError::NotFound)?;
            member.model_profile_override = model_profile_id;
            Ok(())
        })
    }

    fn archive(
        &self,
        id: GroupId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.group_set_status(id, expected_revision, LifecycleStatus::Archived, now, false)
    }
    fn restore(
        &self,
        id: GroupId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<GroupProfile, RepositoryError> {
        self.group_set_status(id, expected_revision, LifecycleStatus::Active, now, true)
    }
}

impl Database {
    fn scalar_mode<T: rusqlite::ToSql, F: FnOnce(&mut GroupProfile)>(
        &self,
        id: GroupId,
        expected: Revision,
        now: TimestampMillis,
        column: &str,
        value: T,
        mutate: F,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected, false)?;
        let mut proposed = current.group.clone();
        mutate(&mut proposed);
        proposed.revision = next_revision(expected)?;
        proposed.updated_at = now;
        proposed.validate()?;
        tx.execute(
            &format!("UPDATE groups SET {column}=?2 WHERE id=?1"),
            params![id.to_string(), value],
        )
        .map_err(db_error)?;
        bump_group(&tx, id, expected, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    fn mutate_members<F>(
        &self,
        id: GroupId,
        expected: Revision,
        now: TimestampMillis,
        mutate: F,
    ) -> Result<GroupProfile, RepositoryError>
    where
        F: FnOnce(&mut Vec<GroupMember>) -> Result<(), RepositoryError>,
    {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_group(&tx, id, expected, false)?;
        let mut members = current.group.members.clone();
        mutate(&mut members)?;
        let mut proposed = current.group.clone();
        proposed.members = members.clone();
        proposed.revision = next_revision(expected)?;
        proposed.updated_at = now;
        proposed.validate()?;
        for member in &members {
            if let Some(model) = member.model_profile_override {
                ensure_chat_model(&tx, model)?;
            }
        }
        write_members(&tx, id, &members)?;
        bump_group(&tx, id, expected, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    fn group_set_status(
        &self,
        id: GroupId,
        expected: Revision,
        status_value: LifecycleStatus,
        now: TimestampMillis,
        allow_archived: bool,
    ) -> Result<GroupProfile, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if current.group.revision != expected {
            return Err(RepositoryError::StaleRevision {
                expected,
                actual: current.group.revision,
            });
        }
        if status_value == LifecycleStatus::Archived
            && current.group.status == LifecycleStatus::Archived
        {
            return Err(RepositoryError::Archived);
        }
        if status_value == LifecycleStatus::Active
            && current.group.status == LifecycleStatus::Active
        {
            return Err(RepositoryError::AlreadyActive);
        }
        if !allow_archived && current.group.status == LifecycleStatus::Archived {
            return Err(RepositoryError::Archived);
        }
        let mut proposed = current.group.clone();
        proposed.status = status_value;
        proposed.revision = next_revision(expected)?;
        proposed.updated_at = now;
        proposed.validate()?;
        tx.execute(
            "UPDATE groups SET status=?2 WHERE id=?1",
            params![id.to_string(), status_name(status_value)],
        )
        .map_err(db_error)?;
        bump_group(&tx, id, expected, now)?;
        let result = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?
            .group;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}

impl lettuce_characters::GroupDependencyReader for Database {
    fn dependencies(&self, id: GroupId) -> Result<DependencyReport, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let details = load_details(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        let mut references = Vec::new();
        for member in &details.group.members {
            references.push(DependencyReference::GroupMemberCharacter {
                character_id: member.character_id,
            });
            if let Some(model_profile_id) = member.model_profile_override {
                references.push(DependencyReference::GroupMemberModel {
                    character_id: member.character_id,
                    model_profile_id,
                });
            }
        }
        if let Selection::Explicit(persona_id) = details.group.persona {
            references.push(DependencyReference::GroupPersona { persona_id });
        }
        if let Some(scene_id) = details.group.starting_scene_id {
            references.push(DependencyReference::GroupStartingScene { scene_id });
        }
        let mut prompts = BTreeSet::new();
        prompts.extend([
            details.group.group_conversation_prompt_id,
            details.group.group_roleplay_prompt_id,
        ]);
        references.extend(
            prompts
                .into_iter()
                .flatten()
                .map(|prompt_id| DependencyReference::Prompt { prompt_id }),
        );
        let mut assets = details.group.presentation.referenced_asset_ids();
        assets.extend(details.group.background_asset_id);
        if let Some(scene) = details.starting_scene {
            assets.extend(scene.scene.assets.iter().map(|link| link.asset_id));
        }
        references.extend(
            assets
                .into_iter()
                .map(|asset_id| DependencyReference::Asset { asset_id }),
        );
        references.sort_by_key(|reference| format!("{reference:?}"));
        let report = DependencyReport { references };
        tx.commit().map_err(db_error)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_characters::{
        CharacterDependencyReader, CharacterPresentationV1, DependencyReference,
        GroupDependencyReader, GroupRepository, PersonaDependencyReader, SceneDocumentV1,
        ScenePart, WidgetImageShape, WidgetImageSource, WidgetNode,
    };
    use lettuce_types::{MediaBlobId, SceneAssetLinkId, SceneVariantId};

    fn image_asset(database: &Database, marker: char) -> AssetId {
        let blob_id = MediaBlobId::new();
        let asset_id = AssetId::new();
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO media_blobs (id,content_hash,kind,mime_type,byte_size,width,height,duration_ms,validation_version,state,created_at,updated_at) VALUES (?1,?2,'image','image/png',1,1,1,NULL,1,'ready',1,1)",
                rusqlite::params![blob_id.to_string(), marker.to_string().repeat(64)],
            )
            .expect("image blob fixture");
        connection
            .execute(
                "INSERT INTO media_assets (id,blob_id,blob_kind,kind,origin,retention,expires_at,provenance_json,revision,created_at,updated_at) VALUES (?1,?2,'image','illustration','upload','library',NULL,'{}',1,1,1)",
                rusqlite::params![asset_id.to_string(), blob_id.to_string()],
            )
            .expect("image asset fixture");
        asset_id
    }

    fn persona(database: &Database, id: PersonaId, status: &str) {
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO personas (id,status,title,normalized_title,nickname,normalized_nickname,description,design_description,avatar_crop_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,?2,'Narrator','narrator',NULL,NULL,'A narrator',NULL,NULL,NULL,1,1,1)",
                rusqlite::params![id.to_string(), status],
            )
            .expect("persona fixture");
    }

    fn model(
        database: &Database,
        id: ModelProfileId,
        kind: &str,
        account: lettuce_types::ProviderAccountId,
    ) {
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT OR IGNORE INTO provider_accounts (id,provider_kind,protocol,label,endpoint,enabled,api_key_secret_ref,secret_headers_json,config_json,revision,created_at,updated_at) VALUES (?1,'test','open_ai_compatible','Test',NULL,1,NULL,'[]','{\"format_version\":1,\"value\":{\"kind\":\"standard\"}}',1,1,1)",
                [account.to_string()],
            )
            .expect("provider fixture");
        connection
            .execute(
                "INSERT INTO model_profiles (id,provider_account_id,external_model_id,display_name,kind,config_json,revision,created_at,updated_at) VALUES (?1,?2,'test/model','Test',?3,'{\"format_version\":1,\"value\":{\"input_modalities\":[\"text\"],\"output_modalities\":[\"text\"]}}',1,1,1)",
                rusqlite::params![id.to_string(), account.to_string(), kind],
            )
            .expect("model fixture");
    }

    fn character(database: &Database, id: CharacterId) {
        let profile_json = serde_json::json!({
            "format_version": 1,
            "value": {
                "name": "Member",
                "nickname": null,
                "description": null,
                "definition": null,
                "design_description": null
            }
        })
        .to_string();
        let provenance_json = serde_json::json!({
            "format_version": 1,
            "value": {
                "creator": null,
                "creator_notes": null,
                "localized_creator_notes": {},
                "sources": [],
                "tags": []
            }
        })
        .to_string();
        let defaults_json = serde_json::json!({
            "format_version": 1,
            "value": {
                "interaction_mode": "roleplay",
                "memory_policy": "manual",
                "model_profile_id": null,
                "default_scene_id": null,
                "default_starter_id": null,
                "direct_prompt_id": null,
                "group_conversation_prompt_id": null,
                "group_roleplay_prompt_id": null,
                "voice": null,
                "voice_autoplay": false
            }
        })
        .to_string();
        let presentation_json = serde_json::json!({
            "format_version": 1,
            "value": CharacterPresentationV1::default()
        })
        .to_string();
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO characters (id,status,name,normalized_name,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,voice_autoplay,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Member', 'member',?2,?3,?4,'roleplay','manual',0,?5,1,1,1)",
                rusqlite::params![id.to_string(), profile_json, provenance_json, defaults_json, presentation_json],
            )
            .expect("character fixture");
    }

    #[test]
    fn group_round_trip_and_keyset_list() {
        let database = Database::open_in_memory().expect("database");
        let first = CharacterId::new();
        let second = CharacterId::new();
        character(&database, first);
        character(&database, second);
        let group_id = GroupId::new();
        let group = GroupProfile::new(
            group_id,
            "Café Cast".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: true,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(1),
        )
        .expect("group");
        let created = GroupRepository::create(
            &database,
            CreateGroupPlan {
                group: group.clone(),
                starting_scene: None,
            },
        )
        .expect("create");
        assert_eq!(created.group, group);
        assert_eq!(
            GroupRepository::get(&database, group_id).expect("get"),
            Some(created.clone())
        );
        let renamed = GroupRepository::rename(
            &database,
            group_id,
            created.group.revision,
            "Renamed Cast".into(),
            TimestampMillis::new(2),
        )
        .expect("rename");
        assert_eq!(renamed.name, "Renamed Cast");
        let scene_id = SceneId::new();
        let variant_id = SceneVariantId::new();
        let mut scene = Scene::new(
            scene_id,
            SceneOwner::Group(group_id),
            0,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "The cast arrives.".into(),
            }])
            .expect("scene document"),
            TimestampMillis::new(2),
        )
        .expect("scene");
        scene.selected_variant_id = Some(variant_id);
        let variant = SceneVariant {
            id: variant_id,
            scene_id,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::Text {
                text: "A variant.".into(),
            }])
            .expect("variant document"),
            direction: Some("Wait.".into()),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(2),
            updated_at: TimestampMillis::new(2),
        };
        let with_scene = GroupRepository::set_starting_scene(
            &database,
            group_id,
            renamed.revision,
            Some(GroupStartingScene {
                scene,
                variants: vec![variant],
            }),
            TimestampMillis::new(3),
        )
        .expect("set scene");
        assert_eq!(with_scene.group.starting_scene_id, Some(scene_id));
        let stored_scene = with_scene.starting_scene.as_ref().expect("scene");
        assert_eq!(stored_scene.scene.revision, Revision::INITIAL);
        assert_eq!(stored_scene.scene.created_at, TimestampMillis::new(3));
        assert_eq!(stored_scene.variants[0].revision, Revision::INITIAL);
        assert_eq!(stored_scene.variants[0].created_at, TimestampMillis::new(3));
        assert_eq!(stored_scene.scene.updated_at, TimestampMillis::new(3));
        let unchanged = GroupRepository::set_starting_scene(
            &database,
            group_id,
            with_scene.group.revision,
            Some(stored_scene.clone()),
            TimestampMillis::new(4),
        )
        .expect("unchanged scene replacement");
        let unchanged_scene = unchanged.starting_scene.as_ref().expect("scene");
        assert_eq!(unchanged_scene.scene.revision, Revision::INITIAL);
        assert_eq!(unchanged_scene.scene.updated_at, TimestampMillis::new(3));
        assert_eq!(unchanged_scene.variants[0].revision, Revision::INITIAL);
        assert_eq!(
            unchanged_scene.variants[0].updated_at,
            TimestampMillis::new(3)
        );
        let mut replacement_scene = stored_scene.scene.clone();
        replacement_scene.content = SceneDocumentV1::new(vec![ScenePart::Text {
            text: "The cast departs.".into(),
        }])
        .expect("replacement document");
        let mut replacement_variant = stored_scene.variants[0].clone();
        replacement_variant.direction = Some("Run.".into());
        let replacement = GroupRepository::set_starting_scene(
            &database,
            group_id,
            unchanged.group.revision,
            Some(GroupStartingScene {
                scene: replacement_scene,
                variants: vec![replacement_variant],
            }),
            TimestampMillis::new(5),
        )
        .expect("replace scene");
        let replaced_scene = replacement.starting_scene.as_ref().expect("scene");
        assert_eq!(replaced_scene.scene.revision.get(), 2);
        assert_eq!(replaced_scene.scene.created_at, TimestampMillis::new(3));
        assert_eq!(replaced_scene.scene.updated_at, TimestampMillis::new(5));
        assert_eq!(replaced_scene.variants[0].revision.get(), 2);
        assert_eq!(
            replaced_scene.variants[0].created_at,
            TimestampMillis::new(3)
        );
        assert_eq!(
            replaced_scene.variants[0].updated_at,
            TimestampMillis::new(5)
        );
        let fresh_scene_id = SceneId::new();
        let fresh_variant_id = SceneVariantId::new();
        let mut fresh_scene = replaced_scene.scene.clone();
        fresh_scene.id = fresh_scene_id;
        fresh_scene.selected_variant_id = Some(fresh_variant_id);
        let mut fresh_variant = replaced_scene.variants[0].clone();
        fresh_variant.id = fresh_variant_id;
        fresh_variant.scene_id = fresh_scene_id;
        let fresh = GroupRepository::set_starting_scene(
            &database,
            group_id,
            replacement.group.revision,
            Some(GroupStartingScene {
                scene: fresh_scene,
                variants: vec![fresh_variant],
            }),
            TimestampMillis::new(7),
        )
        .expect("replace with fresh graph");
        let fresh_graph = fresh.starting_scene.as_ref().expect("fresh graph");
        assert_eq!(fresh_graph.scene.id, fresh_scene_id);
        assert_eq!(fresh_graph.scene.revision, Revision::INITIAL);
        assert_eq!(fresh_graph.scene.created_at, TimestampMillis::new(7));
        assert_eq!(fresh_graph.scene.updated_at, TimestampMillis::new(7));
        assert_eq!(fresh_graph.variants[0].id, fresh_variant_id);
        assert_eq!(fresh_graph.variants[0].revision, Revision::INITIAL);
        assert_eq!(fresh_graph.variants[0].created_at, TimestampMillis::new(7));
        assert_eq!(fresh_graph.variants[0].updated_at, TimestampMillis::new(7));
        let removed = GroupRepository::set_starting_scene(
            &database,
            group_id,
            fresh.group.revision,
            None,
            TimestampMillis::new(8),
        )
        .expect("remove scene");
        assert!(removed.starting_scene.is_none());
        let archived = GroupRepository::archive(
            &database,
            group_id,
            removed.group.revision,
            TimestampMillis::new(6),
        )
        .expect("archive");
        assert_eq!(archived.status, LifecycleStatus::Archived);
        assert!(matches!(
            GroupRepository::archive(
                &database,
                group_id,
                removed.group.revision,
                TimestampMillis::new(7)
            ),
            Err(RepositoryError::StaleRevision { .. })
        ));
        assert!(matches!(
            GroupRepository::archive(
                &database,
                group_id,
                archived.revision,
                TimestampMillis::new(7)
            ),
            Err(RepositoryError::Archived)
        ));
        assert!(matches!(
            GroupRepository::restore(
                &database,
                group_id,
                removed.group.revision,
                TimestampMillis::new(8)
            ),
            Err(RepositoryError::StaleRevision { .. })
        ));
        let restored = GroupRepository::restore(
            &database,
            group_id,
            archived.revision,
            TimestampMillis::new(8),
        )
        .expect("restore");
        assert_eq!(restored.status, LifecycleStatus::Active);
        assert!(matches!(
            GroupRepository::restore(
                &database,
                group_id,
                restored.revision,
                TimestampMillis::new(9)
            ),
            Err(RepositoryError::AlreadyActive)
        ));
        let page = GroupRepository::list(&database, PageRequest::default(), false).expect("list");
        assert_eq!(page.items, vec![restored.clone()]);
        let dependencies =
            GroupDependencyReader::dependencies(&database, group_id).expect("dependencies");
        assert!(
            dependencies
                .references
                .contains(&DependencyReference::GroupMemberCharacter {
                    character_id: first
                })
        );
    }

    #[test]
    fn group_full_graph_round_trip_and_dependency_manifest_are_exact() {
        let database = Database::open_in_memory().expect("database");
        let first = CharacterId::new();
        let second = CharacterId::new();
        character(&database, first);
        character(&database, second);
        let persona_id = PersonaId::new();
        persona(&database, persona_id, "active");
        let account_id = lettuce_types::ProviderAccountId::new();
        let chat_model = ModelProfileId::new();
        model(&database, chat_model, "chat", account_id);
        let presentation_asset = image_asset(&database, 'a');
        let background_asset = image_asset(&database, 'b');
        let scene_background = image_asset(&database, 'c');
        let scene_inline = image_asset(&database, 'd');
        let group_id = GroupId::new();
        let scene_id = SceneId::new();
        let variant_id = SceneVariantId::new();
        let inline_id = SceneAssetLinkId::new();
        let scene = Scene {
            id: scene_id,
            owner: SceneOwner::Group(group_id),
            status: LifecycleStatus::Active,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![
                ScenePart::Text {
                    text: "Opening".into(),
                },
                ScenePart::InlineAsset { link_id: inline_id },
            ])
            .expect("scene document"),
            direction: Some("Set the scene".into()),
            selected_variant_id: Some(variant_id),
            assets: vec![
                SceneAssetLink {
                    id: SceneAssetLinkId::new(),
                    asset_id: scene_background,
                    slot: SceneAssetSlot::Background,
                    ordinal: 0,
                },
                SceneAssetLink {
                    id: inline_id,
                    asset_id: scene_inline,
                    slot: SceneAssetSlot::Inline,
                    ordinal: 0,
                },
            ],
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        };
        let variant = SceneVariant {
            id: variant_id,
            scene_id,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Variant opening".into(),
            }])
            .expect("variant document"),
            direction: Some("Choose this variant".into()),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        };
        let mut presentation = ChatAppearanceV1 {
            chat_widget_area_enabled: true,
            ..ChatAppearanceV1::default()
        };
        presentation.chat_widget_slots.left.push(WidgetNode::Box {
            id: "outer".into(),
            design: None,
            variant: None,
            title: Some("Nested".into()),
            description: None,
            children: vec![WidgetNode::Image {
                id: "asset".into(),
                design: None,
                title: None,
                description: None,
                source: WidgetImageSource::LogicalAsset {
                    asset_id: presentation_asset,
                },
                shape: Some(WidgetImageShape::Square),
            }],
        });
        let mut group = GroupProfile::new(
            group_id,
            "Full Cast".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: Some(chat_model),
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: true,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(10),
        )
        .expect("group");
        group.chat_mode = ChatMode::Roleplay;
        group.persona = Selection::Explicit(persona_id);
        group.speaker_selection = SpeakerSelection::DirectorAction;
        group.memory_policy = MemoryPolicy::Dynamic;
        group.disable_character_lorebooks = true;
        group.group_conversation_prompt_id = Some(PromptDocumentId::new());
        group.group_roleplay_prompt_id = Some(PromptDocumentId::new());
        group.presentation = presentation.clone();
        group.background_asset_id = Some(background_asset);
        group.starting_scene_id = Some(scene_id);
        group.validate().expect("full group");
        let created = GroupRepository::create(
            &database,
            CreateGroupPlan {
                group: group.clone(),
                starting_scene: Some(GroupStartingScene {
                    scene,
                    variants: vec![variant],
                }),
            },
        )
        .expect("create full group");
        assert_eq!(created.group, group);
        assert_eq!(created.group.presentation, presentation);
        assert_eq!(
            created
                .starting_scene
                .as_ref()
                .expect("scene")
                .scene
                .selected_variant_id,
            Some(variant_id)
        );
        assert_eq!(
            GroupRepository::get(&database, group_id).expect("get"),
            Some(created.clone())
        );

        let mut expected = vec![
            DependencyReference::Asset {
                asset_id: presentation_asset,
            },
            DependencyReference::Asset {
                asset_id: background_asset,
            },
            DependencyReference::Asset {
                asset_id: scene_background,
            },
            DependencyReference::Asset {
                asset_id: scene_inline,
            },
            DependencyReference::GroupMemberCharacter {
                character_id: first,
            },
            DependencyReference::GroupMemberCharacter {
                character_id: second,
            },
            DependencyReference::GroupMemberModel {
                character_id: first,
                model_profile_id: chat_model,
            },
            DependencyReference::GroupPersona { persona_id },
            DependencyReference::GroupStartingScene { scene_id },
            DependencyReference::Prompt {
                prompt_id: group.group_conversation_prompt_id.expect("prompt"),
            },
            DependencyReference::Prompt {
                prompt_id: group.group_roleplay_prompt_id.expect("prompt"),
            },
        ];
        expected.sort_by_key(|reference| format!("{reference:?}"));
        assert_eq!(
            GroupDependencyReader::dependencies(&database, group_id)
                .expect("dependencies")
                .references,
            expected
        );
        assert!(
            CharacterDependencyReader::dependencies(&database, first)
                .expect("character dependencies")
                .references
                .contains(&DependencyReference::CharacterInGroup { group_id })
        );
        assert!(
            PersonaDependencyReader::dependencies(&database, persona_id)
                .expect("persona dependencies")
                .references
                .contains(&DependencyReference::PersonaInGroup { group_id })
        );

        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "UPDATE characters SET status='archived' WHERE id=?1",
                [first.to_string()],
            )
            .expect("archive member fixture");
        connection
            .execute(
                "UPDATE personas SET status='archived' WHERE id=?1",
                [persona_id.to_string()],
            )
            .expect("archive persona fixture");
        drop(connection);
        assert_eq!(
            GroupRepository::get(&database, group_id)
                .expect("retained archived references")
                .expect("group")
                .group,
            group
        );
        assert!(matches!(
            GroupRepository::replace_members(
                &database,
                group_id,
                group.revision,
                group.members.clone(),
                TimestampMillis::new(20)
            ),
            Err(RepositoryError::Archived)
        ));
        assert!(matches!(
            GroupRepository::set_persona(
                &database,
                group_id,
                group.revision,
                Selection::Explicit(persona_id),
                TimestampMillis::new(20)
            ),
            Err(RepositoryError::Archived)
        ));
    }

    #[test]
    fn group_mutations_cover_all_fields_member_rules_and_cas() {
        let database = Database::open_in_memory().expect("database");
        let first = CharacterId::new();
        let second = CharacterId::new();
        let third = CharacterId::new();
        character(&database, first);
        character(&database, second);
        character(&database, third);
        let account_id = lettuce_types::ProviderAccountId::new();
        let chat_model = ModelProfileId::new();
        let image_model = ModelProfileId::new();
        model(&database, chat_model, "chat", account_id);
        model(&database, image_model, "image", account_id);
        let group = GroupProfile::new(
            GroupId::new(),
            "Mutable".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(1),
        )
        .expect("group");
        let id = group.id;
        let mut stored = GroupRepository::create(
            &database,
            CreateGroupPlan {
                group,
                starting_scene: None,
            },
        )
        .expect("create")
        .group;
        let stale = stored.revision;
        stored = GroupRepository::rename(
            &database,
            id,
            stored.revision,
            "Renamed".into(),
            TimestampMillis::new(2),
        )
        .expect("rename");
        assert!(matches!(
            GroupRepository::set_chat_mode(
                &database,
                id,
                stale,
                ChatMode::Roleplay,
                TimestampMillis::new(3)
            ),
            Err(RepositoryError::StaleRevision { .. })
        ));
        stored = GroupRepository::set_chat_mode(
            &database,
            id,
            stored.revision,
            ChatMode::Roleplay,
            TimestampMillis::new(3),
        )
        .expect("mode");
        stored = GroupRepository::set_persona(
            &database,
            id,
            stored.revision,
            Selection::Disabled,
            TimestampMillis::new(4),
        )
        .expect("disabled persona");
        assert_eq!(stored.persona, Selection::Disabled);
        stored = GroupRepository::set_persona(
            &database,
            id,
            stored.revision,
            Selection::Inherit,
            TimestampMillis::new(5),
        )
        .expect("inherit persona");
        stored = GroupRepository::set_speaker_selection(
            &database,
            id,
            stored.revision,
            SpeakerSelection::Heuristic,
            TimestampMillis::new(6),
        )
        .expect("speaker");
        stored = GroupRepository::set_memory_policy(
            &database,
            id,
            stored.revision,
            MemoryPolicy::Dynamic,
            TimestampMillis::new(7),
        )
        .expect("memory");
        stored = GroupRepository::set_disable_character_lorebooks(
            &database,
            id,
            stored.revision,
            true,
            TimestampMillis::new(8),
        )
        .expect("lorebooks");
        let conversation_prompt = PromptDocumentId::new();
        let roleplay_prompt = PromptDocumentId::new();
        stored = GroupRepository::set_prompt_defaults(
            &database,
            id,
            stored.revision,
            Some(conversation_prompt),
            Some(roleplay_prompt),
            TimestampMillis::new(9),
        )
        .expect("prompts");
        let asset = image_asset(&database, 'e');
        let mut presentation = ChatAppearanceV1::default();
        presentation
            .chat_widget_slots
            .right
            .push(WidgetNode::Image {
                id: "right".into(),
                design: None,
                title: None,
                description: None,
                source: WidgetImageSource::LogicalAsset { asset_id: asset },
                shape: None,
            });
        stored = GroupRepository::set_presentation(
            &database,
            id,
            stored.revision,
            presentation.clone(),
            TimestampMillis::new(10),
        )
        .expect("presentation");
        let replacement_asset = image_asset(&database, 'g');
        let mut replacement_presentation = ChatAppearanceV1::default();
        replacement_presentation
            .chat_widget_slots
            .left
            .push(WidgetNode::Image {
                id: "replacement".into(),
                design: None,
                title: None,
                description: None,
                source: WidgetImageSource::LogicalAsset {
                    asset_id: replacement_asset,
                },
                shape: None,
            });
        stored = GroupRepository::set_presentation(
            &database,
            id,
            stored.revision,
            replacement_presentation.clone(),
            TimestampMillis::new(10),
        )
        .expect("replace presentation");
        let connection = database.connection().expect("database lock");
        let refs: Vec<String> = connection
            .prepare("SELECT asset_id FROM group_presentation_asset_refs WHERE group_id=?1 ORDER BY asset_id")
            .expect("presentation refs")
            .query_map([id.to_string()], |row| row.get(0))
            .expect("presentation ref rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("presentation ref values");
        drop(connection);
        assert_eq!(refs, vec![replacement_asset.to_string()]);
        stored = GroupRepository::set_background(
            &database,
            id,
            stored.revision,
            Some(replacement_asset),
            TimestampMillis::new(11),
        )
        .expect("background");
        stored = GroupRepository::replace_members(
            &database,
            id,
            stored.revision,
            vec![
                GroupMember {
                    character_id: second,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: Some(chat_model),
                },
                GroupMember {
                    character_id: third,
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(12),
        )
        .expect("replace members");
        stored = GroupRepository::reorder_member(
            &database,
            id,
            stored.revision,
            third,
            0,
            TimestampMillis::new(13),
        )
        .expect("reorder");
        stored = GroupRepository::set_member_muted(
            &database,
            id,
            stored.revision,
            third,
            true,
            TimestampMillis::new(14),
        )
        .expect("mute");
        stored = GroupRepository::set_member_model_override(
            &database,
            id,
            stored.revision,
            third,
            Some(chat_model),
            TimestampMillis::new(15),
        )
        .expect("model override");
        assert!(matches!(
            GroupRepository::set_member_model_override(
                &database,
                id,
                stored.revision,
                third,
                Some(image_model),
                TimestampMillis::new(16)
            ),
            Err(RepositoryError::Invalid(_))
        ));
        assert!(matches!(
            GroupRepository::set_member_model_override(
                &database,
                id,
                stored.revision,
                third,
                Some(ModelProfileId::new()),
                TimestampMillis::new(16)
            ),
            Err(RepositoryError::NotFound)
        ));
        assert!(matches!(
            GroupRepository::set_member_muted(
                &database,
                id,
                stored.revision,
                second,
                true,
                TimestampMillis::new(16)
            ),
            Err(RepositoryError::Invalid(_))
        ));
        let before_failure = GroupRepository::get(&database, id)
            .expect("get before injected failure")
            .expect("group before injected failure");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "CREATE TRIGGER fail_group_member_insert BEFORE INSERT ON group_members BEGIN SELECT RAISE(ABORT, 'test rollback'); END",
                [],
            )
            .expect("install rollback trigger");
        drop(connection);
        assert!(
            GroupRepository::replace_members(
                &database,
                id,
                before_failure.group.revision,
                before_failure.group.members.clone(),
                TimestampMillis::new(17)
            )
            .is_err()
        );
        let connection = database.connection().expect("database lock");
        connection
            .execute("DROP TRIGGER fail_group_member_insert", [])
            .expect("remove rollback trigger");
        drop(connection);
        assert_eq!(
            GroupRepository::get(&database, id)
                .expect("get after injected failure")
                .expect("group after injected failure"),
            before_failure
        );
        assert_eq!(
            GroupRepository::get(&database, id)
                .expect("get")
                .expect("group")
                .group
                .revision,
            stored.revision
        );
        assert_eq!(
            GroupRepository::get(&database, id)
                .expect("get")
                .expect("group")
                .group
                .presentation,
            replacement_presentation
        );
        assert_eq!(stored.revision.get(), 16);
    }

    #[test]
    fn group_assignment_validation_and_strict_corruption_rejection() {
        let database = Database::open_in_memory().expect("database");
        let first = CharacterId::new();
        let second = CharacterId::new();
        character(&database, first);
        character(&database, second);
        let group = GroupProfile::new(
            GroupId::new(),
            "Strict".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(1),
        )
        .expect("group");
        let missing = CharacterId::new();
        assert!(matches!(
            GroupRepository::create(
                &database,
                CreateGroupPlan {
                    group: GroupProfile {
                        members: vec![
                            GroupMember {
                                character_id: missing,
                                ordinal: 0,
                                muted: false,
                                model_profile_override: None
                            },
                            GroupMember {
                                character_id: second,
                                ordinal: 1,
                                muted: false,
                                model_profile_override: None
                            },
                        ],
                        ..group.clone()
                    },
                    starting_scene: None,
                }
            ),
            Err(RepositoryError::NotFound)
        ));
        let created = GroupRepository::create(
            &database,
            CreateGroupPlan {
                group,
                starting_scene: None,
            },
        )
        .expect("create");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "UPDATE groups SET presentation_json='not json' WHERE id=?1",
                [created.group.id.to_string()],
            )
            .expect("corrupt presentation");
        drop(connection);
        assert!(matches!(
            GroupRepository::get(&database, created.group.id),
            Err(RepositoryError::Invalid(_))
        ));

        let scene_group = GroupProfile::new(
            GroupId::new(),
            "Document".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(1),
        )
        .expect("scene group");
        let scene_id = SceneId::new();
        let scene = Scene::new(
            scene_id,
            SceneOwner::Group(scene_group.id),
            0,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Text".into(),
            }])
            .expect("document"),
            TimestampMillis::new(1),
        )
        .expect("scene");
        let scene_created = GroupRepository::create(
            &database,
            CreateGroupPlan {
                group: GroupProfile {
                    starting_scene_id: Some(scene_id),
                    ..scene_group
                },
                starting_scene: Some(GroupStartingScene {
                    scene,
                    variants: Vec::new(),
                }),
            },
        )
        .expect("scene group create");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "UPDATE group_starting_scenes SET content_json='{\"format_version\":99,\"value\":{}}' WHERE group_id=?1",
                [scene_created.group.id.to_string()],
            )
            .expect("corrupt scene document");
        drop(connection);
        assert!(matches!(
            GroupRepository::get(&database, scene_created.group.id),
            Err(RepositoryError::Invalid(_))
        ));

        let database = Database::open_in_memory().expect("association database");
        let first = CharacterId::new();
        let second = CharacterId::new();
        character(&database, first);
        character(&database, second);
        let asset = image_asset(&database, 'f');
        let group = GroupProfile::new(
            GroupId::new(),
            "Association".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(1),
        )
        .expect("group");
        let mut presentation = ChatAppearanceV1::default();
        presentation.chat_widget_slots.left.push(WidgetNode::Image {
            id: "image".into(),
            design: None,
            title: None,
            description: None,
            source: WidgetImageSource::LogicalAsset { asset_id: asset },
            shape: None,
        });
        let group = GroupProfile {
            presentation: presentation.clone(),
            ..group
        };
        let created = GroupRepository::create(
            &database,
            CreateGroupPlan {
                group,
                starting_scene: None,
            },
        )
        .expect("create");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "UPDATE group_presentation_asset_refs SET blob_kind='audio' WHERE group_id=?1 AND asset_id=?2",
                rusqlite::params![created.group.id.to_string(), asset.to_string()],
            )
            .expect_err("CHECK should protect association");
        connection
            .execute(
                "DELETE FROM group_presentation_asset_refs WHERE group_id=?1",
                [created.group.id.to_string()],
            )
            .expect("remove association");
        drop(connection);
        assert!(matches!(
            GroupRepository::get(&database, created.group.id),
            Err(RepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn group_list_cursor_and_two_handle_cas_are_deterministic() {
        let path = std::env::temp_dir().join(format!("lettuce-groups-{}.db", GroupId::new()));
        let first_database = Database::open(&path).expect("first database");
        let a = CharacterId::new();
        let b = CharacterId::new();
        character(&first_database, a);
        character(&first_database, b);
        let mut ids = Vec::new();
        for (index, now) in [10_i64, 20, 30].into_iter().enumerate() {
            let group = GroupProfile::new(
                GroupId::new(),
                format!("Group {index}"),
                vec![
                    GroupMember {
                        character_id: a,
                        ordinal: 0,
                        muted: false,
                        model_profile_override: None,
                    },
                    GroupMember {
                        character_id: b,
                        ordinal: 1,
                        muted: false,
                        model_profile_override: None,
                    },
                ],
                TimestampMillis::new(now),
            )
            .expect("group");
            ids.push(group.id);
            GroupRepository::create(
                &first_database,
                CreateGroupPlan {
                    group,
                    starting_scene: None,
                },
            )
            .expect("create");
        }
        let page = GroupRepository::list(
            &first_database,
            PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(2),
            },
            false,
        )
        .expect("first page");
        assert_eq!(page.items.len(), 2);
        let cursor = page.next_cursor.clone().expect("next cursor");
        let next = GroupRepository::list(
            &first_database,
            PageRequest {
                cursor: Some(cursor),
                limit: lettuce_types::PageLimit::new(2),
            },
            false,
        )
        .expect("second page");
        assert_eq!(next.items.len(), 1);
        assert!(next.next_cursor.is_none());
        assert_eq!(page.items[0].updated_at, TimestampMillis::new(30));
        assert!(ids.contains(&page.items[0].id));

        let second_database = Database::open(&path).expect("second database");
        let id = page.items[0].id;
        let renamed = GroupRepository::rename(
            &first_database,
            id,
            page.items[0].revision,
            "Winner".into(),
            TimestampMillis::new(40),
        )
        .expect("first handle write");
        assert!(matches!(
            GroupRepository::set_chat_mode(
                &second_database,
                id,
                page.items[0].revision,
                ChatMode::Roleplay,
                TimestampMillis::new(41)
            ),
            Err(RepositoryError::StaleRevision { .. })
        ));
        assert_eq!(
            GroupRepository::get(&second_database, id)
                .expect("snapshot")
                .expect("group")
                .group
                .name,
            renamed.name
        );
        drop(second_database);
        drop(first_database);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn group_aggregate_read_transaction_holds_a_two_handle_snapshot() {
        let path =
            std::env::temp_dir().join(format!("lettuce-group-snapshot-{}.db", GroupId::new()));
        let setup = Database::open(&path).expect("setup database");
        let first = CharacterId::new();
        let second = CharacterId::new();
        character(&setup, first);
        character(&setup, second);
        let group = GroupProfile::new(
            GroupId::new(),
            "Snapshot".into(),
            vec![
                GroupMember {
                    character_id: first,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second,
                    ordinal: 1,
                    muted: false,
                    model_profile_override: None,
                },
            ],
            TimestampMillis::new(1),
        )
        .expect("group");
        let details = GroupRepository::create(
            &setup,
            CreateGroupPlan {
                group,
                starting_scene: None,
            },
        )
        .expect("create");
        let group_id = details.group.id;
        let old_name = details.group.name.clone();
        drop(setup);

        let reader = Database::open(&path).expect("reader database");
        let writer = Database::open(&path).expect("writer database");
        let mut reader_connection = reader.connection().expect("reader lock");
        let tx = reader_connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("read transaction");
        tx.query_row(
            "SELECT revision FROM groups WHERE id=?1",
            [group_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .expect("establish group snapshot");
        let writer_connection = writer.connection().expect("writer lock");
        writer_connection
            .execute(
                "UPDATE groups SET name='Changed',normalized_name='changed',revision=revision+1,updated_at=2 WHERE id=?1",
                [group_id.to_string()],
            )
            .expect("write after snapshot");
        drop(writer_connection);

        let snapshot = load_details(&tx, group_id)
            .expect("read group snapshot")
            .expect("group");
        assert_eq!(snapshot.group.name, old_name);
        tx.commit().expect("commit read transaction");
        drop(reader_connection);
        let latest = GroupRepository::get(&reader, group_id)
            .expect("latest read")
            .expect("latest group");
        assert_eq!(latest.group.name, "Changed");
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }
}
