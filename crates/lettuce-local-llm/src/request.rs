//! One local generation request and the rules that turn its raw settings
//! into the values a run uses: sampler profile defaults, range filters,
//! deduplicated device lists, MTP and DFlash draft bounds, the thinking
//! switch (a trailing `/think` or `/no_think` wins over the explicit flag,
//! which wins over "a reasoning format was asked for") and the incremental
//! stop matcher.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::offload::{FlashAttentionPolicy, KvCacheTypes};

pub const MTP_DRAFT_DEFAULT: u32 = 4;
pub const MTP_DRAFT_MAX: u32 = 8;
pub const DFLASH_DRAFT_DEFAULT: u32 = 4;
pub const DFLASH_DRAFT_MAX: u32 = 15;
pub const DFLASH_P_MIN_DEFAULT: f32 = 0.55;
pub const DEFAULT_MAX_TOKENS: u32 = 512;
pub const DEFAULT_BATCH_SIZE: u32 = 512;
pub const STREAM_EMIT_INTERVAL: Duration = Duration::from_millis(32);
pub const STREAM_EMIT_BYTES: usize = 256;

/// Everything one local generation needs. Settings carry the user's raw
/// values; the `resolve_*` methods apply the filters.
#[derive(Clone, Debug, Default)]
pub struct LlamaGenerationRequest {
    pub request_id: Option<String>,
    pub model_path: String,
    pub messages: Vec<Value>,
    pub tools: Option<Value>,
    pub tool_choice: Option<Value>,
    pub stop: Vec<String>,
    pub stream: bool,
    pub prompt_cache_key: Option<String>,
    pub max_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub reasoning: LlamaReasoningInput,
    pub sampling: LlamaSamplingInput,
    pub runtime: LlamaRuntimeInput,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LlamaReasoningInput {
    pub reasoning_format: Option<String>,
    pub reasoning_configured: bool,
    pub enable_thinking: Option<bool>,
    pub chat_template_kwargs: Option<Map<String, Value>>,
    pub parallel_tool_calls: bool,
    /// Gemma4-series forced reasoning: the system prompt opens with the think
    /// marker and the reply starts inside the thought channel.
    pub force_gemma4_reasoning: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LlamaSamplingInput {
    pub profile: Option<String>,
    pub disable_profile_defaults: bool,
    pub order: Option<Vec<String>>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub min_p: Option<f64>,
    pub typical_p: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repeat_penalty: Option<f64>,
    pub n_pen_range: Option<i64>,
    pub dry_multiplier: Option<f64>,
    pub dry_base: Option<f64>,
    pub dry_allowed_length: Option<u32>,
    pub dry_penalty_last_n: Option<i64>,
    pub dry_sequence_breakers: Option<Vec<String>>,
    pub xtc_probability: Option<f64>,
    pub xtc_threshold: Option<f64>,
    pub seed: Option<u32>,
    pub adaptive_target: Option<f64>,
    pub adaptive_decay: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LlamaRuntimeInput {
    pub gpu_layers: Option<u32>,
    pub multi_gpu_enabled: bool,
    pub gpu_device_ids: Vec<usize>,
    pub gpu_distribution_mode: Option<String>,
    pub gpu_manual_layers: Vec<(usize, u32)>,
    pub kv_placement: Option<String>,
    pub main_gpu: Option<i32>,
    pub single_gpu_device_id: Option<usize>,
    pub priority_vram_limit_bytes: Option<u64>,
    pub threads: Option<u32>,
    pub threads_batch: Option<u32>,
    pub batch_size: Option<u32>,
    pub ubatch_size: Option<u32>,
    pub rope_freq_base: Option<f64>,
    pub rope_freq_scale: Option<f64>,
    pub offload_kqv: Option<bool>,
    pub swa_full: Option<bool>,
    pub flash_attention: Option<FlashAttentionPolicy>,
    pub kv_type: Option<String>,
    pub kv_type_k: Option<String>,
    pub kv_type_v: Option<String>,
    pub mmproj_path: Option<String>,
    pub chat_template_override: Option<String>,
    pub chat_template_preset: Option<String>,
    pub raw_completion_fallback: bool,
    pub strict_mode: bool,
    pub mtp_enabled: bool,
    pub mtp_draft_tokens: Option<u32>,
    pub mtp_model_path: Option<String>,
    pub mtp_placement: Option<String>,
    pub dflash_enabled: bool,
    pub dflash_draft_tokens: Option<u32>,
    pub dflash_min_probability: Option<f64>,
    pub dflash_model_path: Option<String>,
}

/// The sampler values the run uses after profile defaults and filters.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSampling {
    pub profile: &'static str,
    pub order: Option<Vec<String>>,
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: Option<u32>,
    pub min_p: Option<f64>,
    pub typical_p: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repeat_penalty: Option<f64>,
    pub n_pen_range: Option<i32>,
    pub dry_multiplier: Option<f64>,
    pub dry_base: Option<f64>,
    pub dry_allowed_length: Option<u32>,
    pub dry_penalty_last_n: Option<i32>,
    pub dry_sequence_breakers: Option<Vec<String>>,
    pub xtc_probability: Option<f64>,
    pub xtc_threshold: Option<f64>,
    pub seed: Option<u32>,
    pub adaptive_target: Option<f64>,
    pub adaptive_decay: Option<f64>,
}

/// The runtime values the run uses after the filters.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRuntime {
    pub gpu_layers: Option<u32>,
    pub multi_gpu_enabled: bool,
    pub gpu_device_ids: Vec<usize>,
    pub gpu_distribution_mode: Option<String>,
    pub gpu_manual_layers: Vec<(usize, u32)>,
    pub kv_placement: Option<String>,
    pub main_gpu: Option<i32>,
    pub single_gpu_device_id: Option<usize>,
    pub priority_vram_limit_bytes: Option<u64>,
    pub threads: Option<u32>,
    pub threads_batch: Option<u32>,
    pub batch_size: u32,
    pub ubatch_size: Option<u32>,
    pub compute_batch_size: u32,
    pub rope_freq_base: Option<f64>,
    pub rope_freq_scale: Option<f64>,
    pub offload_kqv: Option<bool>,
    pub swa_full: Option<bool>,
    pub flash_attention: Option<FlashAttentionPolicy>,
    pub kv_type: Option<String>,
    pub kv_type_k: Option<String>,
    pub kv_type_v: Option<String>,
    pub mmproj_path: Option<String>,
    pub chat_template_override: Option<String>,
    pub chat_template_preset: Option<String>,
    pub raw_completion_fallback: bool,
    pub strict_mode: bool,
    pub mtp_enabled: bool,
    pub mtp_draft_tokens: u32,
    pub mtp_model_path: Option<String>,
    pub mtp_placement: String,
    pub dflash_enabled: bool,
    pub dflash_draft_tokens: u32,
    pub dflash_min_probability: f32,
    pub dflash_model_path: Option<String>,
}

/// The chat template options the prompt builder receives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedThinking {
    pub reasoning_format: Option<String>,
    pub thinking_directive: Option<bool>,
    pub enable_thinking: bool,
    pub chat_template_kwargs: Option<String>,
    pub parallel_tool_calls: bool,
}

impl LlamaGenerationRequest {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Tools only count when the list is non-empty.
    #[must_use]
    pub fn active_tools(&self) -> Option<&Value> {
        self.tools
            .as_ref()
            .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
    }

    #[must_use]
    pub fn max_tokens(&self) -> u32 {
        self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)
    }

    #[must_use]
    pub fn requested_context(&self) -> Option<u32> {
        self.context_length.filter(|value| *value > 0)
    }

    #[must_use]
    pub fn prompt_cache_key(&self) -> Option<String> {
        self.prompt_cache_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(ToOwned::to_owned)
    }

    #[must_use]
    pub fn stop_sequences(&self) -> Vec<String> {
        self.stop
            .iter()
            .filter(|stop| !stop.is_empty())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn resolve_thinking(&self) -> ResolvedThinking {
        let reasoning_format = self
            .reasoning
            .reasoning_format
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                self.reasoning
                    .reasoning_configured
                    .then(|| "auto".to_string())
            });
        let thinking_directive = message_thinking_directive(&self.messages);
        let explicit = self.reasoning.enable_thinking.or_else(|| {
            self.reasoning
                .chat_template_kwargs
                .as_ref()
                .and_then(|kwargs| kwargs.get("enable_thinking"))
                .and_then(Value::as_bool)
        });
        let enable_thinking = thinking_directive
            .or(explicit)
            .unwrap_or(reasoning_format.is_some());
        let mut kwargs = self
            .reasoning
            .chat_template_kwargs
            .clone()
            .unwrap_or_default();
        kwargs.insert("enable_thinking".to_string(), json!(enable_thinking));
        ResolvedThinking {
            reasoning_format,
            thinking_directive,
            enable_thinking,
            chat_template_kwargs: serde_json::to_string(&kwargs).ok(),
            parallel_tool_calls: self.reasoning.parallel_tool_calls,
        }
    }

    #[must_use]
    pub fn resolve_sampling(&self) -> ResolvedSampling {
        let input = &self.sampling;
        let defaults = if input.disable_profile_defaults {
            crate::sampler_profile::SamplerProfileDefaults {
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
            crate::sampler_profile::sampler_profile_defaults(
                input
                    .profile
                    .as_deref()
                    .and_then(crate::sampler_profile::normalize_sampler_profile),
            )
        };
        ResolvedSampling {
            profile: defaults.name,
            order: input.order.clone(),
            temperature: input.temperature.unwrap_or(defaults.temperature),
            top_p: input.top_p.unwrap_or(defaults.top_p),
            top_k: input.top_k.filter(|value| *value > 0).or(defaults.top_k),
            min_p: input.min_p.or(defaults.min_p),
            typical_p: input.typical_p.or(defaults.typical_p),
            frequency_penalty: input.frequency_penalty.or(defaults.frequency_penalty),
            presence_penalty: input.presence_penalty.or(defaults.presence_penalty),
            repeat_penalty: input
                .repeat_penalty
                .filter(|value| (0.0..=2.0).contains(value)),
            n_pen_range: input
                .n_pen_range
                .and_then(|value| i32::try_from(value).ok())
                .filter(|value| (-1..=262_144).contains(value)),
            dry_multiplier: input.dry_multiplier,
            dry_base: input.dry_base,
            dry_allowed_length: input.dry_allowed_length,
            dry_penalty_last_n: input
                .dry_penalty_last_n
                .and_then(|value| i32::try_from(value).ok()),
            dry_sequence_breakers: input
                .dry_sequence_breakers
                .as_ref()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| decode_llama_sequence_breaker(item))
                        .filter(|item| !item.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|items| !items.is_empty()),
            xtc_probability: input.xtc_probability,
            xtc_threshold: input.xtc_threshold,
            seed: input.seed,
            adaptive_target: input.adaptive_target,
            adaptive_decay: input.adaptive_decay,
        }
    }

    #[must_use]
    pub fn resolve_runtime(&self) -> ResolvedRuntime {
        let input = &self.runtime;
        let mut gpu_device_ids = Vec::new();
        for id in &input.gpu_device_ids {
            if !gpu_device_ids.contains(id) {
                gpu_device_ids.push(*id);
            }
        }
        let mut gpu_manual_layers: Vec<(usize, u32)> = Vec::new();
        for (device_id, layers) in &input.gpu_manual_layers {
            if !gpu_manual_layers.iter().any(|(id, _)| id == device_id) {
                gpu_manual_layers.push((*device_id, *layers));
            }
        }
        let batch_size = input
            .batch_size
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_BATCH_SIZE);
        let ubatch_size = input.ubatch_size.filter(|value| *value > 0);
        ResolvedRuntime {
            gpu_layers: input.gpu_layers,
            multi_gpu_enabled: input.multi_gpu_enabled,
            gpu_device_ids,
            gpu_distribution_mode: input
                .gpu_distribution_mode
                .as_deref()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| {
                    matches!(
                        value.as_str(),
                        "balanced" | "proportional" | "priority" | "manual"
                    )
                }),
            gpu_manual_layers,
            kv_placement: input
                .kv_placement
                .as_deref()
                .map(|value| value.trim().to_string())
                .filter(|value| matches!(value.as_str(), "auto" | "split" | "systemRam" | "pin")),
            main_gpu: input.main_gpu.filter(|value| *value >= 0),
            single_gpu_device_id: input.single_gpu_device_id,
            priority_vram_limit_bytes: input.priority_vram_limit_bytes.filter(|value| *value > 0),
            threads: input.threads.filter(|value| *value > 0),
            threads_batch: input.threads_batch.filter(|value| *value > 0),
            batch_size,
            ubatch_size,
            compute_batch_size: ubatch_size.unwrap_or(batch_size).min(batch_size),
            rope_freq_base: input.rope_freq_base,
            rope_freq_scale: input.rope_freq_scale,
            offload_kqv: input.offload_kqv,
            swa_full: input.swa_full,
            flash_attention: input.flash_attention,
            kv_type: normalized_kv_type(input.kv_type.as_deref()),
            kv_type_k: normalized_kv_type(input.kv_type_k.as_deref()),
            kv_type_v: normalized_kv_type(input.kv_type_v.as_deref()),
            mmproj_path: trimmed_non_empty(input.mmproj_path.as_deref()),
            chat_template_override: trimmed_non_empty(input.chat_template_override.as_deref()),
            chat_template_preset: trimmed_non_empty(input.chat_template_preset.as_deref()),
            raw_completion_fallback: input.raw_completion_fallback,
            strict_mode: input.strict_mode,
            mtp_enabled: input.mtp_enabled,
            mtp_draft_tokens: input
                .mtp_draft_tokens
                .filter(|value| *value > 0)
                .unwrap_or(MTP_DRAFT_DEFAULT)
                .min(MTP_DRAFT_MAX),
            mtp_model_path: trimmed_non_empty(input.mtp_model_path.as_deref()),
            mtp_placement: input
                .mtp_placement
                .as_deref()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| matches!(value.as_str(), "auto" | "gpu" | "cpu"))
                .unwrap_or_else(|| "auto".to_string()),
            dflash_enabled: input.dflash_enabled,
            dflash_draft_tokens: input
                .dflash_draft_tokens
                .filter(|value| *value > 0)
                .unwrap_or(DFLASH_DRAFT_DEFAULT)
                .min(DFLASH_DRAFT_MAX),
            dflash_min_probability: input
                .dflash_min_probability
                .map(|value| value as f32)
                .filter(|value| (0.0..=1.0).contains(value))
                .unwrap_or(DFLASH_P_MIN_DEFAULT),
            dflash_model_path: trimmed_non_empty(input.dflash_model_path.as_deref()),
        }
    }
}

fn normalized_kv_type(value: Option<&str>) -> Option<String> {
    value.map(|value| value.trim().to_ascii_lowercase())
}

impl ResolvedRuntime {
    /// Separate K/V cache types when either is set, else the shared type.
    #[must_use]
    pub fn kv_types(&self) -> KvCacheTypes<'_> {
        KvCacheTypes::from_settings(
            self.kv_type.as_deref(),
            self.kv_type_k.as_deref(),
            self.kv_type_v.as_deref(),
        )
    }
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// A trailing `/think` or `/no_think` in the last user message.
#[must_use]
pub fn message_thinking_directive(messages: &[Value]) -> Option<bool> {
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

#[must_use]
pub fn decode_llama_sequence_breaker(value: &str) -> String {
    match value.trim() {
        "\\n" => "\n".to_string(),
        "\\r" => "\r".to_string(),
        "\\t" => "\t".to_string(),
        "\\\"" => "\"".to_string(),
        "\\\\" => "\\".to_string(),
        other => other.to_string(),
    }
}

/// Finds the earliest stop sequence that the newest text could complete.
#[derive(Debug)]
pub struct IncrementalStopMatcher<'a> {
    stop_sequences: &'a [String],
    pub max_len: usize,
}

impl<'a> IncrementalStopMatcher<'a> {
    #[must_use]
    pub fn new(stop_sequences: &'a [String]) -> Self {
        Self {
            max_len: stop_sequences.iter().map(String::len).max().unwrap_or(0),
            stop_sequences,
        }
    }

    #[must_use]
    pub fn find(&self, text: &str, appended_from: usize) -> Option<usize> {
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
                    .find(stop.as_str())
                    .map(|index| search_start + index)
            })
            .min()
    }

    /// How far text can be released without cutting a possible stop.
    #[must_use]
    pub fn safe_end(&self, text: &str, reached_stop: bool) -> usize {
        if reached_stop || self.max_len == 0 {
            text.len()
        } else {
            clamp_to_char_boundary(text, text.len().saturating_sub(self.max_len - 1))
        }
    }
}

#[must_use]
pub fn should_flush_stream(
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

#[must_use]
pub fn clamp_to_char_boundary(text: &str, index: usize) -> usize {
    let mut clamped = index.min(text.len());
    while clamped > 0 && !text.is_char_boundary(clamped) {
        clamped -= 1;
    }
    clamped
}

/// How many of the oldest cache entries must go so `incoming_bytes` fits
/// both the capacity and, when known, the free memory.
#[must_use]
pub fn cache_eviction_count(
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
            .is_none_or(|available| incoming_bytes <= available.saturating_add(freed_bytes));
        if within_capacity && within_headroom {
            break;
        }
        allocated_bytes = allocated_bytes.saturating_sub(bytes);
        freed_bytes = freed_bytes.saturating_add(bytes);
        evictions += 1;
    }
    evictions
}

#[must_use]
pub fn common_token_prefix<T: PartialEq>(left: &[T], right: &[T]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

/// The identity of a context's shape; a cached context is reused only for
/// the same key.
#[derive(Clone, Copy, Debug)]
pub struct TextContextShape<'a> {
    pub n_ctx: u32,
    pub n_batch: u32,
    pub n_ubatch: Option<u32>,
    pub n_outputs_max: u32,
    pub n_threads: Option<u32>,
    pub n_threads_batch: Option<u32>,
    pub offload_kqv: Option<bool>,
    pub swa_full: Option<bool>,
    pub kv_type: Option<&'a str>,
    pub flash_attention: FlashAttentionPolicy,
    pub rope_freq_base: Option<f64>,
    pub rope_freq_scale: Option<f64>,
    pub mtp_active: bool,
    pub mtp_draft_tokens: u32,
    pub dflash_active: bool,
    pub dflash_draft_tokens: u32,
}

impl TextContextShape<'_> {
    #[must_use]
    pub fn key(&self) -> String {
        let flash = match self.flash_attention {
            FlashAttentionPolicy::Auto => "auto",
            FlashAttentionPolicy::Disabled => "disabled",
            FlashAttentionPolicy::Enabled => "enabled",
        };
        format!(
            "ctx={};batch={};ubatch={:?};outputs={};threads={:?};threads_batch={:?};kqv={:?};swa={:?};kv={};flash={flash};rope_base={:?};rope_scale={:?};mtp={};mtp_n={};dflash={};dflash_n={}",
            self.n_ctx,
            self.n_batch,
            self.n_ubatch,
            self.n_outputs_max,
            self.n_threads,
            self.n_threads_batch,
            self.offload_kqv,
            self.swa_full,
            self.kv_type.unwrap_or("f16"),
            self.rope_freq_base,
            self.rope_freq_scale,
            self.mtp_active,
            self.mtp_draft_tokens,
            self.dflash_active,
            self.dflash_draft_tokens,
        )
    }
}

/// llama-server's rule for reusing a cached sequence from `resume_at`: with
/// a windowed sliding-window cache (`n_swa` > 0, the model's window unless
/// the SWA layers get a full-size cache) the cells the window needs may be
/// gone, and a recurrent state only holds its latest position. The cache is
/// unusable when its oldest position `pos_min` lies after
/// `resume_at - n_swa`; an empty cache (`pos_min` -1) or a resume at 0 never
/// blocks.
#[must_use]
pub fn prompt_cache_reuse_blocked(pos_min: i32, resume_at: i32, n_swa: u32) -> bool {
    let n_swa = i32::try_from(n_swa).unwrap_or(i32::MAX);
    pos_min >= 0 && resume_at > 0 && pos_min > resume_at.saturating_sub(n_swa).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_messages(messages: Vec<Value>) -> LlamaGenerationRequest {
        LlamaGenerationRequest {
            messages,
            ..LlamaGenerationRequest::default()
        }
    }

    #[test]
    fn trailing_directive_overrides_the_explicit_flag() {
        let mut request = request_with_messages(vec![
            json!({"role": "user", "content": "first /think"}),
            json!({"role": "assistant", "content": "ok"}),
            json!({"role": "user", "content": [{"type": "text", "text": "again"}, {"type": "text", "text": "please /NO_THINK"}]}),
        ]);
        request.reasoning.enable_thinking = Some(true);
        let thinking = request.resolve_thinking();
        assert_eq!(thinking.thinking_directive, Some(false));
        assert!(!thinking.enable_thinking);
        assert_eq!(
            thinking.chat_template_kwargs.as_deref(),
            Some("{\"enable_thinking\":false}")
        );
    }

    #[test]
    fn thinking_follows_the_flag_then_the_reasoning_format() {
        let mut request = request_with_messages(vec![json!({"role": "user", "content": "hi"})]);
        let mut kwargs = Map::new();
        kwargs.insert("enable_thinking".into(), json!(true));
        kwargs.insert("keep".into(), json!(1));
        request.reasoning.chat_template_kwargs = Some(kwargs);
        let thinking = request.resolve_thinking();
        assert!(thinking.enable_thinking);
        assert_eq!(thinking.reasoning_format, None);
        assert_eq!(
            thinking.chat_template_kwargs.as_deref(),
            Some("{\"enable_thinking\":true,\"keep\":1}")
        );

        let mut configured = request_with_messages(vec![]);
        configured.reasoning.reasoning_configured = true;
        let thinking = configured.resolve_thinking();
        assert_eq!(thinking.reasoning_format.as_deref(), Some("auto"));
        assert!(thinking.enable_thinking);

        configured.reasoning.reasoning_format = Some("  deepseek ".into());
        assert_eq!(
            configured.resolve_thinking().reasoning_format.as_deref(),
            Some("deepseek")
        );
    }

    #[test]
    fn sampling_applies_profile_defaults_and_filters() {
        let mut request = LlamaGenerationRequest::default();
        request.sampling.profile = Some("creative".into());
        request.sampling.top_k = Some(0);
        request.sampling.repeat_penalty = Some(2.5);
        request.sampling.n_pen_range = Some(300_000);
        request.sampling.dry_sequence_breakers = Some(vec!["\\n".into(), "  ".into()]);
        let sampling = request.resolve_sampling();
        let defaults = crate::sampler_profile::sampler_profile_defaults(Some("creative"));
        assert_eq!(sampling.profile, defaults.name);
        assert_eq!(sampling.temperature, defaults.temperature);
        assert_eq!(sampling.top_k, defaults.top_k);
        assert_eq!(sampling.repeat_penalty, None);
        assert_eq!(sampling.n_pen_range, None);
        assert_eq!(sampling.dry_sequence_breakers, Some(vec!["\n".to_string()]));

        request.sampling.disable_profile_defaults = true;
        let custom = request.resolve_sampling();
        assert_eq!(custom.profile, "custom");
        assert_eq!(custom.temperature, 0.8);
        assert_eq!(custom.top_p, 0.95);
        assert_eq!(custom.top_k, None);
    }

    #[test]
    fn runtime_filters_match_legacy() {
        let mut request = LlamaGenerationRequest::default();
        request.runtime.gpu_device_ids = vec![1, 0, 1];
        request.runtime.gpu_manual_layers = vec![(0, 10), (0, 20), (1, 5)];
        request.runtime.gpu_distribution_mode = Some(" Manual ".into());
        request.runtime.kv_placement = Some("SystemRam".into());
        request.runtime.batch_size = Some(0);
        request.runtime.ubatch_size = Some(1024);
        request.runtime.mtp_draft_tokens = Some(99);
        request.runtime.mtp_placement = Some("GPU".into());
        request.runtime.chat_template_preset = Some("  ".into());
        let runtime = request.resolve_runtime();
        assert_eq!(runtime.gpu_device_ids, vec![1, 0]);
        assert_eq!(runtime.gpu_manual_layers, vec![(0, 10), (1, 5)]);
        assert_eq!(runtime.gpu_distribution_mode.as_deref(), Some("manual"));
        assert_eq!(runtime.kv_placement, None);
        assert_eq!(runtime.batch_size, 512);
        assert_eq!(runtime.compute_batch_size, 512);
        assert_eq!(runtime.mtp_draft_tokens, MTP_DRAFT_MAX);
        assert_eq!(runtime.mtp_placement, "gpu");
        assert_eq!(runtime.chat_template_preset, None);
        request.runtime.mtp_draft_tokens = None;
        assert_eq!(
            request.resolve_runtime().mtp_draft_tokens,
            MTP_DRAFT_DEFAULT
        );
    }

    #[test]
    fn prompt_cache_reuse_needs_the_sliding_window_still_cached() {
        assert!(!prompt_cache_reuse_blocked(0, 900, 0));
        assert!(!prompt_cache_reuse_blocked(0, 900, 512));
        assert!(!prompt_cache_reuse_blocked(388, 900, 512));
        assert!(prompt_cache_reuse_blocked(389, 900, 512));
        assert!(!prompt_cache_reuse_blocked(0, 300, 512));
        assert!(prompt_cache_reuse_blocked(10, 300, 512));
        assert!(prompt_cache_reuse_blocked(950, 900, 0));
        assert!(!prompt_cache_reuse_blocked(-1, 900, 512));
        assert!(!prompt_cache_reuse_blocked(5, 0, 512));
    }

    #[test]
    fn dflash_settings_resolve_with_the_legacy_bounds() {
        let mut request = LlamaGenerationRequest::default();
        let runtime = request.resolve_runtime();
        assert!(!runtime.dflash_enabled);
        assert_eq!(runtime.dflash_draft_tokens, DFLASH_DRAFT_DEFAULT);
        assert!((runtime.dflash_min_probability - DFLASH_P_MIN_DEFAULT).abs() < f32::EPSILON);
        assert_eq!(runtime.dflash_model_path, None);
        request.runtime.dflash_enabled = true;
        request.runtime.dflash_draft_tokens = Some(99);
        request.runtime.dflash_min_probability = Some(0.8);
        request.runtime.dflash_model_path = Some("  /m/drafter-dflash.gguf ".into());
        let runtime = request.resolve_runtime();
        assert!(runtime.dflash_enabled);
        assert_eq!(runtime.dflash_draft_tokens, DFLASH_DRAFT_MAX);
        assert!((runtime.dflash_min_probability - 0.8).abs() < f32::EPSILON);
        assert_eq!(
            runtime.dflash_model_path.as_deref(),
            Some("/m/drafter-dflash.gguf")
        );
        request.runtime.dflash_draft_tokens = Some(0);
        request.runtime.dflash_min_probability = Some(1.5);
        request.runtime.dflash_model_path = Some("   ".into());
        let runtime = request.resolve_runtime();
        assert_eq!(runtime.dflash_draft_tokens, DFLASH_DRAFT_DEFAULT);
        assert!((runtime.dflash_min_probability - DFLASH_P_MIN_DEFAULT).abs() < f32::EPSILON);
        assert_eq!(runtime.dflash_model_path, None);
    }

    #[test]
    fn stop_matcher_finds_the_earliest_stop_near_new_text() {
        let stops = vec!["END".to_string(), "\n\nUser:".to_string()];
        let matcher = IncrementalStopMatcher::new(&stops);
        assert_eq!(matcher.find("hello EN", 6), None);
        assert_eq!(matcher.find("hello END more", 8), Some(6));
        assert_eq!(matcher.safe_end("hello wor", false), 9 - 6);
        assert_eq!(matcher.safe_end("hello wor", true), 9);
        let none: Vec<String> = Vec::new();
        assert_eq!(IncrementalStopMatcher::new(&none).find("END", 0), None);
    }

    #[test]
    fn stream_flushes_first_then_by_size_or_interval() {
        assert!(!should_flush_stream(0, false, Duration::ZERO, true));
        assert!(should_flush_stream(1, false, Duration::ZERO, false));
        assert!(!should_flush_stream(
            10,
            true,
            Duration::from_millis(5),
            false
        ));
        assert!(should_flush_stream(
            STREAM_EMIT_BYTES,
            true,
            Duration::ZERO,
            false
        ));
        assert!(should_flush_stream(1, true, STREAM_EMIT_INTERVAL, false));
        assert!(should_flush_stream(1, true, Duration::ZERO, true));
    }

    #[test]
    fn cache_evicts_oldest_until_capacity_and_headroom_fit() {
        assert_eq!(cache_eviction_count(60, 30, [20, 20, 20], 100, None), 0);
        assert_eq!(cache_eviction_count(90, 30, [20, 20, 50], 100, None), 1);
        assert_eq!(cache_eviction_count(40, 30, [20, 20], 100, Some(10)), 1);
        assert_eq!(cache_eviction_count(40, 60, [20, 20], 100, Some(10)), 2);
    }

    #[test]
    fn context_key_and_prefix_match_legacy() {
        let shape = TextContextShape {
            n_ctx: 4096,
            n_batch: 512,
            n_ubatch: Some(256),
            n_outputs_max: 1,
            n_threads: None,
            n_threads_batch: Some(8),
            offload_kqv: Some(true),
            swa_full: None,
            kv_type: None,
            flash_attention: FlashAttentionPolicy::Auto,
            rope_freq_base: None,
            rope_freq_scale: Some(1.0),
            mtp_active: false,
            mtp_draft_tokens: 4,
            dflash_active: true,
            dflash_draft_tokens: 6,
        };
        assert_eq!(
            shape.key(),
            "ctx=4096;batch=512;ubatch=Some(256);outputs=1;threads=None;threads_batch=Some(8);kqv=Some(true);swa=None;kv=f16;flash=auto;rope_base=None;rope_scale=Some(1.0);mtp=false;mtp_n=4;dflash=true;dflash_n=6"
        );
        assert_eq!(common_token_prefix(&[1, 2, 3], &[1, 2, 4, 5]), 2);
        assert_eq!(common_token_prefix::<u8>(&[], &[1]), 0);
    }
}
