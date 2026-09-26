use std::io::{Read, Seek, SeekFrom, Write};

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, MediaKind, RetentionClass,
};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field};

/// A readable file the host opened.
pub trait FileReader: Read + Seek + Send {}

impl<T: Read + Seek + Send> FileReader for T {}

/// The name and size the host reports for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDescription {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileAccessError {
    #[error("the file does not exist")]
    NotFound,
    #[error("the file cannot be accessed")]
    PermissionDenied,
    #[error("the file location is not supported")]
    Unsupported,
    #[error("the file could not be read or written")]
    Io,
}

/// How the backend reaches files the user picked or a save dialog chose;
/// the host implements it for its platform, so the backend sees no platform
/// types.
pub trait FileAccess: Send + Sync {
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError>;

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError>;

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError>;
}

impl IntoApiError for FileAccessError {
    fn into_api_error(self) -> ApiError {
        let code = match self {
            Self::NotFound => ApiErrorCode::NotFound,
            Self::PermissionDenied | Self::Io => ApiErrorCode::Unavailable,
            Self::Unsupported => ApiErrorCode::Unsupported,
        };
        api_error(code, self.to_string())
    }
}

fn source_uri(source: &dto::FileSource) -> Result<&str, ApiError> {
    let uri = source.uri.trim();
    if uri.is_empty() {
        return Err(invalid_field("source", "source is empty"));
    }
    Ok(uri)
}

/// Names a picked file and detects what it holds, for drag and drop and
/// "import anything".
pub async fn files_inspect(
    context: &ApiContext,
    request: dto::FilesInspectRequest,
) -> Result<dto::FileInspection, ApiError> {
    let uri = source_uri(&request.source)?.to_owned();
    context
        .blocking(move |context| {
            let files = context.files();
            let description = files.describe(&uri).map_err(IntoApiError::into_api_error)?;
            let mut reader = files.open(&uri).map_err(IntoApiError::into_api_error)?;
            let kind = super::file_kind::detect_kind(reader.as_mut())
                .map_err(IntoApiError::into_api_error)?;
            Ok(dto::FileInspection {
                name: description.name,
                size: description.size,
                kind,
            })
        })
        .await
}

/// Stores a picked image or audio file as a media asset; the media store
/// checks the content against what the role accepts.
pub async fn assets_ingest(
    context: &ApiContext,
    request: dto::AssetsIngestRequest,
) -> Result<dto::AssetRef, ApiError> {
    let uri = source_uri(&request.source)?.to_owned();
    let role = request.role;
    context
        .blocking(move |context| {
            let media = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            let mut reader = context
                .files()
                .open(&uri)
                .map_err(IntoApiError::into_api_error)?;
            let kind = match role {
                dto::AssetIngestRole::Avatar => AssetKind::AvatarOriginal,
                dto::AssetIngestRole::Background => AssetKind::BackgroundImage,
                dto::AssetIngestRole::VoiceExample => AssetKind::OtherAudio,
                dto::AssetIngestRole::ReferenceImage => AssetKind::OtherImage,
                dto::AssetIngestRole::Attachment => {
                    let header = super::file_kind::read_header(reader.as_mut())
                        .map_err(IntoApiError::into_api_error)?;
                    reader
                        .seek(SeekFrom::Start(0))
                        .map_err(|_| FileAccessError::Io.into_api_error())?;
                    match lettuce_media::sniff_media_kind(&header) {
                        Some(MediaKind::Audio) => AssetKind::MessageAudio,
                        _ => AssetKind::MessageImage,
                    }
                }
            };
            let ingested = media
                .ingest(
                    reader,
                    IngestRequest::new(
                        kind,
                        AssetOrigin::Upload,
                        RetentionClass::Persistent,
                        AssetProvenanceV1::default(),
                    ),
                )
                .map_err(|error| match error {
                    lettuce_media::MediaStoreError::UnsupportedFormat
                    | lettuce_media::MediaStoreError::KindMismatch
                    | lettuce_media::MediaStoreError::InvalidHeader
                    | lettuce_media::MediaStoreError::EmptyInput
                    | lettuce_media::MediaStoreError::InputTooLarge
                    | lettuce_media::MediaStoreError::PixelLimitExceeded
                    | lettuce_media::MediaStoreError::InvalidDimensions => {
                        invalid_field("source", error.to_string())
                    }
                    error => error.into_api_error(),
                })?;
            Ok(context.asset_ref(ingested.asset.id))
        })
        .await
}
