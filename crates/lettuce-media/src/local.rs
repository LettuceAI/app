//! A bounded, local content-addressed media ingestion/use-case adapter.

use std::io::{self, Read};

use blake3::Hash;
use lettuce_platform::{
    ManagedFiles, ObjectKey, ObjectKind, ParentSyncStatus, PlatformError, ReadCapability,
    ReadHandle, WriteCapability,
};
use lettuce_types::{AssetId, ContentHash, MediaBlobId, Revision, TimestampMillis};

use crate::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaAssetRepositoryError, MediaBlob, MediaBlobRepository, MediaBlobRepositoryError, MediaKind,
    RetentionClass,
};

/// The first local ingestion slice is deliberately bounded to 64 MiB per
/// object. This is also below the platform facade's maximum read size.
pub const MAX_MEDIA_BLOB_BYTES: u64 = 64 * 1024 * 1024;
/// Image dimensions are read from bounded headers only; no decoder is used.
/// This guards downstream decoders from pathological allocation requests.
const MAX_IMAGE_PIXELS: u64 = 100_000_000;
const BLOB_VALIDATION_VERSION: u32 = 1;

/// Input metadata supplied by the logical asset owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestRequest {
    pub asset_kind: AssetKind,
    pub origin: AssetOrigin,
    pub retention: RetentionClass,
    pub provenance: AssetProvenanceV1,
    pub declared_mime_type: Option<String>,
}

impl IngestRequest {
    #[must_use]
    pub fn new(
        asset_kind: AssetKind,
        origin: AssetOrigin,
        retention: RetentionClass,
        provenance: AssetProvenanceV1,
    ) -> Self {
        Self {
            asset_kind,
            origin,
            retention,
            provenance,
            declared_mime_type: None,
        }
    }

    #[must_use]
    pub fn with_declared_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.declared_mime_type = Some(mime_type.into());
        self
    }
}

/// The result of ingestion. A blob may be shared by many logically distinct
/// assets, so callers receive both records without receiving a native path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestedMedia {
    pub asset: MediaAsset,
    pub blob: MediaBlob,
}

/// A ready asset opened through a descriptor-backed managed read handle.
pub struct OpenedMediaAsset {
    pub asset: MediaAsset,
    pub blob: MediaBlob,
    pub reader: ReadHandle,
}

impl std::fmt::Debug for OpenedMediaAsset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenedMediaAsset")
            .field("asset", &self.asset)
            .field("blob", &self.blob)
            .field("reader", &self.reader)
            .finish()
    }
}

/// Redacted service errors. None of the variants carry input bytes, native
/// paths, URIs, provenance labels or provider/source data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaStoreError {
    #[error("media input could not be read")]
    InputRead,
    #[error("media input is empty")]
    EmptyInput,
    #[error("media input exceeds the bounded object limit")]
    InputTooLarge,
    #[error("media input format is not allowlisted")]
    UnsupportedFormat,
    #[error("media input has an invalid or truncated header")]
    InvalidHeader,
    #[error("media input kind does not match the requested asset kind")]
    KindMismatch,
    #[error("media input MIME type does not match its detected format")]
    MimeMismatch,
    #[error("media input dimensions exceed the pixel limit")]
    PixelLimitExceeded,
    #[error("media input dimensions are invalid")]
    InvalidDimensions,
    #[error("media input metadata is invalid")]
    InvalidMetadata,
    #[error("media timestamp could not be obtained")]
    Clock,
    #[error("media object could not be committed")]
    File(PlatformError),
    #[error("media object parent synchronization failed")]
    CommitSyncFailed,
    #[error("content-addressed object conflicts with an existing object")]
    ObjectConflict,
    #[error("media catalog operation failed after the object was committed")]
    CatalogFailure,
    #[error("media asset was not found")]
    AssetNotFound,
    #[error("media blob was not found")]
    BlobNotFound,
    #[error("media blob is not ready")]
    NotReady,
    #[error("media asset and blob kinds are incompatible")]
    AssetBlobKindMismatch,
    #[error("media object is missing")]
    ObjectMissing,
    #[error("media object metadata do not match its catalog record")]
    ObjectMetadataMismatch,
    #[error("media repository returned invalid data")]
    RepositoryData,
}

/// Local media blob service. The only filesystem authority it receives is the
/// purpose-scoped `MediaBlobs` capabilities minted by composition.
pub struct LocalMediaBlobStore<BR, AR> {
    files: ManagedFiles,
    read_capability: ReadCapability,
    write_capability: WriteCapability,
    blobs: BR,
    assets: AR,
}

impl<BR, AR> std::fmt::Debug for LocalMediaBlobStore<BR, AR> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LocalMediaBlobStore(<managed capabilities>)")
    }
}

impl<BR, AR> LocalMediaBlobStore<BR, AR> {
    #[must_use]
    pub fn new(
        files: ManagedFiles,
        read_capability: ReadCapability,
        write_capability: WriteCapability,
        blobs: BR,
        assets: AR,
    ) -> Self {
        Self {
            files,
            read_capability,
            write_capability,
            blobs,
            assets,
        }
    }
}

impl<BR, AR> LocalMediaBlobStore<BR, AR>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    /// Reads, bounds, header-sniffs, content-addresses and catalogs one local
    /// image/audio object.
    pub fn ingest<R: Read>(
        &self,
        input: R,
        request: IngestRequest,
    ) -> Result<IngestedMedia, MediaStoreError> {
        request
            .provenance
            .validate()
            .map_err(|_| MediaStoreError::InvalidMetadata)?;
        let bytes = read_bounded(input)?;
        let sniffed = if request.asset_kind == AssetKind::SourceDocument {
            sniff_document(&bytes)?
        } else {
            sniff(&bytes)?
        };
        if sniffed.kind != request.asset_kind.blob_kind() {
            return Err(MediaStoreError::KindMismatch);
        }
        if let Some(declared) = &request.declared_mime_type {
            if !mime_matches(declared, sniffed.mime_type) {
                return Err(MediaStoreError::MimeMismatch);
            }
        }

        let now = TimestampMillis::now().map_err(|_| MediaStoreError::Clock)?;
        let hash = blake3::hash(&bytes);
        let content_hash = content_hash(hash);
        let object_key = object_key(&content_hash)?;
        let expected_size = bytes.len() as u64;
        let blob_id = MediaBlobId::new();
        let staged_blob = MediaBlob {
            id: blob_id,
            content_hash,
            kind: sniffed.kind,
            mime_type: sniffed.mime_type.to_owned(),
            byte_size: expected_size,
            width: sniffed.width,
            height: sniffed.height,
            duration_ms: None,
            validation_version: BLOB_VALIDATION_VERSION,
            state: BlobState::Staged,
            created_at: now,
            updated_at: now,
        };
        staged_blob
            .validate()
            .map_err(|_| MediaStoreError::InvalidMetadata)?;

        self.commit_object(
            &object_key,
            expected_size,
            &staged_blob.content_hash,
            &bytes,
        )?;

        let registered = self
            .blobs
            .register(staged_blob)
            .map_err(|_| MediaStoreError::CatalogFailure)?;
        if registered.kind != sniffed.kind
            || registered.mime_type != sniffed.mime_type
            || registered.byte_size != expected_size
            || registered.width != sniffed.width
            || registered.height != sniffed.height
        {
            return Err(MediaStoreError::CatalogFailure);
        }

        let blob = match registered.state {
            BlobState::Ready => registered,
            BlobState::Staged => self
                .blobs
                .finalize_staged_to_ready(registered.id, now)
                .map_err(|_| MediaStoreError::CatalogFailure)?,
            BlobState::Quarantined | BlobState::Missing => {
                return Err(MediaStoreError::CatalogFailure);
            }
        };

        let asset = MediaAsset::new(
            AssetId::new(),
            blob.id,
            request.asset_kind,
            request.origin,
            request.retention,
            request.provenance,
            Revision::INITIAL,
            now,
            now,
        )
        .map_err(|_| MediaStoreError::CatalogFailure)?;
        let asset = self
            .assets
            .create(asset)
            .map_err(|_| MediaStoreError::CatalogFailure)?;
        Ok(IngestedMedia { asset, blob })
    }

    /// Opens only a ready catalog asset and returns a descriptor-backed reader.
    pub fn open_ready(&self, asset_id: AssetId) -> Result<OpenedMediaAsset, MediaStoreError> {
        let asset = self
            .assets
            .get(asset_id)
            .map_err(|error| match error {
                MediaAssetRepositoryError::NotFound => MediaStoreError::AssetNotFound,
                _ => MediaStoreError::RepositoryData,
            })?
            .ok_or(MediaStoreError::AssetNotFound)?;
        let blob = self
            .blobs
            .get(asset.blob_id)
            .map_err(|error| match error {
                MediaBlobRepositoryError::NotFound => MediaStoreError::BlobNotFound,
                _ => MediaStoreError::RepositoryData,
            })?
            .ok_or(MediaStoreError::BlobNotFound)?;
        asset
            .validate()
            .map_err(|_| MediaStoreError::RepositoryData)?;
        if asset.kind.blob_kind() != blob.kind {
            return Err(MediaStoreError::AssetBlobKindMismatch);
        }
        if blob.state != BlobState::Ready {
            return Err(MediaStoreError::NotReady);
        }
        let key = object_key(&blob.content_hash)?;
        let metadata =
            self.files
                .metadata(&self.read_capability, &key)
                .map_err(|error| match error {
                    PlatformError::NotFound => MediaStoreError::ObjectMissing,
                    other => MediaStoreError::File(other),
                })?;
        if metadata.kind != ObjectKind::File {
            return Err(MediaStoreError::ObjectMetadataMismatch);
        }
        if metadata.len != blob.byte_size {
            return Err(MediaStoreError::ObjectMetadataMismatch);
        }
        let reader =
            self.files
                .open_read(&self.read_capability, &key)
                .map_err(|error| match error {
                    PlatformError::NotFound => MediaStoreError::ObjectMissing,
                    other => MediaStoreError::File(other),
                })?;
        Ok(OpenedMediaAsset {
            asset,
            blob,
            reader,
        })
    }

    fn commit_object(
        &self,
        key: &ObjectKey,
        expected_size: u64,
        expected_hash: &ContentHash,
        bytes: &[u8],
    ) -> Result<(), MediaStoreError> {
        let mut staged = self
            .files
            .stage_new(&self.write_capability, key.clone())
            .map_err(MediaStoreError::File)?;
        std::io::Write::write_all(&mut staged, bytes).map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidInput {
                MediaStoreError::InputTooLarge
            } else {
                MediaStoreError::InputRead
            }
        })?;
        match staged.commit() {
            Ok(receipt) => {
                require_available_parent(receipt.parent_sync)?;
                Ok(())
            }
            Err(PlatformError::Conflict) => {
                let metadata = self
                    .files
                    .metadata(&self.read_capability, key)
                    .map_err(MediaStoreError::File)?;
                if metadata.kind != ObjectKind::File || metadata.len != expected_size {
                    Err(MediaStoreError::ObjectConflict)
                } else {
                    let mut existing = self
                        .files
                        .open_read(&self.read_capability, key)
                        .map_err(MediaStoreError::File)?;
                    let existing =
                        read_bounded(&mut existing).map_err(|_| MediaStoreError::ObjectConflict)?;
                    if content_hash(blake3::hash(&existing)) == *expected_hash {
                        Ok(())
                    } else {
                        Err(MediaStoreError::ObjectConflict)
                    }
                }
            }
            Err(error) => Err(MediaStoreError::File(error)),
        }
    }
}

fn require_available_parent(status: ParentSyncStatus) -> Result<(), MediaStoreError> {
    if status == ParentSyncStatus::Failed {
        Err(MediaStoreError::CommitSyncFailed)
    } else {
        Ok(())
    }
}

fn read_bounded<R: Read>(mut input: R) -> Result<Vec<u8>, MediaStoreError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|_| MediaStoreError::InputRead)?;
        if read == 0 {
            break;
        }
        if (bytes.len() as u64).saturating_add(read as u64) > MAX_MEDIA_BLOB_BYTES {
            return Err(MediaStoreError::InputTooLarge);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.is_empty() {
        Err(MediaStoreError::EmptyInput)
    } else {
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy)]
struct SniffedMedia {
    kind: MediaKind,
    mime_type: &'static str,
    width: Option<u32>,
    height: Option<u32>,
}

// This is bounded signature/header sniffing only. It does not decode bodies,
// verify CRCs or prove that a complete media stream is present.
fn sniff_document(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    let mime_type = if bytes.starts_with(b"%PDF-") {
        if bytes.len() < 8
            || !matches!(
                &bytes[5..8],
                b"1.0" | b"1.1" | b"1.2" | b"1.3" | b"1.4" | b"1.5" | b"1.6" | b"1.7" | b"2.0"
            )
        {
            return Err(MediaStoreError::InvalidHeader);
        }
        "application/pdf"
    } else {
        std::str::from_utf8(bytes).map_err(|_| MediaStoreError::UnsupportedFormat)?;
        "text/plain"
    };
    Ok(SniffedMedia {
        kind: MediaKind::Document,
        mime_type,
        width: None,
        height: None,
    })
}

fn sniff(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return sniff_png(bytes);
    }
    if bytes.starts_with(b"\xff\xd8") {
        return sniff_jpeg(bytes);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return sniff_gif(bytes);
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return sniff_webp(bytes);
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return sniff_wav(bytes);
    }
    if bytes.starts_with(b"OggS") {
        return sniff_ogg(bytes);
    }
    if bytes.starts_with(b"fLaC") {
        return sniff_flac(bytes);
    }
    if bytes.get(4..8) == Some(b"ftyp") {
        return sniff_m4a(bytes);
    }
    if bytes.starts_with(b"ID3") || looks_like_mp3_frame(bytes) {
        return sniff_mp3(bytes);
    }
    Err(MediaStoreError::UnsupportedFormat)
}

fn sniff_png(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 24 || be_u32(&bytes[8..12])? != 13 {
        return Err(MediaStoreError::InvalidHeader);
    }
    if &bytes[12..16] != b"IHDR" {
        return Err(MediaStoreError::InvalidHeader);
    }
    dimensions(
        be_u32(&bytes[16..20])?,
        be_u32(&bytes[20..24])?,
        "image/png",
    )
}

fn sniff_gif(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 10 {
        return Err(MediaStoreError::InvalidHeader);
    }
    dimensions(
        u32::from(le_u16(&bytes[6..8])?),
        u32::from(le_u16(&bytes[8..10])?),
        "image/gif",
    )
}

fn sniff_webp(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 20 || riff_size(bytes)? < 4 || riff_size(bytes)? > bytes.len() - 8 {
        return Err(MediaStoreError::InvalidHeader);
    }
    if &bytes[12..16] == b"VP8X" {
        if bytes.len() < 30 || le_u32(&bytes[16..20])? < 10 {
            return Err(MediaStoreError::InvalidHeader);
        }
        return dimensions(
            1 + (u32::from(bytes[24]) | (u32::from(bytes[25]) << 8) | (u32::from(bytes[26]) << 16)),
            1 + (u32::from(bytes[27]) | (u32::from(bytes[28]) << 8) | (u32::from(bytes[29]) << 16)),
            "image/webp",
        );
    }
    if &bytes[12..16] == b"VP8 " {
        if bytes.len() < 30 || bytes[23..26] != [0x9d, 0x01, 0x2a] {
            return Err(MediaStoreError::InvalidHeader);
        }
        return dimensions(
            u32::from(le_u16(&bytes[26..28])? & 0x3fff),
            u32::from(le_u16(&bytes[28..30])? & 0x3fff),
            "image/webp",
        );
    }
    if &bytes[12..16] == b"VP8L" {
        if bytes.len() < 25 || bytes[20] != 0x2f {
            return Err(MediaStoreError::InvalidHeader);
        }
        let bits = le_u32(&bytes[21..25])?;
        return dimensions(
            1 + (bits & 0x3fff),
            1 + ((bits >> 14) & 0x3fff),
            "image/webp",
        );
    }
    Err(MediaStoreError::InvalidHeader)
}

fn sniff_jpeg(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    let mut cursor = 2usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor] == 0xff {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return Err(MediaStoreError::InvalidHeader);
        }
        let marker = bytes[cursor];
        cursor += 1;
        if marker == 0xda || marker == 0xd9 {
            return Err(MediaStoreError::InvalidHeader);
        }
        if marker == 0x00 || marker == 0xff {
            continue;
        }
        if cursor + 2 > bytes.len() {
            return Err(MediaStoreError::InvalidHeader);
        }
        let segment_length = usize::from(u16::from_be_bytes(
            bytes[cursor..cursor + 2]
                .try_into()
                .map_err(|_| MediaStoreError::InvalidHeader)?,
        ));
        if segment_length < 2 || cursor + segment_length > bytes.len() {
            return Err(MediaStoreError::InvalidHeader);
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if segment_length < 7 {
                return Err(MediaStoreError::InvalidHeader);
            }
            return dimensions(
                u32::from(be_u16(&bytes[cursor + 5..cursor + 7])?),
                u32::from(be_u16(&bytes[cursor + 3..cursor + 5])?),
                "image/jpeg",
            );
        }
        cursor += segment_length;
    }
    Err(MediaStoreError::InvalidHeader)
}

fn sniff_wav(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 12 || riff_size(bytes)? < 4 || riff_size(bytes)? > bytes.len() - 8 {
        return Err(MediaStoreError::InvalidHeader);
    }
    let mut cursor = 12usize;
    let mut saw_format = false;
    while cursor + 8 <= bytes.len() {
        let size = usize::try_from(le_u32(&bytes[cursor + 4..cursor + 8])?)
            .map_err(|_| MediaStoreError::InvalidHeader)?;
        let end = cursor
            .checked_add(8)
            .and_then(|value| value.checked_add(size));
        let Some(end) = end else {
            return Err(MediaStoreError::InvalidHeader);
        };
        if end > bytes.len() {
            return Err(MediaStoreError::InvalidHeader);
        }
        if &bytes[cursor..cursor + 4] == b"fmt " {
            if size < 16
                || le_u16(&bytes[cursor + 8..cursor + 10])? == 0
                || le_u16(&bytes[cursor + 10..cursor + 12])? == 0
                || le_u32(&bytes[cursor + 12..cursor + 16])? == 0
            {
                return Err(MediaStoreError::InvalidHeader);
            }
            saw_format = true;
        }
        cursor = end + (size & 1);
    }
    if saw_format {
        Ok(SniffedMedia {
            kind: MediaKind::Audio,
            mime_type: "audio/wav",
            width: None,
            height: None,
        })
    } else {
        Err(MediaStoreError::InvalidHeader)
    }
}

fn sniff_ogg(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 27 || bytes[4] != 0 || bytes[26] == 0 {
        return Err(MediaStoreError::InvalidHeader);
    }
    let end = 27usize + usize::from(bytes[26]);
    if end > bytes.len() {
        return Err(MediaStoreError::InvalidHeader);
    }
    let payload = bytes[27..end]
        .iter()
        .map(|value| usize::from(*value))
        .sum::<usize>();
    if end
        .checked_add(payload)
        .is_none_or(|value| value > bytes.len())
    {
        return Err(MediaStoreError::InvalidHeader);
    }
    Ok(SniffedMedia {
        kind: MediaKind::Audio,
        mime_type: "audio/ogg",
        width: None,
        height: None,
    })
}

fn sniff_flac(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 8 {
        return Err(MediaStoreError::InvalidHeader);
    }
    let header = bytes[4];
    let size = (usize::from(bytes[5]) << 16) | (usize::from(bytes[6]) << 8) | usize::from(bytes[7]);
    if header & 0x7f != 0
        || size < 34
        || 8usize.checked_add(size).is_none_or(|end| end > bytes.len())
    {
        return Err(MediaStoreError::InvalidHeader);
    }
    Ok(SniffedMedia {
        kind: MediaKind::Audio,
        mime_type: "audio/flac",
        width: None,
        height: None,
    })
}

fn sniff_m4a(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    if bytes.len() < 12 {
        return Err(MediaStoreError::InvalidHeader);
    }
    let size = usize::try_from(be_u32(&bytes[..4])?).map_err(|_| MediaStoreError::InvalidHeader)?;
    if size < 8 || size > bytes.len() || bytes[8..12] != *b"M4A " {
        return Err(MediaStoreError::InvalidHeader);
    }
    Ok(SniffedMedia {
        kind: MediaKind::Audio,
        mime_type: "audio/mp4",
        width: None,
        height: None,
    })
}

fn sniff_mp3(bytes: &[u8]) -> Result<SniffedMedia, MediaStoreError> {
    let start = if bytes.starts_with(b"ID3") {
        if bytes.len() < 10 || bytes[3] == 0xff || bytes[3] & 0xe0 != 0 {
            return Err(MediaStoreError::InvalidHeader);
        }
        let size = (usize::from(bytes[6] & 0x7f) << 21)
            | (usize::from(bytes[7] & 0x7f) << 14)
            | (usize::from(bytes[8] & 0x7f) << 7)
            | usize::from(bytes[9] & 0x7f);
        10usize
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .ok_or(MediaStoreError::InvalidHeader)?
    } else {
        0
    };
    if !looks_like_mp3_frame(&bytes[start..]) {
        return Err(MediaStoreError::InvalidHeader);
    }
    Ok(SniffedMedia {
        kind: MediaKind::Audio,
        mime_type: "audio/mpeg",
        width: None,
        height: None,
    })
}

fn looks_like_mp3_frame(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|header| {
        header[0] == 0xff
            && header[1] & 0xe0 == 0xe0
            && header[1] & 0x06 != 0
            && header[2] & 0xf0 != 0
            && header[2] & 0xf0 != 0xf0
            && header[2] & 0x0c != 0x0c
    })
}

fn dimensions(
    width: u32,
    height: u32,
    mime_type: &'static str,
) -> Result<SniffedMedia, MediaStoreError> {
    if width == 0 || height == 0 {
        return Err(MediaStoreError::InvalidDimensions);
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(MediaStoreError::PixelLimitExceeded)?;
    if pixels > MAX_IMAGE_PIXELS {
        return Err(MediaStoreError::PixelLimitExceeded);
    }
    Ok(SniffedMedia {
        kind: MediaKind::Image,
        mime_type,
        width: Some(width),
        height: Some(height),
    })
}

fn riff_size(bytes: &[u8]) -> Result<usize, MediaStoreError> {
    let value = bytes.get(4..8).ok_or(MediaStoreError::InvalidHeader)?;
    let size = usize::try_from(le_u32(value)?).map_err(|_| MediaStoreError::InvalidHeader)?;
    Ok(size)
}

fn be_u16(bytes: &[u8]) -> Result<u16, MediaStoreError> {
    bytes
        .try_into()
        .map(u16::from_be_bytes)
        .map_err(|_| MediaStoreError::InvalidHeader)
}

fn le_u16(bytes: &[u8]) -> Result<u16, MediaStoreError> {
    bytes
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| MediaStoreError::InvalidHeader)
}

fn be_u32(bytes: &[u8]) -> Result<u32, MediaStoreError> {
    bytes
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| MediaStoreError::InvalidHeader)
}

fn le_u32(bytes: &[u8]) -> Result<u32, MediaStoreError> {
    bytes
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| MediaStoreError::InvalidHeader)
}

fn content_hash(hash: Hash) -> ContentHash {
    // blake3's hex output is always a valid ContentHash.
    ContentHash::parse(hash.to_hex().to_string()).expect("blake3 hex has fixed length")
}

fn object_key(hash: &ContentHash) -> Result<ObjectKey, MediaStoreError> {
    ObjectKey::from_segments([
        "objects",
        &hash.as_str()[..2],
        &hash.as_str()[2..4],
        hash.as_str(),
    ])
    .map_err(MediaStoreError::File)
}

fn mime_matches(declared: &str, detected: &str) -> bool {
    if detected == "text/plain" && declared.trim().eq_ignore_ascii_case("text/markdown") {
        return true;
    }
    let normalized = declared.trim();
    match detected {
        "image/jpeg" => {
            normalized.eq_ignore_ascii_case("image/jpeg")
                || normalized.eq_ignore_ascii_case("image/jpg")
        }
        "audio/wav" => matches!(
            normalized,
            value if value.eq_ignore_ascii_case("audio/wav")
                || value.eq_ignore_ascii_case("audio/x-wav")
                || value.eq_ignore_ascii_case("audio/wave")
        ),
        "audio/mpeg" => {
            normalized.eq_ignore_ascii_case("audio/mpeg")
                || normalized.eq_ignore_ascii_case("audio/mp3")
        }
        "audio/ogg" => {
            normalized.eq_ignore_ascii_case("audio/ogg")
                || normalized.eq_ignore_ascii_case("audio/vorbis")
        }
        "audio/flac" => {
            normalized.eq_ignore_ascii_case("audio/flac")
                || normalized.eq_ignore_ascii_case("audio/x-flac")
        }
        "audio/mp4" => matches!(
            normalized,
            value if value.eq_ignore_ascii_case("audio/mp4")
                || value.eq_ignore_ascii_case("audio/m4a")
                || value.eq_ignore_ascii_case("audio/x-m4a")
        ),
        _ => normalized.eq_ignore_ascii_case(detected),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, io::Read as _, sync::Mutex};

    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_types::{ContentHash, Page, PageRequest};

    use crate::{MediaAssetRepositoryError, MediaBlobRepositoryError};

    use super::*;

    #[derive(Default)]
    struct BlobMemory {
        values: Mutex<BTreeMap<MediaBlobId, MediaBlob>>,
    }

    impl MediaBlobRepository for BlobMemory {
        fn register(&self, blob: MediaBlob) -> Result<MediaBlob, MediaBlobRepositoryError> {
            if blob.state != BlobState::Staged {
                return Err(MediaBlobRepositoryError::InvalidState);
            }
            let mut values = self
                .values
                .lock()
                .map_err(|_| MediaBlobRepositoryError::Storage)?;
            if let Some(existing) = values
                .values()
                .find(|existing| existing.content_hash == blob.content_hash)
            {
                return Ok(existing.clone());
            }
            values.insert(blob.id, blob.clone());
            Ok(blob)
        }

        fn finalize_staged_to_ready(
            &self,
            id: MediaBlobId,
            updated_at: TimestampMillis,
        ) -> Result<MediaBlob, MediaBlobRepositoryError> {
            let mut values = self
                .values
                .lock()
                .map_err(|_| MediaBlobRepositoryError::Storage)?;
            let blob = values
                .get_mut(&id)
                .ok_or(MediaBlobRepositoryError::NotFound)?;
            if blob.state == BlobState::Ready {
                return Ok(blob.clone());
            }
            if blob.state != BlobState::Staged {
                return Err(MediaBlobRepositoryError::InvalidState);
            }
            blob.state = BlobState::Ready;
            blob.updated_at = updated_at;
            Ok(blob.clone())
        }

        fn get(&self, id: MediaBlobId) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
            Ok(self
                .values
                .lock()
                .map_err(|_| MediaBlobRepositoryError::Storage)?
                .get(&id)
                .cloned())
        }

        fn find_by_hash(
            &self,
            hash: &ContentHash,
        ) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
            Ok(self
                .values
                .lock()
                .map_err(|_| MediaBlobRepositoryError::Storage)?
                .values()
                .find(|blob| &blob.content_hash == hash)
                .cloned())
        }
    }

    #[derive(Default)]
    struct AssetMemory {
        values: Mutex<BTreeMap<AssetId, MediaAsset>>,
    }

    impl MediaAssetRepository for AssetMemory {
        fn create(&self, asset: MediaAsset) -> Result<MediaAsset, MediaAssetRepositoryError> {
            let mut values = self
                .values
                .lock()
                .map_err(|_| MediaAssetRepositoryError::Storage)?;
            if values.contains_key(&asset.id) {
                return Err(MediaAssetRepositoryError::AlreadyExists);
            }
            values.insert(asset.id, asset.clone());
            Ok(asset)
        }

        fn get(&self, id: AssetId) -> Result<Option<MediaAsset>, MediaAssetRepositoryError> {
            Ok(self
                .values
                .lock()
                .map_err(|_| MediaAssetRepositoryError::Storage)?
                .get(&id)
                .cloned())
        }

        fn update_retention(
            &self,
            _id: AssetId,
            _expected_revision: Revision,
            _retention: RetentionClass,
            _updated_at: TimestampMillis,
        ) -> Result<MediaAsset, MediaAssetRepositoryError> {
            Err(MediaAssetRepositoryError::Storage)
        }

        fn list_library(
            &self,
            _request: PageRequest,
        ) -> Result<Page<MediaAsset>, MediaAssetRepositoryError> {
            Err(MediaAssetRepositoryError::Storage)
        }
    }

    fn png_fixture() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"payload");
        bytes
    }

    #[test]
    fn local_store_ingests_deduplicates_and_opens_ready_content() {
        let root = std::env::temp_dir().join(format!("lettuce-media-{}", AssetId::new()));
        let snapshot = DirectorySnapshot::new(&root).expect("snapshot");
        let authority = FilesystemAuthority::new(&snapshot).expect("authority");
        let files = authority.managed_files();
        let read = authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read capability");
        let write = authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write capability");
        let store = LocalMediaBlobStore::new(
            files,
            read,
            write,
            BlobMemory::default(),
            AssetMemory::default(),
        );
        let input = png_fixture();
        let first = store
            .ingest(
                input.as_slice(),
                IngestRequest::new(
                    AssetKind::OtherImage,
                    AssetOrigin::Upload,
                    RetentionClass::Library,
                    AssetProvenanceV1::default(),
                )
                .with_declared_mime_type("image/png"),
            )
            .expect("first ingest");
        let second = store
            .ingest(
                input.as_slice(),
                IngestRequest::new(
                    AssetKind::Illustration,
                    AssetOrigin::Import,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("deduplicated ingest");
        assert_eq!(first.blob.id, second.blob.id);
        assert_ne!(first.asset.id, second.asset.id);
        let mut opened = store.open_ready(first.asset.id).expect("open ready");
        let mut read_back = Vec::new();
        opened.reader.read_to_end(&mut read_back).expect("read");
        assert_eq!(read_back, input);
        for (bytes, mime) in [
            (b"%PDF-1.7\nsource".as_slice(), "application/pdf"),
            ("# World 🌍\nSource notes".as_bytes(), "text/markdown"),
        ] {
            let request = IngestRequest::new(
                AssetKind::SourceDocument,
                AssetOrigin::Upload,
                RetentionClass::Library,
                AssetProvenanceV1::default(),
            )
            .with_declared_mime_type(mime);
            let document = store
                .ingest(bytes, request.clone())
                .expect("source document");
            assert_eq!(document.blob.kind, MediaKind::Document);
            let duplicate = store.ingest(bytes, request).expect("deduplicated source");
            assert_eq!(document.blob.id, duplicate.blob.id);
            let mut source = store
                .open_ready(document.asset.id)
                .expect("protected document reader");
            let mut content = Vec::new();
            source
                .reader
                .read_to_end(&mut content)
                .expect("document bytes");
            assert_eq!(content, bytes);
        }
        for bytes in [b"%PDF-x.y".as_slice(), &[0xff]] {
            assert!(
                store
                    .ingest(
                        bytes,
                        IngestRequest::new(
                            AssetKind::SourceDocument,
                            AssetOrigin::Upload,
                            RetentionClass::Library,
                            AssetProvenanceV1::default(),
                        )
                    )
                    .is_err()
            );
        }
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn same_size_conflicting_object_is_rejected_by_content_hash() {
        let root = std::env::temp_dir().join(format!("lettuce-media-{}", AssetId::new()));
        let snapshot = DirectorySnapshot::new(&root).expect("snapshot");
        let authority = FilesystemAuthority::new(&snapshot).expect("authority");
        let files = authority.managed_files();
        let read = authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read capability");
        let write = authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write capability");
        let store = LocalMediaBlobStore::new(
            files.clone(),
            read,
            write.clone(),
            BlobMemory::default(),
            AssetMemory::default(),
        );
        let input = png_fixture();
        let first = store
            .ingest(
                input.as_slice(),
                IngestRequest::new(
                    AssetKind::OtherImage,
                    AssetOrigin::Upload,
                    RetentionClass::Library,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("first ingest");
        let key = object_key(&first.blob.content_hash).expect("object key");
        let mut tampered = input.clone();
        let last = tampered.last_mut().expect("fixture bytes");
        *last ^= 1;
        files
            .write_atomic(&write, key, &tampered)
            .expect("tamper object");
        assert_eq!(
            store
                .ingest(
                    input.as_slice(),
                    IngestRequest::new(
                        AssetKind::OtherImage,
                        AssetOrigin::Upload,
                        RetentionClass::Library,
                        AssetProvenanceV1::default(),
                    ),
                )
                .expect_err("same-size wrong hash must fail"),
            MediaStoreError::ObjectConflict
        );
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn header_sniffing_rejects_kind_and_mime_mismatches() {
        let image = png_fixture();
        assert_eq!(sniff(&image).expect("sniff").width, Some(2));
        assert!(matches!(
            sniff(&image),
            Ok(SniffedMedia {
                kind: MediaKind::Image,
                mime_type: "image/png",
                ..
            })
        ));
        assert!(mime_matches("image/png", "image/png"));
        assert!(!mime_matches("audio/mpeg", "image/png"));
        assert!(matches!(
            sniff(&image[..8]),
            Err(MediaStoreError::InvalidHeader)
        ));
    }

    #[test]
    fn commit_parent_sync_accepts_available_statuses_only() {
        assert!(require_available_parent(ParentSyncStatus::Synced).is_ok());
        assert!(require_available_parent(ParentSyncStatus::Unsupported).is_ok());
        assert_eq!(
            require_available_parent(ParentSyncStatus::Failed),
            Err(MediaStoreError::CommitSyncFailed)
        );
    }
}
