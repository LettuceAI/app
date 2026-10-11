use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaBlob, MediaKind,
    RetentionClass,
};
use lettuce_sync::{
    CanonicalMediaAsset, IncomingChangeRepository, LocalChangeJournal, MediaSyncRepository,
    canonical_batch_hash, media_asset_insert_change,
};
use lettuce_types::{AssetId, ContentHash, MediaBlobId, OperationId, Revision, TimestampMillis};

#[test]
fn deferred_media_pages_are_stable_and_reach_every_asset() {
    let source = crate::Database::open_in_memory().expect("source");
    let target = crate::Database::open_in_memory().expect("target");
    let blob_id = MediaBlobId::new();
    let mut expected = Vec::new();
    for _ in 0..300 {
        let asset_id = AssetId::new();
        expected.push(asset_id);
        let asset = CanonicalMediaAsset {
            asset: MediaAsset {
                id: asset_id,
                blob_id,
                kind: AssetKind::AvatarOriginal,
                origin: AssetOrigin::Upload,
                retention: RetentionClass::Persistent,
                provenance: AssetProvenanceV1::default(),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            blob: MediaBlob {
                id: blob_id,
                content_hash: ContentHash::parse("ab".repeat(32)).expect("hash"),
                kind: MediaKind::Image,
                mime_type: "image/png".into(),
                byte_size: 10,
                width: Some(1),
                height: Some(1),
                duration_ms: None,
                validation_version: 1,
                state: BlobState::Ready,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
        };
        source
            .record_local_change(
                OperationId::new(),
                media_asset_insert_change(&asset).expect("asset change"),
                TimestampMillis::new(1),
            )
            .expect("journal");
    }
    loop {
        let outbound = source
            .outbound_changes(
                &target.local_frontier().expect("frontier"),
                256,
                16 * 1024 * 1024,
            )
            .expect("outbound");
        if outbound.changes.is_empty() {
            break;
        }
        let batch = OperationId::new();
        target
            .stage_incoming_batch(
                outbound.changes[0].origin_device(),
                batch,
                &canonical_batch_hash(&outbound.changes),
                &outbound.changes,
                TimestampMillis::new(2),
            )
            .expect("stage");
        assert_eq!(
            target
                .apply_incoming_batch(batch, TimestampMillis::new(2))
                .expect("apply")
                .state,
            lettuce_sync::IncomingBatchState::Committed
        );
    }
    let first = target.pending_media(None).expect("first page");
    assert_eq!(first.len(), 256);
    assert_eq!(target.pending_media(None).expect("stable page"), first);
    let second = target
        .pending_media(first.last().map(|asset| asset.asset.id))
        .expect("second page");
    assert_eq!(second.len(), 44);
    assert!(
        target
            .pending_media(second.last().map(|asset| asset.asset.id))
            .expect("end")
            .is_empty()
    );
    let actual = first
        .into_iter()
        .chain(second)
        .map(|asset| asset.asset.id)
        .collect::<Vec<_>>();
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(actual, expected);
}
