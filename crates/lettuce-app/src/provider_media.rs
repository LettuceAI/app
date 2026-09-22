//! Provider requests read attachments from the local media store.

use std::io::Read;

use lettuce_media::{LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository};
use lettuce_providers::{ProviderMedia, ProviderMediaError, ProviderMediaSource};
use lettuce_types::AssetId;

/// Ready media assets as provider attachments.
#[derive(Debug)]
pub struct StoredProviderMedia<BR, AR>(pub LocalMediaBlobStore<BR, AR>);

impl<BR, AR> ProviderMediaSource for StoredProviderMedia<BR, AR>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    fn load(&self, asset_id: AssetId) -> Result<ProviderMedia, ProviderMediaError> {
        let mut opened = self
            .0
            .open_ready(asset_id)
            .map_err(|_| ProviderMediaError::Unavailable)?;
        let mut bytes = Vec::new();
        opened
            .reader
            .read_to_end(&mut bytes)
            .map_err(|_| ProviderMediaError::Unavailable)?;
        Ok(ProviderMedia {
            mime_type: opened.blob.mime_type,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
        RetentionClass,
    };
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_providers::ProviderMediaSource;
    use lettuce_types::{AssetId, OperationId};

    use super::StoredProviderMedia;

    fn png() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"attachment bytes");
        bytes
    }

    #[test]
    fn ready_attachments_load_with_their_mime_type() {
        let root = std::env::temp_dir().join(format!("provider-media-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let database = root.join("state.sqlite3");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("authority");
        let store = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write"),
            Database::open(&database).expect("blobs"),
            Database::open(&database).expect("assets"),
        );
        let bytes = png();
        let ingested = store
            .ingest(
                bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::MessageImage,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("ingest");
        let media = StoredProviderMedia(store);
        let loaded = media.load(ingested.asset.id).expect("load");
        assert_eq!(loaded.mime_type, "image/png");
        assert_eq!(loaded.bytes, bytes);
        assert!(media.load(AssetId::new()).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
