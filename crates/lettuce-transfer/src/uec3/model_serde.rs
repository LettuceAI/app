use lettuce_models::*;
use lettuce_types::TimestampMillis;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "ChatParameterProfile", rename_all = "camelCase", default)]
pub(super) struct WireChatParameterProfile {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repetition_penalty: Option<f64>,
    pub reasoning_mode: Option<ReasoningMode>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_budget_tokens: Option<u32>,
    pub prompt_caching: Option<PromptCaching>,
    pub send_thinking_state: Option<bool>,
    #[serde(with = "WireOllamaOptions")]
    pub ollama: OllamaOptions,
    #[serde(with = "WireOpenRouterOptions")]
    pub openrouter: OpenRouterOptions,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "OpenRouterOptions", rename_all = "camelCase", default)]
pub(super) struct WireOpenRouterOptions {
    pub pinned_provider: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "OllamaOptions", rename_all = "camelCase", default)]
pub(super) struct WireOllamaOptions {
    pub num_ctx: Option<u32>,
    pub num_predict: Option<u32>,
    pub num_keep: Option<u32>,
    pub num_batch: Option<u32>,
    pub num_gpu: Option<u32>,
    pub num_thread: Option<u32>,
    pub tfs_z: Option<f64>,
    pub typical_p: Option<f64>,
    pub min_p: Option<f64>,
    pub mirostat: Option<u32>,
    pub mirostat_tau: Option<f64>,
    pub mirostat_eta: Option<f64>,
    pub seed: Option<u32>,
    pub stop: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ChatParameterOverrides", rename_all = "camelCase", default)]
pub(super) struct WireChatParameterOverrides {
    pub temperature: ParameterOverride<f64>,
    pub top_p: ParameterOverride<f64>,
    pub top_k: ParameterOverride<u32>,
    pub max_output_tokens: ParameterOverride<u32>,
    pub context_length: ParameterOverride<u32>,
    pub frequency_penalty: ParameterOverride<f64>,
    pub presence_penalty: ParameterOverride<f64>,
    pub repetition_penalty: ParameterOverride<f64>,
    pub reasoning_mode: ParameterOverride<ReasoningMode>,
    pub reasoning_effort: ParameterOverride<ReasoningEffort>,
    pub reasoning_budget_tokens: ParameterOverride<u32>,
    pub prompt_caching: ParameterOverride<PromptCaching>,
    pub send_thinking_state: ParameterOverride<bool>,
    #[serde(with = "WireOllamaOptionOverrides")]
    pub ollama: OllamaOptionOverrides,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "OllamaOptionOverrides", rename_all = "camelCase", default)]
pub(super) struct WireOllamaOptionOverrides {
    pub num_ctx: ParameterOverride<u32>,
    pub num_predict: ParameterOverride<u32>,
    pub num_keep: ParameterOverride<u32>,
    pub num_batch: ParameterOverride<u32>,
    pub num_gpu: ParameterOverride<u32>,
    pub num_thread: ParameterOverride<u32>,
    pub tfs_z: ParameterOverride<f64>,
    pub typical_p: ParameterOverride<f64>,
    pub min_p: ParameterOverride<f64>,
    pub mirostat: ParameterOverride<u32>,
    pub mirostat_tau: ParameterOverride<f64>,
    pub mirostat_eta: ParameterOverride<f64>,
    pub seed: ParameterOverride<u32>,
    pub stop: ParameterOverride<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "LlamaSamplerSettings", rename_all = "camelCase", default)]
pub(super) struct WireLlamaSamplerSettings {
    pub profile: Option<LlamaSamplerProfile>,
    pub order: Option<Vec<LlamaSamplerStage>>,
    pub min_p: Option<f64>,
    pub typical_p: Option<f64>,
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

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "FeatureGenerationParameters",
    rename_all = "camelCase",
    default
)]
pub(super) struct WireFeatureGenerationParameters {
    #[serde(with = "WireChatParameterOverrides")]
    pub parameters: ChatParameterOverrides,
    #[serde(with = "WireLlamaSamplerSettings")]
    pub llama_sampler: LlamaSamplerSettings,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "FeatureParameters", rename_all = "camelCase", default)]
pub(super) struct WireFeatureParameters {
    #[serde(with = "WireFeatureGenerationParameters")]
    pub dynamic_memory: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub companion_soul_writer: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub companion_memory: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub lorebook_entry_generator: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub lorebook_generator: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub scene_writer: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub help_me_reply: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub group_speaker_selection: FeatureGenerationParameters,
    #[serde(with = "WireFeatureGenerationParameters")]
    pub creation_helper: FeatureGenerationParameters,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ModelCapabilities", rename_all = "camelCase", default)]
pub(super) struct WireModelCapabilities {
    pub format_version: u32,
    #[serde(with = "WireCapabilityEvidence")]
    pub evidence: CapabilityEvidence,
    #[serde(with = "WireModalityCapabilities")]
    pub input_modalities: ModalityCapabilities,
    #[serde(with = "WireModalityCapabilities")]
    pub output_modalities: ModalityCapabilities,
    pub streaming: CapabilityStatus,
    pub tools: CapabilityStatus,
    pub structured_output: CapabilityStatus,
    pub reasoning: CapabilityStatus,
    pub prompt_cache: CapabilityStatus,
    pub context_length: Option<u32>,
    pub max_visible_output_tokens: Option<u32>,
    pub max_total_completion_tokens: Option<u32>,
    #[serde(with = "WireParameterSupport")]
    pub parameter_support: ParameterSupport,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "CapabilityEvidence", rename_all = "camelCase", default)]
pub(super) struct WireCapabilityEvidence {
    pub source: CapabilityEvidenceSource,
    pub source_version: u32,
    pub observed_at: TimestampMillis,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ModalityCapabilities", rename_all = "camelCase", default)]
pub(super) struct WireModalityCapabilities {
    pub text: CapabilityStatus,
    pub image: CapabilityStatus,
    pub audio: CapabilityStatus,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ParameterSupport", rename_all = "camelCase", default)]
pub(super) struct WireParameterSupport {
    pub temperature: CapabilityStatus,
    pub top_p: CapabilityStatus,
    pub top_k: CapabilityStatus,
    pub frequency_penalty: CapabilityStatus,
    pub presence_penalty: CapabilityStatus,
    pub repetition_penalty: CapabilityStatus,
}

impl Default for WireChatParameterProfile {
    fn default() -> Self {
        let value = ChatParameterProfile::default();
        Self {
            temperature: value.temperature,
            top_p: value.top_p,
            top_k: value.top_k,
            max_output_tokens: value.max_output_tokens,
            context_length: value.context_length,
            frequency_penalty: value.frequency_penalty,
            presence_penalty: value.presence_penalty,
            repetition_penalty: value.repetition_penalty,
            reasoning_mode: value.reasoning_mode,
            reasoning_effort: value.reasoning_effort,
            reasoning_budget_tokens: value.reasoning_budget_tokens,
            prompt_caching: value.prompt_caching,
            send_thinking_state: value.send_thinking_state,
            ollama: value.ollama,
            openrouter: value.openrouter,
        }
    }
}

impl Default for WireOpenRouterOptions {
    fn default() -> Self {
        let value = OpenRouterOptions::default();
        Self {
            pinned_provider: value.pinned_provider,
        }
    }
}

impl Default for WireOllamaOptions {
    fn default() -> Self {
        let value = OllamaOptions::default();
        Self {
            num_ctx: value.num_ctx,
            num_predict: value.num_predict,
            num_keep: value.num_keep,
            num_batch: value.num_batch,
            num_gpu: value.num_gpu,
            num_thread: value.num_thread,
            tfs_z: value.tfs_z,
            typical_p: value.typical_p,
            min_p: value.min_p,
            mirostat: value.mirostat,
            mirostat_tau: value.mirostat_tau,
            mirostat_eta: value.mirostat_eta,
            seed: value.seed,
            stop: value.stop,
        }
    }
}

impl Default for WireChatParameterOverrides {
    fn default() -> Self {
        let value = ChatParameterOverrides::default();
        Self {
            temperature: value.temperature,
            top_p: value.top_p,
            top_k: value.top_k,
            max_output_tokens: value.max_output_tokens,
            context_length: value.context_length,
            frequency_penalty: value.frequency_penalty,
            presence_penalty: value.presence_penalty,
            repetition_penalty: value.repetition_penalty,
            reasoning_mode: value.reasoning_mode,
            reasoning_effort: value.reasoning_effort,
            reasoning_budget_tokens: value.reasoning_budget_tokens,
            prompt_caching: value.prompt_caching,
            send_thinking_state: value.send_thinking_state,
            ollama: value.ollama,
        }
    }
}

impl Default for WireOllamaOptionOverrides {
    fn default() -> Self {
        let value = OllamaOptionOverrides::default();
        Self {
            num_ctx: value.num_ctx,
            num_predict: value.num_predict,
            num_keep: value.num_keep,
            num_batch: value.num_batch,
            num_gpu: value.num_gpu,
            num_thread: value.num_thread,
            tfs_z: value.tfs_z,
            typical_p: value.typical_p,
            min_p: value.min_p,
            mirostat: value.mirostat,
            mirostat_tau: value.mirostat_tau,
            mirostat_eta: value.mirostat_eta,
            seed: value.seed,
            stop: value.stop,
        }
    }
}

impl Default for WireLlamaSamplerSettings {
    fn default() -> Self {
        let value = LlamaSamplerSettings::default();
        Self {
            profile: value.profile,
            order: value.order,
            min_p: value.min_p,
            typical_p: value.typical_p,
            repeat_penalty: value.repeat_penalty,
            n_pen_range: value.n_pen_range,
            dry_multiplier: value.dry_multiplier,
            dry_base: value.dry_base,
            dry_allowed_length: value.dry_allowed_length,
            dry_penalty_last_n: value.dry_penalty_last_n,
            dry_sequence_breakers: value.dry_sequence_breakers,
            xtc_probability: value.xtc_probability,
            xtc_threshold: value.xtc_threshold,
            seed: value.seed,
            adaptive_target: value.adaptive_target,
            adaptive_decay: value.adaptive_decay,
        }
    }
}

impl Default for WireFeatureGenerationParameters {
    fn default() -> Self {
        let value = FeatureGenerationParameters::default();
        Self {
            parameters: value.parameters,
            llama_sampler: value.llama_sampler,
        }
    }
}

impl Default for WireFeatureParameters {
    fn default() -> Self {
        let value = FeatureParameters::default();
        Self {
            dynamic_memory: value.dynamic_memory,
            companion_soul_writer: value.companion_soul_writer,
            companion_memory: value.companion_memory,
            lorebook_entry_generator: value.lorebook_entry_generator,
            lorebook_generator: value.lorebook_generator,
            scene_writer: value.scene_writer,
            help_me_reply: value.help_me_reply,
            group_speaker_selection: value.group_speaker_selection,
            creation_helper: value.creation_helper,
        }
    }
}

impl Default for WireModelCapabilities {
    fn default() -> Self {
        let value = ModelCapabilities::default();
        Self {
            format_version: value.format_version,
            evidence: value.evidence,
            input_modalities: value.input_modalities,
            output_modalities: value.output_modalities,
            streaming: value.streaming,
            tools: value.tools,
            structured_output: value.structured_output,
            reasoning: value.reasoning,
            prompt_cache: value.prompt_cache,
            context_length: value.context_length,
            max_visible_output_tokens: value.max_visible_output_tokens,
            max_total_completion_tokens: value.max_total_completion_tokens,
            parameter_support: value.parameter_support,
        }
    }
}

impl Default for WireCapabilityEvidence {
    fn default() -> Self {
        let value = CapabilityEvidence::default();
        Self {
            source: value.source,
            source_version: value.source_version,
            observed_at: value.observed_at,
        }
    }
}

impl Default for WireModalityCapabilities {
    fn default() -> Self {
        let value = ModalityCapabilities::default();
        Self {
            text: value.text,
            image: value.image,
            audio: value.audio,
        }
    }
}

impl Default for WireParameterSupport {
    fn default() -> Self {
        let value = ParameterSupport::default();
        Self {
            temperature: value.temperature,
            top_p: value.top_p,
            top_k: value.top_k,
            frequency_penalty: value.frequency_penalty,
            presence_penalty: value.presence_penalty,
            repetition_penalty: value.repetition_penalty,
        }
    }
}
