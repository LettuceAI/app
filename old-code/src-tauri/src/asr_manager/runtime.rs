use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use hound::{SampleFormat, WavReader};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use whisper_rs::{
    get_lang_str, install_logging_hooks, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters,
};

use super::{asr_apply_corrections, asr_build_prompt, AsrCorrectionApplication};

#[derive(Default)]
pub(crate) struct WhisperRuntimeState {
    contexts: Mutex<HashMap<WhisperContextCacheKey, Arc<WhisperContext>>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct WhisperContextCacheKey {
    model_path: String,
    use_gpu: bool,
    flash_attn: bool,
    gpu_device: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrWhisperSegment {
    pub index: i32,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub no_speech_probability: f32,
    pub speaker_turn_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrWhisperTranscribePcmRequest {
    pub model_path: String,
    /// Little-endian f32 interleaved PCM samples.
    pub pcm_bytes: Vec<u8>,
    pub sample_rate_hz: u32,
    /// Channel count for the interleaved PCM. Defaults to 1.
    pub channels: Option<u16>,
    pub language: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub initial_prompt: Option<String>,
    pub translate: Option<bool>,
    pub detect_language: Option<bool>,
    pub no_context: Option<bool>,
    pub single_segment: Option<bool>,
    pub token_timestamps: Option<bool>,
    pub split_on_word: Option<bool>,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub threads: Option<usize>,
    pub best_of: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub use_gpu: Option<bool>,
    pub force_cpu: Option<bool>,
    pub keep_model_loaded: Option<bool>,
    pub flash_attn: Option<bool>,
    pub gpu_device: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrWhisperTranscribeRequest {
    pub model_path: String,
    pub audio_path: String,
    pub language: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub initial_prompt: Option<String>,
    pub translate: Option<bool>,
    pub detect_language: Option<bool>,
    pub no_context: Option<bool>,
    pub single_segment: Option<bool>,
    pub token_timestamps: Option<bool>,
    pub split_on_word: Option<bool>,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub threads: Option<usize>,
    pub best_of: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub use_gpu: Option<bool>,
    pub force_cpu: Option<bool>,
    pub keep_model_loaded: Option<bool>,
    pub flash_attn: Option<bool>,
    pub gpu_device: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrWhisperRuntimeLoadRequest {
    pub model_path: String,
    pub use_gpu: Option<bool>,
    pub force_cpu: Option<bool>,
    pub flash_attn: Option<bool>,
    pub gpu_device: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrWhisperTranscriptionResponse {
    pub audio_path: String,
    pub model_path: String,
    pub sample_rate_hz: u32,
    pub prompt: String,
    pub raw_text: String,
    pub corrected_text: String,
    pub detected_language: Option<String>,
    pub segments: Vec<AsrWhisperSegment>,
    pub applied_corrections: Vec<AsrCorrectionApplication>,
}

static WHISPER_LOGGING_ONCE: OnceLock<()> = OnceLock::new();

pub(crate) fn initialize_whisper_logging() {
    WHISPER_LOGGING_ONCE.get_or_init(|| {
        install_logging_hooks();
    });
}

fn normalize_prompt_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_language(language: Option<&str>) -> Option<String> {
    language
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn merge_initial_prompt(vocabulary_prompt: &str, custom_prompt: Option<&str>) -> String {
    let mut parts = Vec::new();
    if !vocabulary_prompt.trim().is_empty() {
        parts.push(normalize_prompt_text(vocabulary_prompt));
    }
    if let Some(custom_prompt) = custom_prompt {
        let custom_prompt = normalize_prompt_text(custom_prompt);
        if !custom_prompt.is_empty() {
            parts.push(custom_prompt);
        }
    }
    parts.join(" ")
}

fn canonicalize_existing_path(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Path does not exist: {}", path.display()),
        ));
    }
    fs::canonicalize(&path).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn context_cache_key(
    request: &AsrWhisperTranscribeRequest,
    model_path: &Path,
) -> WhisperContextCacheKey {
    WhisperContextCacheKey {
        model_path: model_path.to_string_lossy().to_string(),
        use_gpu: effective_use_gpu(request.use_gpu, request.force_cpu),
        flash_attn: request.flash_attn.unwrap_or(false),
        gpu_device: request.gpu_device.unwrap_or(0),
    }
}

fn effective_use_gpu(use_gpu: Option<bool>, force_cpu: Option<bool>) -> bool {
    if force_cpu.unwrap_or(false) {
        false
    } else {
        use_gpu.unwrap_or(false)
    }
}

fn should_keep_model_loaded(keep_model_loaded: Option<bool>) -> bool {
    keep_model_loaded.unwrap_or(true)
}

fn load_or_create_context(
    app: &tauri::AppHandle,
    request: &AsrWhisperTranscribeRequest,
    model_path: &Path,
) -> Result<Arc<WhisperContext>, String> {
    let key = context_cache_key(request, model_path);
    let state = app.state::<WhisperRuntimeState>();
    {
        let cache = state.contexts.lock().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Runtime cache lock poisoned: {}", e),
            )
        })?;
        if let Some(context) = cache.get(&key) {
            return Ok(context.clone());
        }
    }

    let mut parameters = WhisperContextParameters::new();
    parameters.use_gpu(effective_use_gpu(request.use_gpu, request.force_cpu));
    parameters.flash_attn(request.flash_attn.unwrap_or(false));
    parameters.gpu_device(request.gpu_device.unwrap_or(0));

    let context = Arc::new(
        WhisperContext::new_with_params(model_path, parameters).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load whisper model: {}", e),
            )
        })?,
    );

    let mut cache = state.contexts.lock().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Runtime cache lock poisoned: {}", e),
        )
    })?;
    Ok(cache.entry(key).or_insert_with(|| context.clone()).clone())
}

fn evict_context(
    app: &tauri::AppHandle,
    request: &AsrWhisperTranscribeRequest,
    model_path: &Path,
) -> Result<(), String> {
    let key = context_cache_key(request, model_path);
    let state = app.state::<WhisperRuntimeState>();
    let mut cache = state.contexts.lock().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Runtime cache lock poisoned: {}", e),
        )
    })?;
    cache.remove(&key);
    Ok(())
}

fn decode_wav_file(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader = WavReader::open(path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "WAV file has zero channels",
        ));
    }

    let samples = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| {
                sample.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
            })
            .collect::<Result<Vec<_>, _>>()?,
        SampleFormat::Int if spec.bits_per_sample <= 16 => reader
            .samples::<i16>()
            .map(|sample| {
                sample
                    .map(|value| value as f32 / i16::MAX as f32)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
            })
            .collect::<Result<Vec<_>, _>>()?,
        SampleFormat::Int if spec.bits_per_sample <= 32 => {
            let scale = ((1_i64 << (spec.bits_per_sample.saturating_sub(1) as u32)) - 1) as f32;
            reader
                .samples::<i32>()
                .map(|sample| {
                    sample
                        .map(|value| value as f32 / scale)
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Unsupported WAV format: {:?} {} bits",
                    spec.sample_format, spec.bits_per_sample
                ),
            ));
        }
    };

    let mono = downmix_to_mono(&samples, spec.channels as usize);
    Ok((resample_to_16khz(&mono, spec.sample_rate), 16_000))
}

fn downmix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    let mut mono = Vec::with_capacity(samples.len() / channels.max(1));
    for frame in samples.chunks(channels) {
        let sum: f32 = frame.iter().copied().sum();
        mono.push(sum / frame.len() as f32);
    }
    mono
}

fn resample_to_16khz(samples: &[f32], source_rate: u32) -> Vec<f32> {
    const TARGET_RATE: u32 = 16_000;
    if source_rate == TARGET_RATE || samples.is_empty() {
        return samples.to_vec();
    }

    let output_len =
        ((samples.len() as u64 * TARGET_RATE as u64) / source_rate as u64).max(1) as usize;
    let step = source_rate as f64 / TARGET_RATE as f64;
    let mut output = Vec::with_capacity(output_len);

    for index in 0..output_len {
        let source_position = index as f64 * step;
        let left_index = source_position.floor() as usize;
        let right_index = (left_index + 1).min(samples.len().saturating_sub(1));
        let fraction = (source_position - left_index as f64) as f32;
        let left = samples[left_index];
        let right = samples[right_index];
        output.push(left + (right - left) * fraction);
    }

    output
}

fn build_params<'a>(
    request: &AsrWhisperTranscribeRequest,
    language: Option<&'a str>,
    initial_prompt: Option<&'a str>,
) -> FullParams<'a, 'static> {
    let mut params = FullParams::new(SamplingStrategy::Greedy {
        best_of: request.best_of.unwrap_or(1).max(1),
    });
    params.set_n_threads(request.threads.unwrap_or(4).max(1) as i32);
    params.set_translate(request.translate.unwrap_or(false));
    params.set_no_context(request.no_context.unwrap_or(false));
    params.set_single_segment(request.single_segment.unwrap_or(false));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(false);

    if let Some(offset_ms) = request.offset_ms {
        params.set_offset_ms(offset_ms.max(0));
    }
    if let Some(duration_ms) = request.duration_ms {
        params.set_duration_ms(duration_ms.max(0));
    }
    if let Some(max_len) = request.max_len {
        params.set_max_len(max_len.max(0));
    }
    if let Some(max_tokens) = request.max_tokens {
        params.set_max_tokens(max_tokens.max(0));
    }
    if let Some(temperature) = request.temperature {
        params.set_temperature(temperature);
    }
    if let Some(temperature_inc) = request.temperature_inc {
        params.set_temperature_inc(temperature_inc);
    }
    if request.token_timestamps.unwrap_or(false) {
        params.set_token_timestamps(true);
    }
    if request.split_on_word.unwrap_or(false) {
        params.set_split_on_word(true);
    }

    let detect_language = request.detect_language.unwrap_or(false)
        || language.is_some_and(|value| value.eq_ignore_ascii_case("auto"));
    if detect_language {
        params.set_language(None);
        params.set_detect_language(true);
    } else {
        params.set_language(language);
    }

    if let Some(initial_prompt) = initial_prompt.filter(|value| !value.is_empty()) {
        params.set_initial_prompt(initial_prompt);
    }

    params
}

fn collect_segments(state: &whisper_rs::WhisperState) -> Result<Vec<AsrWhisperSegment>, String> {
    let mut segments = Vec::new();
    for index in 0..state.full_n_segments() {
        let segment = state.get_segment(index).ok_or_else(|| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Missing whisper segment at index {}", index),
            )
        })?;
        let text = segment
            .to_str_lossy()
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read segment text: {}", e),
                )
            })?
            .into_owned();
        segments.push(AsrWhisperSegment {
            index,
            start_ms: segment.start_timestamp() * 10,
            end_ms: segment.end_timestamp() * 10,
            text,
            no_speech_probability: segment.no_speech_probability(),
            speaker_turn_next: segment.next_segment_speaker_turn(),
        });
    }
    Ok(segments)
}

/// Shared transcription pipeline. Takes already-decoded mono/multichannel f32 PCM
/// at any sample rate; downmixes + resamples to 16kHz internally if needed.
fn run_whisper_on_pcm(
    app: &tauri::AppHandle,
    request: &AsrWhisperTranscribeRequest,
    model_path: &Path,
    audio_path_for_response: String,
    pcm_samples: Vec<f32>,
    source_sample_rate: u32,
    source_channels: usize,
) -> Result<AsrWhisperTranscriptionResponse, String> {
    let mono = downmix_to_mono(&pcm_samples, source_channels.max(1));
    let pcm_16k = resample_to_16khz(&mono, source_sample_rate);
    let sample_rate_hz: u32 = 16_000;

    let context = load_or_create_context(app, request, model_path)?;

    let vocabulary_prompt = asr_build_prompt(
        app.clone(),
        request.language.clone(),
        request.scopes.clone(),
    )?;
    let merged_prompt = merge_initial_prompt(&vocabulary_prompt, request.initial_prompt.as_deref());
    let requested_language = normalize_language(request.language.as_deref());

    let mut state = context.create_state().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create whisper state: {}", e),
        )
    })?;
    let params = build_params(
        request,
        requested_language.as_deref(),
        (!merged_prompt.is_empty()).then_some(merged_prompt.as_str()),
    );

    state.full(params, &pcm_16k).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Whisper transcription failed: {}", e),
        )
    })?;

    let segments = collect_segments(&state)?;
    let raw_text = segments
        .iter()
        .map(|segment| segment.text.trim())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let corrected = asr_apply_corrections(
        app.clone(),
        raw_text.clone(),
        request.language.clone(),
        request.scopes.clone(),
    )?;

    let detected_language =
        get_lang_str(state.full_lang_id_from_state()).map(|value| value.to_string());

    Ok(AsrWhisperTranscriptionResponse {
        audio_path: audio_path_for_response,
        model_path: model_path.to_string_lossy().to_string(),
        sample_rate_hz,
        prompt: merged_prompt,
        raw_text,
        corrected_text: corrected.corrected_text,
        detected_language,
        segments,
        applied_corrections: corrected.applied,
    })
}

fn decode_pcm_bytes(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "PCM payload length {} is not a multiple of 4 (f32 LE)",
                bytes.len()
            ),
        ));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn transcribe_pcm_sync(
    app: &tauri::AppHandle,
    request: AsrWhisperTranscribePcmRequest,
) -> Result<AsrWhisperTranscriptionResponse, String> {
    initialize_whisper_logging();

    if request.sample_rate_hz == 0 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Sample rate must be greater than zero",
        ));
    }
    let channels = request.channels.unwrap_or(1).max(1) as usize;
    let pcm = decode_pcm_bytes(&request.pcm_bytes)?;
    if pcm.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "PCM payload is empty",
        ));
    }

    let model_path = canonicalize_existing_path(&request.model_path)?;
    let file_request = AsrWhisperTranscribeRequest {
        model_path: request.model_path.clone(),
        audio_path: String::new(),
        language: request.language,
        scopes: request.scopes,
        initial_prompt: request.initial_prompt,
        translate: request.translate,
        detect_language: request.detect_language,
        no_context: request.no_context,
        single_segment: request.single_segment,
        token_timestamps: request.token_timestamps,
        split_on_word: request.split_on_word,
        max_len: request.max_len,
        max_tokens: request.max_tokens,
        offset_ms: request.offset_ms,
        duration_ms: request.duration_ms,
        threads: request.threads,
        best_of: request.best_of,
        temperature: request.temperature,
        temperature_inc: request.temperature_inc,
        use_gpu: request.use_gpu,
        force_cpu: request.force_cpu,
        keep_model_loaded: request.keep_model_loaded,
        flash_attn: request.flash_attn,
        gpu_device: request.gpu_device,
    };

    let response = run_whisper_on_pcm(
        app,
        &file_request,
        &model_path,
        String::new(),
        pcm,
        request.sample_rate_hz,
        channels,
    )?;

    if !should_keep_model_loaded(file_request.keep_model_loaded) {
        evict_context(app, &file_request, &model_path)?;
    }

    Ok(response)
}

fn transcribe_file_sync(
    app: &tauri::AppHandle,
    request: AsrWhisperTranscribeRequest,
) -> Result<AsrWhisperTranscriptionResponse, String> {
    initialize_whisper_logging();

    let model_path = canonicalize_existing_path(&request.model_path)?;
    let audio_path = canonicalize_existing_path(&request.audio_path)?;
    // `decode_wav_file` already downmixes to mono and resamples to 16kHz.
    let (pcm, sample_rate) = decode_wav_file(&audio_path)?;

    let response = run_whisper_on_pcm(
        app,
        &request,
        &model_path,
        audio_path.to_string_lossy().to_string(),
        pcm,
        sample_rate,
        1,
    )?;

    if !should_keep_model_loaded(request.keep_model_loaded) {
        evict_context(app, &request, &model_path)?;
    }

    Ok(response)
}

fn preload_model_sync(
    app: &tauri::AppHandle,
    request: AsrWhisperRuntimeLoadRequest,
) -> Result<(), String> {
    initialize_whisper_logging();
    let model_path = canonicalize_existing_path(&request.model_path)?;
    let load_request = AsrWhisperTranscribeRequest {
        model_path: request.model_path,
        audio_path: String::new(),
        language: None,
        scopes: None,
        initial_prompt: None,
        translate: None,
        detect_language: None,
        no_context: None,
        single_segment: None,
        token_timestamps: None,
        split_on_word: None,
        max_len: None,
        max_tokens: None,
        offset_ms: None,
        duration_ms: None,
        threads: None,
        best_of: None,
        temperature: None,
        temperature_inc: None,
        use_gpu: request.use_gpu,
        force_cpu: request.force_cpu,
        keep_model_loaded: Some(true),
        flash_attn: request.flash_attn,
        gpu_device: request.gpu_device,
    };
    let _ = load_or_create_context(app, &load_request, &model_path)?;
    Ok(())
}

#[tauri::command]
pub async fn asr_whisper_transcribe_pcm(
    app: tauri::AppHandle,
    request: AsrWhisperTranscribePcmRequest,
) -> Result<AsrWhisperTranscriptionResponse, String> {
    tauri::async_runtime::spawn_blocking(move || transcribe_pcm_sync(&app, request))
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Whisper task join error: {}", e),
            )
        })?
}

#[tauri::command]
pub async fn asr_whisper_runtime_preload_model(
    app: tauri::AppHandle,
    request: AsrWhisperRuntimeLoadRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || preload_model_sync(&app, request))
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Whisper preload task join error: {}", e),
            )
        })?
}

#[tauri::command]
pub async fn asr_whisper_transcribe_file(
    app: tauri::AppHandle,
    request: AsrWhisperTranscribeRequest,
) -> Result<AsrWhisperTranscriptionResponse, String> {
    tauri::async_runtime::spawn_blocking(move || transcribe_file_sync(&app, request))
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Whisper task join error: {}", e),
            )
        })?
}

#[tauri::command]
pub fn asr_whisper_runtime_clear_cache(app: tauri::AppHandle) -> Result<usize, String> {
    let state = app.state::<WhisperRuntimeState>();
    let mut cache = state.contexts.lock().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Runtime cache lock poisoned: {}", e),
        )
    })?;
    let cleared = cache.len();
    cache.clear();
    Ok(cleared)
}
