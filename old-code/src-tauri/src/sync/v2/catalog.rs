use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableInfo {
    pub name: String,
    pub create_sql: String,
    pub primary_key_columns: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("syncable table {table} has no primary key")]
    MissingPrimaryKey { table: String },
}

const LOCAL_ONLY_TABLES: &[&str] = &[
    "model_pricing_cache",
    "openrouter_provider_pricing_cache",
    "deferred_pricing_refreshes",
];

const SCHEMA_FINGERPRINT_ALGORITHM: &str = "2";

pub fn is_syncable_table(table: &str) -> bool {
    !table.starts_with("sqlite_")
        && !table.starts_with("sync_")
        && !LOCAL_ONLY_TABLES.contains(&table)
}

pub fn syncable_tables(conn: &Connection) -> Result<Vec<TableInfo>, CatalogError> {
    let mut statement = conn.prepare(
        "SELECT name, sql
         FROM sqlite_schema
         WHERE type = 'table' AND sql IS NOT NULL
         ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut tables = Vec::new();
    for row in rows {
        let (name, create_sql) = row?;
        if !is_syncable_table(&name) {
            continue;
        }

        let escaped_name = name.replace('\'', "''");
        let mut pk_statement =
            conn.prepare(&format!("SELECT name, pk FROM pragma_table_info('{escaped_name}')"))?;
        let pk_rows = pk_statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut primary_key_columns = pk_rows
            .filter_map(|row| match row {
                Ok((column, position)) if position > 0 => Some(Ok((position, column))),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        primary_key_columns.sort_by_key(|(position, _)| *position);

        tables.push(TableInfo {
            name,
            create_sql,
            primary_key_columns: primary_key_columns
                .into_iter()
                .map(|(_, column)| column)
                .collect(),
        });
    }
    Ok(tables)
}

pub fn audit_sync_catalog(conn: &Connection) -> Result<Vec<TableInfo>, CatalogError> {
    let tables = syncable_tables(conn)?;
    if let Some(table) = tables
        .iter()
        .find(|table| table.primary_key_columns.is_empty())
    {
        return Err(CatalogError::MissingPrimaryKey {
            table: table.name.clone(),
        });
    }
    Ok(tables)
}

pub fn schema_fingerprint(conn: &Connection) -> Result<String, CatalogError> {
    let tables = audit_sync_catalog(conn)?;
    let mut schema = String::new();
    for table in tables {
        schema.push_str(&table.name);
        schema.push('\0');
        let escaped_name = table.name.replace('\'', "''");
        let mut statement = conn.prepare(&format!(
            "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden
             FROM pragma_table_xinfo('{escaped_name}')
             ORDER BY cid"
        ))?;
        let columns = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        for column in columns {
            let (cid, name, column_type, not_null, default_value, primary_key, hidden) =
                column?;
            schema.push_str(&cid.to_string());
            schema.push('\0');
            schema.push_str(&name);
            schema.push('\0');
            schema.push_str(&column_type.trim().to_ascii_uppercase());
            schema.push('\0');
            schema.push_str(&not_null.to_string());
            schema.push('\0');
            schema.push_str(default_value.as_deref().unwrap_or(""));
            schema.push('\0');
            schema.push_str(&primary_key.to_string());
            schema.push('\0');
            schema.push_str(&hidden.to_string());
            schema.push('\0');
        }
    }
    Ok(blake3::hash(schema.as_bytes()).to_hex().to_string())
}

pub fn cached_schema_fingerprint(conn: &Connection) -> Result<String, CatalogError> {
    let schema_version = conn.query_row("PRAGMA schema_version", [], |row| {
        row.get::<_, i64>(0)
    })?;
    let cached_version = conn
        .query_row(
            "SELECT value FROM sync_v2_local_state WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|value| value.parse::<i64>().ok());
    let cached_fingerprint = conn
        .query_row(
            "SELECT value FROM sync_v2_local_state WHERE key = 'schema_fingerprint'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let cached_algorithm = conn
        .query_row(
            "SELECT value FROM sync_v2_local_state
             WHERE key = 'schema_fingerprint_algorithm'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if cached_version == Some(schema_version)
        && cached_algorithm.as_deref() == Some(SCHEMA_FINGERPRINT_ALGORITHM)
    {
        if let Some(fingerprint) = cached_fingerprint {
            return Ok(fingerprint);
        }
    }

    let fingerprint = schema_fingerprint(conn)?;
    conn.execute(
        "INSERT INTO sync_v2_local_state (key, value)
         VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![schema_version.to_string()],
    )?;
    conn.execute(
        "INSERT INTO sync_v2_local_state (key, value)
         VALUES ('schema_fingerprint', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![fingerprint],
    )?;
    conn.execute(
        "INSERT INTO sync_v2_local_state (key, value)
         VALUES ('schema_fingerprint_algorithm', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_FINGERPRINT_ALGORITHM],
    )?;
    Ok(fingerprint)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{
        audit_sync_catalog, cached_schema_fingerprint, is_syncable_table,
        schema_fingerprint,
    };
    use crate::sync::v2::create_schema;

    #[test]
    fn catalog_includes_user_tables_and_excludes_sync_state_and_caches() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chats (id TEXT PRIMARY KEY, title TEXT NOT NULL);
             CREATE TABLE sync_v2_changes (change_id TEXT PRIMARY KEY);
             CREATE TABLE model_pricing_cache (id TEXT PRIMARY KEY);",
        )
        .unwrap();

        let tables = audit_sync_catalog(&conn).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "chats");
        assert!(is_syncable_table("new_user_feature"));
        assert!(!is_syncable_table("sync_v2_changes"));
    }

    #[test]
    fn fingerprint_changes_when_a_column_is_added() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE chats (id TEXT PRIMARY KEY);")
            .unwrap();
        let before = schema_fingerprint(&conn).unwrap();

        conn.execute_batch("ALTER TABLE chats ADD COLUMN title TEXT;")
            .unwrap();
        let after = schema_fingerprint(&conn).unwrap();

        assert_ne!(before, after);
    }

    #[test]
    fn fingerprint_ignores_equivalent_create_table_formatting() {
        let compact = Connection::open_in_memory().unwrap();
        compact
            .execute_batch("CREATE TABLE chats (id TEXT PRIMARY KEY, title TEXT NOT NULL);")
            .unwrap();
        let formatted = Connection::open_in_memory().unwrap();
        formatted
            .execute_batch(
                "CREATE TABLE chats (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL
                 );",
            )
            .unwrap();

        assert_eq!(
            schema_fingerprint(&compact).unwrap(),
            schema_fingerprint(&formatted).unwrap()
        );
    }

    #[test]
    fn fingerprint_preserves_positional_column_compatibility() {
        let first = Connection::open_in_memory().unwrap();
        first
            .execute_batch(
                "CREATE TABLE chats (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL,
                   updated_at INTEGER NOT NULL
                 );",
            )
            .unwrap();
        let second = Connection::open_in_memory().unwrap();
        second
            .execute_batch(
                "CREATE TABLE chats (
                   id TEXT PRIMARY KEY,
                   updated_at INTEGER NOT NULL,
                   title TEXT NOT NULL
                 );",
            )
            .unwrap();

        assert_ne!(
            schema_fingerprint(&first).unwrap(),
            schema_fingerprint(&second).unwrap()
        );
    }

    #[test]
    fn catalog_rejects_tables_without_primary_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE broken (value TEXT);")
            .unwrap();

        assert!(audit_sync_catalog(&conn).is_err());
    }

    #[test]
    fn cached_fingerprint_invalidates_when_sqlite_schema_changes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE chats (id TEXT PRIMARY KEY);")
            .unwrap();
        create_schema(&conn).unwrap();
        let before = cached_schema_fingerprint(&conn).unwrap();
        assert_eq!(cached_schema_fingerprint(&conn).unwrap(), before);

        conn.execute_batch("ALTER TABLE chats ADD COLUMN title TEXT;")
            .unwrap();
        let after = cached_schema_fingerprint(&conn).unwrap();
        assert_ne!(after, before);
    }

    #[test]
    fn cached_fingerprint_recomputes_when_the_algorithm_changes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE chats (id TEXT PRIMARY KEY);")
            .unwrap();
        create_schema(&conn).unwrap();
        let schema_version = conn
            .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
            .unwrap();
        conn.execute(
            "INSERT INTO sync_v2_local_state (key, value) VALUES ('schema_version', ?1)",
            [schema_version.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sync_v2_local_state (key, value)
             VALUES ('schema_fingerprint', 'obsolete-fingerprint')",
            [],
        )
        .unwrap();

        let fingerprint = cached_schema_fingerprint(&conn).unwrap();
        assert_ne!(fingerprint, "obsolete-fingerprint");
        assert_eq!(
            conn.query_row(
                "SELECT value FROM sync_v2_local_state
                 WHERE key = 'schema_fingerprint_algorithm'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "2"
        );
    }
}
