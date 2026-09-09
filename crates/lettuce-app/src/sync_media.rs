use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_media::{
    LocalSyncMediaStore, MediaAssetRepository, MediaBlobRepository, MediaStoreError,
};
use lettuce_sync::{
    MAX_SYNC_BLOB_CHUNK_BYTES, MediaSyncError, PersonaMediaSyncRepository, SyncBlobChunk,
    SyncMediaCatalog,
};
use lettuce_types::{ContentHash, TimestampMillis};

#[async_trait]
pub trait AuthenticatedMediaSyncTransport: Send {
    async fn exchange_media_catalog(
        &mut self,
        local: SyncMediaCatalog,
        cancellation: &CancellationToken,
    ) -> Result<SyncMediaCatalog, MediaSyncTransportError>;

    async fn fetch_blob_chunk(
        &mut self,
        content_hash: &ContentHash,
        offset: u64,
        max_bytes: usize,
        cancellation: &CancellationToken,
    ) -> Result<SyncBlobChunk, MediaSyncTransportError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaSyncTransportError {
    #[error("media sync transport was cancelled")]
    Cancelled,
    #[error("media sync peer disconnected")]
    Disconnected,
    #[error("media sync transport protocol failed")]
    Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaSyncReport {
    pub advertised_assets: usize,
    pub received_assets: usize,
    pub received_blobs: usize,
    pub received_bytes: u64,
}

#[derive(Debug)]
pub struct SyncMediaCoordinator<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    media: &'a LocalSyncMediaStore<BR, AR>,
}

impl<'a, R: ?Sized, BR, AR> SyncMediaCoordinator<'a, R, BR, AR> {
    #[must_use]
    pub const fn new(repository: &'a R, media: &'a LocalSyncMediaStore<BR, AR>) -> Self {
        Self { repository, media }
    }
}

impl<R, BR, AR> SyncMediaCoordinator<'_, R, BR, AR>
where
    R: PersonaMediaSyncRepository + ?Sized,
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    pub async fn run<T>(
        &self,
        transport: &mut T,
        cancellation: &CancellationToken,
        now: TimestampMillis,
    ) -> Result<MediaSyncReport, SyncMediaExchangeError>
    where
        T: AuthenticatedMediaSyncTransport + ?Sized,
    {
        check_cancelled(cancellation)?;
        let local_ids = self
            .repository
            .referenced_persona_media()
            .map_err(SyncMediaExchangeError::Catalog)?;
        let local_assets = local_ids
            .into_iter()
            .map(|id| self.media.snapshot(id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(SyncMediaExchangeError::Media)?;
        let local_catalog =
            SyncMediaCatalog::new(local_assets).map_err(SyncMediaExchangeError::Catalog)?;
        let advertised_assets = local_catalog.assets().len();
        let remote = transport
            .exchange_media_catalog(local_catalog, cancellation)
            .await
            .map_err(SyncMediaExchangeError::Transport)?;
        let pending = self
            .repository
            .pending_persona_media()
            .map_err(SyncMediaExchangeError::Catalog)?;
        let mut report = MediaSyncReport {
            advertised_assets,
            received_assets: 0,
            received_blobs: 0,
            received_bytes: 0,
        };
        let mut completed_hashes = Vec::<ContentHash>::new();
        for expected in pending {
            let asset = remote
                .assets()
                .iter()
                .find(|asset| asset.asset.id == expected.asset.id)
                .filter(|asset| **asset == expected)
                .ok_or(SyncMediaExchangeError::Catalog(
                    MediaSyncError::InvalidAsset,
                ))?;
            check_cancelled(cancellation)?;
            let mut offset = self
                .media
                .receive_offset(asset)
                .map_err(SyncMediaExchangeError::Media)?;
            let transferred = offset < asset.blob.byte_size;
            while offset < asset.blob.byte_size {
                check_cancelled(cancellation)?;
                let chunk = transport
                    .fetch_blob_chunk(
                        &asset.blob.content_hash,
                        offset,
                        MAX_SYNC_BLOB_CHUNK_BYTES,
                        cancellation,
                    )
                    .await
                    .map_err(SyncMediaExchangeError::Transport)?;
                chunk
                    .validate(&asset.blob.content_hash, offset)
                    .map_err(SyncMediaExchangeError::Catalog)?;
                let next = offset
                    .checked_add(u64::try_from(chunk.bytes.len()).map_err(|_| {
                        SyncMediaExchangeError::Catalog(MediaSyncError::InvalidChunk)
                    })?)
                    .ok_or(SyncMediaExchangeError::Catalog(
                        MediaSyncError::InvalidChunk,
                    ))?;
                if next > asset.blob.byte_size || chunk.complete != (next == asset.blob.byte_size) {
                    return Err(SyncMediaExchangeError::Catalog(
                        MediaSyncError::InvalidChunk,
                    ));
                }
                offset = self
                    .media
                    .append_sync_chunk(asset, offset, &chunk.bytes)
                    .map_err(SyncMediaExchangeError::Media)?;
                report.received_bytes = report
                    .received_bytes
                    .saturating_add(u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX));
            }
            check_cancelled(cancellation)?;
            self.media
                .finish_sync_asset(asset, now)
                .map_err(SyncMediaExchangeError::Media)?;
            report.received_assets = report.received_assets.saturating_add(1);
            if transferred && !completed_hashes.contains(&asset.blob.content_hash) {
                completed_hashes.push(asset.blob.content_hash.clone());
                report.received_blobs = report.received_blobs.saturating_add(1);
            }
        }
        Ok(report)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncMediaExchangeError {
    #[error("media sync exchange was cancelled")]
    Cancelled,
    #[error("media sync catalog failed: {0}")]
    Catalog(MediaSyncError),
    #[error("media sync storage failed: {0}")]
    Media(MediaStoreError),
    #[error("media sync transport failed: {0}")]
    Transport(MediaSyncTransportError),
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), SyncMediaExchangeError> {
    if cancellation.is_cancelled() {
        Err(SyncMediaExchangeError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_characters::{
        Persona, PersonaMedia, PersonaMediaLink, PersonaMediaSlot, PersonaRepository,
    };
    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
        RetentionClass,
    };
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_sync::{
        CanonicalMediaAsset, CausalFrontier, IncomingBatchState, IncomingChangeRepository,
        LocalChangeJournal, MAX_INCOMING_CHANGES, MAX_INCOMING_PAYLOAD_BYTES, canonical_batch_hash,
    };
    use lettuce_types::{OperationId, Revision};

    struct SourceTransport<'a> {
        catalog: Vec<CanonicalMediaAsset>,
        store: &'a LocalSyncMediaStore<Database, Database>,
        disconnect_after_first: bool,
        fetched: usize,
        first_offset: Option<u64>,
    }

    #[async_trait]
    impl AuthenticatedMediaSyncTransport for SourceTransport<'_> {
        async fn exchange_media_catalog(
            &mut self,
            _: SyncMediaCatalog,
            _: &CancellationToken,
        ) -> Result<SyncMediaCatalog, MediaSyncTransportError> {
            SyncMediaCatalog::new(self.catalog.clone())
                .map_err(|_| MediaSyncTransportError::Protocol)
        }

        async fn fetch_blob_chunk(
            &mut self,
            content_hash: &ContentHash,
            offset: u64,
            max_bytes: usize,
            _: &CancellationToken,
        ) -> Result<SyncBlobChunk, MediaSyncTransportError> {
            self.first_offset.get_or_insert(offset);
            if self.disconnect_after_first && self.fetched == 1 {
                return Err(MediaSyncTransportError::Disconnected);
            }
            self.fetched += 1;
            let requested = if self.disconnect_after_first {
                max_bytes.min(16)
            } else {
                max_bytes
            };
            let bytes = self
                .store
                .read_sync_chunk(content_hash, offset, requested)
                .map_err(|_| MediaSyncTransportError::Protocol)?;
            let complete = offset + u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                == self.catalog[0].blob.byte_size;
            Ok(SyncBlobChunk {
                content_hash: content_hash.clone(),
                offset,
                bytes,
                complete,
            })
        }
    }

    fn png_fixture() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"shared persona image bytes");
        bytes
    }

    fn media_paths(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("sync-media-{label}-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("media root");
        let database = root.join("state.sqlite3");
        (root, database)
    }

    fn ingest_store(
        path: &std::path::Path,
        database: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(path).expect("snapshot"))
            .expect("authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(database).expect("blob database"),
            Database::open(database).expect("asset database"),
        )
    }

    fn sync_store(
        path: &std::path::Path,
        database: &std::path::Path,
    ) -> LocalSyncMediaStore<Database, Database> {
        LocalSyncMediaStore::open(
            path.join("platform-v2/media-blobs"),
            Database::open(database).expect("blob database"),
            Database::open(database).expect("asset database"),
        )
        .expect("sync media store")
    }

    #[tokio::test]
    async fn persona_media_resumes_and_materializes_before_the_staged_change_batch() {
        let (source_root, source_path) = media_paths("source");
        let (target_root, target_path) = media_paths("target");
        let source = Database::open(&source_path).expect("source database");
        let target = Database::open(&target_path).expect("target database");
        let ingest = ingest_store(&source_root, &source_path);
        let bytes = png_fixture();
        let avatar = ingest
            .ingest(
                bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::AvatarOriginal,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("avatar");
        let design = ingest
            .ingest(
                bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::Illustration,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("design reference");
        assert_eq!(avatar.blob.id, design.blob.id);
        let persona = PersonaRepository::create(
            &source,
            Persona {
                id: lettuce_types::PersonaId::new(),
                status: lettuce_characters::LifecycleStatus::Active,
                title: "Synced persona".into(),
                description: "Carries two shared image references".into(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
                media: PersonaMedia {
                    links: vec![
                        PersonaMediaLink {
                            asset_id: avatar.asset.id,
                            slot: PersonaMediaSlot::Avatar,
                            ordinal: 0,
                        },
                        PersonaMediaLink {
                            asset_id: design.asset.id,
                            slot: PersonaMediaSlot::DesignReference,
                            ordinal: 0,
                        },
                    ],
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(10),
                updated_at: TimestampMillis::new(10),
            },
        )
        .expect("persona");
        let outgoing = source
            .outbound_changes(
                &CausalFrontier::new(),
                MAX_INCOMING_CHANGES,
                MAX_INCOMING_PAYLOAD_BYTES,
            )
            .expect("outgoing changes");
        assert_eq!(outgoing.changes.len(), 3);
        let batch_id = OperationId::new();
        target
            .stage_incoming_batch(
                source
                    .local_device_id(TimestampMillis::new(11))
                    .expect("source identity"),
                batch_id,
                &canonical_batch_hash(&outgoing.changes),
                &outgoing.changes,
                TimestampMillis::new(11),
            )
            .expect("stage batch");
        assert_eq!(
            target
                .apply_incoming_batch(batch_id, TimestampMillis::new(11))
                .expect("pending batch")
                .state,
            IncomingBatchState::Pending
        );
        assert!(
            PersonaRepository::get(&target, persona.id)
                .expect("target persona read")
                .is_none()
        );

        let source_sync = sync_store(&source_root, &source_path);
        let target_sync = sync_store(&target_root, &target_path);
        let catalog = vec![
            source_sync
                .snapshot(avatar.asset.id)
                .expect("avatar snapshot"),
            source_sync
                .snapshot(design.asset.id)
                .expect("design snapshot"),
        ];
        let mut interrupted = SourceTransport {
            catalog: catalog.clone(),
            store: &source_sync,
            disconnect_after_first: true,
            fetched: 0,
            first_offset: None,
        };
        let coordinator = SyncMediaCoordinator::new(&target, &target_sync);
        assert!(matches!(
            coordinator
                .run(
                    &mut interrupted,
                    &CancellationToken::new(),
                    TimestampMillis::new(12)
                )
                .await,
            Err(SyncMediaExchangeError::Transport(
                MediaSyncTransportError::Disconnected
            ))
        ));

        drop(target_sync);
        let reopened_sync = sync_store(&target_root, &target_path);
        let mut resumed = SourceTransport {
            catalog,
            store: &source_sync,
            disconnect_after_first: false,
            fetched: 0,
            first_offset: None,
        };
        let report = SyncMediaCoordinator::new(&target, &reopened_sync)
            .run(
                &mut resumed,
                &CancellationToken::new(),
                TimestampMillis::new(13),
            )
            .await
            .expect("resumed media exchange");
        assert_eq!(resumed.first_offset, Some(16));
        assert_eq!(report.received_assets, 2);
        assert_eq!(report.received_blobs, 1);
        assert_eq!(
            target
                .apply_incoming_batch(batch_id, TimestampMillis::new(14))
                .expect("apply resumed batch")
                .state,
            IncomingBatchState::Committed
        );
        assert_eq!(
            PersonaRepository::get(&target, persona.id).expect("target persona"),
            Some(persona)
        );
        let target_avatar = MediaAssetRepository::get(&target, avatar.asset.id)
            .expect("target avatar")
            .expect("avatar exists");
        let target_design = MediaAssetRepository::get(&target, design.asset.id)
            .expect("target design")
            .expect("design exists");
        assert_eq!(target_avatar.blob_id, target_design.blob_id);
        assert_eq!(
            reopened_sync
                .read_sync_chunk(&avatar.blob.content_hash, 0, bytes.len())
                .expect("target bytes"),
            bytes
        );
    }
}
