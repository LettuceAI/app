//! Images a chat model returns with its reply, stored as media assets when
//! the reply is finalized.

use base64::Engine as _;
use lettuce_conversations::{GeneratedMedia, InferenceCandidate, MediaAssetRole, MessagePart};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, MediaStoreError, RetentionClass,
};
use lettuce_types::{AssetId, GenerationAttemptId, JobId, ModelProfileId};
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
}

impl std::fmt::Debug for dyn ReplyMediaStore + '_ {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ReplyMediaStore")
    }
}

impl<BR, AR> ReplyMediaStore for LocalMediaBlobStore<BR, AR>
where
    BR: MediaBlobRepository + Send + Sync,
    AR: MediaAssetRepository + Send + Sync,
{
    fn store_reply_image(
        &self,
        asset_id: AssetId,
        origin: ReplyImageOrigin,
        bytes: &[u8],
    ) -> Result<AssetId, MediaStoreError> {
        self.ingest_with_id(
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

/// Stores the candidate's images and appends a media part for each after
/// its other parts. An image whose bytes are not a supported image is left
/// out and logged.
pub(crate) fn attach_reply_media(
    store: Option<&dyn ReplyMediaStore>,
    attempt_id: GenerationAttemptId,
    origin: ReplyImageOrigin,
    candidate: &mut InferenceCandidate,
) -> Result<(), ReplyMediaError> {
    let media = std::mem::take(&mut candidate.media);
    if media.is_empty() {
        return Ok(());
    }
    let store = store.ok_or(ReplyMediaError::NoStore)?;
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
            Ok(asset_id) => candidate.parts.push(MessagePart::MediaAsset {
                asset_id,
                role: MediaAssetRole::Attachment,
            }),
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
            Err(error) => return Err(ReplyMediaError::Store(error)),
        }
    }
    Ok(())
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
    struct Recorded(Mutex<Vec<(AssetId, Vec<u8>)>>);

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
            self.0
                .lock()
                .expect("store lock")
                .push((asset_id, bytes.to_vec()));
            Ok(asset_id)
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
        attach_reply_media(Some(&store), attempt_id, origin, &mut reply).expect("stored");
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
        assert_eq!(
            reply_image_asset_id(attempt_id, 0),
            reply_image_asset_id(attempt_id, 0)
        );

        let mut unstored = candidate(vec![image("AQID")]);
        assert_eq!(
            attach_reply_media(None, attempt_id, origin, &mut unstored),
            Err(ReplyMediaError::NoStore)
        );
        let mut plain = candidate(Vec::new());
        attach_reply_media(None, attempt_id, origin, &mut plain).expect("nothing to store");
    }
}
