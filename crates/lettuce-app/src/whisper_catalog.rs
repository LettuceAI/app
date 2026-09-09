use lettuce_model_hub::{RemoteWhisperModel, WhisperModelError};
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonQueryParameter, RequestPolicy};
use serde::Deserialize;

const HUGGING_FACE_ENDPOINT: &str = "https://huggingface.co";
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
        let detail = self
            .client
            .get_json_with_query(
                HUGGING_FACE_ENDPOINT,
                "/api/models/ggerganov/whisper.cpp",
                &[JsonQueryParameter {
                    name: "blobs",
                    value: "true",
                }],
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
        let detail: ModelDetail =
            serde_json::from_slice(&detail.body).map_err(|_| WhisperCatalogError::InvalidData)?;
        parse_detail(detail)
    }
}

#[derive(Debug, Deserialize)]
struct ModelDetail {
    sha: String,
    siblings: Vec<ModelSibling>,
}

#[derive(Debug, Deserialize)]
struct ModelSibling {
    rfilename: String,
    size: u64,
    lfs: Option<ModelLfs>,
}

#[derive(Debug, Deserialize)]
struct ModelLfs {
    size: u64,
    sha256: String,
}

fn parse_detail(detail: ModelDetail) -> Result<Vec<RemoteWhisperModel>, WhisperCatalogError> {
    validate_revision(&detail.sha)?;
    if detail.siblings.len() > 1_024 {
        return Err(WhisperCatalogError::LimitExceeded);
    }
    let mut models = Vec::new();
    for entry in detail.siblings {
        if !entry.rfilename.starts_with("ggml-")
            || !entry.rfilename.ends_with(".bin")
            || entry.rfilename.contains('/')
            || entry.rfilename.contains("encoder")
            || entry.rfilename.contains(".mlmodelc")
        {
            continue;
        }
        let lfs = entry.lfs.ok_or(WhisperCatalogError::InvalidData)?;
        if lfs.size != entry.size {
            return Err(WhisperCatalogError::InvalidData);
        }
        let model =
            RemoteWhisperModel::pinned(entry.rfilename, &detail.sha, entry.size, lfs.sha256)
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

    #[test]
    fn pinned_detail_preserves_sizes_hashes_capabilities_and_recommendations() {
        let revision = "ab".repeat(20);
        let detail: ModelDetail = serde_json::from_value(serde_json::json!({
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
        }))
        .expect("model detail");
        let models = parse_detail(detail).expect("pinned models");
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
        let mutable: ModelDetail = serde_json::from_value(serde_json::json!({
            "sha": "main",
            "siblings": []
        }))
        .expect("model detail");
        assert_eq!(parse_detail(mutable), Err(WhisperCatalogError::InvalidData));
        let incoherent: ModelDetail = serde_json::from_value(serde_json::json!({
            "sha": "ab".repeat(20),
            "siblings": [{
                "rfilename": "ggml-base.bin",
                "size": 42,
                "lfs": { "size": 41, "sha256": "cd".repeat(32) }
            }]
        }))
        .expect("model detail");
        assert_eq!(
            parse_detail(incoherent),
            Err(WhisperCatalogError::InvalidData)
        );
    }
}
