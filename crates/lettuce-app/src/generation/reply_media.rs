//! Images a chat model returns with its reply, stored as media assets when
//! the reply is finalized.

use base64::Engine as _;
use lettuce_conversations::{GeneratedMedia, InferenceCandidate, MediaAssetRole, MessagePart};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, MediaStoreError, RetentionClass,
};
use lettuce_transfer::PersonaFileRepository;
use lettuce_types::{AssetId, GenerationAttemptId, JobId, ModelProfileId, TimestampMillis};
use uuid::Uuid;

/// Where a reply image came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyImageOrigin {
    pub job_id: JobId,
    pub model_profile_id: ModelProfileId,
}

/// Stores reply images in the media store.
pub trait ReplyMediaStore: Send + Sync {
    /// Stores `bytes` as the image asset `asset_id`; storing the same bytes
    /// under the same id again returns that asset.
    fn store_reply_image(
        &self,
        asset_id: AssetId,
        origin: ReplyImageOrigin,
        bytes: &[u8],
    ) -> Result<AssetId, MediaStoreError>;

    /// Deletes a stored reply image and frees its bytes when no message
    /// links it; a failure is only logged.
    fn discard_reply_image(&self, asset_id: AssetId, now: TimestampMillis);
}

impl std::fmt::Debug for dyn ReplyMediaStore + '_ {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ReplyMediaStore")
    }
}

/// The media store reply images are written to, with the catalog that
/// deletes an image no message links.
pub struct ReplyMediaAssets<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    store: &'a LocalMediaBlobStore<BR, AR>,
}

impl<R: ?Sized, BR, AR> std::fmt::Debug for ReplyMediaAssets<'_, R, BR, AR> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ReplyMediaAssets")
    }
}

impl<'a, R: ?Sized, BR, AR> ReplyMediaAssets<'a, R, BR, AR> {
    #[must_use]
    pub const fn new(repository: &'a R, store: &'a LocalMediaBlobStore<BR, AR>) -> Self {
        Self { repository, store }
    }
}

impl<R, BR, AR> ReplyMediaStore for ReplyMediaAssets<'_, R, BR, AR>
where
    R: PersonaFileRepository + ?Sized,
    BR: MediaBlobRepository + Send + Sync,
    AR: MediaAssetRepository + Send + Sync,
{
    fn store_reply_image(
        &self,
        asset_id: AssetId,
        origin: ReplyImageOrigin,
        bytes: &[u8],
    ) -> Result<AssetId, MediaStoreError> {
        self.store
            .ingest_with_id(
                asset_id,
                bytes,
                IngestRequest::new(
                    AssetKind::GeneratedImage,
                    AssetOrigin::Generated,
                    RetentionClass::Persistent,
                    AssetProvenanceV1 {
                        producing_job_id: Some(origin.job_id),
                        model_profile_id: Some(origin.model_profile_id),
                        source_label: Some("chat_reply".into()),
                        ..AssetProvenanceV1::default()
                    },
                ),
            )
            .map(|ingested| ingested.asset.id)
    }

    fn discard_reply_image(&self, asset_id: AssetId, now: TimestampMillis) {
        let blob_id = match self.repository.discard_unlinked_asset(asset_id) {
            Ok(Some(blob_id)) => blob_id,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%asset_id, %error, "unused reply image was not deleted");
                return;
            }
        };
        if let Err(error) = self.store.release_blob(blob_id, |_| {
            self.repository
                .release_unused_blob(blob_id, now)
                .map_err(|_| MediaStoreError::CatalogFailure)
        }) {
            tracing::warn!(%asset_id, %error, "unused reply image bytes were not deleted");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReplyMediaError {
    #[error("the reply carries images but no media store is attached")]
    NoStore,
    #[error("a reply image could not be stored: {0}")]
    Store(MediaStoreError),
}

/// The asset id of the `index`th image of an attempt's reply, stable across
/// a retried finalization so a stored image is found again.
fn reply_image_asset_id(attempt_id: GenerationAttemptId, index: usize) -> AssetId {
    AssetId::from_uuid(Uuid::new_v5(
        &attempt_id.as_uuid(),
        format!("reply-image-{index}").as_bytes(),
    ))
}

/// Stores the candidate's images, appends a media part for each after its
/// other parts and answers the stored asset ids. An image whose bytes are not
/// a supported image is left out and logged. When an image cannot be stored,
/// the ones already stored are discarded.
pub(crate) fn attach_reply_media(
    store: Option<&dyn ReplyMediaStore>,
    attempt_id: GenerationAttemptId,
    origin: ReplyImageOrigin,
    candidate: &mut InferenceCandidate,
    now: TimestampMillis,
) -> Result<Vec<AssetId>, ReplyMediaError> {
    let media = std::mem::take(&mut candidate.media);
    if media.is_empty() {
        return Ok(Vec::new());
    }
    let store = store.ok_or(ReplyMediaError::NoStore)?;
    let mut stored = Vec::with_capacity(media.len());
    for (index, image) in media.iter().enumerate() {
        let Some(bytes) = decode(image) else {
            tracing::warn!(
                index,
                mime_type = %image.mime_type,
                "reply image is not valid base64; left out of the reply"
            );
            continue;
        };
        match store.store_reply_image(reply_image_asset_id(attempt_id, index), origin, &bytes) {
            Ok(asset_id) => {
                stored.push(asset_id);
                candidate.parts.push(MessagePart::MediaAsset {
                    asset_id,
                    role: MediaAssetRole::Attachment,
                });
            }
            Err(
                error @ (MediaStoreError::EmptyInput
                | MediaStoreError::UnsupportedFormat
                | MediaStoreError::InvalidHeader
                | MediaStoreError::KindMismatch
                | MediaStoreError::MimeMismatch
                | MediaStoreError::PixelLimitExceeded
                | MediaStoreError::InvalidDimensions
                | MediaStoreError::InputTooLarge),
            ) => {
                tracing::warn!(
                    index,
                    mime_type = %image.mime_type,
                    %error,
                    "reply image was refused by the media store; left out of the reply"
                );
            }
            Err(error) => {
                discard_reply_media(store, &stored, now);
                return Err(ReplyMediaError::Store(error));
            }
        }
    }
    Ok(stored)
}

/// Discards the images an attempt stored for a reply that was not finalized;
/// an image a message links is kept.
pub(crate) fn discard_reply_media(
    store: &dyn ReplyMediaStore,
    stored: &[AssetId],
    now: TimestampMillis,
) {
    for asset_id in stored {
        store.discard_reply_image(*asset_id, now);
    }
}

fn decode(image: &GeneratedMedia) -> Option<Vec<u8>> {
    let data: String = image
        .base64_data
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(&data)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&data))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&data))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&data))
        .ok()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorded(Mutex<Vec<(AssetId, Vec<u8>)>>, Mutex<Vec<AssetId>>);

    impl ReplyMediaStore for Recorded {
        fn store_reply_image(
            &self,
            asset_id: AssetId,
            _origin: ReplyImageOrigin,
            bytes: &[u8],
        ) -> Result<AssetId, MediaStoreError> {
            if bytes == b"bad" {
                return Err(MediaStoreError::UnsupportedFormat);
            }
            if bytes == b"down" {
                return Err(MediaStoreError::CatalogFailure);
            }
            self.0
                .lock()
                .expect("store lock")
                .push((asset_id, bytes.to_vec()));
            Ok(asset_id)
        }

        fn discard_reply_image(&self, asset_id: AssetId, _now: TimestampMillis) {
            self.1.lock().expect("discard lock").push(asset_id);
        }
    }

    fn candidate(media: Vec<GeneratedMedia>) -> InferenceCandidate {
        InferenceCandidate {
            ordinal: 0,
            parts: vec![MessagePart::Text { text: "hi".into() }],
            tool_calls: Vec::new(),
            provider_replay: None,
            media,
        }
    }

    fn image(data: &str) -> GeneratedMedia {
        GeneratedMedia {
            mime_type: "image/png".into(),
            base64_data: data.into(),
        }
    }

    #[test]
    fn images_become_attachment_parts_with_stable_ids_and_bad_ones_are_left_out() {
        let store = Recorded::default();
        let attempt_id = GenerationAttemptId::new();
        let origin = ReplyImageOrigin {
            job_id: JobId::new(),
            model_profile_id: ModelProfileId::new(),
        };
        let mut reply = candidate(vec![
            image("AQID"),
            image("!!"),
            image("YmFk"),
            image("BAU"),
        ]);
        let now = TimestampMillis::new(1);
        let ids =
            attach_reply_media(Some(&store), attempt_id, origin, &mut reply, now).expect("stored");
        assert!(reply.media.is_empty());
        let stored = store.0.lock().expect("store lock").clone();
        assert_eq!(
            stored,
            vec![
                (reply_image_asset_id(attempt_id, 0), vec![1, 2, 3]),
                (reply_image_asset_id(attempt_id, 3), vec![4, 5]),
            ]
        );
        assert_eq!(
            reply.parts[1..],
            [
                MessagePart::MediaAsset {
                    asset_id: stored[0].0,
                    role: MediaAssetRole::Attachment,
                },
                MessagePart::MediaAsset {
                    asset_id: stored[1].0,
                    role: MediaAssetRole::Attachment,
                },
            ]
        );
        assert_eq!(ids, vec![stored[0].0, stored[1].0]);
        assert_eq!(
            reply_image_asset_id(attempt_id, 0),
            reply_image_asset_id(attempt_id, 0)
        );

        let mut unstored = candidate(vec![image("AQID")]);
        assert_eq!(
            attach_reply_media(None, attempt_id, origin, &mut unstored, now),
            Err(ReplyMediaError::NoStore)
        );
        let mut plain = candidate(Vec::new());
        attach_reply_media(None, attempt_id, origin, &mut plain, now).expect("nothing to store");
        assert!(store.1.lock().expect("discard lock").is_empty());
    }

    #[test]
    fn a_store_failure_discards_the_images_already_stored() {
        let store = Recorded::default();
        let attempt_id = GenerationAttemptId::new();
        let origin = ReplyImageOrigin {
            job_id: JobId::new(),
            model_profile_id: ModelProfileId::new(),
        };
        let mut reply = candidate(vec![image("AQID"), image("ZG93bg=="), image("BAU")]);
        assert_eq!(
            attach_reply_media(
                Some(&store),
                attempt_id,
                origin,
                &mut reply,
                TimestampMillis::new(1)
            ),
            Err(ReplyMediaError::Store(MediaStoreError::CatalogFailure))
        );
        assert_eq!(
            *store.1.lock().expect("discard lock"),
            vec![reply_image_asset_id(attempt_id, 0)]
        );
    }
}
