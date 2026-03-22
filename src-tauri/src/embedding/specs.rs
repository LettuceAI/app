use super::EmbeddingModelVersion;

pub(crate) const MODEL_FILES_V1: [&str; 3] = [
    "lettuce-emb-512d-kd-v1.onnx",
    "lettuce-emb-512d-kd-v1.onnx.data",
    "tokenizer.json",
];

pub(crate) const MODEL_FILES_V2_REMOTE: [&str; 3] =
    ["model.onnx", "model.onnx.data", "tokenizer.json"];
pub(crate) const MODEL_FILES_V2_LOCAL: [&str; 3] =
    ["v2-model.onnx", "v2-model.onnx.data", "v2-tokenizer.json"];
pub(crate) const MODEL_FILES_V2_LOCAL_LEGACY: [&str; 3] =
    ["v2-model.onnx", "v2-model.onnx.data", "tokenizer.json"];

pub(crate) const MODEL_FILES_V3_REMOTE: [&str; 2] = ["model.int8.onnx", "tokenizer.json"];
pub(crate) const MODEL_FILES_V3_LOCAL: [&str; 2] = ["v3-model.int8.onnx", "v3-tokenizer.json"];

pub(crate) const HUGGINGFACE_BASE_V1: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-512d-v1/resolve/main";
pub(crate) const HUGGINGFACE_BASE_V2: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-512d-v2/resolve/main";
pub(crate) const HUGGINGFACE_BASE_V3: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-512d-v3/resolve/main";

pub(crate) struct DownloadSourceSpec {
    pub(crate) target_version: EmbeddingModelVersion,
    pub(crate) source_label: &'static str,
    pub(crate) remote_files: &'static [&'static str],
    pub(crate) local_files: &'static [&'static str],
    pub(crate) base_url: &'static str,
}

pub(crate) fn download_source_spec(requested: Option<&str>) -> DownloadSourceSpec {
    match requested {
        Some("v1") => DownloadSourceSpec {
            target_version: EmbeddingModelVersion::V1,
            source_label: "v1",
            remote_files: &MODEL_FILES_V1,
            local_files: &MODEL_FILES_V1,
            base_url: HUGGINGFACE_BASE_V1,
        },
        Some("v2") => DownloadSourceSpec {
            target_version: EmbeddingModelVersion::V2,
            source_label: "v2",
            remote_files: &MODEL_FILES_V2_REMOTE,
            local_files: &MODEL_FILES_V2_LOCAL,
            base_url: HUGGINGFACE_BASE_V2,
        },
        _ => DownloadSourceSpec {
            target_version: EmbeddingModelVersion::V3,
            source_label: "v3",
            remote_files: &MODEL_FILES_V3_REMOTE,
            local_files: &MODEL_FILES_V3_LOCAL,
            base_url: HUGGINGFACE_BASE_V3,
        },
    }
}
