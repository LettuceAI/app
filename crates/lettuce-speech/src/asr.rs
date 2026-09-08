use std::{fmt, io::Read};

use lettuce_jobs::handle::CancellationToken;
use lettuce_media::{AssetKind, LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository};
use lettuce_types::{AssetId, ContentHash, JobId, RequestId, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const MAX_TRANSCRIPTION_AUDIO_SAMPLES: usize = 16_000 * 60 * 30;
const MAX_MODEL_ID_SCALARS: usize = 128;
const MAX_LANGUAGE_SCALARS: usize = 32;
const MAX_SCOPE_SCALARS: usize = 64;
const MAX_PROMPT_SCALARS: usize = 2_048;
const MAX_RESULT_TEXT_SCALARS: usize = 1_000_000;
pub(crate) const MAX_SEGMENTS: usize = 100_000;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AsrModelId(String);

impl AsrModelId {
    pub fn new(value: impl Into<String>) -> Result<Self, AsrValidationError> {
        let value = value.into();
        if value.trim() != value
            || value.is_empty()
            || value.chars().count() > MAX_MODEL_ID_SCALARS
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(AsrValidationError::InvalidModelId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AsrModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("AsrModelId").field(&self.0).finish()
    }
}

impl TryFrom<String> for AsrModelId {
    type Error = AsrValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AsrModelId> for String {
    fn from(value: AsrModelId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrModelDescriptor {
    pub id: AsrModelId,
    pub artifact_hash: ContentHash,
    pub english_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TranscriptionOptions {
    pub language: Option<String>,
    pub scopes: Vec<String>,
    pub initial_prompt: Option<String>,
    pub translate: bool,
    pub detect_language: bool,
    pub no_context: bool,
    pub single_segment: bool,
    pub token_timestamps: bool,
    pub split_on_word: bool,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub threads: Option<usize>,
    pub best_of: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub use_gpu: bool,
    pub force_cpu: bool,
    pub keep_model_loaded: bool,
    pub flash_attention: bool,
    pub gpu_device: i32,
}

impl Default for TranscriptionOptions {
    fn default() -> Self {
        Self {
            language: None,
            scopes: vec!["conversation".into(), "global".into()],
            initial_prompt: None,
            translate: false,
            detect_language: false,
            no_context: false,
            single_segment: false,
            token_timestamps: false,
            split_on_word: false,
            max_len: None,
            max_tokens: None,
            offset_ms: None,
            duration_ms: None,
            threads: None,
            best_of: None,
            temperature: None,
            temperature_inc: None,
            use_gpu: true,
            force_cpu: false,
            keep_model_loaded: true,
            flash_attention: false,
            gpu_device: 0,
        }
    }
}

impl TranscriptionOptions {
    pub fn validate(&self) -> Result<(), AsrValidationError> {
        validate_optional_text(&self.language, MAX_LANGUAGE_SCALARS)?;
        validate_optional_text(&self.initial_prompt, MAX_PROMPT_SCALARS)?;
        if self.scopes.len() > 8 {
            return Err(AsrValidationError::InvalidOptions);
        }
        for scope in &self.scopes {
            validate_text(scope, MAX_SCOPE_SCALARS)?;
        }
        if self
            .threads
            .is_some_and(|threads| threads > i32::MAX as usize)
            || self.gpu_device < 0
            || self.temperature.is_some_and(|value| !value.is_finite())
            || self.temperature_inc.is_some_and(|value| !value.is_finite())
        {
            return Err(AsrValidationError::InvalidOptions);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionRequest {
    pub id: RequestId,
    pub audio_asset_id: AssetId,
    pub model: AsrModelDescriptor,
    pub options: TranscriptionOptions,
    pub created_at: TimestampMillis,
}

impl TranscriptionRequest {
    pub fn validate(&self) -> Result<(), AsrValidationError> {
        self.options.validate()?;
        if self.model.english_only
            && self.options.language.as_deref().is_some_and(|language| {
                !language.eq_ignore_ascii_case("en")
                    && !language.eq_ignore_ascii_case("english")
                    && !language.eq_ignore_ascii_case("auto")
            })
        {
            return Err(AsrValidationError::UnsupportedLanguage);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionSegment {
    pub index: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub no_speech_probability: f32,
    pub speaker_turn_next: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedCorrection {
    pub correction_id: String,
    pub wrong: String,
    pub correct: String,
    pub matched_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionResult {
    pub request_id: RequestId,
    pub audio_asset_id: AssetId,
    pub model: AsrModelDescriptor,
    pub sample_rate_hz: u32,
    pub prompt: String,
    pub raw_text: String,
    pub corrected_text: String,
    pub detected_language: Option<String>,
    pub segments: Vec<TranscriptionSegment>,
    pub applied_corrections: Vec<AppliedCorrection>,
    pub completed_at: TimestampMillis,
}

impl TranscriptionResult {
    pub fn validate_for(&self, request: &TranscriptionRequest) -> Result<(), AsrValidationError> {
        if self.request_id != request.id
            || self.audio_asset_id != request.audio_asset_id
            || self.model != request.model
            || self.sample_rate_hz != 16_000
            || self.segments.len() > MAX_SEGMENTS
        {
            return Err(AsrValidationError::InvalidResult);
        }
        validate_result_text(&self.prompt)?;
        validate_result_text(&self.raw_text)?;
        validate_result_text(&self.corrected_text)?;
        validate_optional_text(&self.detected_language, MAX_LANGUAGE_SCALARS)?;
        if self.prompt.chars().count() > MAX_PROMPT_SCALARS {
            return Err(AsrValidationError::InvalidResult);
        }
        validate_segments(&self.segments)?;
        for correction in &self.applied_corrections {
            if correction.correction_id.trim().is_empty()
                || correction.correction_id.chars().count() > 128
                || correction.correction_id.chars().any(char::is_control)
            {
                return Err(AsrValidationError::InvalidResult);
            }
            validate_result_text(&correction.wrong)?;
            validate_result_text(&correction.correct)?;
            validate_result_text(&correction.matched_text)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum TranscriptionState {
    Pending,
    Succeeded { result: Box<TranscriptionResult> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionRecord {
    pub job_id: JobId,
    pub request: TranscriptionRequest,
    pub state: TranscriptionState,
}

impl TranscriptionRecord {
    pub fn validate(&self) -> Result<(), AsrValidationError> {
        self.request.validate()?;
        if let TranscriptionState::Succeeded { result } = &self.state {
            result.validate_for(&self.request)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

impl DecodedAudio {
    pub fn validate(&self) -> Result<(), AsrAudioError> {
        if self.samples.is_empty()
            || self.samples.len() > MAX_TRANSCRIPTION_AUDIO_SAMPLES
            || self.sample_rate_hz == 0
            || self.channels == 0
            || self.samples.len() < usize::from(self.channels)
            || self.samples.len() % usize::from(self.channels) != 0
            || self.samples.iter().any(|sample| !sample.is_finite())
        {
            return Err(AsrAudioError::InvalidAudio);
        }
        Ok(())
    }

    pub fn mono_16khz(&self) -> Result<Vec<f32>, AsrAudioError> {
        self.validate()?;
        let channels = usize::from(self.channels);
        let mono = self
            .samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
            .collect::<Vec<_>>();
        if self.sample_rate_hz == 16_000 {
            return Ok(mono);
        }
        let output_len =
            ((mono.len() as u64 * 16_000) / u64::from(self.sample_rate_hz)).max(1) as usize;
        if output_len > MAX_TRANSCRIPTION_AUDIO_SAMPLES {
            return Err(AsrAudioError::TooLarge);
        }
        let step = f64::from(self.sample_rate_hz) / 16_000_f64;
        Ok((0..output_len)
            .map(|index| {
                let position = index as f64 * step;
                let left_index = position.floor() as usize;
                let right_index = (left_index + 1).min(mono.len() - 1);
                let fraction = (position - left_index as f64) as f32;
                mono[left_index] + (mono[right_index] - mono[left_index]) * fraction
            })
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeTranscription {
    pub raw_text: String,
    pub detected_language: Option<String>,
    pub segments: Vec<TranscriptionSegment>,
}

impl RuntimeTranscription {
    pub fn validate(&self) -> Result<(), AsrValidationError> {
        validate_result_text(&self.raw_text)?;
        validate_optional_text(&self.detected_language, MAX_LANGUAGE_SCALARS)?;
        validate_segments(&self.segments)
    }
}

pub fn merge_transcription_prompt(
    vocabulary: &str,
    custom: Option<&str>,
) -> Result<String, AsrValidationError> {
    let prompt = [Some(vocabulary), custom]
        .into_iter()
        .flatten()
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if prompt.chars().count() > MAX_PROMPT_SCALARS || prompt.contains('\0') {
        return Err(AsrValidationError::InvalidOptions);
    }
    Ok(prompt)
}

pub trait AsrAudioSource: Send + Sync {
    fn decode(&self, asset_id: AssetId) -> Result<DecodedAudio, AsrAudioError>;
}

pub trait AsrPromptLibrary: Send + Sync {
    fn build_prompt(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<String, AsrLibraryError>;
    fn apply_corrections(
        &self,
        text: &str,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<(String, Vec<AppliedCorrection>), AsrLibraryError>;
}

pub trait AsrRuntime: Send + Sync {
    fn transcribe(
        &self,
        model: &AsrModelDescriptor,
        mono_16khz: &[f32],
        prompt: &str,
        options: &TranscriptionOptions,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeTranscription, AsrRuntimeError>;
}

pub trait TranscriptionRepository: Send + Sync {
    fn admit(
        &self,
        record: TranscriptionRecord,
    ) -> Result<TranscriptionRecord, TranscriptionRepositoryError>;
    fn get(&self, job_id: JobId) -> Result<TranscriptionRecord, TranscriptionRepositoryError>;
    fn settle(
        &self,
        job_id: JobId,
        result: TranscriptionResult,
    ) -> Result<TranscriptionRecord, TranscriptionRepositoryError>;
}

impl<BR, AR> AsrAudioSource for LocalMediaBlobStore<BR, AR>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    fn decode(&self, asset_id: AssetId) -> Result<DecodedAudio, AsrAudioError> {
        let opened = self
            .open_ready(asset_id)
            .map_err(|_| AsrAudioError::Unavailable)?;
        if !matches!(
            opened.asset.kind,
            AssetKind::MessageAudio | AssetKind::OtherAudio
        ) {
            return Err(AsrAudioError::InvalidAudio);
        }
        let byte_size =
            usize::try_from(opened.blob.byte_size).map_err(|_| AsrAudioError::TooLarge)?;
        let mut bytes = Vec::with_capacity(byte_size);
        opened
            .reader
            .take(opened.blob.byte_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| AsrAudioError::Unreadable)?;
        if bytes.len() != byte_size {
            return Err(AsrAudioError::Unreadable);
        }
        decode_wav(&bytes)
    }
}

fn decode_wav(bytes: &[u8]) -> Result<DecodedAudio, AsrAudioError> {
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(AsrAudioError::UnsupportedFormat);
    }
    let mut format = None;
    let mut data = None;
    let mut cursor = 12usize;
    while cursor.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let id = &bytes[cursor..cursor + 4];
        let size = usize::try_from(u32::from_le_bytes(
            bytes[cursor + 4..cursor + 8]
                .try_into()
                .map_err(|_| AsrAudioError::InvalidAudio)?,
        ))
        .map_err(|_| AsrAudioError::TooLarge)?;
        let start = cursor + 8;
        let end = start.checked_add(size).ok_or(AsrAudioError::TooLarge)?;
        if end > bytes.len() {
            return Err(AsrAudioError::InvalidAudio);
        }
        if id == b"fmt " {
            format = Some(&bytes[start..end]);
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }
        cursor = end.checked_add(size % 2).ok_or(AsrAudioError::TooLarge)?;
    }
    let format = format
        .filter(|chunk| chunk.len() >= 16)
        .ok_or(AsrAudioError::InvalidAudio)?;
    let data = data.ok_or(AsrAudioError::InvalidAudio)?;
    let codec = u16::from_le_bytes([format[0], format[1]]);
    let channels = u16::from_le_bytes([format[2], format[3]]);
    let sample_rate_hz = u32::from_le_bytes([format[4], format[5], format[6], format[7]]);
    let block_align = u16::from_le_bytes([format[12], format[13]]);
    let bits = u16::from_le_bytes([format[14], format[15]]);
    let expected_block_align = channels
        .checked_mul(bits / 8)
        .ok_or(AsrAudioError::InvalidAudio)?;
    if bits == 0
        || bits % 8 != 0
        || block_align == 0
        || block_align != expected_block_align
        || data.len() % usize::from(block_align) != 0
    {
        return Err(AsrAudioError::InvalidAudio);
    }
    let samples = match (codec, bits) {
        (1, 16) => data
            .chunks_exact(2)
            .map(|chunk| f32::from(i16::from_le_bytes([chunk[0], chunk[1]])) / f32::from(i16::MAX))
            .collect(),
        (1, 24) => data
            .chunks_exact(3)
            .map(|chunk| {
                let value = i32::from_le_bytes([
                    chunk[0],
                    chunk[1],
                    chunk[2],
                    if chunk[2] & 0x80 == 0 { 0 } else { 0xff },
                ]);
                value as f32 / 8_388_607_f32
            })
            .collect(),
        (1, 32) => data
            .chunks_exact(4)
            .map(|chunk| {
                i32::from_le_bytes(chunk.try_into().expect("four-byte chunk")) as f32
                    / i32::MAX as f32
            })
            .collect(),
        (3, 32) => data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
            .collect(),
        _ => return Err(AsrAudioError::UnsupportedFormat),
    };
    let audio = DecodedAudio {
        samples,
        sample_rate_hz,
        channels,
    };
    audio.validate()?;
    Ok(audio)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AsrValidationError {
    #[error("ASR model id is invalid")]
    InvalidModelId,
    #[error("transcription options are invalid")]
    InvalidOptions,
    #[error("the selected model does not support the requested language")]
    UnsupportedLanguage,
    #[error("transcription result is invalid")]
    InvalidResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AsrAudioError {
    #[error("audio is unavailable")]
    Unavailable,
    #[error("audio cannot be read")]
    Unreadable,
    #[error("audio format is unsupported")]
    UnsupportedFormat,
    #[error("audio is invalid")]
    InvalidAudio,
    #[error("audio exceeds the transcription limit")]
    TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AsrLibraryError {
    #[error("speech learning library is unavailable")]
    Unavailable,
    #[error("speech learning library returned invalid data")]
    InvalidData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AsrRuntimeError {
    #[error("transcription was cancelled")]
    Cancelled,
    #[error("speech runtime is unavailable")]
    Unavailable,
    #[error("speech model is unavailable")]
    ModelUnavailable,
    #[error("speech runtime rejected the request")]
    Rejected,
    #[error("speech runtime failed")]
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TranscriptionRepositoryError {
    #[error("transcription was not found")]
    NotFound,
    #[error("transcription conflicts with durable state")]
    Conflict,
    #[error("transcription data are invalid")]
    InvalidData,
    #[error("transcription storage failed")]
    Storage,
}

fn validate_text(value: &str, max: usize) -> Result<(), AsrValidationError> {
    if value.trim() != value
        || value.is_empty()
        || value.chars().count() > max
        || value.chars().any(char::is_control)
    {
        return Err(AsrValidationError::InvalidOptions);
    }
    Ok(())
}

fn validate_optional_text(value: &Option<String>, max: usize) -> Result<(), AsrValidationError> {
    if let Some(value) = value {
        validate_text(value, max)?;
    }
    Ok(())
}

fn validate_result_text(value: &str) -> Result<(), AsrValidationError> {
    if value.chars().count() > MAX_RESULT_TEXT_SCALARS || value.contains('\0') {
        return Err(AsrValidationError::InvalidResult);
    }
    Ok(())
}

fn validate_segments(segments: &[TranscriptionSegment]) -> Result<(), AsrValidationError> {
    if segments.len() > MAX_SEGMENTS {
        return Err(AsrValidationError::InvalidResult);
    }
    for (index, segment) in segments.iter().enumerate() {
        if usize::try_from(segment.index).ok() != Some(index)
            || segment.start_ms > segment.end_ms
            || !segment.no_speech_probability.is_finite()
            || !(0.0..=1.0).contains(&segment.no_speech_probability)
        {
            return Err(AsrValidationError::InvalidResult);
        }
        validate_result_text(&segment.text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_and_resamples_interleaved_pcm() {
        let audio = DecodedAudio {
            samples: vec![1.0, -1.0, 0.5, 0.5, -0.5, -0.5, 0.0, 1.0],
            sample_rate_hz: 8_000,
            channels: 2,
        };
        let mono = audio.mono_16khz().expect("normalized audio");
        assert_eq!(mono.len(), 8);
        assert_eq!(mono[0], 0.0);
        assert_eq!(mono[2], 0.5);
    }

    #[test]
    fn rejects_nonfinite_and_unaligned_audio() {
        let nonfinite = DecodedAudio {
            samples: vec![f32::NAN],
            sample_rate_hz: 16_000,
            channels: 1,
        };
        assert_eq!(nonfinite.validate(), Err(AsrAudioError::InvalidAudio));
        let unaligned = DecodedAudio {
            samples: vec![0.0, 0.0, 0.0],
            sample_rate_hz: 16_000,
            channels: 2,
        };
        assert_eq!(unaligned.validate(), Err(AsrAudioError::InvalidAudio));
    }

    #[test]
    fn decodes_bounded_stereo_pcm_wav() {
        let payload = [i16::MAX, i16::MIN, 0, i16::MAX];
        let data_size = u32::try_from(payload.len() * 2).expect("data size");
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&4_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for sample in payload {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        let decoded = decode_wav(&wav).expect("decoded WAV");
        assert_eq!(decoded.sample_rate_hz, 8_000);
        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.samples.len(), 4);
        assert_eq!(decoded.mono_16khz().expect("normalized").len(), 4);
    }

    #[test]
    fn rejects_truncated_wav_frames() {
        let mut wav = vec![0; 45];
        wav[..4].copy_from_slice(b"RIFF");
        wav[4..8].copy_from_slice(&37_u32.to_le_bytes());
        wav[8..16].copy_from_slice(b"WAVEfmt ");
        wav[16..20].copy_from_slice(&16_u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1_u16.to_le_bytes());
        wav[22..24].copy_from_slice(&2_u16.to_le_bytes());
        wav[24..28].copy_from_slice(&16_000_u32.to_le_bytes());
        wav[32..34].copy_from_slice(&4_u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16_u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(decode_wav(&wav), Err(AsrAudioError::InvalidAudio));
    }

    #[test]
    fn restores_runtime_defaults_from_older_request_documents() {
        let options: TranscriptionOptions = serde_json::from_str(
            r#"{"language":"en","scopes":["conversation"],"initial_prompt":null,"translate":false,"detect_language":false,"use_gpu":true,"keep_model_loaded":true}"#,
        )
        .expect("older options");
        assert_eq!(
            options,
            TranscriptionOptions {
                language: Some("en".into()),
                scopes: vec!["conversation".into()],
                ..TranscriptionOptions::default()
            }
        );
    }
}
