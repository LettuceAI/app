use std::collections::HashMap;

use serde_json::{json, Value};
use tauri::AppHandle;
#[cfg(not(mobile))]
use tauri::Emitter;

use crate::api::{ApiRequest, ApiResponse};
use crate::chat_manager::provider_adapter::extract_text_content;
use crate::chat_manager::tooling::{parse_tool_calls, ToolCall};
use crate::chat_manager::types::{ErrorEnvelope, NormalizedEvent, UsageSummary};
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
    mod prompt;
    mod sampler;

    use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel};
    use llama_cpp_2::sampling::LlamaSampler;
    use llama_cpp_2::TokenToStringError;
    use llama_cpp_sys_2::{
        llama_flash_attn_type, LLAMA_FLASH_ATTN_TYPE_AUTO, LLAMA_FLASH_ATTN_TYPE_DISABLED,
        LLAMA_FLASH_ATTN_TYPE_ENABLED,
    };
    use std::num::NonZeroU32;
    use std::path::Path;
    use std::time::Instant;
    use tokio::sync::oneshot::error::TryRecvError;

    use context::{
        compute_recommended_context, context_attempt_candidates, context_error_detail,
        get_available_memory_bytes, get_available_vram_bytes, is_likely_context_oom_error,
    };
    use engine::{load_engine, using_rocm_backend};
    use prompt::{
        add_bos_label, build_prompt, model_tokenizer_add_bos_label, model_tokenizer_adds_bos,
        prompt_add_bos_reason, prompt_mode_label, resolve_prompt_add_bos, token_piece_bytes,
    };
    use sampler::{
        build_sampler, flash_attention_policy_label, kv_type_label, normalize_sampler_profile,
        offload_kqv_mode_label, sampler_profile_defaults, ResolvedSamplerConfig,
    };

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

    fn parse_stop_sequences(body: &Value) -> Vec<String> {
        fn parse_value(value: &Value) -> Vec<String> {
            match value {
                Value::String(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        Vec::new()
                    } else {
                        vec![trimmed.to_string()]
                    }
                }
                Value::Array(values) => values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .map(str::trim)
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

    fn earliest_stop_match<'a>(
        text: &str,
        stop_sequences: &'a [String],
    ) -> Option<(usize, &'a str)> {
        stop_sequences
            .iter()
            .filter_map(|stop| text.find(stop).map(|index| (index, stop.as_str())))
            .min_by_key(|(index, _)| *index)
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
                    streamed_text.push_str(text);
                    if let Some(id) = request_id {
                        transport::emit_normalized(
                            app,
                            id,
                            NormalizedEvent::Delta {
                                text: text.to_string(),
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn ensure_assistant_role(message: &mut Value) {
        if let Some(object) = message.as_object_mut() {
            object
                .entry("role".to_string())
                .or_insert_with(|| Value::String("assistant".to_string()));
        }
    }

    pub async fn handle_local_request(
        app: AppHandle,
        req: ApiRequest,
    ) -> Result<ApiResponse, String> {
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
        let tools = body
            .get("tools")
            .filter(|value| value.as_array().map(|items| !items.is_empty()).unwrap_or(false));
        let tool_choice = body.get("tool_choice");

        let sampler_profile = body
            .get("llamaSamplerProfile")
            .or_else(|| body.get("llama_sampler_profile"))
            .and_then(|v| v.as_str())
            .and_then(normalize_sampler_profile);
        let sampler_defaults = sampler_profile_defaults(sampler_profile);
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
        let llama_batch_size = body
            .get("llamaBatchSize")
            .or_else(|| body.get("llama_batch_size"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0)
            .unwrap_or(512);
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
        let mut completion_tokens = 0u64;
        let inference_started_at = Instant::now();
        let mut first_token_ms: Option<u64> = None;
        let mut generation_elapsed_ms: Option<u64> = None;
        let mut finish_reason = "stop";
        let mut stream_emitted_len = 0usize;
        let mut final_message = json!({ "role": "assistant", "content": "" });

        let result = (|| -> Result<(), String> {
            log_info(&app, "llama_cpp", "loading llama.cpp engine/model");
            let engine = load_engine(Some(&app), model_path, llama_gpu_layers)?;
            let model = engine
                .model
                .as_ref()
                .ok_or_else(|| "llama.cpp model unavailable".to_string())?;
            let backend = engine
                .backend
                .as_ref()
                .ok_or_else(|| "llama.cpp backend unavailable".to_string())?;
            let max_ctx = model.n_ctx_train().max(1);
            let available_memory_bytes = get_available_memory_bytes();
            let available_vram_bytes = get_available_vram_bytes();
            let recommended_ctx = compute_recommended_context(
                model,
                available_memory_bytes,
                available_vram_bytes,
                max_ctx,
                llama_offload_kqv,
                llama_kv_type_raw.as_deref(),
            );
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
            let built_prompt = build_prompt(
                model,
                messages,
                llama_chat_template_override.as_deref(),
                llama_chat_template_preset.as_deref(),
                llama_raw_completion_fallback,
                tools,
                tool_choice,
            )?;
            let mut stop_sequences = parse_stop_sequences(body);
            for stop in &built_prompt.additional_stop_sequences {
                if !stop.is_empty() && !stop_sequences.iter().any(|existing| existing == stop) {
                    stop_sequences.push(stop.clone());
                }
            }
            let max_stop_sequence_len = stop_sequences
                .iter()
                .map(|stop| stop.len())
                .max()
                .unwrap_or(0);
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
            let prompt = built_prompt.prompt;
            let tokens = model.str_to_token(&prompt, prompt_add_bos).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to tokenize prompt: {e}"),
                )
            })?;
            prompt_tokens = tokens.len() as u64;

            if tokens.len() as u32 >= ctx_size {
                return Err(format!(
                    "Prompt is too long for the context window (prompt tokens: {}, context: {}). Reduce messages or lower context length.",
                    tokens.len(),
                    ctx_size
                ));
            }

            let resolved_offload_kqv = if llama_offload_kqv.is_some() {
                llama_offload_kqv
            } else if using_rocm_backend() {
                // ROCm/HIP builds can be more stable with KQV on CPU by default on some AMD stacks.
                Some(false)
            } else {
                None
            };
            let resolved_flash_attention_policy = if let Some(policy) = llama_flash_attention_policy
            {
                policy
            } else if using_rocm_backend() {
                // Conservative ROCm default to avoid driver/device crashes on some AMD stacks.
                LLAMA_FLASH_ATTN_TYPE_DISABLED
            } else {
                LLAMA_FLASH_ATTN_TYPE_AUTO
            };
            let requested_ctx_size = ctx_size;
            let initial_batch = ctx_size.min(llama_batch_size).max(1);
            let mut resolved_ctx_size = ctx_size;
            let mut resolved_n_batch = initial_batch;
            let mut context_failures = Vec::new();
            let context_attempts = context_attempt_candidates(
                ctx_size,
                tokens.len(),
                requested_context,
                llama_batch_size,
            );
            let mut ctx: Option<_> = None;

            for (attempt_ctx, attempt_batch) in context_attempts {
                let mut ctx_params = LlamaContextParams::default()
                    .with_n_ctx(NonZeroU32::new(attempt_ctx))
                    .with_n_batch(attempt_batch);
                if let Some(n_threads) = llama_threads {
                    ctx_params = ctx_params.with_n_threads(n_threads as i32);
                }
                if let Some(n_threads_batch) = llama_threads_batch {
                    ctx_params = ctx_params.with_n_threads_batch(n_threads_batch as i32);
                }
                if let Some(offload) = resolved_offload_kqv {
                    ctx_params = ctx_params.with_offload_kqv(offload);
                }
                if let Some(kv_type) = llama_kv_type {
                    ctx_params = ctx_params.with_type_k(kv_type).with_type_v(kv_type);
                }
                ctx_params =
                    ctx_params.with_flash_attention_policy(resolved_flash_attention_policy);
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
                        "creating context attempt: ctx={} batch={} gpu_layers={:?} offload_kqv={:?} flash_attention_policy={:?}",
                        attempt_ctx,
                        attempt_batch,
                        llama_gpu_layers,
                        resolved_offload_kqv,
                        resolved_flash_attention_policy
                    ),
                );

                match model.new_context(backend, ctx_params) {
                    Ok(created) => {
                        resolved_ctx_size = attempt_ctx;
                        resolved_n_batch = attempt_batch;
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
                        break;
                    }
                    Err(err) => {
                        let raw_error = err.to_string();
                        let detail = context_error_detail(
                            &raw_error,
                            attempt_ctx,
                            attempt_batch,
                            resolved_offload_kqv,
                            llama_offload_kqv,
                            recommended_ctx,
                            llama_kv_type_raw.as_deref(),
                        );

                        let has_explicit_kv = llama_kv_type_raw.is_some();
                        let likely_oom = is_likely_context_oom_error(&raw_error);
                        if has_explicit_kv || !likely_oom {
                            return Err(crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Failed to create llama context: {detail}"),
                            ));
                        }

                        context_failures.push(format!(
                            "ctx={} batch={} -> {}",
                            attempt_ctx, attempt_batch, detail
                        ));
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
            let context_fallback_activated =
                (ctx_size, n_batch) != (requested_ctx_size, initial_batch);
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
            let gpu_load_fallback_activated = engine.gpu_load_fallback_activated;

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
                    "initialBatchCandidate": initial_batch,
                    "actualNBatchUsed": n_batch,
                    "actualKvTypeUsed": kv_type_label(llama_kv_type_raw.as_deref()),
                    "actualOffloadKqvMode": offload_kqv_mode_label(resolved_offload_kqv),
                    "flashAttentionPolicy": flash_attention_policy_label(resolved_flash_attention_policy),
                    "actualBackendPathUsed": backend_path_used.clone(),
                    "compiledGpuBackends": compiled_gpu_backends,
                    "supportsGpuOffload": supports_gpu_offload,
                    "gpuLoadFallbackActivated": gpu_load_fallback_activated,
                    "contextFallbackActivated": context_fallback_activated,
                }
            });
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "llama runtime resolved: prompt_mode={} template_source={} fallback_prompt={} bos={} ctx={} n_batch={} kv_type={} offload_kqv={} backend_path={} flash_attention={} context_fallback={}",
                    prompt_mode_label(built_prompt.prompt_mode),
                    built_prompt
                        .applied_template_source
                        .as_deref()
                        .unwrap_or("none"),
                    built_prompt.used_raw_completion_fallback,
                    add_bos_label(prompt_add_bos),
                    ctx_size,
                    n_batch,
                    kv_type_label(llama_kv_type_raw.as_deref()),
                    offload_kqv_mode_label(resolved_offload_kqv),
                    backend_path_used,
                    flash_attention_policy_label(resolved_flash_attention_policy),
                    context_fallback_activated,
                ),
            );
            crate::utils::emit_debug(&app, "llama_runtime", runtime_settings);

            let batch_size = n_batch as usize;
            let mut batch = LlamaBatch::new(batch_size, 1);

            // Feed prompt in chunks so large prompts work even when n_batch is capped.
            let tokens_len = tokens.len();
            let mut global_pos: i32 = 0;
            let mut chunk_start = 0usize;
            while chunk_start < tokens_len {
                let chunk_end = (chunk_start + batch_size).min(tokens_len);
                batch.clear();
                for (offset, token) in tokens[chunk_start..chunk_end].iter().copied().enumerate() {
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
                global_pos += (chunk_end - chunk_start) as i32;
                chunk_start = chunk_end;
            }
            log_info(
                &app,
                "llama_cpp",
                format!(
                    "prompt evaluation complete: prompt_tokens={} target_new_tokens={}",
                    prompt_tokens, max_tokens
                ),
            );

            let prompt_len = global_pos;
            let mut n_cur = prompt_len;
            let max_new = max_tokens.min(ctx_size.saturating_sub(n_cur as u32 + 1));

            let sampler_config = ResolvedSamplerConfig {
                profile: sampler_defaults.name,
                temperature,
                top_p,
                top_k,
                min_p,
                typical_p,
                frequency_penalty,
                presence_penalty,
                seed: llama_seed,
            };
            let built_sampler =
                build_sampler(model, &sampler_config, built_prompt.chat_template_result.as_ref())?;
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
                    "order": built_sampler.order,
                    "activeParams": built_sampler.active_params,
                }),
            );
            let mut sampler = built_sampler.sampler;
            let mut structured_parser = built_prompt
                .chat_template_result
                .as_ref()
                .map(|result| result.streaming_state_oaicompat())
                .transpose()
                .map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to initialize llama.cpp structured parser: {e}"),
                    )
                })?;
            let mut streamed_structured_text = String::new();
            let mut structured_parsed_len = 0usize;

            let target_len = prompt_len + max_new as i32;
            let mut reached_eos = false;
            let mut reached_stop_sequence = false;
            let mut pending_utf8 = Vec::<u8>::new();
            while n_cur < target_len {
                if let Some(rx) = abort_rx.as_mut() {
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

                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                sampler.accept(token);

                if model.is_eog_token(token) {
                    reached_eos = true;
                    break;
                }

                let piece_bytes = token_piece_bytes(&model, token)?;

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
                    output.push_str(&piece);
                    if let Some((stop_index, _)) = earliest_stop_match(&output, &stop_sequences) {
                        output.truncate(stop_index);
                        reached_stop_sequence = true;
                    }

                    if built_prompt.chat_template_result.is_none() {
                        if stream && stream_emitted_len < output.len() {
                            let safe_emit_end = if reached_stop_sequence {
                                output.len()
                            } else if max_stop_sequence_len > 0 {
                                clamp_to_char_boundary(
                                    &output,
                                    output
                                        .len()
                                        .saturating_sub(max_stop_sequence_len.saturating_sub(1)),
                                )
                            } else {
                                output.len()
                            };
                            if safe_emit_end > stream_emitted_len {
                                if let Some(ref id) = request_id {
                                    transport::emit_normalized(
                                        &app,
                                        id,
                                        NormalizedEvent::Delta {
                                            text: output[stream_emitted_len..safe_emit_end]
                                                .to_string(),
                                        },
                                    );
                                }
                                stream_emitted_len = safe_emit_end;
                            }
                        }
                    } else if stream {
                        if let Some(parser) = structured_parser.as_mut() {
                            let safe_parse_end = if reached_stop_sequence {
                                output.len()
                            } else if max_stop_sequence_len > 0 {
                                clamp_to_char_boundary(
                                    &output,
                                    output
                                        .len()
                                        .saturating_sub(max_stop_sequence_len.saturating_sub(1)),
                                )
                            } else {
                                output.len()
                            };
                            if safe_parse_end > structured_parsed_len {
                                let delta_input = &output[structured_parsed_len..safe_parse_end];
                                let deltas = parser.update(delta_input, true).map_err(|e| {
                                    crate::utils::err_msg(
                                        module_path!(),
                                        line!(),
                                        format!(
                                            "Failed to parse llama.cpp structured stream: {e}"
                                        ),
                                    )
                                })?;
                                emit_structured_deltas(
                                    &app,
                                    request_id.as_ref(),
                                    deltas,
                                    &mut streamed_structured_text,
                                )?;
                                structured_parsed_len = safe_parse_end;
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
                    first_token_ms = Some(inference_started_at.elapsed().as_millis() as u64);
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
            }

            if !pending_utf8.is_empty() {
                let tail = String::from_utf8_lossy(&pending_utf8).to_string();
                output.push_str(&tail);
                if let Some((stop_index, _)) = earliest_stop_match(&output, &stop_sequences) {
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
                    transport::emit_normalized(
                        &app,
                        id,
                        NormalizedEvent::Delta {
                            text: output[stream_emitted_len..].to_string(),
                        },
                    );
                }
                stream_emitted_len = output.len();
            }

            generation_elapsed_ms = Some(inference_started_at.elapsed().as_millis() as u64);

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
                    &mut streamed_structured_text,
                )?;
            }

            finish_reason = if reached_stop_sequence || reached_eos {
                "stop"
            } else {
                "length"
            };

            let mut final_tool_calls: Vec<ToolCall> = Vec::new();
            let parsed_final_message = if let Some(template_result) =
                built_prompt.chat_template_result.as_ref()
            {
                let is_partial = finish_reason == "length";
                let parsed_message = template_result
                    .parse_response_oaicompat(&output, is_partial)
                    .map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to parse llama.cpp structured response: {e}"),
                        )
                    })?;
                let mut message: Value = serde_json::from_str(&parsed_message).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to deserialize llama.cpp structured message: {e}"),
                    )
                })?;
                ensure_assistant_role(&mut message);

                let full_text = extract_text_content(message.get("content")).unwrap_or_default();
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
                message
            } else {
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
            output = extract_text_content(final_message.get("content")).unwrap_or_default();

            Ok(())
        })();

        if let Some(ref id) = request_id {
            use tauri::Manager;
            let registry = app.state::<crate::abort_manager::AbortRegistry>();
            registry.unregister(id);
        }

        if let Err(err) = result {
            log_error(&app, "llama_cpp", format!("local inference error: {}", err));
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

        if stream {
            if let Some(ref id) = request_id {
                let tokens_per_second = generation_elapsed_ms
                    .and_then(|elapsed_ms| {
                        if elapsed_ms == 0 || completion_tokens == 0 {
                            None
                        } else {
                            Some((completion_tokens as f64) / (elapsed_ms as f64 / 1000.0))
                        }
                    })
                    .filter(|v| v.is_finite() && *v >= 0.0);
                let usage = UsageSummary {
                    prompt_tokens: Some(prompt_tokens),
                    completion_tokens: Some(completion_tokens),
                    total_tokens: Some(prompt_tokens + completion_tokens),
                    cached_prompt_tokens: None,
                    cache_write_tokens: None,
                    reasoning_tokens: None,
                    image_tokens: None,
                    web_search_requests: None,
                    api_cost: None,
                    response_id: None,
                    first_token_ms,
                    tokens_per_second,
                    finish_reason: Some(finish_reason.into()),
                };
                transport::emit_normalized(&app, id, NormalizedEvent::Usage { usage });
                transport::emit_normalized(&app, id, NormalizedEvent::Done);
            }
        }

        let tokens_per_second = generation_elapsed_ms
            .and_then(|elapsed_ms| {
                if elapsed_ms == 0 || completion_tokens == 0 {
                    None
                } else {
                    Some((completion_tokens as f64) / (elapsed_ms as f64 / 1000.0))
                }
            })
            .filter(|v| v.is_finite() && *v >= 0.0);

        let usage_value = json!({
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
            "first_token_ms": first_token_ms,
            "tokens_per_second": tokens_per_second,
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

#[cfg(mobile)]
pub(crate) fn is_unified_memory() -> bool {
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
) -> Result<serde_json::Value, String> {
    #[cfg(not(mobile))]
    {
        let info = desktop::context::llamacpp_context_info(
            app,
            model_path,
            llama_offload_kqv,
            llama_kv_type,
        )
        .await?;
        return serde_json::to_value(info).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to serialize context info: {e}"),
            )
        });
    }
    #[cfg(mobile)]
    {
        let _ = app;
        let _ = model_path;
        let _ = llama_kv_type;
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
        return desktop::engine::unload_engine(&app);
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
