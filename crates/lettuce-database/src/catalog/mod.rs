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
    let ids = serde_json::to_string(&links.iter().map(|link| &link.0).collect::<Vec<_>>()).map_err(|_| RepositoryError::Storage)?;
    tx.execute(&format!("DELETE FROM {table} WHERE {owner_column}=?1 AND asset_id NOT IN (SELECT value FROM json_each(?2))"), params![owner, ids]).map_err(db_error)?;
    let mut statement = tx.prepare(&format!("SELECT asset_id,slot,ordinal FROM {table} WHERE {owner_column}=?1")).map_err(db_error)?;
    let existing = statement.query_map([owner], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, u32>(2)?))).map_err(db_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?;
    drop(statement);
    let offset = existing.iter().map(|row| i64::from(row.2)).chain(links.iter().map(|link| i64::from(link.2))).max().unwrap_or(0) + 1;
    for (index, (asset, slot, ordinal)) in existing.iter().enumerate() {
        let link = links.iter().find(|link| link.0 == *asset).ok_or(RepositoryError::Storage)?;
        if *slot != link.1 || *ordinal != link.2 {
            tx.execute(&format!("UPDATE {table} SET slot='design_reference',ordinal=?3 WHERE {owner_column}=?1 AND asset_id=?2"), params![owner, asset, offset + i64::try_from(index).map_err(|_| RepositoryError::Storage)?]).map_err(db_error)?;
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
