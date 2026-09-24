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
        let transaction = connection.transaction().map_err(model_error)?;
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
        transaction.commit().map_err(model_error)?;
        Ok(changed)
    }
}
