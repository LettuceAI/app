use lettuce_model_hub::{
    HUGGING_FACE_ENDPOINT, HfPinListing, KOKORO_REPOSITORY, KokoroInstallError, RemoteKokoroVoice,
};
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, RequestPolicy};

use crate::{
    KokoroAssetInventoryCoordinator, KokoroAssetInventoryError, KokoroInstalledVoiceSummary,
};

const MAX_REPOSITORY_SIBLINGS: usize = 1_024;
const MAX_REMOTE_VOICES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroAvailableVoice {
    pub id: String,
    pub installed: bool,
    pub remote_path: String,
    pub source_revision: String,
    pub byte_size: u64,
    pub sha256: String,
}

impl KokoroAvailableVoice {
    pub fn remote(&self) -> Result<RemoteKokoroVoice, KokoroCatalogError> {
        RemoteKokoroVoice::pinned(
            self.id.clone(),
            self.source_revision.clone(),
            self.byte_size,
            self.sha256.clone(),
        )
        .map_err(map_manifest_error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroCatalogError {
    #[error("Kokoro voice catalog transport failed: {0}")]
    Network(JsonClientError),
    #[error("Kokoro voice catalog returned an unsuccessful response")]
    Response,
    #[error("Kokoro voice catalog response is invalid")]
    InvalidData,
    #[error("Kokoro voice catalog exceeds its voice limit")]
    LimitExceeded,
    #[error("Kokoro installed voice inventory failed: {0}")]
    Inventory(KokoroAssetInventoryError),
}

#[derive(Debug, Clone)]
pub struct KokoroRemoteVoiceCatalog {
    client: JsonClient,
}

impl KokoroRemoteVoiceCatalog {
    #[must_use]
    pub const fn new(client: JsonClient) -> Self {
        Self { client }
    }

    pub async fn list(
        &self,
        inventory: &KokoroAssetInventoryCoordinator,
    ) -> Result<Vec<KokoroAvailableVoice>, KokoroCatalogError> {
        let installed = inventory
            .installed_voices()
            .map_err(KokoroCatalogError::Inventory)?;
        let request = lettuce_model_hub::model_pin_request(KOKORO_REPOSITORY);
        let response = lettuce_model_hub::send_hugging_face_request(
            &self.client,
            HUGGING_FACE_ENDPOINT,
            &request,
            JsonAuth::None,
            RequestPolicy::PROBE,
        )
        .await
        .map_err(KokoroCatalogError::Network)?;
        if response.status != 200 {
            return Err(KokoroCatalogError::Response);
        }
        let listing = lettuce_model_hub::parse_pin_listing(&response.body)
            .map_err(|_| KokoroCatalogError::InvalidData)?;
        parse_listing(listing, &installed)
    }
}

fn parse_listing(
    listing: HfPinListing,
    installed: &[KokoroInstalledVoiceSummary],
) -> Result<Vec<KokoroAvailableVoice>, KokoroCatalogError> {
    if listing.revision.len() != 40
        || !listing
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || listing.revision != listing.revision.to_ascii_lowercase()
    {
        return Err(KokoroCatalogError::InvalidData);
    }
    let siblings = listing
        .siblings
        .ok_or(KokoroCatalogError::InvalidData)?
        .into_iter()
        .map(|sibling| sibling.size.map(|size| (sibling, size)))
        .collect::<Option<Vec<_>>>()
        .ok_or(KokoroCatalogError::InvalidData)?;
    if siblings.len() > MAX_REPOSITORY_SIBLINGS {
        return Err(KokoroCatalogError::LimitExceeded);
    }
    let installed_ids = installed
        .iter()
        .map(|voice| voice.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut voices = Vec::new();
    for (sibling, size) in siblings {
        let Some(filename) = sibling.path.strip_prefix("voices/") else {
            continue;
        };
        let Some(id) = filename.strip_suffix(".bin") else {
            continue;
        };
        let lfs = sibling.lfs.ok_or(KokoroCatalogError::InvalidData)?;
        if lfs.size != size {
            return Err(KokoroCatalogError::InvalidData);
        }
        let remote = RemoteKokoroVoice::pinned(id, listing.revision.as_str(), size, lfs.sha256)
            .map_err(map_manifest_error)?;
        voices.push(KokoroAvailableVoice {
            installed: installed_ids.contains(remote.id.as_str()),
            id: remote.id,
            remote_path: remote.remote_path,
            source_revision: remote.source_revision,
            byte_size: remote.byte_size,
            sha256: remote.sha256,
        });
        if voices.len() > MAX_REMOTE_VOICES {
            return Err(KokoroCatalogError::LimitExceeded);
        }
    }
    voices.sort_by(|left, right| left.id.cmp(&right.id));
    for pair in voices.windows(2) {
        if pair[0].id == pair[1].id && pair[0] != pair[1] {
            return Err(KokoroCatalogError::InvalidData);
        }
    }
    voices.dedup_by(|left, right| left.id == right.id);
    Ok(voices)
}

fn map_manifest_error(_: KokoroInstallError) -> KokoroCatalogError {
    KokoroCatalogError::InvalidData
}

#[cfg(test)]
mod tests {
    use lettuce_model_hub::KOKORO_SOURCE_REVISION;
    use lettuce_types::ContentHash;

    use super::*;

    fn listing(detail: serde_json::Value) -> HfPinListing {
        lettuce_model_hub::parse_pin_listing(detail.to_string().as_bytes()).expect("model detail")
    }

    fn installed(id: &str) -> KokoroInstalledVoiceSummary {
        KokoroInstalledVoiceSummary {
            id: id.to_owned(),
            artifact: crate::KokoroArtifactSummary {
                byte_size: 10,
                blake3: ContentHash::parse("ab".repeat(32)).expect("content hash"),
            },
        }
    }

    #[test]
    fn catalog_keeps_safe_voice_ids_and_merges_installed_state() {
        let detail = listing(serde_json::json!({
            "sha": KOKORO_SOURCE_REVISION,
            "siblings": [
                { "rfilename": "README.md", "size": 10, "lfs": null },
                {
                    "rfilename": "voices/bm_george.bin",
                    "size": 522240,
                    "lfs": { "size": 522240, "sha256": "ab".repeat(32) }
                },
                {
                    "rfilename": "voices/af_heart.bin",
                    "size": 522240,
                    "lfs": { "size": 522240, "sha256": "cd".repeat(32) }
                },
                {
                    "rfilename": "voices/af_heart.bin",
                    "size": 522240,
                    "lfs": { "size": 522240, "sha256": "cd".repeat(32) }
                },
                { "rfilename": "voices/not-a-voice.txt", "size": 1, "lfs": null }
            ]
        }));
        let voices = parse_listing(detail, &[installed("af_heart")]).expect("voice catalog");
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[0].id, "af_heart");
        assert!(voices[0].installed);
        assert_eq!(voices[0].remote_path, "voices/af_heart.bin");
        assert_eq!(voices[0].source_revision, KOKORO_SOURCE_REVISION);
        assert_eq!(voices[0].sha256, "cd".repeat(32));
        assert_eq!(voices[1].id, "bm_george");
        assert!(!voices[1].installed);
        assert!(voices[0].remote().is_ok());
    }

    #[test]
    fn catalog_accepts_a_new_immutable_revision_and_keeps_its_integrity_facts() {
        let detail = listing(
            serde_json::json!({ "sha": "ef".repeat(20), "siblings": [{ "rfilename": "voices/af_heart.bin", "size": 42, "lfs": { "size": 42, "sha256": "cd".repeat(32) } }] }),
        );
        let catalog = parse_listing(detail, &[]).expect("new upstream revision");
        assert_eq!(catalog[0].source_revision, "ef".repeat(20));
        assert_eq!(
            catalog[0].remote().expect("voice pin").source_revision,
            "ef".repeat(20)
        );
    }

    #[test]
    fn catalog_rejects_mutable_unsafe_and_incoherent_voice_entries() {
        for detail in [
            serde_json::json!({ "sha": "main", "siblings": [] }),
            serde_json::json!({
                "sha": KOKORO_SOURCE_REVISION,
                "siblings": [{
                    "rfilename": "voices/nested/af_heart.bin",
                    "size": 42,
                    "lfs": { "size": 42, "sha256": "ab".repeat(32) }
                }]
            }),
            serde_json::json!({
                "sha": KOKORO_SOURCE_REVISION,
                "siblings": [{
                    "rfilename": "voices/af_heart.bin",
                    "size": 42,
                    "lfs": { "size": 41, "sha256": "ab".repeat(32) }
                }]
            }),
        ] {
            assert_eq!(
                parse_listing(listing(detail), &[]),
                Err(KokoroCatalogError::InvalidData)
            );
        }
    }
}
