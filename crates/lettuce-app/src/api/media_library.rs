use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_media::{MediaLibraryError, MediaStoreError};
use lettuce_types::{AssetId, RequestId};

use super::error::{IntoApiError, invalid_field, parse_id};
use super::{ApiContext, FileAccessError};

fn media_error(asset_id: Option<String>, error: MediaStoreError) -> ApiError {
    let reason = match error {
        MediaStoreError::AssetNotFound => dto::MediaFailureReason::AssetMissing,
        MediaStoreError::BlobNotFound => dto::MediaFailureReason::BlobMissing,
        MediaStoreError::ObjectMissing => dto::MediaFailureReason::ObjectMissing,
        MediaStoreError::NotReady => dto::MediaFailureReason::NotReady,
        MediaStoreError::AssetBlobKindMismatch
        | MediaStoreError::ObjectMetadataMismatch
        | MediaStoreError::RepositoryData => dto::MediaFailureReason::InvalidMetadata,
        _ => dto::MediaFailureReason::Storage,
    };
    let mut error = error.into_api_error();
    error.details = Some(dto::ApiErrorDetails::Media { asset_id, reason });
    error
}

fn host_unavailable(asset_id: Option<String>) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: "media storage is unavailable".into(),
        details: Some(dto::ApiErrorDetails::Media {
            asset_id,
            reason: dto::MediaFailureReason::HostUnavailable,
        }),
    }
}

fn file_error(asset_id: String, error: FileAccessError) -> ApiError {
    let mut api = error.into_api_error();
    if error == FileAccessError::SourceIsTarget {
        api.code = ApiErrorCode::Conflict;
    }
    api.details = Some(dto::ApiErrorDetails::Media {
        asset_id: Some(asset_id),
        reason: if error == FileAccessError::SourceIsTarget {
            dto::MediaFailureReason::ProtectedTarget
        } else {
            dto::MediaFailureReason::Storage
        },
    });
    api
}

pub(super) fn reference_view(reference: lettuce_media::MediaReference) -> dto::MediaReferenceView {
    use lettuce_media::MediaReferenceKind as Kind;
    let kind = match reference.kind {
        Kind::Character => dto::MediaReferenceKind::Character,
        Kind::Persona => dto::MediaReferenceKind::Persona,
        Kind::Group => dto::MediaReferenceKind::Group,
        Kind::Scene => dto::MediaReferenceKind::Scene,
        Kind::Conversation => dto::MediaReferenceKind::Conversation,
        Kind::Creation => dto::MediaReferenceKind::Creation,
        Kind::Speech => dto::MediaReferenceKind::Speech,
        Kind::Image => dto::MediaReferenceKind::Image,
        Kind::Memory => dto::MediaReferenceKind::Memory,
        Kind::Companion => dto::MediaReferenceKind::Companion,
        Kind::Model => dto::MediaReferenceKind::Model,
        Kind::Settings => dto::MediaReferenceKind::Settings,
        Kind::Lorebook => dto::MediaReferenceKind::Lorebook,
        Kind::Prompt => dto::MediaReferenceKind::Prompt,
        Kind::Job => dto::MediaReferenceKind::Job,
        Kind::LegacyImport => dto::MediaReferenceKind::LegacyImport,
        Kind::Sync => dto::MediaReferenceKind::Sync,
        Kind::Usage => dto::MediaReferenceKind::Usage,
        Kind::Transfer => dto::MediaReferenceKind::Transfer,
    };
    dto::MediaReferenceView {
        kind,
        owner_id: reference.owner_id,
    }
}

fn library_error(asset_id: String, error: MediaLibraryError) -> ApiError {
    let code = match error {
        MediaLibraryError::NotFound => ApiErrorCode::NotFound,
        MediaLibraryError::InUse(_) => ApiErrorCode::InUse,
        MediaLibraryError::Conflict => ApiErrorCode::Conflict,
        MediaLibraryError::InvalidCursor => ApiErrorCode::InvalidInput,
        MediaLibraryError::InvalidData => ApiErrorCode::Malformed,
        MediaLibraryError::Storage => ApiErrorCode::Unavailable,
    };
    let message = error.to_string();
    let details = match error {
        MediaLibraryError::InvalidCursor => dto::ApiErrorDetails::InvalidField {
            field: "cursor".into(),
        },
        MediaLibraryError::InUse(references) => dto::ApiErrorDetails::MediaInUse {
            asset_id,
            references: references.into_iter().map(reference_view).collect(),
        },
        error => dto::ApiErrorDetails::Media {
            asset_id: Some(asset_id),
            reason: match error {
                MediaLibraryError::NotFound => dto::MediaFailureReason::AssetMissing,
                MediaLibraryError::InvalidData => dto::MediaFailureReason::InvalidMetadata,
                _ => dto::MediaFailureReason::Storage,
            },
        },
    };
    ApiError {
        code,
        message,
        details: Some(details),
    }
}

pub async fn media_library_remove(
    context: &ApiContext,
    request: dto::MediaLibraryRemoveRequest,
) -> Result<(), ApiError> {
    media_library_remove_with_checkpoint(context, request, || {}).await
}

pub(super) async fn media_library_remove_with_checkpoint(
    context: &ApiContext,
    request: dto::MediaLibraryRemoveRequest,
    after_catalog: impl FnOnce() + Send + 'static,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let id: AssetId = parse_id(&request.asset_id, "asset_id")?;
            let key: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
            let media = context
                .media()
                .ok_or_else(|| host_unavailable(Some(request.asset_id.clone())))?;
            let files = context.database_files().ok_or_else(|| {
                super::storage::file_error(crate::AppDatabaseLocationError::Storage, None)
            })?;
            let lifecycle = files
                .location
                .try_file_lifecycle()
                .map_err(|error| super::storage::file_error(error, None))?;
            if files
                .location
                .active_path()
                .map_err(|error| super::storage::file_error(error, None))?
                != files.active
            {
                return Err(super::storage::file_error(
                    crate::AppDatabaseLocationError::Conflict,
                    None,
                ));
            }
            let now = context.clock().now();
            let blocked = crate::deletion::hard_delete::skip_for_unreadable_files(
                &lifecycle,
                context.backend().database(),
                now,
            )
            .map_err(|error| ApiError {
                code: ApiErrorCode::Unavailable,
                message: error.to_string(),
                details: Some(dto::ApiErrorDetails::DatabaseFiles { file: None }),
            })?;
            let kept = lifecycle
                .kept_media_hashes()
                .map_err(|error| super::storage::file_error(error, None))?;
            let digest = blake3::hash(request.asset_id.as_bytes())
                .to_hex()
                .to_string();
            let mut database_error = None;
            let removed = media
                .remove_released_objects(|| {
                    context
                        .backend()
                        .database()
                        .remove_media_library_asset(id, key, &digest, now)
                        .and_then(|released| {
                            after_catalog();
                            let mut removable = Vec::new();
                            for object in released {
                                if blocked
                                    || kept.contains(&object.content_hash)
                                    || context
                                        .backend()
                                        .database()
                                        .media_object_retained(&object.content_hash)
                                        .map_err(|_| MediaLibraryError::Storage)?
                                {
                                    continue;
                                }
                                removable.push(object);
                            }
                            Ok(removable)
                        })
                        .map_err(|error| {
                            database_error = Some(error);
                            MediaStoreError::CatalogFailure
                        })
                })
                .map_err(|error| {
                    database_error.map_or_else(
                        || media_error(Some(request.asset_id.clone()), error),
                        |error| library_error(request.asset_id.clone(), error),
                    )
                })?;
            if removed.failed > 0 {
                return Err(media_error(
                    Some(request.asset_id.clone()),
                    MediaStoreError::ObjectRemovalFailed,
                ));
            }
            drop(lifecycle);
            let scope = crate::MediaGarbageScope {
                store: media,
                location: &files.location,
                open_database: &files.active,
            };
            crate::sweep_orphan_media_files(context.backend().database(), &scope, now).map_err(
                |error| match error {
                    crate::HardDeleteError::Media(error) => {
                        media_error(Some(request.asset_id.clone()), error)
                    }
                    crate::HardDeleteError::DatabaseFiles(error) => {
                        super::storage::file_error(error, None)
                    }
                    crate::HardDeleteError::Purge(_) => {
                        library_error(request.asset_id.clone(), MediaLibraryError::Storage)
                    }
                },
            )?;
            Ok(())
        })
        .await
}

pub async fn media_library_list(
    context: &ApiContext,
    request: dto::MediaLibraryListRequest,
) -> Result<dto::MediaLibraryPage, ApiError> {
    context
        .blocking(move |context| {
            let role = request.kind.map(|role| match role {
                dto::MediaLibraryRole::Image => lettuce_media::MediaKind::Image,
                dto::MediaLibraryRole::Audio => lettuce_media::MediaKind::Audio,
            });
            let page = context
                .backend()
                .database()
                .media_library_page(
                    role,
                    lettuce_types::PageRequest {
                        cursor: request.cursor,
                        limit: lettuce_types::PageLimit::new(
                            u16::try_from(request.limit.unwrap_or(50)).unwrap_or(u16::MAX),
                        ),
                    },
                )
                .map_err(|error| {
                    let mut error = library_error(String::new(), error);
                    if let Some(dto::ApiErrorDetails::Media { asset_id, .. }) = &mut error.details {
                        *asset_id = None;
                    }
                    error
                })?;
            let media = context.media().ok_or_else(|| host_unavailable(None))?;
            let mut items = Vec::with_capacity(page.items.len());
            for entry in page.items {
                media
                    .open_ready(entry.asset.id)
                    .map_err(|error| media_error(Some(entry.asset.id.to_string()), error))?;
                use lettuce_media::AssetKind as Kind;
                let asset_kind = match entry.asset.kind {
                    Kind::AvatarOriginal => dto::MediaLibraryAssetKind::AvatarOriginal,
                    Kind::BackgroundImage => dto::MediaLibraryAssetKind::BackgroundImage,
                    Kind::Illustration => dto::MediaLibraryAssetKind::Illustration,
                    Kind::LorebookIcon => dto::MediaLibraryAssetKind::LorebookIcon,
                    Kind::MessageImage => dto::MediaLibraryAssetKind::MessageImage,
                    Kind::MessageAudio => dto::MediaLibraryAssetKind::MessageAudio,
                    Kind::GeneratedImage => dto::MediaLibraryAssetKind::GeneratedImage,
                    Kind::SynthesizedSpeech => dto::MediaLibraryAssetKind::SynthesizedSpeech,
                    Kind::OtherImage => dto::MediaLibraryAssetKind::OtherImage,
                    Kind::OtherAudio => dto::MediaLibraryAssetKind::OtherAudio,
                    Kind::SourceDocument => dto::MediaLibraryAssetKind::SourceDocument,
                };
                let retention = match entry.asset.retention {
                    lettuce_media::RetentionClass::Persistent => {
                        dto::MediaLibraryRetention::Persistent
                    }
                    lettuce_media::RetentionClass::Library => dto::MediaLibraryRetention::Library,
                    lettuce_media::RetentionClass::Temporary { .. } => {
                        dto::MediaLibraryRetention::Temporary
                    }
                };
                let role = match entry.blob.kind {
                    lettuce_media::MediaKind::Image => dto::MediaLibraryRole::Image,
                    lettuce_media::MediaKind::Audio => dto::MediaLibraryRole::Audio,
                    _ => {
                        return Err(library_error(
                            entry.asset.id.to_string(),
                            MediaLibraryError::InvalidData,
                        ));
                    }
                };
                items.push(dto::MediaLibraryItem {
                    asset: context.asset_ref(entry.asset.id),
                    role,
                    asset_kind,
                    retention,
                    expires_at: entry
                        .asset
                        .retention
                        .expires_at()
                        .map(lettuce_types::TimestampMillis::get),
                    mime_type: entry.blob.mime_type,
                    byte_size: entry.blob.byte_size,
                    width: entry.blob.width,
                    height: entry.blob.height,
                    duration_ms: entry.blob.duration_ms,
                    source_label: entry.asset.provenance.source_label,
                    created_at: entry.asset.created_at.get(),
                    updated_at: entry.asset.updated_at.get(),
                    references: entry.references.into_iter().map(reference_view).collect(),
                });
            }
            Ok(dto::MediaLibraryPage {
                items,
                next_cursor: page.next_cursor,
            })
        })
        .await
}

pub async fn media_save_to(
    context: &ApiContext,
    request: dto::MediaSaveToRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let id: AssetId = parse_id(&request.asset_id, "asset_id")?;
            if request.target.uri.trim().is_empty() {
                return Err(invalid_field("target.uri", "export target is empty"));
            }
            let media = context.media().ok_or_else(|| ApiError {
                code: ApiErrorCode::Unavailable,
                message: "media storage is unavailable".into(),
                details: Some(dto::ApiErrorDetails::Media {
                    asset_id: Some(request.asset_id.clone()),
                    reason: dto::MediaFailureReason::HostUnavailable,
                }),
            })?;
            let protection = super::files::FileExportProtection::new(context)?;
            let mut opened = media
                .open_ready(id)
                .map_err(|error| media_error(Some(request.asset_id.clone()), error))?;
            let mut target = context
                .files()
                .create_export(&request.target.uri, None, &|uri, target| {
                    protection.protects(uri, target)
                })
                .map_err(|error| file_error(request.asset_id.clone(), error))?;
            std::io::copy(&mut opened.reader, &mut target)
                .and_then(|_| std::io::Write::flush(&mut target))
                .map_err(|_| file_error(request.asset_id, FileAccessError::Io))
        })
        .await
}
