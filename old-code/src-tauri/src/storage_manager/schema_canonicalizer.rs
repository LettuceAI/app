use rusqlite::{Connection, OptionalExtension};

use crate::sync::v2::is_syncable_table;

/// Outcome of a canonicalization pass.
pub struct CanonicalizeReport {
    /// Tables that were rebuilt into the canonical column layout.
    pub rebuilt: Vec<String>,
    /// Syncable tables not present in the canonical schema, renamed to
    /// `legacy_<name>` so they stop contributing to the sync fingerprint.
    pub renamed_legacy: Vec<String>,
    /// Non-fatal problems (failed index/trigger recreation, FK violations).
    pub warnings: Vec<String>,
}

/// One row of `pragma_table_xinfo`, normalized the same way the sync schema
/// fingerprint normalizes it. Two tables with equal shapes hash identically.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ColumnShape {
    cid: i64,
    name: String,
    column_type: String,
    not_null: i64,
    default_value: Option<String>,
    primary_key: i64,
    hidden: i64,
}

/// Rebuilds every syncable table whose column layout differs from the
/// canonical schema produced by `init_db_connection` on an empty database.
///
/// Devices accumulate schema drift because tables are created two ways: fresh
/// installs get the full `CREATE TABLE`, while upgraded installs get columns
/// appended by `ALTER TABLE` in historical order (some of which failed
/// silently). The sync v2 fingerprint hashes column order and declarations, so
/// drifted devices refuse to replicate with each other even on identical app
/// versions. After this pass, every device converges on the same layout.
///
/// Table contents are preserved: rows are copied column-by-name, columns the
/// device never received are filled with their canonical defaults, and columns
/// absent from the canonical schema are dropped (fresh installs never had
/// them). Local triggers are re-created afterwards; indexes come from the
/// canonical schema.
pub fn canonicalize_schema(conn: &Connection) -> Result<CanonicalizeReport, String> {
    let canonical = Connection::open_in_memory()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    super::db::init_db_connection(&canonical)?;

    let canonical_tables = syncable_table_names(&canonical)?;
    let local_tables = syncable_table_names(conn)?;

    let mut report = CanonicalizeReport {
        rebuilt: Vec::new(),
        renamed_legacy: Vec::new(),
        warnings: Vec::new(),
    };

    let fk_was_on: bool = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap_or(false);
    conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let result = canonicalize_inner(conn, &canonical, &canonical_tables, &local_tables, &mut report);

    let restore = if fk_was_on {
        "PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;"
    } else {
        "PRAGMA legacy_alter_table = OFF;"
    };
    let _ = conn.execute_batch(restore);
    result?;

    if !report.rebuilt.is_empty() || !report.renamed_legacy.is_empty() {
        let violations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_foreign_key_check()",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if violations > 0 {
            report.warnings.push(format!(
                "{violations} foreign key violation(s) present after canonicalization (data kept as-is)"
            ));
        }
    }

    Ok(report)
}

fn canonicalize_inner(
    conn: &Connection,
    canonical: &Connection,
    canonical_tables: &[String],
    local_tables: &[String],
    report: &mut CanonicalizeReport,
) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for table in canonical_tables {
        if !local_tables.iter().any(|name| name == table) {
            // The table never existed here (should not happen in practice);
            // create it empty in canonical form together with its indexes.
            create_from_canonical(&tx, canonical, table)?;
            report.rebuilt.push(table.clone());
            continue;
        }
        if table_shape(&tx, table)? == table_shape(canonical, table)? {
            continue;
        }
        rebuild_table(&tx, canonical, table, report)?;
        report.rebuilt.push(table.clone());
    }

    for table in local_tables {
        if canonical_tables.iter().any(|name| name == table) {
            continue;
        }
        let legacy_name = free_legacy_name(&tx, table)?;
        tx.execute(
            &format!(
                "ALTER TABLE {} RENAME TO {}",
                quote_identifier(table),
                quote_identifier(&legacy_name)
            ),
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        report.renamed_legacy.push(table.clone());
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn syncable_table_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND sql IS NOT NULL
             ORDER BY name",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(names
        .into_iter()
        .filter(|name| is_syncable_table(name))
        .collect())
}

fn table_shape(conn: &Connection, table: &str) -> Result<Vec<ColumnShape>, String> {
    let escaped = table.replace('\'', "''");
    let mut statement = conn
        .prepare(&format!(
            "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden
             FROM pragma_table_xinfo('{escaped}')
             ORDER BY cid"
        ))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let columns = statement
        .query_map([], |row| {
            Ok(ColumnShape {
                cid: row.get(0)?,
                name: row.get(1)?,
                column_type: row.get::<_, String>(2)?.trim().to_ascii_uppercase(),
                not_null: row.get(3)?,
                default_value: row.get(4)?,
                primary_key: row.get(5)?,
                hidden: row.get(6)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(columns)
}

fn canonical_create_sql(canonical: &Connection, table: &str) -> Result<String, String> {
    canonical
        .query_row(
            "SELECT sql FROM sqlite_schema
             WHERE type = 'table' AND name = ?1 AND sql IS NOT NULL",
            [table],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn canonical_index_sql(canonical: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut statement = canonical
        .prepare(
            "SELECT sql FROM sqlite_schema
             WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL
             ORDER BY name",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let sql = statement
        .query_map([table], |row| row.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(sql)
}

fn create_from_canonical(
    conn: &Connection,
    canonical: &Connection,
    table: &str,
) -> Result<(), String> {
    conn.execute(&canonical_create_sql(canonical, table)?, [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for sql in canonical_index_sql(canonical, table)? {
        conn.execute(&sql, [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(())
}

fn rebuild_table(
    conn: &Connection,
    canonical: &Connection,
    table: &str,
    report: &mut CanonicalizeReport,
) -> Result<(), String> {
    let local_shape = table_shape(conn, table)?;
    let canonical_shape = table_shape(canonical, table)?;

    // Triggers and any locally-created indexes are dropped along with the old
    // table; capture their SQL so they can be replayed on the rebuilt table.
    let mut statement = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema
             WHERE tbl_name = ?1 AND type IN ('trigger', 'index') AND sql IS NOT NULL
             ORDER BY type, name",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let local_objects = statement
        .query_map([table], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    drop(statement);

    let tmp_name = format!("{table}__pre_canonical");
    conn.execute(
        &format!("DROP TABLE IF EXISTS {}", quote_identifier(&tmp_name)),
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        &format!(
            "ALTER TABLE {} RENAME TO {}",
            quote_identifier(table),
            quote_identifier(&tmp_name)
        ),
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(&canonical_create_sql(canonical, table)?, [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let target_columns = canonical_shape
        .iter()
        .map(|column| quote_identifier(&column.name))
        .collect::<Vec<_>>()
        .join(", ");
    let select_exprs = canonical_shape
        .iter()
        .map(|column| copy_expression(column, &local_shape))
        .collect::<Vec<_>>()
        .join(", ");
    conn.execute(
        &format!(
            "INSERT INTO {} ({target_columns}) SELECT {select_exprs} FROM {}",
            quote_identifier(table),
            quote_identifier(&tmp_name)
        ),
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        &format!("DROP TABLE {}", quote_identifier(&tmp_name)),
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for sql in canonical_index_sql(canonical, table)? {
        if let Err(error) = conn.execute(&sql, []) {
            report
                .warnings
                .push(format!("index recreation failed on {table}: {error}"));
        }
    }
    for (kind, name, sql) in local_objects {
        let exists: Option<String> = conn
            .query_row(
                "SELECT name FROM sqlite_schema WHERE name = ?1",
                [&name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if exists.is_some() {
            continue;
        }
        if let Err(error) = conn.execute(&sql, []) {
            // An index over a column the canonical schema dropped is expected
            // to fail; anything else is worth surfacing.
            report
                .warnings
                .push(format!("{kind} {name} not restored on {table}: {error}"));
        }
    }
    Ok(())
}

/// Expression that supplies a canonical column's value when copying rows out
/// of the drifted table: the old column when present, otherwise the canonical
/// default, otherwise a type-appropriate zero value for NOT NULL columns.
fn copy_expression(column: &ColumnShape, local_shape: &[ColumnShape]) -> String {
    if local_shape.iter().any(|local| local.name == column.name) {
        return quote_identifier(&column.name);
    }
    if let Some(default) = &column.default_value {
        return default.clone();
    }
    if column.not_null == 0 {
        return "NULL".to_string();
    }
    let column_type = column.column_type.as_str();
    if column_type.contains("INT") {
        "0".to_string()
    } else if column_type.contains("REAL")
        || column_type.contains("FLOA")
        || column_type.contains("DOUB")
    {
        "0.0".to_string()
    } else if column_type.contains("BLOB") {
        "X''".to_string()
    } else {
        "''".to_string()
    }
}

fn free_legacy_name(conn: &Connection, table: &str) -> Result<String, String> {
    let mut candidate = format!("legacy_{table}");
    let mut suffix = 1;
    loop {
        let exists: Option<String> = conn
            .query_row(
                "SELECT name FROM sqlite_schema WHERE name = ?1",
                [&candidate],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if exists.is_none() {
            return Ok(candidate);
        }
        suffix += 1;
        candidate = format!("legacy_{table}_{suffix}");
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::canonicalize_schema;
    use crate::storage_manager::db::init_db_connection;
    use crate::sync::v2::schema_fingerprint;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db_connection(&conn).unwrap();
        conn
    }

    /// Simulates an upgraded install: `characters` created with an old subset
    /// of columns and later columns appended by ALTER in historical order.
    fn drifted() -> Connection {
        let conn = fresh();
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = OFF;
            PRAGMA legacy_alter_table = ON;
            ALTER TABLE characters RENAME TO characters_old;
            CREATE TABLE characters (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              avatar_path TEXT,
              description TEXT,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL
            );
            ALTER TABLE characters ADD COLUMN mode TEXT NOT NULL DEFAULT 'roleplay';
            ALTER TABLE characters ADD COLUMN avatar_crop_x REAL;
            DROP TABLE characters_old;
            PRAGMA legacy_alter_table = OFF;
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, name, description, mode, created_at, updated_at)
             VALUES ('c1', 'Aria', 'desc', 'companion', 11, 22)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn drifted_database_converges_to_fresh_fingerprint() {
        let fresh_conn = fresh();
        let drifted_conn = drifted();
        assert_ne!(
            schema_fingerprint(&fresh_conn).unwrap(),
            schema_fingerprint(&drifted_conn).unwrap()
        );

        let report = canonicalize_schema(&drifted_conn).unwrap();
        assert!(report.rebuilt.contains(&"characters".to_string()));
        assert_eq!(
            schema_fingerprint(&fresh_conn).unwrap(),
            schema_fingerprint(&drifted_conn).unwrap()
        );
    }

    #[test]
    fn rebuild_preserves_rows_and_fills_missing_columns_with_defaults() {
        let conn = drifted();
        canonicalize_schema(&conn).unwrap();

        let (name, description, mode, card_type, voice_autoplay): (
            String,
            String,
            String,
            String,
            i64,
        ) = conn
            .query_row(
                "SELECT name, description, mode, card_type, voice_autoplay
                 FROM characters WHERE id = 'c1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(name, "Aria");
        assert_eq!(description, "desc");
        assert_eq!(mode, "companion");
        assert_eq!(card_type, "circle");
        assert_eq!(voice_autoplay, 0);
    }

    #[test]
    fn canonical_database_is_left_untouched() {
        let conn = fresh();
        let before = schema_fingerprint(&conn).unwrap();
        let report = canonicalize_schema(&conn).unwrap();
        assert!(report.rebuilt.is_empty());
        assert!(report.renamed_legacy.is_empty());
        assert_eq!(schema_fingerprint(&conn).unwrap(), before);
    }

    #[test]
    fn leftover_tables_are_renamed_out_of_the_fingerprint() {
        let fresh_conn = fresh();
        let conn = fresh();
        conn.execute_batch(
            "CREATE TABLE ancient_feature (id TEXT PRIMARY KEY, payload TEXT);
             INSERT INTO ancient_feature VALUES ('a', 'keep me');",
        )
        .unwrap();
        assert_ne!(
            schema_fingerprint(&fresh_conn).unwrap(),
            schema_fingerprint(&conn).unwrap()
        );

        let report = canonicalize_schema(&conn).unwrap();
        assert_eq!(report.renamed_legacy, vec!["ancient_feature".to_string()]);
        assert_eq!(
            schema_fingerprint(&fresh_conn).unwrap(),
            schema_fingerprint(&conn).unwrap()
        );
        let payload: String = conn
            .query_row(
                "SELECT payload FROM legacy_ancient_feature WHERE id = 'a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(payload, "keep me");
    }

    #[test]
    fn local_triggers_survive_a_rebuild() {
        let conn = drifted();
        conn.execute_batch(
            "CREATE TRIGGER characters_touch AFTER UPDATE ON characters
             BEGIN UPDATE characters SET updated_at = 999 WHERE id = NEW.id; END;",
        )
        .unwrap();

        canonicalize_schema(&conn).unwrap();

        let trigger: Option<String> = conn
            .query_row(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'trigger' AND name = 'characters_touch'",
                [],
                |row| row.get(0),
            )
            .ok();
        assert_eq!(trigger.as_deref(), Some("characters_touch"));
        conn.execute("UPDATE characters SET name = 'Aria2' WHERE id = 'c1'", [])
            .unwrap();
        let updated_at: i64 = conn
            .query_row(
                "SELECT updated_at FROM characters WHERE id = 'c1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(updated_at, 999);
    }
}
