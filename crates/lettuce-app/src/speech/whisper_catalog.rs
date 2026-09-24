use lettuce_model_hub::{
    HUGGING_FACE_ENDPOINT, HfPinListing, RemoteWhisperModel, WhisperModelError,
};
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonQueryParameter, RequestPolicy};

const WHISPER_CATALOG_REPOSITORY: &str = "ggerganov/whisper.cpp";
const MAX_REMOTE_MODELS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WhisperCatalogError {
    #[error("Whisper catalog transport failed: {0}")]
    Network(JsonClientError),
    #[error("Whisper catalog returned an unsuccessful response")]
    Response,
    #[error("Whisper catalog response is invalid")]
    InvalidData,
    #[error("Whisper catalog exceeds its model limit")]
    LimitExceeded,
}

#[derive(Debug, Clone)]
pub struct WhisperRemoteCatalog {
    client: JsonClient,
}

impl WhisperRemoteCatalog {
    #[must_use]
    pub const fn new(client: JsonClient) -> Self {
        Self { client }
    }

    pub async fn list(&self) -> Result<Vec<RemoteWhisperModel>, WhisperCatalogError> {
        let request = lettuce_model_hub::model_pin_request(WHISPER_CATALOG_REPOSITORY);
        let query = request
            .query
            .iter()
            .map(|(name, value)| JsonQueryParameter { name, value })
            .collect::<Vec<_>>();
        let detail = self
            .client
            .get_json_with_query(
                HUGGING_FACE_ENDPOINT,
                &request.path,
                &query,
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::PROBE,
            )
            .await
            .map_err(WhisperCatalogError::Network)?;
        if detail.status != 200 {
            return Err(WhisperCatalogError::Response);
        }
        let listing = lettuce_model_hub::parse_pin_listing(&detail.body)
            .map_err(|_| WhisperCatalogError::InvalidData)?;
        parse_listing(listing)
    }
}

fn parse_listing(listing: HfPinListing) -> Result<Vec<RemoteWhisperModel>, WhisperCatalogError> {
    let siblings = listing
        .siblings
        .ok_or(WhisperCatalogError::InvalidData)?
        .into_iter()
        .map(|sibling| sibling.size.map(|size| (sibling, size)))
        .collect::<Option<Vec<_>>>()
        .ok_or(WhisperCatalogError::InvalidData)?;
    validate_revision(&listing.revision)?;
    if siblings.len() > 1_024 {
        return Err(WhisperCatalogError::LimitExceeded);
    }
    let mut models = Vec::new();
    for (entry, size) in siblings {
        if !entry.path.starts_with("ggml-")
            || !entry.path.ends_with(".bin")
            || entry.path.contains('/')
            || entry.path.contains("encoder")
            || entry.path.contains(".mlmodelc")
        {
            continue;
        }
        let lfs = entry.lfs.ok_or(WhisperCatalogError::InvalidData)?;
        if lfs.size != size {
            return Err(WhisperCatalogError::InvalidData);
        }
        let model = RemoteWhisperModel::pinned(entry.path, &listing.revision, size, lfs.sha256)
            .map_err(map_model_error)?;
        models.push(model);
        if models.len() > MAX_REMOTE_MODELS {
            return Err(WhisperCatalogError::LimitExceeded);
        }
    }
    models.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(models)
}

fn validate_revision(revision: &str) -> Result<(), WhisperCatalogError> {
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(WhisperCatalogError::InvalidData);
    }
    Ok(())
}

fn map_model_error(_: WhisperModelError) -> WhisperCatalogError {
    WhisperCatalogError::InvalidData
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(detail: serde_json::Value) -> HfPinListing {
        lettuce_model_hub::parse_pin_listing(detail.to_string().as_bytes()).expect("model detail")
    }

    #[test]
    fn pinned_detail_preserves_sizes_hashes_capabilities_and_recommendations() {
        let revision = "ab".repeat(20);
        let detail = listing(serde_json::json!({
            "sha": revision,
            "siblings": [
                { "rfilename": "README.md", "size": 10, "lfs": null },
                {
                    "rfilename": "ggml-small.en-q5_1.bin",
                    "size": 42,
                    "lfs": { "size": 42, "sha256": "cd".repeat(32) }
                },
                {
                    "rfilename": "ggml-base.bin",
                    "size": 84,
                    "lfs": { "size": 84, "sha256": "ef".repeat(32) }
                }
            ]
        }));
        let models = parse_listing(detail).expect("pinned models");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].model_id, "base");
        assert!(models[0].recommended);
        assert_eq!(models[1].sha256, "cd".repeat(32));
        assert!(models[1].english_only);
        assert!(models[1].quantized);
        assert!(models[1].recommended_for_mobile);
    }

    #[test]
    fn catalog_rejects_mutable_revisions_and_incoherent_lfs_metadata() {
        let mutable = listing(serde_json::json!({
            "sha": "main",
            "siblings": []
        }));
        assert_eq!(
            parse_listing(mutable),
            Err(WhisperCatalogError::InvalidData)
        );
        let incoherent = listing(serde_json::json!({
            "sha": "ab".repeat(20),
            "siblings": [{
                "rfilename": "ggml-base.bin",
                "size": 42,
                "lfs": { "size": 41, "sha256": "cd".repeat(32) }
            }]
        }));
        assert_eq!(
            parse_listing(incoherent),
            Err(WhisperCatalogError::InvalidData)
        );
    }
}
