use std::collections::VecDeque;
use std::io::{BufReader, Cursor};
use std::sync::Mutex;

use image::{ImageDecoder, ImageFormat, ImageReader, Limits};
use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_types::{AssetId, ContentHash};

use super::assets::{AssetBytes, AssetRead};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, parse_id};

const PREVIEW_EDGE: u32 = 512;
const CACHE_BYTES: usize = 32 * 1024 * 1024;
const CACHE_ENTRIES: usize = 256;

pub(in crate::api) struct ImagePreviewState {
    permits: tokio::sync::Semaphore,
    cache: Mutex<PreviewCache>,
}

impl Default for ImagePreviewState {
    fn default() -> Self {
        Self {
            permits: tokio::sync::Semaphore::new(2),
            cache: Mutex::new(PreviewCache::default()),
        }
    }
}

#[derive(Default)]
struct PreviewCache {
    entries: VecDeque<(ContentHash, AssetBytes)>,
    bytes: usize,
}

impl PreviewCache {
    fn get(&mut self, hash: &ContentHash) -> Option<AssetBytes> {
        let index = self.entries.iter().position(|(key, _)| key == hash)?;
        let entry = self.entries.remove(index)?;
        let bytes = entry.1.clone();
        self.entries.push_back(entry);
        Some(bytes)
    }

    fn insert(&mut self, hash: ContentHash, preview: AssetBytes) {
        if preview.bytes.len() > CACHE_BYTES {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(key, _)| key == &hash)
            && let Some((_, previous)) = self.entries.remove(index)
        {
            self.bytes -= previous.bytes.len();
        }
        while self.bytes + preview.bytes.len() > CACHE_BYTES || self.entries.len() >= CACHE_ENTRIES {
            if let Some((_, oldest)) = self.entries.pop_front() {
                self.bytes -= oldest.bytes.len();
            }
        }
        self.bytes += preview.bytes.len();
        self.entries.push_back((hash, preview));
    }
}

pub async fn read_asset_preview(context: &ApiContext, asset_id: &str) -> Result<AssetRead, ApiError> {
    let asset_id: AssetId = parse_id(asset_id, "asset_id")?;
    let _permit = context.image_previews().permits.acquire().await.map_err(|_| {
        api_error(ApiErrorCode::Unavailable, "image previews are unavailable")
    })?;
    context.blocking(move |context| {
        let store = context.media().ok_or_else(|| {
            api_error(ApiErrorCode::Unavailable, "the media store is not open")
        })?;
        let opened = store.open_ready(asset_id).map_err(IntoApiError::into_api_error)?;
        if opened.blob.kind != lettuce_media::MediaKind::Image {
            return Err(api_error(ApiErrorCode::InvalidInput, "only images have previews"));
        }
        let state = context.image_previews();
        if let Some(preview) = state.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&opened.blob.content_hash)
        {
            return Ok(AssetRead::Bytes(preview));
        }
        let invalid_image = |error: image::ImageError| {
            api_error(ApiErrorCode::InvalidInput, format!("image preview could not be decoded: {error}"))
        };
        let mut reader = ImageReader::new(BufReader::new(opened.reader))
            .with_guessed_format()
            .map_err(|error| api_error(ApiErrorCode::Internal, format!("image could not be read: {error}")))?;
        let mut limits = Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(invalid_image)?;
        let orientation = decoder.orientation().map_err(invalid_image)?;
        let mut image = image::DynamicImage::from_decoder(decoder).map_err(invalid_image)?;
        image.apply_orientation(orientation);
        let image = if image.width() > PREVIEW_EDGE || image.height() > PREVIEW_EDGE {
            image.thumbnail(PREVIEW_EDGE, PREVIEW_EDGE)
        } else {
            image
        };
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::WebP).map_err(invalid_image)?;
        let bytes = bytes.into_inner();
        let preview = AssetBytes {
            mime_type: "image/webp".into(),
            total_len: bytes.len() as u64,
            start: 0,
            bytes,
        };
        state.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(opened.blob.content_hash, preview.clone());
        Ok(AssetRead::Bytes(preview))
    }).await
}

#[cfg(test)]
mod tests;
