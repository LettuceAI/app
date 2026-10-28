pub(super) mod export_jobs;
pub(super) mod jobs;
#[cfg(test)]
pub(super) mod tests;

use std::io::{Read, Seek, SeekFrom};

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};

use super::{ApiContext, error::{api_error, invalid_field}};

pub(super) fn error(code: ApiErrorCode, reason: dto::BackupFailureReason) -> ApiError {
    let mut error = api_error(code, "backup operation could not complete");
    error.details = Some(dto::ApiErrorDetails::Backup { reason });
    error
}

pub(super) fn file_error(failure: super::FileAccessError) -> ApiError {
    let code = match failure {
        super::FileAccessError::NotFound => ApiErrorCode::NotFound,
        super::FileAccessError::Unsupported => ApiErrorCode::Unsupported,
        super::FileAccessError::SourceIsTarget => ApiErrorCode::Conflict,
        super::FileAccessError::Busy => ApiErrorCode::Busy,
        super::FileAccessError::PermissionDenied | super::FileAccessError::Io => ApiErrorCode::Unavailable,
    };
    error(code, if failure == super::FileAccessError::PermissionDenied { dto::BackupFailureReason::PermissionDenied } else { dto::BackupFailureReason::FileUnavailable })
}

pub async fn backup_inspect(context: &ApiContext, request: dto::BackupInspectRequest) -> Result<dto::BackupHeaderView, ApiError> {
    let uri = request.source.uri;
    if uri.trim().is_empty() {
        return Err(invalid_field("source", "file source is empty"));
    }
    context.blocking(move |context| {
        inspect(context.files(), &uri)
    }).await
}

struct Failure(ApiError);

impl From<lettuce_database::ApiOperationError> for Failure {
    fn from(failure: lettuce_database::ApiOperationError) -> Self {
        Self(error(match failure {
            lettuce_database::ApiOperationError::Conflict => ApiErrorCode::Conflict,
            lettuce_database::ApiOperationError::InvalidData { .. } => ApiErrorCode::InvalidInput,
            lettuce_database::ApiOperationError::Storage { .. } => ApiErrorCode::Unavailable,
        }, dto::BackupFailureReason::Storage))
    }
}

pub async fn backups_list(context: &ApiContext) -> Result<Vec<dto::BackupIndexView>, ApiError> {
    context.blocking(|context| {
        context.backend().database().backup_index().map_err(|failure| Failure::from(failure).0)?
            .into_iter().map(|entry| {
                let availability = match context.files().describe(&entry.uri) {
                    Ok(_) => dto::BackupAvailability::Available,
                    Err(super::FileAccessError::NotFound) => dto::BackupAvailability::Missing,
                    Err(super::FileAccessError::PermissionDenied) => dto::BackupAvailability::PermissionDenied,
                    Err(_) => dto::BackupAvailability::Unavailable,
                };
                Ok(dto::BackupIndexView {
                    id: entry.id.to_string(), uri: entry.uri,
                    header: dto::BackupHeaderView { version: entry.version, created_at: entry.created_at.get(), app_version: entry.app_version },
                    size: entry.size, availability,
                })
            }).collect()
    }).await
}

async fn mutate_index(context: &ApiContext, request: dto::BackupMutationRequest, delete: bool) -> Result<(), ApiError> {
    let command = if delete { "backup_delete" } else { "backup_forget" };
    super::jobs::local::operation_key(command, &request.client_operation_id)?;
    let digest = super::jobs::local::digest(&request)?;
    context.blocking(move |context| {
        let database = context.backend().database();
        let fresh = delete && database.lookup_api_operation(command, &request.client_operation_id).map_err(|failure| Failure::from(failure).0)?.is_none();
        let pending = if fresh { self::export_jobs::pending_export_targets(context)? } else { Default::default() };
        let _lifecycle = if fresh {
            context.database_files().map(|files| files.location.try_file_lifecycle().map_err(|failure| match failure {
                crate::AppDatabaseLocationError::Busy => file_error(super::FileAccessError::Busy),
                _ => super::app::storage::file_error(failure, None),
            })).transpose()?
        } else { None };
        database.commit_api_operation(command, &request.client_operation_id, &digest, context.now(), |transaction| {
            let id = super::error::parse_id::<lettuce_types::JobId>(&request.id, "id").map_err(Failure)?;
            let entry = transaction.backup_index_record(id)?.ok_or_else(|| Failure(error(ApiErrorCode::NotFound, dto::BackupFailureReason::FileUnavailable)))?;
            if delete {
                if let Some(published) = pending.get(&entry.uri) {
                    return Err(Failure(if *published { error(ApiErrorCode::Unavailable, dto::BackupFailureReason::RecoveryRequired) } else { file_error(super::FileAccessError::Busy) }));
                }
                match context.files().remove(&entry.uri) {
                    Ok(()) | Err(super::FileAccessError::NotFound) => {}
                    Err(failure) => return Err(Failure(file_error(failure))),
                }
            }
            transaction.forget_backup(id)?;
            Ok(())
        }).map_err(|failure: Failure| failure.0)
    }).await
}

pub async fn backup_forget(context: &ApiContext, request: dto::BackupMutationRequest) -> Result<(), ApiError> {
    mutate_index(context, request, false).await
}

pub async fn backup_delete(context: &ApiContext, request: dto::BackupMutationRequest) -> Result<(), ApiError> {
    mutate_index(context, request, true).await
}

pub(super) fn inspect(files: &dyn super::FileAccess, uri: &str) -> Result<dto::BackupHeaderView, ApiError> {
        let malformed = || error(ApiErrorCode::Malformed, dto::BackupFailureReason::InvalidEnvelope);
        let mut input = files.open(uri).map_err(file_error)?;
        let mut prefix = [0_u8; 16];
        input.read_exact(&mut prefix).map_err(|failure| {
            if failure.kind() == std::io::ErrorKind::UnexpectedEof { malformed() }
            else { file_error(super::FileAccessError::Io) }
        })?;
        input.seek(SeekFrom::Start(0)).map_err(|_| file_error(super::FileAccessError::Io))?;
        let header = match lettuce_transfer::detect_backup_format(&prefix).map_err(|_| malformed())? {
            lettuce_transfer::BackupFormatVersion::CurrentV2 => lettuce_transfer::read_backup_header(&mut input).map_err(|failure| if matches!(failure, lettuce_transfer::BackupEnvelopeError::Io { .. }) { file_error(super::FileAccessError::Io) } else { malformed() })?,
            lettuce_transfer::BackupFormatVersion::LegacyV1 => lettuce_transfer::read_legacy_backup_header(input).map_err(|failure| if matches!(failure, lettuce_transfer::LegacyBackupInventoryError::Io { .. }) { file_error(super::FileAccessError::Io) } else { malformed() })?,
        };
        Ok(dto::BackupHeaderView {
            version: header.version,
            created_at: header.created_at.get(),
            app_version: header.app_version,
        })
}
