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
use lettuce_types::{AssetId, Page, PageRequest, PersonaId, Revision, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use unicode_normalization::UnicodeNormalization;

use super::Database;
const CROP_VERSION: u32 = 1;
const RECOMMENDATION_VERSION: u32 = 1;
const CURSOR_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    format_version: u32,
    value: T,
}

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
        _ => RepositoryError::Storage,
    }
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
    i64::try_from(value).map_err(|_| RepositoryError::Storage)
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

fn cursor_encode(updated_at: TimestampMillis, id: PersonaId) -> Result<String, RepositoryError> {
    let envelope = Envelope {
        format_version: CURSOR_VERSION,
        value: (updated_at.get(), id.to_string()),
    };
    serde_json::to_vec(&envelope)
        .map(|bytes| super::hex_encode(&bytes))
        .map_err(|_| {
            RepositoryError::Invalid(lettuce_characters::ValidationError::Invariant {
                field: "persona.cursor",
            })
        })
}

fn cursor_decode(value: Option<&str>) -> Result<Option<(i64, PersonaId)>, RepositoryError> {
    let Some(value) = value else { return Ok(None) };
    let bytes = super::hex_decode(value).map_err(|_| {
        RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidValue {
            field: "persona.cursor",
        })
    })?;
    let envelope: Envelope<(i64, String)> = serde_json::from_slice(&bytes).map_err(|_| {
        RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidValue {
            field: "persona.cursor",
        })
    })?;
    if envelope.format_version != CURSOR_VERSION {
        return Err(RepositoryError::Invalid(
            lettuce_characters::ValidationError::InvalidValue {
                field: "persona.cursor",
            },
        ));
    }
    Ok(Some((
        envelope.value.0,
        envelope.value.1.parse().map_err(|_| {
            RepositoryError::Invalid(lettuce_characters::ValidationError::InvalidValue {
                field: "persona.cursor",
            })
        })?,
    )))
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

fn load_persona(
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

fn read_default(connection: &Connection) -> Result<PersonaDefaultState, rusqlite::Error> {
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
        link.ordinal = u32::try_from(ordinal).map_err(|_| RepositoryError::Storage)?;
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
    tx.execute(
        "DELETE FROM persona_media WHERE persona_id=?1",
        [persona_id.to_string()],
    )
    .map_err(db_error)?;
    for link in &media.links {
        tx.execute(
            "INSERT INTO persona_media(persona_id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,'image',?3,?4)",
            params![
                persona_id.to_string(),
                link.asset_id.to_string(),
                media_slot_name(link.slot),
                i64::from(link.ordinal)
            ],
        )
        .map_err(db_error)?;
    }
    Ok(())
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
        .ok_or(RepositoryError::Storage)
}

fn update_root(
    tx: &Transaction<'_>,
    id: PersonaId,
    expected_revision: Revision,
    now: TimestampMillis,
) -> Result<Revision, RepositoryError> {
    let next = expected_revision
        .next()
        .map_err(|_| RepositoryError::Storage)?;
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

fn load_page(
    tx: &rusqlite::Transaction<'_>,
    ids: Vec<PersonaId>,
    limit: usize,
) -> Result<Page<Persona>, RepositoryError> {
    let mut ids = ids;
    let has_more = ids.len() > limit;
    if has_more {
        ids.truncate(limit);
    }
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        items.push(
            load_persona(tx, id)
                .map_err(db_error)?
                .ok_or(RepositoryError::NotFound)?,
        );
    }
    let next_cursor = if has_more {
        items
            .last()
            .map(|persona| cursor_encode(persona.updated_at, persona.id))
            .transpose()?
    } else {
        None
    };
    Ok(Page { items, next_cursor })
}

impl PersonaRepository for Database {
    fn create(&self, persona: Persona) -> Result<Persona, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let stored = insert_persona(&tx, persona)?;
        tx.commit().map_err(db_error)?;
        Ok(stored)
    }

    fn get(&self, id: PersonaId) -> Result<Option<Persona>, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let persona = load_persona(&tx, id).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn get_default_snapshot(&self) -> Result<PersonaDefaultSnapshot, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let state = read_default(&tx).map_err(db_error)?;
        let persona = state
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
        let snapshot = PersonaDefaultSnapshot { state, persona };
        snapshot.validate()?;
        tx.commit().map_err(db_error)?;
        Ok(snapshot)
    }

    fn list(
        &self,
        request: PageRequest,
        include_archived: bool,
    ) -> Result<Page<Persona>, RepositoryError> {
        let cursor = cursor_decode(request.cursor.as_deref())?;
        let limit = usize::from(request.limit.get()).max(1);
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let mut sql = String::from("SELECT id FROM personas WHERE (?1 OR status='active')");
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
                |row| parse_id(row.get(0)?),
            )
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<PersonaId>>>()
            .map_err(db_error)?;
        drop(statement);
        let page = load_page(&tx, ids, limit)?;
        tx.commit().map_err(db_error)?;
        Ok(page)
    }

    fn search(
        &self,
        request: PersonaSearch,
        page: PageRequest,
    ) -> Result<Page<Persona>, RepositoryError> {
        if request.text.trim().is_empty() {
            return self.list(page, request.include_archived);
        }
        let cursor = cursor_decode(page.cursor.as_deref())?;
        let limit = usize::from(page.limit.get()).max(1);
        let text = canonical_title(&request.text)
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{text}%");
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let mut sql = String::from(
            "SELECT id FROM personas WHERE (?1 OR status='active') AND (normalized_title LIKE ?2 ESCAPE '\\' OR normalized_nickname LIKE ?2 ESCAPE '\\')",
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
                |row| parse_id(row.get(0)?),
            )
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<PersonaId>>>()
            .map_err(db_error)?;
        drop(statement);
        let result = load_page(&tx, ids, limit)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn revise(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        draft: PersonaDraftUpdate,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        draft.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_active(&tx, id, expected_revision)?;
        let next = expected_revision
            .next()
            .map_err(|_| RepositoryError::Storage)?;
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
            .ok_or(RepositoryError::Storage)?;
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
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        ensure_active(&tx, id, expected_revision)?;
        image_assets(&tx, media.links.iter().map(|link| link.asset_id))?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?;
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
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
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
            usize::try_from(link.ordinal).map_err(|_| RepositoryError::Storage)?
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
            link.ordinal = u32::try_from(ordinal).map_err(|_| RepositoryError::Storage)?;
        }
        links.extend(same_slot);
        let media = normalize_media(PersonaMedia { links })?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?;
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
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_active(&tx, id, expected_revision)?;
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
            link.ordinal = u32::try_from(ordinal).map_err(|_| RepositoryError::Storage)?;
        }
        let media = normalize_media(PersonaMedia { links })?;
        replace_media(&tx, id, &media)?;
        update_root(&tx, id, expected_revision, now)?;
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::Storage)?;
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
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = ensure_active(&tx, id, expected_revision)?;
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
        let target = usize::try_from(target_ordinal).map_err(|_| RepositoryError::Storage)?;
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
            link.ordinal = u32::try_from(ordinal).map_err(|_| RepositoryError::Storage)?;
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
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }

    fn set_default(
        &self,
        id: PersonaId,
        expected_default_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PersonaDefaultState, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = read_default(&tx).map_err(db_error)?;
        if current.revision != expected_default_revision {
            return Err(RepositoryError::StaleRevision {
                expected: expected_default_revision,
                actual: current.revision,
            });
        }
        let persona = load_persona(&tx, id)
            .map_err(db_error)?
            .ok_or(RepositoryError::NotFound)?;
        if persona.status != LifecycleStatus::Active {
            return Err(RepositoryError::Archived);
        }
        let next = expected_default_revision
            .next()
            .map_err(|_| RepositoryError::Storage)?;
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
            let actual = read_default(&tx).map_err(db_error)?.revision;
            return Err(RepositoryError::StaleRevision {
                expected: expected_default_revision,
                actual,
            });
        }
        let state = read_default(&tx).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(state)
    }

    fn clear_default(
        &self,
        expected_default_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PersonaDefaultState, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = read_default(&tx).map_err(db_error)?;
        if current.revision != expected_default_revision {
            return Err(RepositoryError::StaleRevision {
                expected: expected_default_revision,
                actual: current.revision,
            });
        }
        let next = expected_default_revision
            .next()
            .map_err(|_| RepositoryError::Storage)?;
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
            let actual = read_default(&tx).map_err(db_error)?.revision;
            return Err(RepositoryError::StaleRevision {
                expected: expected_default_revision,
                actual,
            });
        }
        let state = read_default(&tx).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(state)
    }

    fn archive(
        &self,
        request: PersonaArchiveRequest,
    ) -> Result<PersonaArchiveResult, RepositoryError> {
        request.validate()?;
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
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
        let default = read_default(&tx).map_err(db_error)?;
        let is_default = default.persona_id == Some(request.persona_id);
        if is_default {
            let expected_default = request
                .expected_default_revision
                .ok_or(RepositoryError::MissingDefaultRevision)?;
            if default.revision != expected_default {
                return Err(RepositoryError::StaleRevision {
                    expected: expected_default,
                    actual: default.revision,
                });
            }
        }
        let next_persona = request
            .expected_persona_revision
            .next()
            .map_err(|_| RepositoryError::Storage)?;
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
                .map_err(|_| RepositoryError::Storage)?;
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
            .ok_or(RepositoryError::Storage)?;
        let default = read_default(&tx).map_err(db_error)?;
        let result = PersonaArchiveResult { persona, default };
        result.validate()?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn restore(
        &self,
        id: PersonaId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<Persona, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
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
            .map_err(|_| RepositoryError::Storage)?;
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
            .ok_or(RepositoryError::Storage)?;
        tx.commit().map_err(db_error)?;
        Ok(persona)
    }
}

impl PersonaDependencyReader for Database {
    fn dependencies(&self, id: PersonaId) -> Result<DependencyReport, RepositoryError> {
        let mut connection = self.connection().map_err(|_| RepositoryError::Storage)?;
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

        let result = PersonaRepository::archive(
            &database,
            PersonaArchiveRequest {
                persona_id: stored.id,
                expected_persona_revision: stored.revision,
                expected_default_revision: Some(default.revision),
                now: TimestampMillis::new(3),
            },
        )
        .expect("archive");
        assert_eq!(result.persona.status, LifecycleStatus::Archived);
        assert_eq!(result.default.persona_id, None);
        let restored = PersonaRepository::restore(
            &database,
            stored.id,
            result.persona.revision,
            TimestampMillis::new(4),
        )
        .expect("restore");
        assert_eq!(restored.status, LifecycleStatus::Active);
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("empty default")
                .state
                .persona_id,
            None
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
        let first = PersonaRepository::list(
            &database,
            PageRequest {
                cursor: None,
                limit: PageLimit::new(2),
            },
            false,
        )
        .expect("first page");
        assert_eq!(first.items.len(), 2);
        let second = PersonaRepository::list(
            &database,
            PageRequest {
                cursor: first.next_cursor,
                limit: PageLimit::new(2),
            },
            false,
        )
        .expect("second page");
        assert_eq!(second.items.len(), 1);
        let escaped = PersonaRepository::search(
            &database,
            PersonaSearch {
                text: "100%".into(),
                include_archived: false,
            },
            PageRequest::default(),
        )
        .expect("escaped search");
        assert_eq!(escaped.items.len(), 1);
        assert_eq!(escaped.items[0].title, "100% Writer");
        assert!(matches!(
            PersonaRepository::list(
                &database,
                PageRequest {
                    cursor: Some("not-a-cursor".into()),
                    limit: PageLimit::new(2),
                },
                false,
            ),
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
}
