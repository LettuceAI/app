use std::collections::HashMap;

use serde_json::{json, Value};
use tauri::AppHandle;
#[cfg(not(mobile))]
use tauri::Emitter;

use crate::api::{ApiRequest, ApiResponse};
use crate::chat_manager::types::{ErrorEnvelope, NormalizedEvent, UsageSummary};
use crate::transport;
#[cfg(not(mobile))]
use crate::utils::{log_error, log_info, log_warn};

const LOCAL_PROVIDER_ID: &str = "llamacpp";

#[cfg(not(mobile))]
mod desktop {
    use super::*;
    use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
    use llama_cpp_2::llama_backend::LlamaBackend;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel};
    use llama_cpp_2::sampling::LlamaSampler;
    use llama_cpp_2::TokenToStringError;
    use llama_cpp_sys_2::{
        ggml_backend_dev_count, ggml_backend_dev_get, ggml_backend_dev_memory,
        ggml_backend_dev_type, llama_flash_attn_type, GGML_BACKEND_DEVICE_TYPE_ACCEL,
        GGML_BACKEND_DEVICE_TYPE_GPU, GGML_BACKEND_DEVICE_TYPE_IGPU, LLAMA_FLASH_ATTN_TYPE_AUTO,
        LLAMA_FLASH_ATTN_TYPE_DISABLED, LLAMA_FLASH_ATTN_TYPE_ENABLED,
    };
    use std::num::NonZeroU32;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use tokio::sync::oneshot::error::TryRecvError;

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct LlamaCppContextInfo {
        max_context_length: u32,
        recommended_context_length: Option<u32>,
        available_memory_bytes: Option<u64>,
        available_vram_bytes: Option<u64>,
        model_size_bytes: Option<u64>,
    }

    struct LlamaState {
        backend: Option<LlamaBackend>,
        model_path: Option<String>,
        model_params_key: Option<String>,
        model: Option<LlamaModel>,
    }

    fn push_unique_u32(out: &mut Vec<u32>, value: u32) {
        if !out.contains(&value) {
            out.push(value);
        }
    }

    fn context_attempt_candidates(
        initial_ctx_size: u32,
        prompt_tokens: usize,
        requested_context: Option<u32>,
        llama_batch_size: u32,
    ) -> Vec<(u32, u32)> {
        let minimum_ctx = (prompt_tokens as u32).saturating_add(1).max(1);
        let mut ctx_candidates = Vec::new();
        push_unique_u32(&mut ctx_candidates, initial_ctx_size.max(minimum_ctx));

        let mut scaled = if requested_context.is_some() {
            vec![initial_ctx_size.saturating_mul(3) / 4, initial_ctx_size / 2]
        } else {
            vec![
                initial_ctx_size.saturating_mul(3) / 4,
                initial_ctx_size / 2,
                initial_ctx_size / 3,
                initial_ctx_size / 4,
            ]
        };
        scaled.extend([8192, 4096, 3072, 2048, 1024, 768, 512]);

        for candidate in scaled {
            let clamped = candidate.max(minimum_ctx);
            if clamped > 0 {
                push_unique_u32(&mut ctx_candidates, clamped);
            }
        }

        let mut attempts = Vec::new();
        for ctx in ctx_candidates {
            let primary_batch = ctx.min(llama_batch_size).max(1);
            if !attempts.contains(&(ctx, primary_batch)) {
                attempts.push((ctx, primary_batch));
            }
            let reduced_batch = (primary_batch / 2).max(1);
            if reduced_batch != primary_batch && !attempts.contains(&(ctx, reduced_batch)) {
                attempts.push((ctx, reduced_batch));
            }
        }
        attempts
    }

    fn is_likely_context_oom_error(raw_error: &str) -> bool {
        let lower = raw_error.to_ascii_lowercase();
        lower.contains("null reference from llama.cpp")
            || lower.contains("out of memory")
            || lower.contains("oom")
            || lower.contains("alloc")
            || lower.contains("reserve")
            || lower.contains("failed to create")
    }

    fn context_error_detail(
        raw_error: &str,
        ctx_size: u32,
        n_batch: u32,
        resolved_offload_kqv: Option<bool>,
        llama_offload_kqv: Option<bool>,
        recommended_ctx: Option<u32>,
        llama_kv_type_raw: Option<&str>,
    ) -> String {
        if let Some(kv_type_raw) = llama_kv_type_raw {
            return format!(
                "llama.cpp rejected llamaKvType='{}' while creating the context (ctx={}, batch={}, offload_kqv={:?}): {}",
                kv_type_raw, ctx_size, n_batch, resolved_offload_kqv, raw_error
            );
        }

        if raw_error.contains("null reference from llama.cpp") {
            if let Some(recommended) = recommended_ctx {
                if recommended > 0 && ctx_size > recommended {
                    return format!(
                        "Likely memory allocation failure for context {}. Recommended <= {} tokens for current {} budget.",
                        ctx_size,
                        recommended,
                        if llama_offload_kqv == Some(true) {
                            "VRAM"
                        } else {
                            "RAM"
                        }
                    );
                }
            }
            return "Likely memory allocation failure (OOM) in llama.cpp. Try lower context length, lower llamaBatchSize, or a denser KV type (q8_0/q4_0).".to_string();
        }

        raw_error.to_string()
    }

    fn compiled_gpu_backends() -> Vec<&'static str> {
        let mut out = Vec::new();
        if cfg!(feature = "llama-gpu-cuda") || cfg!(feature = "llama-gpu-cuda-no-vmm") {
            out.push("cuda");
        }
        if cfg!(feature = "llama-gpu-rocm") {
            out.push("rocm");
        }
        if cfg!(feature = "llama-gpu-metal") {
            out.push("metal");
        }
        out
    }

    fn using_rocm_backend() -> bool {
        cfg!(feature = "llama-gpu-rocm")
    }

    static ENGINE: OnceLock<Mutex<LlamaState>> = OnceLock::new();

    fn load_engine(
        app: Option<&AppHandle>,
        model_path: &str,
        requested_gpu_layers: Option<u32>,
    ) -> Result<std::sync::MutexGuard<'static, LlamaState>, String> {
        let engine = ENGINE.get_or_init(|| {
            Mutex::new(LlamaState {
                backend: None,
                model_path: None,
                model_params_key: None,
                model: None,
            })
        });

        let mut guard = engine
            .lock()
            .map_err(|_| "llama.cpp engine lock poisoned".to_string())?;

        if guard.backend.is_none() {
            guard.backend = Some(LlamaBackend::init().map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to initialize llama backend: {e}"),
                )
            })?);
        }

        let backend = guard
            .backend
            .as_ref()
            .ok_or_else(|| "llama.cpp backend unavailable".to_string())?;
        if let Some(app) = app {
            let gpu_backends = compiled_gpu_backends();
            let gpu_backend_label = if gpu_backends.is_empty() {
                "none".to_string()
            } else {
                gpu_backends.join(",")
            };
            log_info(
                app,
                "llama_cpp",
                format!(
                    "llama.cpp backend initialized: compiled_gpu_backends={} supports_gpu_offload={}",
                    gpu_backend_label,
                    backend.supports_gpu_offload()
                ),
            );
        }
        let supports_gpu = backend.supports_gpu_offload();
        if let (Some(app), Some(requested)) = (app, requested_gpu_layers) {
            if requested > 0 && !supports_gpu {
                log_warn(
                    app,
                    "llama_cpp",
                    format!(
                        "Requested llamaGpuLayers={} but this build has no active GPU offload; using CPU layers only.",
                        requested
                    ),
                );
            }
        }
        let requested_gpu_layers_key = requested_gpu_layers
            .map(|v| v.to_string())
            .unwrap_or_else(|| "auto".to_string());
        let model_params_key = format!("requested_gpu_layers={requested_gpu_layers_key}");
        let should_reload = guard.model.is_none()
            || guard.model_path.as_deref() != Some(model_path)
            || guard.model_params_key.as_deref() != Some(&model_params_key);
        if should_reload {
            let cpu_params = LlamaModelParams::default().with_n_gpu_layers(0);

            let model = if supports_gpu && requested_gpu_layers != Some(0) {
                let gpu_params = if let Some(explicit_layers) = requested_gpu_layers {
                    LlamaModelParams::default().with_n_gpu_layers(explicit_layers)
                } else {
                    // Let llama.cpp choose the default GPU offload policy/layers.
                    LlamaModelParams::default()
                };

                match LlamaModel::load_from_file(backend, model_path, &gpu_params) {
                    Ok(model) => {
                        if let Some(app) = app {
                            let mode = requested_gpu_layers
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "llama-default".to_string());
                            log_info(
                                app,
                                "llama_cpp",
                                format!("Loaded model with GPU mode {}", mode),
                            );
                        }
                        model
                    }
                    Err(err) => {
                        if let Some(app) = app {
                            log_warn(
                                app,
                                "llama_cpp",
                                format!("GPU model load failed, falling back to CPU: {}", err),
                            );
                            let _ = app.emit(
                                "app://toast",
                                json!({
                                    "variant": "warning",
                                    "title": "GPU fallback",
                                    "description": "Model did not fit in GPU memory. Switched to CPU automatically."
                                }),
                            );
                        }
                        LlamaModel::load_from_file(backend, model_path, &cpu_params).map_err(
                            |e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!("Failed to load llama model: {e}"),
                                )
                            },
                        )?
                    }
                }
            } else {
                LlamaModel::load_from_file(backend, model_path, &cpu_params).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to load llama model: {e}"),
                    )
                })?
            };

            guard.model = Some(model);
            guard.model_path = Some(model_path.to_string());
            guard.model_params_key = Some(model_params_key);
        }

        Ok(guard)
    }

    pub fn unload_engine(app: &AppHandle) -> Result<(), String> {
        let engine = ENGINE.get_or_init(|| {
            Mutex::new(LlamaState {
                backend: None,
                model_path: None,
                model_params_key: None,
                model: None,
            })
        });

        let mut guard = engine
            .lock()
            .map_err(|_| "llama.cpp engine lock poisoned".to_string())?;

        if guard.model.is_some() {
            guard.model = None;
            guard.model_path = None;
            guard.model_params_key = None;
            log_info(app, "llama_cpp", "unloaded llama.cpp model");
        }

        Ok(())
    }

    fn normalize_role(role: &str) -> &'static str {
        match role {
            "system" | "developer" => "system",
            "assistant" => "assistant",
            "user" => "user",
            _ => "user",
        }
    }

    fn sanitize_text(value: &str) -> String {
        value.replace('\0', "")
    }

    fn token_piece_bytes(
        model: &LlamaModel,
        token: llama_cpp_2::token::LlamaToken,
    ) -> Result<Vec<u8>, String> {
        match model.token_to_piece_bytes(token, 8, false, None) {
            Ok(bytes) => Ok(bytes),
            Err(TokenToStringError::InsufficientBufferSpace(needed)) => {
                let required = usize::try_from(-needed).map_err(|_| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Invalid llama token buffer size hint: {needed}"),
                    )
                })?;
                model
                    .token_to_piece_bytes(token, required, false, None)
                    .map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to decode token bytes: {e}"),
                        )
                    })
            }
            Err(e) => Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to decode token bytes: {e}"),
            )),
        }
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

    pub(super) fn get_available_memory_bytes() -> Option<u64> {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        Some(sys.available_memory())
    }

    pub(super) fn get_available_vram_bytes() -> Option<u64> {
        let mut max_free: u64 = 0;
        // SAFETY: read-only ggml backend device enumeration and memory queries.
        unsafe {
            let count = ggml_backend_dev_count();
            for i in 0..count {
                let dev = ggml_backend_dev_get(i);
                if dev.is_null() {
                    continue;
                }
                let dev_type = ggml_backend_dev_type(dev);
                let is_gpu_like = dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                    || dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU
                    || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL;
                if !is_gpu_like {
                    continue;
                }
                let mut free: usize = 0;
                let mut total: usize = 0;
                ggml_backend_dev_memory(dev, &mut free, &mut total);
                if total == 0 {
                    continue;
                }
                let free_u64 = free as u64;
                if free_u64 > max_free {
                    max_free = free_u64;
                }
            }
        }
        if max_free > 0 {
            Some(max_free)
        } else {
            None
        }
    }

    /// Detect if the system uses unified memory (shared RAM/VRAM).
    /// True on Apple Silicon (macOS aarch64) or when only iGPU devices are found.
    pub(super) fn is_unified_memory() -> bool {
        // Apple Silicon always has unified memory
        if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            return true;
        }

        // Check if all GPU-like devices are iGPUs
        let mut found_gpu = false;
        let mut all_igpu = true;
        unsafe {
            let count = ggml_backend_dev_count();
            for i in 0..count {
                let dev = ggml_backend_dev_get(i);
                if dev.is_null() {
                    continue;
                }
                let dev_type = ggml_backend_dev_type(dev);
                if dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                    || dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU
                    || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL
                {
                    found_gpu = true;
                    if dev_type != GGML_BACKEND_DEVICE_TYPE_IGPU {
                        all_igpu = false;
                    }
                }
            }
        }
        found_gpu && all_igpu
    }

    fn kv_bytes_per_value(llama_kv_type: Option<&str>) -> f64 {
        match llama_kv_type
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("f32") => 4.0,
            Some("f16") => 2.0,
            Some("q8_1") | Some("q8_0") => 1.0,
            Some("q6_k") => 0.75,
            Some("q5_k") | Some("q5_1") | Some("q5_0") => 0.625,
            Some("q4_k") | Some("q4_1") | Some("q4_0") => 0.5,
            Some("q3_k") | Some("iq3_s") | Some("iq3_xxs") => 0.375,
            Some("q2_k") | Some("iq2_xs") | Some("iq2_xxs") | Some("iq1_s") => 0.25,
            Some("iq4_nl") => 0.5,
            _ => 2.0, // llama.cpp default KV cache type is F16 when unspecified
        }
    }

    fn estimate_kv_bytes_per_token(model: &LlamaModel, llama_kv_type: Option<&str>) -> Option<u64> {
        let n_layer = u64::from(model.n_layer());
        let n_embd = u64::try_from(model.n_embd()).ok()?;

        // Default to n_head if n_head_kv is not available or zero (older models)
        let n_head = u64::try_from(model.n_head()).unwrap_or(1).max(1);
        let n_head_kv = u64::try_from(model.n_head_kv()).unwrap_or(n_head).max(1);

        // GQA Ratio: In Llama 3, this is 8/32 = 0.25
        // We calculate the effective embedding size for the KV cache
        let gqa_correction = n_head_kv as f64 / n_head as f64;
        let effective_n_embd = (n_embd as f64 * gqa_correction) as u64;

        // K cache + V cache = 2 matrices
        let bytes_per_value = kv_bytes_per_value(llama_kv_type);
        let bytes = (n_layer as f64) * (effective_n_embd as f64) * 2.0 * bytes_per_value;
        Some(bytes.max(0.0) as u64)
    }

    fn compute_recommended_context(
        model: &LlamaModel,
        available_memory_bytes: Option<u64>,
        available_vram_bytes: Option<u64>,
        max_context_length: u32,
        llama_offload_kqv: Option<bool>,
        llama_kv_type: Option<&str>,
    ) -> Option<u32> {
        let available_for_ctx = if llama_offload_kqv == Some(true) {
            let vram = available_vram_bytes?;
            let reserve = (vram / 5).max(512 * 1024 * 1024);
            vram.saturating_sub(reserve)
        } else {
            let ram = available_memory_bytes?;
            let model_size = model.size();
            let reserve = (ram / 5).max(512 * 1024 * 1024);
            ram.saturating_sub(model_size.saturating_add(reserve))
        };
        let kv_bytes_per_token = estimate_kv_bytes_per_token(model, llama_kv_type)?;
        if kv_bytes_per_token == 0 {
            return None;
        }
        let mut recommended = available_for_ctx / kv_bytes_per_token;
        if recommended > u64::from(max_context_length) {
            recommended = u64::from(max_context_length);
        }
        Some(recommended as u32)
    }

    fn extract_text_content(message: &Value) -> String {
        let content = message.get("content");
        match content {
            Some(Value::String(text)) => sanitize_text(text),
            Some(Value::Array(parts)) => {
                let mut out: Vec<String> = Vec::new();
                for part in parts {
                    let part_type = part.get("type").and_then(|v| v.as_str());
                    if part_type == Some("text") {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            let cleaned = sanitize_text(text);
                            if !cleaned.is_empty() {
                                out.push(cleaned);
                            }
                        }
                    }
                }
                out.join("\n")
            }
            _ => String::new(),
        }
    }

    fn build_fallback_prompt(messages: &[Value]) -> String {
        let mut prompt = String::new();
        for message in messages {
            let role = message
                .get("role")
                .and_then(|v| v.as_str())
                .map(normalize_role)
                .unwrap_or("user");
            let content = extract_text_content(message);
            if content.is_empty() {
                continue;
            }
            prompt.push_str(role);
            prompt.push_str(": ");
            prompt.push_str(&content);
            prompt.push('\n');
        }
        prompt.push_str("assistant: ");
        prompt
    }

    fn build_prompt(model: &LlamaModel, messages: &[Value]) -> Result<String, String> {
        let mut chat_messages = Vec::new();
        for message in messages {
            let role = message
                .get("role")
                .and_then(|v| v.as_str())
                .map(normalize_role)
                .unwrap_or("user");
            let content = extract_text_content(message);
            if content.is_empty() {
                continue;
            }
            let chat_message = LlamaChatMessage::new(role.to_string(), content).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Invalid chat message: {e}"),
                )
            })?;
            chat_messages.push(chat_message);
        }

        if chat_messages.is_empty() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "No usable chat messages for llama.cpp",
            ));
        }

        let template = model
            .chat_template(None)
            .or_else(|_| LlamaChatTemplate::new("chatml"))
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to load chat template: {e}"),
                )
            })?;

        let prompt = match model.apply_chat_template(&template, &chat_messages, true) {
            Ok(text) => text,
            Err(_) => build_fallback_prompt(messages),
        };

        Ok(prompt)
    }

    fn build_sampler(
        temperature: f64,
        top_p: f64,
        min_p: Option<f64>,
        top_k: Option<u32>,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
        seed: Option<u32>,
    ) -> LlamaSampler {
        let mut samplers = Vec::new();
        let penalty_freq = frequency_penalty.unwrap_or(0.0);
        let penalty_present = presence_penalty.unwrap_or(0.0);
        if penalty_freq != 0.0 || penalty_present != 0.0 {
            samplers.push(LlamaSampler::penalties(
                -1,
                1.0,
                penalty_freq as f32,
                penalty_present as f32,
            ));
        }

        let k = top_k.unwrap_or(40) as i32;
        samplers.push(LlamaSampler::top_k(k));

        let p = if top_p > 0.0 { top_p } else { 1.0 };
        samplers.push(LlamaSampler::top_p(p as f32, 1));
        if let Some(mp) = min_p {
            if mp > 0.0 {
                samplers.push(LlamaSampler::min_p(mp as f32, 1));
            }
        }

        if temperature > 0.0 {
            samplers.push(LlamaSampler::temp(temperature as f32));
            samplers.push(LlamaSampler::dist(seed.unwrap_or_else(rand::random::<u32>)));
        } else {
            samplers.push(LlamaSampler::greedy());
        }

        LlamaSampler::chain(samplers, false)
    }

    pub async fn llamacpp_context_info(
        app: AppHandle,
        model_path: String,
        llama_offload_kqv: Option<bool>,
        llama_kv_type: Option<String>,
    ) -> Result<LlamaCppContextInfo, String> {
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

        let engine = load_engine(Some(&app), &model_path, None)?;
        let model = engine
            .model
            .as_ref()
            .ok_or_else(|| "llama.cpp model unavailable".to_string())?;
        let max_ctx = model.n_ctx_train().max(1);
        let available_memory_bytes = get_available_memory_bytes();
        let available_vram_bytes = get_available_vram_bytes();
        let recommended_context_length = compute_recommended_context(
            model,
            available_memory_bytes,
            available_vram_bytes,
            max_ctx,
            llama_offload_kqv,
            llama_kv_type.as_deref(),
        );

        Ok(LlamaCppContextInfo {
            max_context_length: max_ctx,
            recommended_context_length,
            available_memory_bytes,
            available_vram_bytes,
            model_size_bytes: Some(model.size()),
        })
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

        let temperature = body
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7);
        let top_p = body.get("top_p").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let min_p = body
            .get("min_p")
            .or_else(|| body.get("minP"))
            .or_else(|| body.get("llamaMinP"))
            .or_else(|| body.get("llama_min_p"))
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
        let top_k = body
            .get("top_k")
            .or_else(|| body.get("topK"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0);
        let frequency_penalty = body.get("frequency_penalty").and_then(|v| v.as_f64());
        let presence_penalty = body.get("presence_penalty").and_then(|v| v.as_f64());
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
            let prompt = build_prompt(model, messages)?;
            let tokens = model.str_to_token(&prompt, AddBos::Always).map_err(|e| {
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

            let mut sampler = build_sampler(
                temperature,
                top_p,
                min_p,
                top_k,
                frequency_penalty,
                presence_penalty,
                llama_seed,
            );

            let target_len = prompt_len + max_new as i32;
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

                if token == model.token_eos() {
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
                    if stream {
                        if let Some(ref id) = request_id {
                            transport::emit_normalized(
                                &app,
                                id,
                                NormalizedEvent::Delta { text: piece },
                            );
                        }
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
                if stream {
                    if let Some(ref id) = request_id {
                        transport::emit_normalized(&app, id, NormalizedEvent::Delta { text: tail });
                    }
                }
            }

            generation_elapsed_ms = Some(inference_started_at.elapsed().as_millis() as u64);

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
                    reasoning_tokens: None,
                    image_tokens: None,
                    first_token_ms,
                    tokens_per_second,
                    finish_reason: Some("stop".into()),
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
                "message": { "role": "assistant", "content": output },
                "finish_reason": "stop"
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
    desktop::get_available_memory_bytes()
}

#[cfg(not(mobile))]
pub(crate) fn available_vram_bytes() -> Option<u64> {
    desktop::get_available_vram_bytes()
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
    desktop::is_unified_memory()
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
        let info =
            desktop::llamacpp_context_info(app, model_path, llama_offload_kqv, llama_kv_type)
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
        return desktop::unload_engine(&app);
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
