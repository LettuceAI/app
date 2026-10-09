use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::RequestId;

use super::{
    ApiContext,
    error::{invalid_field, parse_id},
};
use crate::AppDatabaseLocationError;

pub(super) fn file_error(error: AppDatabaseLocationError, file: Option<String>) -> ApiError {
    ApiError {
        code: match error {
            AppDatabaseLocationError::Busy => ApiErrorCode::Busy,
            AppDatabaseLocationError::InUse => ApiErrorCode::InUse,
            AppDatabaseLocationError::Exists | AppDatabaseLocationError::Conflict => {
                ApiErrorCode::Conflict
            }
            AppDatabaseLocationError::NotFound => ApiErrorCode::NotFound,
            AppDatabaseLocationError::Corrupt => ApiErrorCode::Malformed,
            AppDatabaseLocationError::Storage | AppDatabaseLocationError::Platform(_) => {
                ApiErrorCode::Unavailable
            }
        },
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::DatabaseFiles { file }),
    }
}

pub async fn storage_database_files_list(
    context: &ApiContext,
) -> Result<Vec<dto::DatabaseFileView>, ApiError> {
    context
        .blocking(|context| {
            let files = context
                .database_files()
                .ok_or_else(|| file_error(AppDatabaseLocationError::Storage, None))?;
            let lifecycle = files
                .location
                .try_file_lifecycle()
                .map_err(|error| file_error(error, None))?;
            lifecycle
                .inventory(&files.active)
                .map_err(|error| file_error(error, None))?
                .into_iter()
                .map(|file| {
                    let kind = match file.kind {
                        crate::DatabaseFileKind::Initial => dto::DatabaseFileKind::Initial,
                        crate::DatabaseFileKind::Restore => dto::DatabaseFileKind::Restore,
                        crate::DatabaseFileKind::LegacyRestore => {
                            dto::DatabaseFileKind::LegacyRestore
                        }
                        crate::DatabaseFileKind::Reset => dto::DatabaseFileKind::Reset,
                        crate::DatabaseFileKind::Existing => dto::DatabaseFileKind::Existing,
                    };
                    Ok(dto::DatabaseFileView {
                        file: file.file,
                        kind,
                        created_at: file.created_at.get(),
                        size: file.size,
                        active: file.active,
                        deletable: file.deletable,
                    })
                })
                .collect()
        })
        .await
}

pub async fn storage_database_file_delete(
    context: &ApiContext,
    request: dto::DatabaseFileDeleteRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let key: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
            let file = Some(request.file.clone());
            let files = context
                .database_files()
                .ok_or_else(|| file_error(AppDatabaseLocationError::Storage, file.clone()))?;
            let media = context
                .media()
                .ok_or_else(|| file_error(AppDatabaseLocationError::Storage, file.clone()))?;
            files
                .location
                .database_path(&request.file)
                .map_err(|_| invalid_field("file", "invalid database file identifier"))?;
            {
                let lifecycle = files
                    .location
                    .try_file_lifecycle()
                    .map_err(|error| file_error(error, file.clone()))?;
                lifecycle
                    .delete_file(&request.file, &files.active, key)
                    .map_err(|error| file_error(error, file.clone()))?;
            }
            let scope = crate::MediaGarbageScope {
                store: media,
                location: &files.location,
                open_database: &files.active,
            };
            crate::collect_media_garbage(context.backend().database(), &scope, context.now())
                .map_err(|error| ApiError {
                    code: ApiErrorCode::Unavailable,
                    message: error.to_string(),
                    details: Some(dto::ApiErrorDetails::DatabaseFiles { file: file.clone() }),
                })?;
            crate::sweep_orphan_media_files(context.backend().database(), &scope, context.now())
                .map_err(|error| ApiError {
                    code: ApiErrorCode::Unavailable,
                    message: error.to_string(),
                    details: Some(dto::ApiErrorDetails::DatabaseFiles { file }),
                })?;
            Ok(())
        })
        .await
}
