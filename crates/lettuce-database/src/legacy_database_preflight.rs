use std::path::Path;

use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LegacyDatabaseInventory, LegacyDatabasePreflightError,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

const ROOT_TABLES: [(&str, &str); 10] = [
    ("provider_credentials", "provider_accounts"),
    ("models", "models"),
    ("prompt_templates", "prompts"),
    ("personas", "personas"),
    ("characters", "characters"),
    ("lorebooks", "lorebooks"),
    ("chat_templates", "chat_templates"),
    ("sessions", "direct_conversations"),
    ("group_characters", "group_profiles"),
    ("group_sessions", "group_conversations"),
];

pub fn preflight_legacy_database(
    path: impl AsRef<Path>,
) -> Result<LegacyDatabaseInventory, LegacyDatabasePreflightError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| LegacyDatabasePreflightError::Unavailable)?;
    for (table, _) in ROOT_TABLES {
        require_table(&connection, table)?;
    }
    require_table(&connection, "settings")?;
    let version: Option<i64> = connection
        .query_row(
            "SELECT migration_version FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let version = version.ok_or(LegacyDatabasePreflightError::MissingSettings)?;
    if version != i64::from(LEGACY_DATABASE_SCHEMA_VERSION) {
        return Err(LegacyDatabasePreflightError::UnsupportedVersion {
            found: version,
            supported: LEGACY_DATABASE_SCHEMA_VERSION,
        });
    }

    let counts = ROOT_TABLES
        .map(|(table, label)| count(&connection, table, label))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LegacyDatabaseInventory {
        schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
        provider_accounts: counts[0],
        models: counts[1],
        prompts: counts[2],
        personas: counts[3],
        characters: counts[4],
        lorebooks: counts[5],
        chat_templates: counts[6],
        direct_conversations: counts[7],
        group_profiles: counts[8],
        group_conversations: counts[9],
    })
}

fn require_table(
    connection: &Connection,
    table: &'static str,
) -> Result<(), LegacyDatabasePreflightError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    if exists {
        Ok(())
    } else {
        Err(LegacyDatabasePreflightError::MissingTable { table })
    }
}

fn count(
    connection: &Connection,
    table: &'static str,
    label: &'static str,
) -> Result<u64, LegacyDatabasePreflightError> {
    let sql = format!("SELECT count(*) FROM {table}");
    let value: i64 = connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    checked_count(value, label)
}

fn checked_count(value: i64, table: &'static str) -> Result<u64, LegacyDatabasePreflightError> {
    u64::try_from(value).map_err(|_| LegacyDatabasePreflightError::CountOutOfRange { table })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::MediaBlobId;

    fn legacy_database(version: i64) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("lettuce-legacy-{}.db", MediaBlobId::new()));
        let connection = Connection::open(&path).expect("create legacy database");
        connection
            .execute_batch(
                "CREATE TABLE settings (id INTEGER PRIMARY KEY, migration_version INTEGER NOT NULL);
                 CREATE TABLE provider_credentials (id TEXT PRIMARY KEY);
                 CREATE TABLE models (id TEXT PRIMARY KEY);
                 CREATE TABLE prompt_templates (id TEXT PRIMARY KEY);
                 CREATE TABLE personas (id TEXT PRIMARY KEY);
                 CREATE TABLE characters (id TEXT PRIMARY KEY);
                 CREATE TABLE lorebooks (id TEXT PRIMARY KEY);
                 CREATE TABLE chat_templates (id TEXT PRIMARY KEY);
                 CREATE TABLE sessions (id TEXT PRIMARY KEY);
                 CREATE TABLE group_characters (id TEXT PRIMARY KEY);
                 CREATE TABLE group_sessions (id TEXT PRIMARY KEY);",
            )
            .expect("create legacy schema");
        connection
            .execute(
                "INSERT INTO settings (id, migration_version) VALUES (1, ?1)",
                [version],
            )
            .expect("insert settings");
        for table in ["characters", "characters", "personas", "sessions"] {
            connection
                .execute(
                    &format!("INSERT INTO {table} (id) VALUES (?1)"),
                    [MediaBlobId::new().to_string()],
                )
                .expect("insert root");
        }
        drop(connection);
        path
    }

    #[test]
    fn valid_legacy_database_reports_bounded_root_inventory() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let inventory = preflight_legacy_database(&path).expect("preflight");

        assert_eq!(inventory.schema_version, LEGACY_DATABASE_SCHEMA_VERSION);
        assert_eq!(inventory.characters, 2);
        assert_eq!(inventory.personas, 1);
        assert_eq!(inventory.direct_conversations, 1);
        assert_eq!(inventory.group_conversations, 0);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn unsupported_or_incomplete_legacy_schema_is_rejected() {
        let unsupported = legacy_database(91);
        assert_eq!(
            preflight_legacy_database(&unsupported),
            Err(LegacyDatabasePreflightError::UnsupportedVersion {
                found: 91,
                supported: LEGACY_DATABASE_SCHEMA_VERSION,
            })
        );
        std::fs::remove_file(unsupported).expect("remove unsupported database");

        let incomplete = std::env::temp_dir().join(format!(
            "lettuce-legacy-incomplete-{}.db",
            MediaBlobId::new()
        ));
        drop(Connection::open(&incomplete).expect("create incomplete database"));
        assert_eq!(
            preflight_legacy_database(&incomplete),
            Err(LegacyDatabasePreflightError::MissingTable {
                table: "provider_credentials"
            })
        );
        std::fs::remove_file(incomplete).expect("remove incomplete database");
    }

    #[test]
    fn negative_count_is_rejected_by_the_bounded_conversion() {
        assert_eq!(
            checked_count(-1, "characters"),
            Err(LegacyDatabasePreflightError::CountOutOfRange {
                table: "characters"
            })
        );
    }
}
