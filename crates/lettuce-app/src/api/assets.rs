use std::io::{Read, Seek, SeekFrom};

use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_types::AssetId;

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};

/// The most bytes an open-ended range (`start..`) returns at once; the
/// client asks again for the rest.
pub const OPEN_RANGE_CHUNK: u64 = 4 * 1024 * 1024;

/// The bytes of an asset a request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetRange {
    Whole,
    /// `start..=end`; an end past the asset stops at its last byte.
    Between {
        start: u64,
        end: u64,
    },
    /// From `start`, at most `OPEN_RANGE_CHUNK` bytes.
    From {
        start: u64,
    },
    /// The last `len` bytes.
    Last {
        len: u64,
    },
}

impl AssetRange {
    /// The first byte and length this range covers in an asset of
    /// `total_len` bytes; `None` when it covers none.
    #[must_use]
    pub fn resolve(self, total_len: u64) -> Option<(u64, u64)> {
        match self {
            Self::Whole => Some((0, total_len)),
            Self::Between { start, end } if start < total_len && start <= end => {
                Some((start, end.min(total_len - 1) - start + 1))
            }
            Self::From { start } if start < total_len => {
                Some((start, (total_len - start).min(OPEN_RANGE_CHUNK)))
            }
            Self::Last { len } if len > 0 && total_len > 0 => {
                let len = len.min(total_len);
                Some((total_len - len, len))
            }
            Self::Between { .. } | Self::From { .. } | Self::Last { .. } => None,
        }
    }
}

/// Bytes read from a ready media asset, for the host's asset URI scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetBytes {
    pub mime_type: String,
    /// The asset's whole size.
    pub total_len: u64,
    /// Where `bytes` starts in the asset.
    pub start: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetRead {
    Bytes(AssetBytes),
    /// The range covers no byte of the asset.
    Unsatisfiable {
        total_len: u64,
    },
}

/// Reads the requested bytes of a ready asset; only those bytes are held in
/// memory.
pub async fn read_asset(
    context: &ApiContext,
    asset_id: &str,
    range: AssetRange,
) -> Result<AssetRead, ApiError> {
    let asset_id: AssetId = parse_id(asset_id, "asset_id")?;
    context
        .blocking(move |context| {
            let store = context.media().ok_or_else(|| {
                api_error(ApiErrorCode::Unavailable, "the media store is not open")
            })?;
            let mut opened = store
                .open_ready(asset_id)
                .map_err(IntoApiError::into_api_error)?;
            let total_len = opened.blob.byte_size;
            let Some((start, len)) = range.resolve(total_len) else {
                return Ok(AssetRead::Unsatisfiable { total_len });
            };
            let read_error = |error: std::io::Error| {
                api_error(
                    ApiErrorCode::Internal,
                    format!("media asset could not be read: {error}"),
                )
            };
            opened
                .reader
                .seek(SeekFrom::Start(start))
                .map_err(read_error)?;
            let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or_default());
            (&mut opened.reader)
                .take(len)
                .read_to_end(&mut bytes)
                .map_err(read_error)?;
            if bytes.len() as u64 != len {
                return Err(api_error(
                    ApiErrorCode::Internal,
                    "media asset is shorter than its catalog size",
                ));
            }
            Ok(AssetRead::Bytes(AssetBytes {
                mime_type: opened.blob.mime_type,
                total_len,
                start,
                bytes,
            }))
        })
        .await
}
