use std::{io::Read, path::Path};

use lettuce_platform::{ConfinedInstallStore, InstalledFile, ObjectKey, PlatformError};
use lettuce_types::ContentHash;

use crate::InstalledModelArtifact;

const MAX_KOKORO_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_KOKORO_VOICES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KokoroModelVariant {
    Fp32,
    Fp16,
    Int8,
}

impl KokoroModelVariant {
    pub fn parse(value: &str) -> Result<Self, KokoroAssetError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fp32" | "f32" => Ok(Self::Fp32),
            "fp16" | "f16" => Ok(Self::Fp16),
            "int8" | "i8" | "q8" => Ok(Self::Int8),
            _ => Err(KokoroAssetError::UnsupportedVariant),
        }
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fp32 => "fp32",
            Self::Fp16 => "fp16",
            Self::Int8 => "int8",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fp32 => "Kokoro FP32",
            Self::Fp16 => "Kokoro FP16",
            Self::Int8 => "Kokoro Int8",
        }
    }

    #[must_use]
    pub const fn primary_filename(self) -> &'static str {
        self.candidate_filenames()[0]
    }

    #[must_use]
    pub const fn candidate_filenames(self) -> &'static [&'static str] {
        match self {
            Self::Fp32 => &["model.onnx"],
            Self::Fp16 => &["model_fp16.onnx"],
            Self::Int8 => &["model_quantized.onnx", "model_uint8.onnx"],
        }
    }

    #[must_use]
    pub const fn size_mb(self) -> f32 {
        match self {
            Self::Fp32 => 326.0,
            Self::Fp16 => 163.0,
            Self::Int8 => 92.4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroModelVariantInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    pub size_mb: f32,
    pub mobile_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledKokoroVoice {
    pub id: String,
    pub artifact: InstalledModelArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroAssetStatus {
    pub variant: KokoroModelVariant,
    pub variant_allowed_on_platform: bool,
    pub model: Option<InstalledModelArtifact>,
    pub config: Option<InstalledModelArtifact>,
    pub tokenizer: Option<InstalledModelArtifact>,
    pub tokenizer_config: Option<InstalledModelArtifact>,
    pub installed_voices: Vec<InstalledKokoroVoice>,
    pub selected_voice_installed: Option<bool>,
}

#[derive(Debug)]
pub struct KokoroAssetStore {
    files: ConfinedInstallStore,
}

impl KokoroAssetStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, KokoroAssetError> {
        Ok(Self {
            files: ConfinedInstallStore::open(root).map_err(KokoroAssetError::Platform)?,
        })
    }

    pub fn inspect(
        &self,
        variant: KokoroModelVariant,
        selected_voice_id: Option<&str>,
    ) -> Result<KokoroAssetStatus, KokoroAssetError> {
        let installed_voices = self.installed_voices()?;
        let selected_voice_installed = selected_voice_id
            .map(|selected| installed_voices.iter().any(|voice| voice.id == selected));
        Ok(KokoroAssetStatus {
            variant,
            variant_allowed_on_platform: kokoro_platform_allows_variant(variant),
            model: self.resolve_model(variant)?,
            config: self.inspect_file(&["config.json"])?,
            tokenizer: self.inspect_file(&["tokenizer.json"])?,
            tokenizer_config: self.inspect_file(&["tokenizer_config.json"])?,
            installed_voices,
            selected_voice_installed,
        })
    }

    pub fn installed_voices(&self) -> Result<Vec<InstalledKokoroVoice>, KokoroAssetError> {
        let directory = ObjectKey::single("voices").map_err(KokoroAssetError::Platform)?;
        let entries = self
            .files
            .list(&directory, MAX_KOKORO_VOICES)
            .map_err(KokoroAssetError::Platform)?;
        let mut voices = Vec::new();
        for entry in entries {
            if !entry.is_file || !entry.name.ends_with(".bin") {
                continue;
            }
            let Some(id) = entry.name.strip_suffix(".bin") else {
                continue;
            };
            if id.trim().is_empty() {
                continue;
            }
            let artifact = self
                .inspect_file(&["voices", entry.name.as_str()])?
                .ok_or(KokoroAssetError::ChangedDuringInspection)?;
            voices.push(InstalledKokoroVoice {
                id: id.to_owned(),
                artifact,
            });
        }
        Ok(voices)
    }

    fn resolve_model(
        &self,
        variant: KokoroModelVariant,
    ) -> Result<Option<InstalledModelArtifact>, KokoroAssetError> {
        for filename in variant.candidate_filenames() {
            if let Some(artifact) = self.inspect_file(&["onnx", filename])? {
                return Ok(Some(artifact));
            }
            if let Some(artifact) = self.inspect_file(&[filename])? {
                return Ok(Some(artifact));
            }
        }
        Ok(None)
    }

    fn inspect_file(
        &self,
        segments: &[&str],
    ) -> Result<Option<InstalledModelArtifact>, KokoroAssetError> {
        let key = ObjectKey::from_segments(segments).map_err(KokoroAssetError::Platform)?;
        let Some(mut file) = self
            .files
            .inspect(&key)
            .map_err(KokoroAssetError::Platform)?
        else {
            return Ok(None);
        };
        inspect_artifact(&mut file).map(Some)
    }
}

#[must_use]
pub fn kokoro_supported_model_variants() -> Vec<KokoroModelVariantInfo> {
    [
        KokoroModelVariant::Fp32,
        KokoroModelVariant::Fp16,
        KokoroModelVariant::Int8,
    ]
    .into_iter()
    .filter(|variant| kokoro_platform_allows_variant(*variant))
    .map(|variant| KokoroModelVariantInfo {
        id: variant.id(),
        label: variant.label(),
        filename: variant.primary_filename(),
        size_mb: variant.size_mb(),
        mobile_supported: kokoro_platform_allows_variant(variant),
    })
    .collect()
}

#[must_use]
pub const fn kokoro_platform_allows_variant(variant: KokoroModelVariant) -> bool {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        matches!(variant, KokoroModelVariant::Int8)
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = variant;
        true
    }
}

fn inspect_artifact(file: &mut InstalledFile) -> Result<InstalledModelArtifact, KokoroAssetError> {
    if file.is_empty() || file.len() > MAX_KOKORO_ARTIFACT_BYTES {
        return Err(KokoroAssetError::InvalidArtifact);
    }
    file.rewind().map_err(KokoroAssetError::Platform)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| KokoroAssetError::UnreadableArtifact)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let blake3 = ContentHash::parse(hasher.finalize().to_hex().to_string())
        .map_err(|_| KokoroAssetError::InvalidArtifact)?;
    Ok(InstalledModelArtifact {
        path: file.native_path().to_path_buf(),
        byte_size: file.len(),
        blake3,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroAssetError {
    #[error("Kokoro model variant is unsupported")]
    UnsupportedVariant,
    #[error("Kokoro managed asset access failed: {0}")]
    Platform(PlatformError),
    #[error("Kokoro artifact is invalid")]
    InvalidArtifact,
    #[error("Kokoro artifact is unreadable")]
    UnreadableArtifact,
    #[error("Kokoro assets changed during inspection")]
    ChangedDuringInspection,
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;

    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("kokoro-assets-{}", OperationId::new()))
    }

    #[test]
    fn preserves_variant_aliases_and_platform_catalog() {
        assert_eq!(
            KokoroModelVariant::parse(" f32 ").expect("fp32 alias"),
            KokoroModelVariant::Fp32
        );
        assert_eq!(
            KokoroModelVariant::parse("F16").expect("fp16 alias"),
            KokoroModelVariant::Fp16
        );
        assert_eq!(
            KokoroModelVariant::parse("q8").expect("int8 alias"),
            KokoroModelVariant::Int8
        );
        assert_eq!(
            KokoroModelVariant::parse("q4"),
            Err(KokoroAssetError::UnsupportedVariant)
        );
        let variants = kokoro_supported_model_variants();
        if cfg!(any(target_os = "android", target_os = "ios")) {
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].id, "int8");
        } else {
            assert_eq!(
                variants.iter().map(|entry| entry.id).collect::<Vec<_>>(),
                ["fp32", "fp16", "int8"]
            );
            assert_eq!(variants[0].size_mb, 326.0);
            assert_eq!(variants[1].size_mb, 163.0);
            assert_eq!(variants[2].size_mb, 92.4);
        }
    }

    #[test]
    fn inventories_expected_files_and_sorted_voice_ids() {
        let root = root();
        std::fs::create_dir_all(root.join("onnx")).expect("onnx directory");
        std::fs::create_dir_all(root.join("voices")).expect("voices directory");
        std::fs::write(root.join("onnx/model_quantized.onnx"), b"model").expect("model");
        std::fs::write(root.join("config.json"), b"config").expect("config");
        std::fs::write(root.join("tokenizer.json"), b"tokenizer").expect("tokenizer");
        std::fs::write(root.join("tokenizer_config.json"), b"tokenizer config")
            .expect("tokenizer config");
        std::fs::write(root.join("voices/zf_xiaobei.bin"), b"voice z").expect("voice z");
        std::fs::write(root.join("voices/af_heart.bin"), b"voice a").expect("voice a");
        std::fs::write(root.join("voices/ignored.BIN"), b"ignored").expect("ignored file");
        std::fs::write(root.join("voices/readme.txt"), b"ignored").expect("ignored text");

        let store = KokoroAssetStore::open(&root).expect("asset store");
        let status = store
            .inspect(KokoroModelVariant::Int8, Some("af_heart"))
            .expect("asset status");
        assert_eq!(
            status
                .installed_voices
                .iter()
                .map(|voice| voice.id.as_str())
                .collect::<Vec<_>>(),
            ["af_heart", "zf_xiaobei"]
        );
        assert_eq!(status.selected_voice_installed, Some(true));
        assert!(status.model.is_some());
        assert!(status.config.is_some());
        assert!(status.tokenizer.is_some());
        assert!(status.tokenizer_config.is_some());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn preserves_nested_flat_and_int8_fallback_resolution_order() {
        let root = root();
        std::fs::create_dir_all(root.join("onnx")).expect("onnx directory");
        std::fs::write(root.join("model.onnx"), b"flat fp32").expect("flat fp32");
        std::fs::write(root.join("onnx/model_fp16.onnx"), b"nested fp16").expect("nested fp16");
        std::fs::write(root.join("model_quantized.onnx"), b"primary int8").expect("primary int8");
        std::fs::write(root.join("onnx/model_uint8.onnx"), b"fallback int8")
            .expect("fallback int8");
        let store = KokoroAssetStore::open(&root).expect("asset store");

        let fp32 = store
            .inspect(KokoroModelVariant::Fp32, None)
            .expect("fp32 status")
            .model
            .expect("fp32 model");
        let fp16 = store
            .inspect(KokoroModelVariant::Fp16, None)
            .expect("fp16 status")
            .model
            .expect("fp16 model");
        let int8 = store
            .inspect(KokoroModelVariant::Int8, Some("missing"))
            .expect("int8 status");
        assert!(fp32.path.ends_with("model.onnx"));
        assert!(fp16.path.ends_with("onnx/model_fp16.onnx"));
        assert!(
            int8.model
                .expect("int8 model")
                .path
                .ends_with("model_quantized.onnx")
        );
        assert_eq!(int8.selected_voice_installed, Some(false));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn reports_missing_assets_and_rejects_empty_or_symbolic_assets() {
        let root = root();
        let store = KokoroAssetStore::open(&root).expect("asset store");
        let missing = store
            .inspect(KokoroModelVariant::Int8, None)
            .expect("missing status");
        assert!(missing.model.is_none());
        assert!(missing.installed_voices.is_empty());

        std::fs::write(root.join("model_quantized.onnx"), []).expect("empty model");
        assert_eq!(
            store.inspect(KokoroModelVariant::Int8, None),
            Err(KokoroAssetError::InvalidArtifact)
        );

        #[cfg(unix)]
        {
            std::fs::remove_file(root.join("model_quantized.onnx")).expect("remove empty model");
            std::os::unix::fs::symlink("outside.onnx", root.join("model_quantized.onnx"))
                .expect("model symlink");
            assert!(matches!(
                store.inspect(KokoroModelVariant::Int8, None),
                Err(KokoroAssetError::Platform(PlatformError::SymlinkEscape))
            ));
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
