use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ort::{
    inputs,
    session::{builder::GraphOptimizationLevel, Input, Session},
    tensor::TensorElementType,
    value::{Value, ValueType},
};

use super::phonemizer::{phonemize, voice_lang, EspeakConfig};
use super::voices::{KokoroVoice, KokoroVoiceBlendSpec, STYLE_DIM};

pub const SAMPLE_RATE_HZ: u32 = 24_000;
pub const MAX_PHONEME_LEN: usize = 510;
const CHUNK_CROSSFADE_SAMPLES: usize = 240;

#[derive(thiserror::Error, Debug)]
pub enum KokoroError {
    #[error("ONNX runtime error: {0}")]
    Ort(#[from] ort::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Array shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),
    #[error(
        "espeak-ng not found. Install it or provide a bundled binary path for Kokoro phonemization."
    )]
    EspeakNotFound,
    #[error("Phonemization failed: {0}")]
    PhonemizerFailed(String),
    #[error("Invalid Kokoro config: {0}")]
    Config(String),
    #[error("Failed to parse Kokoro voice data: {0}")]
    VoiceParse(String),
    #[error("Unsupported Kokoro model variant: {0}")]
    UnsupportedVariant(String),
}

pub struct KokoroModel {
    session: Session,
    tokens_input_name: String,
    speed_uses_int32: bool,
    vocab: HashMap<char, i64>,
}

impl KokoroModel {
    pub fn load(model_path: &Path, vocab: HashMap<char, i64>) -> Result<Self, KokoroError> {
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_path)?;

        let tokens_input_name = detect_tokens_input_name(&session.inputs);
        let speed_uses_int32 = detect_speed_uses_int32(&session.inputs);

        Ok(Self {
            session,
            tokens_input_name,
            speed_uses_int32,
            vocab,
        })
    }

    pub fn synthesize(
        &mut self,
        text: &str,
        voice_blend: &[KokoroVoiceBlendSpec],
        speed: f32,
        espeak: &EspeakConfig,
        asset_root: &Path,
    ) -> Result<Vec<f32>, KokoroError> {
        let primary_voice_id = voice_blend
            .first()
            .map(|spec| spec.voice_id.as_str())
            .ok_or_else(|| KokoroError::VoiceParse("No Kokoro voice selected".to_string()))?;
        let lang = voice_lang(primary_voice_id);
        let token_ids = phonemize(text, lang, &self.vocab, espeak, asset_root)?;
        if token_ids.is_empty() {
            return Ok(Vec::new());
        }

        let (voice, _) = KokoroVoice::blend(asset_root, voice_blend)?;
        let chunks = split_chunks(&token_ids);
        let mut combined = Vec::new();

        for chunk in chunks {
            let style = voice.style_for_token_count(chunk.len());
            let audio = self.run_chunk(&chunk, &style, speed)?;
            if audio.is_empty() {
                continue;
            }
            if combined.is_empty() {
                combined.extend_from_slice(&audio);
            } else {
                append_with_crossfade(&mut combined, &audio, CHUNK_CROSSFADE_SAMPLES);
            }
        }

        Ok(combined)
    }

    fn run_chunk(
        &mut self,
        token_ids: &[i64],
        style: &[f32; STYLE_DIM],
        speed: f32,
    ) -> Result<Vec<f32>, KokoroError> {
        let seq_len = token_ids.len() + 2;
        let mut padded = vec![0i64; seq_len];
        padded[1..seq_len - 1].copy_from_slice(token_ids);

        let tokens_value = Value::from_array(([1usize, seq_len], padded))?;
        let style_value = Value::from_array(([1usize, STYLE_DIM], style.to_vec()))?;

        let outputs = if self.speed_uses_int32 {
            let speed_value = Value::from_array(([1usize], vec![speed.round() as i32]))?;
            let input_values = inputs![
                self.tokens_input_name.as_str() => tokens_value,
                "style" => style_value,
                "speed" => speed_value,
            ];
            self.session.run(input_values)?
        } else {
            let speed_value = Value::from_array(([1usize], vec![speed]))?;
            let input_values = inputs![
                self.tokens_input_name.as_str() => tokens_value,
                "style" => style_value,
                "speed" => speed_value,
            ];
            self.session.run(input_values)?
        };

        let (_, waveform) = outputs[0].try_extract_tensor::<f32>()?;
        Ok(waveform.to_vec())
    }
}

pub fn write_wav(samples: &[f32]) -> Result<Vec<u8>, KokoroError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE_HZ,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|err| {
        KokoroError::VoiceParse(format!("Failed to initialize WAV writer: {err}"))
    })?;

    for sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer
            .write_sample(scaled)
            .map_err(|err| KokoroError::VoiceParse(format!("Failed to write WAV sample: {err}")))?;
    }

    writer
        .finalize()
        .map_err(|err| KokoroError::VoiceParse(format!("Failed to finalize WAV output: {err}")))?;

    Ok(cursor.into_inner())
}

fn detect_tokens_input_name(inputs: &[Input]) -> String {
    inputs
        .iter()
        .find(|input| input.name == "input_ids" || input.name == "tokens")
        .map(|input| input.name.clone())
        .unwrap_or_else(|| "input_ids".to_string())
}

fn detect_speed_uses_int32(inputs: &[Input]) -> bool {
    inputs
        .iter()
        .find(|input| input.name == "speed")
        .map(|input| match &input.input_type {
            ValueType::Tensor { ty, .. } => matches!(ty, TensorElementType::Int32),
            _ => false,
        })
        .unwrap_or(false)
}

fn split_chunks(token_ids: &[i64]) -> Vec<Vec<i64>> {
    if token_ids.len() <= MAX_PHONEME_LEN {
        return vec![token_ids.to_vec()];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < token_ids.len() {
        let end = (start + MAX_PHONEME_LEN).min(token_ids.len());
        if end == token_ids.len() {
            chunks.push(token_ids[start..end].to_vec());
            break;
        }

        const PUNCT_IDS: &[i64] = &[1, 2, 3, 4, 5, 6];
        let split = token_ids[start..end]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, id)| PUNCT_IDS.contains(id))
            .map(|(idx, _)| start + idx + 1)
            .unwrap_or(end);

        chunks.push(token_ids[start..split].to_vec());
        start = split;
    }

    chunks
}

pub fn chunk_lengths_for_tokens(token_ids: &[i64]) -> Vec<usize> {
    split_chunks(token_ids)
        .into_iter()
        .map(|chunk| chunk.len())
        .collect()
}

fn append_with_crossfade(dst: &mut Vec<f32>, src: &[f32], crossfade_samples: usize) {
    let overlap = crossfade_samples.min(dst.len()).min(src.len());
    if overlap == 0 {
        dst.extend_from_slice(src);
        return;
    }

    let dst_start = dst.len() - overlap;
    for i in 0..overlap {
        let t = (i + 1) as f32 / (overlap as f32 + 1.0);
        let left = dst[dst_start + i] * (1.0 - t);
        let right = src[i] * t;
        dst[dst_start + i] = left + right;
    }
    dst.extend_from_slice(&src[overlap..]);
}

pub fn resolve_config_path(asset_root: &Path) -> PathBuf {
    asset_root.join("config.json")
}
