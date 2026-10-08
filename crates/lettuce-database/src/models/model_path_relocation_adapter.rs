use lettuce_models::{ModelPathRelocation, ModelRepositoryError};
use lettuce_types::TimestampMillis;
use rusqlite::params;

use crate::{
    Database, MODEL_PROFILE_CONFIG_FORMAT_VERSION, encode_versioned, model_error, model_from_row,
    parse_provider_protocol, to_i64, validate_profile,
};

impl ModelPathRelocation for Database {
    fn relocate_model_paths(
        &self,
        relocate: &dyn Fn(&str) -> Option<String>,
        now: TimestampMillis,
    ) -> Result<u32, ModelRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(model_error)?;
        let changed = relocate_in(&transaction, relocate, now)?;
        transaction.commit().map_err(model_error)?;
        Ok(changed)
    }

    fn relocate_model_paths_and_save_device(
        &self,
        relocate: &dyn Fn(&str) -> Option<String>,
        device: lettuce_settings::DeviceSettings,
        now: TimestampMillis,
    ) -> Result<u32, ModelRepositoryError> {
        device
            .validate()
            .map_err(|_| ModelRepositoryError::InvalidData)?;
        let mut connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(model_error)?;
        let changed = relocate_in(&transaction, relocate, now)?;
        let mut current = crate::read_device_settings(&transaction).map_err(|_| ModelRepositoryError::Storage)?;
        current.llm_models_dir = device.llm_models_dir;
        for root in [
            &mut current.retained_model_roots.whisper,
            &mut current.retained_model_roots.kokoro,
            &mut current.retained_model_roots.embedding,
            &mut current.retained_model_roots.thymos,
        ] {
            *root = root.as_deref().map(|path| relocate(path).unwrap_or_else(|| path.to_owned()));
        }
        crate::write_device_settings(&transaction, &current).map_err(model_error)?;
        transaction.commit().map_err(model_error)?;
        Ok(changed)
    }
}

fn relocate_in(
    transaction: &rusqlite::Transaction<'_>,
    relocate: &dyn Fn(&str) -> Option<String>,
    now: TimestampMillis,
) -> Result<u32, ModelRepositoryError> {
    let rows = {
        let mut statement = transaction
                .prepare(
                    "SELECT p.id, p.provider_account_id, p.external_model_id, p.display_name, \
                     p.kind, p.config_json, p.revision, p.created_at, p.updated_at, a.protocol \
                     FROM model_profiles p JOIN provider_accounts a ON a.id = p.provider_account_id",
                )
                .map_err(model_error)?;
        statement
            .query_map([], |row| {
                Ok((
                    model_from_row(row)?,
                    parse_provider_protocol(&row.get::<_, String>(9)?)?,
                ))
            })
            .map_err(model_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(model_error)?
    };
    let mut changed = 0_u32;
    for (mut profile, protocol) in rows {
        if !lettuce_models::relocate_profile_paths(&mut profile, protocol, relocate) {
            continue;
        }
        validate_profile(&profile)?;
        let config = encode_versioned(&profile.config, MODEL_PROFILE_CONFIG_FORMAT_VERSION)
            .map_err(|_| ModelRepositoryError::InvalidData)?;
        let next = profile
            .revision
            .next()
            .map_err(|_| ModelRepositoryError::Storage)?;
        transaction
            .execute(
                "UPDATE model_profiles SET external_model_id=?2, config_json=?3, revision=?4, \
                     updated_at=?5 WHERE id=?1 AND revision=?6",
                params![
                    profile.id.to_string(),
                    profile.external_model_id,
                    config,
                    to_i64(next.get()).map_err(model_error)?,
                    now.get().max(profile.updated_at.get()),
                    to_i64(profile.revision.get()).map_err(model_error)?,
                ],
            )
            .map_err(model_error)?;
        changed += 1;
    }
    changed +=
        crate::media::whisper_model_adapter::relocate_whisper_models_in(transaction, relocate)
            .map_err(|error| match error {
                lettuce_model_hub::WhisperModelRepositoryError::InvalidData => {
                    ModelRepositoryError::InvalidData
                }
                _ => ModelRepositoryError::Storage,
            })?;
    Ok(changed)
}

#[cfg(test)]
mod certificate_race_tests {
    use super::*;
    use lettuce_settings::DeviceSettingsStore;

    #[test]
    fn relocation_merges_folder_fields_without_losing_a_concurrent_certificate() {
        let database = Database::open_in_memory().expect("database");
        let mut stale = database.load_device_settings().expect("before move");
        stale.llm_models_dir = Some("/new/models".into());
        let mut concurrent = database.load_device_settings().expect("certificate import");
        concurrent.trusted_certificates.push(lettuce_settings::TrustedCertificate {
            id: uuid::Uuid::new_v4(), name: "root.pem".into(), imported_at: 1,
            pem: "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----".into(),
        });
        database.save_device_settings(concurrent.clone()).expect("commit certificate");
        database.relocate_model_paths_and_save_device(&|_| None, stale, TimestampMillis::new(1)).expect("move commit");
        let after = database.load_device_settings().expect("both changes");
        assert_eq!(after.trusted_certificates, concurrent.trusted_certificates);
        assert_eq!(after.llm_models_dir.as_deref(), Some("/new/models"));
    }
    #[test]
    fn relocation_does_not_restore_a_concurrently_cleared_retained_root() {
        let database = Database::open_in_memory().expect("database");
        database.update_device_settings(&|device| device.retained_model_roots.whisper = Some("/old/models/whisper".into())).expect("root");
        let mut stale = database.load_device_settings().expect("move snapshot");
        stale.llm_models_dir = Some("/new/models".into());
        stale.retained_model_roots.whisper = Some("/new/models/whisper".into());
        database.update_device_settings(&|device| device.retained_model_roots.whisper = None).expect("concurrent clear");
        database.relocate_model_paths_and_save_device(&|path| path.strip_prefix("/old/models").map(|suffix|format!("/new/models{suffix}")), stale, TimestampMillis::new(1)).expect("move");
        let after = database.load_device_settings().expect("current");
        assert_eq!(after.retained_model_roots.whisper,None);
        assert_eq!(after.llm_models_dir.as_deref(),Some("/new/models"));
    }

}
