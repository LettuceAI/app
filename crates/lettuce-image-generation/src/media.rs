use std::io::Read;

use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, MediaKind, MediaStoreError, RetentionClass,
};
use lettuce_types::{AssetId, JobId, ModelProfileId};

use crate::{GeneratedImage, ImageInput, ImageOutputPolicy, ProviderImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageMediaError {
    #[error("an input image is unavailable")]
    InputUnavailable,
    #[error("an input asset is not an image")]
    InputNotImage,
    #[error("image output was rejected: {0}")]
    Output(MediaStoreError),
}

/// Reads request images and stores generated ones through media validation.
pub trait ImageMedia: Send + Sync {
    fn load_input(&self, asset_id: AssetId) -> Result<ImageInput, ImageMediaError>;
    fn ingest_output(
        &self,
        job_id: JobId,
        model_profile_id: ModelProfileId,
        policy: ImageOutputPolicy,
        image: ProviderImage,
    ) -> Result<GeneratedImage, ImageMediaError>;
}

impl<BR, AR> ImageMedia for LocalMediaBlobStore<BR, AR>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    fn load_input(&self, asset_id: AssetId) -> Result<ImageInput, ImageMediaError> {
        let mut opened = self
            .open_ready(asset_id)
            .map_err(|_| ImageMediaError::InputUnavailable)?;
        if opened.blob.kind != MediaKind::Image {
            return Err(ImageMediaError::InputNotImage);
        }
        let mut bytes = Vec::new();
        opened
            .reader
            .read_to_end(&mut bytes)
            .map_err(|_| ImageMediaError::InputUnavailable)?;
        Ok(ImageInput {
            mime_type: opened.blob.mime_type,
            bytes,
        })
    }

    fn ingest_output(
        &self,
        job_id: JobId,
        model_profile_id: ModelProfileId,
        policy: ImageOutputPolicy,
        image: ProviderImage,
    ) -> Result<GeneratedImage, ImageMediaError> {
        let retention = match policy {
            ImageOutputPolicy::Retained => RetentionClass::Persistent,
            ImageOutputPolicy::Preview { expires_at } => RetentionClass::Temporary { expires_at },
        };
        let mut request = IngestRequest::new(
            AssetKind::GeneratedImage,
            AssetOrigin::Generated,
            retention,
            AssetProvenanceV1 {
                producing_job_id: Some(job_id),
                model_profile_id: Some(model_profile_id),
                source_label: Some("image_generation".into()),
                ..AssetProvenanceV1::default()
            },
        );
        if let Some(mime_type) = image.declared_mime_type {
            request = request.with_declared_mime_type(mime_type);
        }
        let ingested = self
            .ingest(image.bytes.as_slice(), request)
            .map_err(ImageMediaError::Output)?;
        Ok(GeneratedImage {
            asset_id: ingested.asset.id,
            content_hash: ingested.blob.content_hash,
            mime_type: ingested.blob.mime_type,
            byte_size: ingested.blob.byte_size,
            width: ingested.blob.width,
            height: ingested.blob.height,
            text: image.text,
        })
    }
}
