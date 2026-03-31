import {
  PROVIDER_PARAMETER_SUPPORT,
  type AdvancedModelSettings,
  getProviderReasoningCapability,
} from "../../core/storage/schemas";
import { useI18n } from "../../core/i18n/context";

interface ProviderParameterSupportInfoProps {
  providerId: string;
  compact?: boolean;
}

const PARAMETER_LABELS: Record<keyof AdvancedModelSettings, string> = {
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
  sdSeed: "SD Seed",
  sdNegativePrompt: "SD Negative Prompt",
  sdDenoisingStrength: "SD Denoise",
  sdSize: "SD Default Size",
  llamaGpuLayers: "llama.cpp GPU Layers",
  llamaThreads: "llama.cpp Threads",
  llamaThreadsBatch: "llama.cpp Batch Threads",
  llamaSeed: "llama.cpp Seed",
  llamaRopeFreqBase: "llama.cpp RoPE Base",
  llamaRopeFreqScale: "llama.cpp RoPE Scale",
  llamaOffloadKqv: "llama.cpp Offload KQV",
  llamaBatchSize: "llama.cpp Batch Size",
  llamaKvType: "llama.cpp KV Cache Type",
  llamaFlashAttention: "llama.cpp Flash Attention",
  llamaChatTemplateOverride: "llama.cpp Template Override",
  llamaMmprojPath: "llama.cpp MMProj Path",
  llamaChatTemplatePreset: "llama.cpp Template Preset",
  llamaRawCompletionFallback: "llama.cpp Raw Fallback",
  llamaSamplerProfile: "llama.cpp Sampler Profile",
  llamaMinP: "llama.cpp Min P",
  llamaTypicalP: "llama.cpp Typical P",
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
  promptCachingEnabled: "Prompt Caching",
  promptCachingTtl: "Prompt Caching TTL",
};

const PARAMETER_DESCRIPTIONS: Record<keyof AdvancedModelSettings, string> = {
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
  sdSeed: "Fixed seed for reproducible images",
  sdNegativePrompt: "Negative prompt appended to every request",
  sdDenoisingStrength: "Img2img denoise strength (0-1)",
  sdSize: "Default WIDTHxHEIGHT when the request does not override size",
  llamaGpuLayers: "Number of layers to offload to GPU (0 = CPU)",
  llamaThreads: "CPU threads for generation",
  llamaThreadsBatch: "CPU threads for batch processing",
  llamaSeed: "Random seed (leave blank for random)",
  llamaRopeFreqBase: "RoPE base frequency override",
  llamaRopeFreqScale: "RoPE frequency scale override",
  llamaOffloadKqv: "Offload KQV ops/KV cache to GPU",
  llamaBatchSize: "Prompt processing chunk size (smaller = safer)",
  llamaKvType: "KV cache quantization type",
  llamaFlashAttention: "Flash attention mode (auto/enabled/disabled)",
  llamaChatTemplateOverride: "Exact template string or llama.cpp template name",
  llamaMmprojPath: "Filesystem path to the multimodal projector GGUF used for vision",
  llamaChatTemplatePreset: "Known preset name used when GGUF has no embedded template",
  llamaRawCompletionFallback: "Allow plain raw completion fallback if template resolution fails",
  llamaSamplerProfile:
    "Preset local sampler defaults for chat, creativity, stability, or reasoning",
  llamaMinP: "Min-p sampling threshold for llama.cpp (0-1)",
  llamaTypicalP: "Typical sampling threshold for llama.cpp (0-1)",
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
  promptCachingEnabled: "Enable prompt caching",
  promptCachingTtl: "How long a cached prompt prefix stays reusable",
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
  const provider =
    PROVIDER_PARAMETER_SUPPORT[providerId as keyof typeof PROVIDER_PARAMETER_SUPPORT];

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
            {PARAMETER_LABELS[param]}
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
                <div className="text-sm font-medium text-white">{PARAMETER_LABELS[param]}</div>
                <div className="text-xs text-white/50 truncate">
                  {param === "reasoningEffort" && reasoningCapability.type === "none"
                    ? t("components.providerParameterSupport.reasoningNotSupportedEffort")
                    : param === "reasoningEffort" && reasoningCapability.type === "budget-only"
                      ? t("components.providerParameterSupport.reasoningNotSupportedBudgetOnly")
                      : param === "reasoningBudgetTokens" && reasoningCapability.type === "none"
                        ? t("components.providerParameterSupport.reasoningNotSupported")
                        : PARAMETER_DESCRIPTIONS[param]}
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
                  {PARAMETER_LABELS[param]}
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
