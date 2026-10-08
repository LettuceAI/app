use lettuce_models::{ModelRepositoryError, ProviderAccount};
use lettuce_settings::{DeviceSettings, SecretRecord, TrustedCertificate};
use lettuce_types::ModelProfileId;
use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ApiOperationError, ApiOperationTransaction, Database};

impl From<ApiOperationError> for ModelRepositoryError {
    fn from(error: ApiOperationError) -> Self {
        match error {
            ApiOperationError::Conflict => Self::StaleRevision,
            ApiOperationError::InvalidData => Self::InvalidData,
            ApiOperationError::Storage => Self::Storage,
        }
    }
}

impl Database {
    pub fn stage_provider_secret(&self, record: &SecretRecord) -> Result<(), ModelRepositoryError> {
        self.connection().map_err(|_| ModelRepositoryError::Storage)?.execute(
            "INSERT OR IGNORE INTO provider_secret_gc(reference,purpose_json,state) VALUES (?1,?2,'staged')",
            params![record.reference.to_string(), serde_json::to_string(&record.purpose).map_err(|_| ModelRepositoryError::InvalidData)?],
        ).map_err(crate::model_error)?;
        Ok(())
    }

    pub fn provider_secret_cleanup(&self) -> Result<Vec<SecretRecord>, ModelRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let rows = connection.prepare("SELECT reference,purpose_json FROM provider_secret_gc WHERE state != 'live' ORDER BY reference").map_err(crate::model_error)?
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(crate::model_error)?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(crate::model_error)?;
        let accounts =
            crate::sync_ids(&connection, "provider_accounts").map_err(crate::model_error)?;
        let mut live = std::collections::BTreeSet::new();
        for id in accounts {
            if let Some(account) = crate::sync_load_provider_account(&connection, &id)? {
                live.extend(
                    account_secret_records(&account)
                        .into_iter()
                        .map(|record| record.reference),
                );
            }
        }
        rows.into_iter()
            .map(|(reference, purpose)| {
                Ok(SecretRecord::new(
                    crate::secret_ref_from_text(reference).map_err(crate::model_error)?,
                    serde_json::from_str(&purpose)
                        .map_err(|_| ModelRepositoryError::InvalidData)?,
                ))
            })
            .filter_map(|record| match record {
                Ok(record) if live.contains(&record.reference) => None,
                value => Some(value),
            })
            .collect()
    }

    pub fn provider_secret_is_retired(
        &self,
        record: &SecretRecord,
    ) -> Result<bool, ModelRepositoryError> {
        self.connection().map_err(|_| ModelRepositoryError::Storage)?.query_row("SELECT EXISTS(SELECT 1 FROM provider_secret_gc WHERE reference=?1 AND state='retired')", [record.reference.to_string()], |row| row.get(0)).map_err(crate::model_error)
    }

    pub fn provider_secret_cleanup_done(
        &self,
        record: &SecretRecord,
    ) -> Result<(), ModelRepositoryError> {
        self.connection()
            .map_err(|_| ModelRepositoryError::Storage)?
            .execute(
                "DELETE FROM provider_secret_gc WHERE reference=?1 AND state != 'live'",
                [record.reference.to_string()],
            )
            .map_err(crate::model_error)?;
        Ok(())
    }

    pub fn certificates_with_revision(
        &self,
    ) -> Result<(Vec<TrustedCertificate>, Revision), ModelRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let (settings, revision) = device_in(&connection)?;
        Ok((settings.trusted_certificates, revision))
    }
}

pub fn account_secret_records(account: &ProviderAccount) -> Vec<SecretRecord> {
    use lettuce_settings::SecretPurpose;
    let mut result = Vec::new();
    if let Some(reference) = account.api_key_ref {
        result.push(SecretRecord::new(
            reference,
            SecretPurpose::ProviderApiKey {
                owner: account.secret_owner_id,
            },
        ));
    }
    for header in &account.secret_headers {
        result.push(SecretRecord::new(
            header.secret_ref,
            SecretPurpose::ProviderSecretHeader {
                owner: account.secret_owner_id,
                name: header.name.clone(),
            },
        ));
    }
    if let lettuce_models::ProviderConfig::Ollama(config) = &account.config {
        if let Some(reference) = config.sprout.as_ref().and_then(|sprout| sprout.api_key_ref) {
            result.push(SecretRecord::new(
                reference,
                SecretPurpose::SproutApiKey {
                    owner: account.secret_owner_id,
                },
            ));
        }
    }
    result
}

fn retire(connection: &Connection, account: &ProviderAccount) -> Result<(), ModelRepositoryError> {
    for record in account_secret_records(account) {
        connection.execute("INSERT INTO provider_secret_gc(reference,purpose_json,state) VALUES (?1,?2,'retired') ON CONFLICT(reference) DO UPDATE SET state='retired'", params![record.reference.to_string(), serde_json::to_string(&record.purpose).map_err(|_| ModelRepositoryError::InvalidData)?]).map_err(crate::model_error)?;
    }
    Ok(())
}

impl ApiOperationTransaction<'_, '_> {
    pub fn save_provider_account(
        &self,
        mut account: ProviderAccount,
        expected: Option<Revision>,
    ) -> Result<ProviderAccount, ModelRepositoryError> {
        let stored = crate::sync_load_provider_account(self.transaction, &account.id.to_string())?;
        match (&stored, expected) {
            (None, None) => {}
            (Some(stored), Some(expected))
                if stored.revision == expected
                    && stored.secret_owner_id == account.secret_owner_id =>
            {
                account.revision = expected.next().map_err(|_| ModelRepositoryError::Storage)?;
                account.created_at = stored.created_at;
                retire(self.transaction, stored)?;
            }
            _ => return Err(ModelRepositoryError::StaleRevision),
        }
        crate::sync_upsert_provider_account(self.transaction, &account)?;
        for record in account_secret_records(&account) {
            self.transaction
                .execute(
                    "UPDATE provider_secret_gc SET state='live' WHERE reference=?1",
                    [record.reference.to_string()],
                )
                .map_err(crate::model_error)?;
        }
        Ok(account)
    }

    pub fn delete_provider_account(
        &self,
        id: ProviderAccountId,
        expected: Revision,
        delete_models: bool,
        at: TimestampMillis,
    ) -> Result<(Vec<String>, Vec<String>), ModelRepositoryError> {
        let account = crate::sync_load_provider_account(self.transaction, &id.to_string())?
            .ok_or(ModelRepositoryError::NotFound)?;
        if account.revision != expected {
            return Err(ModelRepositoryError::StaleRevision);
        }
        let ids = self
            .transaction
            .prepare(
                "SELECT id FROM model_profiles WHERE provider_account_id=?1 ORDER BY created_at,id",
            )
            .map_err(crate::model_error)?
            .query_map([id.to_string()], |row| {
                crate::parse_id::<ModelProfileId>(row.get(0)?)
            })
            .map_err(crate::model_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::model_error)?;
        if !ids.is_empty() && !delete_models {
            return Err(ModelRepositoryError::AccountInUse(ids));
        }
        let mut characters = Vec::new();
        let mut groups = Vec::new();
        for model in ids {
            let (mut changed_characters, mut changed_groups) =
                delete_model_in(self.transaction, model, at)?;
            characters.append(&mut changed_characters);
            groups.append(&mut changed_groups);
        }
        retire(self.transaction, &account)?;
        self.transaction
            .execute(
                "DELETE FROM provider_accounts WHERE id=?1",
                [id.to_string()],
            )
            .map_err(crate::model_error)?;
        characters.sort();
        characters.dedup();
        groups.sort();
        groups.dedup();
        Ok((characters, groups))
    }

    pub fn import_certificate(
        &self,
        certificate: TrustedCertificate,
    ) -> Result<(Vec<TrustedCertificate>, Revision), ModelRepositoryError> {
        let (mut settings, revision) = device_in(self.transaction)?;
        if settings
            .trusted_certificates
            .iter()
            .any(|held| held.pem.trim() == certificate.pem.trim())
        {
            return Err(ModelRepositoryError::AlreadyExists);
        }
        settings.trusted_certificates.push(certificate);
        write_certificates_in(self.transaction, &settings, revision)?;
        Ok((
            settings.trusted_certificates,
            revision.next().map_err(|_| ModelRepositoryError::Storage)?,
        ))
    }

    pub fn remove_certificate(
        &self,
        id: uuid::Uuid,
        expected: Revision,
    ) -> Result<(Vec<TrustedCertificate>, Revision), ModelRepositoryError> {
        let (mut settings, revision) = device_in(self.transaction)?;
        if revision != expected {
            return Err(ModelRepositoryError::StaleRevision);
        }
        let before = settings.trusted_certificates.len();
        settings
            .trusted_certificates
            .retain(|certificate| certificate.id != id);
        if before == settings.trusted_certificates.len() {
            return Err(ModelRepositoryError::NotFound);
        }
        write_certificates_in(self.transaction, &settings, revision)?;
        Ok((
            settings.trusted_certificates,
            revision.next().map_err(|_| ModelRepositoryError::Storage)?,
        ))
    }
}

fn device_in(connection: &Connection) -> Result<(DeviceSettings, Revision), ModelRepositoryError> {
    connection
        .query_row(
            "SELECT settings_json,revision FROM device_settings WHERE id=1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(crate::model_error)?
        .map(|(settings, revision)| {
            Ok((
                serde_json::from_str(&settings).map_err(|_| ModelRepositoryError::InvalidData)?,
                crate::to_revision(revision).map_err(crate::model_error)?,
            ))
        })
        .unwrap_or_else(|| Ok((DeviceSettings::default(), Revision::INITIAL)))
}

pub(crate) fn delete_model_in(
    connection: &Connection,
    id: ModelProfileId,
    at: TimestampMillis,
) -> Result<(Vec<String>, Vec<String>), ModelRepositoryError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM model_profiles WHERE id=?1)",
            [id.to_string()],
            |row| row.get(0),
        )
        .map_err(crate::model_error)?;
    if !exists {
        return Err(ModelRepositoryError::NotFound);
    }
    let characters = connection
        .prepare("SELECT id FROM characters WHERE model_profile_id=?1 ORDER BY id")
        .map_err(crate::model_error)?
        .query_map([id.to_string()], |row| row.get::<_, String>(0))
        .map_err(crate::model_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::model_error)?;
    let groups = connection.prepare("SELECT DISTINCT group_id FROM group_members WHERE model_profile_override_id=?1 ORDER BY group_id").map_err(crate::model_error)?.query_map([id.to_string()], |row| row.get::<_, String>(0)).map_err(crate::model_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(crate::model_error)?;
    connection.execute("UPDATE characters SET model_profile_id=NULL,defaults_json=json_set(defaults_json,'$.value.model_profile_id',NULL),revision=revision+1,updated_at=?2 WHERE model_profile_id=?1", params![id.to_string(), at.get()]).map_err(crate::model_error)?;
    for group in &groups {
        connection
            .execute(
                "UPDATE groups SET revision=revision+1,updated_at=?2 WHERE id=?1",
                params![group, at.get()],
            )
            .map_err(crate::model_error)?;
    }
    connection.execute("UPDATE group_members SET model_profile_override_id=NULL WHERE model_profile_override_id=?1", [id.to_string()]).map_err(crate::model_error)?;
    connection.execute("UPDATE app_settings SET default_model_profile_id=CASE WHEN default_model_profile_id=?1 THEN (SELECT id FROM model_profiles WHERE id!=?1 ORDER BY created_at,id LIMIT 1) ELSE default_model_profile_id END,dynamic_memory_model_profile_id=CASE WHEN dynamic_memory_model_profile_id=?1 THEN NULL ELSE dynamic_memory_model_profile_id END,group_speaker_model_profile_id=CASE WHEN group_speaker_model_profile_id=?1 THEN NULL ELSE group_speaker_model_profile_id END,revision=revision+1,updated_at=?2 WHERE default_model_profile_id=?1 OR dynamic_memory_model_profile_id=?1 OR group_speaker_model_profile_id=?1", params![id.to_string(), at.get()]).map_err(crate::model_error)?;
    crate::clear_deleted_settings_models(connection, |profile| profile == id)
        .map_err(crate::model_error)?;
    connection
        .execute("DELETE FROM model_profiles WHERE id=?1", [id.to_string()])
        .map_err(|error| match &error {
            rusqlite::Error::SqliteFailure(code, _)
                if code.extended_code == 787 || code.extended_code == 1811 =>
            {
                ModelRepositoryError::InUse(Vec::new())
            }
            _ => crate::model_error(error),
        })?;
    Ok((characters, groups))
}

impl Database {
    pub fn remove_certificate_cas(
        &self,
        id: uuid::Uuid,
        expected: Revision,
    ) -> Result<(Vec<TrustedCertificate>, Revision), ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(crate::model_error)?;
        let result = ApiOperationTransaction {
            transaction: &transaction,
        }
        .remove_certificate(id, expected)?;
        transaction.commit().map_err(crate::model_error)?;
        Ok(result)
    }
}

fn write_certificates_in(
    connection: &Connection,
    settings: &DeviceSettings,
    expected: Revision,
) -> Result<(), ModelRepositoryError> {
    settings
        .validate()
        .map_err(|_| ModelRepositoryError::InvalidData)?;
    let encoded = serde_json::to_string(settings).map_err(|_| ModelRepositoryError::InvalidData)?;
    let changed = connection.execute("INSERT INTO device_settings(id,settings_json,updated_at,revision) VALUES (1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET settings_json=excluded.settings_json,updated_at=excluded.updated_at,revision=excluded.revision WHERE device_settings.revision=?4", params![encoded, crate::now().map_err(crate::model_error)?.get(), crate::to_i64(expected.next().map_err(|_| ModelRepositoryError::Storage)?.get()).map_err(crate::model_error)?, crate::to_i64(expected.get()).map_err(crate::model_error)?]).map_err(crate::model_error)?;
    if changed != 1 {
        return Err(ModelRepositoryError::StaleRevision);
    }
    Ok(())
}

#[cfg(test)]
mod size_tests {
    #[test]
    fn oversized_device_settings_fail_the_foundation_constraint() {
        let database = crate::Database::open_in_memory().expect("database");
        assert!(database.connection().expect("connection").execute("INSERT INTO device_settings(id,settings_json,updated_at) VALUES(1,json_object('padding',hex(zeroblob(67108864))),0)", []).is_err());
    }
}
