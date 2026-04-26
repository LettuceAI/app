use std::path::{Path, PathBuf};

use serde::Serialize;

use super::model::{
    chunk_lengths_for_tokens, resolve_config_path, write_wav, KokoroError, KokoroModel,
};
use super::phonemizer::{build_trace, EspeakConfig};
use super::vocab::{hardcoded_vocab, load_vocab};
use super::voices::{list_installed_voice_ids, resolve_voice_path, KokoroVoiceBlendSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KokoroModelVariant {
    Fp32,
    Fp16,
    Int8,
}

impl KokoroModelVariant {
    pub fn from_str(value: &str) -> Result<Self, KokoroError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fp32" | "f32" => Ok(Self::Fp32),
            "fp16" | "f16" => Ok(Self::Fp16),
            "int8" | "i8" | "q8" => Ok(Self::Int8),
            other => Err(KokoroError::UnsupportedVariant(other.to_string())),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Fp32 => "fp32",
            Self::Fp16 => "fp16",
            Self::Int8 => "int8",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fp32 => "Kokoro FP32",
            Self::Fp16 => "Kokoro FP16",
            Self::Int8 => "Kokoro Int8",
        }
    }

    pub fn candidate_model_filenames(self) -> &'static [&'static str] {
        match self {
            Self::Fp32 => &["model.onnx"],
            Self::Fp16 => &["model_fp16.onnx"],
            Self::Int8 => &["model_quantized.onnx", "model_uint8.onnx"],
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroModelVariantInfo {
    pub id: String,
    pub label: String,
    pub filename: String,
    pub size_mb: f32,
    pub mobile_supported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroInstalledVoice {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroAssetStatus {
    pub variant: String,
    pub variant_allowed_on_platform: bool,
    pub resolved_model_path: Option<String>,
    pub installed_voices: Vec<KokoroInstalledVoice>,
    pub selected_voice_installed: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct KokoroSynthesisRequest {
    pub asset_root: PathBuf,
    pub variant: KokoroModelVariant,
    pub voice_blend: Vec<KokoroVoiceBlendSpec>,
    pub text: String,
    pub speed: f32,
    pub espeak_bin_path: Option<PathBuf>,
    pub espeak_data_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroVoiceBlendInfo {
    pub voice_id: String,
    pub weight: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroTokenizePreviewSegment {
    pub kind: String,
    pub source_text: String,
    pub ipa: String,
    pub token_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroTokenizePreview {
    pub language: String,
    pub primary_voice_id: String,
    pub voice_blend: Vec<KokoroVoiceBlendInfo>,
    pub normalized_text: String,
    pub effective_text: String,
    pub lexicon_path: String,
    pub lexicon_entry_count: usize,
    pub used_lexicon_entries: Vec<String>,
    pub token_ids: Vec<i64>,
    pub token_count: usize,
    pub chunk_lengths: Vec<usize>,
    pub warnings: Vec<String>,
    pub segments: Vec<KokoroTokenizePreviewSegment>,
}

pub fn kokoro_supported_model_variants() -> Vec<KokoroModelVariantInfo> {
    [
        KokoroModelVariant::Fp32,
        KokoroModelVariant::Fp16,
        KokoroModelVariant::Int8,
    ]
    .into_iter()
    .filter(|variant| kokoro_platform_allows_variant(*variant))
    .map(|variant| KokoroModelVariantInfo {
        id: variant.id().to_string(),
        label: variant.label().to_string(),
        filename: variant.candidate_model_filenames()[0].to_string(),
        size_mb: match variant {
            KokoroModelVariant::Fp32 => 326.0,
            KokoroModelVariant::Fp16 => 163.0,
            KokoroModelVariant::Int8 => 92.4,
        },
        mobile_supported: kokoro_platform_allows_variant(variant),
    })
    .collect()
}

pub fn kokoro_platform_allows_variant(variant: KokoroModelVariant) -> bool {
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

pub fn validate_assets(
    asset_root: &Path,
    variant: KokoroModelVariant,
    selected_voice_id: Option<&str>,
) -> Result<KokoroAssetStatus, KokoroError> {
    let installed_voice_ids = list_installed_voice_ids(asset_root)?;
    let installed_voices = installed_voice_ids
        .iter()
        .map(|voice_id| KokoroInstalledVoice {
            id: voice_id.clone(),
            path: resolve_voice_path(asset_root, voice_id)
                .to_string_lossy()
                .to_string(),
        })
        .collect::<Vec<_>>();
    let selected_voice_installed =
        selected_voice_id.map(|voice_id| installed_voice_ids.iter().any(|id| id == voice_id));

    Ok(KokoroAssetStatus {
        variant: variant.id().to_string(),
        variant_allowed_on_platform: kokoro_platform_allows_variant(variant),
        resolved_model_path: resolve_model_path(asset_root, variant)
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
        installed_voices,
        selected_voice_installed,
    })
}

pub fn synthesize_to_wav(request: KokoroSynthesisRequest) -> Result<Vec<u8>, KokoroError> {
    if !kokoro_platform_allows_variant(request.variant) {
        return Err(KokoroError::UnsupportedVariant(format!(
            "{} is not allowed on this platform",
            request.variant.id()
        )));
    }

    let model_path = resolve_model_path(&request.asset_root, request.variant)?;
    if request.voice_blend.is_empty() {
        return Err(KokoroError::VoiceParse(
            "At least one Kokoro voice must be selected".to_string(),
        ));
    }
    for voice in &request.voice_blend {
        let voice_path = resolve_voice_path(&request.asset_root, &voice.voice_id);
        if !voice_path.is_file() {
            return Err(KokoroError::VoiceParse(format!(
                "Voice file not found: {}",
                voice_path.display()
            )));
        }
    }

    let vocab = load_vocab_or_default(&request.asset_root)?;
    let mut model = KokoroModel::load(&model_path, vocab)?;
    let samples = model.synthesize(
        &request.text,
        &request.voice_blend,
        request.speed,
        &EspeakConfig {
            bin_path: request.espeak_bin_path,
            data_path: request.espeak_data_path,
        },
        &request.asset_root,
    )?;
    write_wav(&samples)
}

pub fn preview_tokenization(
    asset_root: &Path,
    voice_blend: &[KokoroVoiceBlendSpec],
    text: &str,
    espeak_bin_path: Option<PathBuf>,
    espeak_data_path: Option<PathBuf>,
) -> Result<KokoroTokenizePreview, KokoroError> {
    let vocab = load_vocab_or_default(asset_root)?;
    let primary_voice_id = voice_blend
        .first()
        .map(|voice| voice.voice_id.as_str())
        .ok_or_else(|| KokoroError::VoiceParse("No Kokoro voice selected".to_string()))?;
    let lang = super::phonemizer::voice_lang(primary_voice_id);
    let trace = build_trace(
        text,
        lang,
        &vocab,
        &EspeakConfig {
            bin_path: espeak_bin_path,
            data_path: espeak_data_path,
        },
        asset_root,
    )?;
    let chunk_lengths = chunk_lengths_for_tokens(&trace.token_ids);
    let mut warnings = Vec::new();
    if trace.token_ids.len() > 400 {
        warnings.push(
            "Long utterance: official Kokoro voices often degrade past roughly 400 tokens."
                .to_string(),
        );
    }
    if trace.token_ids.len() > super::model::MAX_PHONEME_LEN {
        warnings.push(
            "Sequence exceeds the single-pass 510-token limit and will be chunked.".to_string(),
        );
    }
    if chunk_lengths.len() > 1 {
        warnings.push(format!(
            "Preview will synthesize across {} chunks with crossfading.",
            chunk_lengths.len()
        ));
    }

    Ok(KokoroTokenizePreview {
        language: trace.language,
        primary_voice_id: primary_voice_id.to_string(),
        voice_blend: voice_blend
            .iter()
            .map(|voice| KokoroVoiceBlendInfo {
                voice_id: voice.voice_id.clone(),
                weight: voice.weight,
            })
            .collect(),
        normalized_text: trace.normalized_text,
        effective_text: trace.effective_text,
        lexicon_path: trace.lexicon_path,
        lexicon_entry_count: trace.lexicon_entry_count,
        used_lexicon_entries: trace.used_lexicon_entries,
        token_count: trace.token_ids.len(),
        token_ids: trace.token_ids,
        chunk_lengths,
        warnings,
        segments: trace
            .segments
            .into_iter()
            .map(|segment| KokoroTokenizePreviewSegment {
                kind: segment.kind,
                source_text: segment.source_text,
                ipa: segment.ipa,
                token_ids: segment.token_ids,
            })
            .collect(),
    })
}

fn load_vocab_or_default(
    asset_root: &Path,
) -> Result<std::collections::HashMap<char, i64>, KokoroError> {
    let config_path = resolve_config_path(asset_root);
    if config_path.is_file() {
        match load_vocab(&config_path) {
            Ok(vocab) => Ok(vocab),
            Err(KokoroError::Config(_)) => Ok(hardcoded_vocab()),
            Err(other) => Err(other),
        }
    } else {
        Ok(hardcoded_vocab())
    }
}

fn resolve_model_path(
    asset_root: &Path,
    variant: KokoroModelVariant,
) -> Result<PathBuf, KokoroError> {
    let onnx_dir = asset_root.join("onnx");
    for filename in variant.candidate_model_filenames() {
        let nested = onnx_dir.join(filename);
        if nested.is_file() {
            return Ok(nested);
        }

        let flat = asset_root.join(filename);
        if flat.is_file() {
            return Ok(flat);
        }
    }

    Err(KokoroError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "No Kokoro model file found for {} in {}",
            variant.id(),
            asset_root.display()
        ),
    )))
}
