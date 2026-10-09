use std::path::Path;

use lettuce_models::{ModelRepositoryError, ProviderAccount};
use lettuce_settings::DeviceSettings;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::Database;

#[derive(Debug, Clone)]
pub struct ResetDatabaseSeed {
    pub accounts: Vec<ProviderAccount>,
    pub device: DeviceSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResetDatabaseError {
    #[error("reset source must be write fenced")]
    SourceNotFenced,
    #[error("reset target already exists")]
    Exists,
    #[error("reset seed is invalid")]
    InvalidData,
    #[error("reset database storage is unavailable")]
    Storage,
}

impl Database {
    pub fn read_reset_seed(path: &Path) -> Result<ResetDatabaseSeed, ResetDatabaseError> {
        if !crate::write_fence::FileWriteAccess::new(path)
            .fenced()
            .map_err(|_| ResetDatabaseError::Storage)?
        {
            return Err(ResetDatabaseError::SourceNotFenced);
        }
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| ResetDatabaseError::Storage)?;
        let mut accounts = Vec::new();
        for id in crate::sync_ids(&connection, "provider_accounts")
            .map_err(|_| ResetDatabaseError::Storage)?
        {
            accounts.push(
                crate::sync_load_provider_account(&connection, &id)
                    .map_err(|_| ResetDatabaseError::InvalidData)?
                    .ok_or(ResetDatabaseError::InvalidData)?,
            );
        }
        let old = crate::read_device_settings(&connection)
            .map_err(|_| ResetDatabaseError::InvalidData)?;
        Ok(ResetDatabaseSeed {
            accounts,
            device: DeviceSettings {
                llm_models_dir: old.llm_models_dir,
                retained_model_roots: old.retained_model_roots,
                ..DeviceSettings::default()
            },
        })
    }

    pub fn create_reset_database(
        path: &Path,
        seed: &ResetDatabaseSeed,
    ) -> Result<Self, ResetDatabaseError> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    ResetDatabaseError::Exists
                } else {
                    ResetDatabaseError::Storage
                }
            })?;
        file.sync_all().map_err(|_| ResetDatabaseError::Storage)?;
        drop(file);
        let database = Self::open(path).map_err(|_| ResetDatabaseError::Storage)?;
        {
            let mut connection = database
                .connection()
                .map_err(|_| ResetDatabaseError::Storage)?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| ResetDatabaseError::Storage)?;
            let permitted = DeviceSettings {
                llm_models_dir: seed.device.llm_models_dir.clone(),
                retained_model_roots: seed.device.retained_model_roots.clone(),
                ..DeviceSettings::default()
            };
            if permitted != seed.device {
                return Err(ResetDatabaseError::InvalidData);
            }
            for account in &seed.accounts {
                crate::insert_provider_account_row(&transaction, account).map_err(|error| {
                    match error {
                        ModelRepositoryError::InvalidData => ResetDatabaseError::InvalidData,
                        _ => ResetDatabaseError::Storage,
                    }
                })?;
            }
            crate::replace_device_settings_in(&transaction, &seed.device)
                .map_err(|_| ResetDatabaseError::InvalidData)?;
            transaction
                .commit()
                .map_err(|_| ResetDatabaseError::Storage)?;
        }
        Ok(database)
    }
}
