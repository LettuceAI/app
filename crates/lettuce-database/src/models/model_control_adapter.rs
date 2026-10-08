use lettuce_models::{ModelProfile, ModelRepositoryError};
use lettuce_types::{ModelProfileId, ProviderAccountId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::{ApiOperationTransaction, Database};

impl ApiOperationTransaction<'_, '_> {
    pub fn model_profile(
        &self,
        id: ModelProfileId,
    ) -> Result<Option<ModelProfile>, ModelRepositoryError> {
        crate::sync_load_model_profile(self.transaction, &id.to_string())
    }

    pub fn save_model_profile(
        &self,
        mut profile: ModelProfile,
        expected: Option<Revision>,
        select_default: bool,
    ) -> Result<ModelProfile, ModelRepositoryError> {
        match (self.model_profile(profile.id)?, expected) {
            (None, None) => {}
            (Some(stored), Some(expected)) if stored.revision == expected => {
                profile.revision = expected.next().map_err(|_| ModelRepositoryError::Storage)?;
                profile.created_at = stored.created_at;
            }
            _ => return Err(ModelRepositoryError::StaleRevision),
        }
        crate::validate_profile(&profile)?;
        if crate::sync_load_provider_account(
            self.transaction,
            &profile.provider_account_id.to_string(),
        )?
        .is_none()
        {
            return Err(ModelRepositoryError::AccountMissing);
        }
        if expected.is_none() {
            crate::insert_model_profile_row(self.transaction, &profile)?;
        } else {
            let config = crate::encode_versioned(
                &profile.config,
                crate::MODEL_PROFILE_CONFIG_FORMAT_VERSION,
            )
            .map_err(|_| ModelRepositoryError::InvalidData)?;
            self.transaction.execute("UPDATE model_profiles SET provider_account_id=?2,external_model_id=?3,display_name=?4,kind=?5,config_json=?6,revision=?7,updated_at=?8 WHERE id=?1", params![profile.id.to_string(), profile.provider_account_id.to_string(), profile.external_model_id, profile.display_name, crate::model_kind_name(profile.kind), config, crate::to_i64(profile.revision.get()).map_err(crate::model_error)?, profile.updated_at.get()]).map_err(crate::model_error)?;
        }
        if select_default {
            self.transaction.execute("UPDATE app_settings SET default_model_profile_id=?1,revision=revision+1,updated_at=?2 WHERE id=1 AND default_model_profile_id IS NULL", params![profile.id.to_string(), profile.updated_at.get()]).map_err(crate::model_error)?;
        }
        self.model_profile(profile.id)?
            .ok_or(ModelRepositoryError::NotFound)
    }

    pub fn delete_model_profile(
        &self,
        id: ModelProfileId,
        expected: Revision,
        at: TimestampMillis,
    ) -> Result<(Vec<String>, Vec<String>), ModelRepositoryError> {
        let profile = self
            .model_profile(id)?
            .ok_or(ModelRepositoryError::NotFound)?;
        if profile.revision != expected {
            return Err(ModelRepositoryError::StaleRevision);
        }
        super::provider_control_adapter::delete_model_in(self.transaction, id, at)
    }

    pub fn set_default_model_profile(
        &self,
        id: Option<ModelProfileId>,
        expected: Revision,
        at: TimestampMillis,
    ) -> Result<Revision, ModelRepositoryError> {
        if let Some(id) = id {
            if self.model_profile(id)?.is_none() {
                return Err(ModelRepositoryError::NotFound);
            }
        }
        let next = expected.next().map_err(|_| ModelRepositoryError::Storage)?;
        let changed = self.transaction.execute("UPDATE app_settings SET default_model_profile_id=?1,revision=?2,updated_at=?3 WHERE id=1 AND revision=?4", params![id.map(|id| id.to_string()), crate::to_i64(next.get()).map_err(crate::model_error)?, at.get(), crate::to_i64(expected.get()).map_err(crate::model_error)?]).map_err(crate::model_error)?;
        if changed != 1 {
            return Err(ModelRepositoryError::StaleRevision);
        }
        Ok(next)
    }
}

pub type ModelCatalogSnapshot = (Vec<ModelProfile>, Option<ModelProfileId>, Revision);

impl Database {
    pub fn model_catalog_snapshot(&self) -> Result<ModelCatalogSnapshot, ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection.transaction().map_err(crate::model_error)?;
        let models = transaction
            .prepare(&format!(
                "SELECT {} FROM model_profiles ORDER BY created_at,id",
                crate::MODEL_PROFILE_COLUMNS
            ))
            .map_err(crate::model_error)?
            .query_map([], crate::model_from_row)
            .map_err(crate::model_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::model_error)?;
        let (default, revision) = transaction
            .query_row(
                "SELECT default_model_profile_id,revision FROM app_settings WHERE id=1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?
                            .map(crate::parse_id)
                            .transpose()?,
                        crate::to_revision(row.get(1)?)?,
                    ))
                },
            )
            .map_err(crate::model_error)?;
        transaction.commit().map_err(crate::model_error)?;
        Ok((models, default, revision))
    }

    pub fn model_change_position(&self) -> Result<u64, ModelRepositoryError> {
        self.connection()
            .map_err(|_| ModelRepositoryError::Storage)?
            .query_row(
                "SELECT coalesce(max(position),0) FROM model_changes",
                [],
                |row| crate::to_u64(row.get(0)?),
            )
            .map_err(crate::model_error)
    }

    pub fn record_provider_quota_warning(
        &self,
        id: ProviderAccountId,
        window: &str,
        level: u8,
    ) -> Result<bool, ModelRepositoryError> {
        if ![75, 90, 100].contains(&level) || window.trim().is_empty() {
            return Err(ModelRepositoryError::InvalidData);
        }
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(crate::model_error)?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM provider_accounts WHERE id=?1",
                [id.to_string()],
                |_| Ok(()),
            )
            .optional()
            .map_err(crate::model_error)?;
        if exists.is_none() {
            return Ok(false);
        }
        let previous = transaction
            .query_row(
                "SELECT window,level FROM provider_quota_warnings WHERE account_id=?1",
                [id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u8>(1)?)),
            )
            .optional()
            .map_err(crate::model_error)?;
        if previous
            .is_some_and(|(held_window, held_level)| held_window == window && held_level >= level)
        {
            return Ok(false);
        }
        transaction.execute("INSERT INTO provider_quota_warnings(account_id,window,level) VALUES(?1,?2,?3) ON CONFLICT(account_id) DO UPDATE SET window=excluded.window,level=excluded.level", params![id.to_string(), window, level]).map_err(crate::model_error)?;
        transaction.commit().map_err(crate::model_error)?;
        Ok(true)
    }
}
