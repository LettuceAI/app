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
                        created_at: file.created_at.map(lettuce_types::TimestampMillis::get),
                        modified_at: file.modified_at.get(),
                        size: file.size,
                        active: file.active,
                        deletable: file.deletable,
                        error: file
                            .unreadable
                            .then_some(dto::DatabaseFileError::Unreadable),
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

pub(super) fn summary_error(section: &str) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: "storage summary is unavailable".into(),
        details: Some(dto::ApiErrorDetails::StorageSummary {
            section: section.into(),
        }),
    }
}

pub async fn storage_summary(context: &ApiContext) -> Result<dto::StorageSummary, ApiError> {
    context
        .blocking(|context| {
            let files = context
                .database_files()
                .ok_or_else(|| summary_error("database"))?;
            let lifecycle = files
                .location
                .try_file_lifecycle()
                .map_err(|error| file_error(error, None))?;
            let inventory = lifecycle
                .inventory(&files.active)
                .map_err(|error| file_error(error, None))?;
            let protected = lifecycle
                .kept_media_hashes()
                .map_err(|error| file_error(error, None))?;
            let mut active_database_bytes = 0_u64;
            let mut kept_database_bytes = 0_u64;
            let mut media_objects = std::collections::BTreeMap::new();
            for file in inventory {
                if !file.active
                    && !lifecycle
                        .is_complete(&file.file)
                        .map_err(|error| file_error(error, None))?
                {
                    continue;
                }
                let path = files
                    .location
                    .database_path(&file.file)
                    .map_err(|error| file_error(error, None))?;
                let mut size = file.size;
                for suffix in ["-wal", "-shm"] {
                    let sidecar = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
                    match std::fs::symlink_metadata(sidecar) {
                        Ok(metadata) if metadata.is_file() => {
                            size = size
                                .checked_add(metadata.len())
                                .ok_or_else(|| summary_error("database"))?
                        }
                        Ok(_) => return Err(summary_error("database")),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => return Err(summary_error("database")),
                    }
                }
                let total = if file.active {
                    &mut active_database_bytes
                } else {
                    &mut kept_database_bytes
                };
                *total = total
                    .checked_add(size)
                    .ok_or_else(|| summary_error("database"))?;
                for (hash, kind, size) in
                    lettuce_database::Database::media_storage_objects_in_file(&path)
                        .map_err(|_| summary_error("media"))?
                {
                    if (file.active || protected.contains(&hash))
                        && media_objects
                            .insert(hash, (kind.clone(), size))
                            .is_some_and(|previous| previous != (kind, size))
                    {
                        return Err(summary_error("media"));
                    }
                }
            }
            let mut media = std::collections::BTreeMap::from([
                ("image".to_owned(), 0_u64),
                ("audio".to_owned(), 0),
                ("video".to_owned(), 0),
                ("document".to_owned(), 0),
            ]);
            for (kind, size) in media_objects.into_values() {
                let total = media.get_mut(&kind).ok_or_else(|| summary_error("media"))?;
                *total = total
                    .checked_add(size)
                    .ok_or_else(|| summary_error("media"))?;
            }
            let folder = context
                .app_folder()
                .ok_or_else(|| summary_error("models"))?;
            let active = files
                .location
                .active_path()
                .map_err(|error| file_error(error, None))?;
            let device = lettuce_database::Database::storage_device_settings_in_file(&active)
                .map_err(|_| summary_error("models"))?;
            let retained = crate::speech::speech_roots::retained_model_roots(&device, folder);
            let mut roots = vec![("llm".into(), crate::llm_models_root(&device, folder))];
            roots.extend(
                crate::image_model_roots(&device, folder)
                    .into_iter()
                    .map(|root| ("image".into(), root)),
            );
            for (kind, root) in [
                ("whisper", retained.whisper),
                ("kokoro", retained.kokoro),
                ("embedding", retained.embedding),
                ("thymos", retained.thymos),
            ] {
                roots.push((
                    kind.into(),
                    root.map(std::path::PathBuf::from)
                        .ok_or_else(|| summary_error("models"))?,
                ));
            }
            let diffusion =
                lettuce_image_generation::sd_runtime::layout::DiffusionPaths::legacy_layout(
                    folder,
                    crate::image_model_roots(&device, folder)
                        .into_iter()
                        .next()
                        .ok_or_else(|| summary_error("models"))?,
                );
            roots.push(("lora".into(), diffusion.loras));
            roots.push(("upscaler".into(), diffusion.upscalers));
            let models = context.model_storage_sizes(roots)?;
            let logs = context.logs()?;
            let mut logs_bytes = 0_u64;
            for entry in
                std::fs::read_dir(logs.directory.path()).map_err(|_| summary_error("logs"))?
            {
                let entry = entry.map_err(|_| summary_error("logs"))?;
                let metadata = entry
                    .path()
                    .symlink_metadata()
                    .map_err(|_| summary_error("logs"))?;
                if metadata.is_file() {
                    logs_bytes = logs_bytes
                        .checked_add(metadata.len())
                        .ok_or_else(|| summary_error("logs"))?;
                }
            }
            Ok(dto::StorageSummary {
                active_database_bytes,
                kept_database_bytes,
                media: media
                    .into_iter()
                    .map(|(kind, bytes)| dto::StorageSize { kind, bytes })
                    .collect(),
                models,
                logs_bytes,
            })
        })
        .await
}
