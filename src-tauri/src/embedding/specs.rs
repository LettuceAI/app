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
pub(crate) const MODEL_FILES_V4_REMOTE: [&str; 2] = ["onnx/model.int8.onnx", "tokenizer.json"];
pub(crate) const MODEL_FILES_V4_LOCAL: [&str; 2] = ["v4-model.int8.onnx", "v4-tokenizer.json"];

pub(crate) const COMPANION_EMOTION_MODEL_FILES_REMOTE: [&str; 3] = [
    "onnx/model_quantized.onnx",
    "onnx/tokenizer.json",
    "config.json",
];
pub(crate) const COMPANION_EMOTION_MODEL_FILES_LOCAL: [&str; 3] = [
    "companion-emotion/model.int8.onnx",
    "companion-emotion/tokenizer.json",
    "companion-emotion/config.json",
];
pub(crate) const COMPANION_NER_MODEL_FILES_REMOTE: [&str; 3] =
    ["onnx/model_quantized.onnx", "tokenizer.json", "config.json"];
pub(crate) const COMPANION_NER_MODEL_FILES_LOCAL: [&str; 3] = [
    "companion-ner/model.int8.onnx",
    "companion-ner/tokenizer.json",
    "companion-ner/config.json",
];
pub(crate) const COMPANION_ROUTER_MODEL_FILES_REMOTE: [&str; 3] =
    ["onnx/model_quantized.onnx", "tokenizer.json", "config.json"];
pub(crate) const COMPANION_ROUTER_MODEL_FILES_LOCAL: [&str; 3] = [
    "companion-router/model.int8.onnx",
    "companion-router/tokenizer.json",
    "companion-router/config.json",
];

pub(crate) const HUGGINGFACE_BASE_V1: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-512d-v1/resolve/main";
pub(crate) const HUGGINGFACE_BASE_V2: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-512d-v2/resolve/main";
pub(crate) const HUGGINGFACE_BASE_V3: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-512d-v3/resolve/main";
pub(crate) const HUGGINGFACE_BASE_V4: &str =
    "https://huggingface.co/Zeolit/lettuce-emb-768d-v4/resolve/main";
pub(crate) const HUGGINGFACE_BASE_COMPANION_EMOTION: &str =
    "https://huggingface.co/SamLowe/roberta-base-go_emotions-onnx/resolve/main";
pub(crate) const HUGGINGFACE_BASE_COMPANION_NER: &str =
    "https://huggingface.co/Xenova/distilbert-base-multilingual-cased-ner-hrl/resolve/main";
pub(crate) const HUGGINGFACE_BASE_COMPANION_ROUTER: &str =
    "https://huggingface.co/onnx-community/distilbert-base-uncased-mnli-ONNX/resolve/main";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompanionKind {
    Emotion,
    Ner,
    Router,
}

impl CompanionKind {
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "emotion" => Some(Self::Emotion),
            "ner" => Some(Self::Ner),
            "router" => Some(Self::Router),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Emotion => "companion-emotion",
            Self::Ner => "companion-ner",
            Self::Router => "companion-router",
        }
    }

    pub(crate) fn base_url(self) -> &'static str {
        match self {
            Self::Emotion => HUGGINGFACE_BASE_COMPANION_EMOTION,
            Self::Ner => HUGGINGFACE_BASE_COMPANION_NER,
            Self::Router => HUGGINGFACE_BASE_COMPANION_ROUTER,
        }
    }

    pub(crate) fn remote_files(self) -> &'static [&'static str] {
        match self {
            Self::Emotion => &COMPANION_EMOTION_MODEL_FILES_REMOTE,
            Self::Ner => &COMPANION_NER_MODEL_FILES_REMOTE,
            Self::Router => &COMPANION_ROUTER_MODEL_FILES_REMOTE,
        }
    }

    pub(crate) fn local_files(self) -> &'static [&'static str] {
        match self {
            Self::Emotion => &COMPANION_EMOTION_MODEL_FILES_LOCAL,
            Self::Ner => &COMPANION_NER_MODEL_FILES_LOCAL,
            Self::Router => &COMPANION_ROUTER_MODEL_FILES_LOCAL,
        }
    }
}

pub(crate) struct DownloadFileSpec {
    pub(crate) base_url: &'static str,
    pub(crate) remote_path: &'static str,
    pub(crate) local_path: &'static str,
    pub(crate) progress_name: &'static str,
}

pub(crate) struct DownloadSourceSpec {
    pub(crate) target_version: EmbeddingModelVersion,
    pub(crate) source_label: &'static str,
    pub(crate) remote_files: &'static [&'static str],
    pub(crate) local_files: &'static [&'static str],
    pub(crate) base_url: &'static str,
}

pub(crate) fn download_source_spec(requested: Option<&str>) -> DownloadSourceSpec {
    match requested {
        Some("v3") => DownloadSourceSpec {
            target_version: EmbeddingModelVersion::V3,
            source_label: "v3",
            remote_files: &MODEL_FILES_V3_REMOTE,
            local_files: &MODEL_FILES_V3_LOCAL,
            base_url: HUGGINGFACE_BASE_V3,
        },
        _ => DownloadSourceSpec {
            target_version: EmbeddingModelVersion::V4,
            source_label: "v4",
            remote_files: &MODEL_FILES_V4_REMOTE,
            local_files: &MODEL_FILES_V4_LOCAL,
            base_url: HUGGINGFACE_BASE_V4,
        },
    }
}

/// Build the file plan for the embedding model only (no companion files).
pub(crate) fn embedding_download_plan(requested: Option<&str>) -> Vec<DownloadFileSpec> {
    let source = download_source_spec(requested);
    let mut plan = source
        .remote_files
        .iter()
        .zip(source.local_files.iter())
        .map(|(remote_path, local_path)| DownloadFileSpec {
            base_url: source.base_url,
            remote_path,
            local_path,
            progress_name: local_path,
        })
        .collect::<Vec<_>>();

    if matches!(
        source.target_version,
        EmbeddingModelVersion::V3 | EmbeddingModelVersion::V4
    ) {
        if let Some(item) = plan.get_mut(0) {
            item.progress_name = "model.int8.onnx";
        }
        if let Some(item) = plan.get_mut(1) {
            item.progress_name = "tokenizer.json";
        }
    }

    plan
}

/// Build the file plan for a single companion model.
pub(crate) fn companion_download_plan(kind: CompanionKind) -> Vec<DownloadFileSpec> {
    kind.remote_files()
        .iter()
        .zip(kind.local_files().iter())
        .map(|(remote_path, local_path)| DownloadFileSpec {
            base_url: kind.base_url(),
            remote_path,
            local_path,
            progress_name: local_path,
        })
        .collect()
}

/// Files owned by a given embedding source — used for cleanup of a failed embedding download.
pub(crate) fn embedding_owned_files(version: &EmbeddingModelVersion) -> Vec<&'static str> {
    match version {
        EmbeddingModelVersion::V1 => MODEL_FILES_V1.to_vec(),
        EmbeddingModelVersion::V2 => {
            let mut v = MODEL_FILES_V2_LOCAL.to_vec();
            v.extend(MODEL_FILES_V2_LOCAL_LEGACY.iter().copied());
            v
        }
        EmbeddingModelVersion::V3 => MODEL_FILES_V3_LOCAL.to_vec(),
        EmbeddingModelVersion::V4 => MODEL_FILES_V4_LOCAL.to_vec(),
    }
}
