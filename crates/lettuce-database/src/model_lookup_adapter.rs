use lettuce_models::{ModelLookup, ModelProfile, ModelRepositoryError, ProviderAccount};
use lettuce_types::ProviderAccountId;
use rusqlite::OptionalExtension;

use crate::{
    Database, MODEL_PROFILE_COLUMNS, PROVIDER_ACCOUNT_COLUMNS, model_error, model_from_row,
    provider_from_row,
};

impl ModelLookup for Database {
    fn account_by_kind_and_label(
        &self,
        provider_kind: &str,
        label: &str,
    ) -> Result<Option<ProviderAccount>, ModelRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        connection
            .query_row(
                &format!(
                    "SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts \
                     WHERE provider_kind = ?1 AND label = ?2 ORDER BY created_at, id LIMIT 1"
                ),
                [provider_kind, label],
                provider_from_row,
            )
            .optional()
            .map_err(model_error)
    }

    fn profile_by_external_id(
        &self,
        provider_account_id: ProviderAccountId,
        external_model_id: &str,
    ) -> Result<Option<ModelProfile>, ModelRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| ModelRepositoryError::Storage)?;
        connection
            .query_row(
                &format!(
                    "SELECT {MODEL_PROFILE_COLUMNS} FROM model_profiles \
                     WHERE provider_account_id = ?1 AND external_model_id = ?2 \
                     ORDER BY created_at, id LIMIT 1"
                ),
                [
                    provider_account_id.to_string(),
                    external_model_id.to_owned(),
                ],
                model_from_row,
            )
            .optional()
            .map_err(model_error)
    }
}
