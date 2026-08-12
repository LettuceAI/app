#[cfg(not(mobile))]
use std::collections::{HashMap, VecDeque};
#[cfg(not(mobile))]
use std::io::Cursor;

#[cfg(not(mobile))]
use base64::Engine as _;
#[cfg(not(mobile))]
use serde_json::{json, Value};
use tauri::AppHandle;
#[cfg(not(mobile))]
use tauri::Emitter;

use crate::api::{ApiRequest, ApiResponse};
#[cfg(not(mobile))]
use crate::chat_manager::provider_adapter::{extract_text_content, parse_data_url};
#[cfg(not(mobile))]
use crate::chat_manager::thinking::{normalize_thinking_content, ThinkingTagStreamParser};
#[cfg(not(mobile))]
use crate::chat_manager::tooling::{
    parse_tool_calls, parse_tool_calls_from_text, strip_tool_call_blocks, ToolCall,
};
#[cfg(not(mobile))]
use crate::chat_manager::types::{ErrorEnvelope, MtpStats, NormalizedEvent, UsageSummary};
#[cfg(not(mobile))]
use crate::transport;
#[cfg(not(mobile))]
use crate::utils::{log_error, log_info, log_warn};

const LOCAL_PROVIDER_ID: &str = "llamacpp";
#[cfg(not(mobile))]
const TOKENIZER_ADD_BOS_METADATA_KEY: &str = "tokenizer.ggml.add_bos_token";

#[cfg(not(mobile))]
mod desktop {
    use super::*;
    pub(super) mod context;
    pub(super) mod engine;
    mod mtp;
    pub(super) mod offload;
    mod prompt;
    mod sampler;

    use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
    use llama_cpp_2::context::LlamaContext;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel};
    use llama_cpp_2::mtmd::{MtmdBitmap, MtmdInputChunks, MtmdInputText};
    use llama_cpp_2::sampling::LlamaSampler;
    use llama_cpp_2::token::LlamaToken;
    use llama_cpp_2::TokenToStringError;
    use llama_cpp_sys_2::{
        llama_flash_attn_type, LLAMA_FLASH_ATTN_TYPE_AUTO, LLAMA_FLASH_ATTN_TYPE_DISABLED,
        LLAMA_FLASH_ATTN_TYPE_ENABLED,
    };
    use std::cell::RefCell;
    use std::num::NonZeroU32;
    use std::path::Path;
    use std::sync::{mpsc, Arc, OnceLock};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use tokio::sync::oneshot::error::TryRecvError;

    use context::{
        compute_cpu_fallback_limits, compute_recommended_context, context_attempt_candidates,
        context_error_detail, get_available_memory_bytes, get_available_vram_bytes,
        is_likely_context_oom_error,
    };
    use engine::{
        consume_kqv_fallback_toast, emit_model_load_complete, emit_model_load_failed,
        emit_model_load_finalizing, fit_model_params, load_engine, measure_mmproj_fit_margins,
        shared_backend, using_rocm_backend, LlamaGpuConfig, NativeFitPlan,
    };
    use offload::{
        context_bucket_upper, estimate_mtp_gpu_reserve_bytes, merge_cached_candidate_layers,
        plan_smart_gpu_offload, reserve_device_vram, select_mtp_gpu_device,
    };
    use prompt::{
        add_bos_label, build_prompt, inject_media_markers, model_tokenizer_add_bos_label,
        model_tokenizer_adds_bos, prompt_add_bos_reason, prompt_mode_label, resolve_prompt_add_bos,
        token_piece_bytes, OpenAICompatPromptOptions,
    };
    use sampler::{
        build_sampler, flash_attention_policy_label, kv_type_label, normalize_sampler_profile,
        offload_kqv_mode_label, sampler_profile_defaults, ResolvedSamplerConfig,
        SamplerProfileDefaults,
    };

    const LLAMA_RUNTIME_REPORT_UPDATED_EVENT: &str = "llama-runtime-report-updated";

    struct HotTextContext {
        mtp_runtime: Option<mtp::MtpRuntime<'static>>,
        context: Option<LlamaContext<'static>>,
        model: Arc<LlamaModel>,
        draft_model: Arc<LlamaModel>,
        model_path: String,
        cache_key: String,
        context_key: String,
        tokens: Vec<LlamaToken>,
        allocated_bytes: usize,
    }

    struct HotTextContextCache {
        entries: VecDeque<HotTextContext>,
        allocated_bytes: usize,
    }

    const HOT_CONTEXT_CACHE_MAX_BYTES: usize = 1024 * 1024 * 1024;

    thread_local! {
        static HOT_TEXT_CONTEXTS: RefCell<HotTextContextCache> = const { RefCell::new(HotTextContextCache {
            entries: VecDeque::new(),
            allocated_bytes: 0,
        }) };
    }

    enum LocalWorkerJob {
        Request {
            app: AppHandle,
            request: ApiRequest,
            response: tokio::sync::oneshot::Sender<Result<ApiResponse, String>>,
        },
        Unload {
            app: AppHandle,
            response: tokio::sync::oneshot::Sender<Result<(), String>>,
        },
    }

    static LOCAL_WORKER: OnceLock<mpsc::Sender<LocalWorkerJob>> = OnceLock::new();

    fn local_worker() -> &'static mpsc::Sender<LocalWorkerJob> {
        LOCAL_WORKER.get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<LocalWorkerJob>();
            std::thread::Builder::new()
                .name("lettuce-llama".to_string())
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        match job {
                            LocalWorkerJob::Request {
                                app,
                                request,
                                response,
                            } => {
                                let _ = response.send(handle_local_request_sync(app, request));
                            }
                            LocalWorkerJob::Unload { app, response } => {
                                discard_hot_context();
                                let _ = response.send(engine::unload_engine(&app));
                            }
                        }
                    }
                    discard_hot_context();
                })
                .expect("failed to start llama.cpp inference worker");
            sender
        })
    }

    pub async fn handle_local_request(
        app: AppHandle,
        request: ApiRequest,
    ) -> Result<ApiResponse, String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        local_worker()
            .send(LocalWorkerJob::Request {
                app,
                request,
                response: response_tx,
            })
            .map_err(|_| "llama.cpp inference worker stopped".to_string())?;
        response_rx
            .await
            .map_err(|_| "llama.cpp inference worker dropped its response".to_string())?
    }

    pub async fn unload_local_engine(app: AppHandle) -> Result<(), String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        local_worker()
            .send(LocalWorkerJob::Unload {
                app,
                response: response_tx,
            })
            .map_err(|_| "llama.cpp inference worker stopped".to_string())?;
        response_rx
            .await
            .map_err(|_| "llama.cpp inference worker dropped its response".to_string())?
    }

    pub(super) fn discard_hot_context() {
        HOT_TEXT_CONTEXTS.with(|slot| {
            let mut cache = slot.borrow_mut();
            cache.entries.clear();
            cache.allocated_bytes = 0;
        });
    }

    fn discard_hot_context_if_model_differs(model_path: &str) {
        HOT_TEXT_CONTEXTS.with(|slot| {
            let mut cache = slot.borrow_mut();
            cache
                .entries
                .retain(|cached| cached.model_path == model_path);
            cache.allocated_bytes = cache
                .entries
                .iter()
                .map(|cached| cached.allocated_bytes)
                .sum();
        });
    }

    fn hot_context_matches_model_path(model_path: &str) -> bool {
        HOT_TEXT_CONTEXTS.with(|slot| {
            slot.borrow()
                .entries
                .iter()
                .any(|cached| cached.model_path == model_path)
        })
    }

    fn cache_eviction_count(
        mut allocated_bytes: usize,
        incoming_bytes: usize,
        entry_bytes: impl IntoIterator<Item = usize>,
        capacity_bytes: usize,
        available_bytes: Option<usize>,
    ) -> usize {
        let mut evictions = 0;
        let mut freed_bytes = 0usize;
        for bytes in entry_bytes {
            let within_capacity = allocated_bytes.saturating_add(incoming_bytes) <= capacity_bytes;
            let within_headroom = available_bytes
                .map(|available| incoming_bytes <= available.saturating_add(freed_bytes))
                .unwrap_or(true);
            if within_capacity && within_headroom {
                break;
            }
            allocated_bytes = allocated_bytes.saturating_sub(bytes);
            freed_bytes = freed_bytes.saturating_add(bytes);
            evictions += 1;
        }
        evictions
    }

    fn prepare_hot_context_capacity(available_bytes: Option<usize>) -> usize {
        HOT_TEXT_CONTEXTS.with(|slot| {
            let mut cache = slot.borrow_mut();
            let expected_bytes = cache
                .entries
                .iter()
                .map(|cached| cached.allocated_bytes)
                .max()
                .unwrap_or(0);
            let evicted = cache_eviction_count(
                cache.allocated_bytes,
                expected_bytes,
                cache.entries.iter().map(|cached| cached.allocated_bytes),
                HOT_CONTEXT_CACHE_MAX_BYTES,
                available_bytes,
            );
            for _ in 0..evicted {
                let cached = cache
                    .entries
                    .pop_front()
                    .expect("eviction count cannot exceed cache entries");
                cache.allocated_bytes =
                    cache.allocated_bytes.saturating_sub(cached.allocated_bytes);
            }
            evicted
        })
    }

    fn hot_context_cache_stats() -> (usize, usize) {
        HOT_TEXT_CONTEXTS.with(|slot| {
            let cache = slot.borrow();
            (cache.entries.len(), cache.allocated_bytes)
        })
    }

    fn take_hot_context(
        model: &Arc<LlamaModel>,
        draft_model: &Arc<LlamaModel>,
        cache_key: &str,
        context_key: &str,
    ) -> Option<(
        LlamaContext<'static>,
        Option<mtp::MtpRuntime<'static>>,
        Vec<LlamaToken>,
    )> {
        HOT_TEXT_CONTEXTS.with(|slot| {
            let mut cache = slot.borrow_mut();
            let index = cache.entries.iter().position(|cached| {
                cached.cache_key == cache_key
                    && cached.context_key == context_key
                    && Arc::ptr_eq(&cached.model, model)
                    && Arc::ptr_eq(&cached.draft_model, draft_model)
            })?;
            let mut cached = cache.entries.remove(index)?;
            cache.allocated_bytes = cache.allocated_bytes.saturating_sub(cached.allocated_bytes);
            let context = cached.context.take()?;
            let mtp_runtime = cached.mtp_runtime.take();
            Some((context, mtp_runtime, cached.tokens))
        })
    }

    fn store_hot_context(
        context: LlamaContext<'_>,
        mtp_runtime: Option<mtp::MtpRuntime<'_>>,
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
        // The contexts only borrow the models stored beside them. The worker is
        // single-threaded, and field order drops both contexts before either Arc.
        let context =
            unsafe { std::mem::transmute::<LlamaContext<'_>, LlamaContext<'static>>(context) };
        let mtp_runtime = mtp_runtime.map(|runtime| unsafe {
            std::mem::transmute::<mtp::MtpRuntime<'_>, mtp::MtpRuntime<'static>>(runtime)
        });
        HOT_TEXT_CONTEXTS.with(|slot| {
            let mut cache = slot.borrow_mut();
            if let Some(index) = cache
                .entries
                .iter()
                .position(|cached| cached.cache_key == cache_key)
            {
                if let Some(replaced) = cache.entries.remove(index) {
                    cache.allocated_bytes = cache
                        .allocated_bytes
                        .saturating_sub(replaced.allocated_bytes);
                }
            }
            let evicted = cache_eviction_count(
                cache.allocated_bytes,
                allocated_bytes,
                cache.entries.iter().map(|cached| cached.allocated_bytes),
                HOT_CONTEXT_CACHE_MAX_BYTES,
                None,
            );
            for _ in 0..evicted {
                let cached = cache
                    .entries
                    .pop_front()
                    .expect("eviction count cannot exceed cache entries");
                cache.allocated_bytes =
                    cache.allocated_bytes.saturating_sub(cached.allocated_bytes);
            }
            cache.allocated_bytes = cache.allocated_bytes.saturating_add(allocated_bytes);
            cache.entries.push_back(HotTextContext {
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
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn text_context_key(
        n_ctx: u32,
        n_batch: u32,
        n_ubatch: Option<u32>,
        n_outputs_max: u32,
        n_threads: Option<u32>,
        n_threads_batch: Option<u32>,
        offload_kqv: Option<bool>,
        swa_full: Option<bool>,
        kv_type: Option<&str>,
        flash_attention: llama_flash_attn_type,
        rope_freq_base: Option<f64>,
        rope_freq_scale: Option<f64>,
        mtp_active: bool,
        mtp_draft_tokens: u32,
    ) -> String {
        format!(
            "ctx={n_ctx};batch={n_batch};ubatch={n_ubatch:?};outputs={n_outputs_max};threads={n_threads:?};threads_batch={n_threads_batch:?};kqv={offload_kqv:?};swa={swa_full:?};kv={};flash={};rope_base={rope_freq_base:?};rope_scale={rope_freq_scale:?};mtp={mtp_active};mtp_n={mtp_draft_tokens}",
            kv_type.unwrap_or("f16"),
            flash_attention_policy_label(flash_attention),
        )
    }

    fn common_token_prefix(left: &[LlamaToken], right: &[LlamaToken]) -> usize {
        left.iter()
            .zip(right.iter())
            .take_while(|(left, right)| left == right)
            .count()
    }

    trait GenerationSampler<Ctx> {
        fn sample_generated_token(&mut self, ctx: &Ctx, idx: i32)
            -> llama_cpp_2::token::LlamaToken;
    }

    impl GenerationSampler<llama_cpp_2::context::LlamaContext<'_>> for LlamaSampler {
        fn sample_generated_token(
            &mut self,
            ctx: &llama_cpp_2::context::LlamaContext<'_>,
            idx: i32,
        ) -> llama_cpp_2::token::LlamaToken {
            self.sample(ctx, idx)
        }
    }

    fn sample_generated_token<S, Ctx>(
        sampler: &mut S,
        ctx: &Ctx,
        idx: i32,
    ) -> llama_cpp_2::token::LlamaToken
    where
        S: GenerationSampler<Ctx>,
    {
        // `sample()` already advances sampler state inside llama-cpp-rs.
        sampler.sample_generated_token(ctx, idx)
    }

    fn runtime_report_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0)
    }

    fn update_runtime_report_field(report: &mut Value, key: &str, value: Value) {
        if let Some(map) = report.as_object_mut() {
            map.insert(key.to_string(), value);
        }
    }

    fn persist_runtime_report(app: &AppHandle, model_path: &str, report: Option<&Value>) {
        match crate::storage_manager::models::model_set_llama_runtime_report(
            app, model_path, report,
        ) {
            Ok(true) => {
                let _ = app.emit(
                    LLAMA_RUNTIME_REPORT_UPDATED_EVENT,
                    json!({
                        "modelPath": model_path,
                        "updatedAt": runtime_report_timestamp_ms(),
                    }),
                );
            }
            Ok(false) => {}
            Err(err) => {
                log_warn(
                    app,
                    "llama_cpp",
                    format!("failed to persist llama runtime report: {}", err),
                );
            }
        }
    }

    fn is_aborted_request_error(message: &str) -> bool {
        message.to_ascii_lowercase().contains("aborted")
    }

    fn check_abort_signal(
        abort_rx: Option<&mut tokio::sync::oneshot::Receiver<()>>,
    ) -> Result<(), String> {
        if let Some(rx) = abort_rx {
            match rx.try_recv() {
                Ok(()) => {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        "llama.cpp request aborted by user",
                    ));
                }
                Err(TryRecvError::Closed) | Err(TryRecvError::Empty) => {}
            }
        }

        Ok(())
    }

    fn parse_flash_attention_policy(body: &Value) -> Option<llama_flash_attn_type> {
        let from_string = body
            .get("llamaFlashAttentionPolicy")
            .or_else(|| body.get("llama_flash_attention_policy"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_ascii_lowercase())
            .and_then(|v| match v.as_str() {
                "auto" => Some(LLAMA_FLASH_ATTN_TYPE_AUTO),
                "enabled" | "enable" | "on" | "true" | "1" => Some(LLAMA_FLASH_ATTN_TYPE_ENABLED),
                "disabled" | "disable" | "off" | "false" | "0" => {
                    Some(LLAMA_FLASH_ATTN_TYPE_DISABLED)
                }
                _ => None,
            });

        if from_string.is_some() {
            return from_string;
        }

        body.get("llamaFlashAttention")
            .or_else(|| body.get("llama_flash_attention"))
            .and_then(|v| v.as_bool())
            .map(|enabled| {
                if enabled {
                    LLAMA_FLASH_ATTN_TYPE_ENABLED
                } else {
                    LLAMA_FLASH_ATTN_TYPE_DISABLED
                }
            })
    }

    fn parse_local_reasoning_format(body: &Value) -> Option<String> {
        if let Some(value) = body.get("reasoning_format").and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        let has_reasoning_config = body.get("reasoning").is_some()
            || body
                .get("reasoning_effort")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty());
        if has_reasoning_config {
            Some("auto".to_string())
        } else {
            None
        }
    }

    fn message_thinking_directive(messages: &[Value]) -> Option<bool> {
        let message = messages
            .iter()
            .rev()
            .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))?;
        let text = match message.get("content")? {
            Value::String(text) => text.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => return None,
        };
        match text
            .split_whitespace()
            .last()?
            .to_ascii_lowercase()
            .as_str()
        {
            "/think" => Some(true),
            "/no_think" => Some(false),
            _ => None,
        }
    }

    fn parse_local_thinking_options(
        body: &Value,
        messages: &[Value],
        reasoning_format: Option<&str>,
    ) -> (bool, Option<String>) {
        let explicit = body
            .get("enable_thinking")
            .and_then(Value::as_bool)
            .or_else(|| {
                body.get("chat_template_kwargs")
                    .and_then(Value::as_object)
                    .and_then(|kwargs| kwargs.get("enable_thinking"))
                    .and_then(Value::as_bool)
            });
        let enable_thinking = message_thinking_directive(messages)
            .or(explicit)
            .unwrap_or_else(|| reasoning_format.is_some());
        let mut kwargs = body
            .get("chat_template_kwargs")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        kwargs.insert("enable_thinking".to_string(), json!(enable_thinking));
        let kwargs = serde_json::to_string(&kwargs).ok();
        (enable_thinking, kwargs)
    }

    fn parse_local_parallel_tool_calls(body: &Value) -> bool {
        body.get("parallel_tool_calls")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn decode_llama_sequence_breaker(value: &str) -> String {
        match value.trim() {
            "\\n" => "\n".to_string(),
            "\\r" => "\r".to_string(),
            "\\t" => "\t".to_string(),
            "\\\"" => "\"".to_string(),
            "\\\\" => "\\".to_string(),
            other => other.to_string(),
        }
    }

    fn local_structured_debug_payload(
        request_id: Option<&String>,
        model_path: &str,
        requested_tool_choice: Option<&Value>,
        prompt_options: &OpenAICompatPromptOptions,
        built_prompt: &prompt::BuiltPrompt,
    ) -> Value {
        let template_result = built_prompt.chat_template_result.as_ref();
        let applied_template_source = built_prompt
            .applied_template_source
            .clone()
            .or_else(|| built_prompt.attempted_template_source.clone());

        json!({
            "requestId": request_id,
            "modelPath": model_path,
            "templateSource": applied_template_source,
            "requestedToolChoice": requested_tool_choice.cloned(),
            "resolvedToolChoice": built_prompt.resolved_tool_choice,
            "reasoningFormat": prompt_options.reasoning_format,
            "parallelToolCalls": prompt_options.parallel_tool_calls,
            "enableThinking": prompt_options.enable_thinking,
            "hasGrammar": template_result.and_then(|result| result.grammar.as_ref()).is_some(),
            "grammarLazy": template_result.map(|result| result.grammar_lazy),
            "grammarTriggerCount": template_result.map(|result| result.grammar_triggers.len()),
            "preservedTokenCount": template_result.map(|result| result.preserved_tokens.len()),
            "additionalStopCount": template_result.map(|result| result.additional_stops.len()),
        })
    }

    fn structured_output_failure(
        app: &AppHandle,
        request_id: Option<&String>,
        model_path: &str,
        requested_tool_choice: Option<&Value>,
        prompt_options: &OpenAICompatPromptOptions,
        built_prompt: &prompt::BuiltPrompt,
        stage: &str,
        error: impl std::fmt::Display,
    ) -> String {
        let payload = json!({
            "stage": stage,
            "error": error.to_string(),
            "structured": local_structured_debug_payload(
                request_id,
                model_path,
                requested_tool_choice,
                prompt_options,
                built_prompt,
            ),
        });

        log_warn(
            app,
            "llama_cpp",
            format!(
                "local structured output failed cleanly at stage={} model={} error={}",
                stage, model_path, error
            ),
        );
        crate::utils::emit_debug(app, "llama_structured_failure", payload);

        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Local llama structured output failed during {}: {}",
                stage, error
            ),
        )
    }

    fn parse_stop_sequences(body: &Value) -> Vec<String> {
        fn parse_value(value: &Value) -> Vec<String> {
            match value {
                Value::String(text) => {
                    if text.is_empty() {
                        Vec::new()
                    } else {
                        vec![text.clone()]
                    }
                }
                Value::Array(values) => values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
                _ => Vec::new(),
            }
        }

        parse_value(
            body.get("stop")
                .or_else(|| body.get("stopSequences"))
                .or_else(|| body.get("stop_sequences"))
                .unwrap_or(&Value::Null),
        )
    }

    struct IncrementalStopMatcher<'a> {
        stop_sequences: &'a [String],
        max_len: usize,
    }

    impl<'a> IncrementalStopMatcher<'a> {
        fn new(stop_sequences: &'a [String]) -> Self {
            Self {
                max_len: stop_sequences
                    .iter()
                    .map(|stop| stop.len())
                    .max()
                    .unwrap_or(0),
                stop_sequences,
            }
        }

        fn find(&self, text: &str, appended_from: usize) -> Option<usize> {
            if self.max_len == 0 {
                return None;
            }
            let search_start = clamp_to_char_boundary(
                text,
                appended_from.saturating_sub(self.max_len.saturating_sub(1)),
            );
            self.stop_sequences
                .iter()
                .filter_map(|stop| {
                    text[search_start..]
                        .find(stop)
                        .map(|index| search_start + index)
                })
                .min()
        }
    }

    const STREAM_EMIT_INTERVAL: Duration = Duration::from_millis(32);
    const STREAM_EMIT_BYTES: usize = 256;

    fn should_flush_stream(
        pending_bytes: usize,
        has_flushed: bool,
        elapsed: Duration,
        force: bool,
    ) -> bool {
        pending_bytes > 0
            && (force
                || !has_flushed
                || pending_bytes >= STREAM_EMIT_BYTES
                || elapsed >= STREAM_EMIT_INTERVAL)
    }

    fn clamp_to_char_boundary(text: &str, index: usize) -> usize {
        let mut clamped = index.min(text.len());
        while clamped > 0 && !text.is_char_boundary(clamped) {
            clamped -= 1;
        }
        clamped
    }

    fn emit_structured_deltas(
        app: &AppHandle,
        request_id: Option<&String>,
        deltas: Vec<String>,
        thinking_parser: &mut ThinkingTagStreamParser,
        streamed_text: &mut String,
    ) -> Result<(), String> {
        for delta_json in deltas {
            let delta_value: Value = serde_json::from_str(&delta_json).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to parse llama.cpp structured delta: {e}"),
                )
            })?;

            if let Some(text) = delta_value.get("content").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    let split = thinking_parser.feed(text);
                    if !split.content.is_empty() {
                        streamed_text.push_str(&split.content);
                        if let Some(id) = request_id {
                            transport::emit_normalized(
                                app,
                                id,
                                NormalizedEvent::Delta {
                                    text: split.content,
                                },
                            );
                        }
                    }
                    if !split.reasoning.is_empty() {
                        if let Some(id) = request_id {
                            transport::emit_normalized(
                                app,
                                id,
                                NormalizedEvent::Reasoning {
                                    text: split.reasoning,
                                },
                            );
                        }
                    }
                }
            }

            let explicit_reasoning = delta_value
                .get("reasoning")
                .or_else(|| delta_value.get("reasoning_content"))
                .or_else(|| delta_value.get("thinking"))
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty());

            if let Some(reasoning) = explicit_reasoning {
                if let Some(id) = request_id {
                    transport::emit_normalized(
                        app,
                        id,
                        NormalizedEvent::Reasoning {
                            text: reasoning.to_string(),
                        },
                    );
                }
            }
        }

        Ok(())
    }

    enum PreparedPrompt {
        Text(Vec<llama_cpp_2::token::LlamaToken>),
        Vision(MtmdInputChunks),
    }

    enum InlineMedia {
        Image(Vec<u8>),
        Audio(Vec<u8>),
    }

    fn extract_inline_media(messages: &[Value]) -> Result<Vec<InlineMedia>, String> {
        let mut media = Vec::new();

        for (message_index, message) in messages.iter().enumerate() {
            let Some(parts) = message.get("content").and_then(|v| v.as_array()) else {
                continue;
            };

            for (part_index, part) in parts.iter().enumerate() {
                match part.get("type").and_then(|v| v.as_str()) {
                    Some("image_url") => {
                        let image_url = part
                            .get("image_url")
                            .and_then(|v| v.as_object())
                            .and_then(|obj| obj.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();

                        if image_url.starts_with("http://") || image_url.starts_with("https://") {
                            return Err(crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!(
                                    "llama.cpp local vision only supports inline data URLs; message {} part {} used remote URL",
                                    message_index, part_index
                                ),
                            ));
                        }

                        let Some((mime_type, data)) = parse_data_url(image_url) else {
                            return Err(crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!(
                                    "Invalid inline image data URL in message {} part {}",
                                    message_index, part_index
                                ),
                            ));
                        };

                        if !mime_type.starts_with("image/") {
                            return Err(crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!(
                                    "llama.cpp local vision only supports image data URLs; got '{}' in message {} part {}",
                                    mime_type, message_index, part_index
                                ),
                            ));
                        }

                        let decoded = base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!(
                                        "Failed to decode inline image in message {} part {}: {}",
                                        message_index, part_index, e
                                    ),
                                )
                            })?;
                        let normalized = if mime_type.eq_ignore_ascii_case("image/png") {
                            decoded
                        } else {
                            let image = image::load_from_memory(&decoded).map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!(
                                        "Failed to decode non-PNG inline image in message {} part {}: {}",
                                        message_index, part_index, e
                                    ),
                                )
                            })?;
                            let mut png_bytes = Cursor::new(Vec::new());
                            image
                                .write_to(&mut png_bytes, image::ImageFormat::Png)
                                .map_err(|e| {
                                    crate::utils::err_msg(
                                        module_path!(),
                                        line!(),
                                        format!(
                                            "Failed to normalize inline image to PNG in message {} part {}: {}",
                                            message_index, part_index, e
                                        ),
                                    )
                                })?;
                            png_bytes.into_inner()
                        };
                        media.push(InlineMedia::Image(normalized));
                    }
                    Some("input_audio") => {
                        let data = part
                            .get("input_audio")
                            .and_then(|v| v.as_object())
                            .and_then(|obj| obj.get("data"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();

                        if data.is_empty() {
                            return Err(crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!(
                                    "Missing inline audio data in message {} part {}",
                                    message_index, part_index
                                ),
                            ));
                        }

                        let decoded = base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!(
                                        "Failed to decode inline audio in message {} part {}: {}",
                                        message_index, part_index, e
                                    ),
                                )
                            })?;
                        media.push(InlineMedia::Audio(decoded));
                    }
                    _ => {}
                }
            }
        }

        Ok(media)
    }

    fn ensure_assistant_role(message: &mut Value) {
        if let Some(object) = message.as_object_mut() {
            object
                .entry("role".to_string())
                .or_insert_with(|| Value::String("assistant".to_string()));
        }
    }

    fn recover_message_from_raw_tool_output(output: &str) -> Option<Value> {
        let tool_calls = parse_tool_calls_from_text(output);
        if tool_calls.is_empty() {
            return None;
        }

        let content = strip_tool_call_blocks(output);
        let tool_calls_value = tool_calls
            .iter()
            .map(|call| {
                json!({
                    "id": call.id,
                    "type": "function",
                    "function": {
                        "name": call.name,
                        "arguments": call.raw_arguments.clone().unwrap_or_else(|| call.arguments.to_string()),
                    }
                })
            })
            .collect::<Vec<_>>();

        Some(json!({
            "role": "assistant",
            "content": content,
            "tool_calls": tool_calls_value,
        }))
    }

    fn decode_mtmd_bitmap(
        mtmd_ctx: &llama_cpp_2::mtmd::MtmdContext,
        bytes: &[u8],
    ) -> Result<MtmdBitmap, String> {
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

    fn handle_local_request_sync(app: AppHandle, req: ApiRequest) -> Result<ApiResponse, String> {
        let prompt_cache_key = req
            .cache_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(ToOwned::to_owned);
        let body = req
            .body
            .as_ref()
            .ok_or_else(|| "llama.cpp request missing body".to_string())?;
        let model_path = body
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "llama.cpp request missing model path".to_string())?;

        if !Path::new(model_path).exists() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("llama.cpp model path not found: {}", model_path),
            ));
        }

        let messages = body
            .get("messages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "llama.cpp request missing messages".to_string())?;
        let tools = body.get("tools").filter(|value| {
            value
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        });
        let tool_choice = body.get("tool_choice");
        let reasoning_format = parse_local_reasoning_format(body);
        let thinking_directive = message_thinking_directive(messages);
        let (enable_thinking, chat_template_kwargs) =
            parse_local_thinking_options(body, messages, reasoning_format.as_deref());
        let openai_compat_options = OpenAICompatPromptOptions {
            enable_thinking,
            chat_template_kwargs,
            parallel_tool_calls: parse_local_parallel_tool_calls(body),
            reasoning_format,
        };
        if let Some(enabled) = thinking_directive {
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "local thinking mode overridden by trailing message directive: enable_thinking={enabled}"
                ),
            );
        }
        let llama_mmproj_path = body
            .get("llamaMmprojPath")
            .or_else(|| body.get("llama_mmproj_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let media_items = extract_inline_media(messages)?;
        let image_count = media_items
            .iter()
            .filter(|m| matches!(m, InlineMedia::Image(_)))
            .count();
        let audio_count = media_items
            .iter()
            .filter(|m| matches!(m, InlineMedia::Audio(_)))
            .count();
        let media_requested = !media_items.is_empty();
        if media_requested && llama_mmproj_path.is_none() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "llama.cpp multimodal requests require `llamaMmprojPath` (or `llama_mmproj_path`) to load the multimodal projector",
            ));
        }
        let prompt_messages_owned = if media_requested {
            Some(inject_media_markers(messages))
        } else {
            None
        };
        let prompt_messages = prompt_messages_owned.as_deref().unwrap_or(messages);

        let sampler_profile = body
            .get("llamaSamplerProfile")
            .or_else(|| body.get("llama_sampler_profile"))
            .and_then(|v| v.as_str())
            .and_then(normalize_sampler_profile);
        let disable_sampler_profile_defaults = body
            .get("llamaDisableSamplerProfileDefaults")
            .or_else(|| body.get("llama_disable_sampler_profile_defaults"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let sampler_order = body
            .get("llamaSamplerOrder")
            .or_else(|| body.get("llama_sampler_order"))
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|stage| stage.to_string()))
                    .collect::<Vec<_>>()
            });
        let sampler_defaults = if disable_sampler_profile_defaults {
            SamplerProfileDefaults {
                name: "custom",
                temperature: 0.8,
                top_p: 0.95,
                top_k: None,
                min_p: None,
                typical_p: None,
                frequency_penalty: None,
                presence_penalty: None,
            }
        } else {
            sampler_profile_defaults(sampler_profile)
        };
        let temperature = body
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(sampler_defaults.temperature);
        let top_p = body
            .get("top_p")
            .and_then(|v| v.as_f64())
            .unwrap_or(sampler_defaults.top_p);
        let min_p = body
            .get("min_p")
            .or_else(|| body.get("minP"))
            .or_else(|| body.get("llamaMinP"))
            .or_else(|| body.get("llama_min_p"))
            .and_then(|v| v.as_f64())
            .or(sampler_defaults.min_p);
        let typical_p = body
            .get("typical_p")
            .or_else(|| body.get("typicalP"))
            .or_else(|| body.get("llamaTypicalP"))
            .or_else(|| body.get("llama_typical_p"))
            .and_then(|v| v.as_f64())
            .or(sampler_defaults.typical_p);
        let repeat_penalty = body
            .get("repeat_penalty")
            .or_else(|| body.get("repeatPenalty"))
            .or_else(|| body.get("llamaRepeatPenalty"))
            .or_else(|| body.get("llama_repeat_penalty"))
            .and_then(|v| v.as_f64())
            .filter(|v| (0.0..=2.0).contains(v));
        let n_pen_range = body
            .get("n_pen_range")
            .or_else(|| body.get("nPenRange"))
            .or_else(|| body.get("penalty_last_n"))
            .or_else(|| body.get("repeat_last_n"))
            .or_else(|| body.get("llamaNPenRange"))
            .or_else(|| body.get("llama_n_pen_range"))
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok())
            .filter(|v| (-1..=262_144).contains(v));
        let dry_multiplier = body
            .get("dry_multiplier")
            .or_else(|| body.get("llamaDryMultiplier"))
            .or_else(|| body.get("llama_dry_multiplier"))
            .and_then(|v| v.as_f64());
        let dry_base = body
            .get("dry_base")
            .or_else(|| body.get("llamaDryBase"))
            .or_else(|| body.get("llama_dry_base"))
            .and_then(|v| v.as_f64());
        let dry_allowed_length = body
            .get("dry_allowed_length")
            .or_else(|| body.get("llamaDryAllowedLength"))
            .or_else(|| body.get("llama_dry_allowed_length"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok());
        let dry_penalty_last_n = body
            .get("dry_penalty_last_n")
            .or_else(|| body.get("llamaDryPenaltyLastN"))
            .or_else(|| body.get("llama_dry_penalty_last_n"))
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok());
        let dry_sequence_breakers = body
            .get("dry_sequence_breakers")
            .or_else(|| body.get("llamaDrySequenceBreakers"))
            .or_else(|| body.get("llama_dry_sequence_breakers"))
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .map(decode_llama_sequence_breaker)
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty());
        let xtc_probability = body
            .get("xtc_probability")
            .or_else(|| body.get("xtcProbability"))
            .or_else(|| body.get("llamaXtcProbability"))
            .or_else(|| body.get("llama_xtc_probability"))
            .and_then(|v| v.as_f64());
        let xtc_threshold = body
            .get("xtc_threshold")
            .or_else(|| body.get("xtcThreshold"))
            .or_else(|| body.get("llamaXtcThreshold"))
            .or_else(|| body.get("llama_xtc_threshold"))
            .and_then(|v| v.as_f64());
        let max_tokens = body
            .get("max_tokens")
            .or_else(|| body.get("max_completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512) as u32;
        let llama_gpu_layers = body
            .get("llamaGpuLayers")
            .or_else(|| body.get("llama_gpu_layers"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok());
        let llama_multi_gpu_enabled = body
            .get("llamaMultiGpuEnabled")
            .or_else(|| body.get("llama_multi_gpu_enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let llama_gpu_device_ids = body
            .get("llamaGpuDeviceIds")
            .or_else(|| body.get("llama_gpu_device_ids"))
            .and_then(|v| v.as_array())
            .map(|items| {
                let mut out = Vec::new();
                for item in items {
                    if let Some(value) = item.as_u64().and_then(|v| usize::try_from(v).ok()) {
                        if !out.contains(&value) {
                            out.push(value);
                        }
                    }
                }
                out
            })
            .unwrap_or_default();
        let llama_gpu_distribution_mode = body
            .get("llamaGpuDistributionMode")
            .or_else(|| body.get("llama_gpu_distribution_mode"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| {
                matches!(
                    s.as_str(),
                    "balanced" | "proportional" | "priority" | "manual"
                )
            });
        let llama_gpu_manual_layers: Vec<(usize, u32)> = body
            .get("llamaGpuManualLayers")
            .or_else(|| body.get("llama_gpu_manual_layers"))
            .and_then(|v| v.as_array())
            .map(|items| {
                let mut out: Vec<(usize, u32)> = Vec::new();
                for item in items {
                    let device_id = item
                        .get("deviceId")
                        .or_else(|| item.get("device_id"))
                        .and_then(|v| v.as_u64())
                        .and_then(|v| usize::try_from(v).ok());
                    let layers = item
                        .get("layers")
                        .and_then(|v| v.as_u64())
                        .and_then(|v| u32::try_from(v).ok());
                    if let (Some(device_id), Some(layers)) = (device_id, layers) {
                        if !out.iter().any(|(id, _)| *id == device_id) {
                            out.push((device_id, layers));
                        }
                    }
                }
                out
            })
            .unwrap_or_default();
        let llama_kv_placement = body
            .get("llamaKvPlacement")
            .or_else(|| body.get("llama_kv_placement"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| matches!(s.as_str(), "auto" | "split" | "systemRam" | "pin"));
        let llama_main_gpu = body
            .get("llamaMainGpu")
            .or_else(|| body.get("llama_main_gpu"))
            .and_then(|v| v.as_u64())
            .and_then(|v| i32::try_from(v).ok());
        let llama_single_gpu_device_id = body
            .get("llamaSingleGpuDeviceId")
            .or_else(|| body.get("llama_single_gpu_device_id"))
            .and_then(|v| v.as_u64())
            .and_then(|v| usize::try_from(v).ok());
        let llama_priority_vram_limit_bytes = body
            .get("llamaPriorityVramLimitBytes")
            .or_else(|| body.get("llama_priority_vram_limit_bytes"))
            .and_then(|v| v.as_u64())
            .filter(|v| *v > 0);
        let top_k = body
            .get("top_k")
            .or_else(|| body.get("topK"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0)
            .or(sampler_defaults.top_k);
        let frequency_penalty = body
            .get("frequency_penalty")
            .and_then(|v| v.as_f64())
            .or(sampler_defaults.frequency_penalty);
        let presence_penalty = body
            .get("presence_penalty")
            .and_then(|v| v.as_f64())
            .or(sampler_defaults.presence_penalty);
        let llama_threads = body
            .get("llamaThreads")
            .or_else(|| body.get("llama_threads"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0);
        let llama_threads_batch = body
            .get("llamaThreadsBatch")
            .or_else(|| body.get("llama_threads_batch"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0);
        let mut llama_batch_size = body
            .get("llamaBatchSize")
            .or_else(|| body.get("llama_batch_size"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0)
            .unwrap_or(512);
        let llama_ubatch_size = body
            .get("llamaUbatchSize")
            .or_else(|| body.get("llama_ubatch_size"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0);
        let llama_compute_batch_size = llama_ubatch_size
            .unwrap_or(llama_batch_size)
            .min(llama_batch_size);
        let llama_seed = body
            .get("llamaSeed")
            .or_else(|| body.get("llama_seed"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok());
        let llama_rope_freq_base = body
            .get("llamaRopeFreqBase")
            .or_else(|| body.get("llama_rope_freq_base"))
            .and_then(|v| v.as_f64());
        let llama_rope_freq_scale = body
            .get("llamaRopeFreqScale")
            .or_else(|| body.get("llama_rope_freq_scale"))
            .and_then(|v| v.as_f64());
        let llama_offload_kqv = body
            .get("llamaOffloadKqv")
            .or_else(|| body.get("llama_offload_kqv"))
            .and_then(|v| v.as_bool());
        let llama_swa_full = body
            .get("llamaSwaFull")
            .or_else(|| body.get("llama_swa_full"))
            .and_then(|v| v.as_bool());
        let llama_mtp_enabled = body
            .get("llamaMtpEnabled")
            .or_else(|| body.get("llama_mtp_enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let llama_mtp_draft_tokens = body
            .get("llamaMtpDraftTokens")
            .or_else(|| body.get("llama_mtp_draft_tokens"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0)
            .unwrap_or(mtp::MTP_DRAFT_DEFAULT)
            .min(mtp::MTP_DRAFT_MAX);
        let llama_mtp_model_path = body
            .get("llamaMtpModelPath")
            .or_else(|| body.get("llama_mtp_model_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let llama_mtp_placement = body
            .get("llamaMtpPlacement")
            .or_else(|| body.get("llama_mtp_placement"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "auto" | "gpu" | "cpu"))
            .unwrap_or_else(|| "auto".to_string());
        let llama_flash_attention_policy = parse_flash_attention_policy(body);
        let llama_kv_type_raw = body
            .get("llamaKvType")
            .or_else(|| body.get("llama_kv_type"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_ascii_lowercase());
        let llama_chat_template_override = body
            .get("llamaChatTemplateOverride")
            .or_else(|| body.get("llama_chat_template_override"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let llama_chat_template_preset = body
            .get("llamaChatTemplatePreset")
            .or_else(|| body.get("llama_chat_template_preset"))
            .or_else(|| body.get("llamaChatTemplate"))
            .or_else(|| body.get("llama_chat_template"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let llama_raw_completion_fallback = body
            .get("llamaRawCompletionFallback")
            .or_else(|| body.get("llama_raw_completion_fallback"))
            .or_else(|| body.get("llamaAllowRawCompletionFallback"))
            .or_else(|| body.get("llama_allow_raw_completion_fallback"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let llama_strict_mode = body
            .get("llamaStrictMode")
            .or_else(|| body.get("llama_strict_mode"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let llama_kv_type = llama_kv_type_raw.as_deref().and_then(|s| match s {
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
        });
        let requested_context = body
            .get("context_length")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0);
        let requested_batch_limit = llama_batch_size;
        let requested_ubatch_limit = llama_ubatch_size;

        let request_id = req.request_id.clone();
        let stream = req.stream.unwrap_or(false);

        log_info(
            &app,
            "llama_cpp",
            format!(
                "local inference start model_path={} stream={} request_id={:?}",
                model_path, stream, request_id
            ),
        );

        let mut abort_rx = request_id.as_ref().map(|id| {
            use tauri::Manager;
            let registry = app.state::<crate::abort_manager::AbortRegistry>();
            registry.register(id.clone())
        });

        let mut output = String::new();
        let mut prompt_tokens = 0u64;
        let mut cached_prompt_tokens = 0u64;
        let mut prompt_cache_hit = false;
        let mut prompt_cache_evictions = 0usize;
        let mut completion_tokens = 0u64;
        let request_started_at = Instant::now();
        let mut first_token_ms: Option<u64> = None;
        let mut generation_elapsed_ms: Option<u64> = None;
        let mut generation_elapsed_seconds: Option<f64> = None;
        let mut native_prompt_eval_ms: Option<f64> = None;
        let mut native_prompt_eval_tokens: Option<u64> = None;
        let mut native_prompt_eval_tps: Option<f64> = None;
        let mut native_draft_prompt_eval_ms: Option<f64> = None;
        let mut native_generation_compute_ms: Option<f64> = None;
        let mut native_generation_tps: Option<f64> = None;
        let mut app_generation_overhead_ms: Option<f64> = None;
        let mut metric_samples: Vec<Value> = Vec::new();
        let mut finish_reason = "stop";
        let mut stream_emitted_len = 0usize;
        let mut final_message = json!({ "role": "assistant", "content": "" });
        let mut failure_stage = "load_engine";
        let mut mtp_stats: Option<MtpStats> = None;
        let mut runtime_report = json!({
            "updatedAt": runtime_report_timestamp_ms(),
            "modelPath": model_path,
            "requestedContext": requested_context,
            "requestedBatchLimit": requested_batch_limit,
            "requestedUbatchLimit": requested_ubatch_limit,
            "requestedGpuLayers": llama_gpu_layers,
            "targetNewTokens": max_tokens,
            "thinkingEnabled": openai_compat_options.enable_thinking,
            "thinkingDirective": thinking_directive.map(|enabled| if enabled { "/think" } else { "/no_think" }),
        });

        const KV_LAYER_RETRY_PREFIX: &str = "__kv_layer_retry__:";
        let mut run_generation = |forced_smart_gpu_layers: Option<u32>| -> Result<(), String> {
            check_abort_signal(abort_rx.as_mut())?;
            failure_stage = "load_engine";
            cached_prompt_tokens = 0;
            prompt_cache_hit = false;
            prompt_cache_evictions = 0;
            native_prompt_eval_ms = None;
            native_prompt_eval_tokens = None;
            native_prompt_eval_tps = None;
            native_draft_prompt_eval_ms = None;
            native_generation_compute_ms = None;
            native_generation_tps = None;
            app_generation_overhead_ms = None;
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
                    update_runtime_report_field(&mut runtime_report, field, json!(null));
                }
            }
            // Planning reads free RAM/VRAM, so a resident model (including the
            // first attempt of a KV-aware retry) must not count against the
            // budget or skew the per-device split of the model replacing it.
            if forced_smart_gpu_layers.is_some() {
                discard_hot_context();
            } else {
                discard_hot_context_if_model_differs(model_path);
            }
            if media_requested {
                discard_hot_context();
            }
            let hot_context_resident = hot_context_matches_model_path(model_path);
            if engine::unload_engine_if_model_differs(&app, model_path)? {
                log_info(
                    &app,
                    "llama_cpp",
                    "unloaded previous llama.cpp model before planning (model changed)",
                );
            } else if forced_smart_gpu_layers.is_some() {
                engine::unload_engine(&app)?;
            }
            // A single-GPU device override forces plain single-GPU behavior on
            // the chosen device and wins over any multi-GPU configuration.
            let multi_gpu_active = llama_multi_gpu_enabled
                && llama_gpu_device_ids.len() >= 2
                && llama_single_gpu_device_id.is_none();
            let native_fit_request = llama_gpu_layers.is_none()
                && forced_smart_gpu_layers.is_none()
                && !multi_gpu_active
                && !llama_mtp_enabled
                && requested_context.is_some();
            if native_fit_request && !hot_context_resident {
                engine::unload_engine(&app)?;
            }
            // KV cache placement (multi-GPU only): drives both planning and the
            // runtime context's offload_kqv. "pin" makes the chosen GPU the main
            // device for shared scratch buffers; under layer split each layer's
            // KV still lives on that layer's device.
            let mut kv_main_gpu: Option<i32> = None;
            let kv_placement_offload_kqv: Option<bool> = if multi_gpu_active {
                match llama_kv_placement.as_deref() {
                    Some("split") => Some(true),
                    Some("systemRam") => Some(false),
                    Some("pin") => {
                        // params.main_gpu is positional within params.devices,
                        // but the UI stores the global ggml device index. A
                        // pinned device outside the selected set falls back to
                        // the distribution default.
                        kv_main_gpu = llama_main_gpu.and_then(|id| {
                            usize::try_from(id).ok().and_then(|id| {
                                llama_gpu_device_ids
                                    .iter()
                                    .position(|dev| *dev == id)
                                    .map(|pos| pos as i32)
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
            } else if llama_offload_kqv.is_some() {
                llama_offload_kqv
            } else if llama_mtp_enabled && !media_requested {
                Some(false)
            } else if using_rocm_backend() {
                Some(false)
            } else {
                None
            };
            let resolved_flash_attention_policy = if let Some(policy) = llama_flash_attention_policy
            {
                policy
            } else if using_rocm_backend() {
                LLAMA_FLASH_ATTN_TYPE_DISABLED
            } else {
                LLAMA_FLASH_ATTN_TYPE_AUTO
            };
            let available_memory_bytes = get_available_memory_bytes();
            let distribution_mode = llama_gpu_distribution_mode
                .clone()
                .unwrap_or_else(|| "balanced".to_string());
            let manual_distribution = multi_gpu_active && distribution_mode == "manual";
            // Per-device free VRAM and manual layer counts, aligned to the selected
            // device-id order so they line up with `params.devices` at load time.
            let per_device_vram_raw = if multi_gpu_active {
                context::get_per_device_free_vram(&llama_gpu_device_ids)
            } else {
                Vec::new()
            };
            let per_device_vram: Vec<(usize, u64, u64)> = if multi_gpu_active {
                context::align_per_device_vram(&llama_gpu_device_ids, &per_device_vram_raw)
            } else {
                Vec::new()
            };
            for (id, free, total) in &per_device_vram {
                if !per_device_vram_raw.iter().any(|(dev, _, _)| dev == id) {
                    log_warn(
                        &app,
                        "llama_cpp",
                        format!(
                            "multi-gpu vram query: device {} not in ggml query results; imputing capacity ({} bytes)",
                            id,
                            (*free).max(*total)
                        ),
                    );
                }
            }
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "multi-gpu vram query: {}",
                    per_device_vram
                        .iter()
                        .map(|(id, free, total)| format!("device={id} free={free} total={total}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            let device_free_aligned: Vec<u64> = if multi_gpu_active {
                per_device_vram.iter().map(|(_, free, _)| *free).collect()
            } else {
                Vec::new()
            };
            // The planning budget is per-device reported capacity rather than
            // current free VRAM (see combined_effective_vram_bytes for the
            // tradeoff); the resident model was already unloaded above so our
            // own memory does not skew the numbers.
            let available_vram_bytes = if multi_gpu_active {
                context::combined_effective_vram_bytes(&per_device_vram)
                    .or_else(get_available_vram_bytes)
            } else if let Some(device_id) = llama_single_gpu_device_id {
                // The override targets one device, often the display GPU where
                // desktop usage never frees; budget from its free VRAM like the
                // default single-GPU path, not from capacity.
                context::get_aligned_per_device_vram(&[device_id])
                    .first()
                    .map(|(_, free, _)| *free)
                    .filter(|free| *free > 0)
                    .or_else(get_available_vram_bytes)
            } else {
                get_available_vram_bytes()
            };
            let manual_layers_aligned: Vec<u32> = if manual_distribution {
                llama_gpu_device_ids
                    .iter()
                    .map(|id| {
                        llama_gpu_manual_layers
                            .iter()
                            .find(|(dev, _)| dev == id)
                            .map(|(_, layers)| *layers)
                            .unwrap_or(0)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let mut multi_gpu_distribution: Option<offload::MultiGpuDistribution> = None;
            let mut effective_gpu_layers = llama_gpu_layers;
            let mut smart_gpu_layer_candidates: Option<Vec<u32>> = None;
            let mut smart_kv_aware_layer_estimate: Option<u32> = None;
            let mut native_fit_plan: Option<NativeFitPlan> = None;
            let cached_runtime_report =
                crate::storage_manager::models::model_get_llama_runtime_report(&app, model_path)
                    .ok()
                    .flatten();

            let backend_supports_gpu_offload = shared_backend()?.supports_gpu_offload();
            if backend_supports_gpu_offload && !manual_distribution {
                if let Some(requested) = llama_gpu_layers {
                    if let Ok(metadata) = offload::load_model_metadata(model_path) {
                        let normalized = metadata.normalize_requested_gpu_layers(requested);
                        if normalized != requested {
                            log_info(
                                &app,
                                "llama_cpp",
                                format!(
                                    "normalized requested GPU layers from {} to {} to include the output layer",
                                    requested, normalized
                                ),
                            );
                        }
                        effective_gpu_layers = Some(normalized);
                    }
                }
            }
            let active_mmproj_path = llama_mmproj_path.as_deref().filter(|_| media_requested);
            let llama_mtp_bundled =
                llama_mtp_enabled && !media_requested && mtp::model_has_mtp(model_path);
            let llama_mtp_external_path =
                if llama_mtp_enabled && !media_requested && !llama_mtp_bundled {
                    llama_mtp_model_path
                        .clone()
                        .or_else(|| mtp::discover_external_mtp(model_path))
                } else {
                    None
                };
            if let Some(ref external) = llama_mtp_external_path {
                log_info(
                    &app,
                    "llama_cpp",
                    format!("MTP external draft model resolved: {}", external),
                );
            }
            let planned_mtp_context = requested_context.unwrap_or(16_384).max(1);
            let mtp_gpu_reserve_bytes = llama_mtp_external_path
                .as_deref()
                .map(|path| {
                    estimate_mtp_gpu_reserve_bytes(
                        path,
                        planned_mtp_context,
                        llama_compute_batch_size,
                        llama_kv_type_raw.as_deref(),
                    )
                })
                .transpose()?
                .unwrap_or(0);
            let mtp_drafter_on_gpu = if !backend_supports_gpu_offload
                || llama_mtp_external_path.is_none()
            {
                false
            } else {
                match llama_mtp_placement.as_str() {
                    "cpu" => false,
                    "gpu" => true,
                    _ => available_vram_bytes
                        .is_some_and(|bytes| mtp_gpu_reserve_bytes < bytes.saturating_mul(9) / 10),
                }
            };
            let mtp_gpu_device_id = if !mtp_drafter_on_gpu {
                None
            } else if multi_gpu_active {
                select_mtp_gpu_device(&llama_gpu_device_ids, &device_free_aligned)
            } else {
                llama_single_gpu_device_id
            };
            let device_free_for_distribution = reserve_device_vram(
                &llama_gpu_device_ids,
                &device_free_aligned,
                mtp_gpu_device_id,
                mtp_gpu_reserve_bytes,
            );
            let sidecar_vram_reserve_bytes = if backend_supports_gpu_offload {
                let mmproj_reserve = active_mmproj_path
                    .and_then(|path| std::fs::metadata(path).ok())
                    .map(|meta| meta.len())
                    .unwrap_or(0);
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
                "deviceIds": llama_gpu_device_ids,
                "singleDeviceId": llama_single_gpu_device_id,
                "distributionMode": distribution_mode,
                "manualLayers": manual_layers_aligned,
                "mainGpu": llama_main_gpu,
                "priorityVramLimitBytes": llama_priority_vram_limit_bytes,
                "kvPlacement": llama_kv_placement,
                "offloadKqv": resolved_offload_kqv,
                "kvType": llama_kv_type_raw,
                "swaFull": llama_swa_full,
                "context": requested_context,
                "batch": llama_batch_size,
                "ubatch": llama_ubatch_size,
                "computeBatch": llama_compute_batch_size,
                "flashAttention": resolved_flash_attention_policy,
                "mmprojPath": active_mmproj_path,
                "mtpEnabled": llama_mtp_enabled,
                "mtpPlacement": llama_mtp_placement,
                "mtpModelPath": llama_mtp_external_path,
                "mtpDraftTokens": llama_mtp_draft_tokens,
                "mtpGpuDeviceId": mtp_gpu_device_id,
                "sidecarVramReserveBytes": sidecar_vram_reserve_bytes,
            })
            .to_string();

            if manual_distribution && backend_supports_gpu_offload {
                // Manual mode: fixed per-GPU layer counts, no smart backoff ladder.
                let manual_total_layers = offload::load_model_metadata(model_path)
                    .ok()
                    .map(|metadata| metadata.offload_layer_count())
                    .unwrap_or_else(|| manual_layers_aligned.iter().copied().sum());
                log_info(
                    &app,
                    "llama_cpp",
                    format!(
                        "multi-gpu manual distribution: devices={:?} manual_layers={:?}",
                        llama_gpu_device_ids, manual_layers_aligned
                    ),
                );
                if manual_layers_aligned.iter().all(|&l| l == 0) {
                    log_warn(
                        &app,
                        "llama_cpp",
                        "multi-gpu manual distribution: all layer counts are zero — llamaGpuManualLayers may be missing or device IDs may not match; falling through to CPU",
                    );
                }
                if let Ok(metadata) = offload::load_model_metadata(model_path) {
                    let gib = 1024.0 * 1024.0 * 1024.0;
                    let bytes_per_layer = metadata
                        .model_size_bytes
                        .checked_div(u64::from(metadata.model_layer_count()))
                        .unwrap_or(0);
                    for (position, layers) in manual_layers_aligned.iter().enumerate() {
                        let projected = bytes_per_layer.saturating_mul(u64::from(*layers));
                        let capacity = per_device_vram
                            .get(position)
                            .map(|(_, free, total)| (*free).max(*total))
                            .unwrap_or(0);
                        if capacity > 0 && projected > capacity {
                            log_warn(
                                &app,
                                "llama_cpp",
                                format!(
                                    "multi-gpu manual distribution: {} layers put ~{:.1} GiB of weights on device {} which reports {:.1} GiB total; the load will likely fail and fall back to CPU",
                                    layers,
                                    projected as f64 / gib,
                                    llama_gpu_device_ids
                                        .get(position)
                                        .copied()
                                        .unwrap_or(position),
                                    capacity as f64 / gib,
                                ),
                            );
                        }
                    }
                }
                let dist = offload::plan_multi_gpu_distribution(
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
                effective_gpu_layers = Some(dist.n_gpu_layers);
                smart_gpu_layer_candidates = None;
                multi_gpu_distribution = Some(dist);
            } else if llama_gpu_layers.is_none()
                && !llama_strict_mode
                && backend_supports_gpu_offload
            {
                let mut smart_offload_plan = plan_smart_gpu_offload(
                    model_path,
                    available_memory_bytes,
                    available_vram_bytes,
                    requested_context,
                    llama_compute_batch_size,
                    resolved_offload_kqv,
                    llama_kv_type_raw.as_deref(),
                    resolved_flash_attention_policy,
                    sidecar_vram_reserve_bytes,
                    llama_mtp_bundled,
                )?;
                // Capture the planner's own KV-aware estimate before any cache
                // merge overwrites it: the GPU-KV context-OOM retry must step
                // down to what fits with KV in VRAM, not to a cached count that
                // may only have "succeeded" by spilling KV to RAM.
                smart_kv_aware_layer_estimate = Some(smart_offload_plan.estimated_gpu_layers);
                let current_context_bucket =
                    context_bucket_upper(smart_offload_plan.planned_context.max(1));
                if let Some(report) = cached_runtime_report.as_ref() {
                    let cached_gpu_layers = report
                        .get("actualGpuLayersUsed")
                        .and_then(|value| value.as_u64())
                        .and_then(|value| u32::try_from(value).ok())
                        .filter(|value| *value > 0);
                    let cached_backend_path = report
                        .get("backendPathUsed")
                        .and_then(|value| value.as_str());
                    let cached_status = report.get("status").and_then(|value| value.as_str());
                    let cached_context_bucket = report
                        .get("smartOffloadPlannedContext")
                        .and_then(|value| value.as_u64())
                        .and_then(|value| u32::try_from(value).ok())
                        .or_else(|| {
                            report
                                .get("requestedContext")
                                .and_then(|value| value.as_u64())
                                .and_then(|value| u32::try_from(value).ok())
                        })
                        .or_else(|| {
                            report
                                .get("actualContextUsed")
                                .and_then(|value| value.as_u64())
                                .and_then(|value| u32::try_from(value).ok())
                        })
                        .map(context_bucket_upper);

                    if let (Some(cached_layers), Some(bucket)) =
                        (cached_gpu_layers, cached_context_bucket)
                    {
                        let cached_planning_config = report
                            .get("smartOffloadPlanningConfig")
                            .and_then(|v| v.as_str());
                        let planning_config_matches =
                            cached_planning_config == Some(smart_offload_planning_config.as_str());
                        let cached_total_layers = report
                            .get("smartOffloadTotalLayers")
                            .and_then(|value| value.as_u64())
                            .and_then(|value| u32::try_from(value).ok());
                        let total_layers_match =
                            cached_total_layers == Some(smart_offload_plan.total_layers);
                        let context_bucket_matches =
                            hot_context_resident || bucket == current_context_bucket;
                        // A run that fell back to RAM KV proves its layer count
                        // does NOT fit with GPU KV; reusing it would repeat the
                        // fallback forever.
                        let cached_kqv_fallback = report
                            .get("kqvFallbackActivated")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let cached_vram_budget =
                            report.get("availableVramBytes").and_then(|v| v.as_u64());
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
                            log_info(
                                &app,
                                "llama_cpp",
                                format!(
                                    "smart gpu offload cache invalidated: config_match={} total_layers_match={} kqv_fallback={} vram_budget_match={}",
                                    planning_config_matches,
                                    total_layers_match,
                                    cached_kqv_fallback,
                                    vram_budget_matches
                                ),
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
                            let merged_candidates = merge_cached_candidate_layers(
                                smart_offload_plan.total_layers,
                                cached_layers,
                                &smart_offload_plan.candidate_gpu_layers,
                            );
                            log_info(
                                &app,
                                "llama_cpp",
                                format!(
                                    "smart gpu offload cache hit: context_bucket={} cached_gpu_layers={} merged_candidates={:?}",
                                    current_context_bucket, cached_layers, merged_candidates
                                ),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "smartOffloadCacheHit",
                                json!(true),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "smartOffloadCachedGpuLayers",
                                json!(cached_layers),
                            );
                            smart_offload_plan.candidate_gpu_layers = merged_candidates;
                            smart_offload_plan.estimated_gpu_layers = cached_layers;
                        }
                    }
                }
                effective_gpu_layers = smart_offload_plan.candidate_gpu_layers.first().copied();
                smart_gpu_layer_candidates = Some(smart_offload_plan.candidate_gpu_layers.clone());
                if multi_gpu_active {
                    // Split mode is always LLAMA_SPLIT_MODE_LAYER, so each layer's
                    // KV lives on that layer's device regardless of placement. Pin
                    // only routes shared scratch buffers to the main GPU; KV cost
                    // is therefore always priced per layer, never as one lump on
                    // the pinned device.
                    multi_gpu_distribution = Some(offload::plan_multi_gpu_distribution(
                        &distribution_mode,
                        &device_free_for_distribution,
                        smart_offload_plan.total_layers,
                        smart_offload_plan.bytes_per_layer,
                        smart_offload_plan.kv_bytes_per_layer,
                        smart_offload_plan.estimated_gpu_layers,
                        None,
                        llama_priority_vram_limit_bytes,
                        Some(&smart_offload_plan.offload_unit_costs),
                    ));
                }
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadTotalLayers",
                    json!(smart_offload_plan.total_layers),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadPlannedContext",
                    json!(smart_offload_plan.planned_context),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadRecommendedContext",
                    json!(smart_offload_plan.recommended_context),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadEstimatedGpuLayers",
                    json!(smart_offload_plan.estimated_gpu_layers),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadCandidateLayers",
                    json!(smart_offload_plan.candidate_gpu_layers.clone()),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadKqvVramReserved",
                    json!(smart_offload_plan.kqv_vram_reserved),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadPlanningKqvMode",
                    json!(smart_offload_plan.planning_offload_kqv),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadEstimatedKvBytes",
                    json!(smart_offload_plan.estimated_kv_bytes),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadSidecarVramReserveBytes",
                    json!(smart_offload_plan.estimated_sidecar_vram_reserve_bytes),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadRuntimeReserveBytes",
                    json!(smart_offload_plan.estimated_runtime_reserve_bytes),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "smartOffloadEffectiveVramBudgetBytes",
                    json!(smart_offload_plan.effective_vram_budget_bytes),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "mtpPlacementRequested",
                    json!(llama_mtp_placement),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "mtpPlacementResolved",
                    json!(if mtp_drafter_on_gpu { "gpu" } else { "cpu" }),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "mtpGpuReserveBytes",
                    json!(mtp_gpu_reserve_bytes),
                );
                update_runtime_report_field(
                    &mut runtime_report,
                    "mtpGpuDeviceId",
                    json!(mtp_gpu_device_id),
                );
                log_info(
                    &app,
                    "llama_cpp",
                    format!(
                        "smart gpu offload plan: total_layers={} planned_ctx={} estimated_gpu_layers={} candidates={:?} planning_offload_kqv={:?} reserve_kqv_vram={} kv_bytes={} sidecar_vram_reserve_bytes={} runtime_reserve_bytes={} effective_vram_budget_bytes={}",
                        smart_offload_plan.total_layers,
                        smart_offload_plan.planned_context,
                        smart_offload_plan.estimated_gpu_layers,
                        smart_offload_plan.candidate_gpu_layers,
                        smart_offload_plan.planning_offload_kqv,
                        smart_offload_plan.kqv_vram_reserved,
                        smart_offload_plan.estimated_kv_bytes,
                        smart_offload_plan.estimated_sidecar_vram_reserve_bytes,
                        smart_offload_plan.estimated_runtime_reserve_bytes,
                        smart_offload_plan.effective_vram_budget_bytes,
                    ),
                );
            } else if llama_gpu_layers.is_none() && !llama_strict_mode {
                log_info(
                    &app,
                    "llama_cpp",
                    "skipping smart gpu offload planning because this backend has no GPU offload support",
                );
            }

            let native_fit_eligible =
                native_fit_request && backend_supports_gpu_offload && !hot_context_resident;
            if native_fit_eligible {
                if let Some(fit_context) = requested_context {
                    let fit_batch = fit_context.min(llama_batch_size).max(1);
                    let mut fit_context_params = LlamaContextParams::default()
                        .with_n_ctx(NonZeroU32::new(fit_context))
                        .with_n_batch(fit_batch)
                        .with_n_outputs_max(1)
                        .with_flash_attention_policy(resolved_flash_attention_policy);
                    if let Some(n_ubatch) = llama_ubatch_size {
                        fit_context_params =
                            fit_context_params.with_n_ubatch(n_ubatch.min(fit_batch));
                    }
                    if let Some(n_threads) = llama_threads {
                        fit_context_params = fit_context_params.with_n_threads(n_threads as i32);
                    }
                    if let Some(n_threads_batch) = llama_threads_batch {
                        fit_context_params =
                            fit_context_params.with_n_threads_batch(n_threads_batch as i32);
                    }
                    if let Some(offload) = resolved_offload_kqv {
                        fit_context_params = fit_context_params.with_offload_kqv(offload);
                    }
                    if let Some(swa_full) = llama_swa_full {
                        fit_context_params = fit_context_params.with_swa_full(swa_full);
                    }
                    if let Some(kv_type) = llama_kv_type {
                        fit_context_params =
                            fit_context_params.with_type_k(kv_type).with_type_v(kv_type);
                    }
                    if let Some(base) = llama_rope_freq_base {
                        fit_context_params = fit_context_params.with_rope_freq_base(base as f32);
                    }
                    if let Some(scale) = llama_rope_freq_scale {
                        fit_context_params = fit_context_params.with_rope_freq_scale(scale as f32);
                    }
                    let fit_devices = llama_single_gpu_device_id
                        .map(|device_id| vec![device_id])
                        .unwrap_or_default();
                    let fit_margins = measure_mmproj_fit_margins(active_mmproj_path, &fit_devices)?;
                    match fit_model_params(
                        model_path,
                        &fit_devices,
                        fit_context_params,
                        &fit_margins,
                        fit_context,
                        llama_mtp_bundled,
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
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitApplied",
                                json!(true),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitContext",
                                json!(plan.n_ctx),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitGpuLayers",
                                json!(plan.n_gpu_layers),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitMarginBytes",
                                json!(fit_margins.iter().sum::<usize>()),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitTensorSplit",
                                json!(plan.tensor_split.clone()),
                            );
                            log_info(
                                &app,
                                "llama_cpp",
                                format!(
                                    "llama.cpp native fit selected context={} gpu_layers={} candidates={:?}",
                                    plan.n_ctx, plan.n_gpu_layers, candidates
                                ),
                            );
                            native_fit_plan = Some(plan);
                        }
                        Err(err) => {
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitApplied",
                                json!(false),
                            );
                            update_runtime_report_field(
                                &mut runtime_report,
                                "nativeFitError",
                                json!(err.clone()),
                            );
                            log_warn(
                                &app,
                                "llama_cpp",
                                format!(
                                    "llama.cpp native fit unavailable; using smart offloader candidates: {err}"
                                ),
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
                            update_runtime_report_field(&mut runtime_report, field, value.clone());
                        }
                    }
                }
                log_info(
                    &app,
                    "llama_cpp",
                    "reusing resident llama.cpp model and context without refitting parameters",
                );
            }

            check_abort_signal(abort_rx.as_mut())?;
            if multi_gpu_active && multi_gpu_distribution.is_none() {
                // Smart planning did not run (explicit layers / strict / no offload):
                // still honor the chosen strategy with whatever total we have.
                let max_gpu_layers = offload::load_model_metadata(model_path)
                    .ok()
                    .map(|metadata| metadata.offload_layer_count())
                    .unwrap_or(u32::MAX);
                multi_gpu_distribution = Some(offload::plan_multi_gpu_distribution(
                    &distribution_mode,
                    &device_free_aligned,
                    max_gpu_layers,
                    0,
                    0,
                    effective_gpu_layers.unwrap_or(0),
                    None,
                    llama_priority_vram_limit_bytes,
                    None,
                ));
            }
            let multi_gpu_tensor_split = multi_gpu_distribution
                .as_ref()
                .map(|dist| dist.tensor_split.clone())
                .unwrap_or_default();
            // KV pin takes precedence over a priority-fill primary GPU.
            let multi_gpu_main_gpu = kv_main_gpu.or_else(|| {
                multi_gpu_distribution
                    .as_ref()
                    .and_then(|dist| dist.main_gpu)
            });

            if let Some(device_id) = llama_single_gpu_device_id {
                log_info(
                    &app,
                    "llama_cpp",
                    format!("single-gpu override active: device={device_id}"),
                );
            }
            if let Some(forced) = forced_smart_gpu_layers {
                log_warn(
                    &app,
                    "llama_cpp",
                    format!(
                        "retrying model load at KV-aware layer estimate {forced} after GPU KV context OOM"
                    ),
                );
                effective_gpu_layers = Some(forced);
                smart_gpu_layer_candidates = None;
            }
            log_info(&app, "llama_cpp", "loading llama.cpp engine/model");
            let engine = load_engine(
                Some(&app),
                request_id.as_deref(),
                model_path,
                effective_gpu_layers,
                smart_gpu_layer_candidates.as_deref(),
                native_fit_plan.as_ref(),
                LlamaGpuConfig {
                    multi_gpu_enabled: multi_gpu_active,
                    device_ids: if multi_gpu_active {
                        llama_gpu_device_ids.clone()
                    } else if let Some(device_id) = llama_single_gpu_device_id {
                        vec![device_id]
                    } else {
                        Vec::new()
                    },
                    device_labels: if multi_gpu_active {
                        let known_devices = context::list_gpu_devices();
                        llama_gpu_device_ids
                            .iter()
                            .map(|id| {
                                known_devices
                                    .iter()
                                    .find(|device| device.index == *id)
                                    .map(|device| {
                                        if device.description.trim().is_empty() {
                                            device.name.clone()
                                        } else {
                                            device.description.clone()
                                        }
                                    })
                                    .unwrap_or_else(|| format!("GPU {id}"))
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
                    main_gpu: if multi_gpu_active {
                        multi_gpu_main_gpu
                    } else {
                        None
                    },
                    distribution_mode: if multi_gpu_active {
                        Some(distribution_mode.clone())
                    } else {
                        None
                    },
                    total_layer_count: if multi_gpu_active {
                        offload::load_model_metadata(model_path)
                            .ok()
                            .map(|metadata| metadata.model_layer_count())
                    } else {
                        None
                    },
                },
                llama_strict_mode,
                active_mmproj_path,
                llama_mtp_bundled,
                llama_mtp_external_path.as_deref(),
                mtp_drafter_on_gpu,
                llama_mtp_placement == "auto",
                mtp_gpu_device_id,
            )?;
            let llama_mtp_draft_model = engine.mtp_model.clone();
            let model = engine.model.as_ref();
            let backend = engine.backend.as_ref();
            let mtmd_ctx = engine.mtmd_ctx.as_ref();
            if media_requested && mtmd_ctx.is_none() {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "llama.cpp multimodal request could not initialize the multimodal projector context",
                ));
            }
            if let Some(mtmd_ctx) = mtmd_ctx {
                if image_count > 0 && !mtmd_ctx.support_vision() {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        "The loaded llama.cpp mmproj/model pair does not support vision input",
                    ));
                }
                if audio_count > 0 && !mtmd_ctx.support_audio() {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        "The loaded llama.cpp mmproj/model pair does not support audio input",
                    ));
                }
            }
            let use_vision = media_requested && mtmd_ctx.is_some();
            let llama_mtp_active = llama_mtp_enabled
                && !use_vision
                && {
                    let capable = llama_mtp_bundled || llama_mtp_draft_model.is_some();
                    if !capable {
                        log_warn(
                            &app,
                            "llama_cpp",
                            "MTP requested but the model has no bundled NextN/MTP layers and no external MTP draft model was found; continuing without MTP",
                        );
                    }
                    capable
                };
            if llama_mtp_enabled && use_vision {
                log_warn(
                    &app,
                    "llama_cpp",
                    "MTP requested but disabled for this request because vision input is active",
                );
                let _ = app.emit(
                    "app://toast",
                    json!({
                        "id": "llama-mtp-vision-disabled",
                        "variant": "warning",
                        "title": "MTP disabled for vision",
                        "description": "MTP is not available for image requests yet, so this run will continue without MTP."
                    }),
                );
            }
            let model_reloaded = engine.model_reloaded;
            let max_ctx = model.n_ctx_train().max(1);
            let backend_path_used = engine.backend_path_used.as_deref().unwrap_or("unknown");
            let gpu_load_fallback_activated = engine.gpu_load_fallback_activated;
            let gpu_load_fallback_reason = engine.gpu_load_fallback_reason.clone();
            let actual_gpu_layers_used = engine.actual_gpu_layers_used;
            let cpu_runtime_active = backend_path_used == "cpu"
                || actual_gpu_layers_used == Some(0)
                || !engine.supports_gpu_offload;
            let runtime_offload_kqv = if backend_path_used == "cpu"
                || actual_gpu_layers_used == Some(0)
                || !engine.supports_gpu_offload
            {
                Some(false)
            } else if let Some(placement) = kv_placement_offload_kqv {
                Some(placement)
            } else if llama_offload_kqv.is_some() {
                llama_offload_kqv
            } else if llama_mtp_enabled && !media_requested {
                Some(false)
            } else if using_rocm_backend() {
                Some(false)
            } else {
                None
            };
            let raw_recommended_ctx = compute_recommended_context(
                model,
                available_memory_bytes,
                available_vram_bytes,
                max_ctx,
                actual_gpu_layers_used.unwrap_or(0),
                runtime_offload_kqv,
                llama_kv_type_raw.as_deref(),
            );
            let recommended_ctx = if cpu_runtime_active {
                compute_cpu_fallback_limits(
                    model,
                    available_memory_bytes,
                    max_ctx,
                    actual_gpu_layers_used.unwrap_or(0),
                    llama_kv_type_raw.as_deref(),
                    None,
                    llama_batch_size,
                )
                .map(|(safe_ctx, _)| safe_ctx)
                .or(raw_recommended_ctx)
            } else {
                raw_recommended_ctx
            };
            let mut ctx_size = if let Some(requested) = requested_context {
                requested.min(max_ctx)
            } else if let Some(recommended) = recommended_ctx {
                if recommended == 0 {
                    return Err(
                        "llama.cpp model likely won't fit in memory. Try a smaller model or set a shorter context.".to_string(),
                    );
                }
                recommended.min(max_ctx).max(1)
            } else {
                max_ctx
            };
            update_runtime_report_field(
                &mut runtime_report,
                "updatedAt",
                json!(runtime_report_timestamp_ms()),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "backendPathUsed",
                json!(backend_path_used),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "gpuLoadFallbackActivated",
                json!(gpu_load_fallback_activated),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "gpuFallbackReason",
                json!(gpu_load_fallback_reason),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "supportsGpuOffload",
                json!(engine.supports_gpu_offload),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "actualGpuLayersUsed",
                json!(actual_gpu_layers_used),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "multiGpuEnabled",
                json!(multi_gpu_active),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "multiGpuDeviceIds",
                json!(if multi_gpu_active {
                    llama_gpu_device_ids.clone()
                } else {
                    vec![]
                }),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "smartOffloadPlanningConfig",
                json!(smart_offload_planning_config),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "singleGpuDeviceId",
                json!(llama_single_gpu_device_id),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "smartGpuLayerFallbackActivated",
                json!(engine.smart_gpu_layer_fallback_activated),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "compiledGpuBackends",
                json!(engine.compiled_gpu_backends.clone()),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "availableMemoryBytes",
                json!(available_memory_bytes),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "availableVramBytes",
                json!(available_vram_bytes),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "llamaMultiGpuEnabled",
                json!(multi_gpu_active),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "selectedGpuDeviceIds",
                json!(if multi_gpu_active {
                    Some(llama_gpu_device_ids.clone())
                } else {
                    None
                }),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "llamaGpuDistributionMode",
                json!(if multi_gpu_active {
                    Some(distribution_mode.clone())
                } else {
                    None
                }),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "llamaKvPlacement",
                json!(if multi_gpu_active {
                    llama_kv_placement
                        .clone()
                        .or_else(|| Some("auto".to_string()))
                } else {
                    None
                }),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "llamaMainGpu",
                json!(if multi_gpu_active {
                    llama_main_gpu
                } else {
                    None
                }),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "gpuDeviceLayerPlacement",
                json!(if multi_gpu_active {
                    multi_gpu_distribution
                        .as_ref()
                        .map(|dist| dist.per_device_layers.clone())
                } else {
                    None
                }),
            );
            update_runtime_report_field(&mut runtime_report, "modelSizeBytes", json!(model.size()));
            update_runtime_report_field(
                &mut runtime_report,
                "recommendedContext",
                json!(recommended_ctx),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "strictModeEnabled",
                json!(llama_strict_mode),
            );
            if model_reloaded {
                emit_model_load_finalizing(
                    &app,
                    request_id.as_deref(),
                    model_path,
                    Some(backend_path_used),
                    gpu_load_fallback_activated,
                );
            }
            if !llama_strict_mode && cpu_runtime_active {
                if let Some((safe_ctx, safe_batch)) = compute_cpu_fallback_limits(
                    model,
                    available_memory_bytes,
                    max_ctx,
                    actual_gpu_layers_used.unwrap_or(0),
                    llama_kv_type_raw.as_deref(),
                    requested_context,
                    llama_batch_size,
                ) {
                    if ctx_size > safe_ctx {
                        log_warn(
                            &app,
                            "llama_cpp",
                            format!(
                                "{} clamping context from {} to {} using RAM-derived fallback limits (requested_context={:?}, recommended_context={:?})",
                                if gpu_load_fallback_activated {
                                    "GPU load fell back to CPU;"
                                } else {
                                    "CPU runtime active;"
                                },
                                ctx_size,
                                safe_ctx,
                                requested_context,
                                recommended_ctx
                            ),
                        );
                        ctx_size = safe_ctx;
                    }
                    if llama_batch_size > safe_batch {
                        log_warn(
                            &app,
                            "llama_cpp",
                            format!(
                                "{} reducing llama batch size from {} to {} for CPU headroom",
                                if gpu_load_fallback_activated {
                                    "GPU load fell back to CPU;"
                                } else {
                                    "CPU runtime active;"
                                },
                                llama_batch_size,
                                safe_batch
                            ),
                        );
                        llama_batch_size = safe_batch;
                    }
                }
            }
            update_runtime_report_field(
                &mut runtime_report,
                "initialContextCandidate",
                json!(ctx_size),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "initialBatchCandidate",
                json!(ctx_size.min(llama_batch_size).max(1)),
            );
            failure_stage = "build_prompt";
            let built_prompt = build_prompt(
                model,
                prompt_messages,
                llama_chat_template_override.as_deref(),
                llama_chat_template_preset.as_deref(),
                llama_raw_completion_fallback,
                tools,
                tool_choice,
                &openai_compat_options,
            )?;
            if built_prompt.chat_template_result.is_some() {
                crate::utils::emit_debug(
                    &app,
                    "llama_tool_calling",
                    local_structured_debug_payload(
                        request_id.as_ref(),
                        model_path,
                        tool_choice,
                        &openai_compat_options,
                        &built_prompt,
                    ),
                );
            }
            let mut stop_sequences = parse_stop_sequences(body);
            for stop in &built_prompt.additional_stop_sequences {
                if !stop.is_empty() && !stop_sequences.iter().any(|existing| existing == stop) {
                    stop_sequences.push(stop.clone());
                }
            }
            let stop_matcher = IncrementalStopMatcher::new(&stop_sequences);
            if built_prompt.used_raw_completion_fallback {
                log_warn(
                    &app,
                    "llama_cpp",
                    format!(
                        "using raw completion fallback after chat template resolution/application failed; attempted_source={} reason={}",
                        built_prompt
                            .attempted_template_source
                            .as_deref()
                            .unwrap_or("none"),
                        built_prompt
                            .raw_completion_fallback_reason
                            .as_deref()
                            .unwrap_or("unknown")
                    ),
                );
            } else {
                log_info(
                    &app,
                    "llama_cpp",
                    format!(
                        "using llama chat template source={}",
                        built_prompt
                            .applied_template_source
                            .as_deref()
                            .unwrap_or("unknown")
                    ),
                );
                if let Some(diagnostics) = built_prompt.tool_template_diagnostics.as_deref() {
                    log_warn(
                        &app,
                        "llama_cpp",
                        format!(
                            "llama native tool-call template heuristic warning: source={} {}",
                            built_prompt
                                .applied_template_source
                                .as_deref()
                                .unwrap_or("unknown"),
                            diagnostics
                        ),
                    );
                }
            }
            let model_default_add_bos = model_tokenizer_adds_bos(model);
            let prompt_add_bos = resolve_prompt_add_bos(model, built_prompt.prompt_mode);
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "llama prompt tokenization mode={} add_bos={} model_tokenizer_add_bos={} source={} reason={}",
                    prompt_mode_label(built_prompt.prompt_mode),
                    add_bos_label(prompt_add_bos),
                    model_tokenizer_add_bos_label(model_default_add_bos),
                    built_prompt
                        .applied_template_source
                        .as_deref()
                        .or(built_prompt.attempted_template_source.as_deref())
                        .unwrap_or("none"),
                    prompt_add_bos_reason(built_prompt.prompt_mode, model_default_add_bos),
                ),
            );
            let prompt = built_prompt.prompt.clone();
            let prepared_prompt = if use_vision {
                let mtmd_ctx = mtmd_ctx.ok_or_else(|| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        "llama.cpp multimodal context unavailable",
                    )
                })?;
                let mut bitmaps = Vec::with_capacity(media_items.len());
                for (index, item) in media_items.iter().enumerate() {
                    let bitmap = match item {
                        InlineMedia::Image(bytes) => {
                            decode_mtmd_bitmap(mtmd_ctx, bytes).map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!(
                                        "Failed to decode image {} for llama.cpp vision: {}",
                                        index, e
                                    ),
                                )
                            })?
                        }
                        InlineMedia::Audio(bytes) => MtmdBitmap::from_buffer(mtmd_ctx, bytes)
                            .map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!(
                                        "Failed to decode audio {} for llama.cpp: {}",
                                        index, e
                                    ),
                                )
                            })?,
                    };
                    bitmaps.push(bitmap);
                }
                let bitmap_refs: Vec<&MtmdBitmap> = bitmaps.iter().collect();
                let chunks = mtmd_ctx
                    .tokenize(
                        MtmdInputText {
                            text: prompt.clone(),
                            add_special: matches!(prompt_add_bos, AddBos::Always),
                            parse_special: true,
                        },
                        &bitmap_refs,
                    )
                    .map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to tokenize llama.cpp multimodal prompt: {}", e),
                        )
                    })?;
                prompt_tokens = chunks.total_tokens() as u64;
                PreparedPrompt::Vision(chunks)
            } else {
                let tokens = model.str_to_token(&prompt, prompt_add_bos).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to tokenize prompt: {e}"),
                    )
                })?;
                prompt_tokens = tokens.len() as u64;
                PreparedPrompt::Text(tokens)
            };

            let prompt_eval_span = match &prepared_prompt {
                PreparedPrompt::Text(tokens) => tokens.len(),
                PreparedPrompt::Vision(chunks) => usize::try_from(chunks.total_positions())
                    .map_err(|_| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            "llama.cpp multimodal prompt position count overflowed usize",
                        )
                    })?,
            };

            if prompt_eval_span as u32 >= ctx_size {
                return Err(format!(
                    "Prompt is too long for the context window (prompt tokens: {}, context: {}). Reduce messages or lower context length.",
                    prompt_tokens, ctx_size
                ));
            }

            let preferred_offload_kqv = if let Some(explicit) = llama_offload_kqv {
                Some(explicit)
            } else if llama_mtp_enabled && !media_requested {
                Some(false)
            } else if using_rocm_backend() {
                // Conservative ROCm default to avoid driver/device crashes on some AMD stacks.
                Some(false)
            } else if engine.supports_gpu_offload {
                Some(true)
            } else {
                Some(false)
            };
            let requested_ctx_size = ctx_size;
            let initial_batch = ctx_size.min(llama_batch_size).max(1);
            let mut resolved_ctx_size = ctx_size;
            let mut resolved_n_batch = initial_batch;
            let mut resolved_offload_kqv = preferred_offload_kqv;
            let mut kqv_fallback_activated = false;
            let mut context_failures = Vec::new();
            let context_attempts = if llama_strict_mode {
                vec![(ctx_size, initial_batch)]
            } else {
                context_attempt_candidates(
                    ctx_size,
                    prompt_eval_span,
                    requested_context,
                    llama_batch_size,
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
            let can_fallback_kqv_to_ram = !llama_strict_mode && preferred_offload_kqv == Some(true);
            let hot_draft_model = llama_mtp_draft_model
                .clone()
                .unwrap_or_else(|| engine.model.clone());
            let mut attempt_groups: Vec<(Option<bool>, Vec<(u32, u32)>)> = Vec::new();
            if !same_ctx_attempts.is_empty() {
                attempt_groups.push((preferred_offload_kqv, same_ctx_attempts.clone()));
                if can_fallback_kqv_to_ram {
                    attempt_groups.push((Some(false), same_ctx_attempts.clone()));
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
            let mut ctx: Option<_> = None;
            let mut reused_mtp_runtime = None;
            let mut cached_context_tokens = None;
            let mut active_context_key = None;
            failure_stage = "create_context";

            'context_attempt_groups: for (group_index, (attempt_offload_kqv, attempts)) in
                attempt_groups.into_iter().enumerate()
            {
                if group_index > 0
                    && preferred_offload_kqv == Some(true)
                    && attempt_offload_kqv == Some(false)
                {
                    if forced_smart_gpu_layers.is_none() {
                        if let (Some(estimate), Some(actual)) =
                            (smart_kv_aware_layer_estimate, actual_gpu_layers_used)
                        {
                            if estimate > 0 && estimate < actual {
                                return Err(format!("{KV_LAYER_RETRY_PREFIX}{estimate}"));
                            }
                        }
                    }
                    log_warn(
                        &app,
                        "llama_cpp",
                        format!(
                            "Requested context did not fit with GPU KQV offload; retrying with KV cache on RAM (requested_ctx={}, initial_batch={})",
                            requested_ctx_size, initial_batch
                        ),
                    );
                }

                for (attempt_ctx, attempt_batch) in attempts {
                    let attempt_ubatch = llama_ubatch_size.map(|value| value.min(attempt_batch));
                    let n_outputs_max = if llama_mtp_active {
                        llama_mtp_draft_tokens.saturating_add(1).min(attempt_batch)
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
                    if let Some(n_threads) = llama_threads {
                        ctx_params = ctx_params.with_n_threads(n_threads as i32);
                    }
                    if let Some(n_threads_batch) = llama_threads_batch {
                        ctx_params = ctx_params.with_n_threads_batch(n_threads_batch as i32);
                    }
                    if let Some(offload) = attempt_offload_kqv {
                        ctx_params = ctx_params.with_offload_kqv(offload);
                    }
                    if let Some(swa_full) = llama_swa_full {
                        ctx_params = ctx_params.with_swa_full(swa_full);
                    }
                    if let Some(kv_type) = llama_kv_type {
                        ctx_params = ctx_params.with_type_k(kv_type).with_type_v(kv_type);
                    }
                    ctx_params =
                        ctx_params.with_flash_attention_policy(resolved_flash_attention_policy);
                    if llama_mtp_active {
                        ctx_params = ctx_params.with_n_rs_seq(llama_mtp_draft_tokens);
                    }
                    if let Some(base) = llama_rope_freq_base {
                        ctx_params = ctx_params.with_rope_freq_base(base as f32);
                    }
                    if let Some(scale) = llama_rope_freq_scale {
                        ctx_params = ctx_params.with_rope_freq_scale(scale as f32);
                    }

                    log_info(
                        &app,
                        "llama_cpp",
                        format!(
                            "creating context attempt: ctx={} batch={} ubatch={:?} outputs={} gpu_layers={:?} offload_kqv={:?} flash_attention_policy={:?}",
                            attempt_ctx,
                            attempt_batch,
                            attempt_ubatch,
                            n_outputs_max,
                            actual_gpu_layers_used,
                            attempt_offload_kqv,
                            resolved_flash_attention_policy
                        ),
                    );

                    let attempt_context_key = text_context_key(
                        attempt_ctx,
                        attempt_batch,
                        attempt_ubatch,
                        n_outputs_max,
                        llama_threads,
                        llama_threads_batch,
                        attempt_offload_kqv,
                        llama_swa_full,
                        llama_kv_type_raw.as_deref(),
                        resolved_flash_attention_policy,
                        llama_rope_freq_base,
                        llama_rope_freq_scale,
                        llama_mtp_active,
                        llama_mtp_draft_tokens,
                    );
                    if !use_vision {
                        if let Some(cache_key) = prompt_cache_key.as_deref() {
                            if let Some((cached_ctx, cached_mtp, cached_tokens)) = take_hot_context(
                                &engine.model,
                                &hot_draft_model,
                                cache_key,
                                &attempt_context_key,
                            ) {
                                prompt_cache_hit = true;
                                resolved_ctx_size = attempt_ctx;
                                resolved_n_batch = attempt_batch;
                                resolved_offload_kqv = attempt_offload_kqv;
                                reused_mtp_runtime = cached_mtp;
                                cached_context_tokens = Some(cached_tokens);
                                active_context_key = Some(attempt_context_key);
                                ctx = Some(cached_ctx);
                                log_info(
                                    &app,
                                    "llama_cpp",
                                    "reusing hot llama.cpp context for prompt prefix cache",
                                );
                                break 'context_attempt_groups;
                            }
                        }
                    }
                    let context_memory_headroom = if actual_gpu_layers_used.unwrap_or(0) > 0 {
                        get_available_vram_bytes()
                    } else {
                        get_available_memory_bytes()
                    }
                    .and_then(|bytes| usize::try_from(bytes).ok());
                    prompt_cache_evictions = prompt_cache_evictions
                        .saturating_add(prepare_hot_context_capacity(context_memory_headroom));

                    match model.new_context(backend, ctx_params) {
                        Ok(created) => {
                            resolved_ctx_size = attempt_ctx;
                            resolved_n_batch = attempt_batch;
                            resolved_offload_kqv = attempt_offload_kqv;
                            active_context_key = Some(attempt_context_key);
                            kqv_fallback_activated = preferred_offload_kqv == Some(true)
                                && attempt_offload_kqv == Some(false);
                            if kqv_fallback_activated {
                                log_warn(
                                    &app,
                                    "llama_cpp",
                                    format!(
                                        "KQV GPU offload fallback activated: preserving ctx={} with KV cache on RAM",
                                        attempt_ctx
                                    ),
                                );
                            }
                            if (attempt_ctx, attempt_batch) != (ctx_size, initial_batch) {
                                log_warn(
                                    &app,
                                    "llama_cpp",
                                    format!(
                                        "context fallback activated: requested ctx={} batch={} -> using ctx={} batch={}",
                                        ctx_size, initial_batch, attempt_ctx, attempt_batch
                                    ),
                                );
                            }
                            ctx = Some(created);
                            break 'context_attempt_groups;
                        }
                        Err(err) => {
                            let raw_error = err.to_string();
                            let detail = context_error_detail(
                                &raw_error,
                                attempt_ctx,
                                attempt_batch,
                                attempt_offload_kqv,
                                llama_offload_kqv,
                                recommended_ctx,
                                llama_kv_type_raw.as_deref(),
                            );

                            let likely_oom = is_likely_context_oom_error(&raw_error);
                            if !likely_oom {
                                return Err(crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!("Failed to create llama context: {detail}"),
                                ));
                            }

                            context_failures.push(format!(
                                "ctx={} batch={} offload_kqv={} -> {}",
                                attempt_ctx,
                                attempt_batch,
                                offload_kqv_mode_label(attempt_offload_kqv),
                                detail
                            ));
                        }
                    }
                }
            }

            let mut ctx = ctx.ok_or_else(|| {
                let last_detail = context_failures
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "unknown error".to_string());
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to create llama context after {} fallback attempts. Last failure: {}",
                        context_failures.len(),
                        last_detail
                    ),
                )
            })?;
            ctx_size = resolved_ctx_size;
            let n_batch = resolved_n_batch;
            let n_ubatch = ctx.n_ubatch();
            let context_fallback_activated =
                (ctx_size, n_batch) != (requested_ctx_size, initial_batch);

            let mut mtp_runtime = if let Some(runtime) = reused_mtp_runtime {
                Some(runtime)
            } else if llama_mtp_active {
                let mut draft_params = LlamaContextParams::default()
                    .with_n_ctx(NonZeroU32::new(resolved_ctx_size))
                    .with_n_batch(resolved_n_batch)
                    .with_n_rs_seq(llama_mtp_draft_tokens)
                    .with_flash_attention_policy(resolved_flash_attention_policy);
                if let Some(n_threads) = llama_threads {
                    draft_params = draft_params.with_n_threads(n_threads as i32);
                }
                if let Some(n_threads_batch) = llama_threads_batch {
                    draft_params = draft_params.with_n_threads_batch(n_threads_batch as i32);
                }
                if let Some(offload) = resolved_offload_kqv {
                    draft_params = draft_params.with_offload_kqv(offload);
                }
                if resolved_offload_kqv == Some(false) {
                    // The drafter runs on CPU beside the KV in this mode; ggml's
                    // op offload would otherwise reserve ~194 MiB of GPU compute
                    // (measured) to accelerate ops for a <=draft_n+1 token batch.
                    draft_params = draft_params.with_op_offload(false);
                }
                if let Some(swa_full) = llama_swa_full {
                    draft_params = draft_params.with_swa_full(swa_full);
                }
                if let Some(kv_type) = llama_kv_type {
                    draft_params = draft_params.with_type_k(kv_type).with_type_v(kv_type);
                }
                if let Some(base) = llama_rope_freq_base {
                    draft_params = draft_params.with_rope_freq_base(base as f32);
                }
                if let Some(scale) = llama_rope_freq_scale {
                    draft_params = draft_params.with_rope_freq_scale(scale as f32);
                }

                let mtp_batch = llama_mtp_draft_tokens.max(1) + 1;
                log_info(
                    &app,
                    "llama_cpp",
                    format!(
                        "creating MTP draft context: mode={} ctx={} n_batch={} n_ubatch={} n_outputs_max={} offload_kqv={:?}",
                        if llama_mtp_draft_model.is_some() {
                            "external"
                        } else {
                            "embedded"
                        },
                        resolved_ctx_size,
                        mtp_batch,
                        mtp_batch,
                        mtp_batch,
                        resolved_offload_kqv
                    ),
                );

                match mtp::create_runtime(
                    model,
                    llama_mtp_draft_model.as_deref().unwrap_or(model),
                    &ctx,
                    backend,
                    draft_params,
                    llama_mtp_draft_tokens as usize,
                ) {
                    Ok(mut runtime) => match mtp::enable_nextn_embeddings(&mut ctx, &mut runtime) {
                        Ok(()) => {
                            log_info(
                                &app,
                                "llama_cpp",
                                format!(
                                    "MTP active: mode={} draft_tokens={} ctx={} n_batch={}",
                                    if runtime.shared {
                                        "shared-assistant"
                                    } else {
                                        "embedded"
                                    },
                                    llama_mtp_draft_tokens,
                                    resolved_ctx_size,
                                    runtime.max_batch
                                ),
                            );
                            Some(runtime)
                        }
                        Err(err) => {
                            log_warn(
                                &app,
                                "llama_cpp",
                                format!("MTP setup failed, continuing without MTP: {err}"),
                            );
                            None
                        }
                    },
                    Err(err) => {
                        log_warn(
                            &app,
                            "llama_cpp",
                            format!(
                                "MTP draft context creation failed, continuing without MTP: {err}"
                            ),
                        );
                        None
                    }
                }
            } else {
                None
            };
            if kqv_fallback_activated {
                match consume_kqv_fallback_toast(&app, model_path) {
                    Ok(true) => {
                        let _ = app.emit(
                            "app://toast",
                            json!({
                                "variant": "warning",
                                "title": "KV cache moved to RAM",
                                "description": "Requested context did not fit with GPU KV offload. Continued with RAM-backed KV cache."
                            }),
                        );
                    }
                    Ok(false) => {}
                    Err(err) => {
                        log_warn(
                            &app,
                            "llama_cpp",
                            format!("failed to dedupe KQV fallback toast: {}", err),
                        );
                    }
                }
            }
            let applied_template_source = built_prompt.applied_template_source.clone();
            let applied_template_text = built_prompt.applied_template_text.clone();
            let attempted_template_source = built_prompt.attempted_template_source.clone();
            let attempted_template_text = built_prompt.attempted_template_text.clone();
            let raw_completion_fallback_reason =
                built_prompt.raw_completion_fallback_reason.clone();
            let backend_path_used = engine
                .backend_path_used
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let compiled_gpu_backends = engine.compiled_gpu_backends.clone();
            let supports_gpu_offload = engine.supports_gpu_offload;
            let runtime_settings = json!({
                "requestId": request_id.clone(),
                "modelPath": model_path,
                "prompt": {
                    "mode": prompt_mode_label(built_prompt.prompt_mode),
                    "templateSource": applied_template_source,
                    "templateUsed": applied_template_text,
                    "attemptedTemplateSource": attempted_template_source,
                    "attemptedTemplate": attempted_template_text,
                    "usedRawCompletionFallback": built_prompt.used_raw_completion_fallback,
                    "rawCompletionFallbackReason": raw_completion_fallback_reason,
                    "bosMode": add_bos_label(prompt_add_bos),
                    "bosReason": prompt_add_bos_reason(built_prompt.prompt_mode, model_default_add_bos),
                },
                "runtime": {
                    "requestedContext": requested_context,
                    "initialContextCandidate": requested_ctx_size,
                    "actualContextUsed": ctx_size,
                    "requestedBatchLimit": llama_batch_size,
                    "requestedUbatchLimit": llama_ubatch_size,
                    "initialBatchCandidate": initial_batch,
                    "actualNBatchUsed": n_batch,
                    "actualNUbatchUsed": n_ubatch,
                    "requestedGpuLayers": llama_gpu_layers,
                    "actualGpuLayersUsed": actual_gpu_layers_used,
                    "actualKvTypeUsed": kv_type_label(llama_kv_type_raw.as_deref()),
                    "actualOffloadKqvMode": offload_kqv_mode_label(resolved_offload_kqv),
                    "flashAttentionPolicy": flash_attention_policy_label(resolved_flash_attention_policy),
                    "actualBackendPathUsed": backend_path_used.clone(),
                    "compiledGpuBackends": compiled_gpu_backends,
                    "supportsGpuOffload": supports_gpu_offload,
                    "strictModeEnabled": llama_strict_mode,
                    "gpuLoadFallbackActivated": gpu_load_fallback_activated,
                    "smartGpuLayerFallbackActivated": engine.smart_gpu_layer_fallback_activated,
                    "kqvFallbackActivated": kqv_fallback_activated,
                    "contextFallbackActivated": context_fallback_activated,
                    "mmprojPath": llama_mmproj_path,
                    "mtpRequested": llama_mtp_enabled,
                    "mtpActive": llama_mtp_active,
                    "mtpDraftTokens": llama_mtp_draft_tokens,
                    "mtpSource": if llama_mtp_bundled {
                        "bundled"
                    } else if llama_mtp_draft_model.is_some() {
                        "external"
                    } else {
                        "none"
                    },
                    "visionRequested": media_requested,
                    "visionActive": use_vision,
                    "imageCount": image_count,
                    "audioCount": audio_count,
                }
            });
            update_runtime_report_field(&mut runtime_report, "actualContextUsed", json!(ctx_size));
            update_runtime_report_field(&mut runtime_report, "actualBatchUsed", json!(n_batch));
            update_runtime_report_field(&mut runtime_report, "actualUbatchUsed", json!(n_ubatch));
            update_runtime_report_field(
                &mut runtime_report,
                "actualKvTypeUsed",
                json!(kv_type_label(llama_kv_type_raw.as_deref())),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "actualOffloadKqvMode",
                json!(offload_kqv_mode_label(resolved_offload_kqv)),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "kqvFallbackActivated",
                json!(kqv_fallback_activated),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "flashAttentionPolicy",
                json!(flash_attention_policy_label(
                    resolved_flash_attention_policy
                )),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "contextFallbackActivated",
                json!(context_fallback_activated),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "promptTemplateSource",
                json!(built_prompt
                    .applied_template_source
                    .clone()
                    .or(built_prompt.attempted_template_source.clone())),
            );
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "llama runtime resolved: prompt_mode={} template_source={} fallback_prompt={} bos={} ctx={} n_batch={} n_ubatch={} gpu_layers={:?} kv_type={} offload_kqv={} backend_path={} flash_attention={} smart_gpu_fallback={} kqv_fallback={} context_fallback={}",
                    prompt_mode_label(built_prompt.prompt_mode),
                    built_prompt
                        .applied_template_source
                        .as_deref()
                        .unwrap_or("none"),
                    built_prompt.used_raw_completion_fallback,
                    add_bos_label(prompt_add_bos),
                    ctx_size,
                    n_batch,
                    n_ubatch,
                    actual_gpu_layers_used,
                    kv_type_label(llama_kv_type_raw.as_deref()),
                    offload_kqv_mode_label(resolved_offload_kqv),
                    backend_path_used,
                    flash_attention_policy_label(resolved_flash_attention_policy),
                    engine.smart_gpu_layer_fallback_activated,
                    kqv_fallback_activated,
                    context_fallback_activated,
                ),
            );
            crate::utils::emit_debug(&app, "llama_runtime", runtime_settings);
            if model_reloaded {
                emit_model_load_complete(
                    &app,
                    request_id.as_deref(),
                    model_path,
                    Some(backend_path_used.as_str()),
                    gpu_load_fallback_activated,
                );
            }

            failure_stage = "prompt_evaluation";
            check_abort_signal(abort_rx.as_mut())?;
            ctx.reset_timings();
            if let Some(runtime) = mtp_runtime.as_mut() {
                runtime.draft.reset_timings();
            }
            let batch_size = n_batch as usize;
            let mut batch = LlamaBatch::new(batch_size, 1);
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
                                    .map_err(|e| {
                                        crate::utils::err_msg(
                                            module_path!(),
                                            line!(),
                                            format!("Failed to rewind prompt KV cache: {e}"),
                                        )
                                    })?;
                                if rewind_succeeded {
                                    mtp::reset_for_prompt_reuse(runtime, rewind_from as u32)
                                        .map_err(|e| {
                                            crate::utils::err_msg(module_path!(), line!(), e)
                                        })?;
                                    batch.clear();
                                    batch
                                        .add(
                                            tokens[carry_position],
                                            carry_position as i32,
                                            &[0],
                                            false,
                                        )
                                        .map_err(|e| {
                                            crate::utils::err_msg(
                                                module_path!(),
                                                line!(),
                                                format!(
                                                    "Failed to rebuild prompt-cache carry token: {e}"
                                                ),
                                            )
                                        })?;
                                    ctx.decode(&mut batch).map_err(|e| {
                                        crate::utils::err_msg(
                                            module_path!(),
                                            line!(),
                                            format!(
                                                "Failed to rebuild prompt-cache carry state: {e}"
                                            ),
                                        )
                                    })?;
                                    mtp::set_prefill_carry_from_target(runtime, &ctx, 0).map_err(
                                        |e| crate::utils::err_msg(module_path!(), line!(), e),
                                    )?;
                                    cached_prompt_tokens = carry_position as u64;
                                }
                            } else {
                                rewind_succeeded = ctx
                                    .clear_kv_cache_seq(Some(0), Some(rewind_from as u32), None)
                                    .map_err(|e| {
                                        crate::utils::err_msg(
                                            module_path!(),
                                            line!(),
                                            format!("Failed to rewind prompt KV cache: {e}"),
                                        )
                                    })?;
                                if rewind_succeeded {
                                    mtp::reset_for_prompt_reuse(runtime, rewind_from as u32)
                                        .map_err(|e| {
                                            crate::utils::err_msg(module_path!(), line!(), e)
                                        })?;
                                    cached_prompt_tokens = rewind_from as u64;
                                }
                            }
                        } else {
                            rewind_succeeded = ctx
                                .clear_kv_cache_seq(Some(0), Some(rewind_from as u32), None)
                                .map_err(|e| {
                                    crate::utils::err_msg(
                                        module_path!(),
                                        line!(),
                                        format!("Failed to rewind prompt KV cache: {e}"),
                                    )
                                })?;
                            if rewind_succeeded {
                                cached_prompt_tokens = rewind_from as u64;
                            }
                        }

                        if rewind_succeeded {
                            chunk_start = rewind_from;
                            global_pos = rewind_from as i32;
                        } else {
                            ctx.clear_kv_cache();
                            if let Some(runtime) = mtp_runtime.as_mut() {
                                mtp::reset_for_prompt_reuse(runtime, 0).map_err(|e| {
                                    crate::utils::err_msg(module_path!(), line!(), e)
                                })?;
                            }
                            cached_prompt_tokens = 0;
                            log_warn(
                                &app,
                                "llama_cpp",
                                "prompt KV cache could not be partially rewound; evaluating the full prompt",
                            );
                        }
                    }
                    while chunk_start < tokens_len {
                        check_abort_signal(abort_rx.as_mut())?;
                        let chunk_end = (chunk_start + batch_size).min(tokens_len);
                        batch.clear();
                        for (offset, token) in
                            tokens[chunk_start..chunk_end].iter().copied().enumerate()
                        {
                            let pos = global_pos + offset as i32;
                            let is_last = (chunk_start + offset + 1) == tokens_len;
                            batch.add(token, pos, &[0], is_last).map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!(
                                        "Failed to build llama batch (chunk {}..{} size={} n_batch={}): {e}",
                                        chunk_start, chunk_end, tokens_len, n_batch
                                    ),
                                )
                            })?;
                        }
                        ctx.decode(&mut batch).map_err(|e| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("llama_decode failed during prompt evaluation: {e}"),
                            )
                        })?;
                        if let Some(runtime) = mtp_runtime.as_mut() {
                            mtp::prefill_draft_chunk(
                                runtime,
                                &ctx,
                                &tokens[chunk_start..chunk_end],
                                global_pos,
                                chunk_end == tokens_len,
                            )
                            .map_err(|e| crate::utils::err_msg(module_path!(), line!(), e))?;
                        }
                        check_abort_signal(abort_rx.as_mut())?;
                        global_pos += (chunk_end - chunk_start) as i32;
                        chunk_start = chunk_end;
                    }
                    context_tokens = Some(tokens);
                    batch.n_tokens().saturating_sub(1)
                }
                PreparedPrompt::Vision(chunks) => {
                    check_abort_signal(abort_rx.as_mut())?;
                    let mtmd_ctx = mtmd_ctx.ok_or_else(|| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            "llama.cpp multimodal context unavailable during prompt evaluation",
                        )
                    })?;
                    global_pos = chunks
                        .eval_chunks(mtmd_ctx, &ctx, 0, 0, n_batch as i32, true)
                        .map_err(|e| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("llama.cpp multimodal prompt evaluation failed: {}", e),
                            )
                        })?;
                    check_abort_signal(abort_rx.as_mut())?;
                    -1
                }
            };
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "prompt evaluation complete: prompt_tokens={} prompt_positions={} target_new_tokens={} vision={}",
                    prompt_tokens, global_pos, max_tokens, use_vision
                ),
            );
            update_runtime_report_field(&mut runtime_report, "promptTokens", json!(prompt_tokens));
            update_runtime_report_field(
                &mut runtime_report,
                "cachedPromptTokens",
                json!(cached_prompt_tokens),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "promptPositions",
                json!(u64::try_from(global_pos).ok()),
            );

            let prompt_timings = ctx.timings();
            let prompt_eval_ms = prompt_timings.t_p_eval_ms() + prompt_timings.t_eval_ms();
            let prompt_eval_tokens = i64::from(prompt_timings.n_p_eval())
                .saturating_add(i64::from(prompt_timings.n_eval()))
                .max(0) as u64;
            native_prompt_eval_ms = Some(prompt_eval_ms.max(0.0));
            native_prompt_eval_tokens = Some(prompt_eval_tokens);
            native_prompt_eval_tps = if prompt_eval_ms > 0.0 && prompt_eval_tokens > 0 {
                Some(prompt_eval_tokens as f64 * 1_000.0 / prompt_eval_ms)
            } else {
                None
            };
            native_draft_prompt_eval_ms = mtp_runtime.as_mut().map(|runtime| {
                let timings = runtime.draft.timings();
                (timings.t_p_eval_ms() + timings.t_eval_ms()).max(0.0)
            });
            ctx.reset_timings();
            if let Some(runtime) = mtp_runtime.as_mut() {
                runtime.draft.reset_timings();
            }

            let prompt_len = global_pos;
            let mut n_cur = prompt_len;
            let max_new = max_tokens.min(ctx_size.saturating_sub(n_cur as u32 + 1));

            let sampler_config = ResolvedSamplerConfig {
                profile: sampler_defaults.name,
                order: sampler_order.clone(),
                temperature,
                top_p,
                top_k,
                min_p,
                typical_p,
                repeat_penalty,
                n_pen_range,
                context_size: ctx_size,
                dry_multiplier,
                dry_base,
                dry_allowed_length,
                dry_penalty_last_n,
                dry_sequence_breakers: dry_sequence_breakers.clone(),
                xtc_probability,
                xtc_threshold,
                frequency_penalty,
                presence_penalty,
                seed: llama_seed,
            };
            check_abort_signal(abort_rx.as_mut())?;
            let built_sampler = build_sampler(
                model,
                &sampler_config,
                built_prompt.chat_template_result.as_ref(),
            )
            .map_err(|e| {
                structured_output_failure(
                    &app,
                    request_id.as_ref(),
                    model_path,
                    tool_choice,
                    &openai_compat_options,
                    &built_prompt,
                    "grammar_sampler_init",
                    e,
                )
            })?;
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "llama sampler profile={} order={} active_params={}",
                    sampler_config.profile,
                    built_sampler.order.join(" -> "),
                    built_sampler.active_params,
                ),
            );
            crate::utils::emit_debug(
                &app,
                "llama_sampler",
                json!({
                    "requestId": request_id,
                    "modelPath": model_path,
                    "profile": sampler_config.profile,
                    "requestedOrder": sampler_order,
                    "order": built_sampler.order,
                    "activeParams": built_sampler.active_params,
                }),
            );
            let mut sampler = built_sampler.sampler;
            let mut streamed_thinking_parser = ThinkingTagStreamParser::default();
            let mut structured_parser = if stream && built_prompt.native_tool_parse_supported {
                built_prompt
                    .chat_template_result
                    .as_ref()
                    .map(|result| result.streaming_state_oaicompat())
                    .transpose()
                    .map_err(|e| {
                        structured_output_failure(
                            &app,
                            request_id.as_ref(),
                            model_path,
                            tool_choice,
                            &openai_compat_options,
                            &built_prompt,
                            "structured_parser_init",
                            e,
                        )
                    })?
            } else {
                None
            };
            let mut streamed_structured_text = String::new();
            let mut structured_parsed_len = 0usize;

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
            failure_stage = "generation";
            while n_cur < target_len {
                check_abort_signal(abort_rx.as_mut())?;

                let token = if let Some(runtime) = mtp_runtime.as_mut() {
                    if runtime.pending.is_empty() {
                        let accepted = mtp::mtp_round(
                            &mut ctx,
                            runtime,
                            &mut sampler,
                            model,
                            n_cur,
                            target_len,
                        )
                        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), e))?;
                        runtime.pending.extend(accepted);
                    }
                    match runtime.pending.pop_front() {
                        Some(token) => token,
                        None => break,
                    }
                } else {
                    sample_generated_token(&mut sampler, &ctx, sample_index)
                };

                if model.is_eog_token(token) {
                    reached_eos = true;
                    break;
                }

                let piece_bytes = token_piece_bytes(model, token)?;

                pending_utf8.extend_from_slice(&piece_bytes);
                let mut piece = String::new();

                loop {
                    match std::str::from_utf8(&pending_utf8) {
                        Ok(valid) => {
                            piece.push_str(valid);
                            pending_utf8.clear();
                            break;
                        }
                        Err(err) if err.error_len().is_none() => {
                            break;
                        }
                        Err(err) => {
                            let valid_up_to = err.valid_up_to();
                            if valid_up_to > 0 {
                                let valid = std::str::from_utf8(&pending_utf8[..valid_up_to])
                                    .map_err(|e| {
                                        crate::utils::err_msg(
                                            module_path!(),
                                            line!(),
                                            format!("Failed to decode token prefix: {e}"),
                                        )
                                    })?;
                                piece.push_str(valid);
                                pending_utf8.drain(..valid_up_to);
                                continue;
                            }

                            let invalid_len = err.error_len().unwrap_or(1);
                            piece.push_str(&String::from_utf8_lossy(&pending_utf8[..invalid_len]));
                            pending_utf8.drain(..invalid_len);
                        }
                    }
                }

                if !piece.is_empty() {
                    let appended_from = output.len();
                    output.push_str(&piece);
                    if let Some(stop_index) = stop_matcher.find(&output, appended_from) {
                        output.truncate(stop_index);
                        reached_stop_sequence = true;
                    }

                    if built_prompt.chat_template_result.is_none() {
                        if stream && stream_emitted_len < output.len() {
                            let safe_emit_end = if reached_stop_sequence {
                                output.len()
                            } else if stop_matcher.max_len > 0 {
                                clamp_to_char_boundary(
                                    &output,
                                    output
                                        .len()
                                        .saturating_sub(stop_matcher.max_len.saturating_sub(1)),
                                )
                            } else {
                                output.len()
                            };
                            let pending_bytes = safe_emit_end.saturating_sub(stream_emitted_len);
                            if should_flush_stream(
                                pending_bytes,
                                stream_has_flushed,
                                last_stream_flush_at.elapsed(),
                                reached_stop_sequence,
                            ) {
                                if let Some(ref id) = request_id {
                                    let split = streamed_thinking_parser
                                        .feed(&output[stream_emitted_len..safe_emit_end]);
                                    if !split.content.is_empty() {
                                        transport::emit_normalized(
                                            &app,
                                            id,
                                            NormalizedEvent::Delta {
                                                text: split.content,
                                            },
                                        );
                                    }
                                    if !split.reasoning.is_empty() {
                                        transport::emit_normalized(
                                            &app,
                                            id,
                                            NormalizedEvent::Reasoning {
                                                text: split.reasoning,
                                            },
                                        );
                                    }
                                }
                                stream_emitted_len = safe_emit_end;
                                stream_has_flushed = true;
                                last_stream_flush_at = Instant::now();
                            }
                        }
                    } else if stream {
                        if let Some(parser) = structured_parser.as_mut() {
                            let safe_parse_end = if reached_stop_sequence {
                                output.len()
                            } else if stop_matcher.max_len > 0 {
                                clamp_to_char_boundary(
                                    &output,
                                    output
                                        .len()
                                        .saturating_sub(stop_matcher.max_len.saturating_sub(1)),
                                )
                            } else {
                                output.len()
                            };
                            let pending_bytes =
                                safe_parse_end.saturating_sub(structured_parsed_len);
                            if should_flush_stream(
                                pending_bytes,
                                stream_has_flushed,
                                last_stream_flush_at.elapsed(),
                                reached_stop_sequence,
                            ) {
                                let delta_input = &output[structured_parsed_len..safe_parse_end];
                                let deltas = parser.update(delta_input, true).map_err(|e| {
                                    structured_output_failure(
                                        &app,
                                        request_id.as_ref(),
                                        model_path,
                                        tool_choice,
                                        &openai_compat_options,
                                        &built_prompt,
                                        "structured_stream_parse",
                                        e,
                                    )
                                })?;
                                emit_structured_deltas(
                                    &app,
                                    request_id.as_ref(),
                                    deltas,
                                    &mut streamed_thinking_parser,
                                    &mut streamed_structured_text,
                                )?;
                                structured_parsed_len = safe_parse_end;
                                stream_has_flushed = true;
                                last_stream_flush_at = Instant::now();
                            }
                        }
                    }

                    if reached_stop_sequence {
                        finish_reason = "stop";
                        break;
                    }
                }

                completion_tokens += 1;
                if first_token_ms.is_none() {
                    first_token_ms = Some(request_started_at.elapsed().as_millis() as u64);
                }

                if !heartbeat_emitted || last_heartbeat_at.elapsed().as_secs() >= 1 {
                    heartbeat_emitted = true;
                    last_heartbeat_at = Instant::now();
                    let generation_elapsed = generation_started_at.elapsed();
                    let elapsed_ms = generation_elapsed.as_millis() as u64;
                    let elapsed_seconds = generation_elapsed.as_secs_f64();
                    let has_stable_rate = elapsed_seconds >= 1.0;
                    let tps = if has_stable_rate {
                        (completion_tokens as f64) / elapsed_seconds
                    } else {
                        0.0
                    };
                    let ctx_fill = if ctx_size > 0 {
                        (n_cur as f64) / (ctx_size as f64)
                    } else {
                        0.0
                    };
                    if has_stable_rate {
                        metric_samples.push(json!({
                            "tMs": elapsed_ms,
                            "tokens": completion_tokens,
                            "tps": tps,
                            "ctxFill": ctx_fill,
                        }));
                    }
                    if let Some(ref id) = request_id {
                        let _ = app.emit(
                            "llm-generation-heartbeat",
                            json!({
                                "requestId": id,
                                "tokens": completion_tokens,
                                "elapsedMs": elapsed_ms,
                                "tokensPerSecond": tps,
                                "recentText": output,
                            }),
                        );
                    }
                }

                if mtp_runtime.is_some() {
                    if let Some(tokens) = context_tokens.as_mut() {
                        tokens.push(token);
                    }
                    n_cur += 1;
                    continue;
                }

                batch.clear();
                batch.add(token, n_cur, &[0], true).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to update llama batch: {e}"),
                    )
                })?;
                n_cur += 1;

                ctx.decode(&mut batch).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("llama_decode failed: {e}"),
                    )
                })?;
                if let Some(tokens) = context_tokens.as_mut() {
                    tokens.push(token);
                }
                sample_index = batch.n_tokens() - 1;
            }

            if !pending_utf8.is_empty() {
                let tail = String::from_utf8_lossy(&pending_utf8).to_string();
                let appended_from = output.len();
                output.push_str(&tail);
                if let Some(stop_index) = stop_matcher.find(&output, appended_from) {
                    output.truncate(stop_index);
                    reached_stop_sequence = true;
                    finish_reason = "stop";
                }
            }

            if built_prompt.chat_template_result.is_none()
                && stream
                && stream_emitted_len < output.len()
            {
                if let Some(ref id) = request_id {
                    let split = streamed_thinking_parser.feed(&output[stream_emitted_len..]);
                    if !split.content.is_empty() {
                        transport::emit_normalized(
                            &app,
                            id,
                            NormalizedEvent::Delta {
                                text: split.content,
                            },
                        );
                    }
                    if !split.reasoning.is_empty() {
                        transport::emit_normalized(
                            &app,
                            id,
                            NormalizedEvent::Reasoning {
                                text: split.reasoning,
                            },
                        );
                    }
                }
                stream_emitted_len = output.len();
            }

            let generation_elapsed = generation_started_at.elapsed();
            generation_elapsed_ms = Some(generation_elapsed.as_millis() as u64);
            generation_elapsed_seconds = Some(generation_elapsed.as_secs_f64());
            let target_generation_timings = ctx.timings();
            let target_compute_ms =
                target_generation_timings.t_p_eval_ms() + target_generation_timings.t_eval_ms();
            let draft_compute_ms = mtp_runtime
                .as_mut()
                .map(|runtime| {
                    let timings = runtime.draft.timings();
                    timings.t_p_eval_ms() + timings.t_eval_ms()
                })
                .unwrap_or(0.0);
            let compute_ms = (target_compute_ms + draft_compute_ms).max(0.0);
            native_generation_compute_ms = Some(compute_ms);
            native_generation_tps = if compute_ms > 0.0 && completion_tokens > 0 {
                Some(completion_tokens as f64 * 1_000.0 / compute_ms)
            } else {
                None
            };
            app_generation_overhead_ms =
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
                log_info(
                    &app,
                    "llama_cpp",
                    format!(
                        "MTP stats: rounds={} drafted={} accepted={} tokens_per_round={:.2} draft_acceptance={:.2} configured_draft_n={} final_draft_n={} adaptations={}",
                        runtime.rounds,
                        runtime.drafted,
                        runtime.accepted,
                        tokens_per_round,
                        draft_acceptance,
                        runtime.draft_n_max,
                        runtime.draft_n,
                        runtime.adaptation_count,
                    ),
                );
                let stats = MtpStats {
                    draft_tokens: llama_mtp_draft_tokens,
                    final_draft_tokens: u32::try_from(runtime.draft_n).ok(),
                    adaptation_count: Some(runtime.adaptation_count),
                    rounds: runtime.rounds,
                    drafted: runtime.drafted,
                    accepted: runtime.accepted,
                    tokens_per_round,
                    draft_acceptance,
                };
                if let Ok(value) = serde_json::to_value(&stats) {
                    update_runtime_report_field(&mut runtime_report, "mtpStats", value);
                }
                mtp_stats = Some(stats);
            }

            if let Some(parser) = structured_parser.as_mut() {
                let is_partial = !reached_eos && !reached_stop_sequence;
                let final_input = if structured_parsed_len < output.len() {
                    &output[structured_parsed_len..]
                } else {
                    ""
                };
                let deltas = parser.update(final_input, is_partial).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to finalize llama.cpp structured parse state: {e}"),
                    )
                })?;
                emit_structured_deltas(
                    &app,
                    request_id.as_ref(),
                    deltas,
                    &mut streamed_thinking_parser,
                    &mut streamed_structured_text,
                )?;
            }

            finish_reason = if reached_stop_sequence || reached_eos {
                "stop"
            } else {
                "length"
            };

            let mut final_tool_calls: Vec<ToolCall> = Vec::new();
            let parsed_final_message =
                if let Some(template_result) = built_prompt.chat_template_result.as_ref() {
                    let is_partial = finish_reason == "length";
                    let mut message: Value =
                        if let Some(recovered) = recover_message_from_raw_tool_output(&output) {
                            log_info(
                                &app,
                                "llama_cpp",
                                "using app-level raw tool-call recovery for final llama response",
                            );
                            recovered
                        } else if built_prompt.native_tool_parse_supported {
                            match template_result.parse_response_oaicompat(&output, is_partial) {
                                Ok(parsed_message) => serde_json::from_str(&parsed_message)
                                    .map_err(|e| {
                                        crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Failed to deserialize llama.cpp structured message: {e}"),
                            )
                                    })?,
                                Err(native_err) => {
                                    return Err(structured_output_failure(
                                        &app,
                                        request_id.as_ref(),
                                        model_path,
                                        tool_choice,
                                        &openai_compat_options,
                                        &built_prompt,
                                        "structured_response_parse",
                                        native_err,
                                    ));
                                }
                            }
                        } else {
                            json!({
                                "role": "assistant",
                                "content": output,
                            })
                        };
                    ensure_assistant_role(&mut message);

                    let full_text =
                        extract_text_content(message.get("content")).unwrap_or_default();
                    if stream
                        && full_text.starts_with(&streamed_structured_text)
                        && full_text.len() > streamed_structured_text.len()
                    {
                        if let Some(ref id) = request_id {
                            transport::emit_normalized(
                                &app,
                                id,
                                NormalizedEvent::Delta {
                                    text: full_text[streamed_structured_text.len()..].to_string(),
                                },
                            );
                        }
                    }

                    final_tool_calls = parse_tool_calls(LOCAL_PROVIDER_ID, &message);
                    if !final_tool_calls.is_empty() && finish_reason != "length" {
                        finish_reason = "tool_calls";
                    }
                    crate::utils::emit_debug(
                        &app,
                        "llama_response",
                        json!({
                            "requestId": request_id,
                            "modelPath": model_path,
                            "structured": true,
                            "rawOutput": output,
                            "parsedMessage": message,
                            "toolCallCount": final_tool_calls.len(),
                            "finishReason": finish_reason,
                        }),
                    );
                    message
                } else {
                    crate::utils::emit_debug(
                        &app,
                        "llama_response",
                        json!({
                            "requestId": request_id,
                            "modelPath": model_path,
                            "structured": false,
                            "rawOutput": output,
                            "finishReason": finish_reason,
                        }),
                    );
                    json!({ "role": "assistant", "content": output })
                };

            if stream && !final_tool_calls.is_empty() {
                if let Some(ref id) = request_id {
                    transport::emit_normalized(
                        &app,
                        id,
                        NormalizedEvent::ToolCall {
                            calls: final_tool_calls.clone(),
                        },
                    );
                }
            }

            final_message = parsed_final_message;
            let explicit_reasoning = final_message
                .get("reasoning")
                .or_else(|| final_message.get("reasoning_content"))
                .or_else(|| final_message.get("thinking"))
                .and_then(|value| value.as_str());
            let raw_content = extract_text_content(final_message.get("content"));
            let normalized = normalize_thinking_content(
                raw_content.as_deref().filter(|value| !value.is_empty()),
                explicit_reasoning,
            );
            if let Some(message) = final_message.as_object_mut() {
                message.insert("content".to_string(), json!(normalized.content));
                if normalized.reasoning.is_empty() {
                    message.remove("reasoning");
                    message.remove("reasoning_content");
                    message.remove("thinking");
                } else {
                    message.insert("reasoning".to_string(), json!(normalized.reasoning));
                    message.remove("reasoning_content");
                    message.remove("thinking");
                }
            }
            output = normalized.content;

            if stream {
                if let Some(ref id) = request_id {
                    let tail = streamed_thinking_parser.finish();
                    if !tail.content.is_empty() {
                        transport::emit_normalized(
                            &app,
                            id,
                            NormalizedEvent::Delta { text: tail.content },
                        );
                    }
                    if !tail.reasoning.is_empty() {
                        transport::emit_normalized(
                            &app,
                            id,
                            NormalizedEvent::Reasoning {
                                text: tail.reasoning,
                            },
                        );
                    }
                }
            }

            if let (Some(tokens), Some(context_key)) =
                (context_tokens.take(), active_context_key.take())
            {
                let cache_ready = if llama_mtp_active && mtp_runtime.is_none() {
                    false
                } else if let Some(runtime) = mtp_runtime.as_mut() {
                    match u32::try_from(tokens.len()) {
                        Ok(token_count) => {
                            match mtp::truncate_for_prompt_cache(&mut ctx, runtime, token_count) {
                                Ok(()) => true,
                                Err(err) => {
                                    log_warn(
                                        &app,
                                        "llama_cpp",
                                        format!("discarding unusable prompt cache: {err}"),
                                    );
                                    false
                                }
                            }
                        }
                        Err(_) => false,
                    }
                } else {
                    true
                };
                if cache_ready {
                    if let Some(cache_key) = prompt_cache_key.as_ref() {
                        prompt_cache_evictions =
                            prompt_cache_evictions.saturating_add(store_hot_context(
                                ctx,
                                mtp_runtime.take(),
                                engine.model.clone(),
                                hot_draft_model.clone(),
                                model_path,
                                cache_key.clone(),
                                context_key,
                                tokens,
                            ));
                    }
                }
            }

            Ok(())
        };
        // One-shot retry: when context creation cannot fit the GPU KV cache at
        // the optimistic layer count, rerun the whole load at the KV-aware
        // estimate instead of dropping the entire KV cache to system RAM.
        let mut forced_smart_gpu_layers: Option<u32> = None;
        let result = loop {
            let attempt = run_generation(forced_smart_gpu_layers);
            match attempt {
                Err(err)
                    if forced_smart_gpu_layers.is_none()
                        && err.starts_with(KV_LAYER_RETRY_PREFIX) =>
                {
                    match err[KV_LAYER_RETRY_PREFIX.len()..].parse::<u32>() {
                        Ok(layers) => {
                            forced_smart_gpu_layers = Some(layers);
                        }
                        Err(_) => break Err(err),
                    }
                }
                attempt => break attempt,
            }
        };

        if let Some(ref id) = request_id {
            use tauri::Manager;
            let registry = app.state::<crate::abort_manager::AbortRegistry>();
            registry.unregister(id);
        }

        if let Err(err) = result {
            let request_was_aborted = is_aborted_request_error(&err);
            let failure_status = if request_was_aborted {
                "aborted"
            } else if runtime_report
                .get("gpuLoadFallbackActivated")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                "cpuFallbackFailed"
            } else {
                "failed"
            };
            update_runtime_report_field(
                &mut runtime_report,
                "updatedAt",
                json!(runtime_report_timestamp_ms()),
            );
            update_runtime_report_field(&mut runtime_report, "status", json!(failure_status));
            update_runtime_report_field(&mut runtime_report, "failureStage", json!(failure_stage));
            update_runtime_report_field(&mut runtime_report, "errorMessage", json!(err.clone()));
            update_runtime_report_field(
                &mut runtime_report,
                "completionTokens",
                json!(completion_tokens),
            );
            if request_was_aborted {
                log_info(
                    &app,
                    "llama_cpp",
                    format!("local inference aborted: {}", err),
                );
            } else {
                log_error(&app, "llama_cpp", format!("local inference error: {}", err));
                if !output.is_empty() {
                    log_warn(
                        &app,
                        "llama_cpp",
                        format!("local inference partial output: {}", output),
                    );
                    crate::utils::emit_debug(
                        &app,
                        "llama_response_error",
                        json!({
                            "requestId": request_id,
                            "modelPath": model_path,
                            "failureStage": failure_stage,
                            "error": err,
                            "partialOutput": output,
                        }),
                    );
                }
                persist_runtime_report(&app, model_path, Some(&runtime_report));
                emit_model_load_failed(
                    &app,
                    request_id.as_deref(),
                    model_path,
                    runtime_report
                        .get("backendPathUsed")
                        .and_then(|value| value.as_str()),
                    runtime_report
                        .get("gpuLoadFallbackActivated")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                );
            }
            if stream {
                if let Some(ref id) = request_id {
                    let envelope = ErrorEnvelope {
                        code: Some("LOCAL_INFERENCE_FAILED".into()),
                        message: err.clone(),
                        provider_id: Some(LOCAL_PROVIDER_ID.to_string()),
                        request_id: Some(id.clone()),
                        retryable: Some(false),
                        status: None,
                    };
                    transport::emit_normalized(&app, id, NormalizedEvent::Error { envelope });
                }
            }
            return Err(err);
        }

        let tokens_per_second = generation_elapsed_seconds
            .and_then(|elapsed_seconds| {
                if elapsed_seconds <= 0.0 || completion_tokens == 0 {
                    None
                } else {
                    Some((completion_tokens as f64) / elapsed_seconds)
                }
            })
            .filter(|v| v.is_finite() && *v >= 0.0);
        let (prompt_cache_entries, prompt_cache_bytes) = hot_context_cache_stats();
        update_runtime_report_field(
            &mut runtime_report,
            "promptCacheHit",
            json!(prompt_cache_hit),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "promptCacheEntries",
            json!(prompt_cache_entries),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "promptCacheBytes",
            json!(prompt_cache_bytes),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "promptCacheCapacityBytes",
            json!(HOT_CONTEXT_CACHE_MAX_BYTES),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "promptCacheEvictions",
            json!(prompt_cache_evictions),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "updatedAt",
            json!(runtime_report_timestamp_ms()),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "completionTokens",
            json!(completion_tokens),
        );
        update_runtime_report_field(&mut runtime_report, "finishReason", json!(finish_reason));
        update_runtime_report_field(&mut runtime_report, "firstTokenMs", json!(first_token_ms));
        update_runtime_report_field(
            &mut runtime_report,
            "tokensPerSecond",
            json!(tokens_per_second),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "nativePromptEvalMs",
            json!(native_prompt_eval_ms),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "nativePromptEvalTokens",
            json!(native_prompt_eval_tokens),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "nativePromptEvalTokensPerSecond",
            json!(native_prompt_eval_tps),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "nativeDraftPromptEvalMs",
            json!(native_draft_prompt_eval_ms),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "nativeGenerationComputeMs",
            json!(native_generation_compute_ms),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "nativeGenerationTokensPerSecond",
            json!(native_generation_tps),
        );
        update_runtime_report_field(
            &mut runtime_report,
            "appGenerationOverheadMs",
            json!(app_generation_overhead_ms),
        );
        let fallback_succeeded = runtime_report
            .get("gpuLoadFallbackActivated")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
            && runtime_report
                .get("backendPathUsed")
                .and_then(|value| value.as_str())
                == Some("cpu");
        if fallback_succeeded {
            let suggested_context = runtime_report
                .get("actualContextUsed")
                .cloned()
                .unwrap_or(Value::Null);
            let suggested_batch = runtime_report
                .get("actualBatchUsed")
                .cloned()
                .unwrap_or(Value::Null);
            let suggested_ubatch = runtime_report
                .get("actualUbatchUsed")
                .cloned()
                .unwrap_or(Value::Null);
            update_runtime_report_field(
                &mut runtime_report,
                "status",
                json!("cpuFallbackSucceeded"),
            );
            update_runtime_report_field(
                &mut runtime_report,
                "suggestedSettings",
                json!({
                    "contextLength": suggested_context,
                    "llamaBatchSize": suggested_batch,
                    "llamaUbatchSize": suggested_ubatch,
                }),
            );
            persist_runtime_report(&app, model_path, Some(&runtime_report));
        } else {
            update_runtime_report_field(&mut runtime_report, "status", json!("succeeded"));
            persist_runtime_report(&app, model_path, Some(&runtime_report));
        }

        if completion_tokens > 0 {
            let rr = |key: &str| runtime_report.get(key).cloned().unwrap_or(Value::Null);
            let summary = json!({
                "modelName": model_path,
                "backend": rr("backendPathUsed"),
                "gpuLayers": rr("actualGpuLayersUsed"),
                "nCtx": rr("actualContextUsed"),
                "nBatch": rr("actualBatchUsed"),
                "nUbatch": rr("actualUbatchUsed"),
                "kvType": rr("actualKvTypeUsed"),
                "modelSizeBytes": rr("modelSizeBytes"),
                "promptTokens": prompt_tokens,
                "cachedPromptTokens": cached_prompt_tokens,
                "completionTokens": completion_tokens,
                "totalTokens": prompt_tokens + completion_tokens,
                "ttftMs": first_token_ms,
                "decodeTokensPerSecond": tokens_per_second,
                "generationElapsedMs": generation_elapsed_ms,
                "nativePromptEvalMs": native_prompt_eval_ms,
                "nativePromptEvalTokens": native_prompt_eval_tokens,
                "nativePromptEvalTokensPerSecond": native_prompt_eval_tps,
                "nativeDraftPromptEvalMs": native_draft_prompt_eval_ms,
                "nativeGenerationComputeMs": native_generation_compute_ms,
                "nativeGenerationTokensPerSecond": native_generation_tps,
                "appGenerationOverheadMs": app_generation_overhead_ms,
                "finishReason": finish_reason,
                "mtpStats": rr("mtpStats"),
            });
            let metric_id = request_id
                .clone()
                .unwrap_or_else(|| format!("gen-{}", runtime_report_timestamp_ms()));
            let samples = Value::Array(std::mem::take(&mut metric_samples));
            if let Err(err) = crate::storage_manager::llm_metrics::llm_metrics_insert(
                &app,
                &metric_id,
                Some(model_path),
                &summary,
                &samples,
            ) {
                log_warn(
                    &app,
                    "llama_cpp",
                    format!("failed to persist llm metrics: {}", err),
                );
            }
        }

        if stream {
            if let Some(ref id) = request_id {
                let usage = UsageSummary {
                    prompt_tokens: Some(prompt_tokens),
                    completion_tokens: Some(completion_tokens),
                    total_tokens: Some(prompt_tokens + completion_tokens),
                    cached_prompt_tokens: Some(cached_prompt_tokens),
                    cache_write_tokens: Some(prompt_tokens.saturating_sub(cached_prompt_tokens)),
                    reasoning_tokens: None,
                    image_tokens: None,
                    audio_tokens: None,
                    web_search_requests: None,
                    api_cost: None,
                    response_id: None,
                    first_token_ms,
                    tokens_per_second,
                    finish_reason: Some(finish_reason.into()),
                    mtp_stats: mtp_stats.clone(),
                };
                transport::emit_normalized(&app, id, NormalizedEvent::Usage { usage });
                transport::emit_normalized(&app, id, NormalizedEvent::Done);
            }
        }

        let usage_value = json!({
            "prompt_tokens": prompt_tokens,
            "cached_prompt_tokens": cached_prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
            "first_token_ms": first_token_ms,
            "tokens_per_second": tokens_per_second,
            "mtpStats": mtp_stats,
        });

        let data = json!({
            "id": "local-llama",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": final_message,
                "finish_reason": finish_reason
            }],
            "usage": usage_value,
        });

        Ok(ApiResponse {
            status: 200,
            ok: true,
            headers: HashMap::new(),
            data,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{
            cache_eviction_count, common_token_prefix, message_thinking_directive,
            parse_local_thinking_options, sample_generated_token, should_flush_stream,
            text_context_key, GenerationSampler, IncrementalStopMatcher, STREAM_EMIT_BYTES,
            STREAM_EMIT_INTERVAL,
        };
        use serde_json::{json, Value};
        use std::time::Duration;

        #[derive(Default)]
        struct FakeSampler {
            sample_calls: usize,
            accept_calls: usize,
        }

        struct FakeContext;

        impl FakeSampler {
            fn accept(&mut self, _token: llama_cpp_2::token::LlamaToken) {
                self.accept_calls += 1;
            }
        }

        impl GenerationSampler<FakeContext> for FakeSampler {
            fn sample_generated_token(
                &mut self,
                _ctx: &FakeContext,
                _idx: i32,
            ) -> llama_cpp_2::token::LlamaToken {
                self.sample_calls += 1;
                llama_cpp_2::token::LlamaToken(42)
            }
        }

        #[test]
        fn sample_helper_does_not_require_manual_accept() {
            let ctx = FakeContext;
            let mut sampler = FakeSampler::default();

            let token = sample_generated_token(&mut sampler, &ctx, 7);

            assert_eq!(token, llama_cpp_2::token::LlamaToken(42));
            assert_eq!(sampler.sample_calls, 1);
            assert_eq!(sampler.accept_calls, 0);

            sampler.accept(token);
            assert_eq!(sampler.accept_calls, 1);
        }

        #[test]
        fn prompt_cache_reuses_only_the_contiguous_token_prefix() {
            use llama_cpp_2::token::LlamaToken;

            let cached = [1, 2, 3, 4, 5].map(LlamaToken::new);
            let next = [1, 2, 3, 9, 5].map(LlamaToken::new);

            assert_eq!(common_token_prefix(&cached, &next), 3);
        }

        #[test]
        fn prompt_cache_context_key_tracks_microbatch_size() {
            let key = |ubatch| {
                text_context_key(
                    8192,
                    1024,
                    Some(ubatch),
                    1,
                    None,
                    None,
                    Some(true),
                    None,
                    None,
                    llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO,
                    None,
                    None,
                    false,
                    4,
                )
            };

            assert_ne!(key(512), key(256));
        }

        #[test]
        fn prompt_cache_evicts_oldest_entries_until_incoming_context_fits() {
            assert_eq!(
                cache_eviction_count(900, 400, [300, 300, 300], 1_000, None),
                1
            );
            assert_eq!(
                cache_eviction_count(900, 700, [300, 300, 300], 1_000, None),
                2
            );
        }

        #[test]
        fn prompt_cache_keeps_entries_when_incoming_context_fits() {
            assert_eq!(cache_eviction_count(500, 400, [250, 250], 1_000, None), 0);
        }

        #[test]
        fn prompt_cache_evicts_for_runtime_memory_headroom() {
            assert_eq!(
                cache_eviction_count(500, 400, [250, 250], 1_000, Some(200)),
                1
            );
        }

        #[test]
        fn incremental_stop_matcher_finds_sequence_across_append_boundary() {
            let stops = vec!["END".to_string(), "STOP".to_string()];
            let matcher = IncrementalStopMatcher::new(&stops);

            assert_eq!(matcher.find("hello END", "hello E".len()), Some(6));
        }

        #[test]
        fn stream_coalescer_flushes_first_chunk_latency_and_size_thresholds() {
            assert!(should_flush_stream(1, false, Duration::ZERO, false));
            assert!(!should_flush_stream(1, true, Duration::ZERO, false));
            assert!(should_flush_stream(1, true, STREAM_EMIT_INTERVAL, false));
            assert!(should_flush_stream(
                STREAM_EMIT_BYTES,
                true,
                Duration::ZERO,
                false
            ));
            assert!(should_flush_stream(1, true, Duration::ZERO, true));
        }

        #[test]
        fn trailing_no_think_directive_overrides_reasoning_mode() {
            let messages = vec![json!({
                "role": "user",
                "content": "Answer directly. /no_think"
            })];
            let body = json!({
                "reasoning": { "effort": "high" },
                "chat_template_kwargs": { "custom": 7 }
            });

            assert_eq!(message_thinking_directive(&messages), Some(false));
            let (enabled, kwargs) = parse_local_thinking_options(&body, &messages, Some("auto"));
            assert!(!enabled);
            let kwargs: Value = serde_json::from_str(&kwargs.expect("kwargs")).expect("json");
            assert_eq!(kwargs["enable_thinking"], json!(false));
            assert_eq!(kwargs["custom"], json!(7));
        }

        #[test]
        fn trailing_think_directive_enables_thinking_for_one_turn() {
            let messages = vec![json!({
                "role": "user",
                "content": [{ "type": "text", "text": "Work it out. /think" }]
            })];

            assert_eq!(message_thinking_directive(&messages), Some(true));
        }
    }
}

#[cfg(not(mobile))]
pub(crate) fn available_memory_bytes() -> Option<u64> {
    desktop::context::get_available_memory_bytes()
}

#[cfg(not(mobile))]
pub(crate) fn available_vram_bytes() -> Option<u64> {
    desktop::context::get_available_vram_bytes()
}

#[cfg(mobile)]
pub(crate) fn available_memory_bytes() -> Option<u64> {
    None
}

#[cfg(mobile)]
pub(crate) fn available_vram_bytes() -> Option<u64> {
    None
}

#[cfg(not(mobile))]
pub(crate) fn is_unified_memory() -> bool {
    desktop::context::is_unified_memory()
}

#[cfg(not(mobile))]
pub(crate) fn supports_gpu_offload() -> bool {
    desktop::engine::shared_backend()
        .map(|backend| backend.supports_gpu_offload())
        .unwrap_or(false)
}

#[cfg(mobile)]
pub(crate) fn is_unified_memory() -> bool {
    false
}

#[cfg(mobile)]
pub(crate) fn supports_gpu_offload() -> bool {
    false
}

#[cfg(not(mobile))]
pub use desktop::handle_local_request;
#[cfg(mobile)]
pub async fn handle_local_request(
    _app: AppHandle,
    _req: ApiRequest,
) -> Result<ApiResponse, String> {
    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        "llama.cpp is only supported on desktop builds",
    ))
}

#[tauri::command]
pub async fn llamacpp_context_info(
    app: AppHandle,
    model_path: String,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<String>,
    llama_gpu_layers: Option<u32>,
    llama_multi_gpu_enabled: Option<bool>,
    llama_gpu_device_ids: Option<Vec<usize>>,
    llama_gpu_distribution_mode: Option<String>,
    llama_gpu_manual_layers: Option<Vec<crate::chat_manager::types::GpuLayerAssignment>>,
    llama_main_gpu: Option<i32>,
    llama_single_gpu_device_id: Option<usize>,
    llama_kv_placement: Option<String>,
    llama_priority_vram_limit_bytes: Option<u64>,
    llama_mmproj_path: Option<String>,
    llama_mtp_enabled: Option<bool>,
    llama_mtp_placement: Option<String>,
    llama_mtp_model_path: Option<String>,
) -> Result<serde_json::Value, String> {
    #[cfg(not(mobile))]
    {
        let info = desktop::context::llamacpp_context_info(
            app,
            model_path,
            llama_offload_kqv,
            llama_kv_type,
            llama_gpu_layers,
            llama_multi_gpu_enabled,
            llama_gpu_device_ids,
            llama_gpu_distribution_mode,
            llama_gpu_manual_layers,
            llama_main_gpu,
            llama_single_gpu_device_id,
            llama_kv_placement,
            llama_priority_vram_limit_bytes,
            llama_mmproj_path,
            llama_mtp_enabled,
            llama_mtp_placement,
            llama_mtp_model_path,
        )
        .await?;
        serde_json::to_value(info).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to serialize context info: {e}"),
            )
        })
    }
    #[cfg(mobile)]
    {
        let _ = app;
        let _ = model_path;
        let _ = llama_offload_kqv;
        let _ = llama_kv_type;
        let _ = llama_gpu_layers;
        let _ = llama_multi_gpu_enabled;
        let _ = llama_gpu_device_ids;
        let _ = llama_gpu_distribution_mode;
        let _ = llama_gpu_manual_layers;
        let _ = llama_main_gpu;
        let _ = llama_single_gpu_device_id;
        let _ = llama_kv_placement;
        let _ = llama_priority_vram_limit_bytes;
        let _ = llama_mmproj_path;
        let _ = llama_mtp_enabled;
        let _ = llama_mtp_placement;
        let _ = llama_mtp_model_path;
        Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "llama.cpp is only supported on desktop builds",
        ))
    }
}

#[tauri::command]
pub async fn llamacpp_backend_devices() -> Result<serde_json::Value, String> {
    #[cfg(not(mobile))]
    {
        serde_json::to_value(desktop::context::list_gpu_devices()).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to serialize llama.cpp backend devices: {e}"),
            )
        })
    }
    #[cfg(mobile)]
    {
        Ok(serde_json::Value::Array(Vec::new()))
    }
}

#[tauri::command]
pub async fn llamacpp_embedded_chat_template(
    _app: AppHandle,
    model_path: String,
) -> Result<String, String> {
    #[cfg(not(mobile))]
    {
        use desktop::engine::shared_backend;
        use llama_cpp_2::model::params::LlamaModelParams;
        use llama_cpp_2::model::LlamaModel;
        use std::path::Path;

        if model_path.trim().is_empty() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "llama.cpp model path is empty",
            ));
        }
        if !Path::new(&model_path).exists() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("llama.cpp model path not found: {}", model_path),
            ));
        }

        let backend = shared_backend()?;
        let model = LlamaModel::load_from_file(
            backend.as_ref(),
            &model_path,
            &LlamaModelParams::default().with_n_gpu_layers(0),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load llama model for embedded template read: {e}"),
            )
        })?;

        let template = model.chat_template(None).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("No embedded GGUF chat template found: {e}"),
            )
        })?;

        template.to_string().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to decode embedded GGUF chat template: {e}"),
            )
        })
    }
    #[cfg(mobile)]
    {
        let _ = model_path;
        Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "llama.cpp is only supported on desktop builds",
        ))
    }
}

#[tauri::command]
pub async fn llamacpp_unload(app: AppHandle) -> Result<(), String> {
    #[cfg(not(mobile))]
    {
        desktop::unload_local_engine(app).await
    }
    #[cfg(mobile)]
    {
        let _ = app;
        Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "llama.cpp is only supported on desktop builds",
        ))
    }
}

pub fn is_llama_cpp(provider_id: Option<&str>) -> bool {
    provider_id == Some(LOCAL_PROVIDER_ID)
}
