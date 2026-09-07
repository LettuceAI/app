use std::{path::Path, str::FromStr};

use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LEGACY_PERSONA_PLAN_LIMIT, LegacyCrop, LegacyDatabaseInventory,
    LegacyDatabasePreflightError, LegacyImageRecommendation, LegacyMediaReference,
    LegacyPersonaCandidate, LegacyPersonaPlan,
};
use lettuce_types::{LorebookId, PersonaId, TimestampMillis};
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
    let connection = open_validated(path)?;
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

pub fn plan_legacy_personas(
    path: impl AsRef<Path>,
) -> Result<LegacyPersonaPlan, LegacyDatabasePreflightError> {
    let connection = open_validated(path)?;
    plan_legacy_personas_with_limit(&connection, LEGACY_PERSONA_PLAN_LIMIT)
}

fn open_validated(path: impl AsRef<Path>) -> Result<Connection, LegacyDatabasePreflightError> {
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
    Ok(connection)
}

fn plan_legacy_personas_with_limit(
    connection: &Connection,
    limit: u32,
) -> Result<LegacyPersonaPlan, LegacyDatabasePreflightError> {
    let count = count(connection, "personas", "personas")?;
    if count > u64::from(limit) {
        return Err(LegacyDatabasePreflightError::LimitExceeded {
            table: "personas",
            limit,
        });
    }
    let mut statement = connection
        .prepare(
            "SELECT id,title,description,nickname,avatar_path,avatar_crop_x,avatar_crop_y,avatar_crop_scale,design_description,design_reference_image_ids,active_lorebook_ids,is_default,lora_name,lora_strength,created_at,updated_at FROM personas ORDER BY created_at ASC,id ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<f64>>(5)?,
                row.get::<_, Option<f64>>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<f64>>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, i64>(15)?,
            ))
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let mut personas = Vec::with_capacity(count as usize);
    let mut default_persona_id = None;
    for row in rows {
        let (
            id,
            title,
            description,
            nickname,
            avatar_path,
            crop_x,
            crop_y,
            crop_scale,
            design_description,
            design_reference_image_ids,
            active_lorebook_ids,
            is_default,
            lora_name,
            lora_strength,
            created_at,
            updated_at,
        ) = row.map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
        let id = PersonaId::from_str(&id).map_err(|_| malformed("id"))?;
        require_non_blank(&title, "title")?;
        require_non_blank(&description, "description")?;
        let avatar = avatar_path
            .map(|locator| {
                require_non_blank(&locator, "avatar_path")?;
                Ok(LegacyMediaReference { locator })
            })
            .transpose()?;
        let avatar_crop = legacy_crop(crop_x, crop_y, crop_scale)?;
        let design_references = parse_media_references(design_reference_image_ids)?;
        let active_lorebook_ids = parse_lorebook_ids(&active_lorebook_ids)?;
        let image_recommendation = legacy_image_recommendation(lora_name, lora_strength)?;
        if updated_at < created_at {
            return Err(malformed("timestamps"));
        }
        match is_default {
            0 => {}
            1 if default_persona_id.replace(id).is_none() => {}
            1 => return Err(malformed("is_default")),
            _ => return Err(malformed("is_default")),
        }
        personas.push(LegacyPersonaCandidate {
            id,
            title,
            description,
            nickname,
            avatar,
            avatar_crop,
            design_description,
            design_references,
            image_recommendation,
            active_lorebook_ids,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        });
    }
    Ok(LegacyPersonaPlan {
        personas,
        default_persona_id,
    })
}

fn legacy_crop(
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
) -> Result<Option<LegacyCrop>, LegacyDatabasePreflightError> {
    match (x, y, scale) {
        (None, None, None) => Ok(None),
        (Some(x), Some(y), Some(scale))
            if x.is_finite() && y.is_finite() && scale.is_finite() && scale > 0.0 =>
        {
            Ok(Some(LegacyCrop { x, y, scale }))
        }
        _ => Err(malformed("avatar_crop")),
    }
}

fn parse_media_references(
    value: Option<String>,
) -> Result<Vec<LegacyMediaReference>, LegacyDatabasePreflightError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values: Vec<String> =
        serde_json::from_str(&value).map_err(|_| malformed("design_reference_image_ids"))?;
    values
        .into_iter()
        .map(|locator| {
            require_non_blank(&locator, "design_reference_image_ids")?;
            Ok(LegacyMediaReference { locator })
        })
        .collect()
}

fn parse_lorebook_ids(value: &str) -> Result<Vec<LorebookId>, LegacyDatabasePreflightError> {
    let values: Vec<String> =
        serde_json::from_str(value).map_err(|_| malformed("active_lorebook_ids"))?;
    values
        .into_iter()
        .map(|value| LorebookId::from_str(&value).map_err(|_| malformed("active_lorebook_ids")))
        .collect()
}

fn legacy_image_recommendation(
    name: Option<String>,
    strength: Option<f64>,
) -> Result<Option<LegacyImageRecommendation>, LegacyDatabasePreflightError> {
    match (name, strength) {
        (None, None) => Ok(None),
        (Some(model_name), strength) => {
            require_non_blank(&model_name, "lora_name")?;
            let strength = strength.unwrap_or(0.8);
            if !strength.is_finite() || !(0.0..=2.0).contains(&strength) {
                return Err(malformed("lora_strength"));
            }
            Ok(Some(LegacyImageRecommendation {
                model_name,
                strength,
            }))
        }
        (None, Some(_)) => Err(malformed("lora_strength")),
    }
}

fn require_non_blank(value: &str, field: &'static str) -> Result<(), LegacyDatabasePreflightError> {
    if value.trim().is_empty() {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn malformed(field: &'static str) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MalformedRecord {
        table: "personas",
        field,
    }
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
                 CREATE TABLE personas (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL,
                   description TEXT NOT NULL,
                   nickname TEXT,
                   avatar_path TEXT,
                   avatar_crop_x REAL,
                   avatar_crop_y REAL,
                   avatar_crop_scale REAL,
                   design_description TEXT,
                   design_reference_image_ids TEXT,
                   active_lorebook_ids TEXT NOT NULL DEFAULT '[]',
                   is_default INTEGER NOT NULL DEFAULT 0,
                   lora_name TEXT,
                   lora_strength REAL,
                   created_at INTEGER NOT NULL,
                   updated_at INTEGER NOT NULL
                 );
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
        for table in ["characters", "characters", "sessions"] {
            connection
                .execute(
                    &format!("INSERT INTO {table} (id) VALUES (?1)"),
                    [MediaBlobId::new().to_string()],
                )
                .expect("insert root");
        }
        connection
            .execute(
                "INSERT INTO personas (id,title,description,created_at,updated_at) VALUES (?1,'Reader','Reads stories',10,10)",
                [PersonaId::new().to_string()],
            )
            .expect("insert persona");
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

    #[test]
    fn persona_plan_preserves_fields_default_and_stable_order() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute("DELETE FROM personas", [])
            .expect("clear personas");
        let first_id =
            PersonaId::from_str("00000000-0000-0000-0000-000000000001").expect("first id");
        let second_id =
            PersonaId::from_str("00000000-0000-0000-0000-000000000002").expect("second id");
        let lorebook_id =
            LorebookId::from_str("00000000-0000-0000-0000-000000000003").expect("lorebook id");
        connection
            .execute(
                "INSERT INTO personas (id,title,description,created_at,updated_at) VALUES (?1,'Second','Second description',20,21)",
                [second_id.to_string()],
            )
            .expect("insert second persona");
        connection
            .execute(
                "INSERT INTO personas (id,title,description,nickname,avatar_path,avatar_crop_x,avatar_crop_y,avatar_crop_scale,design_description,design_reference_image_ids,active_lorebook_ids,is_default,lora_name,lora_strength,created_at,updated_at) VALUES (?1,'First','First description','F','avatars/first.png',0.2,0.3,1.4,'Design notes',?2,?3,1,'portrait-style',NULL,10,15)",
                rusqlite::params![
                    first_id.to_string(),
                    "[\"image-ref-1\",\"image-ref-2\"]",
                    format!("[\"{lorebook_id}\"]")
                ],
            )
            .expect("insert first persona");
        drop(connection);

        let plan = plan_legacy_personas(&path).expect("plan personas");

        assert_eq!(plan.default_persona_id, Some(first_id));
        assert_eq!(plan.personas.len(), 2);
        let first = &plan.personas[0];
        assert_eq!(first.id, first_id);
        assert_eq!(first.title, "First");
        assert_eq!(first.description, "First description");
        assert_eq!(first.nickname.as_deref(), Some("F"));
        assert_eq!(
            first
                .avatar
                .as_ref()
                .map(|reference| reference.locator.as_str()),
            Some("avatars/first.png")
        );
        assert_eq!(
            first.avatar_crop,
            Some(LegacyCrop {
                x: 0.2,
                y: 0.3,
                scale: 1.4
            })
        );
        assert_eq!(first.design_description.as_deref(), Some("Design notes"));
        assert_eq!(
            first
                .design_references
                .iter()
                .map(|reference| reference.locator.as_str())
                .collect::<Vec<_>>(),
            ["image-ref-1", "image-ref-2"]
        );
        assert_eq!(
            first.image_recommendation,
            Some(LegacyImageRecommendation {
                model_name: "portrait-style".into(),
                strength: 0.8
            })
        );
        assert_eq!(first.active_lorebook_ids, [lorebook_id]);
        assert_eq!(first.created_at, TimestampMillis::new(10));
        assert_eq!(first.updated_at, TimestampMillis::new(15));
        assert_eq!(plan.personas[1].id, second_id);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn persona_plan_rejects_malformed_fields() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute(
                "UPDATE personas SET design_reference_image_ids = 'not-json'",
                [],
            )
            .expect("corrupt persona");
        drop(connection);

        assert_eq!(
            plan_legacy_personas(&path),
            Err(LegacyDatabasePreflightError::MalformedRecord {
                table: "personas",
                field: "design_reference_image_ids"
            })
        );
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn persona_plan_enforces_its_record_limit_before_loading_rows() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");

        assert_eq!(
            plan_legacy_personas_with_limit(&connection, 0),
            Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "personas",
                limit: 0
            })
        );
        drop(connection);
        std::fs::remove_file(path).expect("remove legacy database");
    }
}
