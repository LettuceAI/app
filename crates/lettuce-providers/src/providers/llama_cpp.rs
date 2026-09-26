//! The embedded llama.cpp runtime as an inference provider. The request
//! carries OpenAI-shaped messages (an assistant tool-call turn has null
//! content when it has no text, tool results are `tool` messages with their
//! call id), the sampler and runtime settings resolved on the chat profile,
//! `parallel_tool_calls` on whenever tools are offered, the output cap plus the
//! reasoning budget, the reasoning request that turns on the template's
//! reasoning format, and the thinking switch only when the model asks for it.
//! With streaming turned off for the account or the model the request runs
//! unstreamed instead of failing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProposedToolCall, ProviderContextPart,
    ProviderFailure, ProviderFailureKind, ToolChoice, ToolRequest,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_local_llm::engine::{EngineObserver, ModelLoadProgress};
use lettuce_local_llm::generation::{
    GenerationHeartbeat, GenerationObserver, LlamaFinishReason, LlamaGenerationError,
    LlamaGenerationOutput, LlamaHost, LlamaHostEvent, LlamaNotice, LlamaRuntime,
    RuntimeReportStore,
};
use lettuce_local_llm::offload::FlashAttentionPolicy;
use lettuce_local_llm::request::{
    LlamaGenerationRequest, LlamaReasoningInput, LlamaRuntimeInput, LlamaSamplingInput,
};
use lettuce_local_llm::tool_calls::LocalToolCall;
use lettuce_models::{
    LlamaCppSettings, LlamaFlashAttention, LlamaGpuDistributionMode, LlamaKvPlacement, LlamaKvType,
    LlamaMtpPlacement, LlamaSamplerProfile, LlamaSamplerStage, ReasoningMode,
    ResolvedLlamaSettings,
};
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::common::{AdapterError, FALLBACK_MAX_OUTPUT_TOKENS};
use crate::media::{
    Attachments, ProviderMedia, ProviderMediaSource, allowed_inputs, load_attachments,
    openai_content_parts,
};
use crate::streaming::stream_normalize::StreamDelta;

const LOCAL_FAILURE_CODE: &str = "LOCAL_INFERENCE_FAILED";
const LOCAL_MODEL_NOT_PICKED_CODE: &str = "LOCAL_MODEL_FILE_NOT_PICKED";

/// Another local runtime that must give way before llama.cpp runs; the
/// stable-diffusion.cpp server is stopped before every llama.cpp request.
#[async_trait::async_trait]
pub trait LocalRuntimeExclusion: Send + Sync {
    /// Fails the llama.cpp request when the other runtime could not stop.
    async fn before_local_llama(&self) -> Result<(), String>;
}

/// The embedded runtime and the application services it reports to.
#[derive(Clone)]
pub struct LocalLlama {
    runtime: Arc<LlamaRuntime>,
    host: Arc<dyn LlamaHost>,
    exclusion: Option<Arc<dyn LocalRuntimeExclusion>>,
}

impl std::fmt::Debug for LocalLlama {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LocalLlama").finish_non_exhaustive()
    }
}

impl LocalLlama {
    #[must_use]
    pub fn new(runtime: Arc<LlamaRuntime>, host: Arc<dyn LlamaHost>) -> Self {
        Self {
            runtime,
            host,
            exclusion: None,
        }
    }

    #[must_use]
    pub fn with_exclusion(mut self, exclusion: Arc<dyn LocalRuntimeExclusion>) -> Self {
        self.exclusion = Some(exclusion);
        self
    }

    #[must_use]
    pub fn runtime(&self) -> &Arc<LlamaRuntime> {
        &self.runtime
    }
}

struct ForwardingObserver {
    request_id: Option<String>,
    host: Arc<dyn LlamaHost>,
    deltas: mpsc::UnboundedSender<StreamDelta>,
}

impl EngineObserver for ForwardingObserver {
    fn model_load_progress(&self, progress: ModelLoadProgress) {
        self.host.event(LlamaHostEvent::ModelLoadProgress(progress));
    }

    fn gpu_fallback(&self) {
        self.host.event(LlamaHostEvent::GpuFallback);
    }
}

impl GenerationObserver for ForwardingObserver {
    fn delta(&self, text: &str) {
        let _ = self.deltas.send(StreamDelta::Text(text.to_owned()));
    }

    fn reasoning(&self, text: &str) {
        let _ = self.deltas.send(StreamDelta::Reasoning(text.to_owned()));
    }

    fn tool_calls(&self, _calls: &[LocalToolCall]) {}

    fn heartbeat(&self, heartbeat: GenerationHeartbeat) {
        self.host.event(LlamaHostEvent::Heartbeat {
            request_id: self.request_id.clone(),
            heartbeat,
        });
    }

    fn notice(&self, notice: LlamaNotice) {
        self.host.event(LlamaHostEvent::Notice(notice));
    }

    fn runtime_report_updated(&self, model_path: &str) {
        self.host.event(LlamaHostEvent::RuntimeReportUpdated {
            model_path: model_path.to_owned(),
        });
    }
}

pub(crate) async fn run(
    local: &LocalLlama,
    media: Option<Arc<dyn ProviderMediaSource>>,
    runtime: &dyn InferenceRuntimePort,
    request: InferenceRequest,
) -> Result<InferenceOutcome, AdapterError> {
    request.validate().map_err(|_| AdapterError::Rejected)?;
    if request.profile.output_policy != lettuce_conversations::OutputPolicy::Plain {
        return Err(AdapterError::Rejected);
    }
    let profile = &request.profile.chat_profile;
    let llama = profile.llama_cpp.as_deref().ok_or(AdapterError::Rejected)?;
    if profile.external_model_id == lettuce_models::UNPICKED_LOCAL_MODEL_FILE {
        return Err(AdapterError::Provider(ProviderFailure {
            kind: ProviderFailureKind::RequestRejected,
            status: 400,
            code: Some(LOCAL_MODEL_NOT_PICKED_CODE.to_owned()),
            message: None,
            request_id: None,
        }));
    }
    if let Some(exclusion) = &local.exclusion
        && let Err(message) = exclusion.before_local_llama().await
    {
        return Err(AdapterError::Provider(ProviderFailure {
            kind: ProviderFailureKind::RequestRejected,
            status: 500,
            code: Some(LOCAL_FAILURE_CODE.to_owned()),
            message: Some(message),
            request_id: None,
        }));
    }
    let streaming = request.stream_sink.is_some()
        && profile.streaming_enabled
        && llama.settings.streaming_enabled != Some(false);
    let attachments = load_attachments(&request, media).await?;
    let generation = generation_request(&request, &attachments, llama, streaming)?;
    let cancel = Arc::clone(&generation.cancel);
    let (delta_sender, mut deltas) = mpsc::unbounded_channel();
    let (done_sender, mut done) = oneshot::channel();
    let observer = Arc::new(ForwardingObserver {
        request_id: generation.request_id.clone(),
        host: Arc::clone(&local.host),
        deltas: delta_sender,
    });
    let reports: Arc<dyn RuntimeReportStore> = local.host.clone();
    local.runtime.generate(
        generation,
        observer,
        reports,
        Box::new(move |result| {
            let _ = done_sender.send(result);
        }),
    );

    let mut sequence = 0_u64;
    let mut streamed = Streamed::default();
    let mut cancel_signalled = false;
    let result = loop {
        tokio::select! {
            biased;
            Some(delta) = deltas.recv(), if streaming => {
                sequence = match emit_delta(runtime, &request, sequence, delta, &cancel, &mut streamed).await {
                    Ok(next) => next,
                    Err(AdapterError::Cancelled) => return streamed.stopped(),
                    Err(error) => return Err(error),
                };
            }
            result = &mut done => break result,
            cancelled = wait_cancelled(runtime, &request), if !cancel_signalled => {
                cancel_signalled = true;
                if cancelled {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        }
    };
    while streaming && let Ok(delta) = deltas.try_recv() {
        sequence =
            match emit_delta(runtime, &request, sequence, delta, &cancel, &mut streamed).await {
                Ok(next) => next,
                Err(AdapterError::Cancelled) => return streamed.stopped(),
                Err(error) => return Err(error),
            };
    }
    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(LlamaGenerationError::Aborted)) => return streamed.stopped(),
        Ok(Err(LlamaGenerationError::WorkerStopped)) | Err(_) => {
            return Err(AdapterError::Transport);
        }
        Ok(Err(LlamaGenerationError::Failed(message))) => {
            return Err(AdapterError::Provider(ProviderFailure {
                kind: ProviderFailureKind::RequestRejected,
                status: 500,
                code: Some(LOCAL_FAILURE_CODE.to_owned()),
                message: Some(message),
                request_id: None,
            }));
        }
    };
    if let Some(metrics) = output.metrics.clone() {
        local.host.record_metrics(metrics);
    }
    outcome(&request, output)
}

async fn wait_cancelled(runtime: &dyn InferenceRuntimePort, request: &InferenceRequest) -> bool {
    match request.cancellation {
        Some(job_id) => runtime.cancelled(job_id).await.is_ok(),
        None => std::future::pending().await,
    }
}

async fn emit_delta(
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
    sequence: u64,
    delta: StreamDelta,
    cancel: &AtomicBool,
    streamed: &mut Streamed,
) -> Result<u64, AdapterError> {
    let next = sequence.checked_add(1).ok_or(AdapterError::Transport)?;
    if let Err(error) =
        crate::streaming::streaming::emit(runtime, request, next, delta.clone()).await
    {
        cancel.store(true, Ordering::Relaxed);
        return Err(error);
    }
    match delta {
        StreamDelta::Text(text) => streamed.text.push_str(&text),
        StreamDelta::Reasoning(text) => streamed.reasoning.push_str(&text),
    }
    Ok(next)
}

/// The text and reasoning a local generation emitted before it stopped.
#[derive(Debug, Default)]
struct Streamed {
    text: String,
    reasoning: String,
}

impl Streamed {
    /// The reply a stopped generation keeps: its streamed text, or a
    /// cancellation when no visible text was streamed.
    fn stopped(self) -> Result<InferenceOutcome, AdapterError> {
        if self.text.trim().is_empty() {
            return Err(AdapterError::Cancelled);
        }
        let mut parts = Vec::with_capacity(2);
        if !self.reasoning.is_empty() {
            parts.push(MessagePart::ReasoningSummary {
                text: self.reasoning,
            });
        }
        parts.push(MessagePart::Text { text: self.text });
        let outcome = InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts,
                tool_calls: Vec::new(),
                provider_replay: None,
            }],
            usage: None,
            finish_reason: FinishReason::Cancelled,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::new(),
        };
        outcome
            .validate()
            .map_err(|_| AdapterError::MalformedResponse)?;
        Ok(outcome)
    }
}

fn generation_request(
    request: &InferenceRequest,
    attachments: &Attachments,
    llama: &ResolvedLlamaSettings,
    streaming: bool,
) -> Result<LlamaGenerationRequest, AdapterError> {
    let profile = &request.profile.chat_profile;
    let parameters = &profile.parameters;
    let settings = &llama.settings;
    let reasoning_enabled = parameters.reasoning_mode == Some(ReasoningMode::Enabled);
    let tools = request.tools.as_ref();
    let mut chat_template_kwargs = None;
    let mut enable_thinking = None;
    if parameters.send_thinking_state {
        enable_thinking = Some(reasoning_enabled);
        let mut kwargs = Map::new();
        kwargs.insert("enable_thinking".to_owned(), json!(reasoning_enabled));
        chat_template_kwargs = Some(kwargs);
    }
    Ok(LlamaGenerationRequest {
        request_id: Some(request.attempt_id.to_string()),
        model_path: profile.external_model_id.clone(),
        messages: wire_messages(request, attachments)?,
        tools: tools.map(wire_tools),
        tool_choice: tools.map(|tools| wire_tool_choice(&tools.choice)),
        stop: Vec::new(),
        stream: streaming,
        prompt_cache_key: request.prompt_cache_key.clone(),
        max_tokens: Some(parameters.total_completion_allowance.unwrap_or_else(|| {
            FALLBACK_MAX_OUTPUT_TOKENS
                .saturating_add(parameters.reasoning_budget_tokens.unwrap_or(0))
        })),
        context_length: parameters.context_length,
        reasoning: LlamaReasoningInput {
            reasoning_format: None,
            reasoning_configured: reasoning_enabled,
            enable_thinking,
            chat_template_kwargs,
            parallel_tool_calls: tools.is_some(),
            force_gemma4_reasoning: llama.settings.force_gemma4_reasoning == Some(true),
        },
        sampling: sampling(parameters, llama),
        runtime: runtime_input(settings),
        cancel: Arc::new(AtomicBool::new(false)),
    })
}

fn sampling(
    parameters: &lettuce_models::ResolvedChatParameters,
    llama: &ResolvedLlamaSettings,
) -> LlamaSamplingInput {
    let sampler = &llama.settings.sampler;
    LlamaSamplingInput {
        profile: sampler.profile.map(|profile| {
            match profile {
                LlamaSamplerProfile::Balanced => "balanced",
                LlamaSamplerProfile::Creative => "creative",
                LlamaSamplerProfile::Stable => "stable",
                LlamaSamplerProfile::Reasoning => "reasoning",
            }
            .to_owned()
        }),
        disable_profile_defaults: llama.disable_sampler_profile_defaults,
        order: sampler.order.as_ref().map(|order| {
            order
                .iter()
                .map(|stage| {
                    match stage {
                        LlamaSamplerStage::Penalties => "penalties",
                        LlamaSamplerStage::Grammar => "grammar",
                        LlamaSamplerStage::TopK => "top_k",
                        LlamaSamplerStage::TopP => "top_p",
                        LlamaSamplerStage::MinP => "min_p",
                        LlamaSamplerStage::Dry => "dry",
                        LlamaSamplerStage::Typical => "typical",
                        LlamaSamplerStage::Xtc => "xtc",
                        LlamaSamplerStage::Temp => "temp",
                        LlamaSamplerStage::AdaptiveP => "adaptive_p",
                    }
                    .to_owned()
                })
                .collect()
        }),
        temperature: parameters.temperature,
        top_p: parameters.top_p,
        top_k: parameters.top_k.filter(|value| *value > 0),
        min_p: sampler.min_p,
        typical_p: sampler.typical_p,
        frequency_penalty: parameters.frequency_penalty,
        presence_penalty: parameters.presence_penalty,
        repeat_penalty: sampler.repeat_penalty,
        n_pen_range: sampler.n_pen_range.map(i64::from),
        dry_multiplier: sampler.dry_multiplier,
        dry_base: sampler.dry_base,
        dry_allowed_length: sampler.dry_allowed_length,
        dry_penalty_last_n: sampler.dry_penalty_last_n.map(i64::from),
        dry_sequence_breakers: sampler.dry_sequence_breakers.clone(),
        xtc_probability: sampler.xtc_probability,
        xtc_threshold: sampler.xtc_threshold,
        seed: sampler.seed,
        adaptive_target: sampler.adaptive_target,
        adaptive_decay: sampler.adaptive_decay,
    }
}

fn kv_type_name(kv_type: LlamaKvType) -> String {
    serde_json::to_value(kv_type)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

fn runtime_input(settings: &LlamaCppSettings) -> LlamaRuntimeInput {
    let device = |id: u32| usize::try_from(id).unwrap_or(usize::MAX);
    LlamaRuntimeInput {
        gpu_layers: settings.gpu_layers,
        multi_gpu_enabled: settings.multi_gpu_enabled.unwrap_or(false),
        gpu_device_ids: settings
            .gpu_device_ids
            .iter()
            .flatten()
            .map(|id| device(*id))
            .collect(),
        gpu_distribution_mode: settings.gpu_distribution_mode.map(|mode| {
            match mode {
                LlamaGpuDistributionMode::Balanced => "balanced",
                LlamaGpuDistributionMode::Proportional => "proportional",
                LlamaGpuDistributionMode::Priority => "priority",
                LlamaGpuDistributionMode::Manual => "manual",
            }
            .to_owned()
        }),
        gpu_manual_layers: settings
            .gpu_manual_layers
            .iter()
            .flatten()
            .map(|assignment| (device(assignment.device_id), assignment.layers))
            .collect(),
        kv_placement: settings.kv_placement.map(|placement| {
            match placement {
                LlamaKvPlacement::Auto => "auto",
                LlamaKvPlacement::Split => "split",
                LlamaKvPlacement::SystemRam => "systemRam",
                LlamaKvPlacement::Pin => "pin",
            }
            .to_owned()
        }),
        main_gpu: settings
            .main_gpu
            .and_then(|value| i32::try_from(value).ok()),
        single_gpu_device_id: settings.single_gpu_device_id.map(device),
        priority_vram_limit_bytes: settings.priority_vram_limit_bytes,
        threads: settings.threads,
        threads_batch: settings.threads_batch,
        batch_size: settings.batch_size,
        ubatch_size: settings.ubatch_size,
        rope_freq_base: settings.rope_freq_base,
        rope_freq_scale: settings.rope_freq_scale,
        offload_kqv: settings.offload_kqv,
        swa_full: settings.swa_full,
        flash_attention: settings.flash_attention.map(|policy| match policy {
            LlamaFlashAttention::Auto => FlashAttentionPolicy::Auto,
            LlamaFlashAttention::Enabled => FlashAttentionPolicy::Enabled,
            LlamaFlashAttention::Disabled => FlashAttentionPolicy::Disabled,
        }),
        kv_type: settings.kv_type.map(kv_type_name),
        kv_type_k: settings.kv_type_k.map(kv_type_name),
        kv_type_v: settings.kv_type_v.map(kv_type_name),
        mmproj_path: settings.mmproj_path.clone(),
        chat_template_override: settings.chat_template_override.clone(),
        chat_template_preset: settings.chat_template_preset.clone(),
        raw_completion_fallback: settings.raw_completion_fallback.unwrap_or(false),
        strict_mode: settings.strict_mode.unwrap_or(false),
        mtp_enabled: settings.mtp_enabled.unwrap_or(false),
        mtp_draft_tokens: settings.mtp_draft_tokens,
        mtp_model_path: settings.mtp_model_path.clone(),
        mtp_placement: settings.mtp_placement.map(|placement| {
            match placement {
                LlamaMtpPlacement::Auto => "auto",
                LlamaMtpPlacement::Gpu => "gpu",
                LlamaMtpPlacement::Cpu => "cpu",
            }
            .to_owned()
        }),
        dflash_enabled: settings.dflash_enabled.unwrap_or(false),
        dflash_draft_tokens: settings.dflash_draft_tokens,
        dflash_min_probability: settings.dflash_min_probability,
        dflash_model_path: settings.dflash_model_path.clone(),
    }
}

fn wire_role(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System | MessageRole::Scene => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
}

fn wire_messages(
    request: &InferenceRequest,
    loaded: &Attachments,
) -> Result<Vec<Value>, AdapterError> {
    let (allow_image, allow_audio) = allowed_inputs(request);
    let mut messages = Vec::new();
    for message in &request.context.messages {
        let results = message
            .parts
            .iter()
            .filter_map(|part| match part {
                ProviderContextPart::ToolResult(result) => Some(result),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !results.is_empty() {
            if results.len() != message.parts.len() || message.role != MessageRole::User {
                return Err(AdapterError::Rejected);
            }
            for result in results {
                let call_id = result
                    .provider_call_id
                    .as_deref()
                    .ok_or(AdapterError::Rejected)?;
                messages.push(json!({
                    "role": "tool",
                    "content": serde_json::to_string(&result.output.value)
                        .map_err(|_| AdapterError::Rejected)?,
                    "tool_call_id": call_id,
                }));
            }
            continue;
        }
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        let mut attachments = Vec::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text } => content.push_str(text),
                ProviderContextPart::ToolCall(call) => {
                    if message.role != MessageRole::Assistant {
                        return Err(AdapterError::Rejected);
                    }
                    let arguments = match &call.raw_arguments {
                        Some(raw) => raw.clone(),
                        None => serde_json::to_string(&call.arguments)
                            .map_err(|_| AdapterError::Rejected)?,
                    };
                    tool_calls.push(json!({
                        "id": call.provider_call_id.as_deref().ok_or(AdapterError::Rejected)?,
                        "type": "function",
                        "function": { "name": call.name, "arguments": arguments },
                    }));
                }
                ProviderContextPart::MediaAsset { asset_id, .. } => {
                    if message.role == MessageRole::User && (allow_image || allow_audio) {
                        attachments.push(loaded.get(asset_id).cloned().unwrap_or(ProviderMedia {
                            mime_type: String::new(),
                            bytes: Vec::new(),
                        }));
                    }
                }
                ProviderContextPart::ToolResult(_) => return Err(AdapterError::Rejected),
            }
        }
        let mut wire = Map::new();
        wire.insert("role".to_owned(), json!(wire_role(message.role)));
        if !attachments.is_empty() {
            wire.insert(
                "content".to_owned(),
                openai_content_parts(&content, &attachments, (allow_image, allow_audio)),
            );
        } else if tool_calls.is_empty() {
            wire.insert("content".to_owned(), json!(content));
        } else {
            wire.insert(
                "content".to_owned(),
                if content.is_empty() {
                    Value::Null
                } else {
                    json!(content)
                },
            );
        }
        if !tool_calls.is_empty() {
            wire.insert("tool_calls".to_owned(), Value::Array(tool_calls));
        }
        messages.push(Value::Object(wire));
    }
    Ok(messages)
}

fn wire_tools(tools: &ToolRequest) -> Value {
    Value::Array(
        tools
            .definitions
            .iter()
            .map(|definition| {
                let mut function = Map::new();
                function.insert("name".to_owned(), json!(definition.name));
                function.insert("parameters".to_owned(), definition.parameters.clone());
                if let Some(description) = &definition.description {
                    function.insert("description".to_owned(), json!(description));
                }
                json!({ "type": "function", "function": function })
            })
            .collect(),
    )
}

fn wire_tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::Required => json!("required"),
        ToolChoice::Named { name } => json!({ "type": "function", "function": { "name": name } }),
    }
}

/// Local parsers accept every argument shape the chat loop executes:
/// `<parameter=k>v</parameter>` bodies, empty or `null` arguments and
/// non-JSON text. Tools read arguments by key, so anything that is not an
/// object behaves as an empty object, and the raw text is kept only when it
/// is the JSON form of the arguments it came with.
fn proposal_arguments(arguments: Value, raw_arguments: Option<String>) -> (Value, Option<String>) {
    let arguments = match arguments {
        Value::Object(_) => arguments,
        _ => Value::Object(Map::new()),
    };
    let raw_arguments = raw_arguments
        .filter(|raw| serde_json::from_str::<Value>(raw).is_ok_and(|parsed| parsed == arguments));
    (arguments, raw_arguments)
}

fn outcome(
    request: &InferenceRequest,
    output: LlamaGenerationOutput,
) -> Result<InferenceOutcome, AdapterError> {
    let tool_calls = output
        .tool_calls
        .into_iter()
        .map(|call| {
            let (arguments, raw_arguments) = proposal_arguments(call.arguments, call.raw_arguments);
            let proposal = ProposedToolCall {
                provider_call_id: Some(call.id),
                name: call.name,
                arguments,
                raw_arguments,
                provider_replay: None,
            };
            proposal
                .validate()
                .map_err(|_| AdapterError::MalformedResponse)?;
            Ok(proposal)
        })
        .collect::<Result<Vec<_>, AdapterError>>()?;
    if let Some(tools) = &request.tools {
        if tool_calls.iter().any(|call| {
            !tools
                .definitions
                .iter()
                .any(|definition| definition.name == call.name)
        }) {
            return Err(AdapterError::MalformedResponse);
        }
    } else if !tool_calls.is_empty() {
        return Err(AdapterError::MalformedResponse);
    }
    let reasoning = output.reasoning.unwrap_or_default();
    if output.content.trim().is_empty() && reasoning.trim().is_empty() && tool_calls.is_empty() {
        return Err(AdapterError::EmptyResponse);
    }
    let mut parts = Vec::new();
    if !reasoning.is_empty() {
        parts.push(MessagePart::ReasoningSummary { text: reasoning });
    }
    if !output.content.is_empty() {
        parts.push(MessagePart::Text {
            text: output.content,
        });
    }
    let mut warning_codes = Vec::new();
    let finish_reason = match output.finish_reason {
        LlamaFinishReason::Length => {
            warning_codes.push(InferenceWarningCode::Truncated);
            FinishReason::Length
        }
        LlamaFinishReason::Stop | LlamaFinishReason::ToolCalls => FinishReason::Stop,
    };
    let usage = output.usage;
    let outcome = InferenceOutcome {
        provider_response_id: None,
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts,
            tool_calls,
            provider_replay: None,
        }],
        usage: Some(InferenceUsage {
            provider_reported_cost: None,
            cache_write_tokens: Some(
                usage
                    .prompt_tokens
                    .saturating_sub(usage.cached_prompt_tokens),
            ),
            web_search_requests: None,
            cached_input_tokens: Some(usage.cached_prompt_tokens),
            reasoning_tokens: None,
            image_tokens: None,
            audio_tokens: None,
            total_tokens: None,
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
        }),
        finish_reason,
        provider_finish_reason: Some(output.finish_reason.as_str().to_owned()),
        provider_request_id: None,
        warning_codes,
    };
    outcome
        .validate()
        .map_err(|_| AdapterError::MalformedResponse)?;
    Ok(outcome)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test fixtures assert exact request and outcome mapping"
)]
mod tests {
    use lettuce_conversations::{
        ProviderNeutralMessage, ToolDefinition, ToolOutput, TranscriptToolCall,
        TranscriptToolResult,
    };
    use lettuce_local_llm::generation::{LlamaMetricsRecord, LlamaUsage};
    use lettuce_models::{LlamaCppSettings, LlamaSamplerSettings, ProviderConfig};

    use super::*;
    use crate::integration_tests::{profile, request};

    #[test]
    fn a_stopped_local_generation_keeps_its_streamed_text() {
        let kept = Streamed {
            text: "Half a rep".into(),
            reasoning: "thinking".into(),
        }
        .stopped()
        .expect("legacy useChatAbortController keeps the streamed reply on stop");
        assert_eq!(kept.finish_reason, FinishReason::Cancelled);
        assert_eq!(
            kept.candidates[0].parts,
            vec![
                MessagePart::ReasoningSummary {
                    text: "thinking".into()
                },
                MessagePart::Text {
                    text: "Half a rep".into()
                },
            ]
        );
        assert_eq!(
            Streamed {
                text: "  ".into(),
                reasoning: "thinking".into(),
            }
            .stopped(),
            Err(AdapterError::Cancelled)
        );
    }

    fn llama_request(settings: LlamaCppSettings) -> InferenceRequest {
        let mut inference = request(profile(
            "llamacpp",
            String::new(),
            ProviderConfig::Standard,
            None,
            lettuce_settings::SecretOwnerId::new(),
        ));
        inference.profile.chat_profile.provider_protocol =
            lettuce_models::ProviderProtocol::LlamaCpp;
        inference.profile.chat_profile.external_model_id = "/models/local.gguf".to_owned();
        inference.profile.chat_profile.llama_cpp = Some(Box::new(ResolvedLlamaSettings {
            settings,
            disable_sampler_profile_defaults: false,
        }));
        inference
    }

    #[test]
    fn messages_keep_the_legacy_tool_turn_shapes() {
        let mut inference = llama_request(LlamaCppSettings::default());
        let execution = lettuce_types::ToolExecutionId::new();
        inference.context.messages.push(ProviderNeutralMessage {
            role: MessageRole::Assistant,
            parts: vec![ProviderContextPart::ToolCall(TranscriptToolCall {
                execution_id: execution,
                provider_call_id: Some("call-1".to_owned()),
                name: "lookup".to_owned(),
                arguments: json!({"q": "tea"}),
                raw_arguments: None,
                provider_replay: None,
            })],
        });
        inference.context.messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::ToolResult(TranscriptToolResult {
                execution_id: execution,
                provider_call_id: Some("call-1".to_owned()),
                name: "lookup".to_owned(),
                output: ToolOutput {
                    value: json!({"found": true}),
                    is_error: false,
                },
            })],
        });
        let messages = wire_messages(&inference, &Attachments::new()).unwrap();
        assert_eq!(
            messages[1],
            json!({"role": "system", "content": "scene text"})
        );
        assert_eq!(
            messages[3],
            json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{"id": "call-1", "type": "function", "function": {"name": "lookup", "arguments": "{\"q\":\"tea\"}"}}],
            })
        );
        assert_eq!(
            messages[4],
            json!({"role": "tool", "content": "{\"found\":true}", "tool_call_id": "call-1"})
        );
    }

    #[test]
    fn requests_carry_tools_budget_thinking_and_the_cache_key() {
        let mut inference = llama_request(LlamaCppSettings {
            dflash_enabled: Some(true),
            dflash_draft_tokens: Some(6),
            dflash_min_probability: Some(0.7),
            dflash_model_path: Some("/models/local-dflash.gguf".to_owned()),
            kv_type_k: Some(LlamaKvType::Q80),
            kv_type_v: Some(LlamaKvType::Q40),
            gpu_distribution_mode: Some(LlamaGpuDistributionMode::Manual),
            kv_placement: Some(LlamaKvPlacement::SystemRam),
            sampler: LlamaSamplerSettings {
                order: Some(vec![LlamaSamplerStage::TopK, LlamaSamplerStage::Temp]),
                ..LlamaSamplerSettings::default()
            },
            ..LlamaCppSettings::default()
        });
        inference.tools = Some(ToolRequest {
            definitions: vec![ToolDefinition {
                name: "lookup".to_owned(),
                description: Some("Look up".to_owned()),
                parameters: json!({"type": "object"}),
                version: 1,
            }],
            choice: ToolChoice::Named {
                name: "lookup".to_owned(),
            },
        });
        inference.prompt_cache_key = Some("conversation".to_owned());
        let parameters = &mut inference.profile.chat_profile.parameters;
        parameters.reasoning_mode = Some(ReasoningMode::Enabled);
        parameters.reasoning_budget_tokens = Some(2048);
        parameters.send_thinking_state = true;
        let llama = inference.profile.chat_profile.llama_cpp.clone().unwrap();
        let generation =
            generation_request(&inference, &Attachments::new(), &llama, false).unwrap();
        assert_eq!(generation.model_path, "/models/local.gguf");
        assert_eq!(generation.max_tokens, Some(4096 + 2048));
        assert!(generation.reasoning.reasoning_configured);
        assert!(generation.reasoning.parallel_tool_calls);
        assert_eq!(generation.reasoning.enable_thinking, Some(true));
        assert_eq!(
            generation.tool_choice,
            Some(json!({"type": "function", "function": {"name": "lookup"}}))
        );
        assert_eq!(
            generation.tools,
            Some(
                json!([{"type": "function", "function": {"name": "lookup", "description": "Look up", "parameters": {"type": "object"}}}])
            )
        );
        assert_eq!(generation.prompt_cache_key.as_deref(), Some("conversation"));
        assert!(generation.runtime.dflash_enabled);
        assert_eq!(generation.runtime.dflash_draft_tokens, Some(6));
        assert_eq!(generation.runtime.dflash_min_probability, Some(0.7));
        assert_eq!(
            generation.runtime.dflash_model_path.as_deref(),
            Some("/models/local-dflash.gguf")
        );
        assert_eq!(generation.runtime.kv_type_k.as_deref(), Some("q8_0"));
        assert_eq!(generation.runtime.kv_type_v.as_deref(), Some("q4_0"));
        assert_eq!(
            generation.runtime.gpu_distribution_mode.as_deref(),
            Some("manual")
        );
        assert_eq!(
            generation.runtime.kv_placement.as_deref(),
            Some("systemRam")
        );
        assert_eq!(
            generation.sampling.order,
            Some(vec!["top_k".to_owned(), "temp".to_owned()])
        );
        assert_eq!(generation.sampling.top_k, Some(40));

        inference
            .profile
            .chat_profile
            .parameters
            .send_thinking_state = false;
        inference.tools = None;
        let plain = generation_request(&inference, &Attachments::new(), &llama, false).unwrap();
        assert_eq!(plain.reasoning.enable_thinking, None);
        assert_eq!(plain.reasoning.chat_template_kwargs, None);
        assert!(!plain.reasoning.parallel_tool_calls);
    }

    fn output(finish_reason: LlamaFinishReason) -> LlamaGenerationOutput {
        LlamaGenerationOutput {
            message: json!({"role": "assistant", "content": "Paris"}),
            content: "Paris".to_owned(),
            reasoning: Some("thought".to_owned()),
            tool_calls: Vec::new(),
            finish_reason,
            usage: LlamaUsage {
                prompt_tokens: 30,
                cached_prompt_tokens: 12,
                completion_tokens: 3,
                first_token_ms: None,
                tokens_per_second: None,
                mtp_stats: None,
            },
            metrics: None::<LlamaMetricsRecord>,
        }
    }

    #[test]
    fn outcomes_report_cache_use_and_truncation() {
        let inference = llama_request(LlamaCppSettings::default());
        let stop = outcome(&inference, output(LlamaFinishReason::Stop)).unwrap();
        assert_eq!(stop.finish_reason, FinishReason::Stop);
        let usage = stop.usage.unwrap();
        assert_eq!(usage.input_tokens, 30);
        assert_eq!(usage.cached_input_tokens, Some(12));
        assert_eq!(usage.cache_write_tokens, Some(18));
        assert_eq!(usage.output_tokens, 3);
        assert_eq!(
            stop.candidates[0].parts,
            vec![
                MessagePart::ReasoningSummary {
                    text: "thought".to_owned()
                },
                MessagePart::Text {
                    text: "Paris".to_owned()
                },
            ]
        );
        let length = outcome(&inference, output(LlamaFinishReason::Length)).unwrap();
        assert_eq!(length.finish_reason, FinishReason::Length);
        assert_eq!(length.warning_codes, vec![InferenceWarningCode::Truncated]);
        let mut unexpected = output(LlamaFinishReason::ToolCalls);
        unexpected.tool_calls.push(LocalToolCall {
            id: "c".to_owned(),
            name: "lookup".to_owned(),
            arguments: json!({}),
            raw_arguments: None,
        });
        assert!(matches!(
            outcome(&inference, unexpected),
            Err(AdapterError::MalformedResponse)
        ));
    }

    #[test]
    fn legacy_tool_argument_shapes_become_valid_proposals() {
        let mut inference = llama_request(LlamaCppSettings::default());
        inference.tools = Some(ToolRequest {
            definitions: vec![ToolDefinition {
                name: "lookup".to_owned(),
                description: None,
                parameters: json!({"type": "object"}),
                version: 1,
            }],
            choice: ToolChoice::Auto,
        });
        let text = "<tool_call><function=lookup><parameter=q>cats</parameter><parameter=n>2</parameter></function></tool_call>";
        let recovered =
            lettuce_local_llm::tool_calls::recover_message_from_raw_tool_output(text).unwrap();
        let mut calls = lettuce_local_llm::tool_calls::parse_tool_calls(&recovered);
        for (id, arguments, raw) in [
            ("empty", Value::Null, None),
            ("blank", Value::String(String::new()), Some(String::new())),
            (
                "text",
                Value::String("find cats".to_owned()),
                Some("find cats".to_owned()),
            ),
            ("json", json!({"q": "x"}), Some(r#"{"q": "x"}"#.to_owned())),
        ] {
            calls.push(LocalToolCall {
                id: id.to_owned(),
                name: "lookup".to_owned(),
                arguments,
                raw_arguments: raw,
            });
        }
        let mut reply = output(LlamaFinishReason::ToolCalls);
        reply.tool_calls = calls;
        let result = outcome(&inference, reply).unwrap();
        let proposals = &result.candidates[0].tool_calls;
        assert_eq!(proposals.len(), 5);
        assert_eq!(proposals[0].arguments, json!({"q": "cats", "n": 2}));
        assert_eq!(proposals[0].raw_arguments, None);
        for proposal in &proposals[1..4] {
            assert_eq!(proposal.arguments, json!({}));
            assert_eq!(proposal.raw_arguments, None);
        }
        assert_eq!(proposals[4].raw_arguments.as_deref(), Some(r#"{"q": "x"}"#));
    }

    #[derive(Default)]
    struct MemoryHost {
        report: std::sync::Mutex<Option<Value>>,
        metrics: std::sync::Mutex<Vec<LlamaMetricsRecord>>,
    }

    impl RuntimeReportStore for MemoryHost {
        fn load(&self, _model_path: &str) -> Result<Option<Value>, String> {
            Ok(self.report.lock().unwrap().clone())
        }

        fn store(&self, _model_path: &str, report: &Value) -> Result<bool, String> {
            *self.report.lock().unwrap() = Some(report.clone());
            Ok(true)
        }
    }

    impl LlamaHost for MemoryHost {
        fn record_metrics(&self, record: LlamaMetricsRecord) {
            self.metrics.lock().unwrap().push(record);
        }

        fn event(&self, _event: LlamaHostEvent) {}
    }

    #[tokio::test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    async fn streams_a_local_generation_through_the_inference_port() {
        use lettuce_conversations::InferencePort;
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let runtime = Arc::new(lettuce_inference::InferenceRuntime::default());
        let host = Arc::new(MemoryHost::default());
        let providers = crate::RemoteProviders::with_runtime(
            Arc::new(lettuce_settings::InMemorySecretStore::default()),
            Arc::new(lettuce_network::JsonClient::new().unwrap()),
            runtime.clone(),
        )
        .with_local_llama(LocalLlama::new(
            Arc::new(LlamaRuntime::start().unwrap()),
            host.clone(),
        ));
        let mut inference = llama_request(LlamaCppSettings {
            gpu_layers: Some(0),
            strict_mode: Some(true),
            ..LlamaCppSettings::default()
        });
        inference.profile.chat_profile.external_model_id = path;
        inference.profile.chat_profile.parameters.temperature = Some(0.0);
        inference
            .profile
            .chat_profile
            .parameters
            .visible_max_output_tokens = Some(24);
        inference
            .profile
            .chat_profile
            .parameters
            .total_completion_allowance = Some(24);
        inference.context.messages = vec![ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: "Name the capital of France in one word. /no_think".to_owned(),
            }],
        }];
        inference.prompt_cache_key = Some("conversation".to_owned());
        let sink_id = lettuce_types::RequestId::new();
        let mut receiver = runtime.register_stream(sink_id).unwrap();
        inference.stream_sink = Some(sink_id);
        let outcome = providers.run(inference).await.unwrap();
        runtime.unregister_stream(sink_id).unwrap();
        let mut streamed = String::new();
        while let Some(event) = receiver.recv().await {
            if let lettuce_conversations::GenerationStreamEvent::TextDelta { text } = event.event {
                streamed.push_str(&text);
            }
        }
        let text = outcome.candidates[0]
            .parts
            .iter()
            .find_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert!(text.contains("Paris"));
        assert_eq!(streamed.trim(), text);
        assert!(outcome.usage.unwrap().input_tokens > 0);
        assert_eq!(
            host.report.lock().unwrap().as_ref().unwrap()["status"],
            json!("succeeded")
        );
        assert_eq!(host.metrics.lock().unwrap().len(), 1);
    }

    struct MemoryMedia(std::collections::HashMap<lettuce_types::AssetId, ProviderMedia>);

    impl ProviderMediaSource for MemoryMedia {
        fn load(
            &self,
            asset_id: lettuce_types::AssetId,
        ) -> Result<ProviderMedia, crate::ProviderMediaError> {
            self.0
                .get(&asset_id)
                .cloned()
                .ok_or(crate::ProviderMediaError::Unavailable)
        }
    }

    fn red_png() -> Vec<u8> {
        let image = image::RgbImage::from_pixel(64, 64, image::Rgb([230, 20, 20]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    fn with_attachments(
        capabilities_image: bool,
    ) -> (InferenceRequest, MemoryMedia, lettuce_types::AssetId) {
        let mut inference = llama_request(LlamaCppSettings::default());
        if capabilities_image {
            inference
                .profile
                .chat_profile
                .capabilities
                .input_modalities
                .image = lettuce_models::CapabilityStatus::Supported;
        }
        let image_id = lettuce_types::AssetId::new();
        let audio_id = lettuce_types::AssetId::new();
        inference.context.messages = vec![
            ProviderNeutralMessage {
                role: MessageRole::Assistant,
                parts: vec![
                    ProviderContextPart::Text {
                        text: "a portrait".to_owned(),
                    },
                    ProviderContextPart::MediaAsset {
                        asset_id: image_id,
                        role: lettuce_conversations::MediaAssetRole::Inline,
                    },
                ],
            },
            ProviderNeutralMessage {
                role: MessageRole::User,
                parts: vec![
                    ProviderContextPart::MediaAsset {
                        asset_id: image_id,
                        role: lettuce_conversations::MediaAssetRole::Attachment,
                    },
                    ProviderContextPart::Text {
                        text: "What color is this?".to_owned(),
                    },
                    ProviderContextPart::MediaAsset {
                        asset_id: audio_id,
                        role: lettuce_conversations::MediaAssetRole::Attachment,
                    },
                ],
            },
        ];
        inference.media_grants = vec![image_id, audio_id];
        let media = MemoryMedia(
            [
                (
                    image_id,
                    ProviderMedia {
                        mime_type: "image/png".to_owned(),
                        bytes: vec![1, 2, 3],
                    },
                ),
                (
                    audio_id,
                    ProviderMedia {
                        mime_type: "audio/mpeg".to_owned(),
                        bytes: vec![4, 5],
                    },
                ),
            ]
            .into_iter()
            .collect(),
        );
        (inference, media, image_id)
    }

    #[test]
    fn user_attachments_follow_the_model_input_capabilities_like_legacy() {
        let (inference, media, _) = with_attachments(true);
        let messages = wire_messages(&inference, &media.0).unwrap();
        assert_eq!(
            messages[0],
            json!({"role": "assistant", "content": "a portrait"})
        );
        assert_eq!(
            messages[1],
            json!({"role": "user", "content": [
                {"type": "text", "text": "What color is this?"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,AQID", "detail": "auto"}},
            ]})
        );
        let (inference, media, _) = with_attachments(false);
        let messages = wire_messages(&inference, &media.0).unwrap();
        assert_eq!(
            messages[1],
            json!({"role": "user", "content": "What color is this?"})
        );
        assert_eq!(
            openai_content_parts(
                "",
                &[ProviderMedia {
                    mime_type: "audio/x-wav".to_owned(),
                    bytes: vec![9],
                }],
                (false, true),
            ),
            json!([{"type": "input_audio", "input_audio": {"data": "CQ==", "format": "wav"}}])
        );
        assert_eq!(
            openai_content_parts("", &[], (true, false)),
            json!([{"type": "text", "text": " "}])
        );
    }

    #[tokio::test]
    async fn attachments_must_be_granted_and_unreadable_ones_are_skipped() {
        let (mut inference, media, image_id) = with_attachments(true);
        let source: Arc<dyn ProviderMediaSource> = Arc::new(MemoryMedia(
            media
                .0
                .into_iter()
                .filter(|(id, _)| *id == image_id)
                .collect(),
        ));
        let loaded = load_attachments(&inference, Some(source.clone()))
            .await
            .unwrap();
        assert_eq!(loaded.len(), 1);
        let messages = wire_messages(&inference, &loaded).unwrap();
        assert_eq!(messages[1]["content"].as_array().map(Vec::len), Some(2));
        let none = wire_messages(&inference, &Attachments::new()).unwrap();
        assert_eq!(
            none[1]["content"],
            json!([{"type": "text", "text": "What color is this?"}])
        );
        inference.media_grants.retain(|id| *id != image_id);
        assert!(matches!(
            load_attachments(&inference, Some(source)).await,
            Err(AdapterError::Rejected)
        ));
    }

    #[tokio::test]
    #[ignore = "needs a vision GGUF in LETTUCE_VISION_MODEL and its projector in LETTUCE_VISION_MMPROJ"]
    async fn describes_an_attached_image_through_the_inference_port() {
        use lettuce_conversations::InferencePort;
        let (Ok(model), Ok(mmproj)) = (
            std::env::var("LETTUCE_VISION_MODEL"),
            std::env::var("LETTUCE_VISION_MMPROJ"),
        ) else {
            return;
        };
        let runtime = Arc::new(lettuce_inference::InferenceRuntime::default());
        let (mut inference, _, image_id) = with_attachments(true);
        let media = Arc::new(MemoryMedia(
            [(
                image_id,
                ProviderMedia {
                    mime_type: "image/png".to_owned(),
                    bytes: red_png(),
                },
            )]
            .into_iter()
            .collect(),
        ));
        let providers = crate::RemoteProviders::with_runtime(
            Arc::new(lettuce_settings::InMemorySecretStore::default()),
            Arc::new(lettuce_network::JsonClient::new().unwrap()),
            runtime,
        )
        .with_local_llama(LocalLlama::new(
            Arc::new(LlamaRuntime::start().unwrap()),
            Arc::new(MemoryHost::default()),
        ))
        .with_media_source(media);
        inference.profile.chat_profile.external_model_id = model;
        inference.profile.chat_profile.llama_cpp = Some(Box::new(ResolvedLlamaSettings {
            settings: LlamaCppSettings {
                gpu_layers: Some(0),
                mmproj_path: Some(mmproj),
                ..LlamaCppSettings::default()
            },
            disable_sampler_profile_defaults: false,
        }));
        inference.profile.chat_profile.parameters.temperature = Some(0.0);
        inference
            .profile
            .chat_profile
            .parameters
            .visible_max_output_tokens = Some(16);
        inference
            .profile
            .chat_profile
            .parameters
            .total_completion_allowance = Some(16);
        inference.context.messages = vec![ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![
                ProviderContextPart::Text {
                    text: "What color is this image? Answer with one word.".to_owned(),
                },
                ProviderContextPart::MediaAsset {
                    asset_id: image_id,
                    role: lettuce_conversations::MediaAssetRole::Attachment,
                },
            ],
        }];
        inference.media_grants = vec![image_id];
        let outcome = providers.run(inference).await.unwrap();
        let text = outcome.candidates[0]
            .parts
            .iter()
            .find_map(|part| match part {
                MessagePart::Text { text } => Some(text.to_lowercase()),
                _ => None,
            })
            .unwrap();
        eprintln!("vision answer: {text}");
        assert!(text.contains("red"));
    }
}
