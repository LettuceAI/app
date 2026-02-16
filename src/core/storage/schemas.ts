import { z } from "zod";

const TokenCount = z.number().int().nonnegative();
const OptionalTokenCount = z.preprocess((v) => (v === null ? undefined : v), TokenCount.optional());

export const PromptScopeSchema = z.enum(["appWide", "modelSpecific", "characterSpecific"]);
export type PromptScope = z.infer<typeof PromptScopeSchema>;

export const PromptEntryRoleSchema = z.enum(["system", "user", "assistant"]);
export type PromptEntryRole = z.infer<typeof PromptEntryRoleSchema>;

export const PromptEntryPositionSchema = z.enum(["relative", "inChat", "conditional", "interval"]);
export type PromptEntryPosition = z.infer<typeof PromptEntryPositionSchema>;

export const SystemPromptEntrySchema = z.object({
  id: z.string(),
  name: z.string().min(1),
  role: PromptEntryRoleSchema,
  content: z.string(),
  enabled: z.boolean().default(true),
  injectionPosition: PromptEntryPositionSchema.default("relative"),
  injectionDepth: z.number().int().min(0).default(0),
  conditionalMinMessages: z.number().int().min(1).nullable().optional(),
  intervalTurns: z.number().int().min(1).nullable().optional(),
  systemPrompt: z.boolean().default(false),
});
export type SystemPromptEntry = z.infer<typeof SystemPromptEntrySchema>;

export const SystemPromptTemplateSchema = z.object({
  id: z.string(),
  name: z.string().min(1),
  scope: PromptScopeSchema,
  targetIds: z.array(z.string()).default([]),
  content: z.string(),
  entries: z.array(SystemPromptEntrySchema).default([]),
  condensePromptEntries: z.boolean().default(false),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type SystemPromptTemplate = z.infer<typeof SystemPromptTemplateSchema>;

export const UsageSummarySchema = z.object({
  promptTokens: OptionalTokenCount,
  completionTokens: OptionalTokenCount,
  totalTokens: OptionalTokenCount,
  reasoningTokens: OptionalTokenCount,
  imageTokens: OptionalTokenCount,
});
export type UsageSummary = z.infer<typeof UsageSummarySchema>;

export const AdvancedModelSettingsSchema = z.object({
  temperature: z.number().min(0).max(2).nullable().optional(),
  topP: z.number().min(0).max(1).nullable().optional(),
  maxOutputTokens: z.number().int().min(1).nullable().optional(),
  contextLength: z.number().int().min(0).nullable().optional(),
  frequencyPenalty: z.number().min(-2).max(2).nullable().optional(),
  presencePenalty: z.number().min(-2).max(2).nullable().optional(),
  topK: z.number().int().min(1).max(500).nullable().optional(),
  // llama.cpp specific settings
  llamaGpuLayers: z.number().int().min(0).max(512).nullable().optional(),
  llamaThreads: z.number().int().min(1).max(256).nullable().optional(),
  llamaThreadsBatch: z.number().int().min(1).max(256).nullable().optional(),
  llamaSeed: z.number().int().min(0).max(2_147_483_647).nullable().optional(),
  llamaRopeFreqBase: z.number().min(0).max(1_000_000).nullable().optional(),
  llamaRopeFreqScale: z.number().min(0).max(10).nullable().optional(),
  llamaOffloadKqv: z.boolean().nullable().optional(),
  // Ollama specific settings
  ollamaNumCtx: z.number().int().min(0).max(262_144).nullable().optional(),
  ollamaNumPredict: z.number().int().min(0).max(131_072).nullable().optional(),
  ollamaNumKeep: z.number().int().min(0).max(32_768).nullable().optional(),
  ollamaNumBatch: z.number().int().min(1).max(16_384).nullable().optional(),
  ollamaNumGpu: z.number().int().min(0).max(512).nullable().optional(),
  ollamaNumThread: z.number().int().min(1).max(256).nullable().optional(),
  ollamaTfsZ: z.number().min(0).max(1).nullable().optional(),
  ollamaTypicalP: z.number().min(0).max(1).nullable().optional(),
  ollamaMinP: z.number().min(0).max(1).nullable().optional(),
  ollamaMirostat: z.number().int().min(0).max(2).nullable().optional(),
  ollamaMirostatTau: z.number().min(0).max(10).nullable().optional(),
  ollamaMirostatEta: z.number().min(0).max(1).nullable().optional(),
  ollamaRepeatPenalty: z.number().min(0).max(2).nullable().optional(),
  ollamaSeed: z.number().int().min(0).max(2_147_483_647).nullable().optional(),
  ollamaStop: z.array(z.string().min(1)).nullable().optional(),
  // Reasoning/thinking settings
  reasoningEnabled: z.boolean().nullable().optional(),
  reasoningEffort: z.enum(["low", "medium", "high"]).nullable().optional(),
  reasoningBudgetTokens: z.number().int().min(1024).nullable().optional(),
});

export type AdvancedModelSettings = z.infer<typeof AdvancedModelSettingsSchema>;

/**
 * Reasoning capability metadata for providers
 *
 * This system handles provider-specific reasoning/thinking capabilities:
 *
 * - **'effort'**: Providers that use `reasoning_effort` parameter with values like
 *   "low", "medium", "high". Includes:
 *   - OpenAI (o1 series): none/minimal/low/medium/high/xhigh
 *   - Groq: none/default/low/medium/high (model-specific)
 *   - Google Gemini 3: minimal/low/medium/high (thinkingLevel)
 *   - OpenAI-compatible: Chutes, OpenRouter, NanoGPT, xAI, Anannas, ZAI
 *   - DeepSeek R1: Outputs reasoning automatically (no effort control)
 *
 * - **'budget-only'**: Uses thinking/reasoning with budget tokens only, no effort levels.
 *   Includes:
 *   - Anthropic Claude: uses `thinking.budget` parameter (min 1024 tokens)
 *   - Google Gemini 2.5: uses thinkingBudget
 *   - Mistral Magistral: uses thinking chunks with budget
 *   - Moonshot Kimi K2: uses `enable_thinking` + `thinking_budget`
 *   - Qwen3/QwQ: uses `enable_thinking` + `thinking_budget`
 *
 * - **'none'**: Providers that don't support reasoning/thinking at all (Featherless)
 *
 * **Special case - OpenRouter**: This provider proxies many models with varying capabilities.
 * We show effort controls by default, but applications should fetch model metadata from
 * OpenRouter's API to check if a specific model supports reasoning via the
 * `supported_parameters` field (look for "reasoning" and "include_reasoning").
 *
 * The UI will:
 * - Show reasoning effort dropdown only for 'effort' providers
 * - Show reasoning budget for both 'effort' and 'budget-only' providers
 * - Hide entire reasoning section for 'none' providers
 */
// Reasoning Support Types
export type ReasoningSupport = "none" | "effort" | "budget-only" | "auto" | "dynamic";

export type ReasoningCapability =
  | { type: "none" } // Provider doesn't support reasoning
  | { type: "effort"; options: Array<{ value: string; label: string; description: string }> } // OpenAI-style effort levels
  | { type: "budget-only" }; // Budget-based only (like Anthropic)

export const PROVIDER_REASONING_CAPABILITIES: Record<string, ReasoningCapability> = {
  openai: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses with less reasoning" },
      { value: "medium", label: "Medium", description: "Balanced reasoning depth" },
      { value: "high", label: "High", description: "Maximum reasoning depth" },
    ],
  },
  chutes: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  openrouter: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  groq: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  deepseek: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  nanogpt: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  xai: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  anannas: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  zai: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  moonshot: { type: "budget-only" },
  anthropic: { type: "budget-only" },
  mistral: { type: "none" },
  gemini: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Minimizes latency" },
      { value: "medium", label: "Medium", description: "Balanced thinking" },
      { value: "high", label: "High", description: "Maximum reasoning" },
    ],
  },
  google: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Minimizes latency" },
      { value: "medium", label: "Medium", description: "Balanced thinking" },
      { value: "high", label: "High", description: "Maximum reasoning" },
    ],
  },
  qwen: { type: "budget-only" },
  ollama: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  lmstudio: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  custom: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  "custom-anthropic": { type: "budget-only" },
  nvidia: {
    type: "effort",
    options: [
      { value: "low", label: "Low", description: "Quick responses" },
      { value: "medium", label: "Medium", description: "Balanced" },
      { value: "high", label: "High", description: "Deep reasoning" },
    ],
  },
  featherless: { type: "none" },
};

/**
 * Get reasoning capability for a provider
 */
export function getProviderReasoningCapability(providerId: string): ReasoningCapability {
  return PROVIDER_REASONING_CAPABILITIES[providerId] ?? { type: "none" };
}

/**
 * Check if an OpenRouter model supports reasoning based on its supported_parameters
 *
 * @param supportedParameters - Array from OpenRouter API's model.supported_parameters
 * @returns true if the model supports reasoning
 *
 * Usage example:
 * ```typescript
 * const models = await invoke('get_openrouter_models');
 * const model = models.find(m => m.id === selectedModelId);
 * const supportsReasoning = checkOpenRouterModelReasoning(model.supported_parameters);
 * ```
 */
export function checkOpenRouterModelReasoning(supportedParameters: string[]): boolean {
  return (
    supportedParameters.includes("reasoning") || supportedParameters.includes("include_reasoning")
  );
}

/**
 * Get dynamic reasoning capability for OpenRouter model
 * This should be called with fresh model data from the OpenRouter API
 */
export function getOpenRouterModelReasoningCapability(
  supportedParameters: string[],
): ReasoningCapability {
  const supportsReasoning = checkOpenRouterModelReasoning(supportedParameters);

  if (supportsReasoning) {
    return {
      type: "effort",
      options: [
        { value: "low", label: "Low", description: "Quick responses" },
        { value: "medium", label: "Medium", description: "Balanced" },
        { value: "high", label: "High", description: "Deep reasoning" },
      ],
    };
  }

  return { type: "none" };
}

/**
 * Provider parameter support information
 * Documents which advanced parameters are supported by each LLM provider
 */
export const PROVIDER_PARAMETER_SUPPORT = {
  chutes: {
    providerId: "chutes",
    displayName: "Chutes",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  openai: {
    providerId: "openai",
    displayName: "OpenAI",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  openrouter: {
    providerId: "openrouter",
    displayName: "OpenRouter",
    reasoningSupport: "dynamic" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: true,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: true,
      ollamaNumPredict: true,
      ollamaNumKeep: true,
      ollamaNumBatch: true,
      ollamaNumGpu: true,
      ollamaNumThread: true,
      ollamaTfsZ: true,
      ollamaTypicalP: true,
      ollamaMinP: true,
      ollamaMirostat: true,
      ollamaMirostatTau: true,
      ollamaMirostatEta: true,
      ollamaRepeatPenalty: true,
      ollamaSeed: true,
      ollamaStop: true,
    },
  },
  anthropic: {
    providerId: "anthropic",
    displayName: "Anthropic",
    reasoningSupport: "budget-only" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: false,
      presencePenalty: false,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: false, // Uses budget-based thinking instead
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  groq: {
    providerId: "groq",
    displayName: "Groq",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  mistral: {
    providerId: "mistral",
    displayName: "Mistral",
    reasoningSupport: "none" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: false,
      reasoningEffort: false,
      reasoningBudgetTokens: false,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  google: {
    providerId: "google",
    displayName: "Google",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: false,
      presencePenalty: false,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  gemini: {
    providerId: "gemini",
    displayName: "Google Gemini",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: false,
      presencePenalty: false,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  deepseek: {
    providerId: "deepseek",
    displayName: "DeepSeek",
    reasoningSupport: "none" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: false, // R1 auto-reasons, no control
      reasoningEffort: false,
      reasoningBudgetTokens: false,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  nanogpt: {
    providerId: "nanogpt",
    displayName: "NanoGPT",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: false,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  xai: {
    providerId: "xai",
    displayName: "xAI (Grok)",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  anannas: {
    providerId: "anannas",
    displayName: "Anannas AI",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: false,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  zai: {
    providerId: "zai",
    displayName: "ZAI (Zhipu / GLM)",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: false,
      presencePenalty: false,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  moonshot: {
    providerId: "moonshot",
    displayName: "Moonshot AI (Kimi)",
    reasoningSupport: "budget-only" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: false,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  qwen: {
    providerId: "qwen",
    displayName: "Qwen",
    reasoningSupport: "budget-only" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: false,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  featherless: {
    providerId: "featherless",
    displayName: "Featherless AI",
    reasoningSupport: "none" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: false,
      reasoningEffort: false,
      reasoningBudgetTokens: false,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  ollama: {
    providerId: "ollama",
    displayName: "Ollama",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  lmstudio: {
    providerId: "lmstudio",
    displayName: "LM Studio",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  llamacpp: {
    providerId: "llamacpp",
    displayName: "llama.cpp",
    reasoningSupport: "none" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: true,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      llamaGpuLayers: true,
      llamaThreads: true,
      llamaThreadsBatch: true,
      llamaSeed: true,
      llamaRopeFreqBase: true,
      llamaRopeFreqScale: true,
      llamaOffloadKqv: true,
      reasoningEnabled: false,
      reasoningEffort: false,
      reasoningBudgetTokens: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  custom: {
    providerId: "custom",
    displayName: "Custom (OpenAI)",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  nvidia: {
    providerId: "nvidia",
    displayName: "NVIDIA NIM",
    reasoningSupport: "effort" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: false,
      reasoningEnabled: true,
      reasoningEffort: true,
      reasoningBudgetTokens: false,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
  "custom-anthropic": {
    providerId: "custom-anthropic",
    displayName: "Custom (Anthropic)",
    reasoningSupport: "budget-only" as ReasoningSupport,
    supportedParameters: {
      temperature: true,
      topP: true,
      maxOutputTokens: true,
      contextLength: false,
      frequencyPenalty: true,
      presencePenalty: true,
      topK: true,
      reasoningEnabled: true,
      reasoningEffort: false,
      reasoningBudgetTokens: true,
      llamaGpuLayers: false,
      llamaThreads: false,
      llamaThreadsBatch: false,
      llamaSeed: false,
      llamaRopeFreqBase: false,
      llamaRopeFreqScale: false,
      llamaOffloadKqv: false,
      ollamaNumCtx: false,
      ollamaNumPredict: false,
      ollamaNumKeep: false,
      ollamaNumBatch: false,
      ollamaNumGpu: false,
      ollamaNumThread: false,
      ollamaTfsZ: false,
      ollamaTypicalP: false,
      ollamaMinP: false,
      ollamaMirostat: false,
      ollamaMirostatTau: false,
      ollamaMirostatEta: false,
      ollamaRepeatPenalty: false,
      ollamaSeed: false,
      ollamaStop: false,
    },
  },
} as const;

export type ProviderId = keyof typeof PROVIDER_PARAMETER_SUPPORT;
export type ProviderParameterSupport = (typeof PROVIDER_PARAMETER_SUPPORT)[ProviderId];

/**
 * Helper function to check if a provider supports a specific parameter
 */
export function providerSupportsParameter(
  providerId: string,
  parameter: keyof AdvancedModelSettings,
): boolean {
  const provider =
    PROVIDER_PARAMETER_SUPPORT[providerId as ProviderId] ||
    (providerId === "google-gemini" ? PROVIDER_PARAMETER_SUPPORT.gemini : null) ||
    (providerId === "google" ? PROVIDER_PARAMETER_SUPPORT.gemini : null) ||
    (providerId === "chutes.ai" ? PROVIDER_PARAMETER_SUPPORT.chutes : null) ||
    (providerId === "moonshot-ai" ? PROVIDER_PARAMETER_SUPPORT.moonshot : null) ||
    (providerId === "z.ai" ? PROVIDER_PARAMETER_SUPPORT.zai : null);

  if (!provider) return false;
  return provider.supportedParameters[parameter] ?? false;
}

/**
 * Gets the reasoning support type for a specific provider
 */
export function getProviderReasoningSupport(providerId: string): ReasoningSupport {
  const provider =
    PROVIDER_PARAMETER_SUPPORT[providerId as ProviderId] ||
    (providerId === "google-gemini" ? PROVIDER_PARAMETER_SUPPORT.gemini : null) ||
    (providerId === "google" ? PROVIDER_PARAMETER_SUPPORT.gemini : null) ||
    (providerId === "chutes.ai" ? PROVIDER_PARAMETER_SUPPORT.chutes : null) ||
    (providerId === "moonshot-ai" ? PROVIDER_PARAMETER_SUPPORT.moonshot : null) ||
    (providerId === "z.ai" ? PROVIDER_PARAMETER_SUPPORT.zai : null);

  if (!provider) return "none";
  return provider.reasoningSupport;
}

/**
 * Helper function to get all supported parameters for a provider
 */

export function getSupportedParameters(providerId: string): (keyof AdvancedModelSettings)[] {
  const provider = PROVIDER_PARAMETER_SUPPORT[providerId as ProviderId];
  if (!provider) return ["temperature", "topP", "maxOutputTokens"];

  return Object.entries(provider.supportedParameters)
    .filter(([_, supported]) => supported)
    .map(([param]) => param as keyof AdvancedModelSettings);
}

export const ImageAttachmentSchema = z.object({
  id: z.string().uuid(),
  /** Base64 encoded image data or a URL - may be empty if storagePath is set (lazy loading) */
  data: z.string(),
  /** MIME type (e.g., 'image/png', 'image/jpeg') */
  mimeType: z.string(),
  /** Original filename if available */
  filename: z.string().nullish(),
  /** Width in pixels */
  width: z.number().int().nullish(),
  /** Height in pixels */
  height: z.number().int().nullish(),
  /** Relative storage path for persisted images (lazy loading) */
  storagePath: z.string().nullish(),
});
export type ImageAttachment = z.infer<typeof ImageAttachmentSchema>;

export const MessageVariantSchema = z.object({
  id: z.string().uuid(),
  content: z.string(),
  createdAt: z.number().int(),
  usage: UsageSummarySchema.optional().nullable(),
  attachments: z.array(ImageAttachmentSchema).optional(),
  /** Reasoning/thinking content from thinking models */
  reasoning: z.string().nullish(),
});
export type MessageVariant = z.infer<typeof MessageVariantSchema>;

export const MessageSchema = z.object({
  id: z.string().uuid(),
  role: z.enum(["system", "user", "assistant", "scene"]),
  content: z.string(),
  createdAt: z.number().int(),
  usage: UsageSummarySchema.optional().nullable(),
  variants: z.array(MessageVariantSchema).optional(),
  selectedVariantId: z.string().uuid().nullish(),
  isPinned: z.boolean().default(false).optional(),
  memoryRefs: z.array(z.string()).optional().default([]),
  usedLorebookEntries: z.array(z.string()).optional(),
  /** Image attachments for multimodal messages */
  attachments: z.array(ImageAttachmentSchema).optional(),
  /** Reasoning/thinking content from thinking models (not sent in API requests) */
  reasoning: z.string().nullish(),
  /** Model actually used for this message generation */
  modelId: z.string().uuid().nullish(),
  /** If set, message was generated by fallback after this primary model failed */
  fallbackFromModelId: z.string().uuid().nullish(),
});
export type StoredMessage = z.infer<typeof MessageSchema>;

// ============================================================================
// Group Chat Schemas
// ============================================================================

export const GroupMemoryEmbeddingSchema = z.object({
  id: z.string(),
  text: z.string(),
  embedding: z.array(z.number()),
  createdAt: z.number().int(),
  tokenCount: z.number().int().nonnegative().default(0),
  isCold: z.boolean().default(false),
  importanceScore: z.number().default(1.0),
  lastAccessedAt: z.number().int().default(0),
  accessCount: z.number().int().default(0),
  isPinned: z.boolean().default(false),
  category: z.string().nullable().optional(),
});
export type GroupMemoryEmbedding = z.infer<typeof GroupMemoryEmbeddingSchema>;

export const SceneVariantSchema = z.object({
  id: z.string().uuid(),
  content: z.string(),
  direction: z.string().optional(),
  createdAt: z.number().int(),
});
export type SceneVariant = z.infer<typeof SceneVariantSchema>;

export const SceneSchema = z.object({
  id: z.string().uuid(),
  content: z.string(),
  direction: z.string().optional(),
  createdAt: z.number().int(),
  variants: z.array(SceneVariantSchema).optional(),
  selectedVariantId: z.string().uuid().nullish(),
});
export type Scene = z.infer<typeof SceneSchema>;

export const DynamicMemorySettingsSchema = z.object({
  enabled: z.boolean().default(false),
  summaryMessageInterval: z.number().min(1).default(20),
  maxEntries: z.number().min(10).max(200).default(50),
  minSimilarityThreshold: z.number().min(0).max(1).default(0.35),
  retrievalLimit: z.number().min(1).max(20).default(5),
  retrievalStrategy: z.enum(["smart", "cosine"]).default("smart"),
  hotMemoryTokenBudget: z.number().min(500).max(10000).default(2000),
  decayRate: z.number().min(0.01).max(0.3).default(0.08),
  coldThreshold: z.number().min(0.1).max(0.5).default(0.3),
  contextEnrichmentEnabled: z.boolean().default(true),
});
export type DynamicMemorySettings = z.infer<typeof DynamicMemorySettingsSchema>;

export const GroupSessionSchema = z.object({
  id: z.string().uuid(),
  name: z.string(),
  characterIds: z.array(z.string().uuid()),
  personaId: z.string().uuid().nullish(),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
  /** Whether this session is archived */
  archived: z.boolean().default(false),
  /** Chat type: "conversation" or "roleplay" */
  chatType: z.enum(["conversation", "roleplay"]).default("conversation"),
  /** Starting scene for roleplay chats */
  startingScene: SceneSchema.optional().nullable(),
  /** Background image path for the group chat */
  backgroundImagePath: z.string().nullish().optional(),
  /** Manual memories (simple text entries) */
  memories: z.array(z.string()).default([]),
  /** Dynamic memory embeddings with semantic search support */
  memoryEmbeddings: z.array(GroupMemoryEmbeddingSchema).default([]),
  /** Summary of memories for context compression */
  memorySummary: z.string().default(""),
  /** Token count of the memory summary */
  memorySummaryTokenCount: z.number().int().default(0),
  /** Speaker selection method for group chat */
  speakerSelectionMethod: z
    .enum(["llm", "heuristic", "round_robin"])
    .default("llm"),
  /** Memory tool events tracking (for dynamic memory cycle gating) */
  memoryToolEvents: z
    .array(
      z.object({
        type: z.string().optional(),
        windowEnd: z.number().int().optional(),
        timestamp: z.number().int().optional(),
        memoriesCount: z.number().int().optional(),
        // Also support the full format used in normal sessions
        id: z.string().optional(),
        windowStart: z.number().int().optional(),
        summary: z.string().optional(),
        error: z.string().optional(),
        status: z.string().optional(),
        stage: z.string().optional(),
        windowMessageIds: z.array(z.string()).optional(),
        actions: z
          .array(
            z.object({
              name: z.string(),
              arguments: z.any().optional(),
              timestamp: z.number().int().optional(),
              updatedMemories: z.array(z.string()).optional(),
            }),
          )
          .optional(),
        createdAt: z.number().int().optional(),
      }),
    )
    .default([]),
});
export type GroupSession = z.infer<typeof GroupSessionSchema>;

export const GroupParticipationSchema = z.object({
  id: z.string().uuid(),
  sessionId: z.string().uuid(),
  characterId: z.string().uuid(),
  speakCount: z.number().int().default(0),
  lastSpokeTurn: z.number().int().nullish(),
  lastSpokeAt: z.number().int().nullish(),
});
export type GroupParticipation = z.infer<typeof GroupParticipationSchema>;

export const GroupMessageVariantSchema = z.object({
  id: z.string().uuid(),
  content: z.string(),
  speakerCharacterId: z.string().uuid().nullish(),
  createdAt: z.number().int(),
  usage: UsageSummarySchema.optional().nullable(),
  reasoning: z.string().nullish(),
  selectionReasoning: z.string().nullish(),
  modelId: z.string().uuid().nullish(),
});
export type GroupMessageVariant = z.infer<typeof GroupMessageVariantSchema>;

export const GroupMessageSchema = z.object({
  id: z.string().uuid(),
  sessionId: z.string().uuid(),
  role: z.enum(["user", "assistant", "scene"]),
  content: z.string(),
  speakerCharacterId: z.string().uuid().nullish(),
  turnNumber: z.number().int(),
  createdAt: z.number().int(),
  usage: UsageSummarySchema.optional().nullable(),
  variants: z.array(GroupMessageVariantSchema).optional(),
  selectedVariantId: z.string().uuid().nullish(),
  isPinned: z.boolean().optional(),
  attachments: z.array(ImageAttachmentSchema).default([]),
  reasoning: z.string().nullish(),
  selectionReasoning: z.string().nullish(),
  modelId: z.string().uuid().nullish(),
});
export type GroupMessage = z.infer<typeof GroupMessageSchema>;

export const GroupSessionPreviewSchema = z.object({
  id: z.string().uuid(),
  name: z.string(),
  characterIds: z.array(z.string().uuid()),
  updatedAt: z.number().int(),
  lastMessage: z.string().nullish(),
  messageCount: z.number().int(),
  archived: z.boolean().default(false),
  chatType: z.enum(["conversation", "roleplay"]).default("conversation"),
});
export type GroupSessionPreview = z.infer<typeof GroupSessionPreviewSchema>;

export const GroupChatResponseSchema = z.object({
  message: GroupMessageSchema,
  characterId: z.string().uuid(),
  characterName: z.string(),
  reasoning: z.string().nullish(),
  selectionReasoning: z.string().nullish(),
  wasMentioned: z.boolean(),
  participationStats: z.array(GroupParticipationSchema),
});
export type GroupChatResponse = z.infer<typeof GroupChatResponseSchema>;

export const ProviderCredentialSchema = z.object({
  id: z.string().uuid(),
  providerId: z.string(),
  label: z.string().min(1),
  apiKey: z.string().optional(),
  baseUrl: z.string().optional(),
  defaultModel: z.string().optional(),
  headers: z.record(z.string()).optional(),
  config: z.record(z.any()).optional(),
});
export type ProviderCredential = z.infer<typeof ProviderCredentialSchema>;

export const ModelScopeSchema = z.enum(["text", "image", "audio"]);
export type ModelScope = z.infer<typeof ModelScopeSchema>;

function normalizeModelScopes(value: unknown): ModelScope[] {
  const scopeOrder: ModelScope[] = ["text", "image", "audio"];
  const fromValue = Array.isArray(value) ? value : [];
  const set = new Set<ModelScope>();
  for (const item of fromValue) {
    if (item === "text" || item === "image" || item === "audio") set.add(item);
  }
  set.add("text");
  return scopeOrder.filter((s) => set.has(s));
}

const ModelScopesSchema = z
  .preprocess((v) => (v == null ? ["text"] : v), z.array(ModelScopeSchema))
  .transform((scopes) => normalizeModelScopes(scopes));

export const ModelSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  providerId: z.string(),
  providerLabel: z.string().min(1),
  displayName: z.string().min(1),
  createdAt: z.number().int(),
  // Input/output modality scopes for chat models. Text is always enabled.
  inputScopes: ModelScopesSchema,
  outputScopes: ModelScopesSchema,
  advancedModelSettings: AdvancedModelSettingsSchema.nullish().optional(),
  promptTemplateId: z.string().nullish().optional(),
  systemPrompt: z.string().nullish().optional(), // Deprecated
});
export type Model = z.infer<typeof ModelSchema>;

export const OnboardingStateSchema = z.object({
  completed: z.boolean(),
  skipped: z.boolean(),
  providerSetupCompleted: z.boolean(),
  modelSetupCompleted: z.boolean(),
});
export type OnboardingState = z.infer<typeof OnboardingStateSchema>;

export function createDefaultOnboardingState(): OnboardingState {
  return {
    completed: false,
    skipped: false,
    providerSetupCompleted: false,
    modelSetupCompleted: false,
  };
}

export const TooltipsStateSchema = z.record(z.boolean());
export type TooltipsState = z.infer<typeof TooltipsStateSchema>;

export const PureModeLevelSchema = z.enum(["off", "low", "standard", "strict"]);
export type PureModeLevel = z.infer<typeof PureModeLevelSchema>;

export const AppStateSchema = z.object({
  onboarding: OnboardingStateSchema,
  theme: z.enum(["light", "dark"]),
  tooltips: TooltipsStateSchema,
  pureModeEnabled: z.boolean().default(true), // Kept for backward compat
  pureModeLevel: PureModeLevelSchema.default("standard"),
  autoDownloadCharacterCardAvatars: z.boolean().default(true),
  analyticsEnabled: z.boolean().default(true),
  appActiveUsageMs: z.number().int().nonnegative().default(0),
  appActiveUsageByDayMs: z.record(z.number().int().nonnegative()).default({}),
  appActiveUsageStartedAtMs: z.number().int().nonnegative().optional(),
  appActiveUsageLastUpdatedAtMs: z.number().int().nonnegative().optional(),
});
export type AppState = z.infer<typeof AppStateSchema>;

export const AccessibilitySoundSchema = z.object({
  enabled: z.boolean().default(false),
  volume: z.number().min(0).max(1).default(0.6),
});
export type AccessibilitySound = z.infer<typeof AccessibilitySoundSchema>;

export const AccessibilitySettingsSchema = z.object({
  send: AccessibilitySoundSchema.default({ enabled: false, volume: 0.5 }),
  success: AccessibilitySoundSchema.default({ enabled: false, volume: 0.6 }),
  failure: AccessibilitySoundSchema.default({ enabled: false, volume: 0.6 }),
  haptics: z.boolean().default(false),
  hapticIntensity: z.enum(["light", "medium", "heavy", "soft", "rigid"]).default("light"),
});
export type AccessibilitySettings = z.infer<typeof AccessibilitySettingsSchema>;

export function createDefaultAccessibilitySettings(): AccessibilitySettings {
  return {
    send: { enabled: false, volume: 0.5 },
    success: { enabled: false, volume: 0.6 },
    failure: { enabled: false, volume: 0.6 },
    haptics: false,
    hapticIntensity: "light",
  };
}

export function createDefaultAppState(): AppState {
  return {
    onboarding: createDefaultOnboardingState(),
    theme: "light",
    tooltips: {},
    pureModeEnabled: true,
    pureModeLevel: "standard",
    autoDownloadCharacterCardAvatars: true,
    analyticsEnabled: true,
    appActiveUsageMs: 0,
    appActiveUsageByDayMs: {},
  };
}

export const SettingsSchema = z.object({
  $version: z.literal(2),
  defaultProviderCredentialId: z.string().uuid().nullable(),
  defaultModelId: z.string().uuid().nullable(),
  providerCredentials: z.array(ProviderCredentialSchema),
  models: z.array(ModelSchema),
  appState: AppStateSchema,
  advancedSettings: z
    .object({
      summarisationModelId: z.string().optional(),
      creationHelperEnabled: z.boolean().optional(),
      creationHelperModelId: z.string().optional(),
      creationHelperStreaming: z.boolean().optional(),
      creationHelperImageModelId: z.string().optional(),
      creationHelperSmartToolSelection: z.boolean().optional(),
      creationHelperEnabledTools: z.array(z.string()).optional(),
      helpMeReplyEnabled: z.boolean().optional(),
      helpMeReplyModelId: z.string().optional(),
      helpMeReplyStreaming: z.boolean().optional(),
      helpMeReplyMaxTokens: z.number().optional(),
      helpMeReplyStyle: z.enum(["conversational", "roleplay"]).optional(),
      manualModeContextWindow: z.number().optional(),
      embeddingMaxTokens: z.number().optional(), // 1024, 2048, or 4096
      embeddingModelVersion: z.enum(["v2", "v3"]).optional(),
      embeddingKeepModelLoaded: z.boolean().optional(),
      dynamicMemory: DynamicMemorySettingsSchema.optional(),
      groupDynamicMemory: DynamicMemorySettingsSchema.optional(),
      accessibility: AccessibilitySettingsSchema.optional(),
    })
    .optional(),
  promptTemplateId: z.string().nullish().optional(),
  systemPrompt: z.string().nullish().optional(), // Deprecated
  migrationVersion: z.number().int().default(0),
});
export type Settings = z.infer<typeof SettingsSchema>;

export function createDefaultSettings(): Settings {
  return {
    $version: 2,
    defaultProviderCredentialId: null,
    defaultModelId: null,
    providerCredentials: [],
    models: [],
    appState: createDefaultAppState(),
    advancedSettings: {
      creationHelperEnabled: false,
      helpMeReplyEnabled: true,
      accessibility: createDefaultAccessibilitySettings(),
    },
    promptTemplateId: null,
    systemPrompt: null,
    migrationVersion: 0,
  };
}

export const LorebookSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});

export type Lorebook = z.infer<typeof LorebookSchema>;

export const LorebookEntrySchema = z.object({
  id: z.string().uuid(),
  lorebookId: z.string().uuid(),
  title: z.string().default(""),
  enabled: z.boolean().default(true),
  alwaysActive: z.boolean().default(false),
  keywords: z.array(z.string()).default([]),
  caseSensitive: z.boolean().default(false),
  content: z.string(),
  priority: z.number().int().default(0),
  displayOrder: z.number().int().default(0),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});

export type LorebookEntry = z.infer<typeof LorebookEntrySchema>;

export const CharacterVoiceConfigSchema = z.object({
  source: z.enum(["user", "provider"]),
  userVoiceId: z.string().optional(),
  providerId: z.string().optional(),
  voiceId: z.string().optional(),
  modelId: z.string().optional(),
  voiceName: z.string().optional(),
});
export type CharacterVoiceConfig = z.infer<typeof CharacterVoiceConfigSchema>;

export const AvatarCropSchema = z.object({
  x: z.number(),
  y: z.number(),
  scale: z.number(),
});
export type AvatarCrop = z.infer<typeof AvatarCropSchema>;

export const CharacterSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  nickname: z.string().optional(),
  avatarPath: z.string().optional(),
  avatarCrop: AvatarCropSchema.optional(),
  backgroundImagePath: z.string().optional(),
  definition: z.string().optional(),
  description: z.string().optional(),
  scenario: z.string().optional(),
  creatorNotes: z.string().optional(),
  creator: z.string().optional(),
  creatorNotesMultilingual: z.record(z.string(), z.any()).nullable().optional(),
  source: z.array(z.string()).optional(),
  tags: z.array(z.string()).optional(),
  rules: z.array(z.string()).default([]),
  scenes: z.array(SceneSchema).default([]),
  defaultSceneId: z.string().uuid().nullish(),
  defaultModelId: z.string().uuid().nullable().optional(),
  fallbackModelId: z.string().uuid().nullable().optional(),
  memoryType: z.enum(["manual", "dynamic"]).default("manual"),
  promptTemplateId: z.string().nullish().optional(),
  disableAvatarGradient: z.boolean().default(false).optional(),
  customGradientEnabled: z.boolean().default(false).optional(),
  customGradientColors: z.array(z.string()).optional(), // Array of hex colors, e.g. ["#ff6b6b", "#4ecdc4"]
  customTextColor: z.string().optional(), // Custom text color hex
  customTextSecondary: z.string().optional(), // Custom secondary text color hex
  voiceConfig: CharacterVoiceConfigSchema.optional(),
  voiceAutoplay: z.boolean().default(false).optional(),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type Character = z.infer<typeof CharacterSchema>;

export const SessionSchema = z.object({
  id: z.string().uuid(),
  characterId: z.string().uuid(),
  title: z.string(),
  selectedSceneId: z.string().uuid().nullish(), // ID of the scene from character.scenes array
  personaId: z.union([z.string().uuid(), z.literal(""), z.null(), z.undefined()]).optional(),
  personaDisabled: z.boolean().optional().default(false),
  voiceAutoplay: z.boolean().nullable().optional(),
  advancedModelSettings: AdvancedModelSettingsSchema.nullish().optional(),
  memories: z.array(z.string()).default([]),
  memoryEmbeddings: z
    .array(
      z.object({
        id: z.string(),
        text: z.string(),
        embedding: z.array(z.number()),
        createdAt: z.number().int(),
        tokenCount: z.number().int().nonnegative().default(0),
        isCold: z.boolean().default(false),
        importanceScore: z.number().default(1.0),
        lastAccessedAt: z.number().int().default(0),
        isPinned: z.boolean().default(false),
        category: z.string().nullable().optional(),
      }),
    )
    .default([])
    .optional(),
  memorySummary: z.string().default("").optional(),
  memorySummaryTokenCount: z.number().default(0),
  memoryToolEvents: z
    .array(
      z.object({
        id: z.string(),
        windowStart: z.number().int(),
        windowEnd: z.number().int(),
        windowMessageIds: z.array(z.string()).optional(),
        summary: z.string(),
        error: z.string().optional(),
        status: z.string().optional(),
        stage: z.string().optional(),
        actions: z.array(
          z.object({
            name: z.string(),
            arguments: z.any().optional(),
            timestamp: z.number().int().optional(),
            updatedMemories: z.array(z.string()).optional(),
          }),
        ),
        createdAt: z.number().int(),
      }),
    )
    .default([])
    .optional(),
  messages: z.array(MessageSchema),
  memoryStatus: z.string().nullish().optional().default("idle"),
  memoryError: z.string().nullish().optional(),
  archived: z.boolean().default(false),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type Session = z.infer<typeof SessionSchema>;

export const PersonaSchema = z.object({
  id: z.string().uuid(),
  title: z.string().min(1),
  description: z.string().min(1),
  avatarPath: z.string().optional(),
  avatarCrop: AvatarCropSchema.optional(),
  isDefault: z.boolean().default(false),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type Persona = z.infer<typeof PersonaSchema>;

export function createDefaultAdvancedModelSettings(): AdvancedModelSettings {
  return {
    temperature: 0.7,
    topP: 1,
    maxOutputTokens: 1024,
  };
}
