//! How well a model runs here: scores and recommendations for Hugging Face
//! files from their GGUF header, and the score of a downloaded model file.

use std::io::Read;
use std::path::Path;

use async_trait::async_trait;
use lettuce_model_hub::{
    GGUF_HEADER_PROBE_BYTES, GGUF_HEADER_RETRY_BYTES, GgufModelMeta, HfBrowseError,
    LocalRunnability, RecommendationData, RunnabilityDefaults, RunnabilityFile,
    RunnabilityHardware, RunnabilityScore,
};
use lettuce_network::ArtifactDownloadClient;
use lettuce_settings::{SecretStore, SecretValue};

use crate::HuggingFaceBrowser;

/// The leading bytes of a file in a Hugging Face repository.
#[async_trait]
pub trait GgufHeaderSource: Send + Sync {
    async fn read_prefix(
        &self,
        model_id: &str,
        filename: &str,
        length: u64,
        token: Option<&SecretValue>,
    ) -> Option<Vec<u8>>;
}

#[async_trait]
impl GgufHeaderSource for ArtifactDownloadClient {
    async fn read_prefix(
        &self,
        model_id: &str,
        filename: &str,
        length: u64,
        token: Option<&SecretValue>,
    ) -> Option<Vec<u8>> {
        self.read_hugging_face_prefix(model_id, filename, length, token)
            .await
            .ok()
    }
}

/// This machine's memory for llama.cpp; nothing on mobile, where models do
/// not run locally.
#[must_use]
pub fn local_runnability_hardware() -> RunnabilityHardware {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let supports_gpu_offload = lettuce_local_llm::llama::shared_backend()
            .is_ok_and(|backend| backend.supports_gpu_offload());
        RunnabilityHardware {
            available_ram: lettuce_local_llm::hardware::get_available_memory_bytes(),
            available_vram: if supports_gpu_offload {
                lettuce_local_llm::hardware::get_available_vram_bytes()
            } else {
                None
            },
            supports_gpu_offload,
            unified_memory: lettuce_local_llm::hardware::is_unified_memory(),
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        RunnabilityHardware {
            available_ram: None,
            available_vram: None,
            supports_gpu_offload: false,
            unified_memory: false,
        }
    }
}

impl HuggingFaceBrowser {
    async fn remote_gguf_meta<S, H>(
        secrets: &S,
        source: &H,
        model_id: &str,
        files: &[RunnabilityFile],
    ) -> Result<Option<GgufModelMeta>, HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        let Some(representative) = files
            .iter()
            .filter(|file| file.size > 0)
            .min_by_key(|file| file.size)
        else {
            return Ok(None);
        };
        let Ok(token) = Self::saved_token(secrets).await else {
            return Ok(None);
        };
        let read =
            |length| source.read_prefix(model_id, &representative.filename, length, token.as_ref());
        let Some(probe) = read(GGUF_HEADER_PROBE_BYTES).await else {
            return Ok(None);
        };
        let first = lettuce_model_hub::parse_gguf_meta(&probe);
        if first.as_ref().is_none_or(GgufModelMeta::has_essentials) {
            return Ok(first);
        }
        Ok(read(GGUF_HEADER_RETRY_BYTES)
            .await
            .and_then(|data| lettuce_model_hub::parse_gguf_meta(&data))
            .or(first))
    }

    /// A score per file, estimated from the smallest file's GGUF header.
    pub async fn runnability<S, H>(
        &self,
        secrets: &S,
        source: &H,
        model_id: &str,
        files: &[RunnabilityFile],
        hardware: RunnabilityHardware,
        defaults: RunnabilityDefaults,
    ) -> Result<Vec<RunnabilityScore>, HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let meta = Self::remote_gguf_meta(secrets, source, model_id, files).await?;
        Ok(lettuce_model_hub::runnability_scores(
            files,
            model_id,
            meta.as_ref(),
            hardware,
            defaults,
        ))
    }

    /// Context limits per file and the recommended file, context and KV type.
    pub async fn recommendation<S, H>(
        &self,
        secrets: &S,
        source: &H,
        model_id: &str,
        files: &[RunnabilityFile],
        hardware: RunnabilityHardware,
        defaults: RunnabilityDefaults,
    ) -> Result<RecommendationData, HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        if files.is_empty() {
            return Ok(RecommendationData::empty());
        }
        let meta = Self::remote_gguf_meta(secrets, source, model_id, files).await?;
        Ok(lettuce_model_hub::build_recommendation(
            files,
            model_id,
            meta.as_ref(),
            hardware,
            defaults.context_length,
        ))
    }
}

fn read_prefix(path: &Path, length: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(length)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

pub(crate) fn local_gguf_meta(path: &Path) -> Option<GgufModelMeta> {
    let probe = read_prefix(path, GGUF_HEADER_PROBE_BYTES)?;
    lettuce_model_hub::gguf_meta_with_retry(&probe, || read_prefix(path, GGUF_HEADER_RETRY_BYTES))
}

/// Files loaded next to a model: its projector, and its MTP draft model
/// unless that runs on the CPU.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalModelSidecars<'a> {
    pub mmproj_path: Option<&'a str>,
    pub mtp_enabled: bool,
    pub mtp_on_cpu: bool,
    pub mtp_model_path: Option<&'a str>,
}

fn file_size(path: Option<&str>) -> u64 {
    path.filter(|path| !path.trim().is_empty())
        .and_then(|path| std::fs::metadata(path).ok())
        .map_or(0, |metadata| metadata.len())
}

/// The score of a downloaded GGUF file on `hardware`.
pub fn local_file_runnability(
    file_path: &str,
    sidecars: &LocalModelSidecars<'_>,
    hardware: RunnabilityHardware,
    defaults: RunnabilityDefaults,
) -> Result<LocalRunnability, String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err("File does not exist".to_owned());
    }
    let size = std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("Failed to read file metadata: {error}"))?;
    let mtp = if sidecars.mtp_enabled && !sidecars.mtp_on_cpu {
        file_size(sidecars.mtp_model_path)
    } else {
        0
    };
    let sidecar_bytes = file_size(sidecars.mmproj_path).saturating_add(mtp);
    Ok(lettuce_model_hub::local_runnability(
        file_path,
        size,
        local_gguf_meta(path).as_ref(),
        hardware,
        sidecar_bytes,
        defaults,
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lettuce_settings::InMemorySecretStore;

    use super::*;

    struct Header {
        bytes: Vec<u8>,
        reads: Mutex<Vec<(String, u64)>>,
    }

    #[async_trait]
    impl GgufHeaderSource for Header {
        async fn read_prefix(
            &self,
            model_id: &str,
            filename: &str,
            length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            self.reads
                .lock()
                .expect("reads")
                .push((format!("{model_id}/{filename}"), length));
            let end = usize::try_from(length)
                .expect("length")
                .min(self.bytes.len());
            Some(self.bytes[..end].to_vec())
        }
    }

    fn header() -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend(3_u32.to_le_bytes());
        out.extend(0_u64.to_le_bytes());
        let entries: [(&str, u32); 5] = [
            ("llama.block_count", 32),
            ("llama.embedding_length", 4096),
            ("llama.attention.head_count", 32),
            ("llama.attention.head_count_kv", 8),
            ("llama.context_length", 8192),
        ];
        out.extend((entries.len() as u64 + 1).to_le_bytes());
        let architecture = "general.architecture";
        out.extend((architecture.len() as u64).to_le_bytes());
        out.extend(architecture.as_bytes());
        out.extend(8_u32.to_le_bytes());
        out.extend(5_u64.to_le_bytes());
        out.extend(b"llama");
        for (key, value) in entries {
            out.extend((key.len() as u64).to_le_bytes());
            out.extend(key.as_bytes());
            out.extend(4_u32.to_le_bytes());
            out.extend(value.to_le_bytes());
        }
        out
    }

    fn file(filename: &str, size: u64) -> RunnabilityFile {
        RunnabilityFile {
            filename: filename.to_owned(),
            size,
            quantization: lettuce_model_hub::extract_quantization(filename),
        }
    }

    #[tokio::test]
    async fn scores_come_from_the_smallest_files_header() {
        let source = Header {
            bytes: header(),
            reads: Mutex::new(Vec::new()),
        };
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let secrets = InMemorySecretStore::default();
        let hardware = RunnabilityHardware {
            available_ram: Some(32_000_000_000),
            available_vram: Some(12_000_000_000),
            supports_gpu_offload: true,
            unified_memory: false,
        };
        let defaults = RunnabilityDefaults::new(None, None);
        let files = [
            file("m-Q8_0.gguf", 8_000_000_000),
            file("m-Q4_K_M.gguf", 4_000_000_000),
            file("m-F16.gguf", 0),
        ];
        let scores = browser
            .runnability(&secrets, &source, "org/m", &files, hardware, defaults)
            .await
            .expect("scores");
        assert_eq!(scores.len(), 3);
        assert_eq!(
            source.reads.lock().expect("reads").as_slice(),
            [("org/m/m-Q4_K_M.gguf".to_owned(), GGUF_HEADER_PROBE_BYTES)]
        );
        let recommendation = browser
            .recommendation(&secrets, &source, "org/m", &files, hardware, defaults)
            .await
            .expect("recommendation");
        assert_eq!(recommendation.model_max_context, 8192);
        assert_eq!(recommendation.files.len(), 2);
        assert!(recommendation.best.is_some());
        let truncated = Header {
            bytes: header()[..90].to_vec(),
            reads: Mutex::new(Vec::new()),
        };
        let recommendation = browser
            .recommendation(&secrets, &truncated, "org/m", &files, hardware, defaults)
            .await
            .expect("recommendation");
        assert!(recommendation.arch.expect("arch").incomplete_parse);
        assert_eq!(
            truncated
                .reads
                .lock()
                .expect("reads")
                .iter()
                .map(|(_, length)| *length)
                .collect::<Vec<_>>(),
            [GGUF_HEADER_PROBE_BYTES, GGUF_HEADER_RETRY_BYTES]
        );
        assert_eq!(
            browser
                .recommendation(&secrets, &source, "org/m", &[], hardware, defaults)
                .await
                .expect("empty"),
            RecommendationData::empty()
        );
    }

    #[test]
    fn a_downloaded_file_counts_its_gpu_sidecars() {
        let dir = std::env::temp_dir().join(format!("lettuce-runnability-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let model = dir.join("model-Q4_K_M.gguf");
        let mmproj = dir.join("mmproj.gguf");
        std::fs::write(&model, header()).expect("model");
        std::fs::write(&mmproj, vec![0_u8; 1000]).expect("mmproj");
        let hardware = RunnabilityHardware {
            available_ram: Some(8_000_000_000),
            available_vram: Some(4_000_000_000),
            supports_gpu_offload: true,
            unified_memory: false,
        };
        let defaults = RunnabilityDefaults::new(None, None);
        let model_path = model.to_string_lossy().into_owned();
        let mmproj_path = mmproj.to_string_lossy().into_owned();
        let bare = local_file_runnability(
            &model_path,
            &LocalModelSidecars::default(),
            hardware,
            defaults,
        )
        .expect("bare");
        assert_eq!(bare.quantization, "Q4_K_M");
        assert_eq!(bare.model_size, header().len() as u64);
        let with_projector = local_file_runnability(
            &model_path,
            &LocalModelSidecars {
                mmproj_path: Some(&mmproj_path),
                ..LocalModelSidecars::default()
            },
            RunnabilityHardware {
                supports_gpu_offload: false,
                ..hardware
            },
            defaults,
        )
        .expect("cpu");
        assert_eq!(with_projector.available_vram, 0);
        assert_eq!(
            local_file_runnability(
                &dir.join("missing.gguf").to_string_lossy(),
                &LocalModelSidecars::default(),
                hardware,
                defaults
            )
            .map(|_| ()),
            Err("File does not exist".to_owned())
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
