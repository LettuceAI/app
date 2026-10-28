//! SQLite persistence for the authored persona aggregate.
//!
//! Persona storage is intentionally independent from character, group and
//! conversation storage.  The adapter owns the canonical scalar columns,
//! private versioned payload envelopes and the singleton default state.

use lettuce_characters::{
    Crop, DependencyReference, DependencyReport, ImageRecommendation, LifecycleStatus, Persona,
    PersonaArchiveRequest, PersonaArchiveResult, PersonaDefaultSnapshot, PersonaDefaultState,
    PersonaDependencyReader, PersonaDraftUpdate, PersonaMedia, PersonaMediaLink, PersonaMediaSlot,
    PersonaRepository, PersonaSearch, RepositoryError,
};
use lettuce_context::BindingRepositoryError;
use lettuce_sync::{
    ChangeOperation, LocalChangeJournalError, NewCanonicalChange,
    canonical_persona_default_payload, canonical_persona_payload, media_asset_create_operation,
    media_asset_insert_change, persona_archive_default_operation, persona_archive_operation,
    persona_attach_media_operation, persona_clear_default_operation, persona_create_operation,
    persona_default_sync_entity, persona_detach_media_operation, persona_reorder_media_operation,
    persona_restore_operation, persona_revise_operation, persona_set_default_operation,
    persona_sync_entity, persona_update_media_operation,
};
use lettuce_transfer::{PersonaFileImport, PersonaFileRepository};
use lettuce_types::{AssetId, Page, PageRequest, PersonaId, Revision, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};
use unicode_normalization::UnicodeNormalization;

use crate::Database;
use crate::sync::sync_adapter::{
    entity_scoped_operation, load_local_change_in, record_local_change_in,
};
const CROP_VERSION: u32 = 1;
const RECOMMENDATION_VERSION: u32 = 1;

use super::{Envelope, cursor_decode, cursor_encode};

fn invalid(field: &'static str) -> rusqlite::Error {
    let _ = field;
    rusqlite::Error::InvalidQuery
}

fn db_error(error: rusqlite::Error) -> RepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => {
            RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
                field: "persona.storage",
            })
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == 1555 || code.extended_code == 2067 =>
        {
            RepositoryError::AlreadyExists
        }
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 787 => {
            RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidReference {
                field: "persona.reference",
            })
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
                field: "persona.storage",
            })
        }
        _ => RepositoryError::Storage {
            reason: "db error: a stored value is not recognised".to_owned(),
        },
    }
}

fn sync_error(error: LocalChangeJournalError) -> RepositoryError {
    match error {
        LocalChangeJournalError::Conflict => RepositoryError::AlreadyExists,
        _ => RepositoryError::Storage {
            reason: "sync error: a stored value is not recognised".to_owned(),
        },
    }
}

fn persona_insert_change(persona: &Persona) -> Result<NewCanonicalChange, RepositoryError> {
    NewCanonicalChange::new(
        persona_sync_entity(persona.id).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?,
        ChangeOperation::Insert,
        None,
        Some(canonical_persona_payload(persona).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?),
    )
    .map_err(|error| RepositoryError::Storage { reason: error.to_string() })
}

fn persona_update_change(
    before: &Persona,
    after: &Persona,
) -> Result<NewCanonicalChange, RepositoryError> {
    let before = canonical_persona_payload(before).map_err(|error| RepositoryError::Storage {
        reason: error.to_string(),
    })?;
    NewCanonicalChange::new(
        persona_sync_entity(after.id).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?,
        ChangeOperation::Update,
        Some(before.content_hash().clone()),
        Some(canonical_persona_payload(after).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?),
    )
    .map_err(|error| RepositoryError::Storage { reason: error.to_string() })
}

fn persona_default_update_change(
    before: &PersonaDefaultState,
    after: &PersonaDefaultState,
) -> Result<NewCanonicalChange, RepositoryError> {
    let before = canonical_persona_default_payload(before).map_err(|error| RepositoryError::Storage {
        reason: error.to_string(),
    })?;
    NewCanonicalChange::new(
        persona_default_sync_entity().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?,
        ChangeOperation::Update,
        Some(before.content_hash().clone()),
        Some(canonical_persona_default_payload(after).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?),
    )
    .map_err(|error| RepositoryError::Storage { reason: error.to_string() })
}

fn replay_persona_update_change(
    tx: &Transaction<'_>,
    operation: lettuce_types::OperationId,
    id: PersonaId,
    expected_revision: Revision,
    now: TimestampMillis,
) -> Result<Option<Persona>, RepositoryError> {
    let Some(change) = load_local_change_in(tx, operation).map_err(sync_error)? else {
        return Ok(None);
    };
    let current = load_persona(tx, id)
        .map_err(db_error)?
        .ok_or(RepositoryError::NotFound)?;
    let next = expected_revision
        .next()
        .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
    if current.revision != next
        || current.updated_at != now
        || change.entity() != &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?
        || change.operation() != ChangeOperation::Update
        || change.payload()
            != Some(&canonical_persona_payload(&current).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?)
    {
        return Err(RepositoryError::StaleRevision {
            expected: expected_revision,
            actual: current.revision,
        });
    }
    Ok(Some(current))
}

fn replay_persona_default_change(
    tx: &Transaction<'_>,
    operation: lettuce_types::OperationId,
    expected_revision: Revision,
    now: TimestampMillis,
) -> Result<Option<PersonaDefaultState>, RepositoryError> {
    let Some(change) = load_local_change_in(tx, operation).map_err(sync_error)? else {
        return Ok(None);
    };
    let current = read_default(tx).map_err(db_error)?;
    let next = expected_revision
        .next()
        .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
    if current.revision != next
        || current.updated_at != now
        || change.entity()
            != &persona_default_sync_entity().map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?
        || change.operation() != ChangeOperation::Update
        || change.payload()
            != Some(
                &canonical_persona_default_payload(&current)
                    .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?,
            )
    {
        return Err(RepositoryError::StaleRevision {
            expected: expected_revision,
            actual: current.revision,
        });
    }
    Ok(Some(current))
}

fn apply_persona_draft(
    mut persona: Persona,
    draft: &PersonaDraftUpdate,
    now: TimestampMillis,
) -> Result<Persona, RepositoryError> {
    persona.title.clone_from(&draft.title);
    persona.description.clone_from(&draft.description);
    persona.nickname.clone_from(&draft.nickname);
    persona
        .design_description
        .clone_from(&draft.design_description);
    persona.avatar_crop = draft.avatar_crop;
    persona
        .image_recommendation
        .clone_from(&draft.image_recommendation);
    persona.bump_revision(now)?;
    persona.validate()?;
    Ok(persona)
}

fn encode<T: Serialize>(value: &T, version: u32) -> Result<String, RepositoryError> {
    serde_json::to_string(&Envelope {
        format_version: version,
        value,
    })
    .map_err(|_| {
        RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
            field: "persona.encoded",
        })
    })
}

fn decode<T: DeserializeOwned>(payload: &str, version: u32) -> Result<T, rusqlite::Error> {
    let envelope: Envelope<T> = serde_json::from_str(payload).map_err(|_| invalid("persona"))?;
    if envelope.format_version != version {
        return Err(invalid("persona.version"));
    }
    Ok(envelope.value)
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
        _ => Err(invalid("persona.status")),
    }
}

fn media_slot_name(value: PersonaMediaSlot) -> &'static str {
    match value {
        PersonaMediaSlot::Avatar => "avatar",
        PersonaMediaSlot::DesignReference => "design_reference",
    }
}

fn media_slot_from_name(value: &str) -> Result<PersonaMediaSlot, rusqlite::Error> {
    match value {
        "avatar" => Ok(PersonaMediaSlot::Avatar),
        "design_reference" => Ok(PersonaMediaSlot::DesignReference),
        _ => Err(invalid("persona.media.slot")),
    }
}

fn parse_id<T: std::str::FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| invalid("persona.id"))
}

fn revision(value: i64) -> Result<Revision, rusqlite::Error> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| invalid("persona.revision"))
}

fn sql_u64(value: u64) -> Result<i64, RepositoryError> {
    i64::try_from(value).map_err(|error| RepositoryError::Storage { reason: error.to_string() })
}

/// The canonical key used by both writes and searches.  NFKC is applied before
/// and after case conversion so composed/decomposed Unicode spellings share a
/// key while SQLite collation and locale never change the persisted value.
fn canonical_title(value: &str) -> String {
    value
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .nfkc()
        .collect()
}

fn image_assets(
    connection: &Connection,
    ids: impl IntoIterator<Item = AssetId>,
) -> Result<(), RepositoryError> {
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
                    field: "persona.asset",
                },
            ))?;
        if kind != "image" {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "persona.image_asset",
                },
            ));
        }
    }
    Ok(())
}

fn record_persona_media_assets(
    tx: &Transaction<'_>,
    ids: impl IntoIterator<Item = AssetId>,
    now: TimestampMillis,
) -> Result<(), RepositoryError> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        if crate::sync::sync_adapter::media_asset_journaled(tx, &id.to_string())
            .map_err(sync_error)?
        {
            continue;
        }
        let asset = crate::load_asset_with_blob(tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        let blob = tx
            .query_row(
                &format!(
                    "SELECT {} FROM media_blobs WHERE id=?1",
                    crate::MEDIA_BLOB_COLUMNS
                ),
                [asset.blob_id.to_string()],
                crate::media_from_row,
            )
            .map_err(db_error)?;
        let request = media_asset_insert_change(&lettuce_media::SyncMediaAsset { asset, blob })
            .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
        record_local_change_in(tx, media_asset_create_operation(id), &request, now)
            .map_err(sync_error)?;
    }
    Ok(())
}

fn parse_persona_row(row: &Row<'_>, id: PersonaId) -> rusqlite::Result<Persona> {
    let title = row.get::<_, String>(2)?;
    let nickname = row.get::<_, Option<String>>(4)?;
    let normalized_title = row.get::<_, String>(3)?;
    let normalized_nickname = row.get::<_, Option<String>>(5)?;
    if canonical_title(&title) != normalized_title
        || normalized_nickname.as_deref() != nickname.as_deref().map(canonical_title).as_deref()
    {
        return Err(invalid("persona.normalized_title"));
    }
    let avatar_crop = row
        .get::<_, Option<String>>(8)?
        .map(|payload| decode::<Crop>(&payload, CROP_VERSION))
        .transpose()?;
    let image_recommendation = row
        .get::<_, Option<String>>(9)?
        .map(|payload| decode::<ImageRecommendation>(&payload, RECOMMENDATION_VERSION))
        .transpose()?;
    let persona = Persona {
        id,
        status: status_from_name(&row.get::<_, String>(1)?)?,
        title,
        description: row.get(6)?,
        nickname,
        design_description: row.get(7)?,
        avatar_crop,
        image_recommendation,
        media: PersonaMedia::default(),
        revision: revision(row.get(10)?)?,
        created_at: TimestampMillis::new(row.get(11)?),
        updated_at: TimestampMillis::new(row.get(12)?),
    };
    persona.validate().map_err(|_| invalid("persona"))?;
    Ok(persona)
}

fn persona_row(connection: &Connection, id: PersonaId) -> Result<Option<Persona>, rusqlite::Error> {
    connection
        .query_row(
            "SELECT id,status,title,normalized_title,nickname,normalized_nickname,description,design_description,avatar_crop_json,image_recommendation_json,revision,created_at,updated_at FROM personas WHERE id=?1",
            [id.to_string()],
            |row| parse_persona_row(row, id),
        )
        .optional()
}

pub(crate) fn load_persona(
    connection: &Connection,
    id: PersonaId,
) -> Result<Option<Persona>, rusqlite::Error> {
    let Some(mut persona) = persona_row(connection, id)? else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT asset_id,blob_kind,slot,ordinal FROM persona_media WHERE persona_id=?1 ORDER BY slot,ordinal,asset_id",
    )?;
    let mut links = Vec::new();
    for row in statement.query_map([id.to_string()], |row| {
        let blob_kind = row.get::<_, String>(1)?;
        if blob_kind != "image" {
            return Err(invalid("persona.media.blob_kind"));
        }
        let slot = media_slot_from_name(&row.get::<_, String>(2)?)?;
        let ordinal: u32 = row
            .get::<_, i64>(3)?
            .try_into()
            .map_err(|_| invalid("persona.media.ordinal"))?;
        if slot == PersonaMediaSlot::Avatar && ordinal != 0 {
            return Err(invalid("persona.media.avatar.ordinal"));
        }
        Ok(PersonaMediaLink {
            asset_id: parse_id(row.get(0)?)?,
            slot,
            ordinal,
        })
    })? {
        links.push(row?);
    }
    image_assets(connection, links.iter().map(|link| link.asset_id))
        .map_err(|_| invalid("persona.media.asset"))?;
    persona.media = PersonaMedia { links };
    persona.validate().map_err(|_| invalid("persona.media"))?;
    Ok(Some(persona))
}

pub(crate) fn get_persona(
    connection: &Connection,
    id: PersonaId,
) -> Result<Option<Persona>, RepositoryError> {
    load_persona(connection, id).map_err(db_error)
}

pub(crate) fn read_default(
    connection: &Connection,
) -> Result<PersonaDefaultState, rusqlite::Error> {
    let state = connection
        .query_row(
            "SELECT default_persona_id,revision,created_at,updated_at FROM persona_defaults WHERE id=1",
            [],
            |row| {
                Ok(PersonaDefaultState {
                    persona_id: row.get::<_, Option<String>>(0)?.map(parse_id).transpose()?,
                    revision: revision(row.get(1)?)?,
                    created_at: TimestampMillis::new(row.get(2)?),
                    updated_at: TimestampMillis::new(row.get(3)?),
                })
            },
        )
        .optional()?
        .ok_or_else(|| invalid("persona.default"))?;
    state.validate().map_err(|_| invalid("persona.default"))?;
    Ok(state)
}

fn ensure_active(
    connection: &Connection,
    id: PersonaId,
    expected_revision: Revision,
) -> Result<Persona, RepositoryError> {
    let current = load_persona(connection, id)
        .map_err(db_error)?
        .ok_or(RepositoryError::NotFound)?;
    if current.revision != expected_revision {
        return Err(RepositoryError::StaleRevision {
            expected: expected_revision,
            actual: current.revision,
        });
    }
    if current.status != LifecycleStatus::Active {
        return Err(RepositoryError::Archived);
    }
    Ok(current)
}

fn normalize_media(mut media: PersonaMedia) -> Result<PersonaMedia, RepositoryError> {
    media.validate()?;
    let links = std::mem::take(&mut media.links);
    let mut avatar = None;
    let mut design = Vec::new();
    for link in links {
        match link.slot {
            PersonaMediaSlot::Avatar => {
                avatar = Some(link);
            }
            PersonaMediaSlot::DesignReference => design.push(link),
        }
    }
    design.sort_by_key(|link| link.ordinal);
    if let Some(mut link) = avatar {
        link.ordinal = 0;
        media.links.push(link);
    }
    for (ordinal, mut link) in design.into_iter().enumerate() {
        link.ordinal = u32::try_from(ordinal).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        media.links.push(link);
    }
    media.validate()?;
    Ok(media)
}

fn replace_media(
    tx: &Transaction<'_>,
    persona_id: PersonaId,
    media: &PersonaMedia,
) -> Result<(), RepositoryError> {
    let links = media.links.iter().map(|link| (link.asset_id.to_string(), media_slot_name(link.slot), link.ordinal)).collect::<Vec<_>>();
    super::replace_media_links(tx, &persona_id.to_string(), &links, ("persona_media", "persona_id", true), db_error)
}

pub(crate) fn insert_persona(
    tx: &Transaction<'_>,
    persona: Persona,
) -> Result<Persona, RepositoryError> {
    persona.validate()?;
    image_assets(tx, persona.media.links.iter().map(|link| link.asset_id))?;
    let media = normalize_media(persona.media.clone())?;
    if tx
        .query_row(
            "SELECT 1 FROM personas WHERE id=?1",
            [persona.id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(db_error)?
        .is_some()
    {
        return Err(RepositoryError::AlreadyExists);
    }
    let crop = persona
        .avatar_crop
        .as_ref()
        .map(|value| encode(value, CROP_VERSION))
        .transpose()?;
    let recommendation = persona
        .image_recommendation
        .as_ref()
        .map(|value| encode(value, RECOMMENDATION_VERSION))
        .transpose()?;
    tx.execute(
        "INSERT INTO personas(id,status,title,normalized_title,nickname,normalized_nickname,description,design_description,avatar_crop_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            persona.id.to_string(),
            status_name(persona.status),
            persona.title,
            canonical_title(&persona.title),
            persona.nickname,
            persona.nickname.as_deref().map(canonical_title),
            persona.description,
            persona.design_description,
            crop,
            recommendation,
            sql_u64(persona.revision.get())?,
            persona.created_at.get(),
            persona.updated_at.get()
        ],
    )
    .map_err(db_error)?;
    replace_media(tx, persona.id, &media)?;
    load_persona(tx, persona.id)
        .map_err(db_error)?
        .ok_or_else(|| RepositoryError::Storage {
            reason: "insert persona: a required value is missing".to_owned(),
        })
}

pub(crate) fn apply_synced_persona(
    tx: &Transaction<'_>,
    persona: Persona,
) -> Result<Persona, RepositoryError> {
    persona.validate()?;
    image_assets(tx, persona.media.links.iter().map(|link| link.asset_id))?;
    let media = normalize_media(persona.media.clone())?;
    if load_persona(tx, persona.id).map_err(db_error)?.is_none() {
        return insert_persona(tx, persona);
    }
    let crop = persona
        .avatar_crop
        .as_ref()
        .map(|value| encode(value, CROP_VERSION))
        .transpose()?;
    let recommendation = persona
        .image_recommendation
        .as_ref()
        .map(|value| encode(value, RECOMMENDATION_VERSION))
        .transpose()?;
    tx.execute(
        "UPDATE personas SET status=?2,title=?3,normalized_title=?4,nickname=?5,
         normalized_nickname=?6,description=?7,design_description=?8,
         avatar_crop_json=?9,image_recommendation_json=?10,revision=?11,
         created_at=?12,updated_at=?13 WHERE id=?1",
        params![
            persona.id.to_string(),
            status_name(persona.status),
            persona.title,
            canonical_title(&persona.title),
            persona.nickname,
            persona.nickname.as_deref().map(canonical_title),
            persona.description,
            persona.design_description,
            crop,
            recommendation,
            sql_u64(persona.revision.get())?,
            persona.created_at.get(),
            persona.updated_at.get()
        ],
    )
    .map_err(db_error)?;
    replace_media(tx, persona.id, &media)?;
    load_persona(tx, persona.id)
        .map_err(db_error)?
        .ok_or_else(|| RepositoryError::Storage {
            reason: "apply synced persona: a required value is missing".to_owned(),
        })
}

pub(crate) fn apply_synced_persona_default(
    tx: &Transaction<'_>,
    state: PersonaDefaultState,
) -> Result<PersonaDefaultState, RepositoryError> {
    state.validate()?;
    if let Some(id) = state.persona_id {
        let persona = load_persona(tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if persona.status != LifecycleStatus::Active {
            return Err(RepositoryError::Archived);
        }
    }
    tx.execute(
        "UPDATE persona_defaults SET default_persona_id=?1,revision=?2,
         created_at=?3,updated_at=?4 WHERE id=1",
        params![
            state.persona_id.map(|id| id.to_string()),
            sql_u64(state.revision.get())?,
            state.created_at.get(),
            state.updated_at.get()
        ],
    )
    .map_err(db_error)?;
    read_default(tx).map_err(db_error)
}

pub(crate) fn revise_persona(
    tx: &Transaction<'_>,
    id: PersonaId,
    expected_revision: Revision,
    draft: PersonaDraftUpdate,
    now: TimestampMillis,
) -> Result<Persona, RepositoryError> {
    draft.validate()?;
    ensure_active(tx, id, expected_revision)?;
    let next = expected_revision
        .next()
        .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
    let crop = draft
        .avatar_crop
        .as_ref()
        .map(|value| encode(value, CROP_VERSION))
        .transpose()?;
    let recommendation = draft
        .image_recommendation
        .as_ref()
        .map(|value| encode(value, RECOMMENDATION_VERSION))
        .transpose()?;
    let changed = tx
        .execute(
            "UPDATE personas SET title=?2,normalized_title=?3,nickname=?4,normalized_nickname=?5,description=?6,design_description=?7,avatar_crop_json=?8,image_recommendation_json=?9,revision=?10,updated_at=?11 WHERE id=?1 AND revision=?12",
            params![
                id.to_string(),
                draft.title,
                canonical_title(&draft.title),
                draft.nickname,
                draft.nickname.as_deref().map(canonical_title),
                draft.description,
                draft.design_description,
                crop,
                recommendation,
                sql_u64(next.get())?,
                now.get(),
                sql_u64(expected_revision.get())?
            ],
        )
        .map_err(db_error)?;
    if changed == 0 {
        let actual = persona_row(tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?
            .revision;
        return Err(RepositoryError::StaleRevision {
            expected: expected_revision,
            actual,
        });
    }
    load_persona(tx, id)
        .map_err(db_error)?
        .ok_or_else(|| RepositoryError::Storage {
            reason: "revise persona: a required value is missing".to_owned(),
        })
}

fn update_root(
    tx: &Transaction<'_>,
    id: PersonaId,
    expected_revision: Revision,
    now: TimestampMillis,
) -> Result<Revision, RepositoryError> {
    let next = expected_revision
        .next()
        .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
    let changed = tx
        .execute(
            "UPDATE personas SET revision=?2,updated_at=?3 WHERE id=?1 AND revision=?4",
            params![
                id.to_string(),
                sql_u64(next.get())?,
                now.get(),
                sql_u64(expected_revision.get())?
            ],
        )
        .map_err(db_error)?;
    if changed == 0 {
        let actual = persona_row(tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?
            .revision;
        return Err(RepositoryError::StaleRevision {
            expected: expected_revision,
            actual,
        });
    }
    Ok(next)
}


fn create_persona_in(tx: &Transaction<'_>, persona: Persona) -> Result<Persona, RepositoryError> {
    let mut canonical = persona.clone();
    canonical.media = normalize_media(canonical.media)?;
    canonical.validate()?;
    let operation = persona_create_operation(canonical.id);
    let request = persona_insert_change(&canonical)?;
    let operation = entity_scoped_operation(
        tx,
        &persona_sync_entity(canonical.id).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?,
        operation,
    )
    .map_err(sync_error)?;
    record_persona_media_assets(
        tx,
        canonical.media.links.iter().map(|link| link.asset_id),
        canonical.created_at,
    )?;
    let admission = record_local_change_in(tx, operation, &request, canonical.created_at)
        .map_err(sync_error)?;
    if admission.created {
        return insert_persona(tx, canonical);
    }
    let stored = load_persona(tx, persona.id)
        .map_err(db_error)?
        .ok_or_else(|| RepositoryError::Storage {
            reason: "create persona in: a required value is missing".to_owned(),
        })?;
    if canonical_persona_payload(&stored).map_err(|error| RepositoryError::Storage {
        reason: error.to_string(),
    })?
        != *admission.change.payload().ok_or_else(|| RepositoryError::Storage {
            reason: "create persona in: a required value is missing".to_owned(),
        })?
    {
        return Err(RepositoryError::Storage {
            reason: "create persona in: stored data failed a consistency check".to_owned(),
        });
    }
    Ok(stored)
}

fn set_default_in(
    tx: &Transaction<'_>,
    id: PersonaId,
    expected_default_revision: Revision,
    now: TimestampMillis,
) -> Result<PersonaDefaultState, RepositoryError> {
    let operation = persona_set_default_operation(expected_default_revision, id);
    let operation = entity_scoped_operation(
        tx,
        &persona_default_sync_entity().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?,
        operation,
    )
    .map_err(sync_error)?;
    if let Some(state) =
        replay_persona_default_change(tx, operation, expected_default_revision, now)?
    {
        return Ok(state);
    }
    let current = read_default(tx).map_err(db_error)?;
    if current.revision != expected_default_revision {
        return Err(RepositoryError::StaleRevision {
            expected: expected_default_revision,
            actual: current.revision,
        });
    }
    let persona = load_persona(tx, id)
        .map_err(db_error)?
        .ok_or(RepositoryError::NotFound)?;
    if persona.status != LifecycleStatus::Active {
        return Err(RepositoryError::Archived);
    }
    let next = expected_default_revision
        .next()
        .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
    let changed = tx
        .execute(
            "UPDATE persona_defaults SET default_persona_id=?1,revision=?2,updated_at=?3 WHERE id=1 AND revision=?4",
            params![
                id.to_string(),
                sql_u64(next.get())?,
                now.get(),
                sql_u64(expected_default_revision.get())?
            ],
        )
        .map_err(db_error)?;
    if changed == 0 {
        let actual = read_default(tx).map_err(db_error)?.revision;
        return Err(RepositoryError::StaleRevision {
            expected: expected_default_revision,
            actual,
        });
    }
    let state = read_default(tx).map_err(db_error)?;
    let request = persona_default_update_change(&current, &state)?;
    let admission = record_local_change_in(tx, operation, &request, now).map_err(sync_error)?;
    if !admission.created {
        return Err(RepositoryError::Storage {
            reason: "set default in: stored data failed a consistency check".to_owned(),
        });
    }
    Ok(state)
}

fn clear_default_in(
    tx: &Transaction<'_>,
    expected_default_revision: Revision,
    now: TimestampMillis,
) -> Result<PersonaDefaultState, RepositoryError> {
    let operation = persona_clear_default_operation(expected_default_revision);
    let operation = entity_scoped_operation(
        tx,
        &persona_default_sync_entity().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?,
        operation,
    )
    .map_err(sync_error)?;
    if let Some(state) =
        replay_persona_default_change(tx, operation, expected_default_revision, now)?
    {
        return Ok(state);
    }
    let current = read_default(tx).map_err(db_error)?;
    if current.revision != expected_default_revision {
        return Err(RepositoryError::StaleRevision {
            expected: expected_default_revision,
            actual: current.revision,
        });
    }
    let next = expected_default_revision
        .next()
        .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
    let changed = tx
        .execute(
            "UPDATE persona_defaults SET default_persona_id=NULL,revision=?1,updated_at=?2 WHERE id=1 AND revision=?3",
            params![
                sql_u64(next.get())?,
                now.get(),
                sql_u64(expected_default_revision.get())?
            ],
        )
        .map_err(db_error)?;
    if changed == 0 {
        let actual = read_default(tx).map_err(db_error)?.revision;
        return Err(RepositoryError::StaleRevision {
            expected: expected_default_revision,
            actual,
        });
    }
    let state = read_default(tx).map_err(db_error)?;
    let request = persona_default_update_change(&current, &state)?;
    let admission = record_local_change_in(tx, operation, &request, now).map_err(sync_error)?;
    if !admission.created {
        return Err(RepositoryError::Storage {
            reason: "clear default in: stored data failed a consistency check".to_owned(),
        });
    }
    Ok(state)
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn import_persona_file(&self, import: &PersonaFileImport) -> Result<Persona, RepositoryError> {
        import_persona_file_in(self.transaction, import)
    }
}

fn import_persona_file_in(tx: &rusqlite::Transaction<'_>, import: &PersonaFileImport) -> Result<Persona, RepositoryError> {
    let persona = create_persona_in(tx, import.persona.clone())?;
    let mut revision = persona.revision;
    for lorebook_id in &import.lorebook_ids {
        revision = crate::lorebook::lorebook_adapter::bind_persona_lorebook_in(
            tx,
            persona.id,
            revision,
            *lorebook_id,
            !import.disabled_lorebook_ids.contains(lorebook_id),
            import.persona.created_at,
        )
        .map_err(|error| match error {
            BindingRepositoryError::NotFound => RepositoryError::NotFound,
            BindingRepositoryError::Conflict => RepositoryError::AlreadyExists,
            BindingRepositoryError::Invalid(_) => RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "persona.lorebooks",
                },
            ),
            BindingRepositoryError::Failure(_) => RepositoryError::Storage {
                reason: "import persona file in: a stored value is not recognised".to_owned(),
            },
        })?;
    }
    if import.make_default {
        let current = read_default(tx).map_err(db_error)?.revision;
        set_default_in(tx, persona.id, current, import.persona.created_at)?;
    }
    let persona = load_persona(tx, persona.id)
        .map_err(db_error)?
        .ok_or_else(|| RepositoryError::Storage {
            reason: "import persona file in: a required value is missing".to_owned(),
        })?;
    Ok(persona)
}

impl PersonaFileRepository for Database {
    fn import_persona_file(&self, import: &PersonaFileImport) -> Result<Persona, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        let persona = import_persona_file_in(&tx, import)?;
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn discard_unlinked_asset(
        &self,
        asset_id: AssetId,
    ) -> Result<Option<lettuce_types::MediaBlobId>, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let Some(blob_id) = tx
            .query_row(
                "SELECT blob_id FROM media_assets WHERE id=?1",
                [asset_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(db_error)?
        else {
            return Ok(None);
        };
        match tx.execute(
            "DELETE FROM media_assets WHERE id=?1",
            [asset_id.to_string()],
        ) {
            Ok(1) => {}
            Ok(_) => return Ok(None),
            Err(error)
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(db_error(error)),
        }
        tx.commit().map_err(db_error)?;
        blob_id
            .parse()
            .map(Some)
            .map_err(|error| RepositoryError::Storage { reason: format!("{error}") })
    }

    fn release_unused_blob(
        &self,
        blob_id: lettuce_types::MediaBlobId,
        now: TimestampMillis,
    ) -> Result<bool, RepositoryError> {
        let connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let released = connection
            .execute(
                "UPDATE media_blobs SET state='missing', updated_at=?2
                  WHERE id=?1 AND state='ready'
                    AND NOT EXISTS (SELECT 1 FROM media_assets WHERE blob_id=?1)
                    AND NOT EXISTS (SELECT 1 FROM legacy_import_media_completions WHERE blob_id=?1)",
                params![blob_id.to_string(), now.get()],
            )
            .map_err(db_error)?;
        Ok(released == 1)
    }
}

impl PersonaRepository for Database {
    fn create(&self, persona: Persona) -> Result<Persona, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let stored = create_persona_in(&tx, persona)?;
        tx.commit().map_err(db_error)?;
        Ok(stored)
    }

    fn get(&self, id: PersonaId) -> Result<Option<Persona>, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let persona = load_persona(&tx, id).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    /// A default that names an archived persona reads as no default.
    fn get_default_snapshot(&self) -> Result<PersonaDefaultSnapshot, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let mut state = read_default(&tx).map_err(db_error)?;
        let mut persona = state
            .persona_id
            .map(|id| load_persona(&tx, id).map_err(db_error))
            .transpose()?
            .flatten();
        if state.persona_id.is_some() && persona.is_none() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidReference {
                    field: "persona.default.persona_id",
                },
            ));
        }
        if persona
            .as_ref()
            .is_some_and(|persona| persona.status != LifecycleStatus::Active)
        {
            state.persona_id = None;
            persona = None;
        }
        let snapshot = PersonaDefaultSnapshot { state, persona };
        snapshot.validate()?;
        tx.commit().map_err(db_error)?;
        Ok(snapshot)
    }

    fn search(
        &self,
        request: PersonaSearch,
        page: PageRequest,
    ) -> Result<Page<Persona>, RepositoryError> {
        use lettuce_characters::LibrarySort;

        let query = canonical_title(request.text.trim());
        let scope_text = if request.search_description {
            format!("\0{query}")
        } else {
            query.clone()
        };
        let cursor = cursor_decode(page.cursor.as_deref(), (request.sort, scope_text.clone(), request.search_nickname, request.include_archived, "persona"))?;
        let limit = usize::from(page.limit.get()).max(1);
        let pattern = format!(
            "%{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let (column, comparison, direction) = match request.sort {
            LibrarySort::CreatedAsc => ("created_at", ">", "ASC"),
            LibrarySort::UpdatedDesc => ("updated_at", "<", "DESC"),
            LibrarySort::Name => ("normalized_title", ">", "ASC"),
        };
        let sql = format!(
            "SELECT id FROM personas WHERE (?1 OR status='active') AND (normalized_title LIKE ?2 ESCAPE '\\' OR (?7 AND persona_search_key(description) LIKE ?2 ESCAPE '\\') OR (?6 AND normalized_nickname LIKE ?2 ESCAPE '\\')) AND (?3 IS NULL OR {column} {comparison} ?3 OR ({column} = ?3 AND id > ?4)) ORDER BY {column} {direction},id ASC LIMIT ?5"
        );
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        connection
            .create_scalar_function(
                "persona_search_key",
                1,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8
                    | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
                    | rusqlite::functions::FunctionFlags::SQLITE_INNOCUOUS,
                |context| Ok(canonical_title(&context.get::<String>(0)?)),
            )
            .map_err(db_error)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let mut statement = tx.prepare(&sql).map_err(db_error)?;
        let ids = statement
            .query_map(
                params![
                    request.include_archived,
                    pattern,
                    cursor.as_ref().map(|value| &value.0),
                    cursor.as_ref().map(|value| &value.1),
                    i64::try_from(limit + 1).unwrap_or(i64::MAX),
                    request.search_nickname,
                    request.search_description
                ],
                |row| parse_id(row.get(0)?),
            )
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<PersonaId>>>()
            .map_err(db_error)?;
        drop(statement);
        let has_more = ids.len() > limit;
        let mut items = Vec::with_capacity(limit.min(ids.len()));
        for id in ids.into_iter().take(limit) {
            items.push(
                load_persona(&tx, id)
                    .map_err(db_error)?
                    .ok_or_else(|| RepositoryError::Storage {
                        reason: "search: a required value is missing".to_owned(),
                    })?,
            );
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| {
                    cursor_encode(match request.sort {
                        LibrarySort::CreatedAsc => serde_json::json!(item.created_at.get()),
                        LibrarySort::UpdatedDesc => serde_json::json!(item.updated_at.get()),
                        LibrarySort::Name => serde_json::json!(canonical_title(&item.title)),
                    }, item.id.to_string(), (request.sort, scope_text.clone(), request.search_nickname, request.include_archived, "persona"))
                })
                .transpose()?
        } else {
            None
        };
        tx.commit().map_err(db_error)?;
        Ok(Page { items, next_cursor })
    }

    fn revise(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        draft: PersonaDraftUpdate,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        draft.validate()?;
        let operation = persona_revise_operation(id, expected_revision);
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        if let Some(change) = load_local_change_in(&tx, operation).map_err(sync_error)? {
            let current = load_persona(&tx, id)
                .map_err(db_error)?
                .ok_or(RepositoryError::NotFound)?;
            let next = expected_revision
                .next()
                .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
            let expected = apply_persona_draft(
                Persona {
                    revision: expected_revision,
                    updated_at: current.created_at,
                    ..current.clone()
                },
                &draft,
                now,
            )?;
            if current.revision != next
                || current != expected
                || change.entity()
                    != &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                        reason: error.to_string(),
                    })?
                || change.operation() != ChangeOperation::Update
                || change.payload()
                    != Some(
                        &canonical_persona_payload(&current)
                            .map_err(|error| RepositoryError::Storage {
                                reason: error.to_string(),
                            })?,
                    )
            {
                return Err(RepositoryError::StaleRevision {
                    expected: expected_revision,
                    actual: current.revision,
                });
            }
            tx.commit().map_err(db_error)?;
            return Ok(current);
        }
        let before = ensure_active(&tx, id, expected_revision)?;
        let expected = apply_persona_draft(before.clone(), &draft, now)?;
        let request = persona_update_change(&before, &expected)?;
        record_persona_media_assets(
            &tx,
            expected.media.links.iter().map(|link| link.asset_id),
            now,
        )?;
        let admission =
            record_local_change_in(&tx, operation, &request, now).map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "revise: stored data failed a consistency check".to_owned(),
            });
        }
        let persona = revise_persona(&tx, id, expected_revision, draft, now)?;
        if persona != expected {
            return Err(RepositoryError::Storage {
                reason: "revise: stored data failed a consistency check".to_owned(),
            });
        }
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn update_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        media: PersonaMedia,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let media = normalize_media(media)?;
        let operation = persona_update_media_operation(id, expected_revision, &media);
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        if let Some(persona) =
            replay_persona_update_change(&tx, operation, id, expected_revision, now)?
        {
            tx.commit().map_err(db_error)?;
            return Ok(persona);
        }
        let before = ensure_active(&tx, id, expected_revision)?;
        image_assets(&tx, media.links.iter().map(|link| link.asset_id))?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or_else(|| RepositoryError::Storage {
                reason: "update media: a required value is missing".to_owned(),
            })?;
        let request = persona_update_change(&before, &persona)?;
        record_persona_media_assets(
            &tx,
            persona.media.links.iter().map(|link| link.asset_id),
            now,
        )?;
        let admission =
            record_local_change_in(&tx, operation, &request, now).map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "update media: stored data failed a consistency check".to_owned(),
            });
        }
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn attach_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        link: PersonaMediaLink,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let operation = persona_attach_media_operation(id, expected_revision, &link);
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        if let Some(persona) =
            replay_persona_update_change(&tx, operation, id, expected_revision, now)?
        {
            tx.commit().map_err(db_error)?;
            return Ok(persona);
        }
        let current = ensure_active(&tx, id, expected_revision)?;
        image_assets(&tx, [link.asset_id])?;
        if current
            .media
            .links
            .iter()
            .any(|value| value.asset_id == link.asset_id)
        {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::Duplicate {
                    field: "persona.media.asset_ids",
                },
            ));
        }
        let mut links: Vec<_> = current
            .media
            .links
            .iter()
            .filter(|value| value.slot != link.slot)
            .cloned()
            .collect();
        let mut same_slot: Vec<_> = current
            .media
            .links
            .iter()
            .filter(|value| value.slot == link.slot)
            .cloned()
            .collect();
        let target = if link.slot == PersonaMediaSlot::Avatar {
            if !same_slot.is_empty() {
                return Err(RepositoryError::Invalid(
                    lettuce_characters::ValidationError::Invariant {
                        field: "persona.media.avatar",
                    },
                ));
            }
            0
        } else {
            usize::try_from(link.ordinal).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?
        };
        if target > same_slot.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "persona.media.ordinal",
                },
            ));
        }
        same_slot.insert(target, link);
        for (ordinal, link) in same_slot.iter_mut().enumerate() {
            link.ordinal = u32::try_from(ordinal).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?;
        }
        links.extend(same_slot);
        let media = normalize_media(PersonaMedia { links })?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or_else(|| RepositoryError::Storage {
                reason: "attach media: a required value is missing".to_owned(),
            })?;
        let request = persona_update_change(&current, &persona)?;
        record_persona_media_assets(
            &tx,
            persona.media.links.iter().map(|link| link.asset_id),
            now,
        )?;
        let admission =
            record_local_change_in(&tx, operation, &request, now).map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "attach media: stored data failed a consistency check".to_owned(),
            });
        }
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn detach_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        asset_id: AssetId,
        slot: PersonaMediaSlot,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let operation = persona_detach_media_operation(id, expected_revision, asset_id, slot);
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        if let Some(persona) =
            replay_persona_update_change(&tx, operation, id, expected_revision, now)?
        {
            tx.commit().map_err(db_error)?;
            return Ok(persona);
        }
        let current = ensure_active(&tx, id, expected_revision)?;
        let before = current.clone();
        if !current
            .media
            .links
            .iter()
            .any(|link| link.asset_id == asset_id && link.slot == slot)
        {
            return Err(RepositoryError::NotFound);
        }
        let mut links: Vec<_> = current
            .media
            .links
            .into_iter()
            .filter(|link| !(link.asset_id == asset_id && link.slot == slot))
            .collect();
        for (ordinal, link) in links
            .iter_mut()
            .filter(|link| link.slot == PersonaMediaSlot::DesignReference)
            .enumerate()
        {
            link.ordinal = u32::try_from(ordinal).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?;
        }
        let media = normalize_media(PersonaMedia { links })?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or_else(|| RepositoryError::Storage {
                reason: "detach media: a required value is missing".to_owned(),
            })?;
        let request = persona_update_change(&before, &persona)?;
        record_persona_media_assets(
            &tx,
            persona.media.links.iter().map(|link| link.asset_id),
            now,
        )?;
        let admission =
            record_local_change_in(&tx, operation, &request, now).map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "detach media: stored data failed a consistency check".to_owned(),
            });
        }
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn reorder_media(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        slot: PersonaMediaSlot,
        asset_id: AssetId,
        target_ordinal: u32,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let operation =
            persona_reorder_media_operation(id, expected_revision, slot, asset_id, target_ordinal);
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        if let Some(persona) =
            replay_persona_update_change(&tx, operation, id, expected_revision, now)?
        {
            tx.commit().map_err(db_error)?;
            return Ok(persona);
        }
        let current = ensure_active(&tx, id, expected_revision)?;
        let before = current.clone();
        let mut selected: Vec<_> = current
            .media
            .links
            .iter()
            .filter(|link| link.slot == slot)
            .cloned()
            .collect();
        let Some(index) = selected.iter().position(|link| link.asset_id == asset_id) else {
            return Err(RepositoryError::NotFound);
        };
        let target = usize::try_from(target_ordinal).map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        if target >= selected.len() {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue {
                    field: "persona.media.ordinal",
                },
            ));
        }
        let value = selected.remove(index);
        selected.insert(target, value);
        for (ordinal, link) in selected.iter_mut().enumerate() {
            link.ordinal = u32::try_from(ordinal).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?;
        }
        let mut links: Vec<_> = current
            .media
            .links
            .into_iter()
            .filter(|link| link.slot != slot)
            .collect();
        links.extend(selected);
        let media = normalize_media(PersonaMedia { links })?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or_else(|| RepositoryError::Storage {
                reason: "reorder media: a required value is missing".to_owned(),
            })?;
        let request = persona_update_change(&before, &persona)?;
        record_persona_media_assets(
            &tx,
            persona.media.links.iter().map(|link| link.asset_id),
            now,
        )?;
        let admission =
            record_local_change_in(&tx, operation, &request, now).map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "reorder media: stored data failed a consistency check".to_owned(),
            });
        }
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn set_default(
        &self,
        id: PersonaId,
        expected_default_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PersonaDefaultState, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let state = set_default_in(&tx, id, expected_default_revision, now)?;
        tx.commit().map_err(db_error)?;
        Ok(state)
    }

    fn clear_default(
        &self,
        expected_default_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PersonaDefaultState, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let state = clear_default_in(&tx, expected_default_revision, now)?;
        tx.commit().map_err(db_error)?;
        Ok(state)
    }

    fn archive(
        &self,
        request: PersonaArchiveRequest,
    ) -> Result<PersonaArchiveResult, RepositoryError> {
        request.validate()?;
        let operation = persona_archive_operation(
            request.persona_id,
            request.expected_persona_revision,
            request.expected_default_revision,
        );
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(request.persona_id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        let default_operation = request
            .expected_default_revision
            .map(|expected_default| {
                entity_scoped_operation(
                    &tx,
                    &persona_default_sync_entity().map_err(|error| RepositoryError::Storage {
                        reason: error.to_string(),
                    })?,
                    persona_archive_default_operation(
                        request.persona_id,
                        request.expected_persona_revision,
                        expected_default,
                    ),
                )
                .map_err(sync_error)
            })
            .transpose()?;
        if let Some(persona) = replay_persona_update_change(
            &tx,
            operation,
            request.persona_id,
            request.expected_persona_revision,
            request.now,
        )? {
            let default = if let (Some(expected_default), Some(default_operation)) =
                (request.expected_default_revision, default_operation)
            {
                replay_persona_default_change(
                    &tx,
                    default_operation,
                    expected_default,
                    request.now,
                )?
                .unwrap_or(read_default(&tx).map_err(db_error)?)
            } else {
                read_default(&tx).map_err(db_error)?
            };
            let result = PersonaArchiveResult { persona, default };
            result.validate()?;
            tx.commit().map_err(db_error)?;
            return Ok(result);
        }
        let current = load_persona(&tx, request.persona_id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if current.revision != request.expected_persona_revision {
            return Err(RepositoryError::StaleRevision {
                expected: request.expected_persona_revision,
                actual: current.revision,
            });
        }
        if current.status != LifecycleStatus::Active {
            return Err(RepositoryError::Archived);
        }
        let before_default = read_default(&tx).map_err(db_error)?;
        let is_default = before_default.persona_id == Some(request.persona_id);
        if is_default {
            let expected_default = request
                .expected_default_revision
                .ok_or(RepositoryError::MissingDefaultRevision)?;
            if before_default.revision != expected_default {
                return Err(RepositoryError::StaleRevision {
                    expected: expected_default,
                    actual: before_default.revision,
                });
            }
        }
        let next_persona = request
            .expected_persona_revision
            .next()
            .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
        let changed = tx
            .execute(
                "UPDATE personas SET status='archived',revision=?2,updated_at=?3 WHERE id=?1 AND revision=?4",
                params![
                    request.persona_id.to_string(),
                    sql_u64(next_persona.get())?,
                    request.now.get(),
                    sql_u64(request.expected_persona_revision.get())?
                ],
            )
            .map_err(db_error)?;
        if changed == 0 {
            let actual = persona_row(&tx, request.persona_id)
                .map_err(db_error)?
                .ok_or(RepositoryError::NotFound)?
                .revision;
            return Err(RepositoryError::StaleRevision {
                expected: request.expected_persona_revision,
                actual,
            });
        }
        if is_default {
            let expected_default = request
                .expected_default_revision
                .ok_or(RepositoryError::MissingDefaultRevision)?;
            let next_default = expected_default
                .next()
                .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
            let changed = tx
                .execute(
                    "UPDATE persona_defaults SET default_persona_id=NULL,revision=?1,updated_at=?2 WHERE id=1 AND revision=?3",
                    params![
                        sql_u64(next_default.get())?,
                        request.now.get(),
                        sql_u64(expected_default.get())?
                    ],
                )
                .map_err(db_error)?;
            if changed == 0 {
                let actual = read_default(&tx).map_err(db_error)?.revision;
                return Err(RepositoryError::StaleRevision {
                    expected: expected_default,
                    actual,
                });
            }
        }
        let persona = load_persona(&tx, request.persona_id)
            .map_err(db_error)?
            .ok_or_else(|| RepositoryError::Storage {
                reason: "archive: a required value is missing".to_owned(),
            })?;
        let default = read_default(&tx).map_err(db_error)?;
        let result = PersonaArchiveResult { persona, default };
        result.validate()?;
        let persona_change = persona_update_change(&current, &result.persona)?;
        record_persona_media_assets(
            &tx,
            result.persona.media.links.iter().map(|link| link.asset_id),
            request.now,
        )?;
        let admission = record_local_change_in(&tx, operation, &persona_change, request.now)
            .map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "archive: stored data failed a consistency check".to_owned(),
            });
        }
        if is_default {
            let default_operation =
                default_operation.ok_or(RepositoryError::MissingDefaultRevision)?;
            let default_change = persona_default_update_change(&before_default, &result.default)?;
            let admission =
                record_local_change_in(&tx, default_operation, &default_change, request.now)
                    .map_err(sync_error)?;
            if !admission.created {
                return Err(RepositoryError::Storage {
                    reason: "archive: stored data failed a consistency check".to_owned(),
                });
            }
        }
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn restore(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let operation = persona_restore_operation(id, expected_revision);
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let operation = entity_scoped_operation(
            &tx,
            &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?,
            operation,
        )
        .map_err(sync_error)?;
        if let Some(persona) =
            replay_persona_update_change(&tx, operation, id, expected_revision, now)?
        {
            tx.commit().map_err(db_error)?;
            return Ok(persona);
        }
        let current = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if current.revision != expected_revision {
            return Err(RepositoryError::StaleRevision {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let next = expected_revision
            .next()
            .map_err(|error| RepositoryError::Storage { reason: error.to_string() })?;
        let changed = tx
            .execute(
            "UPDATE personas SET status='active',revision=?2,updated_at=?3 WHERE id=?1 AND revision=?4",
            params![
                id.to_string(),
                sql_u64(next.get())?,
                now.get(),
                sql_u64(expected_revision.get())?
            ],
            )
            .map_err(db_error)?;
        if changed == 0 {
            let actual = persona_row(&tx, id)
                .map_err(db_error)?
                .ok_or(RepositoryError::NotFound)?
                .revision;
            return Err(RepositoryError::StaleRevision {
                expected: expected_revision,
                actual,
            });
        }
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or_else(|| RepositoryError::Storage {
                reason: "restore: a required value is missing".to_owned(),
            })?;
        let change = persona_update_change(&current, &persona)?;
        record_persona_media_assets(
            &tx,
            persona.media.links.iter().map(|link| link.asset_id),
            now,
        )?;
        let admission = record_local_change_in(&tx, operation, &change, now).map_err(sync_error)?;
        if !admission.created {
            return Err(RepositoryError::Storage {
                reason: "restore: stored data failed a consistency check".to_owned(),
            });
        }
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }
}

impl PersonaDependencyReader for Database {
    fn dependencies(&self, id: PersonaId) -> Result<DependencyReport, RepositoryError> {
        let mut connection = self.connection().map_err(|error| RepositoryError::Storage {
            reason: error.to_string(),
        })?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        let default = read_default(&tx).map_err(db_error)?;
        let references = if default.persona_id == Some(id) {
            vec![DependencyReference::PersonaDefault]
        } else {
            Vec::new()
        };
        let mut references = references;
        let mut group_stmt = tx
            .prepare(
                "SELECT id FROM groups WHERE persona_selection_kind='explicit' AND persona_id=?1 ORDER BY id",
            )
            .map_err(db_error)?;
        for row in group_stmt
            .query_map([id.to_string()], |row| {
                Ok(DependencyReference::PersonaInGroup {
                    group_id: row
                        .get::<_, String>(0)?
                        .parse()
                        .map_err(|_| invalid("persona.group"))?,
                })
            })
            .map_err(db_error)?
        {
            references.push(row.map_err(db_error)?);
        }
        drop(group_stmt);
        let report = DependencyReport { references };
        tx.commit().map_err(db_error)?;
        Ok(report)
    }
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn set_library_persona_media(
        &self,
        id: PersonaId,
        expected: Revision,
        slot: PersonaMediaSlot,
        ordinal: u32,
        asset: Option<AssetId>,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let current = ensure_active(self.transaction, id, expected)?;
        let mut media = current.media.clone();
        let count = media.links.iter().filter(|link| link.slot == slot).count();
        if (slot != PersonaMediaSlot::DesignReference && ordinal != 0)
            || usize::try_from(ordinal).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })? > count
            || (slot == PersonaMediaSlot::DesignReference
                && asset.is_none()
                && usize::try_from(ordinal).map_err(|error| RepositoryError::Storage {
                    reason: error.to_string(),
                })? == count)
        {
            return Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::InvalidValue { field: "ordinal" },
            ));
        }
        media
            .links
            .retain(|link| !(link.slot == slot && link.ordinal == ordinal));
        match asset {
            Some(asset_id) => {
                image_assets(self.transaction, [asset_id])?;
                media.links.push(PersonaMediaLink {
                    asset_id,
                    slot,
                    ordinal,
                });
            }
            None => {
                for link in &mut media.links {
                    if link.slot == slot && link.ordinal > ordinal {
                        link.ordinal -= 1;
                    }
                }
            }
        }
        self.save_persona(
            Some(id),
            Some(expected),
            PersonaDraftUpdate {
                title: current.title,
                description: current.description,
                nickname: current.nickname,
                design_description: current.design_description,
                avatar_crop: current.avatar_crop,
                image_recommendation: current.image_recommendation,
            },
            media,
            now,
        )
    }

    pub fn set_library_default_persona(
        &self,
        id: Option<PersonaId>,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PersonaDefaultState, RepositoryError> {
        let current = read_default(self.transaction).map_err(db_error)?;
        if current.revision != expected_revision {
            return Err(RepositoryError::StaleRevision {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        match id {
            Some(id) => set_default_in(self.transaction, id, expected_revision, now),
            None => clear_default_in(self.transaction, expected_revision, now),
        }
    }

    pub fn save_persona(
        &self,
        id: Option<PersonaId>,
        expected_revision: Option<Revision>,
        draft: PersonaDraftUpdate,
        media: PersonaMedia,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        draft.validate()?;
        let media = normalize_media(media)?;
        image_assets(
            self.transaction,
            media.links.iter().map(|link| link.asset_id),
        )?;
        match (id, expected_revision) {
            (None, None) => {
                let persona = Persona {
                    id: PersonaId::new(),
                    status: LifecycleStatus::Active,
                    title: draft.title,
                    description: draft.description,
                    nickname: draft.nickname,
                    design_description: draft.design_description,
                    avatar_crop: draft.avatar_crop,
                    image_recommendation: draft.image_recommendation,
                    media,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                };
                create_persona_in(self.transaction, persona)
            }
            (Some(id), Some(expected)) => {
                let before = ensure_active(self.transaction, id, expected)?;
                revise_persona(self.transaction, id, expected, draft, now)?;
                replace_media(self.transaction, id, &media)?;
                let after = load_persona(self.transaction, id)
                    .map_err(db_error)?
                    .ok_or_else(|| RepositoryError::Storage {
                        reason: "save persona: a required value is missing".to_owned(),
                    })?;
                after.validate()?;
                record_persona_media_assets(
                    self.transaction,
                    after.media.links.iter().map(|link| link.asset_id),
                    now,
                )?;
                let operation = entity_scoped_operation(
                    self.transaction,
                    &persona_sync_entity(id).map_err(|error| RepositoryError::Storage {
                        reason: error.to_string(),
                    })?,
                    persona_revise_operation(id, expected),
                )
                .map_err(sync_error)?;
                let change = persona_update_change(&before, &after)?;
                if !record_local_change_in(self.transaction, operation, &change, now)
                    .map_err(sync_error)?
                    .created
                {
                    return Err(RepositoryError::Storage {
                        reason: "save persona: stored data failed a consistency check".to_owned(),
                    });
                }
                Ok(after)
            }
            _ => Err(RepositoryError::Invalid(
                lettuce_characters::ValidationError::Invariant {
                    field: "persona.expected_revision",
                },
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_characters::{
        Crop, PersonaDraftUpdate, PersonaMedia, PersonaMediaLink, PersonaMediaSlot,
        PersonaRepository,
    };
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_types::{ContentHash, MediaBlobId, ModelArtifactId, PageLimit};

    fn image_asset(database: &Database, byte: char) -> AssetId {
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(format!("{:02x}", byte as u8).repeat(32))
                .expect("hash"),
            kind: MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: 1,
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let blob = MediaBlobRepository::register(database, blob).expect("blob");
        let blob = MediaBlobRepository::finalize_staged_to_ready(
            database,
            blob.id,
            TimestampMillis::new(1),
        )
        .expect("ready blob");
        let asset = MediaAsset::new(
            AssetId::new(),
            blob.id,
            AssetKind::Illustration,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        )
        .expect("asset");
        MediaAssetRepository::create(database, asset)
            .expect("asset create")
            .id
    }

    fn audio_asset(database: &Database, byte: char) -> AssetId {
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(format!("{:02x}", byte as u8).repeat(32))
                .expect("hash"),
            kind: MediaKind::Audio,
            mime_type: "audio/mpeg".into(),
            byte_size: 1,
            width: None,
            height: None,
            duration_ms: Some(1),
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let blob = MediaBlobRepository::register(database, blob).expect("blob");
        let blob = MediaBlobRepository::finalize_staged_to_ready(
            database,
            blob.id,
            TimestampMillis::new(1),
        )
        .expect("ready blob");
        let asset = MediaAsset::new(
            AssetId::new(),
            blob.id,
            AssetKind::OtherAudio,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        )
        .expect("asset");
        MediaAssetRepository::create(database, asset)
            .expect("asset create")
            .id
    }

    #[test]
    fn a_failed_persona_file_import_leaves_nothing_and_a_retry_writes_one_persona() {
        let database = Database::open_in_memory().expect("database");
        let lorebook = lettuce_context::LorebookRepository::create(
            &database,
            lettuce_context::LorebookMetadataDraft {
                name: "Harbour".into(),
                detection_policy: lettuce_context::DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: lettuce_context::LorebookBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("lorebook");
        let count = |database: &Database| {
            PersonaRepository::search(database, lettuce_characters::PersonaSearch { include_archived: true, search_nickname: false, ..Default::default() }, PageRequest {
                    cursor: None,
                    limit: PageLimit::new(10),
                })
            .expect("personas")
            .items
            .len()
        };
        let persona = Persona::new(
            PersonaId::new(),
            "Wanderer".into(),
            String::new(),
            TimestampMillis::new(5),
        )
        .expect("a persona may have no description");
        let avatar = image_asset(&database, 'a');
        let mut with_avatar = persona.clone();
        with_avatar.media.links.push(PersonaMediaLink {
            asset_id: avatar,
            slot: PersonaMediaSlot::Avatar,
            ordinal: 0,
        });
        let failing = PersonaFileImport {
            persona: with_avatar,
            lorebook_ids: vec![lorebook.book.id, lettuce_types::LorebookId::new()],
            disabled_lorebook_ids: Vec::new(),
            make_default: true,
        };
        assert_eq!(
            database.import_persona_file(&failing),
            Err(RepositoryError::NotFound)
        );
        assert_eq!(count(&database), 0);
        let blob_id = database
            .discard_unlinked_asset(avatar)
            .expect("discard")
            .expect("the unlinked avatar is deleted");
        assert_eq!(MediaAssetRepository::get(&database, avatar), Ok(None));
        assert_eq!(
            database.release_unused_blob(blob_id, TimestampMillis::new(6)),
            Ok(true)
        );
        assert_eq!(
            MediaBlobRepository::get(&database, blob_id)
                .expect("blob")
                .expect("blob row")
                .state,
            BlobState::Missing
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default")
                .state
                .persona_id,
            None
        );
        let imported = database
            .import_persona_file(&PersonaFileImport {
                persona: Persona {
                    id: PersonaId::new(),
                    ..persona
                },
                lorebook_ids: vec![lorebook.book.id],
                disabled_lorebook_ids: Vec::new(),
                make_default: true,
            })
            .expect("retry");
        assert_eq!(count(&database), 1);
        assert_eq!(
            lettuce_context::PersonaLorebookBindingRepository::list_persona_bindings(
                &database,
                imported.id
            )
            .expect("bindings")
            .len(),
            1
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default")
                .state
                .persona_id,
            Some(imported.id)
        );
        let linked = image_asset(&database, 'b');
        let mut linked_persona = Persona::new(
            PersonaId::new(),
            "Linked".into(),
            String::new(),
            TimestampMillis::new(7),
        )
        .expect("persona");
        linked_persona.media.links.push(PersonaMediaLink {
            asset_id: linked,
            slot: PersonaMediaSlot::Avatar,
            ordinal: 0,
        });
        database
            .import_persona_file(&PersonaFileImport {
                persona: linked_persona,
                lorebook_ids: Vec::new(),
                disabled_lorebook_ids: Vec::new(),
                make_default: false,
            })
            .expect("linked avatar import");
        assert_eq!(database.discard_unlinked_asset(linked), Ok(None));
        assert!(
            MediaAssetRepository::get(&database, linked)
                .expect("asset")
                .is_some()
        );
    }

    fn persona(id: PersonaId, avatar: AssetId, reference: AssetId) -> Persona {
        let mut persona = Persona::new(
            id,
            "Élodie".into(),
            "A meticulous writer".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        persona.nickname = Some("Elo".into());
        persona.design_description = Some("Ink and paper".into());
        persona.avatar_crop = Some(Crop::new(0.2, 0.3, 1.1).expect("crop"));
        persona.image_recommendation = Some(ImageRecommendation {
            artifact_id: Some(ModelArtifactId::new()),
            unresolved_legacy_name: None,
            strength: 0.8,
        });
        persona.media = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id: avatar,
                    slot: PersonaMediaSlot::Avatar,
                    ordinal: 0,
                },
                PersonaMediaLink {
                    asset_id: reference,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        };
        persona.validate().expect("fixture persona");
        persona
    }

    #[test]
    fn persona_create_and_revise_commit_canonical_changes_once() {
        let database = Database::open_in_memory().expect("database");
        let value = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "A careful writer".into(),
            TimestampMillis::new(10),
        )
        .expect("persona");
        let created = PersonaRepository::create(&database, value.clone()).expect("create");
        assert_eq!(
            PersonaRepository::create(&database, value).expect("create replay"),
            created
        );
        let create_change = load_local_change_in(
            &database.connection().expect("connection"),
            persona_create_operation(created.id),
        )
        .expect("create change")
        .expect("create journal row");
        assert_eq!(create_change.origin_sequence(), 1);
        assert_eq!(create_change.operation(), ChangeOperation::Insert);
        assert_eq!(
            serde_json::from_slice::<Persona>(
                create_change.payload().expect("create payload").bytes()
            )
            .expect("decode create"),
            created
        );

        let draft = PersonaDraftUpdate {
            title: "Editor".into(),
            description: "A careful editor".into(),
            nickname: Some("Ed".into()),
            design_description: Some("Blue ink".into()),
            avatar_crop: None,
            image_recommendation: None,
        };
        let revised = PersonaRepository::revise(
            &database,
            created.id,
            created.revision,
            draft.clone(),
            TimestampMillis::new(20),
        )
        .expect("revise");
        assert_eq!(
            PersonaRepository::revise(
                &database,
                created.id,
                created.revision,
                draft,
                TimestampMillis::new(20),
            )
            .expect("revise replay"),
            revised
        );
        let revise_change = load_local_change_in(
            &database.connection().expect("connection"),
            persona_revise_operation(created.id, created.revision),
        )
        .expect("revise change")
        .expect("revise journal row");
        assert_eq!(revise_change.origin_sequence(), 2);
        assert_eq!(revise_change.operation(), ChangeOperation::Update);
        assert_eq!(
            revise_change.base_revision(),
            Some(
                create_change
                    .payload()
                    .expect("create payload")
                    .content_hash()
            )
        );
        assert_eq!(
            serde_json::from_slice::<Persona>(
                revise_change.payload().expect("revise payload").bytes()
            )
            .expect("decode revise"),
            revised
        );
    }

    #[test]
    fn persona_sync_conflicts_and_failed_domain_writes_add_no_change() {
        let database = Database::open_in_memory().expect("database");
        let id = PersonaId::new();
        let original = Persona::new(
            id,
            "Writer".into(),
            "A careful writer".into(),
            TimestampMillis::new(10),
        )
        .expect("persona");
        let created = PersonaRepository::create(&database, original).expect("create");
        let changed = Persona::new(
            id,
            "Changed".into(),
            "Different input".into(),
            TimestampMillis::new(10),
        )
        .expect("changed persona");
        assert_eq!(
            PersonaRepository::create(&database, changed),
            Err(RepositoryError::AlreadyExists)
        );
        let stale = PersonaRepository::revise(
            &database,
            id,
            created.revision,
            PersonaDraftUpdate {
                title: "First".into(),
                description: "First revision".into(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
            },
            TimestampMillis::new(20),
        )
        .expect("first revision");
        assert!(matches!(
            PersonaRepository::revise(
                &database,
                id,
                created.revision,
                PersonaDraftUpdate {
                    title: "Second".into(),
                    description: "Changed retry".into(),
                    nickname: None,
                    design_description: None,
                    avatar_crop: None,
                    image_recommendation: None,
                },
                TimestampMillis::new(20),
            ),
            Err(RepositoryError::StaleRevision { actual, .. }) if actual == stale.revision
        ));

        let mut invalid = Persona::new(
            PersonaId::new(),
            "Invalid media".into(),
            "References a missing asset".into(),
            TimestampMillis::new(30),
        )
        .expect("persona");
        invalid.media.links.push(PersonaMediaLink {
            asset_id: AssetId::new(),
            slot: PersonaMediaSlot::Avatar,
            ordinal: 0,
        });
        assert!(PersonaRepository::create(&database, invalid).is_err());
        let connection = database.connection().expect("connection");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM sync_changes", [], |row| row
                    .get::<_, i64>(0))
                .expect("change count"),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT origin_sequence FROM sync_local_state WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("sequence"),
            2
        );
    }

    #[test]
    fn persona_media_mutations_commit_canonical_changes_once_and_roll_back_together() {
        let database = Database::open_in_memory().expect("database");
        let first = image_asset(&database, 'f');
        let second = image_asset(&database, 'g');
        let third = image_asset(&database, 'h');
        let created = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "Artist".into(),
                "Collects visual references".into(),
                TimestampMillis::new(10),
            )
            .expect("persona"),
        )
        .expect("create");

        let replacement = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id: second,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 1,
                },
                PersonaMediaLink {
                    asset_id: first,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        };
        let updated = PersonaRepository::update_media(
            &database,
            created.id,
            created.revision,
            replacement.clone(),
            TimestampMillis::new(20),
        )
        .expect("update media");
        assert_eq!(
            PersonaRepository::update_media(
                &database,
                created.id,
                created.revision,
                replacement.clone(),
                TimestampMillis::new(20),
            )
            .expect("update replay"),
            updated
        );

        let attached_link = PersonaMediaLink {
            asset_id: third,
            slot: PersonaMediaSlot::DesignReference,
            ordinal: 1,
        };
        let attached = PersonaRepository::attach_media(
            &database,
            updated.id,
            updated.revision,
            attached_link.clone(),
            TimestampMillis::new(30),
        )
        .expect("attach");
        assert_eq!(
            PersonaRepository::attach_media(
                &database,
                updated.id,
                updated.revision,
                attached_link.clone(),
                TimestampMillis::new(30),
            )
            .expect("attach replay"),
            attached
        );

        let reordered = PersonaRepository::reorder_media(
            &database,
            attached.id,
            attached.revision,
            PersonaMediaSlot::DesignReference,
            third,
            0,
            TimestampMillis::new(40),
        )
        .expect("reorder");
        assert_eq!(
            PersonaRepository::reorder_media(
                &database,
                attached.id,
                attached.revision,
                PersonaMediaSlot::DesignReference,
                third,
                0,
                TimestampMillis::new(40),
            )
            .expect("reorder replay"),
            reordered
        );

        let detached = PersonaRepository::detach_media(
            &database,
            reordered.id,
            reordered.revision,
            first,
            PersonaMediaSlot::DesignReference,
            TimestampMillis::new(50),
        )
        .expect("detach");
        assert_eq!(
            PersonaRepository::detach_media(
                &database,
                reordered.id,
                reordered.revision,
                first,
                PersonaMediaSlot::DesignReference,
                TimestampMillis::new(50),
            )
            .expect("detach replay"),
            detached
        );

        let operations = [
            persona_update_media_operation(created.id, created.revision, &replacement),
            persona_attach_media_operation(updated.id, updated.revision, &attached_link),
            persona_reorder_media_operation(
                attached.id,
                attached.revision,
                PersonaMediaSlot::DesignReference,
                third,
                0,
            ),
            persona_detach_media_operation(
                reordered.id,
                reordered.revision,
                first,
                PersonaMediaSlot::DesignReference,
            ),
        ];
        let snapshots = [&updated, &attached, &reordered, &detached];
        let bases = [&created, &updated, &attached, &reordered];
        let connection = database.connection().expect("connection");
        for (index, ((operation, snapshot), base)) in
            operations.into_iter().zip(snapshots).zip(bases).enumerate()
        {
            let change = load_local_change_in(&connection, operation)
                .expect("change read")
                .expect("media change");
            assert_eq!(change.origin_sequence(), [4, 6, 7, 8][index]);
            assert_eq!(change.operation(), ChangeOperation::Update);
            assert_eq!(
                change.base_revision(),
                Some(
                    canonical_persona_payload(base)
                        .expect("base payload")
                        .content_hash()
                )
            );
            assert_eq!(
                serde_json::from_slice::<Persona>(change.payload().expect("media payload").bytes())
                    .expect("decode media payload"),
                *snapshot
            );
        }
        drop(connection);

        let missing_media = PersonaMedia {
            links: vec![PersonaMediaLink {
                asset_id: AssetId::new(),
                slot: PersonaMediaSlot::Avatar,
                ordinal: 0,
            }],
        };
        assert!(
            PersonaRepository::update_media(
                &database,
                detached.id,
                detached.revision,
                missing_media,
                TimestampMillis::new(60),
            )
            .is_err()
        );
        assert!(
            PersonaRepository::attach_media(
                &database,
                detached.id,
                detached.revision,
                PersonaMediaLink {
                    asset_id: third,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
                TimestampMillis::new(61),
            )
            .is_err()
        );
        assert!(matches!(
            PersonaRepository::detach_media(
                &database,
                detached.id,
                created.revision,
                third,
                PersonaMediaSlot::DesignReference,
                TimestampMillis::new(62),
            ),
            Err(RepositoryError::StaleRevision { .. })
        ));

        let failed_operation = persona_update_media_operation(
            detached.id,
            detached.revision,
            &PersonaMedia::default(),
        );
        let connection = database.connection().expect("connection");
        connection
            .execute_batch(&format!(
                "CREATE TEMP TRIGGER reject_persona_media_change
                 BEFORE INSERT ON sync_changes
                 WHEN NEW.operation_id = '{}'
                 BEGIN SELECT RAISE(ABORT, 'reject media journal'); END;",
                failed_operation
            ))
            .expect("failure trigger");
        drop(connection);
        assert!(
            PersonaRepository::update_media(
                &database,
                detached.id,
                detached.revision,
                PersonaMedia::default(),
                TimestampMillis::new(70),
            )
            .is_err()
        );
        assert_eq!(
            PersonaRepository::get(&database, detached.id).expect("persona after rollback"),
            Some(detached)
        );
        let connection = database.connection().expect("connection");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM sync_changes", [], |row| row
                    .get::<_, i64>(0))
                .expect("change count"),
            8
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT origin_sequence FROM sync_local_state WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("sequence"),
            8
        );
    }

    #[test]
    fn a_default_archived_by_sync_reads_as_no_default() {
        let database = Database::open_in_memory().expect("database");
        let stored = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "Synced".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        PersonaRepository::set_default(
            &database,
            stored.id,
            Revision::INITIAL,
            TimestampMillis::new(2),
        )
        .expect("set default");
        let mut archived = stored.clone();
        archived.status = LifecycleStatus::Archived;
        archived.revision = Revision::INITIAL.next().expect("revision");
        archived.updated_at = TimestampMillis::new(3);
        {
            let mut connection = database.connection().expect("connection");
            let tx = connection.transaction().expect("transaction");
            apply_synced_persona(&tx, archived).expect("archive through sync");
            tx.commit().expect("commit");
        }
        let snapshot = PersonaRepository::get_default_snapshot(&database)
            .expect("legacy persona_default_get finds no default once the persona is gone");
        assert_eq!(snapshot.state.persona_id, None);
        assert_eq!(snapshot.persona, None);
        let restored = PersonaRepository::set_default(
            &database,
            stored.id,
            snapshot.state.revision,
            TimestampMillis::new(4),
        );
        assert!(restored.is_err());
    }

    #[test]
    fn persona_round_trip_default_archive_restore_and_dependencies() {
        let database = Database::open_in_memory().expect("database");
        let avatar = image_asset(&database, 'a');
        let reference = image_asset(&database, 'b');
        let original = persona(PersonaId::new(), avatar, reference);
        let stored = PersonaRepository::create(&database, original.clone()).expect("create");
        assert_eq!(stored, original);
        assert_eq!(
            PersonaRepository::get(&database, stored.id).expect("get"),
            Some(original)
        );

        let default = PersonaRepository::set_default(
            &database,
            stored.id,
            Revision::INITIAL,
            TimestampMillis::new(2),
        )
        .expect("set default");
        assert_eq!(default.persona_id, Some(stored.id));
        let snapshot = PersonaRepository::get_default_snapshot(&database).expect("snapshot");
        assert_eq!(
            snapshot.persona.as_ref().map(|value| value.id),
            Some(stored.id)
        );
        assert_eq!(
            PersonaDependencyReader::dependencies(&database, stored.id)
                .expect("dependencies")
                .references,
            vec![DependencyReference::PersonaDefault]
        );

        let archive_request = PersonaArchiveRequest {
            persona_id: stored.id,
            expected_persona_revision: stored.revision,
            expected_default_revision: Some(default.revision),
            now: TimestampMillis::new(3),
        };
        let result = PersonaRepository::archive(&database, archive_request).expect("archive");
        assert_eq!(
            PersonaRepository::archive(&database, archive_request).expect("archive replay"),
            result
        );
        assert_eq!(result.persona.status, LifecycleStatus::Archived);
        assert_eq!(result.default.persona_id, None);
        let restored = PersonaRepository::restore(
            &database,
            stored.id,
            result.persona.revision,
            TimestampMillis::new(4),
        )
        .expect("restore");
        assert_eq!(
            PersonaRepository::restore(
                &database,
                stored.id,
                result.persona.revision,
                TimestampMillis::new(4),
            )
            .expect("restore replay"),
            restored
        );
        assert_eq!(restored.status, LifecycleStatus::Active);
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("empty default")
                .state
                .persona_id,
            None
        );
        let connection = database.connection().expect("connection");
        let archive_change = load_local_change_in(
            &connection,
            persona_archive_operation(stored.id, stored.revision, Some(default.revision)),
        )
        .expect("archive change read")
        .expect("archive change");
        let default_change = load_local_change_in(
            &connection,
            persona_archive_default_operation(stored.id, stored.revision, default.revision),
        )
        .expect("default change read")
        .expect("archive default change");
        let restore_change = load_local_change_in(
            &connection,
            persona_restore_operation(stored.id, result.persona.revision),
        )
        .expect("restore change read")
        .expect("restore change");
        assert_eq!(archive_change.origin_sequence(), 5);
        assert_eq!(default_change.origin_sequence(), 6);
        assert_eq!(restore_change.origin_sequence(), 7);
        assert_eq!(
            archive_change.base_revision(),
            Some(
                canonical_persona_payload(&stored)
                    .expect("archive base")
                    .content_hash()
            )
        );
        assert_eq!(
            default_change.base_revision(),
            Some(
                canonical_persona_default_payload(&default)
                    .expect("default base")
                    .content_hash()
            )
        );
        assert_eq!(
            restore_change.base_revision(),
            Some(
                canonical_persona_payload(&result.persona)
                    .expect("restore base")
                    .content_hash()
            )
        );
        assert_eq!(
            serde_json::from_slice::<Persona>(
                restore_change.payload().expect("restore payload").bytes()
            )
            .expect("decode restore"),
            restored
        );
    }

    #[test]
    fn persona_media_detach_and_reorder_are_canonical() {
        let database = Database::open_in_memory().expect("database");
        let first = image_asset(&database, 'c');
        let second = image_asset(&database, 'd');
        let third = image_asset(&database, 'e');
        let mut value = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        value.media = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id: second,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 1,
                },
                PersonaMediaLink {
                    asset_id: first,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        };
        let value = PersonaRepository::create(&database, value).expect("create");
        assert_eq!(value.media.links[0].asset_id, first);
        assert_eq!(value.media.links[1].asset_id, second);
        let value = PersonaRepository::attach_media(
            &database,
            value.id,
            value.revision,
            PersonaMediaLink {
                asset_id: third,
                slot: PersonaMediaSlot::DesignReference,
                ordinal: 1,
            },
            TimestampMillis::new(2),
        )
        .expect("attach");
        let value = PersonaRepository::reorder_media(
            &database,
            value.id,
            value.revision,
            PersonaMediaSlot::DesignReference,
            third,
            0,
            TimestampMillis::new(3),
        )
        .expect("reorder");
        assert_eq!(value.media.links[0].asset_id, third);
        let value = PersonaRepository::detach_media(
            &database,
            value.id,
            value.revision,
            third,
            PersonaMediaSlot::DesignReference,
            TimestampMillis::new(4),
        )
        .expect("detach");
        assert_eq!(value.media.links[0].ordinal, 0);
        assert_eq!(value.media.links[1].ordinal, 1);

        let reversed = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id: second,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 1,
                },
                PersonaMediaLink {
                    asset_id: first,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        };
        let value = PersonaRepository::update_media(
            &database,
            value.id,
            value.revision,
            reversed,
            TimestampMillis::new(5),
        )
        .expect("update media");
        assert_eq!(value.media.links[0].asset_id, first);
        assert_eq!(value.media.links[1].asset_id, second);
    }

    #[test]
    fn persona_read_rejects_corrupt_normalization_and_payload_version() {
        let database = Database::open_in_memory().expect("database");
        let value = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        let id = value.id;
        PersonaRepository::create(&database, value).expect("create");
        let connection = database.connection().expect("lock");
        connection
            .execute(
                "UPDATE personas SET normalized_title='corrupt' WHERE id=?1",
                [id.to_string()],
            )
            .expect("corrupt normalized title");
        drop(connection);
        assert!(matches!(
            PersonaRepository::get(&database, id),
            Err(RepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn unicode_composed_and_decomposed_keys_search_identically() {
        let database = Database::open_in_memory().expect("database");
        let title = "Cafe\u{301} Writer";
        let value = Persona::new(
            PersonaId::new(),
            title.into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        let id = value.id;
        PersonaRepository::create(&database, value).expect("create");
        let page = PersonaRepository::search(
            &database,
            PersonaSearch {
                text: "CAFÉ".into(),
                include_archived: false,
                ..Default::default()
            },
            PageRequest::default(),
        )
        .expect("search");
        assert_eq!(
            page.items.iter().map(|value| value.id).collect::<Vec<_>>(),
            vec![id]
        );
        let connection = database.connection().expect("lock");
        let normalized: String = connection
            .query_row(
                "SELECT normalized_title FROM personas WHERE id=?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .expect("normalized title");
        assert_eq!(normalized, canonical_title(title));
    }

    #[test]
    fn revise_preserves_identity_media_lifecycle_and_timestamps_and_enforces_cas() {
        let database = Database::open_in_memory().expect("database");
        let avatar = image_asset(&database, 'f');
        let reference = image_asset(&database, 'g');
        let original = persona(PersonaId::new(), avatar, reference);
        let stored = PersonaRepository::create(&database, original).expect("create");
        let draft = PersonaDraftUpdate {
            title: "Revised".into(),
            description: "A revised description".into(),
            nickname: Some("Rev".into()),
            design_description: Some("New design".into()),
            avatar_crop: Some(Crop::new(0.4, 0.5, 1.5).expect("crop")),
            image_recommendation: Some(ImageRecommendation {
                artifact_id: None,
                unresolved_legacy_name: Some("legacy-model".into()),
                strength: 1.2,
            }),
        };
        let revised = PersonaRepository::revise(
            &database,
            stored.id,
            stored.revision,
            draft.clone(),
            TimestampMillis::new(2),
        )
        .expect("revise");
        assert_eq!(revised.id, stored.id);
        assert_eq!(revised.media, stored.media);
        assert_eq!(revised.status, LifecycleStatus::Active);
        assert_eq!(revised.created_at, stored.created_at);
        assert_eq!(revised.updated_at, TimestampMillis::new(2));
        assert_eq!(revised.revision, Revision::new(2));
        assert_eq!(revised.title, draft.title);
        assert_eq!(revised.image_recommendation, draft.image_recommendation);
        assert!(matches!(
            PersonaRepository::revise(
                &database,
                stored.id,
                stored.revision,
                draft,
                TimestampMillis::new(3),
            ),
            Err(RepositoryError::StaleRevision { expected, actual })
                if expected == Revision::INITIAL && actual == Revision::new(2)
        ));
        let archived = PersonaRepository::archive(
            &database,
            PersonaArchiveRequest {
                persona_id: revised.id,
                expected_persona_revision: revised.revision,
                expected_default_revision: None,
                now: TimestampMillis::new(3),
            },
        )
        .expect("archive");
        assert!(matches!(
            PersonaRepository::revise(
                &database,
                archived.persona.id,
                archived.persona.revision,
                PersonaDraftUpdate {
                    title: "Nope".into(),
                    description: "Nope".into(),
                    nickname: None,
                    design_description: None,
                    avatar_crop: None,
                    image_recommendation: None,
                },
                TimestampMillis::new(4),
            ),
            Err(RepositoryError::Archived)
        ));
    }

    #[test]
    fn list_search_pagination_archived_filter_escaping_and_cursor_validation() {
        let database = Database::open_in_memory().expect("database");
        let mut ids = Vec::new();
        for (title, byte) in [("100% Writer", 'h'), ("100X Writer", 'i'), ("Other", 'j')] {
            let mut value = Persona::new(
                PersonaId::new(),
                title.into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona");
            value.media = PersonaMedia {
                links: vec![PersonaMediaLink {
                    asset_id: image_asset(&database, byte),
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                }],
            };
            ids.push(
                PersonaRepository::create(&database, value)
                    .expect("create")
                    .id,
            );
        }
        let first = PersonaRepository::search(&database, lettuce_characters::PersonaSearch { include_archived: false, search_nickname: false, ..Default::default() }, PageRequest {
                cursor: None,
                limit: PageLimit::new(2),
            })
        .expect("first page");
        assert_eq!(first.items.len(), 2);
        let second = PersonaRepository::search(&database, lettuce_characters::PersonaSearch { include_archived: false, search_nickname: false, ..Default::default() }, PageRequest {
                cursor: first.next_cursor,
                limit: PageLimit::new(2),
            })
        .expect("second page");
        assert_eq!(second.items.len(), 1);
        let escaped = PersonaRepository::search(
            &database,
            PersonaSearch {
                text: "100%".into(),
                include_archived: false,
                ..Default::default()
            },
            PageRequest::default(),
        )
        .expect("escaped search");
        assert_eq!(escaped.items.len(), 1);
        assert_eq!(escaped.items[0].title, "100% Writer");
        assert!(matches!(
            PersonaRepository::search(&database, lettuce_characters::PersonaSearch { include_archived: false, search_nickname: false, ..Default::default() }, PageRequest {
                    cursor: Some("not-a-cursor".into()),
                    limit: PageLimit::new(2),
                }),
            Err(RepositoryError::Invalid(_))
        ));
        let value = PersonaRepository::get(&database, ids[0])
            .expect("get")
            .expect("persona");
        PersonaRepository::archive(
            &database,
            PersonaArchiveRequest {
                persona_id: value.id,
                expected_persona_revision: value.revision,
                expected_default_revision: None,
                now: TimestampMillis::new(2),
            },
        )
        .expect("archive");
        assert_eq!(
            PersonaRepository::search(
                &database,
                PersonaSearch {
                    text: "100%".into(),
                    include_archived: false,
                    ..Default::default()
                },
                PageRequest::default(),
            )
            .expect("active search")
            .items
            .len(),
            0
        );
        assert_eq!(
            PersonaRepository::search(
                &database,
                PersonaSearch {
                    text: "100%".into(),
                    include_archived: true,
                    ..Default::default()
                },
                PageRequest::default(),
            )
            .expect("all search")
            .items
            .len(),
            1
        );
    }

    fn assert_corrupt_payload_rejected(
        database: &Database,
        id: PersonaId,
        column: &str,
        payload: &str,
        original: &str,
    ) {
        let connection = database.connection().expect("lock");
        connection
            .execute(
                &format!("UPDATE personas SET {column}=?1 WHERE id=?2"),
                params![payload, id.to_string()],
            )
            .expect("corrupt payload");
        drop(connection);
        assert!(matches!(
            PersonaRepository::get(database, id),
            Err(RepositoryError::Invalid(_))
        ));
        let connection = database.connection().expect("lock");
        connection
            .execute(
                &format!("UPDATE personas SET {column}=?1 WHERE id=?2"),
                params![original, id.to_string()],
            )
            .expect("restore payload");
    }

    #[test]
    fn strict_crop_and_recommendation_corruption_is_rejected() {
        let database = Database::open_in_memory().expect("database");
        let value = persona(
            PersonaId::new(),
            image_asset(&database, 'k'),
            image_asset(&database, 'l'),
        );
        let id = value.id;
        PersonaRepository::create(&database, value).expect("create");
        let connection = database.connection().expect("lock");
        let (crop, recommendation): (String, String) = connection
            .query_row(
                "SELECT avatar_crop_json,image_recommendation_json FROM personas WHERE id=?1",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    ))
                },
            )
            .expect("payloads");
        drop(connection);

        for payload in [
            "{",
            r#"{"format_version":1,"value":{"x":0.1,"y":0.2,"scale":1.0,"extra":true}}"#,
            r#"{"format_version":1}"#,
            r#"{"format_version":2,"value":{"x":0.1,"y":0.2,"scale":1.0}}"#,
            r#"{"format_version":1,"value":{"x":0.1,"y":0.2,"scale":0.0}}"#,
        ] {
            assert_corrupt_payload_rejected(&database, id, "avatar_crop_json", payload, &crop);
        }
        for payload in [
            "not-json",
            r#"{"format_version":1,"value":{"artifact_id":null,"unresolved_legacy_name":null,"strength":1.0}}"#,
            r#"{"format_version":1,"value":{"artifact_id":null,"unresolved_legacy_name":"legacy","strength":3.0}}"#,
            r#"{"format_version":1,"value":{"artifact_id":null,"unresolved_legacy_name":"legacy","strength":1.0,"extra":true}}"#,
            r#"{"format_version":2,"value":{"artifact_id":null,"unresolved_legacy_name":"legacy","strength":1.0}}"#,
        ] {
            assert_corrupt_payload_rejected(
                &database,
                id,
                "image_recommendation_json",
                payload,
                &recommendation,
            );
        }
        assert!(
            PersonaRepository::get(&database, id)
                .expect("valid after restore")
                .is_some()
        );
    }

    #[test]
    fn strict_status_revision_times_and_media_order_corruption_is_rejected() {
        let database = Database::open_in_memory().expect("database");
        let first = image_asset(&database, 'm');
        let second = image_asset(&database, 'n');
        let mut value = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        value.media = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id: first,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
                PersonaMediaLink {
                    asset_id: second,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 1,
                },
            ],
        };
        let value = PersonaRepository::create(&database, value).expect("create");
        let connection = database.connection().expect("lock");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .expect("ignore checks");
        connection
            .execute(
                "UPDATE personas SET updated_at=0 WHERE id=?1",
                [value.id.to_string()],
            )
            .expect("corrupt timestamp");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=OFF;")
            .expect("restore checks");
        drop(connection);
        assert!(matches!(
            PersonaRepository::get(&database, value.id),
            Err(RepositoryError::Invalid(_))
        ));
        let connection = database.connection().expect("lock");
        connection
            .execute(
                "UPDATE personas SET updated_at=1 WHERE id=?1",
                [value.id.to_string()],
            )
            .expect("restore timestamp");
        connection
            .execute(
                "UPDATE persona_media SET ordinal=2 WHERE persona_id=?1 AND asset_id=?2",
                params![value.id.to_string(), second.to_string()],
            )
            .expect("corrupt media order");
        drop(connection);
        assert!(matches!(
            PersonaRepository::get(&database, value.id),
            Err(RepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn media_read_rejects_missing_and_wrong_kind_assets_even_with_corrupt_fixture() {
        let database = Database::open_in_memory().expect("database");
        let value = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        let id = value.id;
        PersonaRepository::create(&database, value).expect("create");
        let missing = AssetId::new();
        let connection = database.connection().expect("lock");
        connection
            .execute_batch("PRAGMA foreign_keys=OFF;")
            .expect("disable foreign keys");
        connection
            .execute(
                "INSERT INTO persona_media(persona_id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,'image','design_reference',0)",
                params![id.to_string(), missing.to_string()],
            )
            .expect("missing asset fixture");
        connection
            .execute_batch("PRAGMA foreign_keys=ON;")
            .expect("restore foreign keys");
        drop(connection);
        assert!(matches!(
            PersonaRepository::get(&database, id),
            Err(RepositoryError::Invalid(_))
        ));

        let connection = database.connection().expect("lock");
        connection
            .execute(
                "DELETE FROM persona_media WHERE persona_id=?1",
                [id.to_string()],
            )
            .expect("clear fixture");
        drop(connection);
        let audio = audio_asset(&database, 'q');
        let connection = database.connection().expect("lock");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .expect("ignore checks");
        connection
            .execute(
                "INSERT INTO persona_media(persona_id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,'audio','design_reference',0)",
                params![id.to_string(), audio.to_string()],
            )
            .expect("wrong kind fixture");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=OFF;")
            .expect("restore checks");
        drop(connection);
        assert!(matches!(
            PersonaRepository::get(&database, id),
            Err(RepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn default_cas_archive_policies_and_non_default_behavior_are_atomic() {
        let database = Database::open_in_memory().expect("database");
        let first = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "First".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        let second = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "Second".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        let selected = PersonaRepository::set_default(
            &database,
            first.id,
            Revision::INITIAL,
            TimestampMillis::new(2),
        )
        .expect("default");
        assert!(matches!(
            PersonaRepository::clear_default(&database, Revision::INITIAL, TimestampMillis::new(3)),
            Err(RepositoryError::StaleRevision { expected, actual })
                if expected == Revision::INITIAL && actual == Revision::new(2)
        ));
        let selected_again = PersonaRepository::set_default(
            &database,
            first.id,
            selected.revision,
            TimestampMillis::new(3),
        )
        .expect("same default can bump singleton");
        assert!(matches!(
            PersonaRepository::archive(
                &database,
                PersonaArchiveRequest {
                    persona_id: first.id,
                    expected_persona_revision: first.revision,
                    expected_default_revision: None,
                    now: TimestampMillis::new(4),
                },
            ),
            Err(RepositoryError::MissingDefaultRevision)
        ));
        assert!(matches!(
            PersonaRepository::archive(
                &database,
                PersonaArchiveRequest {
                    persona_id: first.id,
                    expected_persona_revision: first.revision,
                    expected_default_revision: Some(selected.revision),
                    now: TimestampMillis::new(4),
                },
            ),
            Err(RepositoryError::StaleRevision { expected, actual })
                if expected == selected.revision && actual == selected_again.revision
        ));

        let non_default = PersonaRepository::archive(
            &database,
            PersonaArchiveRequest {
                persona_id: second.id,
                expected_persona_revision: second.revision,
                expected_default_revision: Some(Revision::new(999)),
                now: TimestampMillis::new(4),
            },
        )
        .expect("non-default archive ignores singleton token");
        assert_eq!(non_default.default.persona_id, Some(first.id));
        assert_eq!(non_default.default.revision, selected_again.revision);
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("snapshot")
                .state
                .revision,
            selected_again.revision
        );
        let cleared = PersonaRepository::clear_default(
            &database,
            selected_again.revision,
            TimestampMillis::new(5),
        )
        .expect("clear");
        assert_eq!(cleared.persona_id, None);
        let restored = PersonaRepository::restore(
            &database,
            non_default.persona.id,
            non_default.persona.revision,
            TimestampMillis::new(6),
        )
        .expect("restore");
        assert_eq!(restored.status, LifecycleStatus::Active);
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("snapshot")
                .state
                .persona_id,
            None,
            "restore must never reselect a persona"
        );
    }

    #[test]
    fn persona_default_mutations_journal_once_and_fail_atomically() {
        let database = Database::open_in_memory().expect("database");
        let selected_persona = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "Selected".into(),
                "Default persona".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create selected");
        let archived_persona = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "Archived".into(),
                "Unavailable persona".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create archived");
        PersonaRepository::archive(
            &database,
            PersonaArchiveRequest {
                persona_id: archived_persona.id,
                expected_persona_revision: archived_persona.revision,
                expected_default_revision: None,
                now: TimestampMillis::new(2),
            },
        )
        .expect("archive");

        assert_eq!(
            PersonaRepository::set_default(
                &database,
                PersonaId::new(),
                Revision::INITIAL,
                TimestampMillis::new(3),
            ),
            Err(RepositoryError::NotFound)
        );
        assert_eq!(
            PersonaRepository::set_default(
                &database,
                archived_persona.id,
                Revision::INITIAL,
                TimestampMillis::new(3),
            ),
            Err(RepositoryError::Archived)
        );

        let before = PersonaRepository::get_default_snapshot(&database)
            .expect("initial default")
            .state;
        let selected = PersonaRepository::set_default(
            &database,
            selected_persona.id,
            before.revision,
            TimestampMillis::new(10),
        )
        .expect("set default");
        assert_eq!(
            PersonaRepository::set_default(
                &database,
                selected_persona.id,
                before.revision,
                TimestampMillis::new(10),
            )
            .expect("set replay"),
            selected
        );
        let cleared = PersonaRepository::clear_default(
            &database,
            selected.revision,
            TimestampMillis::new(20),
        )
        .expect("clear default");
        assert_eq!(
            PersonaRepository::clear_default(
                &database,
                selected.revision,
                TimestampMillis::new(20),
            )
            .expect("clear replay"),
            cleared
        );

        let connection = database.connection().expect("connection");
        let set_change = load_local_change_in(
            &connection,
            persona_set_default_operation(before.revision, selected_persona.id),
        )
        .expect("set change read")
        .expect("set change");
        let clear_change = load_local_change_in(
            &connection,
            persona_clear_default_operation(selected.revision),
        )
        .expect("clear change read")
        .expect("clear change");
        assert_eq!(set_change.origin_sequence(), 4);
        assert_eq!(clear_change.origin_sequence(), 5);
        assert_eq!(
            set_change.base_revision(),
            Some(
                canonical_persona_default_payload(&before)
                    .expect("set base")
                    .content_hash()
            )
        );
        assert_eq!(
            clear_change.base_revision(),
            Some(
                canonical_persona_default_payload(&selected)
                    .expect("clear base")
                    .content_hash()
            )
        );
        assert_eq!(
            serde_json::from_slice::<PersonaDefaultState>(
                set_change.payload().expect("set payload").bytes()
            )
            .expect("decode set"),
            selected
        );
        assert_eq!(
            serde_json::from_slice::<PersonaDefaultState>(
                clear_change.payload().expect("clear payload").bytes()
            )
            .expect("decode clear"),
            cleared
        );
        drop(connection);

        assert!(matches!(
            PersonaRepository::clear_default(
                &database,
                selected.revision,
                TimestampMillis::new(21),
            ),
            Err(RepositoryError::StaleRevision { .. })
        ));
        let connection = database.connection().expect("connection");
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_persona_default_write
                 BEFORE UPDATE ON persona_defaults
                 BEGIN SELECT RAISE(ABORT, 'reject default write'); END;",
            )
            .expect("default failure trigger");
        drop(connection);
        assert!(
            PersonaRepository::set_default(
                &database,
                selected_persona.id,
                cleared.revision,
                TimestampMillis::new(30),
            )
            .is_err()
        );
        let connection = database.connection().expect("connection");
        connection
            .execute_batch(
                "DROP TRIGGER reject_persona_default_write;
                 CREATE TEMP TRIGGER reject_persona_default_change
                 BEFORE INSERT ON sync_changes
                 BEGIN SELECT RAISE(ABORT, 'reject default journal'); END;",
            )
            .expect("journal failure trigger");
        drop(connection);
        assert!(
            PersonaRepository::set_default(
                &database,
                selected_persona.id,
                cleared.revision,
                TimestampMillis::new(30),
            )
            .is_err()
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default after failures")
                .state,
            cleared
        );
        let connection = database.connection().expect("connection");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM sync_changes", [], |row| row
                    .get::<_, i64>(0))
                .expect("change count"),
            5
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT origin_sequence FROM sync_local_state WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("sequence"),
            5
        );
    }

    #[test]
    fn default_set_race_across_two_handles_has_one_winner() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let path = std::env::temp_dir().join(format!(
            "lettuce-persona-default-race-{}.db",
            AssetId::new()
        ));
        let setup = Database::open(&path).expect("setup");
        let persona = PersonaRepository::create(
            &setup,
            Persona::new(
                PersonaId::new(),
                "Racer".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        drop(setup);
        let first = Database::open(&path).expect("first");
        let second = Database::open(&path).expect("second");
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let first_id = persona.id;
        let first_thread = thread::spawn(move || {
            first_barrier.wait();
            PersonaRepository::set_default(
                &first,
                first_id,
                Revision::INITIAL,
                TimestampMillis::new(2),
            )
        });
        let second_barrier = Arc::clone(&barrier);
        let second_id = persona.id;
        let second_thread = thread::spawn(move || {
            second_barrier.wait();
            PersonaRepository::set_default(
                &second,
                second_id,
                Revision::INITIAL,
                TimestampMillis::new(3),
            )
        });
        let first_result = first_thread.join().expect("first thread");
        let second_result = second_thread.join().expect("second thread");
        assert_eq!(first_result.is_ok() as u8 + second_result.is_ok() as u8, 1);
        assert!(matches!(
            first_result.as_ref().err().or(second_result.as_ref().err()),
            Some(RepositoryError::StaleRevision { .. })
        ));
        drop(std::fs::remove_file(path));
    }

    #[test]
    fn default_aware_archive_rolls_back_when_default_update_fails() {
        let database = Database::open_in_memory().expect("database");
        let value = PersonaRepository::create(
            &database,
            Persona::new(
                PersonaId::new(),
                "Rollback".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        let default = PersonaRepository::set_default(
            &database,
            value.id,
            Revision::INITIAL,
            TimestampMillis::new(2),
        )
        .expect("default");
        let connection = database.connection().expect("lock");
        connection
            .execute_batch(
                "CREATE TRIGGER fail_persona_default_archive BEFORE UPDATE ON persona_defaults
                 WHEN NEW.default_persona_id IS NULL
                 BEGIN SELECT RAISE(ABORT, 'injected archive failure'); END;",
            )
            .expect("trigger");
        drop(connection);
        assert!(
            PersonaRepository::archive(
                &database,
                PersonaArchiveRequest {
                    persona_id: value.id,
                    expected_persona_revision: value.revision,
                    expected_default_revision: Some(default.revision),
                    now: TimestampMillis::new(3),
                },
            )
            .is_err()
        );
        let connection = database.connection().expect("lock");
        connection
            .execute_batch("DROP TRIGGER fail_persona_default_archive")
            .expect("drop trigger");
        drop(connection);
        assert_eq!(
            PersonaRepository::get(&database, value.id)
                .expect("persona after rollback")
                .expect("persona")
                .status,
            LifecycleStatus::Active
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default after rollback")
                .state
                .persona_id,
            Some(value.id)
        );
        let connection = database.connection().expect("lock");
        connection
            .execute_batch(&format!(
                "CREATE TEMP TRIGGER fail_persona_archive_default_journal
                 BEFORE INSERT ON sync_changes
                 WHEN NEW.operation_id = '{}'
                 BEGIN SELECT RAISE(ABORT, 'injected archive journal failure'); END;",
                persona_archive_default_operation(value.id, value.revision, default.revision)
            ))
            .expect("journal failure trigger");
        drop(connection);
        assert!(
            PersonaRepository::archive(
                &database,
                PersonaArchiveRequest {
                    persona_id: value.id,
                    expected_persona_revision: value.revision,
                    expected_default_revision: Some(default.revision),
                    now: TimestampMillis::new(3),
                },
            )
            .is_err()
        );
        assert_eq!(
            PersonaRepository::get(&database, value.id)
                .expect("persona after journal rollback")
                .expect("persona")
                .status,
            LifecycleStatus::Active
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default after journal rollback")
                .state,
            default
        );
        let connection = database.connection().expect("connection");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM sync_changes", [], |row| row
                    .get::<_, i64>(0))
                .expect("change count"),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT origin_sequence FROM sync_local_state WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("sequence"),
            2
        );
    }

    #[test]
    fn persona_media_sql_requires_image_composite_parent_and_restricts_assets() {
        let database = Database::open_in_memory().expect("database");
        let image = image_asset(&database, 'o');
        let audio = audio_asset(&database, 'p');
        let value = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        let id = value.id;
        PersonaRepository::create(&database, value).expect("create");
        let connection = database.connection().expect("lock");
        let image_blob: String = connection
            .query_row(
                "SELECT blob_id FROM media_assets WHERE id=?1",
                [image.to_string()],
                |row| row.get(0),
            )
            .expect("image blob");
        let raw_audio = connection.execute(
            "INSERT INTO persona_media(persona_id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,'audio','design_reference',0)",
            params![id.to_string(), audio.to_string()],
        );
        assert!(
            raw_audio.is_err(),
            "audio persona media must fail SQL constraints"
        );
        let raw_composite = connection.execute(
            "INSERT INTO persona_media(persona_id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,'image','design_reference',0)",
            params![id.to_string(), audio.to_string()],
        );
        assert!(
            raw_composite.is_err(),
            "an audio asset declared as image must fail the composite media FK"
        );
        let missing = AssetId::new();
        let raw_missing = connection.execute(
            "INSERT INTO persona_media(persona_id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,'image','design_reference',0)",
            params![id.to_string(), missing.to_string()],
        );
        assert!(
            raw_missing.is_err(),
            "missing asset must fail SQL constraints"
        );
        drop(connection);
        let value = PersonaRepository::attach_media(
            &database,
            id,
            Revision::INITIAL,
            PersonaMediaLink {
                asset_id: image,
                slot: PersonaMediaSlot::DesignReference,
                ordinal: 0,
            },
            TimestampMillis::new(2),
        )
        .expect("attach");
        let connection = database.connection().expect("lock");
        connection
            .execute("DELETE FROM media_assets WHERE id=?1", [image.to_string()])
            .expect_err("attached asset is restricted");
        drop(connection);
        let _value = PersonaRepository::detach_media(
            &database,
            value.id,
            value.revision,
            image,
            PersonaMediaSlot::DesignReference,
            TimestampMillis::new(3),
        )
        .expect("detach");
        let connection = database.connection().expect("lock");
        connection
            .execute("DELETE FROM media_assets WHERE id=?1", [image.to_string()])
            .expect("detached asset can be deleted");
        assert_eq!(
            connection
                .execute("DELETE FROM media_blobs WHERE id=?1", [image_blob],)
                .expect("detached blob can be deleted"),
            1
        );
    }

    #[test]
    fn default_snapshot_reads_single_consistent_snapshot_across_handles() {
        let path =
            std::env::temp_dir().join(format!("lettuce-persona-snapshot-{}.db", AssetId::new()));
        let setup = Database::open(&path).expect("setup");
        let first = PersonaRepository::create(
            &setup,
            Persona::new(
                PersonaId::new(),
                "First".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        let second = PersonaRepository::create(
            &setup,
            Persona::new(
                PersonaId::new(),
                "Second".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        PersonaRepository::set_default(
            &setup,
            first.id,
            Revision::INITIAL,
            TimestampMillis::new(2),
        )
        .expect("default");
        drop(setup);
        let reader = Database::open(&path).expect("reader");
        let writer = Database::open(&path).expect("writer");
        let mut reader_connection = reader.connection().expect("reader lock");
        let tx = reader_connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("read transaction");
        tx.query_row(
            "SELECT revision FROM persona_defaults WHERE id=1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("establish default snapshot");
        let writer_state = PersonaRepository::set_default(
            &writer,
            second.id,
            Revision::new(2),
            TimestampMillis::new(3),
        )
        .expect("writer default");
        let snapshot_state = read_default(&tx).expect("snapshot default");
        let snapshot_persona = load_persona(&tx, snapshot_state.persona_id.expect("selected"))
            .expect("snapshot persona")
            .expect("selected persona");
        assert_eq!(snapshot_state.persona_id, Some(first.id));
        assert_eq!(snapshot_persona.id, first.id);
        tx.commit().expect("commit snapshot");
        drop(reader_connection);
        assert_eq!(writer_state.persona_id, Some(second.id));
        assert_eq!(
            PersonaRepository::get_default_snapshot(&reader)
                .expect("latest snapshot")
                .state
                .persona_id,
            Some(second.id)
        );
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn becoming_default_during_archive_cannot_leave_an_archived_default() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let path = std::env::temp_dir().join(format!(
            "lettuce-persona-became-default-{}.db",
            AssetId::new()
        ));
        let setup = Database::open(&path).expect("setup");
        let value = PersonaRepository::create(
            &setup,
            Persona::new(
                PersonaId::new(),
                "Racer".into(),
                "Description".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        drop(setup);
        let first = Database::open(&path).expect("first");
        let second = Database::open(&path).expect("second");
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let id = value.id;
        let set_thread = thread::spawn(move || {
            first_barrier.wait();
            PersonaRepository::set_default(&first, id, Revision::INITIAL, TimestampMillis::new(2))
        });
        let second_barrier = Arc::clone(&barrier);
        let archive_thread = thread::spawn(move || {
            second_barrier.wait();
            PersonaRepository::archive(
                &second,
                PersonaArchiveRequest {
                    persona_id: id,
                    expected_persona_revision: Revision::INITIAL,
                    expected_default_revision: Some(Revision::INITIAL),
                    now: TimestampMillis::new(3),
                },
            )
        });
        let set_result = set_thread.join().expect("set thread");
        let archive_result = archive_thread.join().expect("archive thread");
        match (&set_result, &archive_result) {
            (Ok(_), Err(RepositoryError::StaleRevision { .. })) => {
                let final_reader = Database::open(&path).expect("final reader");
                let final_snapshot =
                    PersonaRepository::get_default_snapshot(&final_reader).expect("final snapshot");
                assert_eq!(final_snapshot.state.persona_id, Some(id));
                assert_eq!(
                    PersonaRepository::get(&final_reader, id)
                        .expect("final persona")
                        .expect("persona")
                        .status,
                    LifecycleStatus::Active
                );
            }
            (Err(_), Ok(result)) => {
                assert_eq!(result.persona.status, LifecycleStatus::Archived);
                assert_eq!(result.default.persona_id, None);
                let final_reader = Database::open(&path).expect("final reader");
                let final_snapshot =
                    PersonaRepository::get_default_snapshot(&final_reader).expect("final snapshot");
                assert_eq!(final_snapshot.state.persona_id, None);
                assert_eq!(
                    PersonaRepository::get(&final_reader, id)
                        .expect("final persona")
                        .expect("persona")
                        .status,
                    LifecycleStatus::Archived
                );
            }
            _ => panic!(
                "unexpected serializable race outcome: set={set_result:?}, archive={archive_result:?}"
            ),
        }
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn persona_save_keeps_unchanged_avatar_out_of_gc() {
        let database = Database::open_in_memory().expect("database");
        let asset = image_asset(&database, 'e');
        let mut persona = Persona::new(PersonaId::new(), "Persona".into(), "Description".into(), TimestampMillis::new(1)).expect("persona");
        persona.media.links.push(PersonaMediaLink { asset_id: asset, slot: PersonaMediaSlot::Avatar, ordinal: 0 });
        let persona = PersonaRepository::create(&database, persona).expect("create");
        database.commit_api_operation::<Persona, crate::ApiOperationError>("persona_save", "unchanged", "digest", TimestampMillis::new(10), |scope| scope.save_persona(Some(persona.id), Some(Revision::INITIAL), PersonaDraftUpdate { title: persona.title.clone(), description: persona.description.clone(), nickname: None, design_description: None, avatar_crop: None, image_recommendation: None }, persona.media.clone(), TimestampMillis::new(10)).map_err(|error| { tracing::warn!(%error, "persona save keeps unchanged avatar out of gc failed"); crate::ApiOperationError::Storage {
            reason: "injected failure",
        } })).expect("save");
        let queued: i64 = database.connection().expect("lock").query_row("SELECT count(*) FROM media_gc_candidates", [], |row| row.get(0)).expect("queue");
        assert_eq!(queued, 0);
    }

    #[test]
    fn persona_save_and_receipt_are_atomic_with_replay_and_cas() {
        let database = Database::open_in_memory().expect("database");
        let id = PersonaId::new();
        let persona = Persona::new(
            id,
            "Before".into(),
            "Original".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        PersonaRepository::create(&database, persona.clone()).expect("create");
        let events = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = events.clone();
        database.on_library_change(move |kind, changed_id| {
            if kind == crate::LibraryKind::Persona && changed_id == id.to_string() {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        let apply = |key: &str, digest: &str, revision| {
            database.commit_api_operation::<Persona, crate::ApiOperationError>(
                "persona_save",
                key,
                digest,
                TimestampMillis::new(2),
                |scope| {
                    scope
                        .save_persona(
                            Some(id),
                            Some(revision),
                            PersonaDraftUpdate {
                                title: "After".into(),
                                description: "Changed".into(),
                                nickname: Some("Alias".into()),
                                design_description: None,
                                avatar_crop: None,
                                image_recommendation: None,
                            },
                            PersonaMedia::default(),
                            TimestampMillis::new(2),
                        )
                        .map_err(|error| match error {
                            RepositoryError::StaleRevision { .. } => {
                                crate::ApiOperationError::Conflict
                            }
                            _ => crate::ApiOperationError::Storage {
                                reason: "injected failure",
                            },
                        })
                },
            )
        };
        database.connection().expect("lock").execute_batch("CREATE TEMP TRIGGER fail_persona_save_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT,'late failure'); END;").expect("fault");
        assert!(matches!(apply("save", "digest", Revision::INITIAL), Err(crate::ApiOperationError::Storage { .. })));
        assert_eq!(
            PersonaRepository::get(&database, id).expect("get"),
            Some(persona)
        );
        assert!(
            database
                .lookup_api_operation("persona_save", "save")
                .expect("receipt")
                .is_none()
        );
        database
            .connection()
            .expect("lock")
            .execute_batch("DROP TRIGGER fail_persona_save_receipt;")
            .expect("clear");
        assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 0);
        let saved = apply("save", "digest", Revision::INITIAL).expect("save");
        assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(saved.title, "After");
        assert_eq!(saved.revision, Revision::new(2));
        assert_eq!(
            apply("save", "digest", Revision::INITIAL).expect("replay"),
            saved
        );
        assert_eq!(
            apply("save", "different", Revision::INITIAL),
            Err(crate::ApiOperationError::Conflict)
        );
        assert_eq!(
            apply("stale", "digest", Revision::INITIAL),
            Err(crate::ApiOperationError::Conflict)
        );
    }
    #[test]
    fn persona_save_process_exit_preserves_fields_media_and_replay_state() {
        if let Some(path) = std::env::var_os("LETTUCE_PERSONA_SAVE_CRASH_DATABASE") {
            let database = Database::open(path).expect("child database");
            let id = std::env::var("LETTUCE_PERSONA_SAVE_CRASH_ID")
                .expect("id")
                .parse()
                .expect("persona id");
            {
                let connection = database.connection().expect("lock");
                connection
                    .create_scalar_function(
                        "crash_persona_save",
                        0,
                        rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                        |_| -> rusqlite::Result<i64> {
                            std::process::exit(77);
                        },
                    )
                    .expect("function");
                connection.execute_batch("CREATE TEMP TRIGGER crash_persona_save_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT crash_persona_save(); END;").expect("trigger");
            }
            database
                .commit_api_operation::<Persona, crate::ApiOperationError>(
                    "persona_save",
                    "crash",
                    "digest",
                    TimestampMillis::new(2),
                    |scope| {
                        scope
                            .save_persona(
                                Some(id),
                                Some(Revision::INITIAL),
                                PersonaDraftUpdate {
                                    title: "After".into(),
                                    description: "Changed".into(),
                                    nickname: None,
                                    design_description: None,
                                    avatar_crop: None,
                                    image_recommendation: None,
                                },
                                PersonaMedia::default(),
                                TimestampMillis::new(2),
                            )
                            .map_err(|error| { tracing::warn!(%error, "persona save process exit preserves fields media and replay state failed"); crate::ApiOperationError::Storage {
                                reason: "injected failure",
                            } })
                    },
                )
                .expect("save");
            panic!("crash hook was not reached");
        }
        let root = std::env::temp_dir().join(format!(
            "slice5-persona-save-crash-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).expect("directory");
        let path = root.join("library.sqlite3");
        let database = Database::open(&path).expect("database");
        let asset_id = image_asset(&database, 'c');
        let mut persona = Persona::new(
            PersonaId::new(),
            "Before".into(),
            "Original".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        persona.media.links.push(PersonaMediaLink {
            asset_id,
            slot: PersonaMediaSlot::Avatar,
            ordinal: 0,
        });
        let persona = PersonaRepository::create(&database, persona).expect("create");
        drop(database);
        let output = std::process::Command::new(std::env::current_exe().expect("executable"))
            .args(["--exact", "catalog::persona_adapter::tests::persona_save_process_exit_preserves_fields_media_and_replay_state"])
            .env("LETTUCE_PERSONA_SAVE_CRASH_DATABASE", &path).env("LETTUCE_PERSONA_SAVE_CRASH_ID", persona.id.to_string()).output().expect("child");
        assert_eq!(
            output.status.code(),
            Some(77),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let database = Database::open(&path).expect("reopen");
        assert_eq!(
            PersonaRepository::get(&database, persona.id).expect("get"),
            Some(persona.clone())
        );
        assert!(
            database
                .lookup_api_operation("persona_save", "crash")
                .expect("receipt")
                .is_none()
        );
        database
            .commit_api_operation::<Persona, crate::ApiOperationError>(
                "persona_save",
                "crash",
                "digest",
                TimestampMillis::new(2),
                |scope| {
                    scope
                        .save_persona(
                            Some(persona.id),
                            Some(Revision::INITIAL),
                            PersonaDraftUpdate {
                                title: "After".into(),
                                description: "Changed".into(),
                                nickname: None,
                                design_description: None,
                                avatar_crop: None,
                                image_recommendation: None,
                            },
                            PersonaMedia::default(),
                            TimestampMillis::new(2),
                        )
                        .map_err(|error| { tracing::warn!(%error, "persona save process exit preserves fields media and replay state failed"); crate::ApiOperationError::Storage {
                            reason: "injected failure",
                        } })
                },
            )
            .expect("retry");
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
    #[test]
    fn concurrent_persona_saves_have_one_cas_winner() {
        let database = Database::open_in_memory().expect("database");
        let persona = Persona::new(
            PersonaId::new(),
            "Original".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        let id = persona.id;
        PersonaRepository::create(&database, persona).expect("create");
        let barrier = std::sync::Barrier::new(2);
        let results = std::thread::scope(|threads| {
            let handles: Vec<_> = ["First", "Second"]
                .into_iter()
                .map(|name| {
                    let database = &database;
                    let barrier = &barrier;
                    threads.spawn(move || {
                        barrier.wait();
                        database.commit_api_operation::<Persona, crate::ApiOperationError>(
                            "persona_save",
                            name,
                            name,
                            TimestampMillis::new(2),
                            |scope| {
                                scope
                                    .save_persona(
                                        Some(id),
                                        Some(Revision::INITIAL),
                                        PersonaDraftUpdate {
                                            title: name.into(),
                                            description: "Changed".into(),
                                            nickname: None,
                                            design_description: None,
                                            avatar_crop: None,
                                            image_recommendation: None,
                                        },
                                        PersonaMedia::default(),
                                        TimestampMillis::new(2),
                                    )
                                    .map_err(|error| match error {
                                        RepositoryError::StaleRevision { .. } => {
                                            crate::ApiOperationError::Conflict
                                        }
                                        _ => crate::ApiOperationError::Storage {
                                            reason: "injected failure",
                                        },
                                    })
                            },
                        )
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("worker"))
                .collect::<Vec<_>>()
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(crate::ApiOperationError::Conflict)))
                .count(),
            1
        );
        let winner = results.into_iter().find_map(Result::ok).expect("winner");
        assert_eq!(
            PersonaRepository::get(&database, id).expect("get"),
            Some(winner)
        );
    }
    #[test]
    fn persona_default_and_receipt_roll_back_together() {
        let database = Database::open_in_memory().expect("database");
        let persona = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        PersonaRepository::create(&database, persona.clone()).expect("create");
        let before = PersonaRepository::get_default_snapshot(&database)
            .expect("default")
            .state;
        database.connection().expect("lock").execute_batch("CREATE TEMP TRIGGER fail_default_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT,'late failure'); END;").expect("fault");
        let result = database
            .commit_api_operation::<PersonaDefaultState, crate::ApiOperationError>(
                "persona_default_set",
                "set",
                "digest",
                TimestampMillis::new(2),
                |scope| {
                    scope
                        .set_library_default_persona(
                            Some(persona.id),
                            before.revision,
                            TimestampMillis::new(2),
                        )
                        .map_err(|error| { tracing::warn!(%error, "persona default and receipt roll back together failed"); crate::ApiOperationError::Storage {
                            reason: "injected failure",
                        } })
                },
            );
        assert!(matches!(result, Err(crate::ApiOperationError::Storage { .. })));
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default")
                .state,
            before
        );
        assert!(
            database
                .lookup_api_operation("persona_default_set", "set")
                .expect("receipt")
                .is_none()
        );
    }
    #[test]
    fn default_new_receipt_cannot_replay_a_stale_domain_operation() {
        let database = Database::open_in_memory().expect("database");
        let persona = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "Description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        PersonaRepository::create(&database, persona.clone()).expect("create");
        let expected = PersonaRepository::get_default_snapshot(&database)
            .expect("default")
            .state
            .revision;
        let apply = |key| {
            database.commit_api_operation::<PersonaDefaultState, crate::ApiOperationError>(
                "persona_default_set",
                key,
                "digest",
                TimestampMillis::new(2),
                |scope| {
                    scope
                        .set_library_default_persona(
                            Some(persona.id),
                            expected,
                            TimestampMillis::new(2),
                        )
                        .map_err(|error| match error {
                            RepositoryError::StaleRevision { .. } => {
                                crate::ApiOperationError::Conflict
                            }
                            _ => crate::ApiOperationError::Storage {
                                reason: "injected failure",
                            },
                        })
                },
            )
        };
        apply("first").expect("select");
        assert_eq!(apply("second"), Err(crate::ApiOperationError::Conflict));
        assert!(
            database
                .lookup_api_operation("persona_default_set", "second")
                .expect("receipt")
                .is_none()
        );
    }
}
