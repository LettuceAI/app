//! The synthesized speech cache: reusing a finished synthesis instead of
//! synthesizing the same text again, and dropping the stored audio nothing
//! else keeps.

use lettuce_media::{
    LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository, MediaStoreError,
};
use lettuce_speech::{
    SpeechCacheRepository, SynthesisRecord, SynthesisRepositoryError, SynthesisReuseKey,
};
use lettuce_types::TimestampMillis;

/// How much cached speech audio is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TtsAudioCacheStats {
    pub count: u64,
    pub size_bytes: u64,
}

/// The most recent finished synthesis of the same provider, model, voice,
/// text and prompt whose audio is still stored.
pub fn reusable_tts_synthesis<R: SpeechCacheRepository + ?Sized>(
    cache: &R,
    key: &SynthesisReuseKey,
    now: TimestampMillis,
) -> Result<Option<SynthesisRecord>, SynthesisRepositoryError> {
    cache.find_reusable(key, now)
}

pub fn tts_audio_cache_stats<R: SpeechCacheRepository + ?Sized>(
    cache: &R,
) -> Result<TtsAudioCacheStats, SynthesisRepositoryError> {
    Ok(cache
        .cached_blobs()?
        .into_iter()
        .fold(TtsAudioCacheStats::default(), |stats, blob| {
            TtsAudioCacheStats {
                count: stats.count + 1,
                size_bytes: stats.size_bytes + blob.byte_size,
            }
        }))
}

/// Deletes every cached speech audio file and returns how many went. A file
/// that fails is skipped, and the rest are still deleted.
pub fn clear_tts_audio_cache<R, BR, AR>(
    cache: &R,
    media: &LocalMediaBlobStore<BR, AR>,
    now: TimestampMillis,
) -> Result<u64, SynthesisRepositoryError>
where
    R: SpeechCacheRepository + ?Sized,
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    let mut cleared = 0;
    for blob in cache.cached_blobs()? {
        let released = media.release_blob(blob.blob_id, |_| {
            cache
                .release(blob.blob_id, now)
                .map_err(|_| MediaStoreError::CatalogFailure)
        });
        match released {
            Ok(Some(_)) => cleared += 1,
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, blob_id = %blob.blob_id, "cached speech audio was not deleted")
            }
        }
    }
    Ok(cleared)
}
