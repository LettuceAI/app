//! SQLite migrations and private adapters for domain-owned repository ports.

#![deny(unsafe_op_in_unsafe_fn)]

mod character_adapter;
mod conversation_artifact_adapter;
mod conversation_vertical_slice;
mod group_adapter;
mod lorebook_adapter;
mod persona_adapter;
mod prompt_adapter;

use std::{path::Path, str::FromStr, sync::Mutex, time::Duration};

use lettuce_media::{
    AssetKind, AssetOrigin, BlobState, MediaAsset, MediaAssetRepository, MediaAssetRepositoryError,
    MediaBlob, MediaBlobRepository, MediaBlobRepositoryError, MediaKind, RetentionClass,
};
use lettuce_models::{
    ModelDependencyReference, ModelKind, ModelProfile, ModelProfileRepository,
    ModelRepositoryError, ProviderAccount, ProviderAccountRepository, ProviderConfig,
    ProviderProtocol, SecretHeader,
};
use lettuce_settings::{
    GLOBAL_SETTINGS_FORMAT_VERSION, GlobalSettings, GlobalSettingsStore, GlobalSettingsStoreError,
    SecretRef, StoredGlobalSettings,
};
use lettuce_types::{
    AssetId, ContentHash, MediaBlobId, ModelProfileId, Page, PageRequest, ProviderAccountId,
    Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

const MIGRATION_1: Migration = Migration {
    id: 1,
    sql: include_str!("../migrations/0001_foundation.sql"),
};

const MIGRATION_2: Migration = Migration {
    id: 2,
    sql: include_str!("../migrations/0002_media_assets.sql"),
};

const MIGRATION_3: Migration = Migration {
    id: 3,
    sql: include_str!("../migrations/0003_characters.sql"),
};

const MIGRATION_4: Migration = Migration {
    id: 4,
    sql: include_str!("../migrations/0004_personas.sql"),
};

const MIGRATION_5: Migration = Migration {
    id: 5,
    sql: include_str!("../migrations/0005_groups.sql"),
};

const MIGRATION_6: Migration = Migration {
    id: 6,
    sql: include_str!("../migrations/0006_context.sql"),
};

const MIGRATION_7: Migration = Migration {
    id: 7,
    sql: include_str!("../migrations/0007_builtin_prompt_entries.sql"),
};

const MIGRATION_8: Migration = Migration {
    id: 8,
    sql: include_str!("../migrations/0008_conversations.sql"),
};

const PROVIDER_CONFIG_FORMAT_VERSION: u32 = 1;
const MODEL_PROFILE_CONFIG_FORMAT_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionedJson<T> {
    format_version: u32,
    value: T,
}

fn encode_versioned<T: serde::Serialize>(
    value: &T,
    format_version: u32,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&VersionedJson {
        format_version,
        value,
    })
}

fn decode_versioned<T: serde::de::DeserializeOwned>(
    payload: &str,
    expected_format_version: u32,
) -> Result<T, ()> {
    let document = serde_json::from_str::<VersionedJson<T>>(payload).map_err(|_| ())?;
    if document.format_version != expected_format_version {
        return Err(());
    }
    Ok(document.value)
}

fn decode_provider_config(payload: &str) -> Result<ProviderConfig, ()> {
    let value: serde_json::Value = decode_versioned(payload, PROVIDER_CONFIG_FORMAT_VERSION)?;
    let config = serde_json::from_value::<ProviderConfig>(value.clone()).map_err(|_| ())?;
    let allowed_fields = match config {
        ProviderConfig::Standard => &["kind"][..],
        ProviderConfig::Custom { .. } => {
            &["kind", "chat_path", "models_path", "streaming", "auth"][..]
        }
    };
    let object = value.as_object().ok_or(())?;
    if object
        .keys()
        .all(|field| allowed_fields.contains(&field.as_str()))
    {
        Ok(config)
    } else {
        Err(())
    }
}

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
        apply_migrations(
            &mut connection,
            &[
                MIGRATION_1,
                MIGRATION_2,
                MIGRATION_3,
                MIGRATION_4,
                MIGRATION_5,
                MIGRATION_6,
                MIGRATION_7,
                MIGRATION_8,
            ],
        )?;
        initialize_settings(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let mut connection = Connection::open_in_memory()?;
        configure(&connection, false)?;
        apply_migrations(
            &mut connection,
            &[
                MIGRATION_1,
                MIGRATION_2,
                MIGRATION_3,
                MIGRATION_4,
                MIGRATION_5,
                MIGRATION_6,
                MIGRATION_7,
                MIGRATION_8,
            ],
        )?;
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
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
         id INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL\
         ) STRICT;",
    )?;
    for migration in migrations {
        let checksum = migration_checksum(migration.sql);
        let existing = transaction
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
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (id, checksum, applied_at) VALUES (?1, ?2, ?3)",
            params![migration.id, checksum, now()?.get()],
        )?;
    }
    transaction.commit()?;
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
        config: decode_provider_config(&config).map_err(|_| rusqlite::Error::InvalidQuery)?,
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

fn model_dependency_sort_key(value: &ModelDependencyReference) -> (u8, String, String) {
    match value {
        ModelDependencyReference::CharacterDefault { character_id } => {
            (0, character_id.to_string(), String::new())
        }
        ModelDependencyReference::GroupMemberOverride {
            group_id,
            character_id,
        } => (1, group_id.to_string(), character_id.to_string()),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteFailurePoint {
    None,
    AfterProfilesDelete,
    AfterProfileDelete,
}

fn fail_delete_at(
    failure_point: DeleteFailurePoint,
    checkpoint: DeleteFailurePoint,
) -> Result<(), ModelRepositoryError> {
    if failure_point == checkpoint {
        return Err(ModelRepositoryError::Storage);
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
        let config = encode_versioned(&account.config, PROVIDER_CONFIG_FORMAT_VERSION)
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
        self.delete_with_profiles_inner(id, DeleteFailurePoint::None)
    }
}

impl Database {
    fn delete_with_profiles_inner(
        &self,
        id: ProviderAccountId,
        failure_point: DeleteFailurePoint,
    ) -> Result<(), ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(model_error)?;
        let dependencies = transaction
            .prepare(
                "SELECT characters.id FROM characters \
                 JOIN model_profiles ON model_profiles.id=characters.model_profile_id \
                 WHERE model_profiles.provider_account_id=?1 ORDER BY characters.id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([id.to_string()], |row| {
                        Ok(ModelDependencyReference::CharacterDefault {
                            character_id: row
                                .get::<_, String>(0)?
                                .parse()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(model_error)?;
        let mut dependencies = dependencies;
        let group_dependencies = transaction
            .prepare(
                "SELECT group_id,character_id FROM group_members WHERE model_profile_override_id IN \
                 (SELECT id FROM model_profiles WHERE provider_account_id=?1) ORDER BY group_id,character_id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([id.to_string()], |row| {
                        Ok(ModelDependencyReference::GroupMemberOverride {
                            group_id: row
                                .get::<_, String>(0)?
                                .parse()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            character_id: row
                                .get::<_, String>(1)?
                                .parse()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(model_error)?;
        dependencies.extend(group_dependencies);
        dependencies.sort_by_key(model_dependency_sort_key);
        if !dependencies.is_empty() {
            return Err(ModelRepositoryError::InUse(dependencies));
        }
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
        fail_delete_at(failure_point, DeleteFailurePoint::AfterProfilesDelete)?;
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
        config: decode_versioned(&config, MODEL_PROFILE_CONFIG_FORMAT_VERSION)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
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
        let config = encode_versioned(&profile.config, MODEL_PROFILE_CONFIG_FORMAT_VERSION)
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
        self.delete_and_clear_default_inner(id, DeleteFailurePoint::None)
    }
}

impl Database {
    fn delete_and_clear_default_inner(
        &self,
        id: ModelProfileId,
        failure_point: DeleteFailurePoint,
    ) -> Result<(), ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(model_error)?;
        let dependencies = transaction
            .prepare("SELECT id FROM characters WHERE model_profile_id=?1 ORDER BY id")
            .and_then(|mut statement| {
                statement
                    .query_map([id.to_string()], |row| {
                        Ok(ModelDependencyReference::CharacterDefault {
                            character_id: row
                                .get::<_, String>(0)?
                                .parse()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(model_error)?;
        let mut dependencies = dependencies;
        let group_dependencies = transaction
            .prepare(
                "SELECT group_id,character_id FROM group_members WHERE model_profile_override_id=?1 ORDER BY group_id,character_id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([id.to_string()], |row| {
                        Ok(ModelDependencyReference::GroupMemberOverride {
                            group_id: row
                                .get::<_, String>(0)?
                                .parse()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            character_id: row
                                .get::<_, String>(1)?
                                .parse()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(model_error)?;
        dependencies.extend(group_dependencies);
        dependencies.sort_by_key(model_dependency_sort_key);
        if !dependencies.is_empty() {
            return Err(ModelRepositoryError::InUse(dependencies));
        }
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
        fail_delete_at(failure_point, DeleteFailurePoint::AfterProfileDelete)?;
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

const MEDIA_BLOB_COLUMNS: &str = "id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at";

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
    let blob = MediaBlob {
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
        validation_version: u32::try_from(row.get::<_, i64>(8)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        state,
        created_at: TimestampMillis::new(row.get(10)?),
        updated_at: TimestampMillis::new(row.get(11)?),
    };
    blob.validate().map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(blob)
}
fn media_error(error: rusqlite::Error) -> MediaBlobRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => MediaBlobRepositoryError::InvalidData,
        _ => MediaBlobRepositoryError::Storage,
    }
}

impl MediaBlobRepository for Database {
    fn register(&self, blob: MediaBlob) -> Result<MediaBlob, MediaBlobRepositoryError> {
        blob.validate()
            .map_err(|_| MediaBlobRepositoryError::InvalidData)?;
        let mut connection = self
            .connection()
            .map_err(|_| MediaBlobRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(media_error)?;
        let by_id = transaction
            .query_row(
                &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE id=?1"),
                [blob.id.to_string()],
                media_from_row,
            )
            .optional()
            .map_err(media_error)?;
        let by_hash = transaction
            .query_row(
                &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE content_hash=?1"),
                [blob.content_hash.as_str()],
                media_from_row,
            )
            .optional()
            .map_err(media_error)?;

        if let Some(existing) = by_hash {
            if existing.content_hash != blob.content_hash
                || existing.kind != blob.kind
                || existing.mime_type != blob.mime_type
                || existing.byte_size != blob.byte_size
                || existing.width != blob.width
                || existing.height != blob.height
                || existing.duration_ms != blob.duration_ms
                || existing.validation_version != blob.validation_version
            {
                return Err(MediaBlobRepositoryError::InvalidData);
            }
            if let Some(existing_by_id) = by_id {
                if existing_by_id.content_hash != existing.content_hash {
                    return Err(MediaBlobRepositoryError::InvalidData);
                }
            }
            return Ok(existing);
        }

        if by_id.is_some() {
            return Err(MediaBlobRepositoryError::InvalidData);
        }

        transaction
            .execute(
                "INSERT INTO media_blobs (id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    blob.id.to_string(),
                    blob.content_hash.as_str(),
                    media_kind_name(blob.kind),
                    blob.mime_type,
                    to_i64(blob.byte_size).map_err(media_error)?,
                    blob.width.map(i64::from),
                    blob.height.map(i64::from),
                    blob.duration_ms
                        .map(to_i64)
                        .transpose()
                        .map_err(media_error)?,
                    i64::from(blob.validation_version),
                    blob_state_name(blob.state),
                    blob.created_at.get(),
                    blob.updated_at.get()
                ],
            )
            .map_err(media_error)?;
        let stored = transaction
            .query_row(
                &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE id=?1"),
                [blob.id.to_string()],
                media_from_row,
            )
            .map_err(media_error)?;
        transaction.commit().map_err(media_error)?;
        Ok(stored)
    }
    fn get(&self, id: MediaBlobId) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
        self.connection()
            .map_err(|_| MediaBlobRepositoryError::Storage)?
            .query_row(
                &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE id=?1"),
                [id.to_string()],
                media_from_row,
            )
            .optional()
            .map_err(media_error)
    }
    fn find_by_hash(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
        self.connection()
            .map_err(|_| MediaBlobRepositoryError::Storage)?
            .query_row(
                &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE content_hash=?1"),
                [hash.as_str()],
                media_from_row,
            )
            .optional()
            .map_err(media_error)
    }
}

const MEDIA_ASSET_COLUMNS: &str = "id, blob_id, blob_kind, kind, origin, retention, expires_at, provenance_json, revision, created_at, updated_at";
const ASSET_CURSOR_FORMAT_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryCursor {
    format_version: u32,
    updated_at: i64,
    id: String,
}

fn asset_kind_name(value: AssetKind) -> &'static str {
    match value {
        AssetKind::AvatarOriginal => "avatar_original",
        AssetKind::BackgroundImage => "background_image",
        AssetKind::Illustration => "illustration",
        AssetKind::LorebookIcon => "lorebook_icon",
        AssetKind::MessageImage => "message_image",
        AssetKind::MessageAudio => "message_audio",
        AssetKind::GeneratedImage => "generated_image",
        AssetKind::SynthesizedSpeech => "synthesized_speech",
        AssetKind::OtherImage => "other_image",
        AssetKind::OtherAudio => "other_audio",
    }
}

fn asset_kind_from_name(value: &str) -> Option<AssetKind> {
    Some(match value {
        "avatar_original" => AssetKind::AvatarOriginal,
        "background_image" => AssetKind::BackgroundImage,
        "illustration" => AssetKind::Illustration,
        "lorebook_icon" => AssetKind::LorebookIcon,
        "message_image" => AssetKind::MessageImage,
        "message_audio" => AssetKind::MessageAudio,
        "generated_image" => AssetKind::GeneratedImage,
        "synthesized_speech" => AssetKind::SynthesizedSpeech,
        "other_image" => AssetKind::OtherImage,
        "other_audio" => AssetKind::OtherAudio,
        _ => return None,
    })
}

fn asset_origin_name(value: AssetOrigin) -> &'static str {
    match value {
        AssetOrigin::Upload => "upload",
        AssetOrigin::Import => "import",
        AssetOrigin::RemoteFetch => "remote_fetch",
        AssetOrigin::Generated => "generated",
        AssetOrigin::Synthesized => "synthesized",
        AssetOrigin::Legacy => "legacy",
    }
}

fn asset_origin_from_name(value: &str) -> Option<AssetOrigin> {
    Some(match value {
        "upload" => AssetOrigin::Upload,
        "import" => AssetOrigin::Import,
        "remote_fetch" => AssetOrigin::RemoteFetch,
        "generated" => AssetOrigin::Generated,
        "synthesized" => AssetOrigin::Synthesized,
        "legacy" => AssetOrigin::Legacy,
        _ => return None,
    })
}

fn retention_values(value: RetentionClass) -> (&'static str, Option<i64>) {
    match value {
        RetentionClass::Persistent => ("persistent", None),
        RetentionClass::Library => ("library", None),
        RetentionClass::Temporary { expires_at } => ("temporary", Some(expires_at.get())),
    }
}

fn retention_from_values(name: &str, expires_at: Option<i64>) -> Option<RetentionClass> {
    match (name, expires_at) {
        ("persistent", None) => Some(RetentionClass::Persistent),
        ("library", None) => Some(RetentionClass::Library),
        ("temporary", Some(expires_at)) => Some(RetentionClass::Temporary {
            expires_at: TimestampMillis::new(expires_at),
        }),
        _ => None,
    }
}

fn asset_from_row(row: &Row<'_>) -> rusqlite::Result<MediaAsset> {
    let blob_kind = match row.get::<_, String>(2)?.as_str() {
        "image" => MediaKind::Image,
        "audio" => MediaKind::Audio,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let kind = asset_kind_from_name(row.get::<_, String>(3)?.as_str())
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let origin = asset_origin_from_name(row.get::<_, String>(4)?.as_str())
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let retention = retention_from_values(
        row.get::<_, String>(5)?.as_str(),
        row.get::<_, Option<i64>>(6)?,
    )
    .ok_or(rusqlite::Error::InvalidQuery)?;
    let provenance = serde_json::from_str(row.get::<_, String>(7)?.as_str())
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let asset = MediaAsset::new(
        parse_id(row.get(0)?)?,
        parse_id(row.get(1)?)?,
        kind,
        origin,
        retention,
        provenance,
        to_revision(row.get(8)?)?,
        TimestampMillis::new(row.get(9)?),
        TimestampMillis::new(row.get(10)?),
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)?;
    asset
        .validate_for_blob_kind(blob_kind)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(asset)
}

fn asset_error(error: rusqlite::Error) -> MediaAssetRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => MediaAssetRepositoryError::InvalidData,
        _ => MediaAssetRepositoryError::Storage,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn hex_decode(value: &str) -> Result<Vec<u8>, ()> {
    if value.is_empty() || value.len() % 2 != 0 {
        return Err(());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).ok_or(())?;
            let low = (pair[1] as char).to_digit(16).ok_or(())?;
            Ok(((high << 4) | low) as u8)
        })
        .collect()
}

fn encode_library_cursor(asset: &MediaAsset) -> Result<String, MediaAssetRepositoryError> {
    let cursor = LibraryCursor {
        format_version: ASSET_CURSOR_FORMAT_VERSION,
        updated_at: asset.updated_at.get(),
        id: asset.id.to_string(),
    };
    serde_json::to_vec(&cursor)
        .map(|value| hex_encode(&value))
        .map_err(|_| MediaAssetRepositoryError::InvalidData)
}

fn decode_library_cursor(
    value: Option<&str>,
) -> Result<Option<(i64, AssetId)>, MediaAssetRepositoryError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let bytes = hex_decode(value).map_err(|_| MediaAssetRepositoryError::InvalidData)?;
    let cursor: LibraryCursor =
        serde_json::from_slice(&bytes).map_err(|_| MediaAssetRepositoryError::InvalidData)?;
    if cursor.format_version != ASSET_CURSOR_FORMAT_VERSION {
        return Err(MediaAssetRepositoryError::InvalidData);
    }
    let id = cursor
        .id
        .parse()
        .map_err(|_| MediaAssetRepositoryError::InvalidData)?;
    Ok(Some((cursor.updated_at, id)))
}

fn load_asset_with_blob(
    connection: &Connection,
    id: AssetId,
) -> Result<Option<MediaAsset>, rusqlite::Error> {
    let asset = connection
        .query_row(
            &format!("SELECT {MEDIA_ASSET_COLUMNS} FROM media_assets WHERE id=?1"),
            [id.to_string()],
            asset_from_row,
        )
        .optional()?;
    let Some(asset) = asset else {
        return Ok(None);
    };
    let blob = connection
        .query_row(
            &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE id=?1"),
            [asset.blob_id.to_string()],
            media_from_row,
        )
        .optional()?
        .ok_or(rusqlite::Error::InvalidQuery)?;
    if blob.kind != asset.kind.blob_kind() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(Some(asset))
}

impl MediaAssetRepository for Database {
    fn create(&self, asset: MediaAsset) -> Result<MediaAsset, MediaAssetRepositoryError> {
        asset
            .validate()
            .map_err(|_| MediaAssetRepositoryError::InvalidData)?;
        let (retention, expires_at) = retention_values(asset.retention);
        let provenance = serde_json::to_string(&asset.provenance)
            .map_err(|_| MediaAssetRepositoryError::InvalidData)?;
        let mut connection = self
            .connection()
            .map_err(|_| MediaAssetRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(asset_error)?;
        let blob = transaction
            .query_row(
                &format!("SELECT {MEDIA_BLOB_COLUMNS} FROM media_blobs WHERE id=?1"),
                [asset.blob_id.to_string()],
                media_from_row,
            )
            .optional()
            .map_err(asset_error)?;
        let Some(blob) = blob else {
            return Err(MediaAssetRepositoryError::BlobMissing);
        };
        if blob.kind != asset.kind.blob_kind() {
            return Err(MediaAssetRepositoryError::InvalidData);
        }
        if load_asset_with_blob(&transaction, asset.id)
            .map_err(asset_error)?
            .is_some()
        {
            return Err(MediaAssetRepositoryError::AlreadyExists);
        }
        transaction
            .execute(
                "INSERT INTO media_assets (id, blob_id, blob_kind, kind, origin, retention, expires_at, provenance_json, revision, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    asset.id.to_string(),
                    asset.blob_id.to_string(),
                    media_kind_name(asset.kind.blob_kind()),
                    asset_kind_name(asset.kind),
                    asset_origin_name(asset.origin),
                    retention,
                    expires_at,
                    provenance,
                    to_i64(asset.revision.get()).map_err(asset_error)?,
                    asset.created_at.get(),
                    asset.updated_at.get(),
                ],
            )
            .map_err(asset_error)?;
        let stored = load_asset_with_blob(&transaction, asset.id)
            .map_err(asset_error)?
            .ok_or(MediaAssetRepositoryError::InvalidData)?;
        transaction.commit().map_err(asset_error)?;
        Ok(stored)
    }

    fn get(&self, id: AssetId) -> Result<Option<MediaAsset>, MediaAssetRepositoryError> {
        self.connection()
            .map_err(|_| MediaAssetRepositoryError::Storage)
            .and_then(|connection| load_asset_with_blob(&connection, id).map_err(asset_error))
    }

    fn update_retention(
        &self,
        id: AssetId,
        expected_revision: Revision,
        retention_value: RetentionClass,
        updated_at: TimestampMillis,
    ) -> Result<MediaAsset, MediaAssetRepositoryError> {
        let (retention, expires_at) = retention_values(retention_value);
        let mut connection = self
            .connection()
            .map_err(|_| MediaAssetRepositoryError::Storage)?;
        let transaction = connection.transaction().map_err(asset_error)?;
        let current = load_asset_with_blob(&transaction, id)
            .map_err(asset_error)?
            .ok_or(MediaAssetRepositoryError::NotFound)?;
        if current.revision != expected_revision {
            return Err(MediaAssetRepositoryError::StaleRevision);
        }
        let next_revision = current
            .next_revision()
            .map_err(|_| MediaAssetRepositoryError::InvalidData)?;
        let changed = transaction
            .execute(
                "UPDATE media_assets SET retention=?2, expires_at=?3, revision=?4, updated_at=?5 WHERE id=?1 AND revision=?6",
                params![
                    id.to_string(),
                    retention,
                    expires_at,
                    to_i64(next_revision.get()).map_err(asset_error)?,
                    updated_at.get(),
                    to_i64(expected_revision.get()).map_err(asset_error)?,
                ],
            )
            .map_err(asset_error)?;
        if changed != 1 {
            return Err(MediaAssetRepositoryError::StaleRevision);
        }
        let stored = load_asset_with_blob(&transaction, id)
            .map_err(asset_error)?
            .ok_or(MediaAssetRepositoryError::InvalidData)?;
        transaction.commit().map_err(asset_error)?;
        Ok(stored)
    }

    fn list_library(
        &self,
        request: PageRequest,
    ) -> Result<Page<MediaAsset>, MediaAssetRepositoryError> {
        let cursor = decode_library_cursor(request.cursor.as_deref())?;
        let limit = usize::from(request.limit.get()).max(1);
        let connection = self
            .connection()
            .map_err(|_| MediaAssetRepositoryError::Storage)?;
        let mut ids = Vec::with_capacity(limit + 1);
        let mut statement = connection
            .prepare(
                "SELECT id FROM media_assets WHERE retention='library' AND (?1 IS NULL OR updated_at < ?1 OR (updated_at = ?1 AND id > ?2)) ORDER BY updated_at DESC, id ASC LIMIT ?3",
            )
            .map_err(asset_error)?;
        let (cursor_time, cursor_id) = cursor
            .map(|(time, id)| (Some(time), Some(id.to_string())))
            .unwrap_or((None, None));
        let rows = statement
            .query_map(
                params![
                    cursor_time,
                    cursor_id,
                    i64::try_from(limit + 1).unwrap_or(i64::MAX)
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(asset_error)?;
        for row in rows {
            ids.push(
                row.map_err(asset_error)?
                    .parse()
                    .map_err(|_| MediaAssetRepositoryError::InvalidData)?,
            );
        }
        drop(statement);

        let has_more = ids.len() > limit;
        if has_more {
            ids.truncate(limit);
        }
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            let asset = load_asset_with_blob(&connection, id)
                .map_err(asset_error)?
                .ok_or(MediaAssetRepositoryError::InvalidData)?;
            if asset.retention != RetentionClass::Library {
                return Err(MediaAssetRepositoryError::InvalidData);
            }
            items.push(asset);
        }
        let next_cursor = if has_more {
            items.last().map(encode_library_cursor).transpose()?
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::thread;

    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaAssetRepositoryError, MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_models::{
        CustomAuth, Modality, ModelDependencyReference, ModelKind, ModelProfile,
        ModelProfileConfig, ModelProfileRepository, ModelRepositoryError, ProviderAccount,
        ProviderAccountRepository, ProviderConfig, ProviderProtocol, SecretHeader,
    };
    use lettuce_settings::{
        GlobalSettingsStore, GlobalSettingsStoreError, HeaderName, SecretOwnerId, SecretPurpose,
        SecretRef,
    };
    use lettuce_types::{
        AssetId, CharacterId, ContentHash, GroupId, MediaBlobId, ModelProfileId, PageLimit,
        PageRequest, ProviderAccountId, Revision, TimestampMillis,
    };

    use super::{
        Database, DatabaseError, DeleteFailurePoint, Migration, apply_migrations, hex_encode,
    };

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

    fn media_blob(hash_byte: char, kind: MediaKind) -> MediaBlob {
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(hash_byte.to_string().repeat(64)).expect("hash"),
            kind,
            mime_type: match kind {
                MediaKind::Image => "image/webp",
                MediaKind::Audio => "audio/mpeg",
                MediaKind::Video => "video/mp4",
            }
            .into(),
            byte_size: 42,
            width: None,
            height: None,
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Ready,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        }
    }

    fn media_asset(id: AssetId, blob_id: MediaBlobId, kind: AssetKind) -> MediaAsset {
        MediaAsset::new(
            id,
            blob_id,
            kind,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(20),
            TimestampMillis::new(20),
        )
        .expect("valid media asset")
    }

    fn insert_group_model_reference(
        database: &Database,
        model_profile_id: ModelProfileId,
    ) -> (GroupId, CharacterId) {
        let group_id = GroupId::new();
        let character_id = CharacterId::new();
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO characters (id,status,name,normalized_name,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,voice_autoplay,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Member','member','{}','{}','{}','roleplay','manual',0,'{}',1,1,1)",
                [character_id.to_string()],
            )
            .expect("group character fixture");
        connection
            .execute(
                "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,speaker_selection,memory_policy,disable_character_lorebooks,presentation_json,background_blob_kind,revision,created_at,updated_at) VALUES (?1,'active','Cast','cast','conversation','inherit','llm','manual',0,'{}','image',1,1,1)",
                [group_id.to_string()],
            )
            .expect("group fixture");
        connection
            .execute(
                "INSERT INTO group_members (group_id,character_id,ordinal,muted,model_profile_override_id) VALUES (?1,?2,0,0,?3)",
                rusqlite::params![group_id.to_string(), character_id.to_string(), model_profile_id.to_string()],
            )
            .expect("group model reference");
        (group_id, character_id)
    }

    #[test]
    fn migration_is_idempotent_and_checksum_protected() {
        let database = Database::open_in_memory().expect("open database");
        let mut connection = database.connection().expect("database lock");
        apply_migrations(
            &mut connection,
            &[
                super::MIGRATION_1,
                super::MIGRATION_2,
                super::MIGRATION_3,
                super::MIGRATION_4,
                super::MIGRATION_5,
            ],
        )
        .expect("repeat migration");
        let count: u32 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(count, 8);

        let changed = Migration {
            id: 1,
            sql: "SELECT 1;",
        };
        assert!(matches!(
            apply_migrations(&mut connection, &[changed]),
            Err(DatabaseError::MigrationChecksum { id: 1 })
        ));
        let changed = Migration {
            id: 2,
            sql: "SELECT 2;",
        };
        assert!(matches!(
            apply_migrations(&mut connection, &[changed]),
            Err(DatabaseError::MigrationChecksum { id: 2 })
        ));
        let changed = Migration {
            id: 3,
            sql: "SELECT 3;",
        };
        assert!(matches!(
            apply_migrations(&mut connection, &[changed]),
            Err(DatabaseError::MigrationChecksum { id: 3 })
        ));
        let changed = Migration {
            id: 4,
            sql: "SELECT 4;",
        };
        assert!(matches!(
            apply_migrations(&mut connection, &[changed]),
            Err(DatabaseError::MigrationChecksum { id: 4 })
        ));
        let changed = Migration {
            id: 5,
            sql: "SELECT 5;",
        };
        assert!(matches!(
            apply_migrations(&mut connection, &[changed]),
            Err(DatabaseError::MigrationChecksum { id: 5 })
        ));
    }

    #[test]
    fn m7_file_upgrades_to_m8_and_rejects_checksum_tampering() {
        let path = std::env::temp_dir().join(format!("lettuce-m7-m8-{}.db", MediaBlobId::new()));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open m7 file");
            super::configure(&connection, true).expect("configure m7 file");
            super::apply_migrations(
                &mut connection,
                &[
                    super::MIGRATION_1,
                    super::MIGRATION_2,
                    super::MIGRATION_3,
                    super::MIGRATION_4,
                    super::MIGRATION_5,
                    super::MIGRATION_6,
                    super::MIGRATION_7,
                ],
            )
            .expect("apply m1-m7");
        }
        let database = Database::open(&path).expect("upgrade m7 file");
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        let conversation_tables: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name IN ('conversations', 'conversation_snapshot_artifacts', 'generation_attempts')",
                [],
                |row| row.get(0),
            )
            .expect("conversation tables");
        assert_eq!(conversation_tables, 3);
        drop(connection);
        drop(database);
        let tampered = rusqlite::Connection::open(&path).expect("reopen database");
        tampered
            .execute(
                "UPDATE schema_migrations SET checksum = 'tampered' WHERE id = 8",
                [],
            )
            .expect("tamper checksum");
        drop(tampered);
        assert!(matches!(
            Database::open(&path),
            Err(DatabaseError::MigrationChecksum { id: 8 })
        ));
        let repaired = rusqlite::Connection::open(&path).expect("reopen tampered database");
        repaired
            .execute(
                "UPDATE schema_migrations SET checksum = ?1 WHERE id = 8",
                [super::migration_checksum(super::MIGRATION_8.sql)],
            )
            .expect("repair checksum");
        drop(repaired);
        Database::open(&path).expect("reopen repaired m8 file");
        let _ = std::fs::remove_file(path);
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
    fn model_deletion_reports_sorted_group_member_overrides_on_both_paths() {
        let database = Database::open_in_memory().expect("open database");
        let account =
            ProviderAccountRepository::upsert(&database, provider(), None).expect("account");
        let first = ModelProfileRepository::upsert(&database, profile(account.id), None)
            .expect("first profile");
        let mut second_input = profile(account.id);
        second_input.id = ModelProfileId::new();
        let second =
            ModelProfileRepository::upsert(&database, second_input, None).expect("second profile");
        let first_dependency = insert_group_model_reference(&database, first.id);
        let second_dependency = insert_group_model_reference(&database, second.id);

        assert_eq!(
            ModelProfileRepository::delete_and_clear_default(&database, first.id),
            Err(ModelRepositoryError::InUse(vec![
                ModelDependencyReference::GroupMemberOverride {
                    group_id: first_dependency.0,
                    character_id: first_dependency.1,
                }
            ]))
        );
        let mut expected = vec![
            ModelDependencyReference::GroupMemberOverride {
                group_id: first_dependency.0,
                character_id: first_dependency.1,
            },
            ModelDependencyReference::GroupMemberOverride {
                group_id: second_dependency.0,
                character_id: second_dependency.1,
            },
        ];
        expected.sort_by_key(super::model_dependency_sort_key);
        assert_eq!(
            ProviderAccountRepository::delete_with_profiles(&database, account.id),
            Err(ModelRepositoryError::InUse(expected))
        );

        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "DELETE FROM group_members WHERE group_id IN (?1,?2)",
                rusqlite::params![
                    first_dependency.0.to_string(),
                    second_dependency.0.to_string()
                ],
            )
            .expect("remove group dependencies");
        drop(connection);
        ProviderAccountRepository::delete_with_profiles(&database, account.id)
            .expect("delete account graph after detach");
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
    fn provider_and_profile_config_documents_are_versioned() {
        let database = Database::open_in_memory().expect("open database");
        let account =
            ProviderAccountRepository::upsert(&database, provider(), None).expect("insert account");
        let profile = ModelProfileRepository::upsert(&database, profile(account.id), None)
            .expect("insert profile");
        let connection = database.connection().expect("database lock");
        let provider_config: String = connection
            .query_row(
                "SELECT config_json FROM provider_accounts WHERE id=?1",
                [account.id.to_string()],
                |row| row.get(0),
            )
            .expect("provider config");
        let model_config: String = connection
            .query_row(
                "SELECT config_json FROM model_profiles WHERE id=?1",
                [profile.id.to_string()],
                |row| row.get(0),
            )
            .expect("model config");
        let provider_document: serde_json::Value =
            serde_json::from_str(&provider_config).expect("provider document");
        let model_document: serde_json::Value =
            serde_json::from_str(&model_config).expect("model document");
        assert_eq!(
            provider_document.get("format_version"),
            Some(&serde_json::Value::from(1))
        );
        assert_eq!(
            model_document.get("format_version"),
            Some(&serde_json::Value::from(1))
        );
    }

    #[test]
    fn provider_config_corruption_is_rejected() {
        let database = Database::open_in_memory().expect("open database");
        let account =
            ProviderAccountRepository::upsert(&database, provider(), None).expect("insert account");
        let corrupt_payloads = [
            r#"{"value":{"kind":"standard"}}"#,
            r#"{"format_version":99,"value":{"kind":"standard"}}"#,
            r#"{"format_version":1,"value":{"kind":"standard","extra":true}}"#,
            r#"{"format_version":1,"value":{"kind":"standard"},"extra":true}"#,
            "not json",
        ];
        for payload in corrupt_payloads {
            let connection = database.connection().expect("database lock");
            connection
                .execute(
                    "UPDATE provider_accounts SET config_json=?1 WHERE id=?2",
                    rusqlite::params![payload, account.id.to_string()],
                )
                .expect("corrupt provider config");
            drop(connection);
            assert_eq!(
                ProviderAccountRepository::get(&database, account.id),
                Err(ModelRepositoryError::InvalidData)
            );
        }
    }

    #[test]
    fn model_profile_config_corruption_is_rejected() {
        let database = Database::open_in_memory().expect("open database");
        let account =
            ProviderAccountRepository::upsert(&database, provider(), None).expect("insert account");
        let model = ModelProfileRepository::upsert(&database, profile(account.id), None)
            .expect("insert profile");
        let corrupt_payloads = [
            r#"{"value":{"input_modalities":["text"],"output_modalities":["text"]}}"#,
            r#"{"format_version":99,"value":{"input_modalities":["text"],"output_modalities":["text"]}}"#,
            r#"{"format_version":1,"value":{"input_modalities":["text"],"output_modalities":["text"],"extra":true}}"#,
            r#"{"format_version":1,"value":{"input_modalities":["text"],"output_modalities":["text"]},"extra":true}"#,
            "not json",
        ];
        for payload in corrupt_payloads {
            let connection = database.connection().expect("database lock");
            connection
                .execute(
                    "UPDATE model_profiles SET config_json=?1 WHERE id=?2",
                    rusqlite::params![payload, model.id.to_string()],
                )
                .expect("corrupt model config");
            drop(connection);
            assert_eq!(
                ModelProfileRepository::get(&database, model.id),
                Err(ModelRepositoryError::InvalidData)
            );
        }
    }

    #[test]
    fn global_settings_corruption_is_rejected() {
        let database = Database::open_in_memory().expect("open database");
        let corrupt_documents = [
            (
                99_i64,
                r#"{"pure_mode":"standard","analytics_enabled":true,"update_checks_enabled":true}"#,
            ),
            (
                1,
                r#"{"pure_mode":"standard","analytics_enabled":true,"update_checks_enabled":true,"extra":true}"#,
            ),
            (1, "not json"),
        ];
        for (format_version, payload) in corrupt_documents {
            let connection = database.connection().expect("database lock");
            connection
                .execute(
                    "UPDATE app_settings SET format_version=?1, payload_json=?2 WHERE id=1",
                    rusqlite::params![format_version, payload],
                )
                .expect("corrupt global settings");
            drop(connection);
            assert_eq!(
                GlobalSettingsStore::load(&database),
                Err(GlobalSettingsStoreError::InvalidData)
            );
        }
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
    fn account_graph_delete_rolls_back_after_profile_delete_failure() {
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
        let settings_before = GlobalSettingsStore::load(&database).expect("load settings");

        assert_eq!(
            database
                .delete_with_profiles_inner(account.id, DeleteFailurePoint::AfterProfilesDelete,),
            Err(ModelRepositoryError::Storage)
        );
        assert_eq!(
            ProviderAccountRepository::get(&database, account.id),
            Ok(Some(account))
        );
        assert_eq!(
            ModelProfileRepository::get(&database, profile.id),
            Ok(Some(profile))
        );
        assert_eq!(GlobalSettingsStore::load(&database), Ok(settings_before));
    }

    #[test]
    fn profile_delete_rolls_back_after_profile_delete_failure() {
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
        let settings_before = GlobalSettingsStore::load(&database).expect("load settings");

        assert_eq!(
            database.delete_and_clear_default_inner(
                profile.id,
                DeleteFailurePoint::AfterProfileDelete,
            ),
            Err(ModelRepositoryError::Storage)
        );
        assert_eq!(
            ProviderAccountRepository::get(&database, account.id),
            Ok(Some(account))
        );
        assert_eq!(
            ModelProfileRepository::get(&database, profile.id),
            Ok(Some(profile))
        );
        assert_eq!(GlobalSettingsStore::load(&database), Ok(settings_before));
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
        duplicate.state = BlobState::Missing;
        duplicate.created_at = TimestampMillis::new(31);
        duplicate.updated_at = TimestampMillis::new(32);
        let deduplicated =
            MediaBlobRepository::register(&database, duplicate).expect("register duplicate");
        assert_eq!(deduplicated.id, stored.id);
        assert_eq!(deduplicated.state, stored.state);
        assert_eq!(deduplicated.created_at, stored.created_at);
        assert_eq!(deduplicated.updated_at, stored.updated_at);
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
        assert_eq!(
            MediaBlobRepository::register(&database, collision),
            Err(lettuce_media::MediaBlobRepositoryError::InvalidData)
        );
    }

    #[test]
    fn media_blob_registration_rejects_invalid_metadata_and_each_immutable_conflict() {
        let database = Database::open_in_memory().expect("open database");
        let invalids = [
            {
                let mut blob = media_blob('a', MediaKind::Image);
                blob.mime_type = "  ".into();
                blob
            },
            {
                let mut blob = media_blob('b', MediaKind::Image);
                blob.mime_type = "image/\u{01}".into();
                blob
            },
            {
                let mut blob = media_blob('c', MediaKind::Image);
                blob.validation_version = 0;
                blob
            },
            {
                let mut blob = media_blob('d', MediaKind::Image);
                blob.byte_size = u64::MAX;
                blob
            },
            {
                let mut blob = media_blob('e', MediaKind::Image);
                blob.width = Some(10);
                blob
            },
        ];
        for blob in invalids {
            assert_eq!(
                MediaBlobRepository::register(&database, blob),
                Err(lettuce_media::MediaBlobRepositoryError::InvalidData)
            );
        }

        let first = MediaBlobRepository::register(&database, media_blob('f', MediaKind::Image))
            .expect("register immutable blob");
        let mut conflicts = Vec::new();
        let mut kind = first.clone();
        kind.id = MediaBlobId::new();
        kind.kind = MediaKind::Audio;
        conflicts.push(kind);
        let mut mime = first.clone();
        mime.id = MediaBlobId::new();
        mime.mime_type = "image/png".into();
        conflicts.push(mime);
        let mut size = first.clone();
        size.id = MediaBlobId::new();
        size.byte_size += 1;
        conflicts.push(size);
        let mut width = first.clone();
        width.id = MediaBlobId::new();
        width.width = Some(2);
        width.height = Some(2);
        conflicts.push(width);
        let mut duration = first.clone();
        duration.id = MediaBlobId::new();
        duration.duration_ms = Some(2);
        conflicts.push(duration);
        let mut version = first.clone();
        version.id = MediaBlobId::new();
        version.validation_version = 2;
        conflicts.push(version);
        for conflict in conflicts {
            assert_eq!(
                MediaBlobRepository::register(&database, conflict),
                Err(lettuce_media::MediaBlobRepositoryError::InvalidData)
            );
        }

        let mut same_id_different_hash = first.clone();
        same_id_different_hash.content_hash = ContentHash::parse("0".repeat(64)).expect("hash");
        assert_eq!(
            MediaBlobRepository::register(&database, same_id_different_hash),
            Err(lettuce_media::MediaBlobRepositoryError::InvalidData)
        );
    }

    #[test]
    fn media_assets_round_trip_share_blobs_and_distinguish_create_errors() {
        let database = Database::open_in_memory().expect("open database");
        let blob = MediaBlobRepository::register(&database, media_blob('1', MediaKind::Image))
            .expect("register blob");
        let first = media_asset(AssetId::new(), blob.id, AssetKind::Illustration);
        let second = media_asset(AssetId::new(), blob.id, AssetKind::AvatarOriginal);
        assert_eq!(
            MediaAssetRepository::create(&database, first.clone()).expect("create first"),
            first
        );
        assert_eq!(
            MediaAssetRepository::create(&database, second.clone()).expect("create second"),
            second
        );
        assert_eq!(
            MediaAssetRepository::get(&database, first.id),
            Ok(Some(first.clone()))
        );
        assert_eq!(
            MediaAssetRepository::get(&database, second.id),
            Ok(Some(second.clone()))
        );
        assert_eq!(
            MediaAssetRepository::create(&database, first),
            Err(MediaAssetRepositoryError::AlreadyExists)
        );

        let missing = media_asset(AssetId::new(), MediaBlobId::new(), AssetKind::Illustration);
        assert_eq!(
            MediaAssetRepository::create(&database, missing),
            Err(MediaAssetRepositoryError::BlobMissing)
        );
        let wrong_kind = media_asset(AssetId::new(), blob.id, AssetKind::MessageAudio);
        assert_eq!(
            MediaAssetRepository::create(&database, wrong_kind),
            Err(MediaAssetRepositoryError::InvalidData)
        );
    }

    #[test]
    fn media_asset_retention_cas_and_expiry_representation_are_atomic() {
        let database = Database::open_in_memory().expect("open database");
        let blob = MediaBlobRepository::register(&database, media_blob('2', MediaKind::Image))
            .expect("register blob");
        let asset = MediaAssetRepository::create(
            &database,
            media_asset(AssetId::new(), blob.id, AssetKind::Illustration),
        )
        .expect("create asset");
        let temporary = MediaAssetRepository::update_retention(
            &database,
            asset.id,
            asset.revision,
            RetentionClass::Temporary {
                expires_at: TimestampMillis::new(42),
            },
            TimestampMillis::new(30),
        )
        .expect("temporary update");
        assert_eq!(temporary.revision, Revision::new(2));
        assert_eq!(
            temporary.retention.expires_at(),
            Some(TimestampMillis::new(42))
        );
        assert_eq!(temporary.updated_at, TimestampMillis::new(30));
        let persistent = MediaAssetRepository::update_retention(
            &database,
            asset.id,
            temporary.revision,
            RetentionClass::Persistent,
            TimestampMillis::new(31),
        )
        .expect("persistent update");
        assert_eq!(persistent.revision, Revision::new(3));
        assert_eq!(persistent.retention.expires_at(), None);
        assert_eq!(
            MediaAssetRepository::update_retention(
                &database,
                asset.id,
                temporary.revision,
                RetentionClass::Library,
                TimestampMillis::new(32),
            ),
            Err(MediaAssetRepositoryError::StaleRevision)
        );
        assert_eq!(
            MediaAssetRepository::update_retention(
                &database,
                AssetId::new(),
                Revision::INITIAL,
                RetentionClass::Library,
                TimestampMillis::new(32),
            ),
            Err(MediaAssetRepositoryError::NotFound)
        );
    }

    #[test]
    fn failed_m2_write_rolls_back_without_changing_asset_revision() {
        let database = Database::open_in_memory().expect("open database");
        let blob = MediaBlobRepository::register(&database, media_blob('6', MediaKind::Image))
            .expect("register blob");
        let asset = MediaAssetRepository::create(
            &database,
            media_asset(AssetId::new(), blob.id, AssetKind::Illustration),
        )
        .expect("create asset");
        {
            let connection = database.connection().expect("database lock");
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_asset_retention BEFORE UPDATE OF retention ON media_assets WHEN NEW.retention = 'temporary' BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
                )
                .expect("install failure trigger");
        }
        assert_eq!(
            MediaAssetRepository::update_retention(
                &database,
                asset.id,
                asset.revision,
                RetentionClass::Temporary {
                    expires_at: TimestampMillis::new(99),
                },
                TimestampMillis::new(40),
            ),
            Err(MediaAssetRepositoryError::Storage)
        );
        assert_eq!(
            MediaAssetRepository::get(&database, asset.id),
            Ok(Some(asset))
        );
    }

    #[test]
    fn library_keyset_pagination_is_deterministic_and_cursor_is_strict() {
        let database = Database::open_in_memory().expect("open database");
        let blob = MediaBlobRepository::register(&database, media_blob('3', MediaKind::Image))
            .expect("register blob");
        let mut ids = (0..5).map(|_| AssetId::new()).collect::<Vec<_>>();
        ids.sort();
        for id in ids.iter().copied() {
            let mut asset = media_asset(id, blob.id, AssetKind::Illustration);
            asset.updated_at = TimestampMillis::new(100);
            MediaAssetRepository::create(&database, asset).expect("create library asset");
        }

        let first = MediaAssetRepository::list_library(
            &database,
            PageRequest {
                cursor: None,
                limit: PageLimit::new(2),
            },
        )
        .expect("first page");
        assert_eq!(
            first.items.iter().map(|asset| asset.id).collect::<Vec<_>>(),
            ids[..2].to_vec()
        );
        let second = MediaAssetRepository::list_library(
            &database,
            PageRequest {
                cursor: first.next_cursor.clone(),
                limit: PageLimit::new(2),
            },
        )
        .expect("second page");
        assert_eq!(
            second
                .items
                .iter()
                .map(|asset| asset.id)
                .collect::<Vec<_>>(),
            ids[2..4].to_vec()
        );
        let third = MediaAssetRepository::list_library(
            &database,
            PageRequest {
                cursor: second.next_cursor,
                limit: PageLimit::new(2),
            },
        )
        .expect("third page");
        assert_eq!(
            third.items.iter().map(|asset| asset.id).collect::<Vec<_>>(),
            ids[4..].to_vec()
        );
        assert!(third.next_cursor.is_none());

        for document in [
            serde_json::json!({"format_version": 1, "updated_at": 100, "id": "bad"}),
            serde_json::json!({"format_version": 2, "updated_at": 100, "id": ids[0].to_string()}),
            serde_json::json!({"format_version": 1, "updated_at": 100, "id": ids[0].to_string(), "extra": true}),
        ] {
            let cursor = hex_encode(&serde_json::to_vec(&document).expect("cursor json"));
            assert_eq!(
                MediaAssetRepository::list_library(
                    &database,
                    PageRequest {
                        cursor: Some(cursor),
                        limit: PageLimit::new(2),
                    },
                ),
                Err(MediaAssetRepositoryError::InvalidData)
            );
        }
    }

    #[test]
    fn asset_provenance_corruption_is_rejected_strictly() {
        let database = Database::open_in_memory().expect("open database");
        let blob = MediaBlobRepository::register(&database, media_blob('4', MediaKind::Image))
            .expect("register blob");
        let asset = MediaAssetRepository::create(
            &database,
            media_asset(AssetId::new(), blob.id, AssetKind::Illustration),
        )
        .expect("create asset");
        for payload in [
            "not json".to_owned(),
            r#"{"format_version":2}"#.to_owned(),
            r#"{"format_version":1,"source_label":null,"source_uri_redacted":null,"producing_job_id":null,"model_profile_id":null,"imported_format":null,"extra":true}"#.to_owned(),
        ] {
            let connection = database.connection().expect("database lock");
            connection
                .execute(
                    "UPDATE media_assets SET provenance_json=?1 WHERE id=?2",
                    rusqlite::params![payload, asset.id.to_string()],
                )
                .expect("corrupt provenance");
            drop(connection);
            assert_eq!(
                MediaAssetRepository::get(&database, asset.id),
                Err(MediaAssetRepositoryError::InvalidData)
            );
        }
    }

    #[test]
    fn media_asset_sql_checks_and_integrity_constraints_hold() {
        let database = Database::open_in_memory().expect("open database");
        let blob = MediaBlobRepository::register(&database, media_blob('5', MediaKind::Image))
            .expect("register blob");
        let connection = database.connection().expect("database lock");
        let insert = |blob_id: &str,
                      blob_kind: &str,
                      kind: &str,
                      retention: &str,
                      expires_at: Option<i64>| {
            connection.execute(
                "INSERT INTO media_assets (id, blob_id, blob_kind, kind, origin, retention, expires_at, provenance_json, revision, created_at, updated_at) VALUES (?1,?2,?3,?4,'upload',?5,?6,?7,1,1,1)",
                rusqlite::params![
                    AssetId::new().to_string(), blob_id, blob_kind, kind, retention, expires_at,
                    serde_json::to_string(&AssetProvenanceV1::default()).expect("provenance")
                ],
            )
        };
        assert!(
            insert(
                &blob.id.to_string(),
                "audio",
                "illustration",
                "library",
                None
            )
            .is_err()
        );
        assert!(
            insert(
                &blob.id.to_string(),
                "audio",
                "message_audio",
                "library",
                None
            )
            .is_err()
        );
        assert!(
            insert(
                &blob.id.to_string(),
                "image",
                "message_audio",
                "library",
                None
            )
            .is_err()
        );
        assert!(
            insert(
                &MediaBlobId::new().to_string(),
                "image",
                "illustration",
                "library",
                None
            )
            .is_err()
        );
        assert!(
            insert(
                &blob.id.to_string(),
                "image",
                "illustration",
                "temporary",
                None
            )
            .is_err()
        );
        assert!(
            insert(
                &blob.id.to_string(),
                "image",
                "illustration",
                "library",
                Some(1)
            )
            .is_err()
        );
        insert(
            &blob.id.to_string(),
            "image",
            "illustration",
            "library",
            None,
        )
        .expect("insert valid asset");
        assert!(matches!(
            connection.execute("DELETE FROM media_blobs WHERE id=?1", [blob.id.to_string()]),
            Err(rusqlite::Error::SqliteFailure(_, _))
        ));
        let foreign_key_violations: i64 = connection
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("foreign key check");
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check");
        assert_eq!(foreign_key_violations, 0);
        assert_eq!(integrity, "ok");
    }

    #[test]
    fn existing_m1_file_is_upgraded_to_m2_without_losing_legacy_blob_metadata() {
        let path = std::env::temp_dir().join(format!("lettuce-m1-{}.db", MediaBlobId::new()));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open m1 file");
            super::configure(&connection, false).expect("configure m1 file");
            apply_migrations(&mut connection, &[super::MIGRATION_1]).expect("apply m1");
            connection
                .execute(
                    "INSERT INTO media_blobs (id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at) VALUES (?1,?2,'image','image/webm; codecs=opus',10,NULL,NULL,17,1,'ready',1,2)",
                    rusqlite::params![
                        MediaBlobId::new().to_string(),
                        "9".repeat(64)
                    ],
                )
                .expect("insert legacy blob");
        }
        let database = Database::open(&path).expect("upgrade m1 file");
        let blob = MediaBlobRepository::find_by_hash(
            &database,
            &ContentHash::parse("9".repeat(64)).expect("hash"),
        )
        .expect("read upgraded blob")
        .expect("legacy blob exists");
        assert_eq!(blob.width, None);
        assert_eq!(blob.height, None);
        assert_eq!(blob.duration_ms, Some(17));
        assert_eq!(blob.mime_type, "image/webm; codecs=opus");
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        drop(connection);
        drop(database);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn concurrent_open_upgrades_a_pre_m5_file_once() {
        let path =
            std::env::temp_dir().join(format!("lettuce-pre-m5-race-{}.db", MediaBlobId::new()));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open pre-M5 file");
            super::configure(&connection, true).expect("configure pre-M5 file");
            apply_migrations(
                &mut connection,
                &[
                    super::MIGRATION_1,
                    super::MIGRATION_2,
                    super::MIGRATION_3,
                    super::MIGRATION_4,
                ],
            )
            .expect("create pre-M5 schema");
        }

        let barrier = Arc::new(Barrier::new(3));
        let first_barrier = Arc::clone(&barrier);
        let first_path = path.clone();
        let first = thread::spawn(move || {
            first_barrier.wait();
            Database::open(first_path)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_path = path.clone();
        let second = thread::spawn(move || {
            second_barrier.wait();
            Database::open(second_path)
        });
        barrier.wait();

        assert!(first.join().expect("first upgrade thread").is_ok());
        assert!(second.join().expect("second upgrade thread").is_ok());
        let database = Database::open(&path).expect("reopen upgraded file");
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        drop(connection);
        drop(database);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn m4_file_upgrades_to_m5_group_schema_exactly_once() {
        let path =
            std::env::temp_dir().join(format!("lettuce-m4-upgrade-{}.db", MediaBlobId::new()));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open m4 file");
            super::configure(&connection, true).expect("configure m4 file");
            apply_migrations(
                &mut connection,
                &[
                    super::MIGRATION_1,
                    super::MIGRATION_2,
                    super::MIGRATION_3,
                    super::MIGRATION_4,
                ],
            )
            .expect("apply m1-m4");
        }
        let database = Database::open(&path).expect("upgrade m4 file");
        let connection = database.connection().expect("database lock");
        let migration_count: i64 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(migration_count, 8);
        for table in [
            "groups",
            "group_members",
            "group_presentation_asset_refs",
            "group_starting_scenes",
            "group_scene_variants",
            "group_scene_assets",
        ] {
            let present: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("table lookup");
            assert_eq!(present, 1, "missing table {table}");
        }
        drop(connection);
        drop(database);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn m5_file_upgrades_to_m6_context_schema_exactly_once_and_checksums() {
        let path =
            std::env::temp_dir().join(format!("lettuce-m5-context-{}.db", MediaBlobId::new()));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open m5 file");
            super::configure(&connection, true).expect("configure m5 file");
            apply_migrations(
                &mut connection,
                &[
                    super::MIGRATION_1,
                    super::MIGRATION_2,
                    super::MIGRATION_3,
                    super::MIGRATION_4,
                    super::MIGRATION_5,
                ],
            )
            .expect("apply m1-m5");
        }
        let database = Database::open(&path).expect("upgrade m5 file");
        let connection = database.connection().expect("database lock");
        let count: i64 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(count, 8);
        for table in [
            "prompt_documents",
            "prompt_entries",
            "lorebooks",
            "lorebook_entries",
            "character_lorebook_bindings",
            "persona_lorebook_bindings",
            "group_lorebook_bindings",
        ] {
            let present: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("table lookup");
            assert_eq!(present, 1, "missing table {table}");
        }
        drop(connection);
        drop(database);
        let reopened = Database::open(&path).expect("reopen m6 file");
        let connection = reopened.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        drop(connection);
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn m6_file_upgrades_to_m7_builtin_entry_identity_schema_exactly_once() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-m6-entry-identity-{}.db",
            MediaBlobId::new()
        ));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open m6 file");
            super::configure(&connection, true).expect("configure m6 file");
            apply_migrations(
                &mut connection,
                &[
                    super::MIGRATION_1,
                    super::MIGRATION_2,
                    super::MIGRATION_3,
                    super::MIGRATION_4,
                    super::MIGRATION_5,
                    super::MIGRATION_6,
                ],
            )
            .expect("apply m1-m6");
        }
        let database = Database::open(&path).expect("upgrade m6 file");
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        let column: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('prompt_entries') WHERE name='built_in_entry_key'",
                [],
                |row| row.get(0),
            )
            .expect("entry key column");
        assert_eq!(column, 1);
        let index: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='index' AND name='prompt_entries_built_in_key_idx'",
                [],
                |row| row.get(0),
            )
            .expect("entry key index");
        assert_eq!(index, 1);
        drop(connection);
        drop(database);
        let reopened = Database::open(&path).expect("reopen m7 file");
        let connection = reopened.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        drop(connection);
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn concurrent_open_upgrades_a_pre_m6_file_once() {
        let path =
            std::env::temp_dir().join(format!("lettuce-pre-m6-race-{}.db", MediaBlobId::new()));
        {
            let mut connection = rusqlite::Connection::open(&path).expect("open pre-M6 file");
            super::configure(&connection, true).expect("configure pre-M6 file");
            apply_migrations(
                &mut connection,
                &[
                    super::MIGRATION_1,
                    super::MIGRATION_2,
                    super::MIGRATION_3,
                    super::MIGRATION_4,
                    super::MIGRATION_5,
                ],
            )
            .expect("create pre-M6 schema");
        }
        let barrier = Arc::new(Barrier::new(3));
        let first_barrier = Arc::clone(&barrier);
        let first_path = path.clone();
        let first = thread::spawn(move || {
            first_barrier.wait();
            Database::open(first_path)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_path = path.clone();
        let second = thread::spawn(move || {
            second_barrier.wait();
            Database::open(second_path)
        });
        barrier.wait();
        assert!(first.join().expect("first upgrade").is_ok());
        assert!(second.join().expect("second upgrade").is_ok());
        let database = Database::open(&path).expect("reopen upgraded file");
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .expect("migration count"),
            8
        );
        drop(connection);
        drop(database);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn separate_database_handles_race_safe_blob_registration_and_asset_creation() {
        let path = std::env::temp_dir().join(format!("lettuce-race-{}.db", MediaBlobId::new()));
        let setup = Database::open(&path).expect("open race database");
        let blob = media_blob('a', MediaKind::Image);
        let asset_id = AssetId::new();
        drop(setup);

        let first = Database::open(&path).expect("open first handle");
        let second = Database::open(&path).expect("open second handle");
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let first_blob = blob.clone();
        let first_thread = thread::spawn(move || {
            first_barrier.wait();
            MediaBlobRepository::register(&first, first_blob)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_thread = thread::spawn(move || {
            second_barrier.wait();
            MediaBlobRepository::register(&second, blob)
        });
        let first_blob = first_thread.join().expect("first registration thread");
        let second_blob = second_thread.join().expect("second registration thread");
        let first_blob = first_blob.expect("first registration");
        let second_blob = second_blob.expect("second registration");
        assert_eq!(first_blob.id, second_blob.id);

        let conflict_path = path.clone();
        let first = Database::open(&conflict_path).expect("open conflict first handle");
        let second = Database::open(&conflict_path).expect("open conflict second handle");
        let barrier = Arc::new(Barrier::new(2));
        let conflicting_first = media_blob('b', MediaKind::Image);
        let mut conflicting_second = conflicting_first.clone();
        conflicting_second.id = MediaBlobId::new();
        conflicting_second.mime_type = "image/png".into();
        let first_barrier = Arc::clone(&barrier);
        let first_thread = thread::spawn(move || {
            first_barrier.wait();
            MediaBlobRepository::register(&first, conflicting_first)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_thread = thread::spawn(move || {
            second_barrier.wait();
            MediaBlobRepository::register(&second, conflicting_second)
        });
        let conflict_outcomes = [
            first_thread.join().expect("first conflict thread"),
            second_thread.join().expect("second conflict thread"),
        ];
        assert_eq!(
            conflict_outcomes
                .iter()
                .filter(|result| result.is_ok())
                .count(),
            1
        );
        assert_eq!(
            conflict_outcomes
                .iter()
                .filter(|result| {
                    **result == Err(lettuce_media::MediaBlobRepositoryError::InvalidData)
                })
                .count(),
            1
        );

        let first = Database::open(&path).expect("reopen first handle");
        let second = Database::open(&path).expect("reopen second handle");
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let first_asset = media_asset(asset_id, first_blob.id, AssetKind::Illustration);
        let second_asset = first_asset.clone();
        let first_thread = thread::spawn(move || {
            first_barrier.wait();
            MediaAssetRepository::create(&first, first_asset)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_thread = thread::spawn(move || {
            second_barrier.wait();
            MediaAssetRepository::create(&second, second_asset)
        });
        let outcomes = [
            first_thread.join().expect("first asset thread"),
            second_thread.join().expect("second asset thread"),
        ];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|result| { **result == Err(MediaAssetRepositoryError::AlreadyExists) })
                .count(),
            1
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn database_enables_foreign_keys_and_has_expected_tables() {
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
                "candidate_media_refs",
                "character_lorebook_bindings",
                "character_media",
                "character_presentation_asset_refs",
                "characters",
                "conversation_branches",
                "conversation_message_candidates",
                "conversation_message_revisions",
                "conversation_messages",
                "conversation_operations",
                "conversation_outbox",
                "conversation_participants",
                "conversation_replay_artifacts",
                "conversation_settings",
                "conversation_snapshot_artifacts",
                "conversation_snapshot_refs",
                "conversation_starters",
                "conversation_turns",
                "conversation_usage_refs",
                "conversations",
                "generation_attempts",
                "generation_checkpoints",
                "group_lorebook_bindings",
                "group_members",
                "group_presentation_asset_refs",
                "group_scene_assets",
                "group_scene_variants",
                "group_starting_scenes",
                "groups",
                "lorebook_entries",
                "lorebooks",
                "media_assets",
                "media_blobs",
                "model_profiles",
                "persona_defaults",
                "persona_lorebook_bindings",
                "persona_media",
                "personas",
                "prompt_documents",
                "prompt_entries",
                "provider_accounts",
                "revision_media_refs",
                "scene_assets",
                "scene_variants",
                "scenes",
                "schema_migrations",
                "starter_messages",
                "turn_lorebooks",
            ]
        );
    }

    #[test]
    fn group_schema_enforces_composite_graph_and_image_foreign_keys() {
        let database = Database::open_in_memory().expect("open database");
        let audio_blob =
            MediaBlobRepository::register(&database, media_blob('b', MediaKind::Audio))
                .expect("register audio blob");
        let audio_asset_id = AssetId::new();
        MediaAssetRepository::create(
            &database,
            media_asset(audio_asset_id, audio_blob.id, AssetKind::OtherAudio),
        )
        .expect("create audio asset");

        let group_one = lettuce_types::GroupId::new().to_string();
        let group_two = lettuce_types::GroupId::new().to_string();
        let scene_one = lettuce_types::SceneId::new().to_string();
        let scene_two = lettuce_types::SceneId::new().to_string();
        let variant_two = lettuce_types::SceneVariantId::new().to_string();
        let mut connection = database.connection().expect("database lock");
        for group_id in [&group_one, &group_two] {
            connection
                .execute(
                    "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,speaker_selection,memory_policy,disable_character_lorebooks,presentation_json,background_blob_kind,revision,created_at,updated_at) VALUES (?1,'active','Cast','cast','conversation','inherit','llm','manual',0,'{}','image',1,1,1)",
                    [group_id],
                )
                .expect("insert group fixture");
        }
        for (group_id, scene_id) in [(&group_one, &scene_one), (&group_two, &scene_two)] {
            connection
                .execute(
                    "INSERT INTO group_starting_scenes (group_id,id,status,ordinal,content_json,revision,created_at,updated_at) VALUES (?1,?2,'active',0,'{}',1,1,1)",
                    rusqlite::params![group_id, scene_id],
                )
                .expect("insert scene fixture");
        }
        let image_blob = MediaBlobId::new().to_string();
        let image_asset = AssetId::new().to_string();
        connection
            .execute(
                "INSERT INTO media_blobs (id,content_hash,kind,mime_type,byte_size,width,height,validation_version,state,created_at,updated_at) VALUES (?1,?2,'image','image/png',1,1,1,1,'ready',1,1)",
                rusqlite::params![image_blob, "a".repeat(64)],
            )
            .expect("insert image blob");
        connection
            .execute(
                "INSERT INTO media_assets (id,blob_id,blob_kind,kind,origin,retention,provenance_json,revision,created_at,updated_at) VALUES (?1,?2,'image','illustration','upload','library','{}',1,1,1)",
                rusqlite::params![image_asset, image_blob],
            )
            .expect("insert image asset");
        connection
            .execute(
                "INSERT INTO group_presentation_asset_refs (group_id,asset_id,blob_kind) VALUES (?1,?2,'image')",
                rusqlite::params![group_one, image_asset],
            )
            .expect("insert image reference");
        assert!(
            connection
                .execute(
                    "DELETE FROM media_assets WHERE id=?1",
                    [image_asset.as_str()],
                )
                .is_err()
        );

        assert!(connection
            .execute(
                "INSERT INTO group_scene_variants (group_id,id,scene_id,ordinal,content_json,revision,created_at,updated_at) VALUES (?1,?2,?3,0,'{}',1,1,1)",
                rusqlite::params![group_one, lettuce_types::SceneVariantId::new().to_string(), scene_two],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO group_scene_variants (group_id,id,scene_id,ordinal,content_json,revision,created_at,updated_at) VALUES (?1,?2,?3,0,'{}',1,1,1)",
                rusqlite::params![group_two, variant_two, scene_two],
            )
            .expect("insert valid variant fixture");
        assert!(connection
            .execute(
                "INSERT INTO group_presentation_asset_refs (group_id,asset_id,blob_kind) VALUES (?1,?2,'image')",
                rusqlite::params![group_one, audio_asset_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO group_scene_assets (group_id,scene_id,id,asset_id,blob_kind,slot,ordinal) VALUES (?1,?2,?3,?4,'image','inline',0)",
                rusqlite::params![
                    group_one,
                    scene_one,
                    lettuce_types::SceneAssetLinkId::new().to_string(),
                    audio_asset_id.to_string(),
                ],
            )
            .is_err());

        let selected = connection
            .transaction()
            .expect("start selected-variant transaction");
        selected
            .execute(
                "UPDATE group_starting_scenes SET selected_variant_id=?1 WHERE group_id=?2 AND id=?3",
                rusqlite::params![variant_two, group_one, scene_one],
            )
            .expect("write cross-group selected variant");
        assert!(selected.commit().is_err());

        let starting = connection
            .transaction()
            .expect("start starting-scene transaction");
        starting
            .execute(
                "UPDATE groups SET starting_scene_id=?1 WHERE id=?2",
                rusqlite::params![scene_two, group_one],
            )
            .expect("write cross-group starting scene");
        assert!(starting.commit().is_err());
    }
}
