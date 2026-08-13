//! SQLite migrations and private adapters for domain-owned repository ports.

#![deny(unsafe_op_in_unsafe_fn)]

use std::{path::Path, str::FromStr, sync::Mutex, time::Duration};

use lettuce_media::{
    BlobState, MediaBlob, MediaBlobRepository, MediaBlobRepositoryError, MediaKind,
};
use lettuce_models::{
    ModelKind, ModelProfile, ModelProfileRepository, ModelRepositoryError, ProviderAccount,
    ProviderAccountRepository, ProviderConfig, ProviderProtocol, SecretHeader,
};
use lettuce_settings::{
    GLOBAL_SETTINGS_FORMAT_VERSION, GlobalSettings, GlobalSettingsStore, GlobalSettingsStoreError,
    SecretRef, StoredGlobalSettings,
};
use lettuce_types::{
    ContentHash, MediaBlobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

const MIGRATION_1: Migration = Migration {
    id: 1,
    sql: include_str!("../migrations/0001_foundation.sql"),
};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database operation failed")]
    Sql(#[from] rusqlite::Error),
    #[error("database lock is unavailable")]
    Lock,
    #[error("applied migration {id} has a different checksum")]
    MigrationChecksum { id: u32 },
}

pub struct Database {
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let mut connection = Connection::open(path)?;
        configure(&connection, true)?;
        apply_migrations(&mut connection, &[MIGRATION_1])?;
        initialize_settings(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let mut connection = Connection::open_in_memory()?;
        configure(&connection, false)?;
        apply_migrations(&mut connection, &[MIGRATION_1])?;
        initialize_settings(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DatabaseError> {
        self.connection.lock().map_err(|_| DatabaseError::Lock)
    }
}

fn configure(connection: &Connection, persistent: bool) -> rusqlite::Result<()> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    if persistent {
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Migration {
    id: u32,
    sql: &'static str,
}

fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in sql.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn apply_migrations(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<(), DatabaseError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
         id INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL\
         ) STRICT;",
    )?;
    for migration in migrations {
        let checksum = migration_checksum(migration.sql);
        let existing = connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE id = ?1",
                [migration.id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != checksum {
                return Err(DatabaseError::MigrationChecksum { id: migration.id });
            }
            continue;
        }
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (id, checksum, applied_at) VALUES (?1, ?2, ?3)",
            params![migration.id, checksum, now()?.get()],
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn initialize_settings(connection: &Connection) -> Result<(), DatabaseError> {
    let now = now()?.get();
    let payload = serde_json::to_string(&GlobalSettings::default())
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    connection.execute(
        "INSERT OR IGNORE INTO app_settings \
         (id, default_model_profile_id, format_version, payload_json, revision, created_at, updated_at) \
         VALUES (1, NULL, ?1, ?2, 1, ?3, ?3)",
        params![GLOBAL_SETTINGS_FORMAT_VERSION, payload, now],
    )?;
    Ok(())
}

fn now() -> Result<TimestampMillis, rusqlite::Error> {
    TimestampMillis::now().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn parse_id<T: FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn to_revision(value: i64) -> Result<Revision, rusqlite::Error> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

fn to_u64(value: i64) -> Result<u64, rusqlite::Error> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn to_i64(value: u64) -> Result<i64, rusqlite::Error> {
    i64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

impl GlobalSettingsStore for Database {
    fn load(&self) -> Result<StoredGlobalSettings, GlobalSettingsStoreError> {
        self.connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?
            .query_row(
                "SELECT default_model_profile_id, format_version, payload_json, revision, created_at, updated_at \
                 FROM app_settings WHERE id = 1",
                [],
                |row| {
                    let format_version: u32 = row.get(1)?;
                    if format_version != GLOBAL_SETTINGS_FORMAT_VERSION {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    let payload: String = row.get(2)?;
                    Ok(StoredGlobalSettings {
                        settings: serde_json::from_str(&payload)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        default_model_profile_id: row
                            .get::<_, Option<String>>(0)?
                            .map(parse_id)
                            .transpose()?,
                        revision: to_revision(row.get(3)?)?,
                        created_at: TimestampMillis::new(row.get(4)?),
                        updated_at: TimestampMillis::new(row.get(5)?),
                    })
                },
            )
            .map_err(|error| match error {
                rusqlite::Error::InvalidQuery => GlobalSettingsStoreError::InvalidData,
                _ => GlobalSettingsStoreError::Storage,
            })
    }

    fn save(
        &self,
        settings: GlobalSettings,
        default_model_profile_id: Option<ModelProfileId>,
        expected_revision: Revision,
    ) -> Result<StoredGlobalSettings, GlobalSettingsStoreError> {
        let payload =
            serde_json::to_string(&settings).map_err(|_| GlobalSettingsStoreError::InvalidData)?;
        let next = expected_revision
            .next()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let changed = self
            .connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?
            .execute(
                "UPDATE app_settings SET default_model_profile_id=?1, payload_json=?2, \
                 revision=?3, updated_at=?4 WHERE id=1 AND revision=?5",
                params![
                    default_model_profile_id.map(|id| id.to_string()),
                    payload,
                    to_i64(next.get()).map_err(|_| GlobalSettingsStoreError::Storage)?,
                    now().map_err(|_| GlobalSettingsStoreError::Storage)?.get(),
                    to_i64(expected_revision.get())
                        .map_err(|_| GlobalSettingsStoreError::Storage)?
                ],
            )
            .map_err(|error| match error {
                rusqlite::Error::SqliteFailure(code, _)
                    if code.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    GlobalSettingsStoreError::ModelProfileMissing
                }
                _ => GlobalSettingsStoreError::Storage,
            })?;
        if changed == 0 {
            return Err(GlobalSettingsStoreError::StaleRevision);
        }
        self.load()
    }
}

fn provider_protocol_name(value: ProviderProtocol) -> &'static str {
    match value {
        ProviderProtocol::OpenAiCompatible => "open_ai_compatible",
        ProviderProtocol::Anthropic => "anthropic",
        ProviderProtocol::Gemini => "gemini",
        ProviderProtocol::Ollama => "ollama",
        ProviderProtocol::LlamaCpp => "llama_cpp",
        ProviderProtocol::StableDiffusion => "stable_diffusion",
    }
}

fn parse_provider_protocol(value: &str) -> rusqlite::Result<ProviderProtocol> {
    match value {
        "open_ai_compatible" => Ok(ProviderProtocol::OpenAiCompatible),
        "anthropic" => Ok(ProviderProtocol::Anthropic),
        "gemini" => Ok(ProviderProtocol::Gemini),
        "ollama" => Ok(ProviderProtocol::Ollama),
        "llama_cpp" => Ok(ProviderProtocol::LlamaCpp),
        "stable_diffusion" => Ok(ProviderProtocol::StableDiffusion),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn model_kind_name(value: ModelKind) -> &'static str {
    match value {
        ModelKind::Chat => "chat",
        ModelKind::Image => "image",
        ModelKind::Embedding => "embedding",
        ModelKind::Speech => "speech",
    }
}

fn parse_model_kind(value: &str) -> rusqlite::Result<ModelKind> {
    match value {
        "chat" => Ok(ModelKind::Chat),
        "image" => Ok(ModelKind::Image),
        "embedding" => Ok(ModelKind::Embedding),
        "speech" => Ok(ModelKind::Speech),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn secret_ref_from_text(value: String) -> rusqlite::Result<SecretRef> {
    uuid::Uuid::parse_str(&value)
        .map(SecretRef::from_uuid)
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

fn provider_from_row(row: &Row<'_>) -> rusqlite::Result<ProviderAccount> {
    let secret_headers: String = row.get(7)?;
    let config: String = row.get(8)?;
    Ok(ProviderAccount {
        id: parse_id(row.get(0)?)?,
        provider_kind: row.get(1)?,
        protocol: parse_provider_protocol(&row.get::<_, String>(2)?)?,
        label: row.get(3)?,
        endpoint: row.get(4)?,
        enabled: row.get(5)?,
        api_key_ref: row
            .get::<_, Option<String>>(6)?
            .map(secret_ref_from_text)
            .transpose()?,
        secret_headers: serde_json::from_str::<Vec<SecretHeader>>(&secret_headers)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        config: serde_json::from_str::<ProviderConfig>(&config)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        revision: to_revision(row.get(9)?)?,
        created_at: TimestampMillis::new(row.get(10)?),
        updated_at: TimestampMillis::new(row.get(11)?),
    })
}

fn model_error(error: rusqlite::Error) -> ModelRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => ModelRepositoryError::InvalidData,
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 787 => {
            ModelRepositoryError::AccountMissing
        }
        rusqlite::Error::SqliteFailure(code, _) if matches!(code.extended_code, 1555 | 2067) => {
            ModelRepositoryError::AlreadyExists
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            ModelRepositoryError::InvalidData
        }
        _ => ModelRepositoryError::Storage,
    }
}

fn validate_account(account: &ProviderAccount) -> Result<(), ModelRepositoryError> {
    if account.provider_kind.trim().is_empty()
        || account.label.trim().is_empty()
        || account.revision.get() == 0
    {
        return Err(ModelRepositoryError::InvalidData);
    }
    Ok(())
}

fn validate_profile(profile: &ModelProfile) -> Result<(), ModelRepositoryError> {
    if profile.external_model_id.trim().is_empty()
        || profile.display_name.trim().is_empty()
        || profile.config.input_modalities.is_empty()
        || profile.config.output_modalities.is_empty()
        || profile.revision.get() == 0
    {
        return Err(ModelRepositoryError::InvalidData);
    }
    Ok(())
}

impl ProviderAccountRepository for Database {
    fn upsert(
        &self,
        account: ProviderAccount,
        expected_revision: Option<Revision>,
    ) -> Result<ProviderAccount, ModelRepositoryError> {
        validate_account(&account)?;
        let headers = serde_json::to_string(&account.secret_headers)
            .map_err(|_| ModelRepositoryError::InvalidData)?;
        let config = serde_json::to_string(&account.config)
            .map_err(|_| ModelRepositoryError::InvalidData)?;
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let changed = if let Some(expected) = expected_revision {
            let next = expected.next().map_err(|_| ModelRepositoryError::Storage)?;
            connection.execute(
                "UPDATE provider_accounts SET provider_kind=?2, protocol=?3, label=?4, endpoint=?5, \
                 enabled=?6, api_key_secret_ref=?7, secret_headers_json=?8, config_json=?9, \
                 revision=?10, updated_at=?11 WHERE id=?1 AND revision=?12",
                params![account.id.to_string(), account.provider_kind, provider_protocol_name(account.protocol),
                    account.label, account.endpoint, account.enabled, account.api_key_ref.map(|v| v.to_string()),
                    headers, config, to_i64(next.get()).map_err(model_error)?, account.updated_at.get(),
                    to_i64(expected.get()).map_err(model_error)?],
            ).map_err(model_error)?
        } else {
            connection.execute(
                "INSERT INTO provider_accounts (id, provider_kind, protocol, label, endpoint, enabled, \
                 api_key_secret_ref, secret_headers_json, config_json, revision, created_at, updated_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![account.id.to_string(), account.provider_kind, provider_protocol_name(account.protocol),
                    account.label, account.endpoint, account.enabled, account.api_key_ref.map(|v| v.to_string()),
                    headers, config, to_i64(account.revision.get()).map_err(model_error)?,
                    account.created_at.get(), account.updated_at.get()],
            ).map_err(model_error)?
        };
        if changed == 0 {
            return Err(ModelRepositoryError::StaleRevision);
        }
        connection
            .query_row(
                "SELECT id, provider_kind, protocol, label, endpoint, enabled, api_key_secret_ref, \
                 secret_headers_json, config_json, revision, created_at, updated_at \
                 FROM provider_accounts WHERE id=?1",
                [account.id.to_string()],
                provider_from_row,
            )
            .optional()
            .map_err(model_error)?
            .ok_or(ModelRepositoryError::NotFound)
    }

    fn get(&self, id: ProviderAccountId) -> Result<Option<ProviderAccount>, ModelRepositoryError> {
        self.connection()
            .map_err(|_| ModelRepositoryError::Storage)?
            .query_row(
                "SELECT id, provider_kind, protocol, label, endpoint, enabled, api_key_secret_ref, \
                 secret_headers_json, config_json, revision, created_at, updated_at \
                 FROM provider_accounts WHERE id=?1",
                [id.to_string()],
                provider_from_row,
            )
            .optional()
            .map_err(model_error)
    }

    fn delete(&self, id: ProviderAccountId) -> Result<(), ModelRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let dependencies = connection
            .prepare("SELECT id FROM model_profiles WHERE provider_account_id=?1 ORDER BY id")
            .and_then(|mut statement| {
                statement
                    .query_map([id.to_string()], |row| parse_id(row.get(0)?))?
                    .collect::<rusqlite::Result<Vec<ModelProfileId>>>()
            })
            .map_err(model_error)?;
        if !dependencies.is_empty() {
            return Err(ModelRepositoryError::AccountInUse(dependencies));
        }
        let changed = connection
            .execute(
                "DELETE FROM provider_accounts WHERE id=?1",
                [id.to_string()],
            )
            .map_err(model_error)?;
        if changed == 0 {
            return Err(ModelRepositoryError::NotFound);
        }
        Ok(())
    }

    fn delete_with_profiles(&self, id: ProviderAccountId) -> Result<(), ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection.transaction().map_err(model_error)?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM provider_accounts WHERE id=?1",
                [id.to_string()],
                |_| Ok(()),
            )
            .optional()
            .map_err(model_error)?
            .is_some();
        if !exists {
            return Err(ModelRepositoryError::NotFound);
        }
        transaction
            .execute(
                "UPDATE app_settings SET default_model_profile_id=NULL, revision=revision+1, updated_at=?2 \
                 WHERE id=1 AND default_model_profile_id IN \
                 (SELECT id FROM model_profiles WHERE provider_account_id=?1)",
                params![id.to_string(), now().map_err(model_error)?.get()],
            )
            .map_err(model_error)?;
        transaction
            .execute(
                "DELETE FROM model_profiles WHERE provider_account_id=?1",
                [id.to_string()],
            )
            .map_err(model_error)?;
        transaction
            .execute(
                "DELETE FROM provider_accounts WHERE id=?1",
                [id.to_string()],
            )
            .map_err(model_error)?;
        transaction.commit().map_err(model_error)
    }
}

fn model_from_row(row: &Row<'_>) -> rusqlite::Result<ModelProfile> {
    let config: String = row.get(5)?;
    Ok(ModelProfile {
        id: parse_id(row.get(0)?)?,
        provider_account_id: parse_id(row.get(1)?)?,
        external_model_id: row.get(2)?,
        display_name: row.get(3)?,
        kind: parse_model_kind(&row.get::<_, String>(4)?)?,
        config: serde_json::from_str(&config).map_err(|_| rusqlite::Error::InvalidQuery)?,
        revision: to_revision(row.get(6)?)?,
        created_at: TimestampMillis::new(row.get(7)?),
        updated_at: TimestampMillis::new(row.get(8)?),
    })
}

impl ModelProfileRepository for Database {
    fn upsert(
        &self,
        profile: ModelProfile,
        expected_revision: Option<Revision>,
    ) -> Result<ModelProfile, ModelRepositoryError> {
        validate_profile(&profile)?;
        let config = serde_json::to_string(&profile.config)
            .map_err(|_| ModelRepositoryError::InvalidData)?;
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let changed = if let Some(expected) = expected_revision {
            let next = expected.next().map_err(|_| ModelRepositoryError::Storage)?;
            connection.execute(
                "UPDATE model_profiles SET provider_account_id=?2, external_model_id=?3, display_name=?4, \
                 kind=?5, config_json=?6, revision=?7, updated_at=?8 WHERE id=?1 AND revision=?9",
                params![profile.id.to_string(), profile.provider_account_id.to_string(), profile.external_model_id,
                    profile.display_name, model_kind_name(profile.kind), config, to_i64(next.get()).map_err(model_error)?,
                    profile.updated_at.get(), to_i64(expected.get()).map_err(model_error)?],
            ).map_err(model_error)?
        } else {
            connection.execute(
                "INSERT INTO model_profiles (id, provider_account_id, external_model_id, display_name, kind, \
                 config_json, revision, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![profile.id.to_string(), profile.provider_account_id.to_string(), profile.external_model_id,
                    profile.display_name, model_kind_name(profile.kind), config,
                    to_i64(profile.revision.get()).map_err(model_error)?, profile.created_at.get(), profile.updated_at.get()],
            ).map_err(model_error)?
        };
        if changed == 0 {
            return Err(ModelRepositoryError::StaleRevision);
        }
        connection
            .query_row(
                "SELECT id, provider_account_id, external_model_id, display_name, kind, config_json, \
                 revision, created_at, updated_at FROM model_profiles WHERE id=?1",
                [profile.id.to_string()],
                model_from_row,
            )
            .optional()
            .map_err(model_error)?
            .ok_or(ModelRepositoryError::NotFound)
    }

    fn get(&self, id: ModelProfileId) -> Result<Option<ModelProfile>, ModelRepositoryError> {
        self.connection().map_err(|_| ModelRepositoryError::Storage)?
            .query_row("SELECT id, provider_account_id, external_model_id, display_name, kind, config_json, \
                        revision, created_at, updated_at FROM model_profiles WHERE id=?1",
                       [id.to_string()], model_from_row).optional().map_err(model_error)
    }

    fn delete_and_clear_default(&self, id: ModelProfileId) -> Result<(), ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection.transaction().map_err(model_error)?;
        transaction
            .execute(
                "UPDATE app_settings SET default_model_profile_id=NULL, revision=revision+1, \
                             updated_at=?2 WHERE id=1 AND default_model_profile_id=?1",
                params![id.to_string(), now().map_err(model_error)?.get()],
            )
            .map_err(model_error)?;
        let changed = transaction
            .execute("DELETE FROM model_profiles WHERE id=?1", [id.to_string()])
            .map_err(model_error)?;
        if changed == 0 {
            return Err(ModelRepositoryError::NotFound);
        }
        transaction.commit().map_err(model_error)
    }
}

fn media_kind_name(value: MediaKind) -> &'static str {
    match value {
        MediaKind::Image => "image",
        MediaKind::Audio => "audio",
        MediaKind::Video => "video",
    }
}
fn blob_state_name(value: BlobState) -> &'static str {
    match value {
        BlobState::Staged => "staged",
        BlobState::Ready => "ready",
        BlobState::Quarantined => "quarantined",
        BlobState::Missing => "missing",
    }
}
fn media_from_row(row: &Row<'_>) -> rusqlite::Result<MediaBlob> {
    let kind = match row.get::<_, String>(2)?.as_str() {
        "image" => MediaKind::Image,
        "audio" => MediaKind::Audio,
        "video" => MediaKind::Video,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let state = match row.get::<_, String>(9)?.as_str() {
        "staged" => BlobState::Staged,
        "ready" => BlobState::Ready,
        "quarantined" => BlobState::Quarantined,
        "missing" => BlobState::Missing,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(MediaBlob {
        id: parse_id(row.get(0)?)?,
        content_hash: row
            .get::<_, String>(1)?
            .parse()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        kind,
        mime_type: row.get(3)?,
        byte_size: to_u64(row.get(4)?)?,
        width: row
            .get::<_, Option<i64>>(5)?
            .map(u32::try_from)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        height: row
            .get::<_, Option<i64>>(6)?
            .map(u32::try_from)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        duration_ms: row
            .get::<_, Option<i64>>(7)?
            .map(u64::try_from)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        validation_version: row.get(8)?,
        state,
        created_at: TimestampMillis::new(row.get(10)?),
        updated_at: TimestampMillis::new(row.get(11)?),
    })
}
fn media_error(error: rusqlite::Error) -> MediaBlobRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => MediaBlobRepositoryError::InvalidData,
        _ => MediaBlobRepositoryError::Storage,
    }
}

impl MediaBlobRepository for Database {
    fn register(&self, blob: MediaBlob) -> Result<MediaBlob, MediaBlobRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| MediaBlobRepositoryError::Storage)?;
        connection.execute("INSERT INTO media_blobs (id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(content_hash) DO NOTHING",
            params![blob.id.to_string(), blob.content_hash.as_str(), media_kind_name(blob.kind), blob.mime_type, to_i64(blob.byte_size).map_err(media_error)?, blob.width, blob.height,
                blob.duration_ms.map(to_i64).transpose().map_err(media_error)?, blob.validation_version, blob_state_name(blob.state), blob.created_at.get(), blob.updated_at.get()]).map_err(media_error)?;
        let stored = connection.query_row("SELECT id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at FROM media_blobs WHERE content_hash=?1", [blob.content_hash.as_str()], media_from_row).map_err(media_error)?;
        Ok(stored)
    }
    fn get(&self, id: MediaBlobId) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
        self.connection().map_err(|_| MediaBlobRepositoryError::Storage)?.query_row("SELECT id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at FROM media_blobs WHERE id=?1", [id.to_string()], media_from_row).optional().map_err(media_error)
    }
    fn find_by_hash(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
        self.connection().map_err(|_| MediaBlobRepositoryError::Storage)?.query_row("SELECT id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at FROM media_blobs WHERE content_hash=?1", [hash.as_str()], media_from_row).optional().map_err(media_error)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_media::{BlobState, MediaBlob, MediaBlobRepository, MediaKind};
    use lettuce_models::{
        CustomAuth, Modality, ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository,
        ModelRepositoryError, ProviderAccount, ProviderAccountRepository, ProviderConfig,
        ProviderProtocol, SecretHeader,
    };
    use lettuce_settings::{
        GlobalSettingsStore, HeaderName, SecretOwnerId, SecretPurpose, SecretRef,
    };
    use lettuce_types::{
        ContentHash, MediaBlobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
    };

    use super::{Database, DatabaseError, Migration, apply_migrations};

    fn provider() -> ProviderAccount {
        let id = ProviderAccountId::new();
        ProviderAccount {
            id,
            provider_kind: "openrouter".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            label: "OpenRouter".into(),
            endpoint: Some("https://openrouter.ai/api/v1".into()),
            enabled: true,
            api_key_ref: Some(SecretRef::new()),
            secret_headers: vec![SecretHeader {
                name: HeaderName::new("X-Private").expect("valid header"),
                secret_ref: SecretRef::new(),
            }],
            config: ProviderConfig::Custom {
                chat_path: "/chat/completions".into(),
                models_path: Some("/models".into()),
                streaming: true,
                auth: CustomAuth::Bearer,
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        }
    }

    fn profile(account: ProviderAccountId) -> ModelProfile {
        ModelProfile {
            id: ModelProfileId::new(),
            provider_account_id: account,
            external_model_id: "example/model".into(),
            display_name: "Example".into(),
            kind: ModelKind::Chat,
            config: ModelProfileConfig {
                input_modalities: vec![Modality::Text, Modality::Image],
                output_modalities: vec![Modality::Text],
                temperature: Some(0.8),
                context_length: Some(32_768),
                max_output_tokens: Some(2_048),
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(20),
            updated_at: TimestampMillis::new(20),
        }
    }

    #[test]
    fn migration_is_idempotent_and_checksum_protected() {
        let database = Database::open_in_memory().expect("open database");
        let mut connection = database.connection().expect("database lock");
        apply_migrations(&mut connection, &[super::MIGRATION_1]).expect("repeat migration");
        let count: u32 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(count, 1);

        let changed = Migration {
            id: 1,
            sql: "SELECT 1;",
        };
        assert!(matches!(
            apply_migrations(&mut connection, &[changed]),
            Err(DatabaseError::MigrationChecksum { id: 1 })
        ));
    }

    #[test]
    fn account_profile_and_default_obey_foreign_keys() {
        let database = Database::open_in_memory().expect("open database");
        let account =
            ProviderAccountRepository::upsert(&database, provider(), None).expect("insert account");
        let profile = ModelProfileRepository::upsert(&database, profile(account.id), None)
            .expect("insert profile");

        let settings = GlobalSettingsStore::load(&database).expect("load settings");
        GlobalSettingsStore::save(
            &database,
            settings.settings,
            Some(profile.id),
            settings.revision,
        )
        .expect("select default");

        assert!(matches!(
            ProviderAccountRepository::delete(&database, account.id),
            Err(ModelRepositoryError::AccountInUse(ids)) if ids == vec![profile.id]
        ));
        ModelProfileRepository::delete_and_clear_default(&database, profile.id)
            .expect("delete profile");
        assert_eq!(
            GlobalSettingsStore::load(&database)
                .expect("load settings")
                .default_model_profile_id,
            None
        );
        ProviderAccountRepository::delete(&database, account.id).expect("delete unused account");
    }

    #[test]
    fn provider_round_trip_contains_references_but_not_secret_values() {
        let database = Database::open_in_memory().expect("open database");
        let expected = provider();
        let stored = ProviderAccountRepository::upsert(&database, expected.clone(), None)
            .expect("insert account");
        assert_eq!(stored, expected);

        let connection = database.connection().expect("database lock");
        let columns = connection
            .prepare("PRAGMA table_info(provider_accounts)")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("inspect schema");
        let forbidden = [
            "api_key",
            "ciphertext",
            "nonce",
            "key_reference",
            "header_value",
        ];
        assert!(
            forbidden
                .iter()
                .all(|name| !columns.iter().any(|column| column == name))
        );
        assert!(columns.iter().any(|column| column == "api_key_secret_ref"));
        drop(connection);

        let purpose = SecretPurpose::ProviderApiKey {
            owner: SecretOwnerId::from_uuid(expected.id.as_uuid()),
        };
        assert!(matches!(purpose, SecretPurpose::ProviderApiKey { .. }));

        assert!(matches!(
            ProviderAccountRepository::upsert(&database, expected, None),
            Err(ModelRepositoryError::AlreadyExists)
        ));
    }

    #[test]
    fn missing_account_and_invalid_domain_values_are_distinct() {
        let database = Database::open_in_memory().expect("open database");
        let missing = ProviderAccountId::new();
        assert_eq!(
            ModelProfileRepository::upsert(&database, profile(missing), None),
            Err(ModelRepositoryError::AccountMissing)
        );

        let mut invalid = provider();
        invalid.provider_kind = "  ".into();
        assert_eq!(
            ProviderAccountRepository::upsert(&database, invalid, None),
            Err(ModelRepositoryError::InvalidData)
        );
        let mut invalid_profile = profile(missing);
        invalid_profile.config.input_modalities.clear();
        assert_eq!(
            ModelProfileRepository::upsert(&database, invalid_profile, None),
            Err(ModelRepositoryError::InvalidData)
        );
    }

    #[test]
    fn account_and_profiles_are_deleted_atomically() {
        let database = Database::open_in_memory().expect("open database");
        let account =
            ProviderAccountRepository::upsert(&database, provider(), None).expect("insert account");
        let profile = ModelProfileRepository::upsert(&database, profile(account.id), None)
            .expect("insert profile");
        let settings = GlobalSettingsStore::load(&database).expect("load settings");
        GlobalSettingsStore::save(
            &database,
            settings.settings,
            Some(profile.id),
            settings.revision,
        )
        .expect("select profile");

        ProviderAccountRepository::delete_with_profiles(&database, account.id)
            .expect("delete graph");
        assert_eq!(
            ProviderAccountRepository::get(&database, account.id).expect("read account"),
            None
        );
        assert_eq!(
            ModelProfileRepository::get(&database, profile.id).expect("read profile"),
            None
        );
        assert_eq!(
            GlobalSettingsStore::load(&database)
                .expect("load settings")
                .default_model_profile_id,
            None
        );
    }

    #[test]
    fn media_registration_deduplicates_content() {
        let database = Database::open_in_memory().expect("open database");
        let hash = ContentHash::parse("ab".repeat(32)).expect("valid hash");
        let first = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: hash.clone(),
            kind: MediaKind::Image,
            mime_type: "image/webp".into(),
            byte_size: 42,
            width: Some(4),
            height: Some(5),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Ready,
            created_at: TimestampMillis::new(30),
            updated_at: TimestampMillis::new(30),
        };
        let stored =
            MediaBlobRepository::register(&database, first.clone()).expect("register first");
        let mut duplicate = first;
        duplicate.id = MediaBlobId::new();
        let deduplicated =
            MediaBlobRepository::register(&database, duplicate).expect("register duplicate");
        assert_eq!(deduplicated.id, stored.id);
        assert_eq!(
            MediaBlobRepository::find_by_hash(&database, &hash)
                .expect("find blob")
                .map(|blob| blob.id),
            Some(stored.id)
        );

        let collision = MediaBlob {
            id: stored.id,
            content_hash: ContentHash::parse("cd".repeat(32)).expect("valid hash"),
            kind: MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: 99,
            width: Some(2),
            height: Some(2),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Ready,
            created_at: TimestampMillis::new(31),
            updated_at: TimestampMillis::new(31),
        };
        assert!(MediaBlobRepository::register(&database, collision).is_err());
    }

    #[test]
    fn database_enables_foreign_keys_and_has_only_five_foundation_tables() {
        let database = Database::open_in_memory().expect("open database");
        let connection = database.connection().expect("database lock");
        let foreign_keys: u8 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign key pragma");
        assert_eq!(foreign_keys, 1);
        let tables = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("list tables");
        assert_eq!(
            tables,
            vec![
                "app_settings",
                "media_blobs",
                "model_profiles",
                "provider_accounts",
                "schema_migrations"
            ]
        );
    }
}
