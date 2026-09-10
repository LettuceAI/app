use lettuce_transfer::{
    PROVIDER_BACKUP_GRAPH_VERSION, ProviderBackupGraph, ProviderBackupSelections,
    ProviderBackupSource, ProviderBackupSourceError,
};
use lettuce_types::{ModelProfileId, PromptDocumentId};
use rusqlite::{OptionalExtension, TransactionBehavior};

use crate::{Database, model_from_row, parse_id, provider_from_row};

fn backup_error(error: rusqlite::Error) -> ProviderBackupSourceError {
    if error == rusqlite::Error::InvalidQuery {
        ProviderBackupSourceError::InvalidData
    } else {
        ProviderBackupSourceError::Storage
    }
}

impl ProviderBackupSource for Database {
    fn read_provider_backup_graph(&self) -> Result<ProviderBackupGraph, ProviderBackupSourceError> {
        let mut connection = self
            .connection()
            .map_err(|_| ProviderBackupSourceError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(backup_error)?;
        let accounts = transaction
            .prepare(
                "SELECT id,provider_kind,protocol,label,endpoint,enabled,api_key_secret_ref,secret_owner_id,secret_headers_json,config_json,revision,created_at,updated_at,streaming_enabled,allow_invalid_tls FROM provider_accounts ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], provider_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let profiles = transaction
            .prepare(
                "SELECT id,provider_account_id,external_model_id,display_name,kind,config_json,revision,created_at,updated_at FROM model_profiles ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], model_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let prompt_ids = transaction
            .prepare("SELECT id FROM prompt_documents ORDER BY id")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| parse_id(row.get(0)?))?
                    .collect::<rusqlite::Result<Vec<PromptDocumentId>>>()
            })
            .map_err(backup_error)?;
        let prompts = prompt_ids
            .into_iter()
            .map(|id| {
                crate::prompt_adapter::load_document(&transaction, id)?
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let selections = transaction
            .query_row(
                "SELECT default_model_profile_id,dynamic_memory_model_profile_id,group_speaker_model_profile_id,default_prompt_document_id FROM app_settings WHERE id=1",
                [],
                |row| {
                    Ok(ProviderBackupSelections {
                        default_model_profile_id: row
                            .get::<_, Option<String>>(0)?
                            .map(parse_id::<ModelProfileId>)
                            .transpose()?,
                        dynamic_memory_model_profile_id: row
                            .get::<_, Option<String>>(1)?
                            .map(parse_id::<ModelProfileId>)
                            .transpose()?,
                        group_speaker_model_profile_id: row
                            .get::<_, Option<String>>(2)?
                            .map(parse_id::<ModelProfileId>)
                            .transpose()?,
                        default_prompt_document_id: row
                            .get::<_, Option<String>>(3)?
                            .map(parse_id::<PromptDocumentId>)
                            .transpose()?,
                    })
                },
            )
            .optional()
            .map_err(backup_error)?
            .ok_or(ProviderBackupSourceError::InvalidData)?;
        transaction.commit().map_err(backup_error)?;
        Ok(ProviderBackupGraph {
            version: PROVIDER_BACKUP_GRAPH_VERSION,
            accounts,
            profiles,
            prompts,
            selections,
        })
    }
}
