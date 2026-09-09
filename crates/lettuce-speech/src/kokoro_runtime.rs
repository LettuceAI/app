use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use lettuce_jobs::handle::CancellationToken;
use ort::{
    inputs,
    session::{Input, RunOptions, Session, builder::GraphOptimizationLevel},
    tensor::TensorElementType,
    value::{Value, ValueType},
};

use crate::{KOKORO_STYLE_DIMENSIONS, KokoroVoiceBlend};

pub const KOKORO_SAMPLE_RATE_HZ: u32 = 24_000;
pub const KOKORO_MAX_PHONEME_TOKENS: usize = 510;
const KOKORO_CROSSFADE_SAMPLES: usize = 240;
const PUNCTUATION_TOKEN_IDS: &[i64] = &[1, 2, 3, 4, 5, 6];
const WAV_HEADER_BYTES: usize = 44;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KokoroOnnxRuntimeLink {
    Dynamic(PathBuf),
    Linked,
}

#[derive(Debug)]
pub struct OnnxKokoroRuntime {
    session: Session,
    tokens_input_name: String,
    speed_uses_int32: bool,
}

impl OnnxKokoroRuntime {
    pub fn load(
        model_path: &Path,
        runtime: &KokoroOnnxRuntimeLink,
    ) -> Result<Self, KokoroRuntimeError> {
        initialize_onnx_runtime(runtime)?;
        let session = Session::builder()
            .map_err(|_| KokoroRuntimeError::Unavailable)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| KokoroRuntimeError::Unavailable)?
            .commit_from_file(model_path)
            .map_err(|_| KokoroRuntimeError::ModelLoad)?;
        let tokens_input_name = detect_tokens_input_name(&session.inputs)
            .ok_or(KokoroRuntimeError::InvalidModel)?;
        if !session.inputs.iter().any(|input| input.name == "style")
            || !session.inputs.iter().any(|input| input.name == "speed")
            || session.outputs.is_empty()
        {
            return Err(KokoroRuntimeError::InvalidModel);
        }
        let speed_uses_int32 = detect_speed_uses_int32(&session.inputs);
        Ok(Self {
            session,
            tokens_input_name,
            speed_uses_int32,
        })
    }
}

pub trait KokoroChunkRuntime {
    fn run_chunk(
        &mut self,
        token_ids: &[i64],
        style: &[f32; KOKORO_STYLE_DIMENSIONS],
        speed: f32,
        cancellation: &CancellationToken,
    ) -> Result<Vec<f32>, KokoroRuntimeError>;
}

impl KokoroChunkRuntime for OnnxKokoroRuntime {
    fn run_chunk(
        &mut self,
        token_ids: &[i64],
        style: &[f32; KOKORO_STYLE_DIMENSIONS],
        speed: f32,
        cancellation: &CancellationToken,
    ) -> Result<Vec<f32>, KokoroRuntimeError> {
        validate_chunk(token_ids, style, speed, cancellation)?;
        let sequence_length = token_ids.len() + 2;
        let mut padded = vec![0_i64; sequence_length];
        padded[1..sequence_length - 1].copy_from_slice(token_ids);
        let tokens = Value::from_array(([1_usize, sequence_length], padded))
            .map_err(|_| KokoroRuntimeError::InvalidInput)?;
        let style = Value::from_array(([1_usize, KOKORO_STYLE_DIMENSIONS], style.to_vec()))
            .map_err(|_| KokoroRuntimeError::InvalidInput)?;
        let options = Arc::new(RunOptions::new().map_err(|_| KokoroRuntimeError::Inference)?);
        let finished = Arc::new(AtomicBool::new(false));
        let canceller = spawn_canceller(
            Arc::clone(&options),
            Arc::clone(&finished),
            cancellation.clone(),
        );
        let result = if self.speed_uses_int32 {
            let speed = Value::from_array(([1_usize], vec![speed.round() as i32]))
                .map_err(|_| KokoroRuntimeError::InvalidInput)?;
            self.session.run_with_options(
                inputs![
                    self.tokens_input_name.as_str() => tokens,
                    "style" => style,
                    "speed" => speed
                ],
                options.as_ref(),
            )
        } else {
            let speed = Value::from_array(([1_usize], vec![speed]))
                .map_err(|_| KokoroRuntimeError::InvalidInput)?;
            self.session.run_with_options(
                inputs![
                    self.tokens_input_name.as_str() => tokens,
                    "style" => style,
                    "speed" => speed
                ],
                options.as_ref(),
            )
        };
        finished.store(true, Ordering::Release);
        if canceller.join().is_err() {
            return Err(KokoroRuntimeError::CancellationMonitor);
        }
        let outputs = result.map_err(|_| {
            if cancellation.is_cancelled() {
                KokoroRuntimeError::Cancelled
            } else {
                KokoroRuntimeError::Inference
            }
        })?;
        if cancellation.is_cancelled() {
            return Err(KokoroRuntimeError::Cancelled);
        }
        let output = outputs
            .values()
            .next()
            .ok_or(KokoroRuntimeError::InvalidOutput)?;
        let (_, samples) = output
            .try_extract_tensor::<f32>()
            .map_err(|_| KokoroRuntimeError::InvalidOutput)?;
        validate_samples(samples)?;
        Ok(samples.to_vec())
    }
}

pub fn synthesize_kokoro_tokens(
    runtime: &mut impl KokoroChunkRuntime,
    token_ids: &[i64],
    voice: &KokoroVoiceBlend,
    speed: f32,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, KokoroRuntimeError> {
    if !speed.is_finite() || speed <= 0.0 {
        return Err(KokoroRuntimeError::InvalidInput);
    }
    if cancellation.is_cancelled() {
        return Err(KokoroRuntimeError::Cancelled);
    }
    let chunks = split_kokoro_token_chunks(token_ids);
    let mut combined = Vec::new();
    for chunk in chunks {
        if cancellation.is_cancelled() {
            return Err(KokoroRuntimeError::Cancelled);
        }
        let style = voice.style_for_token_count(chunk.len());
        let audio = runtime.run_chunk(chunk, &style, speed, cancellation)?;
        validate_samples(&audio)?;
        if audio.is_empty() {
            continue;
        }
        append_with_crossfade(&mut combined, &audio)?;
    }
    encode_kokoro_wav(&combined)
}

#[must_use]
pub fn kokoro_chunk_lengths(token_ids: &[i64]) -> Vec<usize> {
    split_kokoro_token_chunks(token_ids)
        .into_iter()
        .map(<[i64]>::len)
        .collect()
}

fn split_kokoro_token_chunks(token_ids: &[i64]) -> Vec<&[i64]> {
    if token_ids.is_empty() {
        return Vec::new();
    }
    if token_ids.len() <= KOKORO_MAX_PHONEME_TOKENS {
        return vec![token_ids];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < token_ids.len() {
        let end = (start + KOKORO_MAX_PHONEME_TOKENS).min(token_ids.len());
        if end == token_ids.len() {
            chunks.push(&token_ids[start..end]);
            break;
        }
        let split = token_ids[start..end]
            .iter()
            .rposition(|id| PUNCTUATION_TOKEN_IDS.contains(id))
            .map_or(end, |index| start + index + 1);
        chunks.push(&token_ids[start..split]);
        start = split;
    }
    chunks
}

fn append_with_crossfade(dst: &mut Vec<f32>, src: &[f32]) -> Result<(), KokoroRuntimeError> {
    let maximum_samples = (lettuce_media::MAX_MEDIA_BLOB_BYTES as usize - WAV_HEADER_BYTES) / 2;
    let overlap = KOKORO_CROSSFADE_SAMPLES.min(dst.len()).min(src.len());
    let final_len = dst
        .len()
        .checked_add(src.len().saturating_sub(overlap))
        .filter(|length| *length <= maximum_samples)
        .ok_or(KokoroRuntimeError::LimitExceeded)?;
    if overlap == 0 {
        dst.extend_from_slice(src);
        return Ok(());
    }
    let dst_start = dst.len() - overlap;
    for index in 0..overlap {
        let fraction = (index + 1) as f32 / (overlap as f32 + 1.0);
        dst[dst_start + index] =
            dst[dst_start + index] * (1.0 - fraction) + src[index] * fraction;
    }
    dst.reserve(final_len - dst.len());
    dst.extend_from_slice(&src[overlap..]);
    Ok(())
}

fn encode_kokoro_wav(samples: &[f32]) -> Result<Vec<u8>, KokoroRuntimeError> {
    validate_samples(samples)?;
    let data_bytes = samples
        .len()
        .checked_mul(2)
        .and_then(|length| u32::try_from(length).ok())
        .ok_or(KokoroRuntimeError::LimitExceeded)?;
    let riff_bytes = data_bytes
        .checked_add(36)
        .ok_or(KokoroRuntimeError::LimitExceeded)?;
    let mut wav = Vec::with_capacity(WAV_HEADER_BYTES + samples.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_bytes.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&KOKORO_SAMPLE_RATE_HZ.to_le_bytes());
    wav.extend_from_slice(&(KOKORO_SAMPLE_RATE_HZ * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        let encoded = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        wav.extend_from_slice(&encoded.to_le_bytes());
    }
    Ok(wav)
}

fn validate_chunk(
    token_ids: &[i64],
    style: &[f32; KOKORO_STYLE_DIMENSIONS],
    speed: f32,
    cancellation: &CancellationToken,
) -> Result<(), KokoroRuntimeError> {
    if cancellation.is_cancelled() {
        return Err(KokoroRuntimeError::Cancelled);
    }
    if token_ids.is_empty()
        || token_ids.len() > KOKORO_MAX_PHONEME_TOKENS
        || token_ids.iter().any(|id| *id < 0)
        || style.iter().any(|value| !value.is_finite())
        || !speed.is_finite()
        || speed <= 0.0
    {
        return Err(KokoroRuntimeError::InvalidInput);
    }
    Ok(())
}

fn validate_samples(samples: &[f32]) -> Result<(), KokoroRuntimeError> {
    let maximum_samples = (lettuce_media::MAX_MEDIA_BLOB_BYTES as usize - WAV_HEADER_BYTES) / 2;
    if samples.len() > maximum_samples || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(KokoroRuntimeError::InvalidOutput);
    }
    Ok(())
}

fn detect_tokens_input_name(inputs: &[Input]) -> Option<String> {
    inputs
        .iter()
        .find(|input| matches!(input.name.as_str(), "input_ids" | "tokens"))
        .map(|input| input.name.clone())
}

fn detect_speed_uses_int32(inputs: &[Input]) -> bool {
    inputs
        .iter()
        .find(|input| input.name == "speed")
        .is_some_and(|input| {
            matches!(
                &input.input_type,
                ValueType::Tensor {
                    ty: TensorElementType::Int32,
                    ..
                }
            )
        })
}

fn initialize_onnx_runtime(runtime: &KokoroOnnxRuntimeLink) -> Result<(), KokoroRuntimeError> {
    let result = match runtime {
        KokoroOnnxRuntimeLink::Dynamic(path) => {
            let path = path.to_str().ok_or(KokoroRuntimeError::Unavailable)?;
            ort::init_from(path).with_name("lettuce-speech").commit()
        }
        KokoroOnnxRuntimeLink::Linked => ort::init().with_name("lettuce-speech").commit(),
    };
    result.map(|_| ()).map_err(|_| KokoroRuntimeError::Unavailable)
}

fn spawn_canceller(
    options: Arc<RunOptions>,
    finished: Arc<AtomicBool>,
    cancellation: CancellationToken,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !finished.load(Ordering::Acquire) {
            if cancellation.is_cancelled() {
                let _ = options.terminate();
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroRuntimeError {
    #[error("Kokoro ONNX Runtime is unavailable")]
    Unavailable,
    #[error("Kokoro model could not be loaded")]
    ModelLoad,
    #[error("Kokoro model contract is invalid")]
    InvalidModel,
    #[error("Kokoro synthesis input is invalid")]
    InvalidInput,
    #[error("Kokoro inference failed")]
    Inference,
    #[error("Kokoro inference output is invalid")]
    InvalidOutput,
    #[error("Kokoro synthesis output exceeds its limit")]
    LimitExceeded,
    #[error("Kokoro synthesis was cancelled")]
    Cancelled,
    #[error("Kokoro cancellation monitor failed")]
    CancellationMonitor,
}

#[cfg(test)]
mod tests {
    use crate::{KokoroVoiceBlendSpec, KokoroVoiceMaterial, blend_kokoro_voices};

    use super::*;

    #[derive(Default)]
    struct Runtime {
        chunk_lengths: Vec<usize>,
    }

    impl KokoroChunkRuntime for Runtime {
        fn run_chunk(
            &mut self,
            token_ids: &[i64],
            _: &[f32; KOKORO_STYLE_DIMENSIONS],
            _: f32,
            _: &CancellationToken,
        ) -> Result<Vec<f32>, KokoroRuntimeError> {
            self.chunk_lengths.push(token_ids.len());
            let value = (self.chunk_lengths.len() - 1) as f32;
            Ok(vec![value; 300])
        }
    }

    fn voice() -> KokoroVoiceBlend {
        let bytes = std::iter::repeat_n(1.0_f32, KOKORO_STYLE_DIMENSIONS)
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        blend_kokoro_voices(
            &[KokoroVoiceBlendSpec {
                voice_id: "af_heart".to_owned(),
                weight: 1.0,
            }],
            &[KokoroVoiceMaterial {
                voice_id: "af_heart",
                bytes: &bytes,
            }],
        )
        .expect("voice")
    }

    #[test]
    fn splits_on_legacy_punctuation_and_crossfades_chunks_into_wav() {
        let mut tokens = vec![50; 512];
        tokens[499] = 4;
        let mut runtime = Runtime::default();
        let wav = synthesize_kokoro_tokens(
            &mut runtime,
            &tokens,
            &voice(),
            1.0,
            &CancellationToken::new(),
        )
        .expect("synthesis");

        assert_eq!(runtime.chunk_lengths, [500, 12]);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), WAV_HEADER_BYTES + 360 * 2);
        assert_eq!(i16::from_le_bytes([wav[164], wav[165]]), 136);
        assert_eq!(i16::from_le_bytes([wav[642], wav[643]]), 32_631);
    }

    #[test]
    fn empty_tokens_produce_a_valid_empty_wav_and_cancellation_wins() {
        let mut runtime = Runtime::default();
        let cancellation = CancellationToken::new();
        let wav = synthesize_kokoro_tokens(&mut runtime, &[], &voice(), 1.0, &cancellation)
            .expect("empty wav");
        assert_eq!(wav.len(), WAV_HEADER_BYTES);
        cancellation.cancel();
        assert_eq!(
            synthesize_kokoro_tokens(&mut runtime, &[50], &voice(), 1.0, &cancellation),
            Err(KokoroRuntimeError::Cancelled)
        );
    }
}
