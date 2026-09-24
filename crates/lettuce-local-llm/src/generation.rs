//! One local llama.cpp generation, carried over from the legacy request
//! handler: planning (smart offload with its per-model cache, llama.cpp's
//! own fitter behind its gate, multi-GPU distribution, MTP drafter
//! placement), loading, the prompt, the context attempt ladder with the KV
//! cache falling back to RAM (after one reload at the KV-aware layer
//! estimate), the prompt prefix cache, prefill, generation with MTP, stop
//! sequences and streaming, the structured tool-call parse, the runtime
//! report and the metrics.
//!
//! Runs execute on one worker thread that owns the loaded model and the hot
//! context cache, as in legacy.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::Cursor;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use lettuce_inference::thinking::{
    ThinkingTagParser, normalize_thinking_content, normalize_thinking_content_starting_in_reasoning,
};
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::mtmd::{MtmdBitmap, MtmdContext, MtmdInputChunks, MtmdInputText};
use llama_cpp_2::token::LlamaToken;
use serde_json::{Value, json};

use crate::context::{
    ModelShape, align_per_device_vram, combined_effective_vram_bytes, compute_cpu_fallback_limits,
    compute_recommended_context, context_attempt_candidates, context_error_detail,
    is_likely_context_oom_error,
};
use crate::engine::{
    BackendPath, EngineLoadRequest, EngineObserver, LlamaEngine, LlamaEngineError, LlamaGpuConfig,
    NativeFitPlan, emit_model_load_complete, emit_model_load_failed, emit_model_load_finalizing,
    fit_model_params, measure_mmproj_fit_margins, using_rocm_backend,
};
use crate::hardware::{
    get_aligned_per_device_vram, get_available_memory_bytes, get_available_vram_bytes,
    get_per_device_free_vram, list_gpu_devices,
};
use crate::llama::{flash_attention_type, load_model_metadata, shared_backend};
use crate::mtp::{MtpRuntime, discover_external_mtp, model_has_mtp};
use crate::offload::{
    FlashAttentionPolicy, MultiGpuDistribution, OffloadRequest, context_bucket_upper,
    merge_cached_candidate_layers, plan_multi_gpu_distribution, reserve_device_vram,
    select_mtp_gpu_device,
};
use crate::prompt::{
    BuiltPrompt, GEMMA4_REASONING_CLOSE, GEMMA4_REASONING_PREFILL, OpenAICompatPromptOptions,
    PromptRequest, add_bos_label, build_prompt, inject_media_markers,
    model_tokenizer_add_bos_label, model_tokenizer_adds_bos, prepend_reasoning_system_prefix,
    prompt_add_bos_reason, prompt_mode_label, resolve_prompt_add_bos, token_piece_bytes,
};
use crate::request::{
    IncrementalStopMatcher, LlamaGenerationRequest, ResolvedRuntime, ResolvedSampling,
    TextContextShape, cache_eviction_count, common_token_prefix, should_flush_stream,
};
use crate::sampler::{
    ResolvedSamplerConfig, build_sampler, flash_attention_policy_label, kv_type_label,
    offload_kqv_mode_label,
};
use crate::tool_calls::{LocalToolCall, parse_tool_calls, recover_message_from_raw_tool_output};

pub const HOT_CONTEXT_CACHE_MAX_BYTES: usize = 1024 * 1024 * 1024;

/// Events of a run. Deltas and reasoning arrive only for streamed requests.
pub trait GenerationObserver: EngineObserver {
    fn delta(&self, text: &str);
    fn reasoning(&self, text: &str);
    fn tool_calls(&self, calls: &[LocalToolCall]);
    fn heartbeat(&self, heartbeat: GenerationHeartbeat);
    fn notice(&self, notice: LlamaNotice);
    fn runtime_report_updated(&self, model_path: &str);
}

/// The per-model runtime report (legacy `llamaLastRuntimeReport`). `store`
/// returns whether the stored report changed.
pub trait RuntimeReportStore: Send + Sync {
    fn load(&self, model_path: &str) -> Result<Option<Value>, String>;
    fn store(&self, model_path: &str, report: &Value) -> Result<bool, String>;
}

/// What the application gives local generations: the runtime report store,
/// the metrics sink and the events legacy sent to the frontend.
pub trait LlamaHost: RuntimeReportStore {
    fn record_metrics(&self, record: LlamaMetricsRecord);
    fn event(&self, event: LlamaHostEvent);
}

#[derive(Clone, Debug, PartialEq)]
pub enum LlamaHostEvent {
    ModelLoadProgress(crate::engine::ModelLoadProgress),
    GpuFallback,
    Heartbeat {
        request_id: Option<String>,
        heartbeat: GenerationHeartbeat,
    },
    Notice(LlamaNotice),
    RuntimeReportUpdated {
        model_path: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlamaNotice {
    /// MTP was requested but a vision request runs without it.
    MtpDisabledForVision,
    /// The context only fit with the KV cache in RAM (shown once per model).
    KvCacheMovedToRam,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationHeartbeat {
    pub tokens: u64,
    pub elapsed_ms: u64,
    pub tokens_per_second: f64,
    pub recent_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlamaFinishReason {
    Stop,
    Length,
    ToolCalls,
}

impl LlamaFinishReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Length => "length",
            Self::ToolCalls => "tool_calls",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlamaMtpStats {
    pub draft_tokens: u32,
    pub final_draft_tokens: Option<u32>,
    pub adaptation_count: Option<u32>,
    pub rounds: u64,
    pub drafted: u64,
    pub accepted: u64,
    pub tokens_per_round: f64,
    pub draft_acceptance: f64,
}

impl LlamaMtpStats {
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut value = json!({
            "draftTokens": self.draft_tokens,
            "rounds": self.rounds,
            "drafted": self.drafted,
            "accepted": self.accepted,
            "tokensPerRound": self.tokens_per_round,
            "draftAcceptance": self.draft_acceptance,
        });
        if let Some(map) = value.as_object_mut() {
            if let Some(final_draft_tokens) = self.final_draft_tokens {
                map.insert("finalDraftTokens".into(), json!(final_draft_tokens));
            }
            if let Some(adaptation_count) = self.adaptation_count {
                map.insert("adaptationCount".into(), json!(adaptation_count));
            }
        }
        value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlamaUsage {
    pub prompt_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub completion_tokens: u64,
    pub first_token_ms: Option<u64>,
    pub tokens_per_second: Option<f64>,
    pub mtp_stats: Option<LlamaMtpStats>,
}

/// A finished run's metrics row (legacy `llm_metrics`).
#[derive(Clone, Debug, PartialEq)]
pub struct LlamaMetricsRecord {
    pub id: String,
    pub model_path: String,
    pub summary: Value,
    pub samples: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LlamaGenerationOutput {
    pub message: Value,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<LocalToolCall>,
    pub finish_reason: LlamaFinishReason,
    pub usage: LlamaUsage,
    pub metrics: Option<LlamaMetricsRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LlamaGenerationError {
    #[error("llama.cpp request aborted by user")]
    Aborted,
    #[error("{0}")]
    Failed(String),
    #[error("llama.cpp inference worker stopped")]
    WorkerStopped,
}

impl From<LlamaEngineError> for LlamaGenerationError {
    fn from(error: LlamaEngineError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<crate::llama::LlamaRuntimeError> for LlamaGenerationError {
    fn from(error: crate::llama::LlamaRuntimeError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<crate::prompt::PromptError> for LlamaGenerationError {
    fn from(error: crate::prompt::PromptError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<crate::mtp::MtpError> for LlamaGenerationError {
    fn from(error: crate::mtp::MtpError) -> Self {
        Self::Failed(error.to_string())
    }
}

type GenerationDone = Box<dyn FnOnce(Result<LlamaGenerationOutput, LlamaGenerationError>) + Send>;
type UnloadDone = Box<dyn FnOnce(Result<(), LlamaGenerationError>) + Send>;

enum WorkerJob {
    Generate {
        request: Box<LlamaGenerationRequest>,
        observer: Arc<dyn GenerationObserver>,
        reports: Arc<dyn RuntimeReportStore>,
        done: GenerationDone,
    },
    Unload {
        done: UnloadDone,
    },
}

/// The llama.cpp worker. Requests run one at a time on its thread; the
/// completion callback receives the result on that thread.
#[derive(Debug)]
pub struct LlamaRuntime {
    sender: mpsc::Sender<WorkerJob>,
}

impl std::fmt::Debug for WorkerJob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generate { request, .. } => formatter
                .debug_struct("Generate")
                .field("model_path", &request.model_path)
                .finish_non_exhaustive(),
            Self::Unload { .. } => formatter.write_str("Unload"),
        }
    }
}

impl LlamaRuntime {
    pub fn start() -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<WorkerJob>();
        std::thread::Builder::new()
            .name("lettuce-llama".to_string())
            .spawn(move || {
                let mut worker = WorkerState::default();
                while let Ok(job) = receiver.recv() {
                    match job {
                        WorkerJob::Generate {
                            request,
                            observer,
                            reports,
                            done,
                        } => done(generate(
                            &mut worker,
                            &request,
                            observer.as_ref(),
                            reports.as_ref(),
                        )),
                        WorkerJob::Unload { done } => {
                            worker.hot.clear();
                            done(worker.engine.unload().map_err(Into::into));
                        }
                    }
                }
                worker.hot.clear();
            })?;
        Ok(Self { sender })
    }

    pub fn generate(
        &self,
        request: LlamaGenerationRequest,
        observer: Arc<dyn GenerationObserver>,
        reports: Arc<dyn RuntimeReportStore>,
        done: GenerationDone,
    ) {
        if let Err(mpsc::SendError(WorkerJob::Generate { done, .. })) =
            self.sender.send(WorkerJob::Generate {
                request: Box::new(request),
                observer,
                reports,
                done,
            })
        {
            done(Err(LlamaGenerationError::WorkerStopped));
        }
    }

    pub fn unload(&self, done: UnloadDone) {
        if let Err(mpsc::SendError(WorkerJob::Unload { done })) =
            self.sender.send(WorkerJob::Unload { done })
        {
            done(Err(LlamaGenerationError::WorkerStopped));
        }
    }
}

#[derive(Default)]
struct WorkerState {
    engine: LlamaEngine,
    hot: HotContextCache,
}

/// A context kept after a run for prompt-prefix reuse. The contexts borrow
/// the models stored beside them; field order drops both contexts (draft
/// first) before either model.
struct HotTextContext {
    mtp_runtime: Option<MtpRuntime<'static>>,
    context: Option<LlamaContext<'static>>,
    model: Arc<LlamaModel>,
    draft_model: Arc<LlamaModel>,
    model_path: String,
    cache_key: String,
    context_key: String,
    tokens: Vec<LlamaToken>,
    allocated_bytes: usize,
}

type TakenContext = (
    LlamaContext<'static>,
    Option<MtpRuntime<'static>>,
    Vec<LlamaToken>,
);

#[derive(Default)]
struct HotContextCache {
    entries: VecDeque<HotTextContext>,
    allocated_bytes: usize,
}

impl HotContextCache {
    fn clear(&mut self) {
        self.entries.clear();
        self.allocated_bytes = 0;
    }

    fn retain_model(&mut self, model_path: &str) {
        self.entries
            .retain(|cached| cached.model_path == model_path);
        self.allocated_bytes = self
            .entries
            .iter()
            .map(|cached| cached.allocated_bytes)
            .sum();
    }

    fn holds_model(&self, model_path: &str) -> bool {
        self.entries
            .iter()
            .any(|cached| cached.model_path == model_path)
    }

    fn evict_oldest(&mut self, count: usize) {
        for _ in 0..count {
            let Some(cached) = self.entries.pop_front() else {
                break;
            };
            self.allocated_bytes = self.allocated_bytes.saturating_sub(cached.allocated_bytes);
        }
    }

    fn prepare_capacity(&mut self, available_bytes: Option<usize>) -> usize {
        let expected_bytes = self
            .entries
            .iter()
            .map(|cached| cached.allocated_bytes)
            .max()
            .unwrap_or(0);
        let evicted = cache_eviction_count(
            self.allocated_bytes,
            expected_bytes,
            self.entries.iter().map(|cached| cached.allocated_bytes),
            HOT_CONTEXT_CACHE_MAX_BYTES,
            available_bytes,
        );
        self.evict_oldest(evicted);
        evicted
    }

    fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.allocated_bytes)
    }

    fn take(
        &mut self,
        model: &Arc<LlamaModel>,
        draft_model: &Arc<LlamaModel>,
        cache_key: &str,
        context_key: &str,
    ) -> Option<TakenContext> {
        let index = self.entries.iter().position(|cached| {
            cached.cache_key == cache_key
                && cached.context_key == context_key
                && Arc::ptr_eq(&cached.model, model)
                && Arc::ptr_eq(&cached.draft_model, draft_model)
        })?;
        let mut cached = self.entries.remove(index)?;
        self.allocated_bytes = self.allocated_bytes.saturating_sub(cached.allocated_bytes);
        let context = cached.context.take()?;
        let mtp_runtime = cached.mtp_runtime.take();
        Some((context, mtp_runtime, std::mem::take(&mut cached.tokens)))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one cached entry's parts, as legacy stored them"
    )]
    fn store(
        &mut self,
        context: LlamaContext<'_>,
        mtp_runtime: Option<MtpRuntime<'_>>,
        model: Arc<LlamaModel>,
        draft_model: Arc<LlamaModel>,
        model_path: &str,
        cache_key: String,
        context_key: String,
        tokens: Vec<LlamaToken>,
    ) -> usize {
        let mut allocated_bytes = context.allocated_memory_size();
        if let Some(runtime) = mtp_runtime.as_ref() {
            allocated_bytes = allocated_bytes
                .saturating_add(runtime.draft.allocated_memory_size())
                .saturating_add(runtime.carry_hidden.capacity() * std::mem::size_of::<f32>())
                .saturating_add(runtime.h_last.capacity() * std::mem::size_of::<f32>())
                .saturating_add(runtime.pending.capacity() * std::mem::size_of::<LlamaToken>());
        }
        allocated_bytes =
            allocated_bytes.saturating_add(tokens.capacity() * std::mem::size_of::<LlamaToken>());
        if allocated_bytes == 0 || allocated_bytes > HOT_CONTEXT_CACHE_MAX_BYTES {
            return 0;
        }
        let context =
            unsafe { std::mem::transmute::<LlamaContext<'_>, LlamaContext<'static>>(context) };
        let mtp_runtime = mtp_runtime.map(|runtime| unsafe {
            std::mem::transmute::<MtpRuntime<'_>, MtpRuntime<'static>>(runtime)
        });
        if let Some(index) = self
            .entries
            .iter()
            .position(|cached| cached.cache_key == cache_key)
            && let Some(replaced) = self.entries.remove(index)
        {
            self.allocated_bytes = self
                .allocated_bytes
                .saturating_sub(replaced.allocated_bytes);
        }
        let evicted = cache_eviction_count(
            self.allocated_bytes,
            allocated_bytes,
            self.entries.iter().map(|cached| cached.allocated_bytes),
            HOT_CONTEXT_CACHE_MAX_BYTES,
            None,
        );
        self.evict_oldest(evicted);
        self.allocated_bytes = self.allocated_bytes.saturating_add(allocated_bytes);
        self.entries.push_back(HotTextContext {
            mtp_runtime,
            context: Some(context),
            model,
            draft_model,
            model_path: model_path.to_string(),
            cache_key,
            context_key,
            tokens,
            allocated_bytes,
        });
        evicted
    }
}

enum InlineMedia {
    Image(Vec<u8>),
    Audio(Vec<u8>),
}

enum PreparedPrompt {
    Text(Vec<LlamaToken>),
    Vision(MtmdInputChunks),
}

type AttemptGroup = (Option<bool>, Vec<(u32, u32)>);

enum AttemptError {
    RetryAtLayers(u32),
    Failed(LlamaGenerationError),
}

impl<E: Into<LlamaGenerationError>> From<E> for AttemptError {
    fn from(error: E) -> Self {
        Self::Failed(error.into())
    }
}

fn failed(message: impl Into<String>) -> AttemptError {
    AttemptError::Failed(LlamaGenerationError::Failed(message.into()))
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn set_field(report: &mut Value, key: &str, value: Value) {
    if let Some(map) = report.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

fn persist_report(
    observer: &dyn GenerationObserver,
    reports: &dyn RuntimeReportStore,
    model_path: &str,
    report: &Value,
) {
    match reports.store(model_path, report) {
        Ok(true) => observer.runtime_report_updated(model_path),
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to persist llama runtime report");
        }
    }
}

fn parse_data_url(data_url: &str) -> Option<(&str, &str)> {
    let (prefix, data) = data_url.split_once(";base64,")?;
    Some((prefix.strip_prefix("data:")?, data))
}

fn extract_inline_media(messages: &[Value]) -> Result<Vec<InlineMedia>, LlamaGenerationError> {
    let fail = |message: String| LlamaGenerationError::Failed(message);
    let mut media = Vec::new();
    for (message_index, message) in messages.iter().enumerate() {
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for (part_index, part) in parts.iter().enumerate() {
            match part.get("type").and_then(Value::as_str) {
                Some("image_url") => {
                    let image_url = part
                        .get("image_url")
                        .and_then(Value::as_object)
                        .and_then(|object| object.get("url"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if image_url.starts_with("http://") || image_url.starts_with("https://") {
                        return Err(fail(format!(
                            "llama.cpp local vision only supports inline data URLs; message {message_index} part {part_index} used remote URL"
                        )));
                    }
                    let Some((mime_type, data)) = parse_data_url(image_url) else {
                        return Err(fail(format!(
                            "Invalid inline image data URL in message {message_index} part {part_index}"
                        )));
                    };
                    if !mime_type.starts_with("image/") {
                        return Err(fail(format!(
                            "llama.cpp local vision only supports image data URLs; got '{mime_type}' in message {message_index} part {part_index}"
                        )));
                    }
                    let decoded = base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .map_err(|error| {
                            fail(format!(
                                "Failed to decode inline image in message {message_index} part {part_index}: {error}"
                            ))
                        })?;
                    let normalized = if mime_type.eq_ignore_ascii_case("image/png") {
                        decoded
                    } else {
                        let image = image::load_from_memory(&decoded).map_err(|error| {
                            fail(format!(
                                "Failed to decode non-PNG inline image in message {message_index} part {part_index}: {error}"
                            ))
                        })?;
                        let mut png_bytes = Cursor::new(Vec::new());
                        image
                            .write_to(&mut png_bytes, image::ImageFormat::Png)
                            .map_err(|error| {
                                fail(format!(
                                    "Failed to normalize inline image to PNG in message {message_index} part {part_index}: {error}"
                                ))
                            })?;
                        png_bytes.into_inner()
                    };
                    media.push(InlineMedia::Image(normalized));
                }
                Some("input_audio") => {
                    let data = part
                        .get("input_audio")
                        .and_then(Value::as_object)
                        .and_then(|object| object.get("data"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if data.is_empty() {
                        return Err(fail(format!(
                            "Missing inline audio data in message {message_index} part {part_index}"
                        )));
                    }
                    let decoded = base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .map_err(|error| {
                            fail(format!(
                                "Failed to decode inline audio in message {message_index} part {part_index}: {error}"
                            ))
                        })?;
                    media.push(InlineMedia::Audio(decoded));
                }
                _ => {}
            }
        }
    }
    Ok(media)
}

fn decode_mtmd_bitmap(mtmd_ctx: &MtmdContext, bytes: &[u8]) -> Result<MtmdBitmap, String> {
    match MtmdBitmap::from_buffer(mtmd_ctx, bytes) {
        Ok(bitmap) => Ok(bitmap),
        Err(original_error) => {
            let image = image::load_from_memory(bytes).map_err(|decode_error| {
                format!("{original_error} (normalization decode failed: {decode_error})")
            })?;
            let mut normalized = Cursor::new(Vec::new());
            image
                .write_to(&mut normalized, image::ImageFormat::Png)
                .map_err(|encode_error| {
                    format!("{original_error} (PNG normalization failed: {encode_error})")
                })?;
            MtmdBitmap::from_buffer(mtmd_ctx, normalized.get_ref()).map_err(|retry_error| {
                format!("{original_error} (after PNG normalization: {retry_error})")
            })
        }
    }
}

/// The request-side KV cache types (the offload probe accepts fewer).
fn parse_request_kv_type(value: &str) -> Option<KvCacheType> {
    match value {
        "f32" => Some(KvCacheType::F32),
        "f16" => Some(KvCacheType::F16),
        "q8_1" => Some(KvCacheType::Q8_1),
        "q8_0" => Some(KvCacheType::Q8_0),
        "q6_k" => Some(KvCacheType::Q6_K),
        "q5_k" => Some(KvCacheType::Q5_K),
        "q5_1" => Some(KvCacheType::Q5_1),
        "q5_0" => Some(KvCacheType::Q5_0),
        "q4_k" => Some(KvCacheType::Q4_K),
        "q4_1" => Some(KvCacheType::Q4_1),
        "q4_0" => Some(KvCacheType::Q4_0),
        "q3_k" => Some(KvCacheType::Q3_K),
        "q2_k" => Some(KvCacheType::Q2_K),
        "iq4_nl" => Some(KvCacheType::IQ4_NL),
        "iq3_s" => Some(KvCacheType::IQ3_S),
        "iq3_xxs" => Some(KvCacheType::IQ3_XXS),
        "iq2_xs" => Some(KvCacheType::IQ2_XS),
        "iq2_xxs" => Some(KvCacheType::IQ2_XXS),
        "iq1_s" => Some(KvCacheType::IQ1_S),
        _ => None,
    }
}

fn model_shape(model: &LlamaModel) -> ModelShape {
    ModelShape {
        n_layer: model.n_layer(),
        n_layer_nextn: model.n_layer_nextn(),
        n_embd: model.n_embd(),
        n_head: model.n_head(),
        n_head_kv: model.n_head_kv(),
        size: model.size(),
    }
}

fn backend_path_from_label(label: Option<&str>) -> Option<BackendPath> {
    match label {
        Some("gpu_offload") => Some(BackendPath::GpuOffload),
        Some("cpu") => Some(BackendPath::Cpu),
        _ => None,
    }
}

struct Run<'r> {
    request: &'r LlamaGenerationRequest,
    model_path: &'r str,
    observer: &'r dyn GenerationObserver,
    reports: &'r dyn RuntimeReportStore,
    options: OpenAICompatPromptOptions,
    sampling: ResolvedSampling,
    runtime: ResolvedRuntime,
    requested_context: Option<u32>,
    max_tokens: u32,
    batch_size_limit: u32,
    prompt_cache_key: Option<String>,
    media: Vec<InlineMedia>,
    image_count: usize,
    audio_count: usize,
    prompt_messages: Cow<'r, [Value]>,
    output: String,
    prompt_tokens: u64,
    cached_prompt_tokens: u64,
    prompt_cache_hit: bool,
    prompt_cache_evictions: usize,
    completion_tokens: u64,
    started_at: Instant,
    first_token_ms: Option<u64>,
    generation_elapsed_ms: Option<u64>,
    generation_elapsed_seconds: Option<f64>,
    native_prompt_eval_ms: Option<f64>,
    native_prompt_eval_tokens: Option<u64>,
    native_prompt_eval_tps: Option<f64>,
    native_draft_prompt_eval_ms: Option<f64>,
    native_generation_compute_ms: Option<f64>,
    native_generation_tps: Option<f64>,
    app_generation_overhead_ms: Option<f64>,
    metric_samples: Vec<Value>,
    finish_reason: LlamaFinishReason,
    final_message: Value,
    tool_calls: Vec<LocalToolCall>,
    failure_stage: &'static str,
    mtp_stats: Option<LlamaMtpStats>,
    report: Value,
}

fn generate(
    worker: &mut WorkerState,
    request: &LlamaGenerationRequest,
    observer: &dyn GenerationObserver,
    reports: &dyn RuntimeReportStore,
) -> Result<LlamaGenerationOutput, LlamaGenerationError> {
    let model_path = request.model_path.as_str();
    if !Path::new(model_path).exists() {
        return Err(LlamaGenerationError::Failed(format!(
            "llama.cpp model path not found: {model_path}"
        )));
    }
    let thinking = request.resolve_thinking();
    let options = OpenAICompatPromptOptions {
        enable_thinking: thinking.enable_thinking,
        chat_template_kwargs: thinking.chat_template_kwargs.clone(),
        parallel_tool_calls: thinking.parallel_tool_calls,
        reasoning_format: thinking.reasoning_format.clone(),
    };
    if let Some(enabled) = thinking.thinking_directive {
        tracing::info!(
            enable_thinking = enabled,
            "local thinking mode overridden by trailing message directive"
        );
    }
    let runtime = request.resolve_runtime();
    let media = extract_inline_media(&request.messages)?;
    let image_count = media
        .iter()
        .filter(|item| matches!(item, InlineMedia::Image(_)))
        .count();
    let audio_count = media.len() - image_count;
    if !media.is_empty() && runtime.mmproj_path.is_none() {
        return Err(LlamaGenerationError::Failed(
            "llama.cpp multimodal requests require `llamaMmprojPath` (or `llama_mmproj_path`) to load the multimodal projector".to_string(),
        ));
    }
    let mut prompt_messages = if media.is_empty() {
        Cow::Borrowed(request.messages.as_slice())
    } else {
        Cow::Owned(inject_media_markers(&request.messages))
    };
    if request.reasoning.force_gemma4_reasoning {
        prompt_messages = Cow::Owned(prepend_reasoning_system_prefix(&prompt_messages));
    }
    let requested_context = request.requested_context();
    let max_tokens = request.max_tokens();
    tracing::info!(
        model_path,
        stream = request.stream,
        request_id = ?request.request_id,
        "local inference start"
    );
    let report = json!({
        "updatedAt": timestamp_ms(),
        "modelPath": model_path,
        "requestedContext": requested_context,
        "requestedBatchLimit": runtime.batch_size,
        "requestedUbatchLimit": runtime.ubatch_size,
        "requestedGpuLayers": runtime.gpu_layers,
        "targetNewTokens": max_tokens,
        "thinkingEnabled": options.enable_thinking,
        "thinkingDirective": thinking
            .thinking_directive
            .map(|enabled| if enabled { "/think" } else { "/no_think" }),
    });
    let batch_size_limit = runtime.batch_size;
    let mut run = Run {
        request,
        model_path,
        observer,
        reports,
        options,
        sampling: request.resolve_sampling(),
        runtime,
        requested_context,
        max_tokens,
        batch_size_limit,
        prompt_cache_key: request.prompt_cache_key(),
        media,
        image_count,
        audio_count,
        prompt_messages,
        output: String::new(),
        prompt_tokens: 0,
        cached_prompt_tokens: 0,
        prompt_cache_hit: false,
        prompt_cache_evictions: 0,
        completion_tokens: 0,
        started_at: Instant::now(),
        first_token_ms: None,
        generation_elapsed_ms: None,
        generation_elapsed_seconds: None,
        native_prompt_eval_ms: None,
        native_prompt_eval_tokens: None,
        native_prompt_eval_tps: None,
        native_draft_prompt_eval_ms: None,
        native_generation_compute_ms: None,
        native_generation_tps: None,
        app_generation_overhead_ms: None,
        metric_samples: Vec::new(),
        finish_reason: LlamaFinishReason::Stop,
        final_message: json!({ "role": "assistant", "content": "" }),
        tool_calls: Vec::new(),
        failure_stage: "load_engine",
        mtp_stats: None,
        report,
    };

    let mut forced_smart_gpu_layers = None;
    let result = loop {
        match run.attempt(worker, forced_smart_gpu_layers) {
            Err(AttemptError::RetryAtLayers(layers)) if forced_smart_gpu_layers.is_none() => {
                forced_smart_gpu_layers = Some(layers);
            }
            Err(AttemptError::RetryAtLayers(layers)) => {
                break Err(LlamaGenerationError::Failed(format!(
                    "llama.cpp KV-aware retry requested again at {layers} layers"
                )));
            }
            Err(AttemptError::Failed(error)) => break Err(error),
            Ok(()) => break Ok(()),
        }
    };

    if let Err(error) = result {
        run.finish_failed(&error);
        return Err(error);
    }
    Ok(run.finish_succeeded(worker))
}

impl Run<'_> {
    fn check_abort(&self) -> Result<(), AttemptError> {
        if self.request.is_cancelled() {
            return Err(AttemptError::Failed(LlamaGenerationError::Aborted));
        }
        Ok(())
    }

    fn structured_failure(
        &self,
        built_prompt: &BuiltPrompt,
        stage: &str,
        error: impl std::fmt::Display,
    ) -> AttemptError {
        tracing::warn!(
            stage,
            model_path = self.model_path,
            %error,
            structured = %self.structured_debug_payload(built_prompt),
            "local structured output failed cleanly"
        );
        failed(format!(
            "Local llama structured output failed during {stage}: {error}"
        ))
    }

    fn structured_debug_payload(&self, built_prompt: &BuiltPrompt) -> Value {
        let template_result = built_prompt.chat_template_result.as_ref();
        json!({
            "requestId": self.request.request_id,
            "modelPath": self.model_path,
            "templateSource": built_prompt
                .applied_template_source
                .clone()
                .or_else(|| built_prompt.attempted_template_source.clone()),
            "requestedToolChoice": self.request.tool_choice,
            "resolvedToolChoice": built_prompt.resolved_tool_choice,
            "reasoningFormat": self.options.reasoning_format,
            "parallelToolCalls": self.options.parallel_tool_calls,
            "enableThinking": self.options.enable_thinking,
            "hasGrammar": template_result.and_then(|result| result.grammar.as_ref()).is_some(),
            "grammarLazy": template_result.map(|result| result.grammar_lazy),
            "grammarTriggerCount": template_result.map(|result| result.grammar_triggers.len()),
            "preservedTokenCount": template_result.map(|result| result.preserved_tokens.len()),
            "additionalStopCount": template_result.map(|result| result.additional_stops.len()),
        })
    }

    fn emit_content(&self, thinking: &mut ThinkingTagParser, text: &str) -> String {
        let split = thinking.feed(text);
        if !split.content.is_empty() {
            self.observer.delta(&split.content);
        }
        if !split.reasoning.is_empty() {
            self.observer.reasoning(&split.reasoning);
        }
        split.content
    }

    fn emit_structured_deltas(
        &self,
        deltas: Vec<String>,
        thinking: &mut ThinkingTagParser,
        streamed_text: &mut String,
    ) -> Result<(), AttemptError> {
        for delta_json in deltas {
            let delta: Value = serde_json::from_str(&delta_json).map_err(|error| {
                failed(format!(
                    "Failed to parse llama.cpp structured delta: {error}"
                ))
            })?;
            if let Some(text) = delta
                .get("content")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                let content = self.emit_content(thinking, text);
                streamed_text.push_str(&content);
            }
            if let Some(reasoning) = delta
                .get("reasoning")
                .or_else(|| delta.get("reasoning_content"))
                .or_else(|| delta.get("thinking"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                self.observer.reasoning(reasoning);
            }
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the legacy request pipeline, kept in its order"
    )]
    fn attempt(
        &mut self,
        worker: &mut WorkerState,
        forced_smart_gpu_layers: Option<u32>,
    ) -> Result<(), AttemptError> {
        self.check_abort()?;
        self.failure_stage = "load_engine";
        self.cached_prompt_tokens = 0;
        self.prompt_cache_hit = false;
        self.prompt_cache_evictions = 0;
        self.native_prompt_eval_ms = None;
        self.native_prompt_eval_tokens = None;
        self.native_prompt_eval_tps = None;
        self.native_draft_prompt_eval_ms = None;
        self.native_generation_compute_ms = None;
        self.native_generation_tps = None;
        self.app_generation_overhead_ms = None;
        if forced_smart_gpu_layers.is_some() {
            for field in [
                "actualGpuLayersUsed",
                "backendPathUsed",
                "gpuLoadFallbackActivated",
                "gpuFallbackReason",
                "smartGpuLayerFallbackActivated",
                "smartOffloadCacheHit",
                "smartOffloadCachedGpuLayers",
            ] {
                set_field(&mut self.report, field, Value::Null);
            }
        }
        let model_path = self.model_path;
        let media_requested = !self.media.is_empty();
        let rt = self.runtime.clone();
        if forced_smart_gpu_layers.is_some() {
            worker.hot.clear();
        } else {
            worker.hot.retain_model(model_path);
        }
        if media_requested {
            worker.hot.clear();
        }
        let hot_context_resident = worker.hot.holds_model(model_path);
        if worker.engine.unload_if_model_differs(model_path)? {
            tracing::info!("unloaded previous llama.cpp model before planning (model changed)");
        } else if forced_smart_gpu_layers.is_some() {
            worker.engine.unload()?;
        }
        let multi_gpu_active = rt.multi_gpu_enabled
            && rt.gpu_device_ids.len() >= 2
            && rt.single_gpu_device_id.is_none();
        let native_fit_request = rt.gpu_layers.is_none()
            && forced_smart_gpu_layers.is_none()
            && !multi_gpu_active
            && !rt.mtp_enabled
            && self.requested_context.is_some();
        if native_fit_request && !hot_context_resident {
            worker.engine.unload()?;
        }
        let mut kv_main_gpu: Option<i32> = None;
        let kv_placement_offload_kqv: Option<bool> = if multi_gpu_active {
            match rt.kv_placement.as_deref() {
                Some("split") => Some(true),
                Some("systemRam") => Some(false),
                Some("pin") => {
                    kv_main_gpu = rt.main_gpu.and_then(|id| {
                        usize::try_from(id).ok().and_then(|id| {
                            rt.gpu_device_ids
                                .iter()
                                .position(|device| *device == id)
                                .map(|position| position as i32)
                        })
                    });
                    Some(true)
                }
                _ => None,
            }
        } else {
            None
        };
        let resolved_offload_kqv = if let Some(placement) = kv_placement_offload_kqv {
            Some(placement)
        } else if rt.offload_kqv.is_some() {
            rt.offload_kqv
        } else if rt.mtp_enabled && !media_requested || using_rocm_backend() {
            Some(false)
        } else {
            None
        };
        let flash_policy = if let Some(policy) = rt.flash_attention {
            policy
        } else if using_rocm_backend() {
            FlashAttentionPolicy::Disabled
        } else {
            FlashAttentionPolicy::Auto
        };
        let available_memory_bytes = get_available_memory_bytes();
        let distribution_mode = rt
            .gpu_distribution_mode
            .clone()
            .unwrap_or_else(|| "balanced".to_string());
        let manual_distribution = multi_gpu_active && distribution_mode == "manual";
        let per_device_vram_raw = if multi_gpu_active {
            get_per_device_free_vram(&rt.gpu_device_ids, false)
        } else {
            Vec::new()
        };
        let per_device_vram: Vec<(usize, u64, u64)> = if multi_gpu_active {
            align_per_device_vram(&rt.gpu_device_ids, &per_device_vram_raw)
        } else {
            Vec::new()
        };
        for (id, free, total) in &per_device_vram {
            if !per_device_vram_raw
                .iter()
                .any(|(device, _, _)| device == id)
            {
                tracing::warn!(
                    device = id,
                    imputed_bytes = (*free).max(*total),
                    "multi-gpu vram query: device not in ggml query results; imputing capacity"
                );
            }
        }
        let device_free_aligned: Vec<u64> =
            per_device_vram.iter().map(|(_, free, _)| *free).collect();
        let available_vram_bytes = if multi_gpu_active {
            combined_effective_vram_bytes(&per_device_vram).or_else(get_available_vram_bytes)
        } else if let Some(device_id) = rt.single_gpu_device_id {
            get_aligned_per_device_vram(&[device_id])
                .first()
                .map(|(_, free, _)| *free)
                .filter(|free| *free > 0)
                .or_else(get_available_vram_bytes)
        } else {
            get_available_vram_bytes()
        };
        let manual_layers_aligned: Vec<u32> = if manual_distribution {
            rt.gpu_device_ids
                .iter()
                .map(|id| {
                    rt.gpu_manual_layers
                        .iter()
                        .find(|(device, _)| device == id)
                        .map_or(0, |(_, layers)| *layers)
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut multi_gpu_distribution: Option<MultiGpuDistribution> = None;
        let mut effective_gpu_layers = rt.gpu_layers;
        let mut smart_gpu_layer_candidates: Option<Vec<u32>> = None;
        let mut smart_kv_aware_layer_estimate: Option<u32> = None;
        let mut native_fit_plan: Option<NativeFitPlan> = None;
        let cached_runtime_report = self.reports.load(model_path).ok().flatten();

        let backend_supports_gpu_offload = shared_backend()?.supports_gpu_offload();
        if backend_supports_gpu_offload
            && !manual_distribution
            && let Some(requested) = rt.gpu_layers
            && let Ok(metadata) = load_model_metadata(model_path)
        {
            let normalized = metadata.normalize_requested_gpu_layers(requested);
            if normalized != requested {
                tracing::info!(
                    requested,
                    normalized,
                    "normalized requested GPU layers to include the output layer"
                );
            }
            effective_gpu_layers = Some(normalized);
        }
        let active_mmproj_path = rt.mmproj_path.as_deref().filter(|_| media_requested);
        let mtp_bundled = rt.mtp_enabled && !media_requested && model_has_mtp(model_path);
        let mtp_external_path = if rt.mtp_enabled && !media_requested && !mtp_bundled {
            rt.mtp_model_path
                .clone()
                .or_else(|| discover_external_mtp(model_path))
        } else {
            None
        };
        if let Some(external) = mtp_external_path.as_deref() {
            tracing::info!(external, "MTP external draft model resolved");
        }
        let planned_mtp_context = self.requested_context.unwrap_or(16_384).max(1);
        let mtp_gpu_reserve_bytes = mtp_external_path
            .as_deref()
            .map(|path| {
                crate::llama::estimate_mtp_gpu_reserve_bytes(
                    path,
                    planned_mtp_context,
                    rt.compute_batch_size,
                    rt.kv_types(),
                )
            })
            .transpose()?
            .unwrap_or(0);
        let mtp_drafter_on_gpu = if !backend_supports_gpu_offload || mtp_external_path.is_none() {
            false
        } else {
            match rt.mtp_placement.as_str() {
                "cpu" => false,
                "gpu" => true,
                _ => available_vram_bytes
                    .is_some_and(|bytes| mtp_gpu_reserve_bytes < bytes.saturating_mul(9) / 10),
            }
        };
        let mtp_gpu_device_id = if !mtp_drafter_on_gpu {
            None
        } else if multi_gpu_active {
            select_mtp_gpu_device(&rt.gpu_device_ids, &device_free_aligned)
        } else {
            rt.single_gpu_device_id
        };
        let device_free_for_distribution = reserve_device_vram(
            &rt.gpu_device_ids,
            &device_free_aligned,
            mtp_gpu_device_id,
            mtp_gpu_reserve_bytes,
        );
        let sidecar_vram_reserve_bytes = if backend_supports_gpu_offload {
            let mmproj_reserve = active_mmproj_path
                .and_then(|path| std::fs::metadata(path).ok())
                .map_or(0, |meta| meta.len());
            let mtp_reserve = if mtp_drafter_on_gpu {
                mtp_gpu_reserve_bytes
            } else {
                0
            };
            mmproj_reserve.saturating_add(mtp_reserve)
        } else {
            0
        };
        let smart_offload_planning_config = json!({
            "multiGpu": multi_gpu_active,
            "deviceIds": rt.gpu_device_ids,
            "singleDeviceId": rt.single_gpu_device_id,
            "distributionMode": distribution_mode,
            "manualLayers": manual_layers_aligned,
            "mainGpu": rt.main_gpu,
            "priorityVramLimitBytes": rt.priority_vram_limit_bytes,
            "kvPlacement": rt.kv_placement,
            "offloadKqv": resolved_offload_kqv,
            "kvType": planning_kv_type(&rt),
            "swaFull": rt.swa_full,
            "context": self.requested_context,
            "batch": self.batch_size_limit,
            "ubatch": rt.ubatch_size,
            "computeBatch": rt.compute_batch_size,
            "flashAttention": flash_attention_type(flash_policy),
            "mmprojPath": active_mmproj_path,
            "mtpEnabled": rt.mtp_enabled,
            "mtpPlacement": rt.mtp_placement,
            "mtpModelPath": mtp_external_path,
            "mtpDraftTokens": rt.mtp_draft_tokens,
            "mtpGpuDeviceId": mtp_gpu_device_id,
            "sidecarVramReserveBytes": sidecar_vram_reserve_bytes,
        })
        .to_string();

        if manual_distribution && backend_supports_gpu_offload {
            let metadata = load_model_metadata(model_path).ok();
            let manual_total_layers = metadata.map_or_else(
                || manual_layers_aligned.iter().copied().sum(),
                |metadata| metadata.offload_layer_count(),
            );
            tracing::info!(
                devices = ?rt.gpu_device_ids,
                manual_layers = ?manual_layers_aligned,
                "multi-gpu manual distribution"
            );
            if manual_layers_aligned.iter().all(|layers| *layers == 0) {
                tracing::warn!(
                    "multi-gpu manual distribution: all layer counts are zero; llamaGpuManualLayers may be missing or device IDs may not match; falling through to CPU"
                );
            }
            if let Some(metadata) = metadata {
                let gib = 1024.0 * 1024.0 * 1024.0;
                let bytes_per_layer = metadata
                    .model_size_bytes
                    .checked_div(u64::from(metadata.model_layer_count()))
                    .unwrap_or(0);
                for (position, layers) in manual_layers_aligned.iter().enumerate() {
                    let projected = bytes_per_layer.saturating_mul(u64::from(*layers));
                    let capacity = per_device_vram
                        .get(position)
                        .map_or(0, |(_, free, total)| (*free).max(*total));
                    if capacity > 0 && projected > capacity {
                        tracing::warn!(
                            layers,
                            projected_gib = projected as f64 / gib,
                            device = rt.gpu_device_ids.get(position).copied().unwrap_or(position),
                            capacity_gib = capacity as f64 / gib,
                            "multi-gpu manual distribution puts more weights on a device than it reports; the load will likely fail and fall back to CPU"
                        );
                    }
                }
            }
            let distribution = plan_multi_gpu_distribution(
                "manual",
                &device_free_aligned,
                manual_total_layers,
                0,
                0,
                0,
                Some(&manual_layers_aligned),
                None,
                None,
            );
            effective_gpu_layers = Some(distribution.n_gpu_layers);
            smart_gpu_layer_candidates = None;
            multi_gpu_distribution = Some(distribution);
        } else if rt.gpu_layers.is_none() && !rt.strict_mode && backend_supports_gpu_offload {
            let mut plan = crate::llama::plan_smart_gpu_offload(
                model_path,
                OffloadRequest {
                    available_memory_bytes,
                    available_vram_bytes,
                    requested_context: self.requested_context,
                    n_batch: rt.compute_batch_size,
                    resolved_offload_kqv,
                    kv_types: rt.kv_types(),
                    flash_attention_policy: flash_policy,
                    sidecar_vram_reserve_bytes,
                    bundled_mtp_draft: mtp_bundled,
                },
            )?;
            smart_kv_aware_layer_estimate = Some(plan.estimated_gpu_layers);
            let current_context_bucket = context_bucket_upper(plan.planned_context.max(1));
            if let Some(report) = cached_runtime_report.as_ref() {
                let report_u32 = |key: &str| {
                    report
                        .get(key)
                        .and_then(Value::as_u64)
                        .and_then(|value| u32::try_from(value).ok())
                };
                let cached_gpu_layers =
                    report_u32("actualGpuLayersUsed").filter(|value| *value > 0);
                let cached_backend_path = report.get("backendPathUsed").and_then(Value::as_str);
                let cached_status = report.get("status").and_then(Value::as_str);
                let cached_context_bucket = report_u32("smartOffloadPlannedContext")
                    .or_else(|| report_u32("requestedContext"))
                    .or_else(|| report_u32("actualContextUsed"))
                    .map(context_bucket_upper);
                if let (Some(cached_layers), Some(bucket)) =
                    (cached_gpu_layers, cached_context_bucket)
                {
                    let planning_config_matches = report
                        .get("smartOffloadPlanningConfig")
                        .and_then(Value::as_str)
                        == Some(smart_offload_planning_config.as_str());
                    let total_layers_match =
                        report_u32("smartOffloadTotalLayers") == Some(plan.total_layers);
                    let context_bucket_matches =
                        hot_context_resident || bucket == current_context_bucket;
                    let cached_kqv_fallback = report
                        .get("kqvFallbackActivated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let cached_vram_budget =
                        report.get("availableVramBytes").and_then(Value::as_u64);
                    let vram_budget_matches = hot_context_resident
                        || match (cached_vram_budget, available_vram_bytes) {
                            (Some(cached), Some(current)) => {
                                cached.abs_diff(current) <= current / 20
                            }
                            _ => true,
                        };
                    if !planning_config_matches
                        || !total_layers_match
                        || cached_kqv_fallback
                        || !vram_budget_matches
                    {
                        tracing::info!(
                            planning_config_matches,
                            total_layers_match,
                            cached_kqv_fallback,
                            vram_budget_matches,
                            "smart gpu offload cache invalidated"
                        );
                    }
                    if cached_status == Some("succeeded")
                        && cached_backend_path == Some("gpu_offload")
                        && context_bucket_matches
                        && planning_config_matches
                        && total_layers_match
                        && !cached_kqv_fallback
                        && vram_budget_matches
                    {
                        let merged = merge_cached_candidate_layers(
                            plan.total_layers,
                            cached_layers,
                            &plan.candidate_gpu_layers,
                        );
                        tracing::info!(
                            context_bucket = current_context_bucket,
                            cached_layers,
                            merged_candidates = ?merged,
                            "smart gpu offload cache hit"
                        );
                        set_field(&mut self.report, "smartOffloadCacheHit", json!(true));
                        set_field(
                            &mut self.report,
                            "smartOffloadCachedGpuLayers",
                            json!(cached_layers),
                        );
                        plan.candidate_gpu_layers = merged;
                        plan.estimated_gpu_layers = cached_layers;
                    }
                }
            }
            effective_gpu_layers = plan.candidate_gpu_layers.first().copied();
            smart_gpu_layer_candidates = Some(plan.candidate_gpu_layers.clone());
            if multi_gpu_active {
                multi_gpu_distribution = Some(plan_multi_gpu_distribution(
                    &distribution_mode,
                    &device_free_for_distribution,
                    plan.total_layers,
                    plan.bytes_per_layer,
                    plan.kv_bytes_per_layer,
                    plan.estimated_gpu_layers,
                    None,
                    rt.priority_vram_limit_bytes,
                    Some(&plan.offload_unit_costs),
                ));
            }
            for (key, value) in [
                ("smartOffloadTotalLayers", json!(plan.total_layers)),
                ("smartOffloadPlannedContext", json!(plan.planned_context)),
                (
                    "smartOffloadRecommendedContext",
                    json!(plan.recommended_context),
                ),
                (
                    "smartOffloadEstimatedGpuLayers",
                    json!(plan.estimated_gpu_layers),
                ),
                (
                    "smartOffloadCandidateLayers",
                    json!(plan.candidate_gpu_layers),
                ),
                ("smartOffloadKqvVramReserved", json!(plan.kqv_vram_reserved)),
                (
                    "smartOffloadPlanningKqvMode",
                    json!(plan.planning_offload_kqv),
                ),
                (
                    "smartOffloadEstimatedKvBytes",
                    json!(plan.estimated_kv_bytes),
                ),
                (
                    "smartOffloadSidecarVramReserveBytes",
                    json!(plan.estimated_sidecar_vram_reserve_bytes),
                ),
                (
                    "smartOffloadRuntimeReserveBytes",
                    json!(plan.estimated_runtime_reserve_bytes),
                ),
                (
                    "smartOffloadEffectiveVramBudgetBytes",
                    json!(plan.effective_vram_budget_bytes),
                ),
                ("mtpPlacementRequested", json!(rt.mtp_placement)),
                (
                    "mtpPlacementResolved",
                    json!(if mtp_drafter_on_gpu { "gpu" } else { "cpu" }),
                ),
                ("mtpGpuReserveBytes", json!(mtp_gpu_reserve_bytes)),
                ("mtpGpuDeviceId", json!(mtp_gpu_device_id)),
            ] {
                set_field(&mut self.report, key, value);
            }
            tracing::info!(
                total_layers = plan.total_layers,
                planned_ctx = plan.planned_context,
                estimated_gpu_layers = plan.estimated_gpu_layers,
                candidates = ?plan.candidate_gpu_layers,
                planning_offload_kqv = ?plan.planning_offload_kqv,
                reserve_kqv_vram = plan.kqv_vram_reserved,
                kv_bytes = plan.estimated_kv_bytes,
                sidecar_vram_reserve_bytes = plan.estimated_sidecar_vram_reserve_bytes,
                runtime_reserve_bytes = plan.estimated_runtime_reserve_bytes,
                effective_vram_budget_bytes = plan.effective_vram_budget_bytes,
                "smart gpu offload plan"
            );
        } else if rt.gpu_layers.is_none() && !rt.strict_mode {
            tracing::info!(
                "skipping smart gpu offload planning because this backend has no GPU offload support"
            );
        }

        let kv_types = rt.kv_types();
        let kv_label = kv_types.label();
        let k_type = kv_types.k.and_then(parse_request_kv_type);
        let v_type = kv_types.v.and_then(parse_request_kv_type);
        let native_fit_eligible =
            native_fit_request && backend_supports_gpu_offload && !hot_context_resident;
        if native_fit_eligible {
            if let Some(fit_context) = self.requested_context {
                let fit_batch = fit_context.min(self.batch_size_limit).max(1);
                let mut fit_params = LlamaContextParams::default()
                    .with_n_ctx(NonZeroU32::new(fit_context))
                    .with_n_batch(fit_batch)
                    .with_n_outputs_max(1)
                    .with_flash_attention_policy(flash_attention_type(flash_policy));
                if let Some(n_ubatch) = rt.ubatch_size {
                    fit_params = fit_params.with_n_ubatch(n_ubatch.min(fit_batch));
                }
                fit_params =
                    apply_common_params(fit_params, &rt, resolved_offload_kqv, k_type, v_type);
                let fit_devices = rt
                    .single_gpu_device_id
                    .map(|device_id| vec![device_id])
                    .unwrap_or_default();
                let fit_margins = measure_mmproj_fit_margins(active_mmproj_path, &fit_devices)?;
                match fit_model_params(
                    model_path,
                    &fit_devices,
                    fit_params,
                    &fit_margins,
                    fit_context,
                    mtp_bundled,
                ) {
                    Ok(plan) => {
                        let mut candidates = vec![plan.n_gpu_layers];
                        if let Some(fallbacks) = smart_gpu_layer_candidates.take() {
                            candidates.extend(
                                fallbacks
                                    .into_iter()
                                    .filter(|layers| *layers != plan.n_gpu_layers),
                            );
                        }
                        effective_gpu_layers = Some(plan.n_gpu_layers);
                        smart_gpu_layer_candidates = Some(candidates.clone());
                        for (key, value) in [
                            ("nativeFitApplied", json!(true)),
                            ("nativeFitContext", json!(plan.n_ctx)),
                            ("nativeFitGpuLayers", json!(plan.n_gpu_layers)),
                            (
                                "nativeFitMarginBytes",
                                json!(fit_margins.iter().sum::<usize>()),
                            ),
                            ("nativeFitTensorSplit", json!(plan.tensor_split)),
                        ] {
                            set_field(&mut self.report, key, value);
                        }
                        tracing::info!(
                            context = plan.n_ctx,
                            gpu_layers = plan.n_gpu_layers,
                            candidates = ?candidates,
                            "llama.cpp native fit selected"
                        );
                        native_fit_plan = Some(plan);
                    }
                    Err(error) => {
                        set_field(&mut self.report, "nativeFitApplied", json!(false));
                        set_field(&mut self.report, "nativeFitError", json!(error.to_string()));
                        tracing::warn!(
                            %error,
                            "llama.cpp native fit unavailable; using smart offloader candidates"
                        );
                    }
                }
            }
        } else if native_fit_request && hot_context_resident {
            if let Some(report) = cached_runtime_report.as_ref() {
                for field in [
                    "nativeFitApplied",
                    "nativeFitContext",
                    "nativeFitGpuLayers",
                    "nativeFitMarginBytes",
                    "nativeFitTensorSplit",
                    "nativeFitError",
                ] {
                    if let Some(value) = report.get(field) {
                        set_field(&mut self.report, field, value.clone());
                    }
                }
            }
            tracing::info!(
                "reusing resident llama.cpp model and context without refitting parameters"
            );
        }

        self.check_abort()?;
        let metadata_layers = || load_model_metadata(model_path).ok();
        if multi_gpu_active && multi_gpu_distribution.is_none() {
            let max_gpu_layers =
                metadata_layers().map_or(u32::MAX, |metadata| metadata.offload_layer_count());
            multi_gpu_distribution = Some(plan_multi_gpu_distribution(
                &distribution_mode,
                &device_free_aligned,
                max_gpu_layers,
                0,
                0,
                effective_gpu_layers.unwrap_or(0),
                None,
                rt.priority_vram_limit_bytes,
                None,
            ));
        }
        let multi_gpu_tensor_split = multi_gpu_distribution
            .as_ref()
            .map(|distribution| distribution.tensor_split.clone())
            .unwrap_or_default();
        let multi_gpu_main_gpu = kv_main_gpu.or_else(|| {
            multi_gpu_distribution
                .as_ref()
                .and_then(|distribution| distribution.main_gpu)
        });
        if let Some(device_id) = rt.single_gpu_device_id {
            tracing::info!(device = device_id, "single-gpu override active");
        }
        if let Some(forced) = forced_smart_gpu_layers {
            tracing::warn!(
                layers = forced,
                "retrying model load at KV-aware layer estimate after GPU KV context OOM"
            );
            effective_gpu_layers = Some(forced);
            smart_gpu_layer_candidates = None;
        }
        tracing::info!("loading llama.cpp engine/model");
        let gpu_config = LlamaGpuConfig {
            multi_gpu_enabled: multi_gpu_active,
            device_ids: if multi_gpu_active {
                rt.gpu_device_ids.clone()
            } else {
                rt.single_gpu_device_id
                    .map(|device_id| vec![device_id])
                    .unwrap_or_default()
            },
            device_labels: if multi_gpu_active {
                let known_devices = list_gpu_devices();
                rt.gpu_device_ids
                    .iter()
                    .map(|id| {
                        known_devices
                            .iter()
                            .find(|device| device.index == *id)
                            .map_or_else(
                                || format!("GPU {id}"),
                                |device| {
                                    if device.description.trim().is_empty() {
                                        device.name.clone()
                                    } else {
                                        device.description.clone()
                                    }
                                },
                            )
                    })
                    .collect()
            } else {
                Vec::new()
            },
            tensor_split: if multi_gpu_active {
                multi_gpu_tensor_split
            } else {
                Vec::new()
            },
            main_gpu: multi_gpu_main_gpu.filter(|_| multi_gpu_active),
            distribution_mode: multi_gpu_active.then(|| distribution_mode.clone()),
            total_layer_count: if multi_gpu_active {
                metadata_layers().map(|metadata| metadata.model_layer_count())
            } else {
                None
            },
        };
        let request_id = self.request.request_id.as_deref();
        let hot = &mut worker.hot;
        let engine = worker.engine.load(
            Some(self.observer),
            &EngineLoadRequest {
                request_id,
                model_path,
                requested_gpu_layers: effective_gpu_layers,
                auto_gpu_layer_candidates: smart_gpu_layer_candidates.as_deref(),
                native_fit_plan: native_fit_plan.as_ref(),
                gpu_config,
                strict_mode: rt.strict_mode,
                mmproj_path: active_mmproj_path,
                load_bundled_mtp: mtp_bundled,
                mtp_model_path: mtp_external_path.as_deref(),
                mtp_drafter_on_gpu,
                mtp_gpu_fallback_allowed: rt.mtp_placement == "auto",
                mtp_gpu_device_id,
            },
            || hot.clear(),
        )?;
        let mtp_draft_model = engine.mtp_model.clone();
        let model = engine.model.as_ref();
        let backend = engine.backend.as_ref();
        let mtmd_ctx = engine.mtmd_ctx.as_deref();
        if media_requested && mtmd_ctx.is_none() {
            return Err(failed(
                "llama.cpp multimodal request could not initialize the multimodal projector context",
            ));
        }
        if let Some(mtmd_ctx) = mtmd_ctx {
            if self.image_count > 0 && !mtmd_ctx.support_vision() {
                return Err(failed(
                    "The loaded llama.cpp mmproj/model pair does not support vision input",
                ));
            }
            if self.audio_count > 0 && !mtmd_ctx.support_audio() {
                return Err(failed(
                    "The loaded llama.cpp mmproj/model pair does not support audio input",
                ));
            }
        }
        let use_vision = media_requested && mtmd_ctx.is_some();
        let mtp_active = rt.mtp_enabled
            && !use_vision
            && {
                let capable = mtp_bundled || mtp_draft_model.is_some();
                if !capable {
                    tracing::warn!(
                        "MTP requested but the model has no bundled NextN/MTP layers and no external MTP draft model was found; continuing without MTP"
                    );
                }
                capable
            };
        if rt.mtp_enabled && use_vision {
            tracing::warn!(
                "MTP requested but disabled for this request because vision input is active"
            );
            self.observer.notice(LlamaNotice::MtpDisabledForVision);
        }
        let model_reloaded = engine.model_reloaded;
        let max_ctx = model.n_ctx_train().max(1);
        let backend_path_used = engine.backend_path_used;
        let backend_label = backend_path_used.map_or("unknown", BackendPath::as_str);
        let gpu_load_fallback_activated = engine.gpu_load_fallback_activated;
        let actual_gpu_layers_used = engine.actual_gpu_layers_used;
        let cpu_runtime_active = backend_path_used == Some(BackendPath::Cpu)
            || actual_gpu_layers_used == Some(0)
            || !engine.supports_gpu_offload;
        let runtime_offload_kqv = if cpu_runtime_active {
            Some(false)
        } else if let Some(placement) = kv_placement_offload_kqv {
            Some(placement)
        } else if rt.offload_kqv.is_some() {
            rt.offload_kqv
        } else if rt.mtp_enabled && !media_requested || using_rocm_backend() {
            Some(false)
        } else {
            None
        };
        let shape = model_shape(model);
        let raw_recommended_ctx = compute_recommended_context(
            &shape,
            available_memory_bytes,
            available_vram_bytes,
            max_ctx,
            actual_gpu_layers_used.unwrap_or(0),
            runtime_offload_kqv,
            kv_types,
        );
        let mut batch_size_limit = self.batch_size_limit;
        let recommended_ctx = if cpu_runtime_active {
            compute_cpu_fallback_limits(
                &shape,
                available_memory_bytes,
                max_ctx,
                actual_gpu_layers_used.unwrap_or(0),
                kv_types,
                None,
                batch_size_limit,
            )
            .map(|(safe_ctx, _)| safe_ctx)
            .or(raw_recommended_ctx)
        } else {
            raw_recommended_ctx
        };
        let mut ctx_size = if let Some(requested) = self.requested_context {
            requested.min(max_ctx)
        } else if let Some(recommended) = recommended_ctx {
            if recommended == 0 {
                return Err(failed(
                    "llama.cpp model likely won't fit in memory. Try a smaller model or set a shorter context.",
                ));
            }
            recommended.min(max_ctx).max(1)
        } else {
            max_ctx
        };
        let known_devices_multi = if multi_gpu_active {
            Some(rt.gpu_device_ids.clone())
        } else {
            None
        };
        for (key, value) in [
            ("updatedAt", json!(timestamp_ms())),
            ("backendPathUsed", json!(backend_label)),
            (
                "gpuLoadFallbackActivated",
                json!(gpu_load_fallback_activated),
            ),
            ("gpuFallbackReason", json!(engine.gpu_load_fallback_reason)),
            ("supportsGpuOffload", json!(engine.supports_gpu_offload)),
            ("actualGpuLayersUsed", json!(actual_gpu_layers_used)),
            ("multiGpuEnabled", json!(multi_gpu_active)),
            (
                "multiGpuDeviceIds",
                json!(known_devices_multi.clone().unwrap_or_default()),
            ),
            (
                "smartOffloadPlanningConfig",
                json!(smart_offload_planning_config),
            ),
            ("singleGpuDeviceId", json!(rt.single_gpu_device_id)),
            (
                "smartGpuLayerFallbackActivated",
                json!(engine.smart_gpu_layer_fallback_activated),
            ),
            ("compiledGpuBackends", json!(engine.compiled_gpu_backends)),
            ("availableMemoryBytes", json!(available_memory_bytes)),
            ("availableVramBytes", json!(available_vram_bytes)),
            ("llamaMultiGpuEnabled", json!(multi_gpu_active)),
            ("selectedGpuDeviceIds", json!(known_devices_multi)),
            (
                "llamaGpuDistributionMode",
                json!(multi_gpu_active.then(|| distribution_mode.clone())),
            ),
            (
                "llamaKvPlacement",
                json!(multi_gpu_active.then(|| {
                    rt.kv_placement
                        .clone()
                        .unwrap_or_else(|| "auto".to_string())
                })),
            ),
            (
                "llamaMainGpu",
                json!(rt.main_gpu.filter(|_| multi_gpu_active)),
            ),
            (
                "gpuDeviceLayerPlacement",
                json!(if multi_gpu_active {
                    multi_gpu_distribution
                        .as_ref()
                        .map(|distribution| distribution.per_device_layers.clone())
                } else {
                    None
                }),
            ),
            ("modelSizeBytes", json!(model.size())),
            ("recommendedContext", json!(recommended_ctx)),
            ("strictModeEnabled", json!(rt.strict_mode)),
        ] {
            set_field(&mut self.report, key, value);
        }
        if model_reloaded {
            emit_model_load_finalizing(self.observer, request_id, model_path, backend_path_used);
        }
        if !rt.strict_mode
            && cpu_runtime_active
            && let Some((safe_ctx, safe_batch)) = compute_cpu_fallback_limits(
                &shape,
                available_memory_bytes,
                max_ctx,
                actual_gpu_layers_used.unwrap_or(0),
                kv_types,
                self.requested_context,
                batch_size_limit,
            )
        {
            let reason = if gpu_load_fallback_activated {
                "GPU load fell back to CPU"
            } else {
                "CPU runtime active"
            };
            if ctx_size > safe_ctx {
                tracing::warn!(
                    reason,
                    from = ctx_size,
                    to = safe_ctx,
                    requested_context = ?self.requested_context,
                    recommended_context = ?recommended_ctx,
                    "clamping context using RAM-derived fallback limits"
                );
                ctx_size = safe_ctx;
            }
            if batch_size_limit > safe_batch {
                tracing::warn!(
                    reason,
                    from = batch_size_limit,
                    to = safe_batch,
                    "reducing llama batch size for CPU headroom"
                );
                batch_size_limit = safe_batch;
                self.batch_size_limit = safe_batch;
            }
        }
        set_field(&mut self.report, "initialContextCandidate", json!(ctx_size));
        set_field(
            &mut self.report,
            "initialBatchCandidate",
            json!(ctx_size.min(batch_size_limit).max(1)),
        );

        self.failure_stage = "build_prompt";
        let mut built_prompt = build_prompt(
            model,
            &PromptRequest {
                messages: &self.prompt_messages,
                chat_template_override: rt.chat_template_override.as_deref(),
                chat_template_preset: rt.chat_template_preset.as_deref(),
                allow_raw_completion_fallback: rt.raw_completion_fallback,
                tools: self.request.active_tools(),
                tool_choice: self.request.tool_choice.as_ref(),
                options: &self.options,
            },
        )?;
        if self.request.reasoning.force_gemma4_reasoning {
            built_prompt.prompt.push_str(GEMMA4_REASONING_PREFILL);
            tracing::info!(
                prefill = GEMMA4_REASONING_PREFILL,
                "forced reasoning prefill appended to prompt"
            );
        }
        if built_prompt.chat_template_result.is_some() {
            tracing::debug!(
                payload = %self.structured_debug_payload(&built_prompt),
                "llama tool calling"
            );
        }
        let mut stop_sequences = self.request.stop_sequences();
        for stop in &built_prompt.additional_stop_sequences {
            if !stop.is_empty() && !stop_sequences.iter().any(|existing| existing == stop) {
                stop_sequences.push(stop.clone());
            }
        }
        let stop_matcher = IncrementalStopMatcher::new(&stop_sequences);
        if built_prompt.used_raw_completion_fallback {
            tracing::warn!(
                attempted_source = built_prompt
                    .attempted_template_source
                    .as_deref()
                    .unwrap_or("none"),
                reason = built_prompt
                    .raw_completion_fallback_reason
                    .as_deref()
                    .unwrap_or("unknown"),
                "using raw completion fallback after chat template resolution/application failed"
            );
        } else {
            let source = built_prompt
                .applied_template_source
                .as_deref()
                .unwrap_or("unknown");
            tracing::info!(source, "using llama chat template");
            if let Some(diagnostics) = built_prompt.tool_template_diagnostics.as_deref() {
                tracing::warn!(
                    source,
                    diagnostics,
                    "llama native tool-call template heuristic warning"
                );
            }
        }
        let model_default_add_bos = model_tokenizer_adds_bos(model);
        let prompt_add_bos = resolve_prompt_add_bos(model, built_prompt.prompt_mode);
        tracing::info!(
            mode = prompt_mode_label(built_prompt.prompt_mode),
            add_bos = add_bos_label(prompt_add_bos),
            model_tokenizer_add_bos = model_tokenizer_add_bos_label(model_default_add_bos),
            source = built_prompt
                .applied_template_source
                .as_deref()
                .or(built_prompt.attempted_template_source.as_deref())
                .unwrap_or("none"),
            reason = prompt_add_bos_reason(built_prompt.prompt_mode, model_default_add_bos),
            "llama prompt tokenization"
        );
        let prepared_prompt = if use_vision {
            let mtmd_ctx =
                mtmd_ctx.ok_or_else(|| failed("llama.cpp multimodal context unavailable"))?;
            let mut bitmaps = Vec::with_capacity(self.media.len());
            for (index, item) in self.media.iter().enumerate() {
                let bitmap = match item {
                    InlineMedia::Image(bytes) => {
                        decode_mtmd_bitmap(mtmd_ctx, bytes).map_err(|error| {
                            failed(format!(
                                "Failed to decode image {index} for llama.cpp vision: {error}"
                            ))
                        })?
                    }
                    InlineMedia::Audio(bytes) => {
                        MtmdBitmap::from_buffer(mtmd_ctx, bytes).map_err(|error| {
                            failed(format!(
                                "Failed to decode audio {index} for llama.cpp: {error}"
                            ))
                        })?
                    }
                };
                bitmaps.push(bitmap);
            }
            let bitmap_refs: Vec<&MtmdBitmap> = bitmaps.iter().collect();
            let chunks = mtmd_ctx
                .tokenize(
                    MtmdInputText {
                        text: built_prompt.prompt.clone(),
                        add_special: matches!(prompt_add_bos, AddBos::Always),
                        parse_special: true,
                    },
                    &bitmap_refs,
                )
                .map_err(|error| {
                    failed(format!(
                        "Failed to tokenize llama.cpp multimodal prompt: {error}"
                    ))
                })?;
            self.prompt_tokens = chunks.total_tokens() as u64;
            PreparedPrompt::Vision(chunks)
        } else {
            let tokens = model
                .str_to_token(&built_prompt.prompt, prompt_add_bos)
                .map_err(|error| failed(format!("Failed to tokenize prompt: {error}")))?;
            self.prompt_tokens = tokens.len() as u64;
            PreparedPrompt::Text(tokens)
        };
        let prompt_eval_span = match &prepared_prompt {
            PreparedPrompt::Text(tokens) => tokens.len(),
            PreparedPrompt::Vision(chunks) => {
                usize::try_from(chunks.total_positions()).map_err(|_| {
                    failed("llama.cpp multimodal prompt position count overflowed usize")
                })?
            }
        };
        if prompt_eval_span as u32 >= ctx_size {
            return Err(failed(format!(
                "Prompt is too long for the context window (prompt tokens: {}, context: {ctx_size}). Reduce messages or lower context length.",
                self.prompt_tokens
            )));
        }

        let preferred_offload_kqv = if let Some(explicit) = rt.offload_kqv {
            Some(explicit)
        } else if rt.mtp_enabled && !media_requested || using_rocm_backend() {
            Some(false)
        } else {
            Some(engine.supports_gpu_offload)
        };
        let requested_ctx_size = ctx_size;
        let initial_batch = ctx_size.min(batch_size_limit).max(1);
        let mut resolved_ctx_size = ctx_size;
        let mut resolved_n_batch = initial_batch;
        let mut resolved_kqv = preferred_offload_kqv;
        let mut kqv_fallback_activated = false;
        let mut context_failures = Vec::new();
        let context_attempts = if rt.strict_mode {
            vec![(ctx_size, initial_batch)]
        } else {
            context_attempt_candidates(
                ctx_size,
                prompt_eval_span,
                self.requested_context,
                batch_size_limit,
            )
        };
        let same_ctx_attempts: Vec<(u32, u32)> = context_attempts
            .iter()
            .copied()
            .filter(|(attempt_ctx, _)| *attempt_ctx == requested_ctx_size)
            .collect();
        let reduced_ctx_attempts: Vec<(u32, u32)> = context_attempts
            .iter()
            .copied()
            .filter(|(attempt_ctx, _)| *attempt_ctx != requested_ctx_size)
            .collect();
        let can_fallback_kqv_to_ram = !rt.strict_mode && preferred_offload_kqv == Some(true);
        let hot_draft_model = mtp_draft_model
            .clone()
            .unwrap_or_else(|| engine.model.clone());
        let mut attempt_groups: Vec<AttemptGroup> = Vec::new();
        if !same_ctx_attempts.is_empty() {
            attempt_groups.push((preferred_offload_kqv, same_ctx_attempts.clone()));
            if can_fallback_kqv_to_ram {
                attempt_groups.push((Some(false), same_ctx_attempts));
            }
        }
        if !reduced_ctx_attempts.is_empty() {
            attempt_groups.push((
                if can_fallback_kqv_to_ram {
                    Some(false)
                } else {
                    preferred_offload_kqv
                },
                reduced_ctx_attempts,
            ));
        }
        let mut ctx: Option<LlamaContext<'_>> = None;
        let mut reused_mtp_runtime: Option<MtpRuntime<'_>> = None;
        let mut cached_context_tokens = None;
        let mut active_context_key = None;
        self.failure_stage = "create_context";

        'groups: for (group_index, (attempt_kqv, attempts)) in
            attempt_groups.into_iter().enumerate()
        {
            if group_index > 0 && preferred_offload_kqv == Some(true) && attempt_kqv == Some(false)
            {
                if forced_smart_gpu_layers.is_none()
                    && let (Some(estimate), Some(actual)) =
                        (smart_kv_aware_layer_estimate, actual_gpu_layers_used)
                    && estimate > 0
                    && estimate < actual
                {
                    return Err(AttemptError::RetryAtLayers(estimate));
                }
                tracing::warn!(
                    requested_ctx = requested_ctx_size,
                    initial_batch,
                    "requested context did not fit with GPU KQV offload; retrying with KV cache on RAM"
                );
            }
            for (attempt_ctx, attempt_batch) in attempts {
                let attempt_ubatch = rt.ubatch_size.map(|value| value.min(attempt_batch));
                let n_outputs_max = if mtp_active {
                    rt.mtp_draft_tokens.saturating_add(1).min(attempt_batch)
                } else {
                    1
                };
                let mut ctx_params = LlamaContextParams::default()
                    .with_n_ctx(NonZeroU32::new(attempt_ctx))
                    .with_n_batch(attempt_batch)
                    .with_n_outputs_max(n_outputs_max);
                if let Some(n_ubatch) = attempt_ubatch {
                    ctx_params = ctx_params.with_n_ubatch(n_ubatch);
                }
                ctx_params = apply_common_params(ctx_params, &rt, attempt_kqv, k_type, v_type)
                    .with_flash_attention_policy(flash_attention_type(flash_policy));
                if mtp_active {
                    ctx_params = ctx_params.with_n_rs_seq(rt.mtp_draft_tokens);
                }
                tracing::info!(
                    ctx = attempt_ctx,
                    batch = attempt_batch,
                    ubatch = ?attempt_ubatch,
                    outputs = n_outputs_max,
                    gpu_layers = ?actual_gpu_layers_used,
                    offload_kqv = ?attempt_kqv,
                    flash_attention = flash_attention_policy_label(flash_policy),
                    "creating context attempt"
                );
                let attempt_context_key = TextContextShape {
                    n_ctx: attempt_ctx,
                    n_batch: attempt_batch,
                    n_ubatch: attempt_ubatch,
                    n_outputs_max,
                    n_threads: rt.threads,
                    n_threads_batch: rt.threads_batch,
                    offload_kqv: attempt_kqv,
                    swa_full: rt.swa_full,
                    kv_type: kv_label.as_deref(),
                    flash_attention: flash_policy,
                    rope_freq_base: rt.rope_freq_base,
                    rope_freq_scale: rt.rope_freq_scale,
                    mtp_active,
                    mtp_draft_tokens: rt.mtp_draft_tokens,
                }
                .key();
                if !use_vision
                    && let Some(cache_key) = self.prompt_cache_key.as_deref()
                    && let Some((cached_ctx, cached_mtp, cached_tokens)) = worker.hot.take(
                        &engine.model,
                        &hot_draft_model,
                        cache_key,
                        &attempt_context_key,
                    )
                {
                    self.prompt_cache_hit = true;
                    resolved_ctx_size = attempt_ctx;
                    resolved_n_batch = attempt_batch;
                    resolved_kqv = attempt_kqv;
                    reused_mtp_runtime = cached_mtp;
                    cached_context_tokens = Some(cached_tokens);
                    active_context_key = Some(attempt_context_key);
                    ctx = Some(cached_ctx);
                    tracing::info!("reusing hot llama.cpp context for prompt prefix cache");
                    break 'groups;
                }
                let context_memory_headroom = if actual_gpu_layers_used.unwrap_or(0) > 0 {
                    get_available_vram_bytes()
                } else {
                    get_available_memory_bytes()
                }
                .and_then(|bytes| usize::try_from(bytes).ok());
                self.prompt_cache_evictions = self
                    .prompt_cache_evictions
                    .saturating_add(worker.hot.prepare_capacity(context_memory_headroom));

                match model.new_context(backend, ctx_params) {
                    Ok(created) => {
                        resolved_ctx_size = attempt_ctx;
                        resolved_n_batch = attempt_batch;
                        resolved_kqv = attempt_kqv;
                        active_context_key = Some(attempt_context_key);
                        kqv_fallback_activated =
                            preferred_offload_kqv == Some(true) && attempt_kqv == Some(false);
                        if kqv_fallback_activated {
                            tracing::warn!(
                                ctx = attempt_ctx,
                                "KQV GPU offload fallback activated: preserving context with KV cache on RAM"
                            );
                        }
                        if (attempt_ctx, attempt_batch) != (ctx_size, initial_batch) {
                            tracing::warn!(
                                requested_ctx = ctx_size,
                                requested_batch = initial_batch,
                                ctx = attempt_ctx,
                                batch = attempt_batch,
                                "context fallback activated"
                            );
                        }
                        ctx = Some(created);
                        break 'groups;
                    }
                    Err(error) => {
                        let raw_error = error.to_string();
                        let detail = context_error_detail(
                            &raw_error,
                            attempt_ctx,
                            attempt_batch,
                            attempt_kqv,
                            rt.offload_kqv,
                            recommended_ctx,
                            kv_label.as_deref(),
                        );
                        if !is_likely_context_oom_error(&raw_error) {
                            return Err(failed(format!(
                                "Failed to create llama context: {detail}"
                            )));
                        }
                        context_failures.push(format!(
                            "ctx={attempt_ctx} batch={attempt_batch} offload_kqv={} -> {detail}",
                            offload_kqv_mode_label(attempt_kqv)
                        ));
                    }
                }
            }
        }

        let mut ctx = ctx.ok_or_else(|| {
            failed(format!(
                "Failed to create llama context after {} fallback attempts. Last failure: {}",
                context_failures.len(),
                context_failures
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "unknown error".to_string())
            ))
        })?;
        ctx_size = resolved_ctx_size;
        let n_batch = resolved_n_batch;
        let n_ubatch = ctx.n_ubatch();
        let context_fallback_activated = (ctx_size, n_batch) != (requested_ctx_size, initial_batch);

        let draft_source = mtp_draft_model.as_deref().unwrap_or(model);
        let mut mtp_runtime = if let Some(runtime) = reused_mtp_runtime {
            Some(runtime)
        } else if mtp_active {
            let mut draft_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(resolved_ctx_size))
                .with_n_batch(resolved_n_batch)
                .with_n_rs_seq(rt.mtp_draft_tokens)
                .with_flash_attention_policy(flash_attention_type(flash_policy));
            if let Some(n_threads) = rt.threads {
                draft_params = draft_params.with_n_threads(n_threads as i32);
            }
            if let Some(n_threads_batch) = rt.threads_batch {
                draft_params = draft_params.with_n_threads_batch(n_threads_batch as i32);
            }
            if let Some(offload) = resolved_kqv {
                draft_params = draft_params.with_offload_kqv(offload);
            }
            if resolved_kqv == Some(false) {
                draft_params = draft_params.with_op_offload(false);
            }
            if let Some(swa_full) = rt.swa_full {
                draft_params = draft_params.with_swa_full(swa_full);
            }
            if let Some(k_type) = k_type {
                draft_params = draft_params.with_type_k(k_type);
            }
            if let Some(v_type) = v_type {
                draft_params = draft_params.with_type_v(v_type);
            }
            if let Some(base) = rt.rope_freq_base {
                draft_params = draft_params.with_rope_freq_base(base as f32);
            }
            if let Some(scale) = rt.rope_freq_scale {
                draft_params = draft_params.with_rope_freq_scale(scale as f32);
            }
            let mode = if mtp_draft_model.is_some() {
                "external"
            } else {
                "embedded"
            };
            let mtp_batch = rt.mtp_draft_tokens.max(1) + 1;
            tracing::info!(
                mode,
                ctx = resolved_ctx_size,
                n_batch = mtp_batch,
                offload_kqv = ?resolved_kqv,
                "creating MTP draft context"
            );
            match MtpRuntime::new(
                model,
                draft_source,
                &ctx,
                backend,
                draft_params,
                rt.mtp_draft_tokens as usize,
            ) {
                Ok(mut runtime) => match runtime.enable_nextn_embeddings(&mut ctx) {
                    Ok(()) => {
                        tracing::info!(
                            mode = if runtime.shared {
                                "shared-assistant"
                            } else {
                                "embedded"
                            },
                            draft_tokens = rt.mtp_draft_tokens,
                            ctx = resolved_ctx_size,
                            n_batch = runtime.max_batch,
                            "MTP active"
                        );
                        Some(runtime)
                    }
                    Err(error) => {
                        tracing::warn!(%error, "MTP setup failed, continuing without MTP");
                        None
                    }
                },
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "MTP draft context creation failed, continuing without MTP"
                    );
                    None
                }
            }
        } else {
            None
        };
        if kqv_fallback_activated {
            match worker.engine.consume_kqv_fallback_toast(model_path) {
                Ok(true) => self.observer.notice(LlamaNotice::KvCacheMovedToRam),
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "failed to dedupe KQV fallback toast"),
            }
        }
        let mtp_source = if mtp_bundled {
            "bundled"
        } else if mtp_draft_model.is_some() {
            "external"
        } else {
            "none"
        };
        tracing::debug!(
            settings = %json!({
                "requestId": self.request.request_id,
                "modelPath": model_path,
                "prompt": {
                    "mode": prompt_mode_label(built_prompt.prompt_mode),
                    "templateSource": built_prompt.applied_template_source,
                    "templateUsed": built_prompt.applied_template_text,
                    "attemptedTemplateSource": built_prompt.attempted_template_source,
                    "attemptedTemplate": built_prompt.attempted_template_text,
                    "usedRawCompletionFallback": built_prompt.used_raw_completion_fallback,
                    "rawCompletionFallbackReason": built_prompt.raw_completion_fallback_reason,
                    "bosMode": add_bos_label(prompt_add_bos),
                    "bosReason": prompt_add_bos_reason(built_prompt.prompt_mode, model_default_add_bos),
                },
                "runtime": {
                    "requestedContext": self.requested_context,
                    "initialContextCandidate": requested_ctx_size,
                    "actualContextUsed": ctx_size,
                    "requestedBatchLimit": batch_size_limit,
                    "requestedUbatchLimit": rt.ubatch_size,
                    "initialBatchCandidate": initial_batch,
                    "actualNBatchUsed": n_batch,
                    "actualNUbatchUsed": n_ubatch,
                    "requestedGpuLayers": rt.gpu_layers,
                    "actualGpuLayersUsed": actual_gpu_layers_used,
                    "actualKvTypeUsed": kv_type_label(kv_label.as_deref()),
                    "actualOffloadKqvMode": offload_kqv_mode_label(resolved_kqv),
                    "flashAttentionPolicy": flash_attention_policy_label(flash_policy),
                    "actualBackendPathUsed": backend_label,
                    "compiledGpuBackends": engine.compiled_gpu_backends,
                    "supportsGpuOffload": engine.supports_gpu_offload,
                    "strictModeEnabled": rt.strict_mode,
                    "gpuLoadFallbackActivated": gpu_load_fallback_activated,
                    "smartGpuLayerFallbackActivated": engine.smart_gpu_layer_fallback_activated,
                    "kqvFallbackActivated": kqv_fallback_activated,
                    "contextFallbackActivated": context_fallback_activated,
                    "mmprojPath": rt.mmproj_path,
                    "mtpRequested": rt.mtp_enabled,
                    "mtpActive": mtp_active,
                    "mtpDraftTokens": rt.mtp_draft_tokens,
                    "mtpSource": mtp_source,
                    "visionRequested": media_requested,
                    "visionActive": use_vision,
                    "imageCount": self.image_count,
                    "audioCount": self.audio_count,
                }
            }),
            "llama runtime"
        );
        for (key, value) in [
            ("actualContextUsed", json!(ctx_size)),
            ("actualBatchUsed", json!(n_batch)),
            ("actualUbatchUsed", json!(n_ubatch)),
            (
                "actualKvTypeUsed",
                json!(kv_type_label(kv_label.as_deref())),
            ),
            (
                "actualOffloadKqvMode",
                json!(offload_kqv_mode_label(resolved_kqv)),
            ),
            ("kqvFallbackActivated", json!(kqv_fallback_activated)),
            (
                "flashAttentionPolicy",
                json!(flash_attention_policy_label(flash_policy)),
            ),
            (
                "contextFallbackActivated",
                json!(context_fallback_activated),
            ),
            (
                "promptTemplateSource",
                json!(
                    built_prompt
                        .applied_template_source
                        .clone()
                        .or(built_prompt.attempted_template_source.clone())
                ),
            ),
        ] {
            set_field(&mut self.report, key, value);
        }
        tracing::info!(
            prompt_mode = prompt_mode_label(built_prompt.prompt_mode),
            template_source = built_prompt.applied_template_source.as_deref().unwrap_or("none"),
            fallback_prompt = built_prompt.used_raw_completion_fallback,
            bos = add_bos_label(prompt_add_bos),
            ctx = ctx_size,
            n_batch,
            n_ubatch,
            gpu_layers = ?actual_gpu_layers_used,
            kv_type = kv_type_label(kv_label.as_deref()),
            offload_kqv = offload_kqv_mode_label(resolved_kqv),
            backend_path = backend_label,
            flash_attention = flash_attention_policy_label(flash_policy),
            smart_gpu_fallback = engine.smart_gpu_layer_fallback_activated,
            kqv_fallback = kqv_fallback_activated,
            context_fallback = context_fallback_activated,
            "llama runtime resolved"
        );
        if model_reloaded {
            emit_model_load_complete(
                self.observer,
                request_id,
                model_path,
                backend_path_used,
                gpu_load_fallback_activated,
            );
        }

        self.failure_stage = "prompt_evaluation";
        self.check_abort()?;
        ctx.reset_timings();
        if let Some(runtime) = mtp_runtime.as_mut() {
            runtime.draft.reset_timings();
        }
        let batch_capacity = n_batch as usize;
        let mut batch = LlamaBatch::new(batch_capacity, 1);
        let mut global_pos: i32 = 0;
        let mut context_tokens: Option<Vec<LlamaToken>> = None;
        let prompt_last_logits_index = match prepared_prompt {
            PreparedPrompt::Text(tokens) => {
                let tokens_len = tokens.len();
                let mut chunk_start = 0usize;
                if let Some(cached_tokens) = cached_context_tokens.take() {
                    let common_prefix = common_token_prefix(&cached_tokens, &tokens);
                    let mut rewind_from = common_prefix.saturating_sub(1);
                    let rewind_succeeded;
                    if let Some(runtime) = mtp_runtime.as_mut() {
                        if !runtime.shared && common_prefix >= 2 {
                            let carry_position = common_prefix - 2;
                            rewind_from = common_prefix - 1;
                            rewind_succeeded = ctx
                                .clear_kv_cache_seq(Some(0), Some(carry_position as u32), None)
                                .map_err(|error| {
                                    failed(format!("Failed to rewind prompt KV cache: {error}"))
                                })?;
                            if rewind_succeeded {
                                runtime.reset_for_prompt_reuse(rewind_from as u32)?;
                                batch.clear();
                                batch
                                    .add(tokens[carry_position], carry_position as i32, &[0], false)
                                    .map_err(|error| {
                                        failed(format!(
                                            "Failed to rebuild prompt-cache carry token: {error}"
                                        ))
                                    })?;
                                ctx.decode(&mut batch).map_err(|error| {
                                    failed(format!(
                                        "Failed to rebuild prompt-cache carry state: {error}"
                                    ))
                                })?;
                                runtime.set_prefill_carry_from_target(&ctx, 0)?;
                                self.cached_prompt_tokens = carry_position as u64;
                            }
                        } else {
                            rewind_succeeded = ctx
                                .clear_kv_cache_seq(Some(0), Some(rewind_from as u32), None)
                                .map_err(|error| {
                                    failed(format!("Failed to rewind prompt KV cache: {error}"))
                                })?;
                            if rewind_succeeded {
                                runtime.reset_for_prompt_reuse(rewind_from as u32)?;
                                self.cached_prompt_tokens = rewind_from as u64;
                            }
                        }
                    } else {
                        rewind_succeeded = ctx
                            .clear_kv_cache_seq(Some(0), Some(rewind_from as u32), None)
                            .map_err(|error| {
                                failed(format!("Failed to rewind prompt KV cache: {error}"))
                            })?;
                        if rewind_succeeded {
                            self.cached_prompt_tokens = rewind_from as u64;
                        }
                    }
                    if rewind_succeeded {
                        chunk_start = rewind_from;
                        global_pos = rewind_from as i32;
                    } else {
                        ctx.clear_kv_cache();
                        if let Some(runtime) = mtp_runtime.as_mut() {
                            runtime.reset_for_prompt_reuse(0)?;
                        }
                        self.cached_prompt_tokens = 0;
                        tracing::warn!(
                            "prompt KV cache could not be partially rewound; evaluating the full prompt"
                        );
                    }
                }
                while chunk_start < tokens_len {
                    self.check_abort()?;
                    let chunk_end = (chunk_start + batch_capacity).min(tokens_len);
                    batch.clear();
                    for (offset, token) in
                        tokens[chunk_start..chunk_end].iter().copied().enumerate()
                    {
                        let pos = global_pos + offset as i32;
                        let is_last = chunk_start + offset + 1 == tokens_len;
                        batch.add(token, pos, &[0], is_last).map_err(|error| {
                            failed(format!(
                                "Failed to build llama batch (chunk {chunk_start}..{chunk_end} size={tokens_len} n_batch={n_batch}): {error}"
                            ))
                        })?;
                    }
                    ctx.decode(&mut batch).map_err(|error| {
                        failed(format!(
                            "llama_decode failed during prompt evaluation: {error}"
                        ))
                    })?;
                    if let Some(runtime) = mtp_runtime.as_mut() {
                        runtime.prefill_draft_chunk(
                            &ctx,
                            &tokens[chunk_start..chunk_end],
                            global_pos,
                            chunk_end == tokens_len,
                        )?;
                    }
                    self.check_abort()?;
                    global_pos += (chunk_end - chunk_start) as i32;
                    chunk_start = chunk_end;
                }
                context_tokens = Some(tokens);
                batch.n_tokens().saturating_sub(1)
            }
            PreparedPrompt::Vision(chunks) => {
                self.check_abort()?;
                let mtmd_ctx = mtmd_ctx.ok_or_else(|| {
                    failed("llama.cpp multimodal context unavailable during prompt evaluation")
                })?;
                global_pos = chunks
                    .eval_chunks(mtmd_ctx, &ctx, 0, 0, n_batch as i32, true)
                    .map_err(|error| {
                        failed(format!(
                            "llama.cpp multimodal prompt evaluation failed: {error}"
                        ))
                    })?;
                self.check_abort()?;
                -1
            }
        };
        tracing::info!(
            prompt_tokens = self.prompt_tokens,
            prompt_positions = global_pos,
            target_new_tokens = self.max_tokens,
            vision = use_vision,
            "prompt evaluation complete"
        );
        set_field(&mut self.report, "promptTokens", json!(self.prompt_tokens));
        set_field(
            &mut self.report,
            "cachedPromptTokens",
            json!(self.cached_prompt_tokens),
        );
        set_field(
            &mut self.report,
            "promptPositions",
            json!(u64::try_from(global_pos).ok()),
        );
        let prompt_timings = ctx.timings();
        let prompt_eval_ms = prompt_timings.t_p_eval_ms() + prompt_timings.t_eval_ms();
        let prompt_eval_tokens = i64::from(prompt_timings.n_p_eval())
            .saturating_add(i64::from(prompt_timings.n_eval()))
            .max(0) as u64;
        self.native_prompt_eval_ms = Some(prompt_eval_ms.max(0.0));
        self.native_prompt_eval_tokens = Some(prompt_eval_tokens);
        self.native_prompt_eval_tps = (prompt_eval_ms > 0.0 && prompt_eval_tokens > 0)
            .then(|| prompt_eval_tokens as f64 * 1_000.0 / prompt_eval_ms);
        self.native_draft_prompt_eval_ms = mtp_runtime.as_mut().map(|runtime| {
            let timings = runtime.draft.timings();
            (timings.t_p_eval_ms() + timings.t_eval_ms()).max(0.0)
        });
        ctx.reset_timings();
        if let Some(runtime) = mtp_runtime.as_mut() {
            runtime.draft.reset_timings();
        }

        let prompt_len = global_pos;
        let mut n_cur = prompt_len;
        let max_new = self
            .max_tokens
            .min(ctx_size.saturating_sub(n_cur as u32 + 1));
        let sampler_config = ResolvedSamplerConfig {
            profile: self.sampling.profile,
            order: self.sampling.order.clone(),
            temperature: self.sampling.temperature,
            top_p: self.sampling.top_p,
            top_k: self.sampling.top_k,
            min_p: self.sampling.min_p,
            typical_p: self.sampling.typical_p,
            repeat_penalty: self.sampling.repeat_penalty,
            n_pen_range: self.sampling.n_pen_range,
            context_size: ctx_size,
            dry_multiplier: self.sampling.dry_multiplier,
            dry_base: self.sampling.dry_base,
            dry_allowed_length: self.sampling.dry_allowed_length,
            dry_penalty_last_n: self.sampling.dry_penalty_last_n,
            dry_sequence_breakers: self.sampling.dry_sequence_breakers.clone(),
            xtc_probability: self.sampling.xtc_probability,
            xtc_threshold: self.sampling.xtc_threshold,
            frequency_penalty: self.sampling.frequency_penalty,
            presence_penalty: self.sampling.presence_penalty,
            seed: self.sampling.seed,
            adaptive_target: self.sampling.adaptive_target,
            adaptive_decay: self.sampling.adaptive_decay,
        };
        self.check_abort()?;
        let built_sampler = build_sampler(
            model,
            &sampler_config,
            built_prompt.chat_template_result.as_ref(),
        )
        .map_err(|error| self.structured_failure(&built_prompt, "grammar_sampler_init", error))?;
        tracing::info!(
            profile = sampler_config.profile,
            order = %built_sampler.order.join(" -> "),
            active_params = %built_sampler.active_params,
            "llama sampler"
        );
        let mut sampler = built_sampler.sampler;
        let stream = self.request.stream;
        let mut thinking = if self.request.reasoning.force_gemma4_reasoning {
            ThinkingTagParser::starting_in_reasoning(GEMMA4_REASONING_CLOSE)
        } else {
            ThinkingTagParser::default()
        };
        let mut structured_parser = if stream && built_prompt.native_tool_parse_supported {
            built_prompt
                .chat_template_result
                .as_ref()
                .map(|result| result.streaming_state_oaicompat())
                .transpose()
                .map_err(|error| {
                    self.structured_failure(&built_prompt, "structured_parser_init", error)
                })?
        } else {
            None
        };
        let structured = built_prompt.chat_template_result.is_some();
        let mut streamed_structured_text = String::new();
        let mut structured_parsed_len = 0usize;
        let mut stream_emitted_len = 0usize;
        let target_len = prompt_len + max_new as i32;
        let mut reached_eos = false;
        let mut reached_stop_sequence = false;
        let mut pending_utf8 = Vec::<u8>::new();
        let mut sample_index = prompt_last_logits_index;
        let generation_started_at = Instant::now();
        let mut last_heartbeat_at = Instant::now();
        let mut heartbeat_emitted = false;
        let mut last_stream_flush_at = Instant::now();
        let mut stream_has_flushed = false;
        self.failure_stage = "generation";
        while n_cur < target_len {
            self.check_abort()?;
            let token = if let Some(runtime) = mtp_runtime.as_mut() {
                if runtime.pending.is_empty() {
                    let accepted =
                        runtime.round(&mut ctx, &mut sampler, model, n_cur, target_len)?;
                    runtime.pending.extend(accepted);
                }
                match runtime.pending.pop_front() {
                    Some(token) => token,
                    None => break,
                }
            } else {
                sampler.sample(&ctx, sample_index)
            };
            if model.is_eog_token(token) {
                reached_eos = true;
                break;
            }
            pending_utf8.extend_from_slice(&token_piece_bytes(model, token)?);
            let piece = drain_utf8(&mut pending_utf8);

            if !piece.is_empty() {
                let appended_from = self.output.len();
                self.output.push_str(&piece);
                if let Some(stop_index) = stop_matcher.find(&self.output, appended_from) {
                    self.output.truncate(stop_index);
                    reached_stop_sequence = true;
                }
                if !structured {
                    if stream && stream_emitted_len < self.output.len() {
                        let safe_emit_end =
                            stop_matcher.safe_end(&self.output, reached_stop_sequence);
                        let pending_bytes = safe_emit_end.saturating_sub(stream_emitted_len);
                        if should_flush_stream(
                            pending_bytes,
                            stream_has_flushed,
                            last_stream_flush_at.elapsed(),
                            reached_stop_sequence,
                        ) {
                            self.emit_content(
                                &mut thinking,
                                &self.output[stream_emitted_len..safe_emit_end],
                            );
                            stream_emitted_len = safe_emit_end;
                            stream_has_flushed = true;
                            last_stream_flush_at = Instant::now();
                        }
                    }
                } else if stream && let Some(parser) = structured_parser.as_mut() {
                    let safe_parse_end = stop_matcher.safe_end(&self.output, reached_stop_sequence);
                    let pending_bytes = safe_parse_end.saturating_sub(structured_parsed_len);
                    if should_flush_stream(
                        pending_bytes,
                        stream_has_flushed,
                        last_stream_flush_at.elapsed(),
                        reached_stop_sequence,
                    ) {
                        let deltas = parser
                            .update(&self.output[structured_parsed_len..safe_parse_end], true)
                            .map_err(|error| {
                                self.structured_failure(
                                    &built_prompt,
                                    "structured_stream_parse",
                                    error,
                                )
                            })?;
                        self.emit_structured_deltas(
                            deltas,
                            &mut thinking,
                            &mut streamed_structured_text,
                        )?;
                        structured_parsed_len = safe_parse_end;
                        stream_has_flushed = true;
                        last_stream_flush_at = Instant::now();
                    }
                }
                if reached_stop_sequence {
                    break;
                }
            }

            self.completion_tokens += 1;
            if self.first_token_ms.is_none() {
                self.first_token_ms = Some(self.started_at.elapsed().as_millis() as u64);
            }
            if !heartbeat_emitted || last_heartbeat_at.elapsed().as_secs() >= 1 {
                heartbeat_emitted = true;
                last_heartbeat_at = Instant::now();
                let generation_elapsed = generation_started_at.elapsed();
                let elapsed_ms = generation_elapsed.as_millis() as u64;
                let elapsed_seconds = generation_elapsed.as_secs_f64();
                let has_stable_rate = elapsed_seconds >= 1.0;
                let tps = if has_stable_rate {
                    self.completion_tokens as f64 / elapsed_seconds
                } else {
                    0.0
                };
                let ctx_fill = if ctx_size > 0 {
                    f64::from(n_cur) / f64::from(ctx_size)
                } else {
                    0.0
                };
                if has_stable_rate {
                    self.metric_samples.push(json!({
                        "tMs": elapsed_ms,
                        "tokens": self.completion_tokens,
                        "tps": tps,
                        "ctxFill": ctx_fill,
                    }));
                }
                self.observer.heartbeat(GenerationHeartbeat {
                    tokens: self.completion_tokens,
                    elapsed_ms,
                    tokens_per_second: tps,
                    recent_text: self.output.clone(),
                });
            }

            if mtp_runtime.is_some() {
                if let Some(tokens) = context_tokens.as_mut() {
                    tokens.push(token);
                }
                n_cur += 1;
                continue;
            }
            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|error| failed(format!("Failed to update llama batch: {error}")))?;
            n_cur += 1;
            ctx.decode(&mut batch)
                .map_err(|error| failed(format!("llama_decode failed: {error}")))?;
            if let Some(tokens) = context_tokens.as_mut() {
                tokens.push(token);
            }
            sample_index = batch.n_tokens() - 1;
        }

        if !pending_utf8.is_empty() {
            let tail = String::from_utf8_lossy(&pending_utf8).to_string();
            let appended_from = self.output.len();
            self.output.push_str(&tail);
            if let Some(stop_index) = stop_matcher.find(&self.output, appended_from) {
                self.output.truncate(stop_index);
                reached_stop_sequence = true;
            }
        }
        if !structured && stream && stream_emitted_len < self.output.len() {
            self.emit_content(&mut thinking, &self.output[stream_emitted_len..]);
        }

        let generation_elapsed = generation_started_at.elapsed();
        self.generation_elapsed_ms = Some(generation_elapsed.as_millis() as u64);
        self.generation_elapsed_seconds = Some(generation_elapsed.as_secs_f64());
        let target_timings = ctx.timings();
        let target_compute_ms = target_timings.t_p_eval_ms() + target_timings.t_eval_ms();
        let draft_compute_ms = mtp_runtime.as_mut().map_or(0.0, |runtime| {
            let timings = runtime.draft.timings();
            timings.t_p_eval_ms() + timings.t_eval_ms()
        });
        let compute_ms = (target_compute_ms + draft_compute_ms).max(0.0);
        self.native_generation_compute_ms = Some(compute_ms);
        self.native_generation_tps = (compute_ms > 0.0 && self.completion_tokens > 0)
            .then(|| self.completion_tokens as f64 * 1_000.0 / compute_ms);
        self.app_generation_overhead_ms =
            Some((generation_elapsed.as_secs_f64() * 1_000.0 - compute_ms).max(0.0));

        if let Some(runtime) = mtp_runtime.as_ref() {
            let tokens_per_round = if runtime.rounds > 0 {
                runtime.accepted as f64 / runtime.rounds as f64
            } else {
                0.0
            };
            let draft_acceptance = if runtime.drafted > 0 {
                runtime.accepted.saturating_sub(runtime.rounds) as f64 / runtime.drafted as f64
            } else {
                0.0
            };
            tracing::info!(
                rounds = runtime.rounds,
                drafted = runtime.drafted,
                accepted = runtime.accepted,
                tokens_per_round,
                draft_acceptance,
                configured_draft_n = runtime.draft_n_max,
                final_draft_n = runtime.draft_n,
                adaptations = runtime.adaptation_count,
                "MTP stats"
            );
            let stats = LlamaMtpStats {
                draft_tokens: rt.mtp_draft_tokens,
                final_draft_tokens: u32::try_from(runtime.draft_n).ok(),
                adaptation_count: Some(runtime.adaptation_count),
                rounds: runtime.rounds,
                drafted: runtime.drafted,
                accepted: runtime.accepted,
                tokens_per_round,
                draft_acceptance,
            };
            set_field(&mut self.report, "mtpStats", stats.to_json());
            self.mtp_stats = Some(stats);
        }

        if let Some(parser) = structured_parser.as_mut() {
            let is_partial = !reached_eos && !reached_stop_sequence;
            let final_input = self.output.get(structured_parsed_len..).unwrap_or("");
            let deltas = parser.update(final_input, is_partial).map_err(|error| {
                failed(format!(
                    "Failed to finalize llama.cpp structured parse state: {error}"
                ))
            })?;
            self.emit_structured_deltas(deltas, &mut thinking, &mut streamed_structured_text)?;
        }

        self.finish_reason = if reached_stop_sequence || reached_eos {
            LlamaFinishReason::Stop
        } else {
            LlamaFinishReason::Length
        };

        let mut final_tool_calls = Vec::new();
        let parsed_message = if let Some(template_result) =
            built_prompt.chat_template_result.as_ref()
        {
            let is_partial = self.finish_reason == LlamaFinishReason::Length;
            let mut message = if let Some(recovered) =
                recover_message_from_raw_tool_output(&self.output)
            {
                tracing::info!("using app-level raw tool-call recovery for final llama response");
                recovered
            } else if built_prompt.native_tool_parse_supported {
                match template_result.parse_response_oaicompat(&self.output, is_partial) {
                    Ok(parsed) => serde_json::from_str(&parsed).map_err(|error| {
                        failed(format!(
                            "Failed to deserialize llama.cpp structured message: {error}"
                        ))
                    })?,
                    Err(error) => {
                        return Err(self.structured_failure(
                            &built_prompt,
                            "structured_response_parse",
                            error,
                        ));
                    }
                }
            } else {
                json!({ "role": "assistant", "content": self.output })
            };
            if let Some(object) = message.as_object_mut() {
                object
                    .entry("role".to_string())
                    .or_insert_with(|| Value::String("assistant".to_string()));
            }
            let full_text = extract_text_content(message.get("content")).unwrap_or_default();
            if stream
                && full_text.starts_with(&streamed_structured_text)
                && full_text.len() > streamed_structured_text.len()
            {
                self.observer
                    .delta(&full_text[streamed_structured_text.len()..]);
            }
            final_tool_calls = parse_tool_calls(&message);
            if !final_tool_calls.is_empty() && self.finish_reason != LlamaFinishReason::Length {
                self.finish_reason = LlamaFinishReason::ToolCalls;
            }
            tracing::debug!(
                request_id = ?self.request.request_id,
                model_path,
                raw_output = %self.output,
                parsed_message = %message,
                tool_call_count = final_tool_calls.len(),
                finish_reason = self.finish_reason.as_str(),
                "llama structured response"
            );
            message
        } else {
            tracing::debug!(
                request_id = ?self.request.request_id,
                model_path,
                raw_output = %self.output,
                finish_reason = self.finish_reason.as_str(),
                "llama response"
            );
            json!({ "role": "assistant", "content": self.output })
        };

        if stream && !final_tool_calls.is_empty() {
            self.observer.tool_calls(&final_tool_calls);
        }
        self.tool_calls = final_tool_calls;
        self.final_message = parsed_message;
        let explicit_reasoning = self
            .final_message
            .get("reasoning")
            .or_else(|| self.final_message.get("reasoning_content"))
            .or_else(|| self.final_message.get("thinking"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let raw_content = extract_text_content(self.final_message.get("content"));
        let content = raw_content.as_deref().filter(|value| !value.is_empty());
        let normalized = if self.request.reasoning.force_gemma4_reasoning {
            normalize_thinking_content_starting_in_reasoning(
                content,
                explicit_reasoning.as_deref(),
                GEMMA4_REASONING_CLOSE,
            )
        } else {
            normalize_thinking_content(content, explicit_reasoning.as_deref())
        };
        if let Some(message) = self.final_message.as_object_mut() {
            message.insert("content".to_string(), json!(normalized.content));
            message.remove("reasoning_content");
            message.remove("thinking");
            if normalized.reasoning.is_empty() {
                message.remove("reasoning");
            } else {
                message.insert("reasoning".to_string(), json!(normalized.reasoning));
            }
        }
        self.output = normalized.content;

        if stream {
            let tail = thinking.finish();
            if !tail.content.is_empty() {
                self.observer.delta(&tail.content);
            }
            if !tail.reasoning.is_empty() {
                self.observer.reasoning(&tail.reasoning);
            }
        }

        if let (Some(tokens), Some(context_key)) =
            (context_tokens.take(), active_context_key.take())
        {
            let cache_ready = if mtp_active && mtp_runtime.is_none() {
                false
            } else if let Some(runtime) = mtp_runtime.as_mut() {
                match u32::try_from(tokens.len()) {
                    Ok(token_count) => {
                        match runtime.truncate_for_prompt_cache(&mut ctx, token_count) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::warn!(%error, "discarding unusable prompt cache");
                                false
                            }
                        }
                    }
                    Err(_) => false,
                }
            } else {
                true
            };
            if cache_ready && let Some(cache_key) = self.prompt_cache_key.clone() {
                let evicted = worker.hot.store(
                    ctx,
                    mtp_runtime.take(),
                    engine.model.clone(),
                    hot_draft_model.clone(),
                    model_path,
                    cache_key,
                    context_key,
                    tokens,
                );
                self.prompt_cache_evictions = self.prompt_cache_evictions.saturating_add(evicted);
            }
        }
        Ok(())
    }

    fn finish_failed(&mut self, error: &LlamaGenerationError) {
        let aborted = matches!(error, LlamaGenerationError::Aborted);
        let gpu_fallback = self
            .report
            .get("gpuLoadFallbackActivated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status = if aborted {
            "aborted"
        } else if gpu_fallback {
            "cpuFallbackFailed"
        } else {
            "failed"
        };
        set_field(&mut self.report, "updatedAt", json!(timestamp_ms()));
        set_field(&mut self.report, "status", json!(status));
        set_field(&mut self.report, "failureStage", json!(self.failure_stage));
        set_field(&mut self.report, "errorMessage", json!(error.to_string()));
        set_field(
            &mut self.report,
            "completionTokens",
            json!(self.completion_tokens),
        );
        if aborted {
            tracing::info!(%error, "local inference aborted");
            return;
        }
        tracing::error!(%error, "local inference error");
        if !self.output.is_empty() {
            tracing::warn!(
                request_id = ?self.request.request_id,
                model_path = self.model_path,
                failure_stage = self.failure_stage,
                partial_output = %self.output,
                "local inference partial output"
            );
        }
        persist_report(self.observer, self.reports, self.model_path, &self.report);
        emit_model_load_failed(
            self.observer,
            self.request.request_id.as_deref(),
            self.model_path,
            backend_path_from_label(self.report.get("backendPathUsed").and_then(Value::as_str)),
            gpu_fallback,
        );
    }

    fn finish_succeeded(mut self, worker: &WorkerState) -> LlamaGenerationOutput {
        let tokens_per_second = self
            .generation_elapsed_seconds
            .and_then(|elapsed_seconds| {
                (elapsed_seconds > 0.0 && self.completion_tokens > 0)
                    .then(|| self.completion_tokens as f64 / elapsed_seconds)
            })
            .filter(|value| value.is_finite() && *value >= 0.0);
        let (cache_entries, cache_bytes) = worker.hot.stats();
        for (key, value) in [
            ("promptCacheHit", json!(self.prompt_cache_hit)),
            ("promptCacheEntries", json!(cache_entries)),
            ("promptCacheBytes", json!(cache_bytes)),
            (
                "promptCacheCapacityBytes",
                json!(HOT_CONTEXT_CACHE_MAX_BYTES),
            ),
            ("promptCacheEvictions", json!(self.prompt_cache_evictions)),
            ("updatedAt", json!(timestamp_ms())),
            ("completionTokens", json!(self.completion_tokens)),
            ("finishReason", json!(self.finish_reason.as_str())),
            ("firstTokenMs", json!(self.first_token_ms)),
            ("tokensPerSecond", json!(tokens_per_second)),
            ("nativePromptEvalMs", json!(self.native_prompt_eval_ms)),
            (
                "nativePromptEvalTokens",
                json!(self.native_prompt_eval_tokens),
            ),
            (
                "nativePromptEvalTokensPerSecond",
                json!(self.native_prompt_eval_tps),
            ),
            (
                "nativeDraftPromptEvalMs",
                json!(self.native_draft_prompt_eval_ms),
            ),
            (
                "nativeGenerationComputeMs",
                json!(self.native_generation_compute_ms),
            ),
            (
                "nativeGenerationTokensPerSecond",
                json!(self.native_generation_tps),
            ),
            (
                "appGenerationOverheadMs",
                json!(self.app_generation_overhead_ms),
            ),
        ] {
            set_field(&mut self.report, key, value);
        }
        let fallback_succeeded = self
            .report
            .get("gpuLoadFallbackActivated")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && self.report.get("backendPathUsed").and_then(Value::as_str) == Some("cpu");
        if fallback_succeeded {
            let field = |key: &str| self.report.get(key).cloned().unwrap_or(Value::Null);
            let suggested = json!({
                "contextLength": field("actualContextUsed"),
                "llamaBatchSize": field("actualBatchUsed"),
                "llamaUbatchSize": field("actualUbatchUsed"),
            });
            set_field(&mut self.report, "status", json!("cpuFallbackSucceeded"));
            set_field(&mut self.report, "suggestedSettings", suggested);
        } else {
            set_field(&mut self.report, "status", json!("succeeded"));
        }
        persist_report(self.observer, self.reports, self.model_path, &self.report);

        let metrics = (self.completion_tokens > 0).then(|| {
            let field = |key: &str| self.report.get(key).cloned().unwrap_or(Value::Null);
            let summary = json!({
                "modelName": self.model_path,
                "backend": field("backendPathUsed"),
                "gpuLayers": field("actualGpuLayersUsed"),
                "nCtx": field("actualContextUsed"),
                "nBatch": field("actualBatchUsed"),
                "nUbatch": field("actualUbatchUsed"),
                "kvType": field("actualKvTypeUsed"),
                "modelSizeBytes": field("modelSizeBytes"),
                "promptTokens": self.prompt_tokens,
                "cachedPromptTokens": self.cached_prompt_tokens,
                "completionTokens": self.completion_tokens,
                "totalTokens": self.prompt_tokens + self.completion_tokens,
                "ttftMs": self.first_token_ms,
                "decodeTokensPerSecond": tokens_per_second,
                "generationElapsedMs": self.generation_elapsed_ms,
                "nativePromptEvalMs": self.native_prompt_eval_ms,
                "nativePromptEvalTokens": self.native_prompt_eval_tokens,
                "nativePromptEvalTokensPerSecond": self.native_prompt_eval_tps,
                "nativeDraftPromptEvalMs": self.native_draft_prompt_eval_ms,
                "nativeGenerationComputeMs": self.native_generation_compute_ms,
                "nativeGenerationTokensPerSecond": self.native_generation_tps,
                "appGenerationOverheadMs": self.app_generation_overhead_ms,
                "finishReason": self.finish_reason.as_str(),
                "mtpStats": field("mtpStats"),
            });
            LlamaMetricsRecord {
                id: self
                    .request
                    .request_id
                    .clone()
                    .unwrap_or_else(|| format!("gen-{}", timestamp_ms())),
                model_path: self.model_path.to_string(),
                summary,
                samples: std::mem::take(&mut self.metric_samples),
            }
        });

        let reasoning = self
            .final_message
            .get("reasoning")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        LlamaGenerationOutput {
            content: self.output,
            reasoning,
            message: self.final_message,
            tool_calls: self.tool_calls,
            finish_reason: self.finish_reason,
            usage: LlamaUsage {
                prompt_tokens: self.prompt_tokens,
                cached_prompt_tokens: self.cached_prompt_tokens,
                completion_tokens: self.completion_tokens,
                first_token_ms: self.first_token_ms,
                tokens_per_second,
                mtp_stats: self.mtp_stats,
            },
            metrics,
        }
    }
}

/// The planning-config `kvType`: the shared type as legacy wrote it, or the
/// K and V types when they differ.
fn planning_kv_type(rt: &ResolvedRuntime) -> Value {
    let kv_types = rt.kv_types();
    if kv_types.shared().is_some() {
        json!(kv_types.k)
    } else {
        json!({ "k": kv_types.k, "v": kv_types.v })
    }
}

fn apply_common_params(
    mut params: LlamaContextParams,
    rt: &ResolvedRuntime,
    offload_kqv: Option<bool>,
    k_type: Option<KvCacheType>,
    v_type: Option<KvCacheType>,
) -> LlamaContextParams {
    if let Some(n_threads) = rt.threads {
        params = params.with_n_threads(n_threads as i32);
    }
    if let Some(n_threads_batch) = rt.threads_batch {
        params = params.with_n_threads_batch(n_threads_batch as i32);
    }
    if let Some(offload) = offload_kqv {
        params = params.with_offload_kqv(offload);
    }
    if let Some(swa_full) = rt.swa_full {
        params = params.with_swa_full(swa_full);
    }
    if let Some(k_type) = k_type {
        params = params.with_type_k(k_type);
    }
    if let Some(v_type) = v_type {
        params = params.with_type_v(v_type);
    }
    if let Some(base) = rt.rope_freq_base {
        params = params.with_rope_freq_base(base as f32);
    }
    if let Some(scale) = rt.rope_freq_scale {
        params = params.with_rope_freq_scale(scale as f32);
    }
    params
}

/// Takes the decodable text out of `pending`, replacing invalid bytes. An
/// incomplete trailing sequence holds back everything still pending, as in
/// legacy.
fn drain_utf8(pending: &mut Vec<u8>) -> String {
    let mut piece = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                piece.push_str(valid);
                pending.clear();
                break;
            }
            Err(error) if error.error_len().is_none() => break,
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    piece.push_str(&String::from_utf8_lossy(&pending[..valid_up_to]));
                    pending.drain(..valid_up_to);
                    continue;
                }
                let invalid_len = error.error_len().unwrap_or(1);
                piece.push_str(&String::from_utf8_lossy(&pending[..invalid_len]));
                pending.drain(..invalid_len);
            }
        }
    }
    piece
}

/// The legacy text of a message's content: a string, the `text` parts joined
/// by newlines (`None` when blank), or any other value as JSON.
fn extract_text_content(content: Option<&Value>) -> Option<String> {
    match content {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(parts)) => {
            let text = parts
                .iter()
                .filter_map(|part| {
                    let object = part.as_object()?;
                    if object.get("type").and_then(Value::as_str) != Some("text") {
                        return None;
                    }
                    object.get("text").and_then(Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        Some(other) => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_draining_keeps_split_characters_for_the_next_token() {
        let mut pending = "a\u{e9}".as_bytes()[..2].to_vec();
        assert_eq!(drain_utf8(&mut pending), "");
        assert_eq!(pending, vec![b'a', 0xC3]);
        pending.push(0xA9);
        assert_eq!(drain_utf8(&mut pending), "a\u{e9}");
        assert!(pending.is_empty());
        let mut invalid = vec![b'x', 0xFF, b'y'];
        assert_eq!(drain_utf8(&mut invalid), "x\u{fffd}y");
    }

    #[test]
    fn mtp_stats_serialize_like_legacy() {
        let stats = LlamaMtpStats {
            draft_tokens: 4,
            final_draft_tokens: Some(3),
            adaptation_count: None,
            rounds: 2,
            drafted: 6,
            accepted: 5,
            tokens_per_round: 2.5,
            draft_acceptance: 0.5,
        };
        assert_eq!(
            stats.to_json(),
            json!({
                "draftTokens": 4,
                "finalDraftTokens": 3,
                "rounds": 2,
                "drafted": 6,
                "accepted": 5,
                "tokensPerRound": 2.5,
                "draftAcceptance": 0.5,
            })
        );
    }

    #[test]
    fn inline_media_rejects_remote_urls_and_missing_audio() {
        let remote = vec![json!({"role": "user", "content": [
            {"type": "image_url", "image_url": {"url": "https://x/y.png"}}
        ]})];
        assert!(matches!(
            extract_inline_media(&remote),
            Err(LlamaGenerationError::Failed(message)) if message.contains("used remote URL")
        ));
        let audio = vec![json!({"role": "user", "content": [
            {"type": "input_audio", "input_audio": {"data": ""}}
        ]})];
        assert!(matches!(
            extract_inline_media(&audio),
            Err(LlamaGenerationError::Failed(message)) if message == "Missing inline audio data in message 0 part 0"
        ));
        let png = vec![json!({"role": "user", "content": [
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAEC"}},
            {"type": "text", "text": "hi"}
        ]})];
        assert!(matches!(
            extract_inline_media(&png).as_deref(),
            Ok([InlineMedia::Image(bytes)]) if bytes == &[0, 1, 2]
        ));
    }

    #[test]
    fn text_content_joins_parts_like_legacy() {
        assert_eq!(
            extract_text_content(Some(&json!([
                {"type": "text", "text": "a"},
                "b",
                {"type": "image_url", "text": "x"},
                {"type": "text", "text": "c"}
            ]))),
            Some("a\nc".to_string())
        );
        assert_eq!(extract_text_content(Some(&json!(null))), None);
        assert_eq!(
            extract_text_content(Some(&json!([{"type": "text", "text": " "}]))),
            None
        );
        assert_eq!(extract_text_content(Some(&json!(5))), Some("5".to_string()));
    }

    #[derive(Default)]
    struct Recorder {
        deltas: std::sync::Mutex<Vec<String>>,
        report_updates: std::sync::Mutex<u32>,
    }

    impl EngineObserver for Recorder {
        fn model_load_progress(&self, _progress: crate::engine::ModelLoadProgress) {}
        fn gpu_fallback(&self) {}
    }

    impl GenerationObserver for Recorder {
        fn delta(&self, text: &str) {
            self.deltas.lock().expect("deltas").push(text.to_string());
        }
        fn reasoning(&self, _text: &str) {}
        fn tool_calls(&self, _calls: &[LocalToolCall]) {}
        fn heartbeat(&self, _heartbeat: GenerationHeartbeat) {}
        fn notice(&self, _notice: LlamaNotice) {}
        fn runtime_report_updated(&self, _model_path: &str) {
            *self.report_updates.lock().expect("updates") += 1;
        }
    }

    #[derive(Default)]
    struct MemoryReports(std::sync::Mutex<Option<Value>>);

    impl RuntimeReportStore for MemoryReports {
        fn load(&self, _model_path: &str) -> Result<Option<Value>, String> {
            Ok(self.0.lock().expect("report").clone())
        }
        fn store(&self, _model_path: &str, report: &Value) -> Result<bool, String> {
            *self.0.lock().expect("report") = Some(report.clone());
            Ok(true)
        }
    }

    fn run_blocking(
        runtime: &LlamaRuntime,
        request: LlamaGenerationRequest,
        observer: Arc<Recorder>,
        reports: Arc<MemoryReports>,
    ) -> Result<LlamaGenerationOutput, LlamaGenerationError> {
        let (sender, receiver) = mpsc::channel();
        runtime.generate(
            request,
            observer,
            reports,
            Box::new(move |result| {
                let _ = sender.send(result);
            }),
        );
        receiver.recv().expect("result")
    }

    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn generates_on_cpu_and_reuses_the_prompt_cache() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let runtime = LlamaRuntime::start().expect("worker");
        let reports = Arc::new(MemoryReports::default());
        let base = LlamaGenerationRequest {
            request_id: Some("smoke".into()),
            model_path: path.clone(),
            messages: vec![
                json!({"role": "user", "content": "Name the capital of France in one word. /no_think"}),
            ],
            stream: true,
            prompt_cache_key: Some("conversation-1".into()),
            max_tokens: Some(24),
            context_length: Some(2048),
            sampling: crate::request::LlamaSamplingInput {
                temperature: Some(0.0),
                ..Default::default()
            },
            runtime: crate::request::LlamaRuntimeInput {
                gpu_layers: Some(0),
                strict_mode: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let observer = Arc::new(Recorder::default());
        let first = run_blocking(&runtime, base.clone(), observer.clone(), reports.clone())
            .expect("first run");
        eprintln!("first: {:?} {:?}", first.content, first.usage);
        assert!(first.content.contains("Paris"));
        assert!(first.usage.completion_tokens > 0);
        assert_eq!(
            observer.deltas.lock().expect("deltas").concat().trim(),
            first.content
        );
        let report = reports
            .0
            .lock()
            .expect("report")
            .clone()
            .expect("stored report");
        assert_eq!(report["status"], json!("succeeded"));
        assert_eq!(report["backendPathUsed"], json!("cpu"));
        assert!(first.metrics.is_some());

        let mut followup = base.clone();
        followup
            .messages
            .push(json!({"role": "assistant", "content": first.content}));
        followup
            .messages
            .push(json!({"role": "user", "content": "And of Italy? /no_think"}));
        let second = run_blocking(
            &runtime,
            followup,
            Arc::new(Recorder::default()),
            reports.clone(),
        )
        .expect("second run");
        eprintln!("second: {:?} {:?}", second.content, second.usage);
        let report = reports
            .0
            .lock()
            .expect("report")
            .clone()
            .expect("stored report");
        eprintln!(
            "cache: hit={} entries={} bytes={} evictions={}",
            report["promptCacheHit"],
            report["promptCacheEntries"],
            report["promptCacheBytes"],
            report["promptCacheEvictions"]
        );
        assert!(second.content.contains("Rome"));
        let report = reports
            .0
            .lock()
            .expect("report")
            .clone()
            .expect("stored report");
        assert_eq!(report["promptCacheHit"], json!(true));

        let mut split_kv = base.clone();
        split_kv.prompt_cache_key = None;
        split_kv.runtime.kv_type_k = Some("q8_0".into());
        split_kv.runtime.kv_type_v = Some("q4_0".into());
        let with_split_kv = run_blocking(
            &runtime,
            split_kv,
            Arc::new(Recorder::default()),
            reports.clone(),
        )
        .expect("split kv run");
        eprintln!("split kv: {:?}", with_split_kv.content);
        assert!(with_split_kv.content.contains("Paris"));
        let report = reports
            .0
            .lock()
            .expect("report")
            .clone()
            .expect("stored report");
        assert_eq!(report["actualKvTypeUsed"], json!("k=q8_0,v=q4_0"));

        if let Ok(draft) = std::env::var("LETTUCE_MTP_MODEL") {
            let mut mtp = base;
            mtp.prompt_cache_key = None;
            mtp.runtime.mtp_enabled = true;
            mtp.runtime.mtp_model_path = Some(draft);
            let with_mtp = run_blocking(&runtime, mtp, Arc::new(Recorder::default()), reports)
                .expect("mtp run");
            eprintln!("mtp: {:?} {:?}", with_mtp.content, with_mtp.usage);
            assert_eq!(with_mtp.content, first.content);
            assert!(with_mtp.usage.mtp_stats.is_some());
        }
        let (sender, receiver) = mpsc::channel();
        runtime.unload(Box::new(move |result| {
            let _ = sender.send(result);
        }));
        receiver.recv().expect("unload").expect("unloaded");
    }
}
