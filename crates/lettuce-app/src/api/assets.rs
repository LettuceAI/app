use std::io::Read;

use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_types::AssetId;

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};

/// A ready media asset's bytes, for the host's asset URI scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetBytes {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

pub async fn read_asset(context: &ApiContext, asset_id: &str) -> Result<AssetBytes, ApiError> {
    let asset_id: AssetId = parse_id(asset_id, "asset_id")?;
    context
        .blocking(move |context| {
            let store = context.media().ok_or_else(|| {
                api_error(ApiErrorCode::Unavailable, "the media store is not open")
            })?;
            let mut opened = store
                .open_ready(asset_id)
                .map_err(IntoApiError::into_api_error)?;
            let mut bytes = Vec::new();
            opened.reader.read_to_end(&mut bytes).map_err(|error| {
                api_error(
                    ApiErrorCode::Internal,
                    format!("media asset could not be read: {error}"),
                )
            })?;
            Ok(AssetBytes {
                mime_type: opened.blob.mime_type,
                bytes,
            })
        })
        .await
}
