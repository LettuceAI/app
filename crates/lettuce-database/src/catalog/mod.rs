pub(crate) mod library_counts;
pub(crate) mod character_adapter;
pub(crate) mod character_file_adapter;
pub(crate) mod creation_adapter;
pub(crate) mod group_adapter;
pub(crate) mod persona_adapter;
pub(crate) mod prompt_adapter;
pub(crate) mod source_delete;

fn replace_media_links(
    tx: &rusqlite::Transaction<'_>,
    owner: &str,
    links: &[(String, &str, u32)],
    table: (&str, &str, bool),
    db_error: fn(rusqlite::Error) -> lettuce_characters::RepositoryError,
) -> Result<(), lettuce_characters::RepositoryError> {
    use lettuce_characters::RepositoryError;
    use rusqlite::params;

    let (table, owner_column, image_kind) = table;
    let ids = serde_json::to_string(&links.iter().map(|link| &link.0).collect::<Vec<_>>()).map_err(|error| RepositoryError::Storage {
        reason: error.to_string(),
    })?;
    tx.execute(&format!("DELETE FROM {table} WHERE {owner_column}=?1 AND asset_id NOT IN (SELECT value FROM json_each(?2))"), params![owner, ids]).map_err(db_error)?;
    let mut statement = tx.prepare(&format!("SELECT asset_id,slot,ordinal FROM {table} WHERE {owner_column}=?1")).map_err(db_error)?;
    let existing = statement.query_map([owner], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, u32>(2)?))).map_err(db_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?;
    drop(statement);
    let offset = existing.iter().map(|row| i64::from(row.2)).chain(links.iter().map(|link| i64::from(link.2))).max().unwrap_or(0) + 1;
    for (index, (asset, slot, ordinal)) in existing.iter().enumerate() {
        let link = links.iter().find(|link| link.0 == *asset).ok_or_else(|| RepositoryError::Storage {
            reason: "replace media links: a required value is missing".to_owned(),
        })?;
        if *slot != link.1 || *ordinal != link.2 {
            tx.execute(&format!("UPDATE {table} SET slot='design_reference',ordinal=?3 WHERE {owner_column}=?1 AND asset_id=?2"), params![owner, asset, offset + i64::try_from(index).map_err(|error| RepositoryError::Storage {
                reason: error.to_string(),
            })?]).map_err(db_error)?;
        }
    }
    let (kind_column, kind_value) = if image_kind { (",blob_kind", ",'image'") } else { ("", "") };
    for (asset, slot, ordinal) in links {
        if existing.iter().any(|row| row.0 == *asset && row.1 == *slot && row.2 == *ordinal) {
            continue;
        }
        tx.execute(&format!("INSERT INTO {table}({owner_column},asset_id{kind_column},slot,ordinal) VALUES (?1,?2{kind_value},?3,?4) ON CONFLICT({owner_column},asset_id) DO UPDATE SET slot=excluded.slot,ordinal=excluded.ordinal"), params![owner, asset, slot, ordinal]).map_err(db_error)?;
    }
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    format_version: u32,
    value: T,
}

fn cursor_encode(
    key: serde_json::Value,
    id: String,
    scope: (lettuce_characters::LibrarySort, String, bool, bool, &str),
) -> Result<String, lettuce_characters::RepositoryError> {
    let (sort, query, search_nickname, include_archived, entity) = scope;
    serde_json::to_vec(&Envelope {
        format_version: 1,
        value: (key, id, sort, query, search_nickname, include_archived, entity),
    })
    .map(|bytes| crate::hex_encode(&bytes))
    .map_err(|error| lettuce_characters::RepositoryError::Storage { reason: error.to_string() })
}

fn cursor_decode(
    value: Option<&str>,
    scope: (lettuce_characters::LibrarySort, String, bool, bool, &str),
) -> Result<Option<(rusqlite::types::Value, String)>, lettuce_characters::RepositoryError> {
    use lettuce_characters::{LibrarySort, RepositoryError, ValidationError};
    use lettuce_types::{CharacterId, GroupId, PersonaId};

    let field = match scope.4 {
        "character" => "character.cursor",
        "persona" => "persona.cursor",
        "group" => "group.cursor",
        _ => return Err(RepositoryError::Storage {
            reason: "cursor decode: a stored value is not recognised".to_owned(),
        }),
    };
    let invalid = || RepositoryError::Invalid(ValidationError::InvalidValue { field });
    let Some(value) = value else { return Ok(None) };
    let bytes = crate::hex_decode(value).map_err(|_| invalid())?;
    let envelope: Envelope<(serde_json::Value, String, LibrarySort, String, bool, bool, String)> = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let (key, id, sort, query, search_nickname, include_archived, entity) = envelope.value;
    if envelope.format_version != 1 || (sort, query, search_nickname, include_archived, entity.as_str()) != scope {
        return Err(invalid());
    }
    let valid_id = match entity.as_str() {
        "character" => id.parse::<CharacterId>().is_ok(),
        "persona" => id.parse::<PersonaId>().is_ok(),
        "group" => id.parse::<GroupId>().is_ok(),
        _ => false,
    };
    if !valid_id {
        return Err(invalid());
    }
    let key = match sort {
        LibrarySort::Name => rusqlite::types::Value::Text(key.as_str().ok_or_else(invalid)?.to_owned()),
        _ => rusqlite::types::Value::Integer(key.as_i64().ok_or_else(invalid)?),
    };
    Ok(Some((key, id)))
}
