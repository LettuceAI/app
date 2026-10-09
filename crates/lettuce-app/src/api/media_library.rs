use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_media::MediaStoreError;
use lettuce_types::AssetId;

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
