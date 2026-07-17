import {
  PROVIDER_PARAMETER_SUPPORT,
  type AdvancedModelSettings,
  getProviderReasoningCapability,
  resolveParameterSupport,
} from "../../core/storage/schemas";
import { useI18n } from "../../core/i18n/context";

interface ProviderParameterSupportInfoProps {
  providerId: string;
  compact?: boolean;
}

type StaticParameter = Exclude<keyof AdvancedModelSettings, "llamaNPenRange" | `sdcpp${string}`>;

const PARAMETER_LABELS: Record<StaticParameter, string> = {
  temperature: "Temperature",
  topP: "Top P",
  maxOutputTokens: "Max Tokens",
  contextLength: "Context Length",
  frequencyPenalty: "Frequency Penalty",
  presencePenalty: "Presence Penalty",
  topK: "Top K",
  sdSteps: "SD Steps",
  sdCfgScale: "SD CFG Scale",
  sdSampler: "SD Sampler",
  sdScheduler: "SD Scheduler",
  sdSeed: "SD Seed",
  sdNegativePrompt: "SD Negative Prompt",
  sdDenoisingStrength: "SD Denoise",
  sdImageCfgScale: "SD Image CFG",
  sdDistilledGuidance: "SD Distilled Guidance",
  sdSlgScale: "SD Skip-layer Guidance Scale",
  sdSlgLayers: "SD Skip-layer Guidance Layers",
  sdSlgLayerStart: "SD Skip-layer Guidance Start",
  sdSlgLayerEnd: "SD Skip-layer Guidance End",
  sdCacheMode: "SD Step Caching",
  sdCacheOption: "SD Step Caching Options",
  sdEta: "SD Eta",
  sdFlowShift: "SD Flow Shift",
  sdSize: "SD Default Size",
  sdVaeTilingEnabled: "SD VAE Tiling",
  sdVaeTileSizeX: "SD VAE Tile Width",
  sdVaeTileSizeY: "SD VAE Tile Height",
  sdVaeTileOverlap: "SD VAE Tile Overlap",
  sdAutoResizeRefImages: "SD Auto-resize References",
  sdIncreaseRefIndex: "SD Reference Indexing",
  sdHiresEnabled: "SD High-resolution Pass",
  sdHiresUpscaler: "SD High-resolution Upscaler",
  sdHiresScale: "SD High-resolution Scale",
  sdHiresWidth: "SD High-resolution Width",
  sdHiresHeight: "SD High-resolution Height",
  sdHiresSteps: "SD High-resolution Steps",
  sdHiresDenoisingStrength: "SD High-resolution Denoise",
  sdOffloadMode: "Memory Offload",
  sdExtraPrompt: "Extra Prompt",
  sdPromptWriterInstructions: "Prompt Writer Instructions",
  sdBaseLoras: "Base LoRAs",
  llamaGpuLayers: "llama.cpp GPU Layers",
  llamaMultiGpuEnabled: "llama.cpp Multi-GPU",
  llamaGpuDeviceIds: "llama.cpp GPU Devices",
  llamaGpuDistributionMode: "llama.cpp GPU Distribution",
  llamaGpuManualLayers: "llama.cpp Manual GPU Layers",
  llamaCpuLayers: "llama.cpp CPU Layers",
  llamaKvPlacement: "llama.cpp KV Cache Placement",
  llamaMainGpu: "llama.cpp Main GPU",
  llamaSingleGpuDeviceId: "llama.cpp GPU Device",
  llamaPriorityVramLimitBytes: "llama.cpp Priority VRAM Limit",
  llamaThreads: "llama.cpp Threads",
  llamaThreadsBatch: "llama.cpp Batch Threads",
  llamaSeed: "llama.cpp Seed",
  llamaRopeFreqBase: "llama.cpp RoPE Base",
  llamaRopeFreqScale: "llama.cpp RoPE Scale",
  llamaOffloadKqv: "llama.cpp Offload KQV",
  llamaBatchSize: "llama.cpp Batch Size",
  llamaUbatchSize: "llama.cpp Microbatch Size",
  llamaKvType: "llama.cpp KV Cache Type",
  llamaFlashAttention: "llama.cpp Flash Attention",
  llamaSwaFull: "llama.cpp Full SWA Cache",
  llamaChatTemplateOverride: "llama.cpp Template Override",
  llamaMmprojPath: "llama.cpp MMProj Path",
  llamaChatTemplatePreset: "llama.cpp Template Preset",
  llamaRawCompletionFallback: "llama.cpp Raw Fallback",
  llamaStrictMode: "llama.cpp Strict Mode",
  llamaMtpEnabled: "llama.cpp Multi-Token Prediction",
  llamaMtpPlacement: "llama.cpp MTP Placement",
  llamaMtpDraftTokens: "llama.cpp MTP Draft Tokens",
  llamaMtpModelPath: "llama.cpp MTP Model Path",
  llamaStreamingEnabled: "llama.cpp Streaming",
  llamaSamplerProfile: "llama.cpp Sampler Profile",
  llamaSamplerOrder: "llama.cpp Sampler Order",
  llamaRepeatPenalty: "llama.cpp Repeat Penalty",
  llamaMinP: "llama.cpp Min P",
  llamaTypicalP: "llama.cpp Typical P",
  llamaDryMultiplier: "llama.cpp DRY Multiplier",
  llamaDryBase: "llama.cpp DRY Base",
  llamaDryAllowedLength: "llama.cpp DRY Allowed Length",
  llamaDryPenaltyLastN: "llama.cpp DRY Penalty Last N",
  llamaDrySequenceBreakers: "llama.cpp DRY Sequence Breakers",
  llamaXtcProbability: "llama.cpp XTC Probability",
  llamaXtcThreshold: "llama.cpp XTC Threshold",
  llamaLastRuntimeReport: "llama.cpp Runtime Report",
  ollamaNumCtx: "Ollama Num Ctx",
  ollamaNumPredict: "Ollama Num Predict",
  ollamaNumKeep: "Ollama Num Keep",
  ollamaNumBatch: "Ollama Num Batch",
  ollamaNumGpu: "Ollama Num GPU",
  ollamaNumThread: "Ollama Num Thread",
  ollamaTfsZ: "Ollama TFS Z",
  ollamaTypicalP: "Ollama Typical P",
  ollamaMinP: "Ollama Min P",
  ollamaMirostat: "Ollama Mirostat",
  ollamaMirostatTau: "Ollama Mirostat Tau",
  ollamaMirostatEta: "Ollama Mirostat Eta",
  ollamaRepeatPenalty: "Ollama Repeat Penalty",
  ollamaSeed: "Ollama Seed",
  ollamaStop: "Ollama Stop Sequences",
  reasoningEnabled: "Reasoning",
  reasoningEffort: "Reasoning Effort",
  reasoningBudgetTokens: "Reasoning Budget",
  forceSendThinkingState: "Force Send Thinking State",
  promptCachingEnabled: "Prompt Caching",
  promptCachingTtl: "Prompt Caching TTL",
  openRouterProvider: "Pinned OpenRouter Provider",
  featureGenerationSettings: "Feature Generation Overrides",
};

const PARAMETER_DESCRIPTIONS: Record<StaticParameter, string> = {
  temperature: "Controls randomness (0-2)",
  topP: "Nucleus sampling threshold (0-1)",
  maxOutputTokens: "Maximum response length",
  contextLength: "Override context window (local only)",
  frequencyPenalty: "Reduce token repetition (-2 to 2)",
  presencePenalty: "Encourage new topics (-2 to 2)",
  topK: "Limit token pool size (1-500)",
  sdSteps: "Diffusion sampling steps",
  sdCfgScale: "Prompt guidance scale",
  sdSampler: "Sampler name sent to the diffusion backend",
  sdScheduler: "Noise schedule used across diffusion sampling steps",
  sdSeed: "Fixed seed for reproducible images",
  sdNegativePrompt: "Negative prompt appended to every request",
  sdDenoisingStrength: "Img2img denoise strength (0-1)",
  sdImageCfgScale: "Reference-image guidance scale",
  sdDistilledGuidance: "Guidance value used by distilled diffusion architectures",
  sdSlgScale: "Skip-layer guidance strength (0 disables it)",
  sdSlgLayers: "Comma-separated transformer layers skipped for guidance",
  sdSlgLayerStart: "Sampling fraction where skip-layer guidance starts",
  sdSlgLayerEnd: "Sampling fraction where skip-layer guidance stops",
  sdCacheMode: "Diffusion step caching to speed up generation",
  sdCacheOption: "Comma-separated key=value tuning for step caching",
  sdEta: "Stochastic sampler noise",
  sdFlowShift: "Timestep shift used by flow-based image models",
  sdSize: "Default WIDTHxHEIGHT when the request does not override size",
  sdVaeTilingEnabled: "Decode the image in VAE tiles to reduce peak memory",
  sdVaeTileSizeX: "VAE decode tile width",
  sdVaeTileSizeY: "VAE decode tile height",
  sdVaeTileOverlap: "Overlap between adjacent VAE tiles",
  sdAutoResizeRefImages: "Resize reference images to match generation dimensions",
  sdIncreaseRefIndex: "Assign successive positional indices to reference images",
  sdHiresEnabled: "Run an upscale and refinement pass after initial generation",
  sdHiresUpscaler: "Upscaler used before the refinement pass",
  sdHiresScale: "Multiplier used for high-resolution upscaling",
  sdHiresWidth: "Explicit high-resolution target width",
  sdHiresHeight: "Explicit high-resolution target height",
  sdHiresSteps: "Sampling steps used by the high-resolution pass",
  sdHiresDenoisingStrength: "Denoising strength used by the high-resolution pass",
  sdOffloadMode: "Where model weights live during generation",
  sdExtraPrompt: "Always prepended to the prompt (quality tags, style boilerplate)",
  sdPromptWriterInstructions: "Format guidance given to the scene writer for this model",
  sdBaseLoras: "LoRAs applied to every image generation made with this model",
  llamaGpuLayers: "Number of layers to offload to GPU (0 = CPU)",
  llamaMultiGpuEnabled: "Allow llama.cpp to split model loading across selected GPUs",
  llamaGpuDeviceIds: "GPU backend device indices used when multi-GPU is enabled",
  llamaGpuDistributionMode: "How layers are spread across GPUs (balanced, by VRAM, priority fill, manual)",
  llamaGpuManualLayers: "Per-GPU layer counts used in manual distribution mode",
  llamaCpuLayers: "Layers kept on CPU in manual multi-GPU distribution",
  llamaKvPlacement: "Where the KV cache lives across GPUs and system RAM",
  llamaMainGpu: "Primary GPU for KV pinning and priority fill",
  llamaSingleGpuDeviceId: "Run on one specific GPU instead of the default device",
  llamaPriorityVramLimitBytes: "VRAM cap on the primary GPU before overflowing to the next (priority fill)",
  llamaThreads: "CPU threads for generation",
  llamaThreadsBatch: "CPU threads for batch processing",
  llamaSeed: "Random seed (leave blank for random)",
  llamaRopeFreqBase: "RoPE base frequency override",
  llamaRopeFreqScale: "RoPE frequency scale override",
  llamaOffloadKqv: "Offload KQV ops/KV cache to GPU",
  llamaBatchSize: "Prompt processing chunk size (smaller = safer)",
  llamaUbatchSize: "Physical prompt microbatch size used by llama.cpp",
  llamaKvType: "KV cache quantization type",
  llamaFlashAttention: "Flash attention mode (auto/enabled/disabled)",
  llamaSwaFull: "Keep the full sliding-window KV cache (better long-context recall on SWA models, higher VRAM)",
  llamaChatTemplateOverride: "Exact template string or llama.cpp template name",
  llamaMmprojPath: "Filesystem path to the multimodal projector GGUF used for vision",
  llamaChatTemplatePreset: "Known preset name used when GGUF has no embedded template",
  llamaRawCompletionFallback: "Allow plain raw completion fallback if template resolution fails",
  llamaStrictMode:
    "Disable llama.cpp safety fallbacks that lower offload, context, or batch settings",
  llamaMtpEnabled:
    "Speculative decoding via the model's bundled MTP layers (needs an MTP-capable GGUF)",
  llamaMtpPlacement: "Automatic, GPU, or CPU placement for the external MTP draft model",
  llamaMtpDraftTokens: "Speculative tokens drafted per MTP step (1 to 8)",
  llamaMtpModelPath: "External MTP draft GGUF path (auto-discovered from sibling mtp-*.gguf when empty)",
  llamaStreamingEnabled: "Disable incremental token streaming for llama.cpp models",
  llamaSamplerProfile:
    "Preset local sampler defaults for chat, creativity, stability, or reasoning",
  llamaSamplerOrder: "Array of sampler stages executed in the saved order for local llama.cpp",
  llamaRepeatPenalty: "Multiplicatively penalize tokens already present in the local context",
  llamaMinP: "Min-p sampling threshold for llama.cpp (0-1)",
  llamaTypicalP: "Typical sampling threshold for llama.cpp (0-1)",
  llamaDryMultiplier: "Enable DRY sequence repetition penalties when greater than 0",
  llamaDryBase: "DRY exponential penalty base",
  llamaDryAllowedLength: "Ignore repeated sequences shorter than this length",
  llamaDryPenaltyLastN: "How far back DRY scans for matching sequences (-1 = full context)",
  llamaDrySequenceBreakers: "Sequence boundaries that reset DRY matching",
  llamaXtcProbability: "Chance of excluding top tokens for more variety (0 = off)",
  llamaXtcThreshold: "Minimum probability for a token to be eligible for XTC removal",
  llamaLastRuntimeReport: "Persisted diagnostics from the last local llama.cpp run",
  ollamaNumCtx: "Ollama context window size",
  ollamaNumPredict: "Max tokens to generate",
  ollamaNumKeep: "Tokens to keep from prompt",
  ollamaNumBatch: "Batch size for prompt processing",
  ollamaNumGpu: "GPU layers offload",
  ollamaNumThread: "CPU threads for inference",
  ollamaTfsZ: "Tail free sampling (0-1)",
  ollamaTypicalP: "Typical sampling (0-1)",
  ollamaMinP: "Min-p sampling (0-1)",
  ollamaMirostat: "Mirostat sampler (0/1/2)",
  ollamaMirostatTau: "Mirostat target entropy",
  ollamaMirostatEta: "Mirostat learning rate",
  ollamaRepeatPenalty: "Repeat penalty (0-2)",
  ollamaSeed: "Random seed",
  ollamaStop: "Stop sequences list",
  reasoningEnabled: "Enable/disable thinking mode",
  reasoningEffort: "Thinking depth - OpenAI/DeepSeek style",
  reasoningBudgetTokens: "Max tokens for extended thinking",
  forceSendThinkingState: "Explicitly send enable_thinking instead of omitting it",
  promptCachingEnabled: "Enable prompt caching",
  promptCachingTtl: "How long a cached prompt prefix stays reusable",
  openRouterProvider: "Route this model through one OpenRouter endpoint",
  featureGenerationSettings: "Per-feature sampling overrides for app features using this model",
};

/**
 * Display which parameters are supported by a specific provider
 * Optimized for bottom menu display
 */
export function ProviderParameterSupportInfo({
  providerId,
  compact = false,
}: ProviderParameterSupportInfoProps) {
  const { t } = useI18n();
  const provider = resolveParameterSupport(providerId);
  const parameterLabel = (parameter: keyof AdvancedModelSettings) =>
    parameter === "llamaNPenRange"
      ? `llama.cpp ${t("editModel.llamaSampler.penaltyRange")}`
      : PARAMETER_LABELS[parameter as StaticParameter];
  const parameterDescription = (parameter: keyof AdvancedModelSettings) =>
    parameter === "llamaNPenRange"
      ? t("editModel.llamaSampler.penaltyRangeDescription")
      : PARAMETER_DESCRIPTIONS[parameter as StaticParameter];

  if (!provider) {
    return (
      <div className="rounded-lg border border-white/10 bg-white/5 p-3">
        <p className="text-xs text-white/50">
          {t("components.providerParameterSupport.unknownProvider", { providerId })}
        </p>
      </div>
    );
  }

  const supportedParameters = provider.supportedParameters as Partial<
    Record<keyof AdvancedModelSettings, boolean>
  >;
  const parameters = Object.keys(supportedParameters) as (keyof AdvancedModelSettings)[];
  const reasoningCapability = getProviderReasoningCapability(providerId);

  if (compact) {
    const supported = parameters.filter((param) => supportedParameters[param]);
    return (
      <div className="flex flex-wrap gap-1">
        {supported.map((param) => (
          <span
            key={param}
            className="rounded-md border border-emerald-400/30 bg-emerald-400/10 px-2 py-0.5 text-[10px] font-medium text-emerald-200"
          >
            {parameterLabel(param)}
          </span>
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 gap-2">
        {parameters.map((param) => {
          const isSupported = supportedParameters[param];
          return (
            <div
              key={param}
              className={`flex items-center gap-3 rounded-lg border p-3 transition ${
                isSupported
                  ? "border-emerald-400/20 bg-emerald-400/5"
                  : "border-white/10 bg-white/5 opacity-50"
              }`}
            >
              <div className="shrink-0">
                {isSupported ? (
                  <svg
                    className="h-5 w-5 text-emerald-400"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                ) : (
                  <svg
                    className="h-5 w-5 text-white/30"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium text-white">{parameterLabel(param)}</div>
                <div className="text-xs text-white/50 truncate">
                  {param === "reasoningEffort" && reasoningCapability.type === "none"
                    ? t("components.providerParameterSupport.reasoningNotSupportedEffort")
                    : param === "reasoningEffort" && reasoningCapability.type === "budget-only"
                      ? t("components.providerParameterSupport.reasoningNotSupportedBudgetOnly")
                      : param === "reasoningBudgetTokens" && reasoningCapability.type === "none"
                        ? t("components.providerParameterSupport.reasoningNotSupported")
                        : parameterDescription(param)}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <div className="rounded-lg border border-blue-400/20 bg-blue-400/5 p-3">
        <div className="flex gap-2">
          <svg
            className="h-4 w-4 shrink-0 text-blue-400 mt-0.5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          <div className="text-xs text-blue-200/80 leading-relaxed space-y-1">
            <p>
              {t("components.providerParameterSupport.unsupportedParametersIgnored", {
                providerName: provider.displayName,
              })}
            </p>
            {reasoningCapability.type === "effort" && (
              <p className="font-medium text-amber-200">
                ⚡ {t("components.providerParameterSupport.reasoningEffortSupported")}
              </p>
            )}
            {reasoningCapability.type === "budget-only" && (
              <p className="font-medium text-amber-200">
                💭 {t("components.providerParameterSupport.reasoningBudgetSupported")}
              </p>
            )}
            {reasoningCapability.type === "none" && (
              <p className="text-white/50">
                {t("components.providerParameterSupport.reasoningNotSupportedProvider")}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

/**
 * Show a summary of all providers and their parameter support
 */
export function AllProvidersParameterSupport() {
  const { t } = useI18n();
  const parameterLabel = (parameter: keyof AdvancedModelSettings) =>
    parameter === "llamaNPenRange"
      ? `llama.cpp ${t("editModel.llamaSampler.penaltyRange")}`
      : PARAMETER_LABELS[parameter as StaticParameter];
  const allProviders = Object.values(PROVIDER_PARAMETER_SUPPORT);
  const allParams = Object.keys(
    PROVIDER_PARAMETER_SUPPORT.openai.supportedParameters,
  ) as (keyof AdvancedModelSettings)[];

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-semibold text-white">
        {t("components.providerParameterSupport.matrixTitle")}
      </h3>

      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-white/10">
              <th className="pb-2 pr-4 text-left font-medium text-white/60">
                {t("components.providerParameterSupport.providerColumn")}
              </th>
              {allParams.map((param) => (
                <th key={param} className="pb-2 px-2 text-center font-medium text-white/60">
                  {parameterLabel(param)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {allProviders.map((provider) => (
              <tr key={provider.providerId} className="border-b border-white/5">
                <td className="py-2.5 pr-4 font-medium text-white">{provider.displayName}</td>
                {allParams.map((param) => {
                  const isSupported = (
                    provider.supportedParameters as Partial<
                      Record<keyof AdvancedModelSettings, boolean>
                    >
                  )[param];
                  return (
                    <td key={param} className="py-2.5 px-2 text-center">
                      {isSupported ? (
                        <svg
                          className="mx-auto h-4 w-4 text-emerald-400"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M5 13l4 4L19 7"
                          />
                        </svg>
                      ) : (
                        <svg
                          className="mx-auto h-4 w-4 text-white/20"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M6 18L18 6M6 6l12 12"
                          />
                        </svg>
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="text-[10px] text-white/40 space-y-1">
        <p>✓ = {t("components.providerParameterSupport.supportedIndicator")}</p>
        <p>✗ = {t("components.providerParameterSupport.notSupportedIndicator")}</p>
      </div>
    </div>
  );
}
